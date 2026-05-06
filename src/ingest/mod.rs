//! Ingest module - Upload files to the lakehouse
//!
//! Implements content-addressed storage with Iceberg catalog integration.

use crate::cli::IngestArgs;
use crate::domain::{ContentHash, FileCategory, FileInfo};
use crate::lakehouse::{writer, LakehouseConfig};
use crate::scan::scan_file;
use crate::storage;
use anyhow::{Context, Result};
use chrono::Utc;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use opendal::Operator;
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

/// Result of processing a single file during ingest.
enum IngestOutcome {
    Uploaded(Box<FileInfo>),
    AlreadyExists,
}

/// Run the ingest command
pub async fn run(args: IngestArgs) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    let config = LakehouseConfig::default();

    println!();
    println!(
        "{}",
        style("═══════════════════════════════════════════════════════════════").cyan()
    );
    println!("  📤 Anti-Entropator Ingest");
    println!(
        "{}",
        style("═══════════════════════════════════════════════════════════════").cyan()
    );
    println!();
    println!("  Source:  {}", path.display());
    println!("  Target:  {}", config.warehouse);
    println!("  Dry run: {}", if args.dry_run { "yes" } else { "no" });
    println!();

    // Check lakehouse connectivity first (unless dry-run)
    if !args.dry_run {
        print!("  Checking lakehouse connectivity... ");
        match check_connectivity(&config).await {
            Ok(_) => println!("{}", style("OK").green()),
            Err(e) => {
                println!("{}", style("FAILED").red());
                anyhow::bail!(
                    "Cannot connect to lakehouse: {}. Run `docker compose up -d`",
                    e
                );
            }
        }
    }

    // Collect files to ingest
    let files = collect_files(&path, &args)?;
    println!("  Found {} files to ingest", files.len());

    if files.is_empty() {
        println!("\n  Nothing to ingest.");
        return Ok(());
    }

    // Create progress bar
    let pb = ProgressBar::new(files.len() as u64);
    let pb_style = ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .context("Invalid ingest progress bar template")?
        .progress_chars("#>-");
    pb.set_style(pb_style);

    let mut uploaded_files = Vec::new();
    let mut uploaded_count = 0u64;
    let mut exists_count = 0u64;
    let mut errors = Vec::new();
    let mut total_bytes = 0u64;

    // Create OpenDAL operator (only if not dry-run)
    let operator = if !args.dry_run {
        Some(storage::create_operator(&config)?)
    } else {
        None
    };

    for file_path in &files {
        pb.inc(1);
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        pb.set_message(file_name.clone());

        match process_file(file_path, &path, &config, operator.as_ref(), args.dry_run).await {
            Ok(IngestOutcome::Uploaded(info)) => {
                uploaded_count += 1;
                total_bytes += info.size_bytes;
                uploaded_files.push(*info);
            }
            Ok(IngestOutcome::AlreadyExists) => {
                exists_count += 1;
            }
            Err(e) => {
                errors.push(format!("{}: {}", file_path.display(), e));
            }
        }
    }

    pb.finish_and_clear();

    // Commit to Iceberg if not dry-run and there are new uploads
    let commit_result = if !args.dry_run && !uploaded_files.is_empty() {
        print!("  Committing metadata to Iceberg catalog... ");
        match writer::commit_files(uploaded_files, &config).await {
            Ok(_) => {
                println!("{}", style("OK").green());
                Some(Ok(()))
            }
            Err(e) => {
                println!("{}", style("FAILED").red());
                Some(Err(e))
            }
        }
    } else {
        None
    };

    finalize_ingest(
        commit_result,
        uploaded_count,
        exists_count,
        &errors,
        total_bytes,
        args.dry_run,
    )
}

