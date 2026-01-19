//! Configuration module
//!
//! Handles loading and managing application configuration.

#![allow(dead_code)] // Config scaffolding - will be wired up in later phases

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Lakehouse configuration
    #[serde(default)]
    pub lakehouse: LakehouseConfig,

    /// Profile configuration
    #[serde(default)]
    pub profile: ProfileConfig,

    /// Category mappings (extension -> category override)
    #[serde(default)]
    pub categories: HashMap<String, String>,

    /// Ignore patterns
    #[serde(default)]
    pub ignore: IgnoreConfig,

    /// External tools configuration
    #[serde(default)]
    pub external_tools: ExternalToolsConfig,
}

/// Lakehouse connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LakehouseConfig {
    /// RustFS S3 endpoint
    #[serde(default = "default_s3_endpoint")]
    pub s3_endpoint: String,

    /// RustFS access key
    #[serde(default)]
    pub s3_access_key: String,

    /// RustFS secret key
    #[serde(default)]
    pub s3_secret_key: String,

    /// Bucket name for data
    #[serde(default = "default_bucket")]
    pub bucket: String,

    /// Iceberg REST catalog endpoint (Lakekeeper)
    #[serde(default = "default_catalog_endpoint")]
    pub catalog_endpoint: String,

    /// Warehouse path prefix
    #[serde(default = "default_warehouse")]
    pub warehouse: String,
}

fn default_s3_endpoint() -> String {
    "http://localhost:19000".to_string()
}

fn default_bucket() -> String {
    "anti-entropator".to_string()
}

fn default_catalog_endpoint() -> String {
    "http://localhost:8181".to_string()
}

fn default_warehouse() -> String {
    "s3://anti-entropator/warehouse".to_string()
}

impl Default for LakehouseConfig {
    fn default() -> Self {
        Self {
            s3_endpoint: default_s3_endpoint(),
            s3_access_key: String::new(),
            s3_secret_key: String::new(),
            bucket: default_bucket(),
            catalog_endpoint: default_catalog_endpoint(),
            warehouse: default_warehouse(),
        }
    }
}

/// Profile command configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Maximum files to hash for duplicate detection
    #[serde(default = "default_max_hash_files")]
    pub max_hash_files: usize,

    /// Hash block size (bytes)
    #[serde(default = "default_hash_block_size")]
    pub hash_block_size: usize,

    /// Maximum files to track for "no extension" list
    #[serde(default = "default_max_no_ext")]
    pub max_no_extension_examples: usize,

    /// Maximum largest files to track
    #[serde(default = "default_max_largest")]
    pub max_largest_files: usize,
}

fn default_max_hash_files() -> usize {
    5000
}

fn default_hash_block_size() -> usize {
    65536 // 64KB
}

fn default_max_no_ext() -> usize {
    20
}

fn default_max_largest() -> usize {
    15
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            max_hash_files: default_max_hash_files(),
            hash_block_size: default_hash_block_size(),
            max_no_extension_examples: default_max_no_ext(),
            max_largest_files: default_max_largest(),
        }
    }
}

/// Ignore configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoreConfig {
    /// Patterns to ignore (glob)
    #[serde(default = "default_ignore_patterns")]
    pub patterns: Vec<String>,

    /// Ignore hidden files (starting with .)
    #[serde(default = "default_true")]
    pub hidden: bool,

    /// Ignore system files (.DS_Store, Thumbs.db, etc.)
    #[serde(default = "default_true")]
    pub system: bool,
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        "node_modules".to_string(),
        ".git".to_string(),
        "__pycache__".to_string(),
        "*.pyc".to_string(),
        ".venv".to_string(),
        "target".to_string(),
    ]
}

fn default_true() -> bool {
    true
}

impl Default for IgnoreConfig {
    fn default() -> Self {
        Self {
            patterns: default_ignore_patterns(),
            hidden: true,
            system: true,
        }
    }
}

/// External tools configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalToolsConfig {
    /// Enable external tool usage
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Path to ffprobe (auto-detect if empty)
    #[serde(default)]
    pub ffprobe_path: Option<PathBuf>,

    /// Path to exiftool (auto-detect if empty)
    #[serde(default)]
    pub exiftool_path: Option<PathBuf>,

    /// Path to pdfinfo (auto-detect if empty)
    #[serde(default)]
    pub pdfinfo_path: Option<PathBuf>,
}

impl Default for ExternalToolsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ffprobe_path: None,
            exiftool_path: None,
            pdfinfo_path: None,
        }
    }
}

impl Config {
    /// Load configuration from file or use defaults
    pub fn load(path: Option<&PathBuf>) -> anyhow::Result<Self> {
        if let Some(config_path) = path {
            if config_path.exists() {
                let content = std::fs::read_to_string(config_path)?;
                let config: Config = toml::from_str(&content)?;
                return Ok(config);
            }
        }

        // Try default locations
        let default_paths = [
            PathBuf::from("anti_entropator.toml"),
            PathBuf::from(".anti_entropator.toml"),
            dirs::config_dir()
                .map(|p| p.join("anti_entropator").join("config.toml"))
                .unwrap_or_default(),
        ];

        for path in &default_paths {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let config: Config = toml::from_str(&content)?;
                return Ok(config);
            }
        }

        Ok(Config::default())
    }
}

mod dirs {
    use std::path::PathBuf;

    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "anti-entropator", "anti_entropator")
            .map(|p| p.config_dir().to_path_buf())
    }
}
