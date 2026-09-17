//! Iceberg schema definition for the file catalog
//!
//! This module provides the authoritative schema for the `file_catalog` table.

use anyhow::{Context, Result};
use iceberg::spec::{NestedField, PrimitiveType, Schema, Type};
use std::sync::Arc;

/// The namespace for anti-entropator tables
pub const NAMESPACE: &str = "anti_entropator";
/// The file catalog table name
pub const FILE_CATALOG_TABLE: &str = "file_catalog";

/// Number of fields in the current schema. Legacy tables created before the
/// ADR-009 observation columns have [`LEGACY_FIELD_COUNT`] fields.
#[cfg(test)]
pub const FIELD_COUNT: usize = 25;
/// Field count of tables created before the observation columns were added.
#[cfg(test)]
pub const LEGACY_FIELD_COUNT: usize = 20;

/// Build the file_catalog schema matching FileInfo structure.
///
/// Fields 1-20 are the original catalog columns. Fields 21-25 are the ADR-009
/// observation columns; they are optional so tables and rows created before
/// them remain readable, and `init` adds them to existing tables in place.
pub fn build_file_catalog_schema() -> Result<Schema> {
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
        Arc::new(NestedField::required(
            19,
            "parent_dir",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            20,
            "group_id",
            Type::Primitive(PrimitiveType::Uuid),
        )),
        // ── ADR-009 observation columns ──
        Arc::new(NestedField::optional(
            21,
            "source_id",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            22,
            "relative_path",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            23,
            "run_id",
            Type::Primitive(PrimitiveType::Uuid),
        )),
        Arc::new(NestedField::optional(
            24,
            "observation_status",
            Type::Primitive(PrimitiveType::String),
        )),
        Arc::new(NestedField::optional(
            25,
            "observed_at",
            Type::Primitive(PrimitiveType::Timestamptz),
        )),
    ];

    Schema::builder()
        .with_fields(fields)
        .with_identifier_field_ids([1])
        .build()
        .context("Failed to build file_catalog schema")
}

/// Check that a live table schema can accept rows built from
/// [`build_file_catalog_schema`]: every expected field must exist with the
/// same id and type. Extra table fields are tolerated (they read as absent).
///
/// The writer builds Arrow batches from the code schema, so a table that lags
/// behind it (created before the ADR-009 columns) must be upgraded by `init`
/// before ingest can commit.
pub fn verify_table_schema(table_schema: &Schema) -> Result<()> {
    let expected = build_file_catalog_schema()?;
    let mut missing = Vec::new();
    let mut mismatched = Vec::new();

    for field in expected.as_struct().fields() {
        match table_schema.field_by_name(&field.name) {
            None => missing.push(field.name.clone()),
            Some(actual) if actual.id != field.id || actual.field_type != field.field_type => {
                mismatched.push(format!(
                    "{} (table: id {} {}, expected: id {} {})",
                    field.name, actual.id, actual.field_type, field.id, field.field_type
                ));
            }
            Some(_) => {}
        }
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "table schema is missing column(s) {}; run `anti_entropator init` to upgrade it",
            missing.join(", ")
        );
    }
    if !mismatched.is_empty() {
        anyhow::bail!(
            "table schema does not match the catalog definition: {}",
            mismatched.join("; ")
        );
    }
    Ok(())
}

