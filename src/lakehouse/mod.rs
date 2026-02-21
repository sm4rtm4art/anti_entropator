//! Lakehouse module - Stack operations (up, init)
//!
//! Handles connectivity to RustFS and Lakekeeper, and initializes the warehouse.
//!
//! Uses Lakekeeper's DEFAULT PROJECT which doesn't require X-Project-Id headers.

pub mod schema;
pub mod writer;

use anyhow::{bail, Context, Result};
use console::{style, Emoji};
use schema::{build_file_catalog_schema, FILE_CATALOG_TABLE, NAMESPACE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[FAIL] ");

/// The warehouse name in Lakekeeper
const WAREHOUSE_NAME: &str = "anti-entropator";

/// Configuration for lakehouse connections
#[derive(Debug, Clone)]
pub struct LakehouseConfig {
    pub s3_endpoint: String,
    /// S3 endpoint as seen from within Docker network (for Lakekeeper)
    pub s3_endpoint_internal: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub bucket: String,
    pub catalog_endpoint: String,
    pub warehouse: String,
}

impl Default for LakehouseConfig {
    fn default() -> Self {
        Self {
            s3_endpoint: std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8200".to_string()),
            s3_endpoint_internal: std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL")
                .unwrap_or_else(|_| "http://rustfs:9000".to_string()),
            s3_access_key: std::env::var("RUSTFS_ACCESS_KEY")
                .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
                .unwrap_or_default(),
            s3_secret_key: std::env::var("RUSTFS_SECRET_KEY")
                .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
                .unwrap_or_default(),
            bucket: std::env::var("ANTI_ENTROPATOR_BUCKET")
                .unwrap_or_else(|_| "anti-entropator".to_string()),
            catalog_endpoint: std::env::var("ANTI_ENTROPATOR_CATALOG_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8100".to_string()),
            warehouse: std::env::var("ANTI_ENTROPATOR_WAREHOUSE")
                .unwrap_or_else(|_| WAREHOUSE_NAME.to_string()),
        }
    }
}

// ============================================================================
// Lakekeeper Management API types
// ============================================================================

#[derive(Debug, Serialize)]
struct CreateWarehouseRequest {
    #[serde(rename = "warehouse-name")]
    warehouse_name: String,
    #[serde(rename = "storage-profile")]
    storage_profile: S3StorageProfile,
    #[serde(rename = "storage-credential")]
    storage_credential: S3StorageCredential,
}

#[derive(Debug, Serialize)]
struct S3StorageProfile {
    #[serde(rename = "type")]
    profile_type: String,
    bucket: String,
    region: String,
    endpoint: String,
    #[serde(rename = "path-style-access")]
    path_style_access: bool,
    #[serde(rename = "key-prefix")]
    key_prefix: String,
    #[serde(rename = "sts-enabled")]
    sts_enabled: bool,
}

#[derive(Debug, Serialize)]
struct S3StorageCredential {
    #[serde(rename = "type")]
    cred_type: String,
    #[serde(rename = "credential-type")]
    credential_type: String,
    #[serde(rename = "aws-access-key-id")]
    aws_access_key_id: String,
    #[serde(rename = "aws-secret-access-key")]
    aws_secret_access_key: String,
}

#[derive(Debug, Deserialize)]
struct WarehouseInfo {
    #[allow(dead_code)]
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ListWarehousesResponse {
    warehouses: Vec<WarehouseInfo>,
}

/// Check if the lakehouse stack is up and running
pub async fn check_up() -> Result<()> {
    println!();
    println!("{}", style("Checking lakehouse stack...").cyan().bold());
    println!();

    let config = LakehouseConfig::default();
    let mut all_ok = true;

    // Check RustFS
    print!("  RustFS ({})... ", config.s3_endpoint);
    match check_rustfs(&config).await {
        Ok(_) => println!("{}", style("OK").green()),
        Err(e) => {
            println!("{} {}", CROSS, style(e.to_string()).red());
            all_ok = false;
        }
    }

    // Check Lakekeeper
    print!("  Lakekeeper ({})... ", config.catalog_endpoint);
    match check_catalog(&config).await {
        Ok(_) => println!("{}", style("OK").green()),
        Err(e) => {
            println!("{} {}", CROSS, style(e.to_string()).red());
            all_ok = false;
        }
    }

    println!();

    if all_ok {
        println!("{} Lakehouse stack is ready!", CHECK);
        Ok(())
    } else {
        bail!("Some services are not available. Run `docker compose up -d` to start them.")
    }
}

/// Initialize the lakehouse (create bucket, warehouse, namespace, and table)
pub async fn init() -> Result<()> {
    println!();
    println!(
        "{}",
        style("Initializing Anti-Entropator lakehouse...")
            .cyan()
            .bold()
    );
    println!();

    let config = LakehouseConfig::default();
    validate_s3_credentials(&config)?;

    // First check connectivity
    check_rustfs(&config)
        .await
        .context("RustFS not available. Run `docker compose up -d`")?;
    check_catalog(&config)
        .await
        .context("Lakekeeper not available. Run `docker compose up -d`")?;

    // Create bucket if it doesn't exist
    print!("  Creating S3 bucket '{}'... ", config.bucket);
    match create_bucket(&config).await {
        Ok(created) => {
            if created {
                println!("{}", style("created").green());
            } else {
                println!("{}", style("already exists").yellow());
            }
        }
        Err(e) => {
            println!("{}", style(format!("error: {}", e)).red());
            return Err(e);
        }
    }

    // Create Lakekeeper warehouse (uses built-in default project, no X-Project-Id needed)
    print!("  Creating Lakekeeper warehouse '{}'... ", WAREHOUSE_NAME);
    match ensure_warehouse(&config).await {
        Ok(created) => {
            if created {
                println!("{}", style("created").green());
            } else {
                println!("{}", style("already exists").yellow());
            }
        }
        Err(e) => {
            println!("{}", style(format!("error: {}", e)).red());
            return Err(e);
        }
    }

    // Create namespace using iceberg-rust client
    print!("  Creating Iceberg namespace '{}'... ", NAMESPACE);
    match create_namespace(&config).await {
        Ok(created) => {
            if created {
                println!("{}", style("created").green());
            } else {
                println!("{}", style("already exists").yellow());
            }
        }
        Err(e) => {
            println!("{}", style(format!("error: {}", e)).red());
            return Err(e);
        }
    }

    print!(
        "  Creating Iceberg table '{}.{}'... ",
        NAMESPACE, FILE_CATALOG_TABLE
    );
    match create_file_catalog_table(&config).await {
        Ok(created) => {
            if created {
                println!("{}", style("created").green());
            } else {
                println!("{}", style("already exists").yellow());
            }
        }
        Err(e) => {
            println!("{}", style(format!("error: {}", e)).red());
            return Err(e);
        }
    }

    println!();
    println!("{} Lakehouse initialized!", CHECK);
    println!();
    println!("  Bucket:    s3://{}", config.bucket);
    println!("  Warehouse: {}", WAREHOUSE_NAME);
    println!("  Catalog:   {}", config.catalog_endpoint);
    println!("  Table:     {}.{}", NAMESPACE, FILE_CATALOG_TABLE);

    Ok(())
}

fn validate_s3_credentials(config: &LakehouseConfig) -> Result<()> {
    if config.s3_access_key.trim().is_empty() || config.s3_secret_key.trim().is_empty() {
        bail!("Missing S3 credentials. Set RUSTFS_ACCESS_KEY and RUSTFS_SECRET_KEY in .env")
    } else {
        Ok(())
    }
}

async fn check_rustfs(config: &LakehouseConfig) -> Result<()> {
    tracing::debug!(endpoint = %config.s3_endpoint, "Checking RustFS connectivity");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let resp = client
        .get(&config.s3_endpoint)
        .send()
        .await
        .context("Cannot connect to RustFS")?;

    // 403 is expected without auth but means server is up
    if resp.status().is_success() || resp.status().as_u16() == 403 {
        tracing::debug!("RustFS is available");
        Ok(())
    } else {
        tracing::error!(status = %resp.status(), "RustFS returned unexpected status");
        bail!("RustFS returned unexpected status: {}", resp.status())
    }
}

async fn check_catalog(config: &LakehouseConfig) -> Result<()> {
    tracing::debug!(endpoint = %config.catalog_endpoint, "Checking Lakekeeper connectivity");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let swagger_url = format!("{}/swagger-ui/", base);

    let resp = client
        .get(&swagger_url)
        .send()
        .await
        .context("Cannot connect to Lakekeeper")?;

    if resp.status().is_success() {
        tracing::debug!("Lakekeeper is available");
        Ok(())
    } else {
        tracing::error!(status = %resp.status(), "Lakekeeper returned unexpected status");
        bail!("Lakekeeper returned unexpected status: {}", resp.status())
    }
}

async fn create_bucket(config: &LakehouseConfig) -> Result<bool> {
    use aws_config::BehaviorVersion;
    use aws_sdk_s3::config::{Credentials, Region};

    tracing::debug!(bucket = %config.bucket, endpoint = %config.s3_endpoint, "Creating S3 bucket");

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

    let client = aws_sdk_s3::Client::from_conf(s3_config);

    // Check if bucket exists
    match client.head_bucket().bucket(&config.bucket).send().await {
        Ok(_) => {
            tracing::debug!(bucket = %config.bucket, "Bucket already exists");
            return Ok(false);
        }
        Err(e) => {
            let is_not_found = e
                .raw_response()
                .map(|r| r.status().as_u16() == 404)
                .unwrap_or(false);

            if is_not_found {
                tracing::debug!(bucket = %config.bucket, "Bucket does not exist, will create");
            } else {
                tracing::warn!(bucket = %config.bucket, error = %e, "HEAD bucket failed, will try to create anyway");
            }
        }
    }

    client
        .create_bucket()
        .bucket(&config.bucket)
        .send()
        .await
        .context("Failed to create bucket")?;

    tracing::info!(bucket = %config.bucket, "Created S3 bucket");
    Ok(true)
}

/// Create warehouse in Lakekeeper's DEFAULT PROJECT (no X-Project-Id header needed)
async fn ensure_warehouse(config: &LakehouseConfig) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let warehouses_url = format!("{}/management/v1/warehouse", base);

    tracing::debug!(url = %warehouses_url, "Listing warehouses");

    // List existing warehouses (default project, no header needed)
    let resp = client
        .get(&warehouses_url)
        .send()
        .await
        .context("Failed to list warehouses")?;

    if resp.status().is_success() {
        let list: ListWarehousesResponse = resp
            .json()
            .await
            .context("Failed to parse warehouses list")?;

        let warehouse_names: Vec<_> = list.warehouses.iter().map(|w| w.name.as_str()).collect();
        tracing::debug!(warehouses = ?warehouse_names, "Found warehouses");

        if list.warehouses.iter().any(|w| w.name == WAREHOUSE_NAME) {
            tracing::debug!(warehouse = %WAREHOUSE_NAME, "Warehouse already exists");
            return Ok(false);
        }
    }

    tracing::debug!(warehouse = %WAREHOUSE_NAME, "Creating warehouse");

    // Create the warehouse
    let create_req = CreateWarehouseRequest {
        warehouse_name: WAREHOUSE_NAME.to_string(),
        storage_profile: S3StorageProfile {
            profile_type: "s3".to_string(),
            bucket: config.bucket.clone(),
            region: "us-east-1".to_string(),
            endpoint: config.s3_endpoint_internal.clone(),
            path_style_access: true,
            key_prefix: "warehouse".to_string(),
            sts_enabled: false,
        },
        storage_credential: S3StorageCredential {
            cred_type: "s3".to_string(),
            credential_type: "access-key".to_string(),
            aws_access_key_id: config.s3_access_key.clone(),
            aws_secret_access_key: config.s3_secret_key.clone(),
        },
    };

    let resp = client
        .post(&warehouses_url)
        .json(&create_req)
        .send()
        .await
        .context("Failed to create warehouse")?;

    match resp.status().as_u16() {
        200 | 201 => {
            tracing::info!(warehouse = %WAREHOUSE_NAME, "Created warehouse");
            Ok(true)
        }
        409 => {
            tracing::debug!(warehouse = %WAREHOUSE_NAME, "Warehouse already exists (409)");
            Ok(false)
        }
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(warehouse = %WAREHOUSE_NAME, status = %status, body = %body, "Failed to create warehouse");
            bail!("Failed to create warehouse: {} - {}", status, body);
        }
    }
}