/// Finalize the ingest operation: print summary and determine command outcome.
fn finalize_ingest(
    commit_result: Option<Result<()>>,
    uploaded_count: u64,
    exists_count: u64,
    errors: &[String],
    total_bytes: u64,
    dry_run: bool,
) -> Result<()> {
    println!();
    println!("─── Ingest Results ─────────────────────────────────────────────");
    println!();
    if dry_run {
        println!(
            "  Would upload:    {} files ({})",
            uploaded_count,
            humansize::format_size(total_bytes, humansize::BINARY)
        );
    } else {
        println!(
            "  Uploaded:        {} files ({})",
            uploaded_count,
            humansize::format_size(total_bytes, humansize::BINARY)
        );
    }
    println!("  Already in store: {} files", exists_count);
    println!("  Errors:          {} files", errors.len());
    println!();

    if !errors.is_empty() {
        println!("  Errors:");
        for err in errors.iter().take(5) {
            println!("    - {}", err);
        }
        if errors.len() > 5 {
            println!("    ... and {} more", errors.len() - 5);
        }
        println!();
    }

    if dry_run {
        println!(
            "{}",
            style("  Dry run - no files were uploaded. Remove --dry-run to actually ingest.").dim()
        );
        println!();
        return Ok(());
    }

    match commit_result {
        Some(Err(e)) => {
            println!(
                "{}",
                style("  Ingest incomplete: metadata commit failed.").red()
            );
            println!("  Objects may have been uploaded but are not registered in the catalog.");
            println!();
            Err(e.context("metadata commit failed"))
        }
        _ => {
            if errors.is_empty() {
                println!("{}", style("  Files ingested successfully!").green());
            } else {
                println!(
                    "{}",
                    style("  Ingest completed with errors (see above).").yellow()
                );
            }
            println!();
            println!("  Next steps:");
            println!("    1. Run `anti_entropator query` to explore your catalog");
            println!("    2. Run `anti_entropator duplicates` to find duplicate files");
            println!();
            Ok(())
        }
    }
}

/// Check lakehouse connectivity
async fn check_connectivity(config: &LakehouseConfig) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // Check RustFS
    client
        .get(&config.s3_endpoint)
        .send()
        .await
        .context("Cannot connect to RustFS")?;

    Ok(())
}

/// Collect files to ingest based on filters
fn collect_files(path: &Path, args: &IngestArgs) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let mut type_filter: Option<HashSet<String>> = None;

    // Parse glob patterns upfront (fail early on invalid syntax)
    let exclude_patterns: Vec<glob::Pattern> = args
        .exclude
        .iter()
        .map(|p| {
            glob::Pattern::new(p).with_context(|| format!("invalid exclude glob pattern: '{}'", p))
        })
        .collect::<Result<Vec<_>>>()?;
    let include_patterns: Vec<glob::Pattern> = args
        .include
        .iter()
        .map(|p| {
            glob::Pattern::new(p).with_context(|| format!("invalid include glob pattern: '{}'", p))
        })
        .collect::<Result<Vec<_>>>()?;

    // Parse type filter
    if !args.types.is_empty() {
        type_filter = Some(args.types.iter().map(|t| t.to_lowercase()).collect());
    }

    // Parse max size
    let max_size: Option<u64> = args.max_size.as_ref().and_then(|s| parse_size(s));

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;

        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();
        let filename = file_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        // Apply exclude patterns (match against filename)
        if exclude_patterns.iter().any(|pat| pat.matches(&filename)) {
            continue;
        }

        // Apply include patterns (match against filename)
        if !include_patterns.is_empty()
            && !include_patterns.iter().any(|pat| pat.matches(&filename))
        {
            continue;
        }

        // Apply type filter
        if let Some(ref types) = type_filter {
            let ext = file_path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            let category = FileCategory::from_extension(&ext).to_string();

            if !types.contains(&category) && !types.contains(&ext) {
                continue;
            }
        }

        // Apply size filter
        if let Some(max) = max_size {
            if let Ok(metadata) = entry.metadata() {
                if metadata.len() > max {
                    continue;
                }
            }
        }

        files.push(file_path.to_path_buf());

        // Apply limit
        if let Some(limit) = args.limit {
            if files.len() >= limit {
                break;
            }
        }
    }

    Ok(files)
}

/// Process a single file: scan, hash, upload, return outcome.
async fn process_file(
    path: &Path,
    root_path: &Path,
    config: &LakehouseConfig,
    operator: Option<&Operator>,
    dry_run: bool,
) -> Result<IngestOutcome> {
    // 1. Scan file for initial metadata
    let mut info = scan_file(path).await?;

    // 2. Set parent directory (relative to root)
    if let Ok(relative) = path.strip_prefix(root_path) {
        if let Some(parent) = relative.parent() {
            info = info.with_parent_dir(parent.to_string_lossy().to_string());
        }
    }

    // 3. Compute object key based on content hash
    let hash = if let Some(ref h) = info.content_hash {
        h.0.clone()
    } else {
        let h = compute_hash(path)?;
        info.content_hash = Some(ContentHash::new(h.clone()));
        h
    };

    let content_hash = ContentHash::new(hash);
    let object_key = content_hash.to_object_key();
    let object_uri = format!("s3://{}/{}", config.bucket, object_key);
    info.object_uri = Some(object_uri);

    if dry_run {
        return Ok(IngestOutcome::Uploaded(Box::new(info)));
    }

    let op = operator.ok_or_else(|| anyhow::anyhow!("Storage operator not available"))?;

    // 4. Check if object already exists
    let exists = op
        .exists(&object_key)
        .await
        .context("Failed to check object existence")?;

    if exists {
        return Ok(IngestOutcome::AlreadyExists);
    }

    // 5. Upload file
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    op.write(&object_key, bytes)
        .await
        .context("Failed to upload to storage")?;

    info.ingested_at = Some(Utc::now());

    Ok(IngestOutcome::Uploaded(Box::new(info)))
}

