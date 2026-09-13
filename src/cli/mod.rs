//! CLI module - Command-line interface using clap
//!
//! Defines all commands and their arguments for the Anti-Entropator.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

const KIBIBYTE: u64 = 1024;
const MEBIBYTE: u64 = KIBIBYTE * 1024;
const GIBIBYTE: u64 = MEBIBYTE * 1024;

fn parse_byte_size(value: &str) -> Result<u64, String> {
    let normalized = value.trim().to_ascii_uppercase();
    let (number, multiplier) = if let Some(number) = normalized.strip_suffix("GB") {
        (number, GIBIBYTE)
    } else if let Some(number) = normalized.strip_suffix("MB") {
        (number, MEBIBYTE)
    } else if let Some(number) = normalized.strip_suffix("KB") {
        (number, KIBIBYTE)
    } else if let Some(number) = normalized.strip_suffix('B') {
        (number, 1)
    } else {
        return Err(format!(
            "invalid size '{value}': expected a non-negative integer followed by B, KB, MB, or GB"
        ));
    };

    let number = number.trim().parse::<u64>().map_err(|_| {
        format!(
            "invalid size '{value}': expected a non-negative integer followed by B, KB, MB, or GB"
        )
    })?;

    number
        .checked_mul(multiplier)
        .ok_or_else(|| format!("invalid size '{value}': value exceeds the supported byte range"))
}

/// Anti-Entropator: A Local Data Lakehouse for File Organization
///
/// Transform a chaotic downloads folder into a queryable, organized data lakehouse.
#[derive(Parser, Debug)]
#[command(name = "anti_entropator")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Profile a directory to understand its contents (read-only, no Docker required)
    Profile(ProfileArgs),

    /// Run preflight checks (Docker, endpoints, credentials, external tools)
    Doctor,

    /// Check if the lakehouse stack is running and reachable
    Up,

    /// Initialize the warehouse and register an Iceberg warehouse in the catalog (Lakekeeper)
    Init,

    /// Scan a directory and enrich file metadata (no uploads)
    Scan(ScanArgs),

    /// Ingest files into the lakehouse (upload to RustFS + commit to Iceberg via Lakekeeper)
    Ingest(IngestArgs),

    /// Interactive SQL REPL (planned, not yet implemented)
    Sql,

    /// Execute a one-shot SQL query
    Query {
        /// The SQL query to execute
        sql: String,
    },

    /// Find and report duplicate files (planned, not yet implemented)
    Duplicates,

    /// Merge an ingest branch into main (planned, not yet implemented)
    Merge,
}

#[derive(Parser, Debug)]
pub struct ProfileArgs {
    /// Path to the directory to profile
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output directory for JSON/Markdown reports
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    /// Use decimal units (GB) instead of binary (GiB)
    #[arg(long)]
    pub decimal: bool,

    /// Skip MIME type detection (faster, extension-only)
    #[arg(long)]
    pub no_mime: bool,

    /// Skip duplicate estimation (faster)
    #[arg(long)]
    pub no_duplicates: bool,

    /// Maximum files to quick-hash for duplicate estimation
    #[arg(long, default_value = "5000")]
    pub max_hash_files: usize,

    /// Output format
    #[arg(long, value_enum, default_value = "table")]
    pub format: OutputFormat,
}

#[derive(Parser, Debug)]
pub struct ScanArgs {
    /// Path to the directory to scan
    pub path: PathBuf,

    /// Limit the number of files to scan
    #[arg(long)]
    pub limit: Option<usize>,

    /// Output format
    #[arg(long, value_enum, default_value = "table")]
    pub format: OutputFormat,

    /// Dry run - show what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Parser, Debug)]
pub struct IngestArgs {
    /// Path to the directory to ingest
    pub path: PathBuf,

    /// Include only files whose names match these glob patterns (e.g., *.jpg)
    #[arg(long)]
    pub include: Vec<String>,

    /// Exclude files whose names match these glob patterns (e.g., *.log)
    #[arg(long)]
    pub exclude: Vec<String>,

    /// Only ingest files of these types (e.g., "pdf,image,video")
    #[arg(long, value_delimiter = ',')]
    pub types: Vec<String>,

    /// Maximum file size to ingest (e.g., "1GB", "500MB")
    #[arg(long, value_name = "SIZE", value_parser = parse_byte_size)]
    pub max_size: Option<u64>,

    /// Limit the number of files to ingest
    #[arg(long)]
    pub limit: Option<usize>,

    /// Dry run - show what would be done without uploading
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
pub enum OutputFormat {
    /// Pretty-printed tables
    #[default]
    Table,
    /// JSON output
    Json,
    /// Markdown report
    Markdown,
}

#[cfg(test)]
mod tests {
    use super::parse_byte_size;

    #[test]
    fn parse_byte_size_accepts_supported_units() {
        assert_eq!(parse_byte_size("500B"), Ok(500));
        assert_eq!(parse_byte_size("100KB"), Ok(100 * 1024));
        assert_eq!(parse_byte_size("1MB"), Ok(1024 * 1024));
        assert_eq!(parse_byte_size("2GB"), Ok(2 * 1024 * 1024 * 1024));
    }

    #[test]
    fn parse_byte_size_normalizes_case_and_whitespace() {
        assert_eq!(parse_byte_size(" 10 mb "), Ok(10 * 1024 * 1024));
    }

    #[test]
    fn parse_byte_size_rejects_invalid_values() {
        for value in ["100TB", "100", "MB", "abc", "-1KB"] {
            assert!(
                parse_byte_size(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn parse_byte_size_rejects_overflow() {
        assert!(parse_byte_size("18446744073709551615GB").is_err());
    }
}