/// Get the warehouse prefix from Lakekeeper (for building REST API paths)
/// Result of fetching catalog configuration from Lakekeeper
pub struct CatalogConfigResult {
    /// The warehouse prefix (UUID) used in API paths
    pub prefix: String,
    /// The canonical URI for the catalog (e.g., http://localhost:8100/catalog)
    pub uri: String,
}

pub async fn get_warehouse_prefix(config: &LakehouseConfig) -> Result<CatalogConfigResult> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let config_url = format!("{}/catalog/v1/config?warehouse={}", base, config.warehouse);

    tracing::debug!(url = %config_url, warehouse = %config.warehouse, "Getting catalog config");

    let resp = client
        .get(&config_url)
        .send()
        .await
        .context("Failed to get catalog config")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Failed to get catalog config: {} - {}", status, body);
    }

    #[derive(Deserialize)]
    struct CatalogConfig {
        defaults: HashMap<String, String>,
        overrides: HashMap<String, String>,
    }

    let config_resp: CatalogConfig = resp
        .json()
        .await
        .context("Failed to parse catalog config")?;

    tracing::debug!(defaults = ?config_resp.defaults, overrides = ?config_resp.overrides, "Received catalog config");

    let prefix = config_resp
        .defaults
        .get("prefix")
        .cloned()
        .context("No prefix in catalog config")?;

    // Use the URI from overrides if available, otherwise fall back to our configured endpoint
    let uri = config_resp
        .overrides
        .get("uri")
        .cloned()
        .unwrap_or_else(|| config.catalog_endpoint.clone());

    Ok(CatalogConfigResult { prefix, uri })
}

