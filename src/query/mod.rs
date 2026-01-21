//! Query module - Execute SQL queries via DataFusion
//!
//! Connects DataFusion to the Iceberg table and executes SQL.

use crate::lakehouse::schema::{FILE_CATALOG_TABLE, NAMESPACE};
use crate::lakehouse::LakehouseConfig;
use anyhow::{Context, Result};
use datafusion::prelude::*;
use iceberg::CatalogBuilder;
use iceberg_catalog_rest::{RestCatalog, RestCatalogBuilder};
use iceberg_datafusion::IcebergCatalogProvider;
use std::collections::HashMap;
use std::sync::Arc;

/// Run a one-shot SQL query
pub async fn run(sql: String) -> Result<()> {
    let config = LakehouseConfig::default();

    // 1. Initialize Catalog
    let mut props = HashMap::new();

    // Fetch the correct prefix and URI for this warehouse (Lakekeeper specific)
    let catalog_config = crate::lakehouse::get_warehouse_prefix(&config).await?;
    props.insert("uri".to_string(), catalog_config.uri.clone());
    props.insert("prefix".to_string(), catalog_config.prefix.clone());
    props.insert("warehouse".to_string(), config.warehouse.clone());

    let catalog: RestCatalog = RestCatalogBuilder::default()
        .load("anti_entropator", props)
        .await
        .context("Failed to build RestCatalog")?;

    // 2. Setup DataFusion
    let ctx = SessionContext::new();

    // 3. Register Iceberg Catalog
    // Use IcebergCatalogProvider to bridge the entire Iceberg catalog into DataFusion
    let catalog_provider = IcebergCatalogProvider::try_new(Arc::new(catalog))
        .await
        .context("Failed to create IcebergCatalogProvider")?;

    ctx.register_catalog("iceberg", Arc::new(catalog_provider));

    // 4. Execute Query
    // Map 'files' to the full path in the Iceberg catalog if present
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
