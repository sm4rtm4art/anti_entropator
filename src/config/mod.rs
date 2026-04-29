//! Configuration module
//!
//! Handles loading and managing application configuration.
//! [`LakehouseConfig`] is the single source of truth — re-exported by
//! `crate::lakehouse` so downstream code does not need to change imports.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Application configuration
#[allow(dead_code)]
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

/// Lakehouse connection configuration.
///
/// Single source of truth for S3 + Lakekeeper connectivity.
/// Env vars take precedence over static defaults; TOML fields override both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LakehouseConfig {
    /// RustFS S3 endpoint (host-accessible)
    #[serde(default = "default_s3_endpoint")]
    pub s3_endpoint: String,

    /// S3 endpoint as seen from within Docker network (for Lakekeeper)
    #[serde(default = "default_s3_endpoint_internal")]
    pub s3_endpoint_internal: String,

    /// RustFS access key
    #[serde(default = "default_s3_access_key")]
    pub s3_access_key: String,

    /// RustFS secret key
    #[serde(default = "default_s3_secret_key")]
    pub s3_secret_key: String,

    /// Bucket name for data
    #[serde(default = "default_bucket")]
    pub bucket: String,

    /// Iceberg REST catalog endpoint (Lakekeeper)
    #[serde(default = "default_catalog_endpoint")]
    pub catalog_endpoint: String,

    /// Warehouse name in Lakekeeper
    #[serde(default = "default_warehouse")]
    pub warehouse: String,

    /// Lakekeeper project ID (resolved at runtime via `ensure_project`).
    #[serde(default)]
    pub project_id: Option<String>,
}

fn default_s3_endpoint() -> String {
    std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:8200".to_string())
}

fn default_s3_endpoint_internal() -> String {
    std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL")
        .unwrap_or_else(|_| "http://rustfs:9000".to_string())
}

fn default_s3_access_key() -> String {
    std::env::var("RUSTFS_ACCESS_KEY")
        .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
        .unwrap_or_default()
}

fn default_s3_secret_key() -> String {
    std::env::var("RUSTFS_SECRET_KEY")
        .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
        .unwrap_or_default()
}

fn default_bucket() -> String {
    std::env::var("ANTI_ENTROPATOR_BUCKET").unwrap_or_else(|_| "anti-entropator".to_string())
}

fn default_catalog_endpoint() -> String {
    std::env::var("ANTI_ENTROPATOR_CATALOG_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:8100".to_string())
}

fn default_warehouse() -> String {
    std::env::var("ANTI_ENTROPATOR_WAREHOUSE").unwrap_or_else(|_| "anti-entropator".to_string())
}

impl Default for LakehouseConfig {
    fn default() -> Self {
        Self {
            s3_endpoint: default_s3_endpoint(),
            s3_endpoint_internal: default_s3_endpoint_internal(),
            s3_access_key: default_s3_access_key(),
            s3_secret_key: default_s3_secret_key(),
            bucket: default_bucket(),
            catalog_endpoint: default_catalog_endpoint(),
            warehouse: default_warehouse(),
            project_id: std::env::var("ANTI_ENTROPATOR_PROJECT_ID").ok(),
        }
    }
}

/// Profile command configuration
#[allow(dead_code)]
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

#[allow(dead_code)]
fn default_max_hash_files() -> usize {
    5000
}

#[allow(dead_code)]
fn default_hash_block_size() -> usize {
    65536 // 64KB
}

#[allow(dead_code)]
fn default_max_no_ext() -> usize {
    20
}

#[allow(dead_code)]
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
#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
mod dirs {
    use std::path::PathBuf;

