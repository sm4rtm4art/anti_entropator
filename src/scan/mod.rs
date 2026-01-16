//! Scan module - File enrichment pipeline
//!
//! Scans directories and enriches file metadata using:
//! - MIME type detection
//! - External tools (ffprobe, exiftool, pdfinfo)
//! - Suggested naming based on metadata

mod enrichers;

use crate::cli::ScanArgs;
use crate::domain::{ContentHash, FileCategory, FileInfo, PartialHash};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

// Enrichers are accessed via enrichers:: module path

/// Run the scan command
pub async fn run(args: ScanArgs) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    println!();
    println!(
        "{}",
        style("═══════════════════════════════════════════════════════════════").cyan()
    );
    println!("  📂 Anti-Entropator Scan");
    println!(
        "{}",
        style("═══════════════════════════════════════════════════════════════").cyan()
    );
    println!();
    println!("  Path: {}", path.display());
    if let Some(limit) = args.limit {
        println!("  Limit: {} files", limit);
    }
    println!("  Dry run: {}", if args.dry_run { "yes" } else { "no" });
    println!();

    // Count files first
    let total_files: usize = WalkDir::new(&path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();

    let file_count = args
        .limit
        .map(|l| l.min(total_files))
        .unwrap_or(total_files);

    println!(
        "  Found {} files total, scanning {}",
        total_files, file_count
    );
    println!();

    // Create progress bar
    let pb = ProgressBar::new(file_count as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut scanned = Vec::new();
    let mut errors = Vec::new();

    for entry in WalkDir::new(&path).follow_links(false) {
        // Check limit
        if let Some(limit) = args.limit {
            if scanned.len() >= limit {
                break;
            }
        }

        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!(
                    "{}: {}",
                    e.path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    e
                ));
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        pb.inc(1);

        let file_path = entry.path();
        pb.set_message(format!(
            "{}",
            file_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        match scan_file(file_path).await {
            Ok(info) => scanned.push(info),
            Err(e) => errors.push(format!("{}: {}", file_path.display(), e)),
        }
    }

    pb.finish_and_clear();

    // Print summary
    println!("─── Scan Results ───────────────────────────────────────────────");
    println!();
    println!("  Scanned: {} files", scanned.len());
    println!("  Errors:  {} files", errors.len());
    println!();

    // Category breakdown
    let mut by_category: std::collections::HashMap<FileCategory, (usize, u64)> =
        std::collections::HashMap::new();

    for info in &scanned {
        let entry = by_category.entry(info.category).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += info.size_bytes;
    }

    println!("  By category:");
    let mut cats: Vec<_> = by_category.iter().collect();
    cats.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));

    for (cat, (count, bytes)) in cats {
        println!(
            "    {:<12} {:>6} files  {:>10}",
            cat.to_string(),
            count,
            humansize::format_size(*bytes, humansize::BINARY)
        );
    }
    println!();

    // Show files with suggested names
    let with_suggestions: Vec<_> = scanned
        .iter()
        .filter(|f| f.suggested_name.is_some())
        .collect();

    if !with_suggestions.is_empty() {
        println!(
            "  Files with suggested renames ({}):",
            with_suggestions.len()
        );
        for info in with_suggestions.iter().take(10) {
            println!(
                "    {} → {}",
                style(&info.filename).dim(),
                style(info.suggested_name.as_ref().unwrap()).green()
            );
            if let Some(ref reason) = info.name_reason {
                println!("      ({})", style(reason).dim());
            }
        }
        if with_suggestions.len() > 10 {
            println!("    ... and {} more", with_suggestions.len() - 10);
        }
        println!();
    }

    // Show duplicates found
    let duplicates: Vec<_> = scanned.iter().filter(|f| f.is_duplicate).collect();

    if !duplicates.is_empty() {
        println!(
            "  {} potential duplicates found",
            style(duplicates.len()).yellow()
        );
        println!();
    }

    if args.dry_run {
        println!(
            "{}",
            style("  Dry run - no changes made. Remove --dry-run to persist results.").dim()
        );
    } else {
        println!(
            "{}",
            style("  Results ready for ingest. Run `anti_entropator ingest` to upload.").green()
        );
    }

    println!();

    Ok(())
}

