//! CLI module - Command-line interface using clap
//!
//! Defines all commands and their arguments for the Anti-Entropator.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    #[arg(long)]
    pub max_size: Option<String>,

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
