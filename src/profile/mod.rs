//! Profile module - Read-only directory profiling
//!
//! Implements the `profile` command that analyzes a directory to understand
//! its contents without making any changes.

mod output;
mod scanner;

use crate::cli::{OutputFormat, ProfileArgs};
use crate::domain::stats::ProfileResult;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;

use output::{
    generate_markdown_report, print_json_report, print_markdown_report, print_table_report,
};
use scanner::scan;

/// Run the profile command
pub async fn run(args: ProfileArgs) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    // Create progress bar
    let pb = ProgressBar::new_spinner();
    let pb_style = ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .context("Invalid profile spinner template")?;
    pb.set_style(pb_style);
    pb.set_message("Scanning directory...");

    // Run the scan
    let options = ScanOptions {
        detect_mime: !args.no_mime,
        detect_duplicates: !args.no_duplicates,
        max_hash_files: args.max_hash_files,
        use_decimal: args.decimal,
    };

    let result = scan_directory(&path, &options, Some(&pb)).await?;

    pb.finish_and_clear();

    // Output results
    match args.format {
        OutputFormat::Table => {
            print_table_report(&result, args.decimal)?;
        }
        OutputFormat::Json => {
            print_json_report(&result)?;
        }
        OutputFormat::Markdown => {
            print_markdown_report(&result, args.decimal)?;
        }
    }

    // Optionally write to files
    if let Some(ref out_dir) = args.out {
        std::fs::create_dir_all(out_dir)?;

        let json_path = out_dir.join("profile.json");
        let md_path = out_dir.join("profile.md");

        std::fs::write(&json_path, serde_json::to_string_pretty(&result)?)?;
        println!("\n📄 JSON report written to: {}", json_path.display());

        let md_content = generate_markdown_report(&result, args.decimal)?;
        std::fs::write(&md_path, md_content)?;
        println!("📄 Markdown report written to: {}", md_path.display());
    }

    Ok(())
}

/// Scan options
#[derive(Debug, Clone)]
#[allow(dead_code)] // use_decimal planned for future use
pub struct ScanOptions {
    /// Detect MIME types by reading file headers
    pub detect_mime: bool,

    /// Estimate duplicates via quick-hash
    pub detect_duplicates: bool,

    /// Maximum files to hash for duplicate detection
    pub max_hash_files: usize,

    /// Use decimal units (GB) instead of binary (GiB)
    pub use_decimal: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            detect_mime: true,
            detect_duplicates: true,
            max_hash_files: 5000,
            use_decimal: false,
        }
    }
}

/// Scan a directory and return profile results
pub async fn scan_directory(
    path: &Path,
    options: &ScanOptions,
    progress: Option<&ProgressBar>,
) -> Result<ProfileResult> {
    scan(path, options, progress).await
}