/// Scan a single file and return enriched FileInfo
async fn scan_file(path: &Path) -> Result<FileInfo> {
    let metadata = std::fs::metadata(path).context("Failed to read file metadata")?;

    let filename = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let extension = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_else(|| "(none)".to_string());

    let size_bytes = metadata.len();

    let modified_at = metadata.modified().ok().map(DateTime::<Utc>::from);

    let created_at = metadata.created().ok().map(DateTime::<Utc>::from);

    let mut info = FileInfo::new(
        path.to_path_buf(),
        filename.clone(),
        extension.clone(),
        size_bytes,
        modified_at,
        created_at,
    );

    // Detect MIME type
    if let Some(mime) = infer::get_from_path(path).ok().flatten() {
        info = info.with_mime_type(mime.mime_type().to_string());
    }

    // Compute hashes
    if size_bytes > 0 && size_bytes < 100 * 1024 * 1024 {
        // Skip files > 100MB for now
        // Partial hash (first 64KB)
        if let Ok(partial) = compute_partial_hash(path, 64 * 1024) {
            info = info.with_partial_hash(PartialHash::new(partial));
        }

        // Full hash for smaller files
        if size_bytes < 10 * 1024 * 1024 {
            if let Ok(full) = compute_full_hash(path) {
                info = info.with_content_hash(ContentHash::new(full));
            }
        }
    }

    // Try to get suggested name from external tools
    if let Some((name, reason)) = get_suggested_name(path, &info).await {
        info = info.with_suggested_name(name, reason);
    }

    Ok(info)
}

/// Compute partial hash (first N bytes)
fn compute_partial_hash(path: &Path, block_size: usize) -> Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; block_size];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    let result = hasher.finalize();

    Ok(format!("{:x}", result))
}

/// Compute full SHA-256 hash
fn compute_full_hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Get suggested name from external tools or metadata
async fn get_suggested_name(path: &Path, info: &FileInfo) -> Option<(String, String)> {
    let extension = &info.extension;

    // Try external tools based on file type
    match info.category {
        FileCategory::Image => {
            if let Some(result) = enrichers::exiftool_datetime(path).await {
                return Some(result);
            }
        }
        FileCategory::Video | FileCategory::Audio => {
            if let Some(result) = enrichers::ffprobe_datetime(path).await {
                return Some(result);
            }
        }
        FileCategory::Document => {
            if extension == ".pdf" {
                if let Some(result) = enrichers::pdfinfo_title(path).await {
                    return Some(result);
                }
            }
        }
        _ => {}
    }

    // Fallback: use modified date if filename looks generic
    if is_generic_filename(&info.filename) {
        if let Some(modified) = info.modified_at {
            let new_name = format!("{}{}", modified.format("%Y-%m-%d_%H-%M-%S"), extension);
            return Some((new_name, "modified_date_fallback".to_string()));
        }
    }

    None
}

/// Check if filename looks generic/unhelpful
fn is_generic_filename(name: &str) -> bool {
    let lower = name.to_lowercase();

    // Common generic patterns
    lower.starts_with("download")
        || lower.starts_with("untitled")
        || lower.starts_with("screenshot")
        || lower.starts_with("img_")
        || lower.starts_with("image")
        || lower.starts_with("video")
        || lower.starts_with("photo")
        || lower.contains("(1)")
        || lower.contains("(2)")
        || lower.contains("(3)")
        || is_uuid_like(&lower)
        || is_hex_hash(&lower)
}

fn is_uuid_like(s: &str) -> bool {
    // Check for UUID pattern: 8-4-4-4-12 hex chars
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() >= 5 {
        let lengths: Vec<usize> = parts.iter().take(5).map(|p| p.len()).collect();
        if lengths == vec![8, 4, 4, 4, 12] {
            return parts
                .iter()
                .take(5)
                .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }
    false
}

fn is_hex_hash(s: &str) -> bool {
    // Long hex strings (20+ chars) are likely hashes
    let base = s.split('.').next().unwrap_or(s);
    base.len() >= 20 && base.chars().all(|c| c.is_ascii_hexdigit())
}
