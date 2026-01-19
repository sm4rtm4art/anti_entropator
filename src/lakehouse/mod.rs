//! Lakehouse module - Stack operations (up, init)
//!
//! Handles connectivity to RustFS and Lakekeeper, and initializes the warehouse.
//!
//! Uses Lakekeeper's DEFAULT PROJECT which doesn't require X-Project-Id headers.

use anyhow::{bail, Context, Result};
use console::{style, Emoji};
use iceberg::spec::{NestedField, PrimitiveType, Schema, Type};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[FAIL] ");

/// The namespace for anti-entropator tables
const NAMESPACE: &str = "anti_entropator";
/// The file catalog table name
const FILE_CATALOG_TABLE: &str = "file_catalog";
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
                .unwrap_or_else(|_| "http://localhost:19000".to_string()),
            s3_endpoint_internal: std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT_INTERNAL")
                .unwrap_or_else(|_| "http://rustfs:9000".to_string()),
            s3_access_key: std::env::var("RUSTFS_ACCESS_KEY")
                .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
                .unwrap_or_else(|_| "antiuser".to_string()),
            s3_secret_key: std::env::var("RUSTFS_SECRET_KEY")
                .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
                .unwrap_or_else(|_| "antipassword".to_string()),
            bucket: std::env::var("ANTI_ENTROPATOR_BUCKET")
                .unwrap_or_else(|_| "anti-entropator".to_string()),
            catalog_endpoint: std::env::var("ANTI_ENTROPATOR_CATALOG_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:8181".to_string()),
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

    // Create Lakekeeper warehouse in DEFAULT PROJECT (no header needed)
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

async fn check_rustfs(config: &LakehouseConfig) -> Result<()> {
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
        Ok(())
    } else {
        bail!("RustFS returned unexpected status: {}", resp.status())
    }
}

async fn check_catalog(config: &LakehouseConfig) -> Result<()> {
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
        Ok(())
    } else {
        bail!("Lakekeeper returned unexpected status: {}", resp.status())
    }
}

async fn create_bucket(config: &LakehouseConfig) -> Result<bool> {
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

    let client = aws_sdk_s3::Client::from_conf(s3_config);

    // Check if bucket exists
    match client.head_bucket().bucket(&config.bucket).send().await {
        Ok(_) => return Ok(false), // Already exists
        Err(e) => {
            let is_not_found = e
                .raw_response()
                .map(|r| r.status().as_u16() == 404)
                .unwrap_or(false);

            if !is_not_found {
                // Try to create anyway, might be permission issue on HEAD
            }
        }
    }

    client
        .create_bucket()
        .bucket(&config.bucket)
        .send()
        .await
        .context("Failed to create bucket")?;

    Ok(true)
}

/// Create warehouse in Lakekeeper's DEFAULT PROJECT (no X-Project-Id header needed)
async fn ensure_warehouse(config: &LakehouseConfig) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let warehouses_url = format!("{}/management/v1/warehouse", base);

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
        if list.warehouses.iter().any(|w| w.name == WAREHOUSE_NAME) {
            return Ok(false); // Already exists
        }
    }

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
        200 | 201 => Ok(true),
        409 => Ok(false), // Already exists
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Failed to create warehouse: {} - {}", status, body);
        }
    }
}

/// Get the warehouse prefix from Lakekeeper (for building REST API paths)
async fn get_warehouse_prefix(config: &LakehouseConfig) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let config_url = format!("{}/catalog/v1/config?warehouse={}", base, config.warehouse);

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
    }

    let config_resp: CatalogConfig = resp
        .json()
        .await
        .context("Failed to parse catalog config")?;
    config_resp
        .defaults
        .get("prefix")
        .cloned()
        .context("No prefix in catalog config")
}

/// Create the namespace using direct HTTP (Lakekeeper REST API)
async fn create_namespace(config: &LakehouseConfig) -> Result<bool> {
    let prefix = get_warehouse_prefix(config).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');

    // Check if namespace exists
    let check_url = format!("{}/catalog/v1/{}/namespaces/{}", base, prefix, NAMESPACE);
    if let Ok(resp) = client.head(&check_url).send().await {
        if resp.status().is_success() {
            return Ok(false); // Already exists
        }
    }

    // Create namespace
    let create_url = format!("{}/catalog/v1/{}/namespaces", base, prefix);

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
        200 | 201 => Ok(true),
        409 => Ok(false), // Already exists
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Failed to create namespace: {} - {}", status, body);
        }
    }
}

/// Build the file_catalog schema matching FileInfo structure
fn build_file_catalog_schema() -> Result<Schema> {
    let fields = vec![
        Arc::new(NestedField::required(
            1,
            "id",
            Type::Primitive(PrimitiveType::Uuid),
        )),
        Arc::new(NestedField::required(
            2,
            "source_path",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::required(
            3,
            "filename",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::required(
            4,
            "extension",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            5,
            "mime_type",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::required(
            6,
            "category",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::required(
            7,
            "size_bytes",
            Type::Primitive(PrimitiveType::Long),
        )),
        Arc::new(NestedField::optional(
            8,
            "content_hash",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            9,
            "partial_hash",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            10,
            "created_at",
            Type::Primitive(PrimitiveType::Timestamptz),
        )),
        Arc::new(NestedField::optional(
            11,
            "modified_at",
            Type::Primitive(PrimitiveType::Timestamptz),
        )),
        Arc::new(NestedField::required(
            12,
            "scanned_at",
            Type::Primitive(PrimitiveType::Timestamptz),
        )),
        Arc::new(NestedField::optional(
            13,
            "object_uri",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            14,
            "ingested_at",
            Type::Primitive(PrimitiveType::Timestamptz),
        )),
        Arc::new(NestedField::optional(
            15,
            "suggested_name",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            16,
            "name_reason",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::required(
            17,
            "is_duplicate",
            Type::Primitive(PrimitiveType::Boolean),
        )),
        Arc::new(NestedField::optional(
            18,
            "duplicate_of",
            Type::Primitive(PrimitiveType::Uuid),
        )),
    ];

    Schema::builder()
        .with_fields(fields)
        .with_identifier_field_ids([1])
        .build()
        .context("Failed to build file_catalog schema")
}

/// Create the file_catalog table using direct HTTP (Lakekeeper REST API)
async fn create_file_catalog_table(config: &LakehouseConfig) -> Result<bool> {
    let prefix = get_warehouse_prefix(config).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');

    // Check if table exists
    let check_url = format!(
        "{}/catalog/v1/{}/namespaces/{}/tables/{}",
        base, prefix, NAMESPACE, FILE_CATALOG_TABLE
    );
    if let Ok(resp) = client.head(&check_url).send().await {
        if resp.status().is_success() {
            return Ok(false); // Already exists
        }
    }

    // Build schema
    let schema = build_file_catalog_schema()?;

    // Create table
    let create_url = format!(
        "{}/catalog/v1/{}/namespaces/{}/tables",
        base, prefix, NAMESPACE
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
        200 | 201 => Ok(true),
        409 => Ok(false), // Already exists
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Failed to create table: {} - {}", status, body);
        }
    }
}
