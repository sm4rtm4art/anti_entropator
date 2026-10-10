//! Configuration module
//!
//! Environment-backed application configuration.
//! [`LakehouseConfig`] is the single source of truth — re-exported by
//! `crate::lakehouse` so downstream code does not need to change imports.

/// Lakehouse connection configuration.
///
/// Single source of truth for S3 + Lakekeeper connectivity.
/// Values come from environment variables with static fallbacks; the CLI has
/// no config-file loader.
#[derive(Debug, Clone)]
pub struct LakehouseConfig {
    /// RustFS S3 endpoint (host-accessible)
    pub s3_endpoint: String,

    /// S3 endpoint as seen from within Docker network (for Lakekeeper)
    pub s3_endpoint_internal: String,

    /// RustFS access key
    pub s3_access_key: String,

    /// RustFS secret key
    pub s3_secret_key: String,

    /// S3 signing/storage region
    pub s3_region: String,

    /// Bucket name for data
    pub bucket: String,

    /// Iceberg REST catalog endpoint (Lakekeeper)
    pub catalog_endpoint: String,

    /// Warehouse name in Lakekeeper
    pub warehouse: String,

    /// Lakekeeper project ID (resolved at runtime via `ensure_project`).
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

fn default_s3_region() -> String {
    std::env::var("ANTI_ENTROPATOR_S3_REGION")
        .or_else(|_| std::env::var("AWS_REGION"))
        .unwrap_or_else(|_| "eu-central-1".to_string())
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
            s3_region: default_s3_region(),
            bucket: default_bucket(),
            catalog_endpoint: default_catalog_endpoint(),
            warehouse: default_warehouse(),
            project_id: std::env::var("ANTI_ENTROPATOR_PROJECT_ID").ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_expected_lakehouse_values() {
        let cfg = LakehouseConfig::default();
        // Env vars may override; check static fallbacks only when env is clean
        if std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT").is_err() {
            assert_eq!(cfg.s3_endpoint, "http://localhost:8200");
        }
        if std::env::var("ANTI_ENTROPATOR_CATALOG_ENDPOINT").is_err() {
            assert_eq!(cfg.catalog_endpoint, "http://localhost:8100");
        }
        if std::env::var("ANTI_ENTROPATOR_BUCKET").is_err() {
            assert_eq!(cfg.bucket, "anti-entropator");
        }
        if std::env::var("ANTI_ENTROPATOR_WAREHOUSE").is_err() {
            assert_eq!(cfg.warehouse, "anti-entropator");
        }
        if std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL").is_err() {
            assert_eq!(cfg.s3_endpoint_internal, "http://rustfs:9000");
        }
        if std::env::var("ANTI_ENTROPATOR_S3_REGION").is_err()
            && std::env::var("AWS_REGION").is_err()
        {
            assert_eq!(cfg.s3_region, "eu-central-1");
        }
        assert!(cfg.project_id.is_none() || std::env::var("ANTI_ENTROPATOR_PROJECT_ID").is_ok());
    }

    #[test]
    fn lakehouse_config_default_matches_helper_fns() {
        let cfg = LakehouseConfig::default();
        assert_eq!(cfg.s3_endpoint, default_s3_endpoint());
        assert_eq!(cfg.s3_endpoint_internal, default_s3_endpoint_internal());
        assert_eq!(cfg.s3_access_key, default_s3_access_key());
        assert_eq!(cfg.s3_secret_key, default_s3_secret_key());
        assert_eq!(cfg.s3_region, default_s3_region());
        assert_eq!(cfg.bucket, default_bucket());
        assert_eq!(cfg.catalog_endpoint, default_catalog_endpoint());
        assert_eq!(cfg.warehouse, default_warehouse());
    }
}
