//! Ingest module - Upload files to the lakehouse
//!
//! Implements content-addressed storage with Iceberg catalog integration.

use crate::cli::IngestArgs;
use crate::domain::{ContentHash, FileCategory, FileInfo};
use crate::lakehouse::{writer, LakehouseConfig};
use crate::scan::scan_file;
use anyhow::{Context, Result};
use aws_sdk_s3::primitives::ByteStream;
use chrono::Utc;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::path::Path;
use walkdir::WalkDir;

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
    let mut skipped_count = 0u64;
    let mut errors = Vec::new();
    let mut total_bytes = 0u64;

    // Create S3 client (only if not dry-run)
    let s3_client = if !args.dry_run {
        Some(create_s3_client(&config).await?)
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

        match process_file(file_path, &path, &config, s3_client.as_ref(), args.dry_run).await {
            Ok(Some(info)) => {
                uploaded_count += 1;
                total_bytes += info.size_bytes;
                uploaded_files.push(info);
            }
            Ok(None) => {
                skipped_count += 1;
            }
            Err(e) => {
                errors.push(format!("{}: {}", file_path.display(), e));
            }
        }
    }

    pb.finish_and_clear();

    // Commit to Iceberg if not dry-run
    if !args.dry_run && !uploaded_files.is_empty() {
        print!("  Committing metadata to Iceberg catalog... ");
        match writer::commit_files(uploaded_files, &config).await {
            Ok(_) => println!("{}", style("OK").green()),
            Err(e) => {
                println!("{}", style("FAILED").red());
                println!("  Warning: Metadata commit failed: {}", e);
            }
        }
    }

    // Print summary
    println!();
    println!("─── Ingest Results ─────────────────────────────────────────────");
    println!();
    println!(
        "  Uploaded: {} files ({})",
        uploaded_count,
        humansize::format_size(total_bytes, humansize::BINARY)
    );
    println!("  Skipped:  {} files (already exist)", skipped_count);
    println!("  Errors:   {} files", errors.len());
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

    if args.dry_run {
        println!(
            "{}",
            style("  Dry run - no files were uploaded. Remove --dry-run to actually ingest.").dim()
        );
    } else {
        println!("{}", style("  Files ingested successfully!").green());
        println!();
        println!("  Next steps:");
        println!("    1. Run `anti_entropator query` to explore your catalog");
        println!("    2. Run `anti_entropator duplicates` to find duplicate files");
    }

    println!();

    Ok(())
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

/// Create S3 client for RustFS
async fn create_s3_client(config: &LakehouseConfig) -> Result<aws_sdk_s3::Client> {
    use aws_config::BehaviorVersion;
    use aws_sdk_s3::config::{Credentials, Region};

    let creds = Credentials::new(
        &config.s3_access_key,
        &config.s3_secret_key,
        None,
        None,
        "anti_entropator",
    );

    let s3_config = aws_sdk_s3::Config::builder()
        .behavior_version(BehaviorVersion::latest())
        .endpoint_url(&config.s3_endpoint)
        .region(Region::new("us-east-1"))
        .credentials_provider(creds)
        .force_path_style(true)
        .build();

    Ok(aws_sdk_s3::Client::from_conf(s3_config))
}

/// Collect files to ingest based on filters
fn collect_files(path: &Path, args: &IngestArgs) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let mut type_filter: Option<HashSet<String>> = None;

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

        // Apply exclude patterns
        if !args.exclude.is_empty() {
            let path_str = file_path.to_string_lossy();
            if args
                .exclude
                .iter()
                .any(|pattern| path_str.contains(pattern))
            {
                continue;
            }
        }

        // Apply include patterns (if specified)
        if !args.include.is_empty() {
            let path_str = file_path.to_string_lossy();
            if !args
                .include
                .iter()
                .any(|pattern| path_str.contains(pattern))
            {
                continue;
            }
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

/// Process a single file: scan, hash, upload, return enriched FileInfo
async fn process_file(
    path: &Path,
    root_path: &Path,
    config: &LakehouseConfig,
    client: Option<&aws_sdk_s3::Client>,
    dry_run: bool,
) -> Result<Option<FileInfo>> {
    // 1. Scan file for initial metadata
    let mut info = scan_file(path).await?;

    // 2. Set parent directory (relative to root)
    if let Ok(relative) = path.strip_prefix(root_path) {
        if let Some(parent) = relative.parent() {
            info = info.with_parent_dir(parent.to_string_lossy().to_string());
        }
    }

    // 3. Compute object key based on content hash
    // If scan_file didn't compute content hash (e.g. file too large), compute it now
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
        return Ok(Some(info));
    }

    let client = client.ok_or_else(|| anyhow::anyhow!("S3 client not available"))?;

    // 3. Check if object already exists
    let exists = client
        .head_object()
        .bucket(&config.bucket)
        .key(&object_key)
        .send()
        .await
        .is_ok();

    if exists {
        // Even if it exists, we return the info so it gets cataloged in Iceberg
        // This is crucial for the "logical duplication" feature
        return Ok(Some(info));
    }

    // 4. Upload file
    let body = ByteStream::from_path(path).await?;

    client
        .put_object()
        .bucket(&config.bucket)
        .key(&object_key)
        .body(body)
        .send()
        .await
        .context("Failed to upload to S3")?;

    info.ingested_at = Some(Utc::now());

    Ok(Some(info))
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
