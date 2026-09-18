//! Query module - Execute SQL queries via DataFusion
//!
//! Connects DataFusion to the Iceberg table and executes SQL.
//! Storage access is routed through OpenDAL via `object_store_opendal`.

use crate::lakehouse::schema::{FILE_CATALOG_TABLE, NAMESPACE};
use crate::lakehouse::{build_rest_catalog, LakehouseConfig};
use crate::storage;
use anyhow::{Context, Result};
use datafusion::prelude::*;
use iceberg_catalog_rest::RestCatalog;
use iceberg_datafusion::IcebergCatalogProvider;
use object_store::ObjectStore;
use object_store_opendal::OpendalStore;
use std::sync::{Arc, LazyLock};
use url::Url;

static FILES_TABLE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)((?:FROM|JOIN)\s+)files\b")
        .expect("FILES_TABLE_RE is a valid static regex")
});

/// Fully qualified name of the canonical table, `iceberg.<namespace>.<table>`.
pub(crate) fn qualified_table_name() -> String {
    format!("iceberg.{}.{}", NAMESPACE, FILE_CATALOG_TABLE)
}

/// Build a DataFusion session with the Iceberg REST catalog registered as
/// `iceberg` and object access routed through OpenDAL.
///
/// Shared by the `query` command and by ingest's catalog read; it performs
/// no output so callers with a machine-readable stdout can use it.
pub(crate) async fn build_session(config: &LakehouseConfig) -> Result<SessionContext> {
    let catalog: RestCatalog = build_rest_catalog(config).await?;

    let ctx = SessionContext::new();

    let operator = storage::create_operator(config)?;
    let opendal_store = Arc::new(OpendalStore::new(operator));
    let s3_url =
        Url::parse(&format!("s3://{}", config.bucket)).context("Failed to parse bucket URL")?;
    ctx.register_object_store(&s3_url, opendal_store as Arc<dyn ObjectStore>);

    let catalog_provider = IcebergCatalogProvider::try_new(Arc::new(catalog))
        .await
        .context("Failed to create IcebergCatalogProvider")?;
    ctx.register_catalog("iceberg", Arc::new(catalog_provider));

    Ok(ctx)
}

/// Run a one-shot SQL query
pub async fn run(sql: String) -> Result<()> {
    let config = LakehouseConfig::default();
    let ctx = build_session(&config).await?;

    // Execute Query (rewrite `FROM files` / `JOIN files` shorthand)
    let query_sql = rewrite_table_reference(&sql);
    println!("  Executing query: {}", query_sql);
    let df = ctx.sql(&query_sql).await?;

    df.show().await?;

    Ok(())
}

/// Rewrite the `files` shorthand to the fully qualified Iceberg table name,
/// but only in table-reference positions (after FROM or JOIN keywords).
fn rewrite_table_reference(sql: &str) -> String {
    let qualified = qualified_table_name();
    FILES_TABLE_RE
        .replace_all(sql, format!("${{1}}{}", qualified))
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_from_files() {
        let result = rewrite_table_reference("SELECT * FROM files");
        assert_eq!(result, "SELECT * FROM iceberg.anti_entropator.file_catalog");
    }

    #[test]
    fn rewrite_join_files() {
        let result =
            rewrite_table_reference("SELECT * FROM other JOIN files ON other.id = files.id");
        assert!(result.contains("JOIN iceberg.anti_entropator.file_catalog ON"));
    }

    #[test]
    fn rewrite_preserves_filesize() {
        let result = rewrite_table_reference("SELECT filesize FROM files LIMIT 10");
        assert!(result.contains("filesize"));
        assert!(result.contains("FROM iceberg.anti_entropator.file_catalog"));
    }

    #[test]
    fn rewrite_preserves_profiles() {
        let result = rewrite_table_reference("SELECT * FROM files WHERE category = 'profiles'");
        assert!(result.contains("'profiles'"));
        assert!(result.contains("FROM iceberg.anti_entropator.file_catalog"));
    }

    #[test]
    fn rewrite_preserves_string_literal() {
        let result = rewrite_table_reference("SELECT 'files' AS label FROM files");
        assert!(result.contains("'files'"));
        assert!(result.contains("FROM iceberg.anti_entropator.file_catalog"));
    }

    #[test]
    fn rewrite_no_match() {
        let input = "SELECT * FROM iceberg.anti_entropator.file_catalog LIMIT 5";
        let result = rewrite_table_reference(input);
        assert_eq!(result, input);
    }
}