    pub fn config_dir() -> Option<PathBuf> {
        directories::ProjectDirs::from("com", "anti-entropator", "anti_entropator")
            .map(|p| p.config_dir().to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FULL_TOML: &str = r#"
[lakehouse]
s3_endpoint = "http://minio:9000"
s3_endpoint_internal = "http://minio-internal:9000"
s3_access_key = "testkey"
s3_secret_key = "testsecret"
bucket = "my-bucket"
catalog_endpoint = "http://lakekeeper:8181"
warehouse = "s3://my-bucket/wh"
project_id = "proj-123"

[profile]
max_hash_files = 100
hash_block_size = 4096
max_no_extension_examples = 5
max_largest_files = 3

[ignore]
patterns = ["*.tmp", "build"]
hidden = false
system = false

[external_tools]
enabled = false

[categories]
rs = "Code"
toml = "Config"
"#;

    const PARTIAL_TOML: &str = r#"
[lakehouse]
s3_endpoint = "http://custom:9000"
"#;

    #[test]
    fn default_config_has_expected_lakehouse_values() {
        let cfg = Config::default();
        // Env vars may override; check static fallbacks only when env is clean
        if std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT").is_err() {
            assert_eq!(cfg.lakehouse.s3_endpoint, "http://localhost:8200");
        }
        if std::env::var("ANTI_ENTROPATOR_CATALOG_ENDPOINT").is_err() {
            assert_eq!(cfg.lakehouse.catalog_endpoint, "http://localhost:8100");
        }
        if std::env::var("ANTI_ENTROPATOR_BUCKET").is_err() {
            assert_eq!(cfg.lakehouse.bucket, "anti-entropator");
        }
        if std::env::var("ANTI_ENTROPATOR_WAREHOUSE").is_err() {
            assert_eq!(cfg.lakehouse.warehouse, "anti-entropator");
        }
        if std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL").is_err() {
            assert_eq!(cfg.lakehouse.s3_endpoint_internal, "http://rustfs:9000");
        }
        assert!(
            cfg.lakehouse.project_id.is_none()
                || std::env::var("ANTI_ENTROPATOR_PROJECT_ID").is_ok()
        );
    }

    #[test]
    fn default_config_has_expected_profile_values() {
        let cfg = Config::default();
        assert_eq!(cfg.profile.max_hash_files, 5000);
        assert_eq!(cfg.profile.hash_block_size, 65536);
        assert_eq!(cfg.profile.max_no_extension_examples, 20);
        assert_eq!(cfg.profile.max_largest_files, 15);
    }

    #[test]
    fn default_config_has_expected_ignore_values() {
        let cfg = Config::default();
        assert!(cfg.ignore.hidden);
        assert!(cfg.ignore.system);
        assert!(cfg.ignore.patterns.contains(&"node_modules".to_string()));
        assert!(cfg.ignore.patterns.contains(&".git".to_string()));
        assert!(cfg.ignore.patterns.contains(&"target".to_string()));
        assert_eq!(cfg.ignore.patterns.len(), 6);
    }

    #[test]
    fn default_config_has_expected_external_tools_values() {
        let cfg = Config::default();
        assert!(cfg.external_tools.enabled);
        assert!(cfg.external_tools.ffprobe_path.is_none());
        assert!(cfg.external_tools.exiftool_path.is_none());
        assert!(cfg.external_tools.pdfinfo_path.is_none());
    }

    #[test]
    fn default_config_categories_empty() {
        let cfg = Config::default();
        assert!(cfg.categories.is_empty());
    }

