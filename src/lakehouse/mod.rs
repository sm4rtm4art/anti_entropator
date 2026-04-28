//! Lakehouse module - Stack operations (up, init)
//!
//! Handles connectivity to RustFS and Lakekeeper, and initializes the warehouse.

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

/// The project name in Lakekeeper
const PROJECT_NAME: &str = "anti-entropator";

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
    /// Lakekeeper project ID (resolved at runtime via `ensure_project`).
    pub project_id: Option<String>,
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
            project_id: std::env::var("ANTI_ENTROPATOR_PROJECT_ID").ok(),
        }
    }
}

/// Resolve the project ID from (in order): config field, env var, state file.
///
/// Does NOT create a new project -- returns an error if none is found.
/// Run `init` first to bootstrap the project.
pub async fn resolve_project_id(config: &LakehouseConfig) -> Result<String> {
    if let Some(ref id) = config.project_id {
        return Ok(id.clone());
    }
    if let Some(id) = load_project_id_from_state() {
        return Ok(id);
    }
    bail!(
        "No Lakekeeper project ID found. Run `anti_entropator init` first, \
         or set ANTI_ENTROPATOR_PROJECT_ID in your environment."
    )
}

const STATE_FILE: &str = ".lakehouse_state.json";

fn save_project_id_to_state(project_id: &str) -> Result<()> {
    let state = serde_json::json!({ "project_id": project_id });
    std::fs::write(STATE_FILE, serde_json::to_string_pretty(&state)?)
        .context("Failed to write lakehouse state file")?;
    tracing::debug!(path = %STATE_FILE, "Saved project ID to state file");
    Ok(())
}

