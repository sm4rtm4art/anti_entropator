//! Query module - Execute SQL queries via DataFusion
//!
//! Connects DataFusion to the Iceberg table and executes SQL.
//! Storage access is routed through OpenDAL via `object_store_opendal`.

use crate::lakehouse::schema::{FILE_CATALOG_TABLE, NAMESPACE};
use crate::lakehouse::LakehouseConfig;
use crate::storage;
use anyhow::{Context, Result};
use datafusion::prelude::*;
use iceberg::CatalogBuilder;
use iceberg_catalog_rest::{RestCatalog, RestCatalogBuilder};
use iceberg_datafusion::IcebergCatalogProvider;
use object_store::ObjectStore;
use object_store_opendal::OpendalStore;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;

/// Run a one-shot SQL query
pub async fn run(sql: String) -> Result<()> {
    let config = LakehouseConfig::default();

    // 1. Initialize Catalog
    let catalog_config = crate::lakehouse::get_warehouse_prefix(&config).await?;

    let mut props = HashMap::new();
    props.insert("uri".to_string(), catalog_config.uri.clone());
    props.insert("prefix".to_string(), catalog_config.prefix.clone());
    props.insert("warehouse".to_string(), config.warehouse.clone());
    props.insert("header.X-Project-Id".to_string(), catalog_config.project_id);

    // Override S3 config to use host-accessible endpoint with direct credentials
    // (Lakekeeper stores internal Docker endpoints which are unreachable from host)
    props.insert("s3.endpoint".to_string(), config.s3_endpoint.clone());
    props.insert("s3.access-key-id".to_string(), config.s3_access_key.clone());
    props.insert(
        "s3.secret-access-key".to_string(),
        config.s3_secret_key.clone(),
    );
    props.insert("s3.region".to_string(), "us-east-1".to_string());
    props.insert("s3.path-style-access".to_string(), "true".to_string());
    props.insert("s3.allow-http".to_string(), "true".to_string());
    props.insert("s3.remote-signing-enabled".to_string(), "false".to_string());

    let catalog: RestCatalog = RestCatalogBuilder::default()
        .load("anti_entropator", props)
        .await
        .context("Failed to build RestCatalog")?;

    // 2. Setup DataFusion with OpenDAL-backed ObjectStore
    let ctx = SessionContext::new();

    let operator = storage::create_operator(&config)?;
    let opendal_store = Arc::new(OpendalStore::new(operator));
    let s3_url =
        Url::parse(&format!("s3://{}", config.bucket)).context("Failed to parse bucket URL")?;
    ctx.register_object_store(&s3_url, opendal_store as Arc<dyn ObjectStore>);

    // 3. Register Iceberg Catalog
    let catalog_provider = IcebergCatalogProvider::try_new(Arc::new(catalog))
        .await
        .context("Failed to create IcebergCatalogProvider")?;

    ctx.register_catalog("iceberg", Arc::new(catalog_provider));

    // 4. Execute Query
    let query_sql = sql.replace(
        "files",
        &format!("iceberg.{}.{}", NAMESPACE, FILE_CATALOG_TABLE),
    );
    println!("  Executing query: {}", query_sql);
    let df = ctx.sql(&query_sql).await?;

    // 5. Show Results
    df.show().await?;

    Ok(())
}

/// Start an interactive SQL REPL
pub async fn repl() -> Result<()> {
    println!("Interactive SQL REPL starting...");
    println!("(Not yet fully implemented - using one-shot query for now)");
    println!("Tip: Try 'SELECT category, count(*) FROM files GROUP BY category'");

    Ok(())
}