/// Create the namespace using direct HTTP (Lakekeeper REST API)
async fn create_namespace(config: &LakehouseConfig) -> Result<bool> {
    let catalog_config = get_warehouse_prefix(config).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');

    // Check if namespace exists
    let check_url = format!(
        "{}/catalog/v1/{}/namespaces/{}",
        base, catalog_config.prefix, NAMESPACE
    );

    tracing::debug!(url = %check_url, namespace = %NAMESPACE, "Checking namespace existence");

    if let Ok(resp) = client.head(&check_url).send().await {
        if resp.status().is_success() {
            tracing::debug!(namespace = %NAMESPACE, "Namespace already exists");
            return Ok(false);
        }
    }

    tracing::debug!(namespace = %NAMESPACE, "Creating namespace");

    // Create namespace
    let create_url = format!("{}/catalog/v1/{}/namespaces", base, catalog_config.prefix);

    #[derive(Serialize)]
    struct CreateNamespaceRequest {
        namespace: Vec<String>,
    }

    let req = CreateNamespaceRequest {
        namespace: vec![NAMESPACE.to_string()],
    };

    let resp = client
        .post(&create_url)
        .json(&req)
        .send()
        .await
        .context("Failed to create namespace")?;

    match resp.status().as_u16() {
        200 | 201 => {
            tracing::info!(namespace = %NAMESPACE, "Created namespace");
            Ok(true)
        }
        409 => {
            tracing::debug!(namespace = %NAMESPACE, "Namespace already exists (409)");
            Ok(false)
        }
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(namespace = %NAMESPACE, status = %status, body = %body, "Failed to create namespace");
            bail!("Failed to create namespace: {} - {}", status, body);
        }
    }
}