fn load_project_id_from_state() -> Option<String> {
    let data = std::fs::read_to_string(STATE_FILE).ok()?;
    let val: serde_json::Value = serde_json::from_str(&data).ok()?;
    val.get("project_id")
        .and_then(|v| v.as_str())
        .map(String::from)
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

    let mut config = LakehouseConfig::default();
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

    // Ensure Lakekeeper project exists (required since Lakekeeper >= 0.11)
    print!("  Creating Lakekeeper project '{}'... ", PROJECT_NAME);
    let project_id = if let Some(id) = config
        .project_id
        .clone()
        .or_else(load_project_id_from_state)
    {
        if verify_project_exists(&config, &id).await {
            println!("{}", style("already exists").yellow());
            id
        } else {
            tracing::debug!(stale_id = %id, "Stale project ID, creating new project");
            match ensure_project(&config).await {
                Ok((new_id, _)) => {
                    println!("{}", style("created").green());
                    new_id
                }
                Err(e) => {
                    println!("{}", style(format!("error: {}", e)).red());
                    return Err(e);
                }
            }
        }
    } else {
        match ensure_project(&config).await {
            Ok((id, _)) => {
                println!("{}", style("created").green());
                id
            }
            Err(e) => {
                println!("{}", style(format!("error: {}", e)).red());
                return Err(e);
            }
        }
    };
    config.project_id = Some(project_id.clone());
    if let Err(e) = save_project_id_to_state(&project_id) {
        tracing::warn!(error = %e, "Could not persist project ID to state file");
    }

    // Create Lakekeeper warehouse within the project
    print!("  Creating Lakekeeper warehouse '{}'... ", WAREHOUSE_NAME);
    match ensure_warehouse(&config, &project_id).await {
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

/// Check whether a project ID is still valid in Lakekeeper.
async fn verify_project_exists(config: &LakehouseConfig, project_id: &str) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    else {
        return false;
    };
    let url = format!(
        "{}/management/v1/project",
        config.catalog_endpoint.trim_end_matches('/')
    );
    let resp = client
        .get(&url)
        .header("X-Project-Id", project_id)
        .send()
        .await;
    matches!(resp, Ok(r) if r.status().is_success())
}

/// Ensure a Lakekeeper project exists, returning its UUID.
///
/// Lakekeeper >= 0.11 requires an explicit project before warehouses can be created.
async fn ensure_project(config: &LakehouseConfig) -> Result<(String, bool)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');

    // Try listing projects to find ours (Lakekeeper exposes GET /management/v1/project
    // but only with X-Project-Id, so we try to create and handle conflict).
    let create_url = format!("{}/management/v1/project", base);

    #[derive(Serialize)]
    struct CreateProjectRequest {
        #[serde(rename = "project-name")]
        project_name: String,
    }

    #[derive(Deserialize)]
    struct ProjectResponse {
        #[serde(rename = "project-id")]
        project_id: String,
    }

    let resp = client
        .post(&create_url)
        .json(&CreateProjectRequest {
            project_name: PROJECT_NAME.to_string(),
        })
        .send()
        .await
        .context("Failed to create project")?;

    match resp.status().as_u16() {
        200 | 201 => {
            let pr: ProjectResponse = resp
                .json()
                .await
                .context("Failed to parse project response")?;
            tracing::info!(project = %PROJECT_NAME, id = %pr.project_id, "Created project");
            Ok((pr.project_id, true))
        }
        409 => {
            // Already exists -- retrieve it
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if let Some(id) = body.get("project-id").and_then(|v| v.as_str()) {
                tracing::debug!(project = %PROJECT_NAME, id = %id, "Project already exists");
                return Ok((id.to_string(), false));
            }
            // If we can't extract the ID from the 409 body, list projects to find it
            bail!("Project already exists but could not extract project-id from response: {body}");
        }
        _ => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Failed to create project: {} - {}", status, body);
        }
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

/// Create an S3 bucket via direct HTTP with AWS SigV4 authentication.
///
/// Uses the S3 path-style API:
///   - `HEAD /{bucket}` to check existence
///   - `PUT /{bucket}` to create
async fn create_bucket(config: &LakehouseConfig) -> Result<bool> {
    tracing::debug!(bucket = %config.bucket, endpoint = %config.s3_endpoint, "Creating S3 bucket");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.s3_endpoint.trim_end_matches('/');
    let bucket_url = format!("{}/{}", base, config.bucket);
    let host = s3_host_from_endpoint(&config.s3_endpoint);

    // Check if bucket exists (HEAD /{bucket})
    let resp = send_signed_s3(
        &client,
        "HEAD",
        &bucket_url,
        &host,
        &format!("/{}", config.bucket),
        config,
    )
    .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            tracing::debug!(bucket = %config.bucket, "Bucket already exists");
            return Ok(false);
        }
        Ok(r) if r.status().as_u16() == 404 => {
            tracing::debug!(bucket = %config.bucket, "Bucket does not exist, will create");
        }
        Ok(r) => {
            tracing::warn!(bucket = %config.bucket, status = %r.status(),
                "HEAD bucket returned unexpected status, will try to create anyway");
        }
        Err(e) => {
            tracing::warn!(bucket = %config.bucket, error = %e,
                "HEAD bucket failed, will try to create anyway");
        }
    }

    // Create bucket (PUT /{bucket})
    let resp = send_signed_s3(
        &client,
        "PUT",
        &bucket_url,
        &host,
        &format!("/{}", config.bucket),
        config,
    )
    .await
    .context("Failed to create bucket")?;

    if resp.status().is_success() {
        tracing::info!(bucket = %config.bucket, "Created S3 bucket");
        Ok(true)
    } else if resp.status().as_u16() == 409 {
        tracing::debug!(bucket = %config.bucket, "Bucket already exists (409)");
        Ok(false)
    } else {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Failed to create bucket: {} - {}", status, body)
    }
}

// ============================================================================
// Minimal AWS Signature V4 signing for S3 bucket operations
// ============================================================================

