//! Ingest module - Upload files to the lakehouse
//!
//! Implements content-addressed storage with Iceberg catalog integration.

mod output;
mod upload;

use crate::cli::{IngestArgs, IngestOutputFormat};
use crate::domain::observation::normalize_relative_path;
use crate::domain::{ContentHash, FileCategory, FileInfo, ObservationStatus};
use crate::file_hash;
use crate::lakehouse::{writer, LakehouseConfig};
use crate::scan::scan_file;
use crate::storage;
use anyhow::{Context, Result};
use chrono::Utc;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use opendal::Operator;
use output::{
    catalog_commit_from_result, outcome_error, print_human_report, print_json_report, IngestMode,
    IngestSummary,
};
use std::collections::HashSet;
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

/// Result of processing a single file during ingest.
enum IngestOutcome {
    /// A new object: uploaded in ingest mode, or "would upload" in preview modes.
    Uploaded(Box<FileInfo>),
    /// The content-addressed object already exists in the store.
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
    let json_output = args.format == IngestOutputFormat::Json;
    let mode = IngestMode::from_flags(args.dry_run, args.offline);
    let ctx = RunContext::new(&path);
    tracing::info!(run_id = %ctx.run_id, source_id = %ctx.source_id, mode = mode.label(), "Starting ingest run");

    if !json_output {
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
        println!("  Mode:    {}", mode.label());
        println!("  Run:     {}", ctx.run_id);
        println!();
    }

    // Check lakehouse connectivity first (skipped only by --dry-run --offline)
    if mode.is_connected() {
        if !json_output {
            print!("  Checking lakehouse connectivity... ");
        }
        match check_connectivity(&config).await {
            Ok(_) => {
                if !json_output {
                    println!("{}", style("OK").green());
                }
            }
            Err(e) => {
                if !json_output {
                    println!("{}", style("FAILED").red());
                }
                anyhow::bail!(
                    "Cannot connect to lakehouse: {}. Run `docker compose up -d`",
                    e
                );
            }
        }
    }

    // Collect files to ingest
    let files = collect_files(&path, &args)?;
    tracing::info!(
        candidates = files.len(),
        mode = mode.label(),
        "Collected ingest candidates"
    );
    if !json_output {
        println!("  Found {} files to ingest", files.len());
    }

    if files.is_empty() {
        return finalize_ingest(
            IngestSummary::build(
                mode,
                ctx.run_id,
                0,
                0,
                0,
                0,
                &[],
                catalog_commit_from_result(&None),
            ),
            None,
            args.format,
        );
    }

    // Create progress bar (stderr). Hide it in JSON mode so stdout stays one document.
    let pb = if json_output {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(files.len() as u64);
        let pb_style = ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .context("Invalid ingest progress bar template")?
            .progress_chars("#>-");
        pb.set_style(pb_style);
        pb
    };

    let mut uploaded_files = Vec::new();
    let mut uploaded_count = 0u64;
    let mut exists_count = 0u64;
    let mut errors = Vec::new();
    let mut total_bytes = 0u64;