/// Create the file_catalog table using direct HTTP (Lakekeeper REST API)
async fn create_file_catalog_table(config: &LakehouseConfig) -> Result<bool> {
    let catalog_config = get_warehouse_prefix(config).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');

    // Check if table exists
    let check_url = format!(
        "{}/catalog/v1/{}/namespaces/{}/tables/{}",
        base, catalog_config.prefix, NAMESPACE, FILE_CATALOG_TABLE
    );

    tracing::debug!(url = %check_url, table = %FILE_CATALOG_TABLE, "Checking table existence");

    if let Ok(resp) = client.head(&check_url).send().await {
        if resp.status().is_success() {
            tracing::debug!(table = %FILE_CATALOG_TABLE, "Table already exists");
            return Ok(false);
        }
    }

    tracing::debug!(table = %FILE_CATALOG_TABLE, "Creating table");

    // Build schema
    let schema = build_file_catalog_schema()?;

    // Create table
    let create_url = format!(
        "{}/catalog/v1/{}/namespaces/{}/tables",
        base, catalog_config.prefix, NAMESPACE
    );

    #[derive(Serialize)]
    struct CreateTableRequest {
        name: String,
        schema: serde_json::Value,
    }

    let schema_json = serde_json::to_value(&schema).context("Failed to serialize schema")?;

    let req = CreateTableRequest {
        name: FILE_CATALOG_TABLE.to_string(),
        schema: schema_json,
    };

    let resp = client
        .post(&create_url)
        .json(&req)
        .send()
        .await
        .context("Failed to create table")?;

    match resp.status().as_u16() {
        200 | 201 => {
            tracing::info!(table = %FILE_CATALOG_TABLE, "Created table");
            Ok(true)
        }
        409 => {
            tracing::debug!(table = %FILE_CATALOG_TABLE, "Table already exists (409)");
            Ok(false)
        }
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(table = %FILE_CATALOG_TABLE, status = %status, body = %body, "Failed to create table");
            bail!("Failed to create table: {} - {}", status, body);
        }
    }
}