/// Send an S3 request signed with AWS Signature V4 (empty body).
async fn send_signed_s3(
    client: &reqwest::Client,
    method: &str,
    url: &str,
    host: &str,
    canonical_uri: &str,
    config: &LakehouseConfig,
) -> Result<reqwest::Response> {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::{Digest, Sha256};

    let now = chrono::Utc::now();
    let date_stamp = now.format("%Y%m%d").to_string();
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let region = "us-east-1";
    let service = "s3";
    let empty_hash = crate::utils::to_lower_hex(&Sha256::digest(b""));

    // Canonical request
    let signed_headers = "host;x-amz-content-sha256;x-amz-date";
    let canonical_request = format!(
        "{method}\n{canonical_uri}\n\nhost:{host}\nx-amz-content-sha256:{empty_hash}\nx-amz-date:{amz_date}\n\n{signed_headers}\n{empty_hash}"
    );

    let credential_scope = format!("{date_stamp}/{region}/{service}/aws4_request");
    let canonical_request_hash = crate::utils::to_lower_hex(&Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign =
        format!("AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{canonical_request_hash}");

    // Derive signing key
    type HmacSha256 = Hmac<Sha256>;
    let mut mac =
        HmacSha256::new_from_slice(format!("AWS4{}", config.s3_secret_key).as_bytes()).unwrap();
    mac.update(date_stamp.as_bytes());
    let date_key = mac.finalize().into_bytes();

    let mut mac = HmacSha256::new_from_slice(&date_key).unwrap();
    mac.update(region.as_bytes());
    let region_key = mac.finalize().into_bytes();

    let mut mac = HmacSha256::new_from_slice(&region_key).unwrap();
    mac.update(service.as_bytes());
    let service_key = mac.finalize().into_bytes();

    let mut mac = HmacSha256::new_from_slice(&service_key).unwrap();
    mac.update(b"aws4_request");
    let signing_key = mac.finalize().into_bytes();

    let mut mac = HmacSha256::new_from_slice(&signing_key).unwrap();
    mac.update(string_to_sign.as_bytes());
    let signature = crate::utils::to_lower_hex(mac.finalize().into_bytes().as_slice());

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}",
        config.s3_access_key
    );

    let builder = match method {
        "HEAD" => client.head(url),
        "PUT" => client.put(url),
        _ => client.get(url),
    };

    builder
        .header("Host", host)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &empty_hash)
        .header("Authorization", &authorization)
        .send()
        .await
        .map_err(Into::into)
}

fn s3_host_from_endpoint(endpoint: &str) -> String {
    endpoint
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string()
}

async fn ensure_warehouse(config: &LakehouseConfig, project_id: &str) -> Result<bool> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let warehouses_url = format!("{}/management/v1/warehouse", base);

    tracing::debug!(url = %warehouses_url, project_id = %project_id, "Listing warehouses");

    let resp = client
        .get(&warehouses_url)
        .header("X-Project-Id", project_id)
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
        .header("X-Project-Id", project_id)
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

/// Result of fetching catalog configuration from Lakekeeper.
pub struct CatalogConfigResult {
    /// The warehouse prefix (UUID) used in API paths
    pub prefix: String,
    /// The canonical URI for the catalog (e.g., http://localhost:8100/catalog)
    pub uri: String,
    /// The Lakekeeper project ID (required as `header.X-Project-Id` on REST catalog requests)
    pub project_id: String,
}

pub async fn get_warehouse_prefix(config: &LakehouseConfig) -> Result<CatalogConfigResult> {
    let project_id = resolve_project_id(config).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    let base = config.catalog_endpoint.trim_end_matches('/');
    let config_url = format!("{}/catalog/v1/config?warehouse={}", base, config.warehouse);

    tracing::debug!(url = %config_url, warehouse = %config.warehouse, project_id = %project_id, "Getting catalog config");

    let resp = client
        .get(&config_url)
        .header("X-Project-Id", &project_id)
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

    Ok(CatalogConfigResult {
        prefix,
        uri,
        project_id,
    })
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
