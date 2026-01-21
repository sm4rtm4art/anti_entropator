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

/// Build the file_catalog schema matching FileInfo structure
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
    ];

    Schema::builder()
        .with_fields(fields)
        .with_identifier_field_ids([1])
        .build()
        .context("Failed to build file_catalog schema")
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
        // Schema has as_struct() to access the underlying struct type
        assert_eq!(schema.as_struct().fields().len(), 20);
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