    // Create OpenDAL operator (needed for existence checks and uploads)
    let operator = if mode.is_connected() {
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

        match process_file(file_path, &path, &config, operator.as_ref(), mode, &ctx).await {
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

    // Commit to Iceberg only in ingest mode and only when something was uploaded
    let commit_result = if mode.writes() && !uploaded_files.is_empty() {
        if !json_output {
            print!("  Committing metadata to Iceberg catalog... ");
        }
        match writer::commit_files(uploaded_files, &config).await {
            Ok(_) => {
                if !json_output {
                    println!("{}", style("OK").green());
                }
                Some(Ok(()))
            }
            Err(e) => {
                if !json_output {
                    println!("{}", style("FAILED").red());
                }
                Some(Err(e))
            }
        }
    } else {
        None
    };

    let catalog_commit = catalog_commit_from_result(&commit_result);
    finalize_ingest(
        IngestSummary::build(
            mode,
            ctx.run_id,
            files.len() as u64,
            uploaded_count,
            exists_count,
            total_bytes,
            &errors,
            catalog_commit,
        ),
        commit_result,
        args.format,
    )
}

/// Finalize the ingest operation: print summary and determine command outcome.
fn finalize_ingest(
    summary: IngestSummary,
    commit_result: Option<Result<()>>,
    format: IngestOutputFormat,
) -> Result<()> {
    match format {
        IngestOutputFormat::Json => print_json_report(&summary)?,
        IngestOutputFormat::Human if summary.candidates == 0 => {
            println!("\n  Nothing to ingest.");
        }
        IngestOutputFormat::Human => print_human_report(&summary),
    }

    outcome_error(&summary, commit_result)
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
        if let Some(max) = args.max_size {
            let metadata = entry.metadata().with_context(|| {
                format!(
                    "Failed to read file metadata for size filtering: {}",
                    file_path.display()
                )
            })?;
            if metadata.len() > max {
                continue;
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

/// Identity shared by every observation of one ingest run (ADR-009).
#[derive(Debug, Clone)]
struct RunContext {
    /// Identifies this logical ingest attempt; stamped on every committed row.
    run_id: Uuid,
    /// Stable identifier of the ingest root. Slice 3a derives it from the
    /// canonical root path; `--source` (slice 3b) will allow overriding it.
    source_id: String,
}

impl RunContext {
    fn new(root: &Path) -> Self {
        Self {
            run_id: Uuid::new_v4(),
            source_id: root.to_string_lossy().into_owned(),
        }
    }
}

/// How many times a file whose bytes changed mid-upload is rescanned and
/// retried before it is reported as a per-file error.
const SOURCE_CHANGED_RETRIES: usize = 1;

/// Process a single file: scan, hash, then depending on `mode` stop
/// (`--dry-run --offline`), verify existence only (`--dry-run`), or verify and
/// upload (ingest).
async fn process_file(
    path: &Path,
    root_path: &Path,
    config: &LakehouseConfig,
    operator: Option<&Operator>,
    mode: IngestMode,
    ctx: &RunContext,
) -> Result<IngestOutcome> {
    let mut info = prepare_file(path, root_path, config, ctx).await?;

    if !mode.is_connected() {
        // --dry-run --offline: no store access, so every candidate is "would upload".
        return Ok(IngestOutcome::Uploaded(Box::new(info)));
    }

    let op = operator.ok_or_else(|| anyhow::anyhow!("Storage operator not available"))?;

    let mut attempts_left = SOURCE_CHANGED_RETRIES;
    loop {
        let hash = info
            .content_hash
            .clone()
            .expect("prepare_file always sets the content hash");

        // Existing blobs are verified (size, and SHA-256 metadata when present),
        // not merely checked for existence. A mismatch is a per-file error,
        // never an overwrite.
        match upload::verify_existing(op, &hash, info.size_bytes).await? {
            upload::BlobVerdict::Missing => {}
            upload::BlobVerdict::Verified | upload::BlobVerdict::Legacy => {
                return Ok(IngestOutcome::AlreadyExists);
            }
        }

        if !mode.writes() {
            // --dry-run: existence is known, but nothing is uploaded.
            return Ok(IngestOutcome::Uploaded(Box::new(info)));
        }

        match upload::upload_verified(op, path, &hash, info.size_bytes).await {
            Ok(upload::UploadOutcome::Uploaded { .. }) => {
                info.ingested_at = Some(Utc::now());
                return Ok(IngestOutcome::Uploaded(Box::new(info)));
            }
            Ok(upload::UploadOutcome::AlreadyExists) => return Ok(IngestOutcome::AlreadyExists),
            Err(e) if e.is::<upload::SourceChanged>() && attempts_left > 0 => {
                attempts_left -= 1;
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "Source changed during upload; rescanning and retrying"
                );
                info = prepare_file(path, root_path, config, ctx).await?;
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!(
                        "upload failed after {} retr{}",
                        SOURCE_CHANGED_RETRIES,
                        if SOURCE_CHANGED_RETRIES == 1 {
                            "y"
                        } else {
                            "ies"
                        }
                    )
                });
            }
        }
    }
}

/// Scan and hash one file and attach its observation identity. Pure with
/// respect to the store; safe to call again after a `SourceChanged` retry.
async fn prepare_file(
    path: &Path,
    root_path: &Path,
    config: &LakehouseConfig,
    ctx: &RunContext,
) -> Result<FileInfo> {
    // 1. Scan file for initial metadata
    let mut info = scan_file(path).await?;

    // 2. Path identity relative to the ingest root
    let relative = path.strip_prefix(root_path).unwrap_or(path);
    if let Some(parent) = relative.parent() {
        info = info.with_parent_dir(parent.to_string_lossy().to_string());
    }
    let relative_path = normalize_relative_path(relative);

    // 3. Content hash (scan_file only hashes small files inline)
    if info.content_hash.is_none() {
        let hash_path = path.to_path_buf();
        let h = tokio::task::spawn_blocking(move || file_hash::full_sha256(&hash_path))
            .await
            .context("Blocking hash task panicked")??;
        info.content_hash = Some(ContentHash::new(h));
    }
    let content_hash = info.content_hash.clone().expect("content hash set above");

    info.object_uri = Some(format!(
        "s3://{}/{}",
        config.bucket,
        content_hash.to_object_key()
    ));

    // 4. Observation identity; derives the deterministic row id from the hash.
    Ok(info.with_observation(
        ctx.source_id.clone(),
        relative_path,
        ctx.run_id,
        ObservationStatus::Present,
        Utc::now(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{IngestArgs, IngestOutputFormat};
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
            offline: true,
            format: IngestOutputFormat::Human,
        }
    }

    fn finalize(
        commit_result: Option<Result<()>>,
        uploaded_count: u64,
        exists_count: u64,
        errors: &[String],
        total_bytes: u64,
        mode: IngestMode,
    ) -> Result<()> {
        finalize_with_format(
            commit_result,
            uploaded_count,
            exists_count,
            errors,
            total_bytes,
            mode,
            IngestOutputFormat::Human,
        )
    }

    fn finalize_with_format(
        commit_result: Option<Result<()>>,
        uploaded_count: u64,
        exists_count: u64,
        errors: &[String],
        total_bytes: u64,
        mode: IngestMode,
        format: IngestOutputFormat,
    ) -> Result<()> {
        let candidates = uploaded_count + exists_count + errors.len() as u64;
        let catalog_commit = catalog_commit_from_result(&commit_result);
        finalize_ingest(
            IngestSummary::build(
                mode,
                Uuid::nil(),
                candidates,
                uploaded_count,
                exists_count,
                total_bytes,
                errors,
                catalog_commit,
            ),
            commit_result,
            format,
        )
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
        args.max_size = Some(1024);

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

    // ── IngestOutcome tally tests ──

    fn make_test_info(name: &str, size: u64) -> FileInfo {
        let mut info = FileInfo::new(
            std::path::PathBuf::from(format!("/test/{}", name)),
            name.to_string(),
            ".txt".to_string(),
            size,
            None,
            None,
        );
        info.ingested_at = Some(chrono::Utc::now());
        info
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
        let result = finalize(Some(Ok(())), 3, 1, &[], 1024, IngestMode::Ingest);
        assert!(result.is_ok());
    }

    #[test]
    fn finalize_commit_failure() {
        let err = anyhow::anyhow!("catalog connection refused");
        let result = finalize(Some(Err(err)), 3, 0, &[], 1024, IngestMode::Ingest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata commit"));
    }

    #[test]
    fn finalize_no_commit() {
        let result = finalize(None, 0, 5, &[], 0, IngestMode::Ingest);
        assert!(result.is_ok());
    }

    #[test]
    fn finalize_partial_processing_failure() {
        let errors = vec!["unreadable.txt: permission denied".to_string()];
        let result = finalize(Some(Ok(())), 2, 0, &errors, 1024, IngestMode::Ingest);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest incomplete: 1 file(s) failed"));
    }

    #[test]
    fn finalize_complete_processing_failure() {
        let errors = vec![
            "unreadable-a.txt: permission denied".to_string(),
            "unreadable-b.txt: permission denied".to_string(),
        ];
        let result = finalize(None, 0, 0, &errors, 0, IngestMode::Ingest);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest incomplete: 2 file(s) failed"));
    }

    #[test]
    fn finalize_dry_run_processing_failure() {
        let errors = vec!["unreadable.txt: permission denied".to_string()];
        let result = finalize(None, 0, 0, &errors, 0, IngestMode::DryRunOffline);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }

    #[test]
    fn finalize_json_commit_failure_still_exits_nonzero() {
        let err = anyhow::anyhow!("catalog connection refused");
        let result = finalize_with_format(
            Some(Err(err)),
            3,
            0,
            &[],
            1024,
            IngestMode::Ingest,
            IngestOutputFormat::Json,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata commit"));
    }

    #[test]
    fn finalize_connected_dry_run_processing_failure() {
        let errors = vec!["unreadable.txt: permission denied".to_string()];
        let result = finalize(None, 1, 2, &errors, 0, IngestMode::DryRun);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }

    #[test]
    fn finalize_connected_dry_run_success_without_commit_is_ok() {
        let result = finalize(None, 1, 2, &[], 12, IngestMode::DryRun);
        assert!(result.is_ok());
    }
}