/// Compute SHA-256 hash of a file (fallback if scan didn't do it)
fn compute_hash(path: &Path) -> Result<String> {
    use sha2::Digest;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = std::io::Read::read(&mut file, &mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Parse size string like "100MB", "1GB"
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim().to_uppercase();

    let (num_str, multiplier) = if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024)
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024 * 1024)
    } else if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024)
    } else if s.ends_with("B") {
        (&s[..s.len() - 1], 1)
    } else {
        return None;
    };

    num_str.trim().parse::<u64>().ok().map(|n| n * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::IngestArgs;
    use std::path::PathBuf;

    fn default_args(path: PathBuf) -> IngestArgs {
        IngestArgs {
            path,
            include: vec![],
            exclude: vec![],
            types: vec![],
            max_size: None,
            limit: None,
            dry_run: true,
        }
    }

    // ── parse_size ──

    #[test]
    fn parse_size_megabytes() {
        assert_eq!(parse_size("1MB"), Some(1024 * 1024));
    }

    #[test]
    fn parse_size_kilobytes() {
        assert_eq!(parse_size("100KB"), Some(100 * 1024));
    }

    #[test]
    fn parse_size_gigabytes() {
        assert_eq!(parse_size("2GB"), Some(2 * 1024 * 1024 * 1024));
    }

    #[test]
    fn parse_size_bytes() {
        assert_eq!(parse_size("500B"), Some(500));
    }

    #[test]
    fn parse_size_zero() {
        assert_eq!(parse_size("0MB"), Some(0));
    }

    #[test]
    fn parse_size_with_whitespace() {
        assert_eq!(parse_size("  10 MB  "), Some(10 * 1024 * 1024));
    }

    #[test]
    fn parse_size_lowercase() {
        assert_eq!(parse_size("5mb"), Some(5 * 1024 * 1024));
    }

    #[test]
    fn parse_size_invalid_suffix() {
        assert_eq!(parse_size("100TB"), None);
    }

    #[test]
    fn parse_size_no_suffix() {
        assert_eq!(parse_size("100"), None);
    }

    #[test]
    fn parse_size_no_number() {
        assert_eq!(parse_size("MB"), None);
    }

    #[test]
    fn parse_size_garbage() {
        assert_eq!(parse_size("abc"), None);
    }

    // ── collect_files ──

    fn make_test_tree(dir: &std::path::Path) {
        std::fs::write(dir.join("photo.jpg"), b"fake jpg").unwrap();
        std::fs::write(dir.join("readme.txt"), b"hello world").unwrap();
        std::fs::write(dir.join("data.csv"), b"a,b,c").unwrap();
        std::fs::write(dir.join("debug.log"), b"log line").unwrap();
        let sub = dir.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("nested.rs"), b"fn main() {}").unwrap();
    }

    #[test]
    fn collect_files_no_filters() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let args = default_args(dir.path().to_path_buf());

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn collect_files_exclude_pattern() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.exclude = vec!["*.log".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 4);
        assert!(!files.iter().any(|f| f.to_string_lossy().contains(".log")));
    }

    #[test]
    fn collect_files_include_pattern() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["*.txt".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("readme.txt"));
    }

    #[test]
    fn collect_files_limit() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.limit = Some(2);

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn collect_files_max_size() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("small.txt"), b"hi").unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![0u8; 2048]).unwrap();
        let mut args = default_args(dir.path().to_path_buf());
        args.max_size = Some("1KB".to_string());

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("small.txt"));
    }

    #[test]
    fn collect_files_type_filter_by_category() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.types = vec!["image".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("photo.jpg"));
    }

    #[test]
    fn collect_files_type_filter_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.types = vec!["rs".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("nested.rs"));
    }

    #[test]
    fn collect_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let args = default_args(dir.path().to_path_buf());
        let files = collect_files(dir.path(), &args).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn collect_files_combined_filters() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.exclude = vec!["*.rs".to_string()];
        args.limit = Some(3);

        let files = collect_files(dir.path(), &args).unwrap();
        assert!(files.len() <= 3);
        assert!(!files.iter().any(|f| f.to_string_lossy().ends_with(".rs")));
    }

    #[test]
    fn collect_files_include_glob_star() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["*.jpg".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("photo.jpg"));
    }

    #[test]
    fn collect_files_exclude_glob_star() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.exclude = vec!["*.rs".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 4);
        assert!(!files.iter().any(|f| f.to_string_lossy().ends_with(".rs")));
    }

    #[test]
    fn collect_files_include_matches_nested_filename() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["*.rs".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("nested.rs"));
    }

    #[test]
    fn collect_files_glob_does_not_substring_match() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["files".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert!(
            files.is_empty(),
            "bare word should not substring-match filenames"
        );
    }

    #[test]
    fn collect_files_invalid_glob_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["[invalid".to_string()];

        let result = collect_files(dir.path(), &args);
        assert!(result.is_err());
    }

    // ── compute_hash ──

    #[test]
    fn compute_hash_known_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, b"hello world").unwrap();

        let hash = compute_hash(&path).unwrap();
        // SHA-256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn compute_hash_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.bin");
        std::fs::write(&path, b"").unwrap();

        let hash = compute_hash(&path).unwrap();
        // SHA-256 of empty input
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn compute_hash_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("det.bin");
        std::fs::write(&path, b"reproducible content").unwrap();

        let hash1 = compute_hash(&path).unwrap();
        let hash2 = compute_hash(&path).unwrap();
        assert_eq!(hash1, hash2);
    }

    // ── IngestOutcome tally tests ──

    fn make_test_info(name: &str, size: u64) -> FileInfo {
        use crate::domain::FileCategory;
        FileInfo {
            id: uuid::Uuid::new_v4(),
            source_path: std::path::PathBuf::from(format!("/test/{}", name)),
            filename: name.to_string(),
            extension: "txt".to_string(),
            mime_type: None,
            category: FileCategory::Document,
            size_bytes: size,
            content_hash: None,
            partial_hash: None,
            created_at: None,
            modified_at: None,
            scanned_at: chrono::Utc::now(),
            object_uri: None,
            ingested_at: Some(chrono::Utc::now()),
            suggested_name: None,
            name_reason: None,
            is_duplicate: false,
            duplicate_of: None,
            parent_dir: String::new(),
            group_id: None,
        }
    }

    /// Fold outcomes into counts and commit batch (pure, no I/O).
    fn tally_outcomes(outcomes: Vec<IngestOutcome>) -> (Vec<FileInfo>, u64, u64, u64) {
        let mut uploaded_files = Vec::new();
        let mut uploaded_count = 0u64;
        let mut exists_count = 0u64;
        let mut total_bytes = 0u64;
        for outcome in outcomes {
            match outcome {
                IngestOutcome::Uploaded(info) => {
                    uploaded_count += 1;
                    total_bytes += info.size_bytes;
                    uploaded_files.push(*info);
                }
                IngestOutcome::AlreadyExists => {
                    exists_count += 1;
                }
            }
        }
        (uploaded_files, uploaded_count, exists_count, total_bytes)
    }

    #[test]
    fn tally_mixed_outcomes() {
        let outcomes = vec![
            IngestOutcome::Uploaded(Box::new(make_test_info("a.txt", 100))),
            IngestOutcome::Uploaded(Box::new(make_test_info("b.txt", 200))),
            IngestOutcome::AlreadyExists,
        ];
        let (files, uploaded, exists, bytes) = tally_outcomes(outcomes);
        assert_eq!(uploaded, 2);
        assert_eq!(exists, 1);
        assert_eq!(files.len(), 2);
        assert_eq!(bytes, 300);
    }

    #[test]
    fn tally_all_exists() {
        let outcomes = vec![
            IngestOutcome::AlreadyExists,
            IngestOutcome::AlreadyExists,
            IngestOutcome::AlreadyExists,
        ];
        let (files, uploaded, exists, _bytes) = tally_outcomes(outcomes);
        assert_eq!(uploaded, 0);
        assert_eq!(exists, 3);
        assert!(files.is_empty());
    }

    // ── finalize_ingest tests ──

    #[test]
    fn finalize_commit_success() {
        let result = finalize_ingest(Some(Ok(())), 3, 1, &[], 1024, false);
        assert!(result.is_ok());
    }

    #[test]
    fn finalize_commit_failure() {
        let err = anyhow::anyhow!("catalog connection refused");
        let result = finalize_ingest(Some(Err(err)), 3, 0, &[], 1024, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata commit"));
    }

    #[test]
    fn finalize_no_commit() {
        let result = finalize_ingest(None, 0, 5, &[], 0, false);
        assert!(result.is_ok());
    }
}
