//! Lakehouse module - Stack operations (up, init)
//!
//! Handles connectivity to RustFS and Nessie, and initializes the Iceberg table.

use anyhow::{bail, Context, Result};
use console::{style, Emoji};
use std::time::Duration;

static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[FAIL] ");

/// Configuration for lakehouse connections
#[derive(Debug, Clone)]
pub struct LakehouseConfig {
    pub s3_endpoint: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub bucket: String,
    pub nessie_endpoint: String,
    pub warehouse: String,
}

impl Default for LakehouseConfig {
    fn default() -> Self {
        Self {
            s3_endpoint: std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:9000".to_string()),
            s3_access_key: std::env::var("RUSTFS_ROOT_USER")
                .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
                .unwrap_or_else(|_| "antiuser".to_string()),
            s3_secret_key: std::env::var("RUSTFS_ROOT_PASSWORD")
                .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
                .unwrap_or_else(|_| "antipassword".to_string()),
            bucket: std::env::var("ANTI_ENTROPATOR_BUCKET")
                .unwrap_or_else(|_| "anti-entropator".to_string()),
            nessie_endpoint: std::env::var("ANTI_ENTROPATOR_NESSIE_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:19120/api/v1".to_string()),
            warehouse: std::env::var("ANTI_ENTROPATOR_WAREHOUSE")
                .unwrap_or_else(|_| "s3://anti-entropator/warehouse".to_string()),
        }
    }
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

    // Check Nessie
    print!("  Nessie ({})... ", config.nessie_endpoint);
    match check_nessie(&config).await {
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

/// Initialize the lakehouse (create bucket, table)
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
    check_nessie(&config)
        .await
        .context("Nessie not available. Run `docker compose up -d`")?;

    // Create bucket if it doesn't exist
    print!("  Creating bucket '{}'... ", config.bucket);
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

    // Note: Full Iceberg table creation requires more complex Nessie/Iceberg integration
    // For now we just verify connectivity and bucket creation
    println!(
        "  Creating Iceberg table 'file_catalog'... {}",
        style("skipped (requires Iceberg client)").dim()
    );

    println!();
    println!(
        "{} Lakehouse initialized! Bucket '{}' is ready.",
        CHECK, config.bucket
    );
    println!();
    println!("  Warehouse: {}", config.warehouse);
    println!("  Nessie:    {}", config.nessie_endpoint);
    println!();
    println!(
        "{}",
        style("Note: Full Iceberg table creation will be available in a future version.").dim()
    );

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

async fn check_nessie(config: &LakehouseConfig) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    // Try v2 config endpoint
    let base = config
        .nessie_endpoint
        .trim_end_matches("/api/v1")
        .trim_end_matches('/');
    let config_url = format!("{}/api/v2/config", base);

    let resp = client
        .get(&config_url)
        .send()
        .await
        .context("Cannot connect to Nessie")?;

    if resp.status().is_success() {
        Ok(())
    } else {
        bail!("Nessie returned unexpected status: {}", resp.status())
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
            // Check if it's a 404 (doesn't exist) or actual error
            let is_not_found = e
                .raw_response()
                .map(|r| r.status().as_u16() == 404)
                .unwrap_or(false);

            if !is_not_found {
                // Try to create anyway, might be permission issue on HEAD
            }
        }
    }

    // Create bucket
    client
        .create_bucket()
        .bucket(&config.bucket)
        .send()
        .await
        .context("Failed to create bucket")?;

    Ok(true)
}
