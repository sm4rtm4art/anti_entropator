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
use std::sync::Arc;
use url::Url;

/// CLI shorthand for the canonical table, registered as a real table in the
/// session's default schema so SQL name resolution handles it (literals,
/// comments, and CTEs are untouched).
pub const FILES_ALIAS: &str = "files";

/// Fully qualified name of the canonical table, `iceberg.<namespace>.<table>`.
pub(crate) fn qualified_table_name() -> String {
    format!("iceberg.{}.{}", NAMESPACE, FILE_CATALOG_TABLE)
}

/// Register `files` as an alias of the canonical Iceberg table in the default
/// catalog/schema of `ctx`. Only the `query` command does this; ingest's state
/// read uses the qualified name.
///
/// Fails when the canonical table cannot be resolved (typically: `init` has
/// not run). Callers decide whether that is fatal.
pub(crate) async fn register_files_alias(ctx: &SessionContext) -> Result<()> {
    let qualified = qualified_table_name();
    let provider = ctx
        .table_provider(qualified.as_str())
        .await
        .with_context(|| format!("Cannot resolve {qualified} for the `{FILES_ALIAS}` shorthand"))?;
    ctx.register_table(FILES_ALIAS, provider)
        .with_context(|| format!("Cannot register `{FILES_ALIAS}` alias"))?;
    Ok(())
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

    // `files` is an alias table, not a text rewrite: the SQL is executed as
    // written. If the canonical table is missing the alias is skipped and a
    // query that references `files` fails with DataFusion's own error.
    if let Err(e) = register_files_alias(&ctx).await {
        tracing::warn!(error = %e, "`files` shorthand unavailable; run `init` first");
    }
    println!("  Executing query: {}", sql);
    let df = ctx.sql(&sql).await?;

    df.show().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Array, Int64Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use datafusion::catalog::{
        CatalogProvider, MemTable, MemoryCatalogProvider, MemorySchemaProvider, SchemaProvider,
    };

    /// A session whose `iceberg.anti_entropator.file_catalog` is an in-memory
    /// table with `n` rows, standing in for the REST catalog.
    fn session_with_canonical_table(n: usize) -> SessionContext {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("relative_path", DataType::Utf8, false),
        ]));
        let ids: Vec<i64> = (0..n as i64).collect();
        let paths: Vec<String> = ids.iter().map(|i| format!("p{i}.txt")).collect();
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int64Array::from(ids)),
                Arc::new(StringArray::from(paths)),
            ],
        )
        .unwrap();
        let table = MemTable::try_new(schema, vec![vec![batch]]).unwrap();

        let ns = MemorySchemaProvider::new();
        ns.register_table(FILE_CATALOG_TABLE.to_string(), Arc::new(table))
            .unwrap();
        let catalog = MemoryCatalogProvider::new();
        catalog.register_schema(NAMESPACE, Arc::new(ns)).unwrap();
        let ctx = SessionContext::new();
        ctx.register_catalog("iceberg", Arc::new(catalog));
        ctx
    }

    async fn count(ctx: &SessionContext, sql: &str) -> i64 {
        let batches = ctx.sql(sql).await.unwrap().collect().await.unwrap();
        let col = batches[0].column(0);
        col.as_any()
            .downcast_ref::<Int64Array>()
            .expect("count(*) is Int64")
            .value(0)
    }

    async fn strings(ctx: &SessionContext, sql: &str) -> Vec<String> {
        let batches = ctx.sql(sql).await.unwrap().collect().await.unwrap();
        batches
            .iter()
            .flat_map(|b| {
                let col = b.column(0).as_any().downcast_ref::<StringArray>().unwrap();
                (0..col.len())
                    .map(|i| col.value(i).to_string())
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    #[tokio::test]
    async fn files_alias_resolves_to_the_canonical_table() {
        let ctx = session_with_canonical_table(4);
        register_files_alias(&ctx).await.unwrap();
        let fqn = qualified_table_name();
        assert_eq!(
            count(&ctx, "SELECT count(*) FROM files").await,
            count(&ctx, &format!("SELECT count(*) FROM {fqn}")).await
        );
        // Quoted, and the alias's own qualified name, both resolve.
        assert_eq!(count(&ctx, r#"SELECT count(*) FROM "files""#).await, 4);
        assert_eq!(
            count(&ctx, "SELECT count(*) FROM datafusion.public.files").await,
            4
        );
        // JOIN position and column qualification through the alias.
        assert_eq!(
            count(
                &ctx,
                "SELECT count(*) FROM files a JOIN files b ON a.id = b.id"
            )
            .await,
            4
        );
    }

    #[tokio::test]
    async fn literals_and_comments_containing_from_files_are_not_rewritten() {
        let ctx = session_with_canonical_table(1);
        register_files_alias(&ctx).await.unwrap();
        // The regex used to turn this literal into the qualified name.
        assert_eq!(
            strings(&ctx, "SELECT 'FROM files' AS label").await,
            vec!["FROM files"]
        );
        assert_eq!(
            strings(&ctx, "SELECT 'JOIN files' AS label FROM files").await,
            vec!["JOIN files"]
        );
        // A comment stays a comment; the query still runs.
        assert_eq!(
            count(&ctx, "SELECT count(*) FROM files -- see: FROM files\n").await,
            1
        );
    }

    #[tokio::test]
    async fn a_cte_named_files_follows_sql_scoping() {
        let ctx = session_with_canonical_table(5);
        register_files_alias(&ctx).await.unwrap();
        // Inside the query the CTE shadows the alias; the regex rewrite used to
        // redirect the outer FROM to the catalog table instead.
        assert_eq!(
            count(
                &ctx,
                "WITH files AS (SELECT 1 AS id) SELECT count(*) FROM files"
            )
            .await,
            1
        );
        // Without the CTE, the alias is back.
        assert_eq!(count(&ctx, "SELECT count(*) FROM files").await, 5);
    }

    #[tokio::test]
    async fn alias_registration_fails_clearly_without_the_canonical_table() {
        let ctx = SessionContext::new();
        let err = register_files_alias(&ctx).await.unwrap_err().to_string();
        assert!(
            err.contains("iceberg.anti_entropator.file_catalog"),
            "{err}"
        );
        assert!(err.contains("`files` shorthand"), "{err}");
        // The session itself is still usable for queries that do not need it.
        assert_eq!(
            count(&ctx, "SELECT count(*) FROM (VALUES (1), (2))").await,
            2
        );
    }
}
