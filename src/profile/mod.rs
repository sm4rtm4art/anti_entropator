//! Profile module - Read-only directory profiling
//!
//! Implements the `profile` command that analyzes a directory to understand
//! its contents without making any changes.

mod output;
mod scanner;

use crate::cli::{OutputFormat, ProfileArgs};
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

use output::{
    generate_markdown_report, print_json_report, print_markdown_report, print_table_report,
};
use scanner::scan;

/// Run the profile command
pub async fn run(args: ProfileArgs) -> Result<()> {
    let path = match args.path.canonicalize() {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => args.path.clone(),
        Err(e) => {
            anyhow::bail!("Cannot resolve path '{}': {}", args.path.display(), e);
        }
    };

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
    };

    let result = scan(&path, &options, Some(&pb)).await?;

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
        tokio::fs::create_dir_all(out_dir).await?;

        let json_path = out_dir.join("profile.json");
        let md_path = out_dir.join("profile.md");

        tokio::fs::write(&json_path, serde_json::to_string_pretty(&result)?).await?;
        println!("\n  JSON report written to: {}", json_path.display());

        let md_content = generate_markdown_report(&result, args.decimal)?;
        tokio::fs::write(&md_path, md_content).await?;
        println!("  Markdown report written to: {}", md_path.display());
    }

    Ok(())
}

/// Scan options
#[derive(Debug, Clone)]
pub(crate) struct ScanOptions {
    /// Detect MIME types by reading file headers
    pub detect_mime: bool,

    /// Estimate duplicates via quick-hash
    pub detect_duplicates: bool,

    /// Maximum files to hash for duplicate detection
    pub max_hash_files: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            detect_mime: true,
            detect_duplicates: true,
            max_hash_files: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn run_fails_on_symlink_loop_with_resolution_error() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("create tempdir");
        let a = dir.path().join("a");
        let b = dir.path().join("b");

        symlink(&b, &a).expect("create symlink a -> b");
        symlink(&a, &b).expect("create symlink b -> a");

        let args = ProfileArgs {
            path: a,
            out: None,
            decimal: false,
            no_mime: true,
            no_duplicates: true,
            max_hash_files: 5000,
            format: OutputFormat::Table,
        };

        let error = run(args).await.expect_err("symlink loop should fail");
        assert!(error.to_string().contains("Cannot resolve path"));
    }
}
