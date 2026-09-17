//! Additive schema evolution for `file_catalog`.
//!
//! `init` calls [`ensure_table_schema`] after the table exists so tables
//! created before the ADR-009 observation columns gain them in place. Only
//! optional root-level columns are ever added; nothing is renamed, retyped, or
//! dropped. Field ids are assigned by the catalog from `last_column_id`, which
//! for an untouched legacy table yields exactly the ids the code schema
//! expects; [`verify_table_schema`] confirms that after the commit.

use crate::lakehouse::schema::{missing_fields, verify_table_schema};
use crate::lakehouse::{build_rest_catalog, writer::load_table, LakehouseConfig};
use anyhow::{bail, Context, Result};
use iceberg::transaction::{AddColumn, ApplyTransactionAction, Transaction};

/// Bring the live table schema up to the code schema by adding any missing
/// optional columns. Returns the names of the columns added (empty when the
/// table was already current). Idempotent.
pub async fn ensure_table_schema(config: &LakehouseConfig) -> Result<Vec<String>> {
    let catalog = build_rest_catalog(config).await?;
    let table = load_table(&catalog).await?;

    let missing = missing_fields(table.metadata().current_schema())?;
    if missing.is_empty() {
        verify_table_schema(table.metadata().current_schema())?;
        return Ok(Vec::new());
    }

    let transaction = Transaction::new(&table);
    let mut action = transaction.update_schema();
    let mut added = Vec::with_capacity(missing.len());
    for field in &missing {
        if field.required {
            bail!(
                "cannot add required column '{}' to an existing table; additive evolution only supports optional columns",
                field.name
            );
        }
        tracing::info!(column = %field.name, r#type = %field.field_type, "Adding column to file_catalog");
        action = action.add_column(AddColumn::optional(
            &field.name,
            field.field_type.as_ref().clone(),
        ));
        added.push(field.name.clone());
    }

    let transaction = action
        .apply(transaction)
        .context("Failed to apply schema update")?;
    let table = transaction
        .commit(&catalog)
        .await
        .context("Failed to commit schema update")?;

    verify_table_schema(table.metadata().current_schema()).context(
        "schema update committed but the resulting table schema does not match the catalog definition",
    )?;

    Ok(added)
}