/// Fields from [`build_file_catalog_schema`] that `table_schema` lacks, in
/// schema order. Used by `init` to evolve legacy tables additively.
pub fn missing_fields(table_schema: &Schema) -> Result<Vec<Arc<NestedField>>> {
    let expected = build_file_catalog_schema()?;
    Ok(expected
        .as_struct()
        .fields()
        .iter()
        .filter(|f| table_schema.field_by_name(&f.name).is_none())
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(NAMESPACE, "anti_entropator");
        assert_eq!(FILE_CATALOG_TABLE, "file_catalog");
    }

    #[test]
    fn test_build_schema_succeeds() {
        let result = build_file_catalog_schema();
        assert!(result.is_ok());
    }

    #[test]
    fn test_schema_has_correct_field_count() {
        let schema = build_file_catalog_schema().unwrap();
        assert_eq!(schema.as_struct().fields().len(), FIELD_COUNT);
    }

    #[test]
    fn observation_fields_are_optional_and_appended() {
        // ADR-009 columns must be optional (legacy rows have no value) and
        // must come after the legacy columns with stable ids 21-25.
        let schema = build_file_catalog_schema().unwrap();
        let fields = schema.as_struct().fields();
        let expected = [
            (21, "source_id"),
            (22, "relative_path"),
            (23, "run_id"),
            (24, "observation_status"),
            (25, "observed_at"),
        ];
        for (i, (id, name)) in expected.iter().enumerate() {
            let field = &fields[LEGACY_FIELD_COUNT + i];
            assert_eq!(field.id, *id, "field id for {name}");
            assert_eq!(field.name, *name);
            assert!(!field.required, "{name} must be optional");
        }
    }

    #[test]
    fn field_ids_are_unique_and_sequential() {
        let schema = build_file_catalog_schema().unwrap();
        let ids: Vec<i32> = schema.as_struct().fields().iter().map(|f| f.id).collect();
        let expected: Vec<i32> = (1..=FIELD_COUNT as i32).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn test_schema_field_names() {
        let schema = build_file_catalog_schema().unwrap();
        let field_names: Vec<&str> = schema
            .as_struct()
            .fields()
            .iter()
            .map(|f| f.name.as_str())
            .collect();

        assert!(field_names.contains(&"id"));
        assert!(field_names.contains(&"source_path"));
        assert!(field_names.contains(&"filename"));
        assert!(field_names.contains(&"content_hash"));
        assert!(field_names.contains(&"is_duplicate"));
        assert!(field_names.contains(&"parent_dir"));
    }

    #[test]
    fn test_schema_identifier_field() {
        let schema = build_file_catalog_schema().unwrap();
        let identifier_ids: Vec<i32> = schema.identifier_field_ids().collect();
        assert_eq!(identifier_ids.len(), 1);
        assert!(identifier_ids.contains(&1)); // id field
    }

    /// The 20-field schema as `init` created it before ADR-009.
    fn legacy_schema() -> Schema {
        let full = build_file_catalog_schema().unwrap();
        let fields: Vec<_> = full
            .as_struct()
            .fields()
            .iter()
            .take(LEGACY_FIELD_COUNT)
            .cloned()
            .collect();
        Schema::builder()
            .with_fields(fields)
            .with_identifier_field_ids([1])
            .build()
            .unwrap()
    }

    #[test]
    fn verify_accepts_current_schema() {
        let schema = build_file_catalog_schema().unwrap();
        assert!(verify_table_schema(&schema).is_ok());
    }

    #[test]
    fn verify_rejects_legacy_schema_with_actionable_message() {
        let err = verify_table_schema(&legacy_schema())
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing column(s)"), "{err}");
        assert!(err.contains("source_id"), "{err}");
        assert!(err.contains("observed_at"), "{err}");
        assert!(err.contains("anti_entropator init"), "{err}");
    }

    #[test]
    fn verify_rejects_id_or_type_drift() {
        // Same names, but `run_id` was assigned a different id and type.
        let full = build_file_catalog_schema().unwrap();
        let fields: Vec<_> = full
            .as_struct()
            .fields()
            .iter()
            .map(|f| {
                if f.name == "run_id" {
                    Arc::new(NestedField::optional(
                        99,
                        "run_id",
                        Type::Primitive(PrimitiveType::String),
                    ))
                } else {
                    f.clone()
                }
            })
            .collect();
        let drifted = Schema::builder()
            .with_fields(fields)
            .with_identifier_field_ids([1])
            .build()
            .unwrap();

        let err = verify_table_schema(&drifted).unwrap_err().to_string();
        assert!(err.contains("does not match"), "{err}");
        assert!(err.contains("run_id"), "{err}");
    }

    #[test]
    fn missing_fields_lists_observation_columns_for_legacy_schema() {
        let missing = missing_fields(&legacy_schema()).unwrap();
        let names: Vec<&str> = missing.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "source_id",
                "relative_path",
                "run_id",
                "observation_status",
                "observed_at"
            ]
        );
        assert!(missing.iter().all(|f| !f.required));
    }

    #[test]
    fn missing_fields_is_empty_for_current_schema() {
        let schema = build_file_catalog_schema().unwrap();
        assert!(missing_fields(&schema).unwrap().is_empty());
    }

    #[test]
    fn test_schema_required_fields() {
        let schema = build_file_catalog_schema().unwrap();

        // These fields should be required
        let required_field_ids = [1, 2, 3, 4, 6, 7, 12, 17, 19]; // id, source_path, filename, extension, category, size_bytes, scanned_at, is_duplicate, parent_dir

        for field in schema.as_struct().fields() {
            if required_field_ids.contains(&field.id) {
                assert!(
                    field.required,
                    "Field {} (id={}) should be required",
                    field.name, field.id
                );
            }
        }
    }
}