    #[test]
    fn load_explicit_path_full_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, FULL_TOML).unwrap();

        let cfg = Config::load(Some(&path)).unwrap();

        assert_eq!(cfg.lakehouse.s3_endpoint, "http://minio:9000");
        assert_eq!(
            cfg.lakehouse.s3_endpoint_internal,
            "http://minio-internal:9000"
        );
        assert_eq!(cfg.lakehouse.s3_access_key, "testkey");
        assert_eq!(cfg.lakehouse.s3_secret_key, "testsecret");
        assert_eq!(cfg.lakehouse.bucket, "my-bucket");
        assert_eq!(cfg.lakehouse.catalog_endpoint, "http://lakekeeper:8181");
        assert_eq!(cfg.lakehouse.warehouse, "s3://my-bucket/wh");
        assert_eq!(cfg.lakehouse.project_id, Some("proj-123".to_string()));

        assert_eq!(cfg.profile.max_hash_files, 100);
        assert_eq!(cfg.profile.hash_block_size, 4096);
        assert_eq!(cfg.profile.max_no_extension_examples, 5);
        assert_eq!(cfg.profile.max_largest_files, 3);

        assert!(!cfg.ignore.hidden);
        assert!(!cfg.ignore.system);
        assert_eq!(cfg.ignore.patterns, vec!["*.tmp", "build"]);

        assert!(!cfg.external_tools.enabled);

        assert_eq!(cfg.categories.get("rs").unwrap(), "Code");
        assert_eq!(cfg.categories.get("toml").unwrap(), "Config");
    }

    #[test]
    fn load_explicit_path_partial_toml_fills_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, PARTIAL_TOML).unwrap();

        let cfg = Config::load(Some(&path)).unwrap();

        assert_eq!(cfg.lakehouse.s3_endpoint, "http://custom:9000");
        // Rest should be defaults
        assert_eq!(cfg.lakehouse.bucket, "anti-entropator");
        assert_eq!(cfg.profile.max_hash_files, 5000);
        assert!(cfg.ignore.hidden);
        assert!(cfg.external_tools.enabled);
    }

    #[test]
    fn load_nonexistent_explicit_path_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.toml");
        let cfg = Config::load(Some(&path)).unwrap();
        assert_eq!(cfg.lakehouse.s3_endpoint, default_s3_endpoint());
    }

    #[test]
    fn load_none_with_no_files_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();

        let cfg = Config::load(None).unwrap();
        assert_eq!(cfg.lakehouse.s3_endpoint, default_s3_endpoint());
        assert_eq!(cfg.profile.max_hash_files, 5000);

        std::env::set_current_dir(original_dir).unwrap();
    }

    #[test]
    fn load_malformed_toml_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "this is not [valid toml {{{").unwrap();

        let result = Config::load(Some(&path));
        assert!(result.is_err());
    }

    #[test]
    fn load_empty_toml_uses_all_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.toml");
        std::fs::write(&path, "").unwrap();

        let cfg = Config::load(Some(&path)).unwrap();
        assert_eq!(cfg.lakehouse.s3_endpoint, default_s3_endpoint());
        assert_eq!(cfg.lakehouse.bucket, default_bucket());
        assert_eq!(cfg.profile.max_hash_files, 5000);
        assert!(cfg.ignore.hidden);
    }

    #[test]
    fn config_serialization_roundtrip() {
        let original = Config::default();
        let toml_str = toml::to_string(&original).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(
            deserialized.lakehouse.s3_endpoint,
            original.lakehouse.s3_endpoint
        );
        assert_eq!(deserialized.lakehouse.bucket, original.lakehouse.bucket);
        assert_eq!(
            deserialized.profile.max_hash_files,
            original.profile.max_hash_files
        );
        assert_eq!(deserialized.ignore.patterns, original.ignore.patterns);
        assert_eq!(
            deserialized.external_tools.enabled,
            original.external_tools.enabled
        );
    }

    #[test]
    fn lakehouse_config_default_matches_helper_fns() {
        let cfg = LakehouseConfig::default();
        assert_eq!(cfg.s3_endpoint, default_s3_endpoint());
        assert_eq!(cfg.s3_endpoint_internal, default_s3_endpoint_internal());
        assert_eq!(cfg.s3_access_key, default_s3_access_key());
        assert_eq!(cfg.s3_secret_key, default_s3_secret_key());
        assert_eq!(cfg.bucket, default_bucket());
        assert_eq!(cfg.catalog_endpoint, default_catalog_endpoint());
        assert_eq!(cfg.warehouse, default_warehouse());
    }

    #[test]
    fn ignore_config_default_matches_helper_fns() {
        let cfg = IgnoreConfig::default();
        assert_eq!(cfg.patterns, default_ignore_patterns());
        assert_eq!(cfg.hidden, default_true());
        assert_eq!(cfg.system, default_true());
    }

    #[test]
    fn profile_config_default_matches_helper_fns() {
        let cfg = ProfileConfig::default();
        assert_eq!(cfg.max_hash_files, default_max_hash_files());
        assert_eq!(cfg.hash_block_size, default_hash_block_size());
        assert_eq!(cfg.max_no_extension_examples, default_max_no_ext());
        assert_eq!(cfg.max_largest_files, default_max_largest());
    }

    #[test]
    fn config_dir_does_not_panic() {
        let dir = dirs::config_dir();
        // May be None in sandboxed CI without HOME/XDG; just verify no panic
        if let Some(path) = dir {
            assert!(path.to_string_lossy().contains("anti_entropator"));
        }
    }

    #[test]
    fn load_toml_with_unknown_keys_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extra.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "[lakehouse]").unwrap();
        writeln!(f, "s3_endpoint = \"http://test:9000\"").unwrap();
        writeln!(f, "unknown_future_field = \"ignored\"").unwrap();

        // Serde default behavior: unknown fields are ignored
        let cfg = Config::load(Some(&path)).unwrap();
        assert_eq!(cfg.lakehouse.s3_endpoint, "http://test:9000");
    }
}
