//! Iceberg writer - Write FileInfo to Iceberg tables
//!
//! Handles conversion of FileInfo to Arrow RecordBatches and committing to Iceberg.

use crate::domain::FileInfo;
use crate::lakehouse::schema::{build_file_catalog_schema, FILE_CATALOG_TABLE, NAMESPACE};
use crate::lakehouse::{get_warehouse_prefix, LakehouseConfig};
use anyhow::{Context, Result};
use arrow::array::{
    ArrayRef, BooleanArray, FixedSizeBinaryBuilder, Int64Array, StringArray,
    TimestampMicrosecondArray,
};
use arrow::record_batch::RecordBatch;
use iceberg::io::{FileIO, FileIOBuilder};
use iceberg::spec::{DataContentType, DataFile, Struct};
use iceberg::table::Table;
use iceberg::transaction::{ApplyTransactionAction, Transaction};
use iceberg::writer::file_writer::location_generator::{
    DefaultLocationGenerator, LocationGenerator,
};
use iceberg::writer::file_writer::{FileWriter, FileWriterBuilder, ParquetWriterBuilder};
use iceberg::{Catalog, CatalogBuilder, TableIdent};
use iceberg_catalog_rest::{RestCatalog, RestCatalogBuilder};
use parquet::file::properties::WriterProperties;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Main entry point: Commit a list of FileInfo objects to the Iceberg table
pub async fn commit_files(files: Vec<FileInfo>, config: &LakehouseConfig) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    // 1. Convert to Arrow Batch
    let batch = files_to_batch(&files)?;

    // 2. Initialize Catalog & Load Table
    let catalog = init_catalog(config).await?;
    let table = load_table(&catalog).await?;

    // 3. Write Data File (Parquet) to S3
    let data_files = write_parquet_file(&table, &batch, config).await?;

    // 4. Commit Transaction
    commit_transaction(&catalog, &table, data_files).await?;

    println!(
        "  Committed {} records to Iceberg table '{}.{}'...",
        files.len(),
        NAMESPACE,
        FILE_CATALOG_TABLE
    );

    Ok(())
}

// ==================== Helper Functions ====================

/// Initialize the RestCatalog with S3 credentials
///
/// Note: Lakekeeper stores internal Docker endpoints in table configs,
/// so we must override S3 settings to use the host-accessible endpoint.
async fn init_catalog(config: &LakehouseConfig) -> Result<RestCatalog> {
    // Fetch the correct prefix and URI for this warehouse (Lakekeeper specific)
    let catalog_config = get_warehouse_prefix(config).await?;

    let mut props = HashMap::new();
    // Use the URI from Lakekeeper's config response (includes /catalog path)
    props.insert("uri".to_string(), catalog_config.uri.clone());
    props.insert("prefix".to_string(), catalog_config.prefix.clone());
    props.insert("warehouse".to_string(), config.warehouse.clone());

    // Override S3 configuration to use host-accessible endpoint
    // (Lakekeeper stores internal Docker endpoint which is unreachable from host)
    props.insert("s3.endpoint".to_string(), config.s3_endpoint.clone());
    props.insert("s3.access-key-id".to_string(), config.s3_access_key.clone());
    props.insert(
        "s3.secret-access-key".to_string(),
        config.s3_secret_key.clone(),
    );
    props.insert("s3.region".to_string(), "us-east-1".to_string());
    props.insert("s3.path-style-access".to_string(), "true".to_string());
    props.insert("s3.allow-http".to_string(), "true".to_string());
    // Disable remote signing - use direct S3 credentials instead of Lakekeeper signer
    props.insert("s3.remote-signing-enabled".to_string(), "false".to_string());

    println!(
        "  Connecting to catalog at {} with warehouse {}",
        config.catalog_endpoint, config.warehouse
    );

    RestCatalogBuilder::default()
        .load("anti_entropator", props)
        .await
        .context("Failed to build RestCatalog")
}

/// Load the target table from the catalog
async fn load_table(catalog: &RestCatalog) -> Result<Table> {
    let table_id = TableIdent::from_strs(vec![NAMESPACE, FILE_CATALOG_TABLE])?;
    catalog
        .load_table(&table_id)
        .await
        .context("Failed to load table")
}

/// Configure and create the FileIO for S3
fn create_file_io(config: &LakehouseConfig) -> Result<FileIO> {
    FileIOBuilder::new("s3")
        .with_props(vec![
            ("s3.endpoint", config.s3_endpoint.clone()),
            ("s3.region", "us-east-1".to_string()),
            ("s3.access-key-id", config.s3_access_key.clone()),
            ("s3.secret-access-key", config.s3_secret_key.clone()),
            ("s3.allow-http", "true".to_string()),
            ("s3.path-style-access", "true".to_string()),
        ])
        .build()
        .context("Failed to build FileIO")
}

/// Write the RecordBatch to a Parquet file in S3 and return DataFile metadata
async fn write_parquet_file(
    table: &Table,
    batch: &RecordBatch,
    config: &LakehouseConfig,
) -> Result<Vec<DataFile>> {
    let file_io = create_file_io(config)?;
    let location_generator = DefaultLocationGenerator::new(table.metadata().clone())?;
    let file_id = Uuid::new_v4();
    let file_name = format!("{file_id}.parquet");
    let file_path = location_generator.generate_location(None, &file_name);

    let output_file = file_io.new_output(file_path)?;

    let mut writer = ParquetWriterBuilder::new(
        WriterProperties::builder().build(),
        table.metadata().current_schema().clone(),
    )
    .build(output_file)
    .await?;

    writer.write(batch).await?;
    let data_file_builders = writer.close().await?;
    println!(
        "  Parquet file written. Preparing transaction with {} data files...",
        data_file_builders.len()
    );

    let mut data_files = Vec::new();
    for mut dfb in data_file_builders {
        let df = dfb
            .content(DataContentType::Data)
            .partition(Struct::empty())
            .build()
            .context("Failed to build DataFile")?;
        data_files.push(df);
    }

    Ok(data_files)
}

/// Commit the transaction adding the new data files
async fn commit_transaction(
    catalog: &dyn Catalog,
    table: &Table,
    data_files: Vec<DataFile>,
) -> Result<()> {
    let transaction = Transaction::new(table);

    let append = transaction.fast_append().add_data_files(data_files);

    let transaction = append
        .apply(transaction)
        .context("Failed to apply append action")?;
    let _table = transaction.commit(catalog).await?;

    Ok(())
}

// ==================== Batch Building ====================

/// Builder for accumulating FileInfo data into Arrow column arrays
struct BatchColumnsBuilder {
    ids: FixedSizeBinaryBuilder,
    source_paths: Vec<String>,
    filenames: Vec<String>,
    extensions: Vec<String>,
    mime_types: Vec<Option<String>>,
    categories: Vec<String>,
    sizes: Vec<i64>,
    content_hashes: Vec<Option<String>>,
    partial_hashes: Vec<Option<String>>,
    created_ats: Vec<Option<i64>>,
    modified_ats: Vec<Option<i64>>,
    scanned_ats: Vec<i64>,
    object_uris: Vec<Option<String>>,
    ingested_ats: Vec<Option<i64>>,
    suggested_names: Vec<Option<String>>,
    name_reasons: Vec<Option<String>>,
    is_duplicates: Vec<bool>,
    duplicate_ofs: FixedSizeBinaryBuilder,
    parent_dirs: Vec<String>,
    group_ids: FixedSizeBinaryBuilder,
}

impl BatchColumnsBuilder {
    /// Create a new builder with pre-allocated capacity
    fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
            source_paths: Vec::with_capacity(capacity),
            filenames: Vec::with_capacity(capacity),
            extensions: Vec::with_capacity(capacity),
            mime_types: Vec::with_capacity(capacity),
            categories: Vec::with_capacity(capacity),
            sizes: Vec::with_capacity(capacity),
            content_hashes: Vec::with_capacity(capacity),
            partial_hashes: Vec::with_capacity(capacity),
            created_ats: Vec::with_capacity(capacity),
            modified_ats: Vec::with_capacity(capacity),
            scanned_ats: Vec::with_capacity(capacity),
            object_uris: Vec::with_capacity(capacity),
            ingested_ats: Vec::with_capacity(capacity),
            suggested_names: Vec::with_capacity(capacity),
            name_reasons: Vec::with_capacity(capacity),
            is_duplicates: Vec::with_capacity(capacity),
            duplicate_ofs: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
            parent_dirs: Vec::with_capacity(capacity),
            group_ids: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
        }
    }

    /// Add a FileInfo to the builder
    fn push(&mut self, f: &FileInfo) -> Result<()> {
        self.ids.append_value(f.id.as_bytes())?;
        self.source_paths
            .push(f.source_path.to_string_lossy().to_string());
        self.filenames.push(f.filename.clone());
        self.extensions.push(f.extension.clone());
        self.mime_types.push(f.mime_type.clone());
        self.categories.push(f.category.to_string());
        self.sizes.push(f.size_bytes as i64);
        self.content_hashes
            .push(f.content_hash.as_ref().map(|h| h.0.clone()));
        self.partial_hashes
            .push(f.partial_hash.as_ref().map(|h| h.0.clone()));
        self.created_ats
            .push(f.created_at.map(|dt| dt.timestamp_micros()));
        self.modified_ats
            .push(f.modified_at.map(|dt| dt.timestamp_micros()));
        self.scanned_ats.push(f.scanned_at.timestamp_micros());
        self.object_uris.push(f.object_uri.clone());
        self.ingested_ats
            .push(f.ingested_at.map(|dt| dt.timestamp_micros()));
        self.suggested_names.push(f.suggested_name.clone());
        self.name_reasons.push(f.name_reason.clone());
        self.is_duplicates.push(f.is_duplicate);

        match f.duplicate_of {
            Some(dup_id) => self.duplicate_ofs.append_value(dup_id.as_bytes())?,
            None => self.duplicate_ofs.append_null(),
        }

        self.parent_dirs.push(f.parent_dir.clone());

        match f.group_id {
            Some(gid) => self.group_ids.append_value(gid.as_bytes())?,
            None => self.group_ids.append_null(),
        }

        Ok(())
    }

    /// Build the final Arrow arrays from accumulated data
    fn build_arrays(mut self) -> Vec<ArrayRef> {
        vec![
            Arc::new(self.ids.finish()) as ArrayRef,
            Arc::new(StringArray::from(self.source_paths)) as ArrayRef,
            Arc::new(StringArray::from(self.filenames)) as ArrayRef,
            Arc::new(StringArray::from(self.extensions)) as ArrayRef,
            Arc::new(StringArray::from(self.mime_types)) as ArrayRef,
            Arc::new(StringArray::from(self.categories)) as ArrayRef,
            Arc::new(Int64Array::from(self.sizes)) as ArrayRef,
            Arc::new(StringArray::from(self.content_hashes)) as ArrayRef,
            Arc::new(StringArray::from(self.partial_hashes)) as ArrayRef,
            Arc::new(TimestampMicrosecondArray::from(self.created_ats).with_timezone("+00:00"))
                as ArrayRef,
            Arc::new(TimestampMicrosecondArray::from(self.modified_ats).with_timezone("+00:00"))
                as ArrayRef,
            Arc::new(TimestampMicrosecondArray::from(self.scanned_ats).with_timezone("+00:00"))
                as ArrayRef,
            Arc::new(StringArray::from(self.object_uris)) as ArrayRef,
            Arc::new(TimestampMicrosecondArray::from(self.ingested_ats).with_timezone("+00:00"))
                as ArrayRef,
            Arc::new(StringArray::from(self.suggested_names)) as ArrayRef,
            Arc::new(StringArray::from(self.name_reasons)) as ArrayRef,
            Arc::new(BooleanArray::from(self.is_duplicates)) as ArrayRef,
            Arc::new(self.duplicate_ofs.finish()) as ArrayRef,
            Arc::new(StringArray::from(self.parent_dirs)) as ArrayRef,
            Arc::new(self.group_ids.finish()) as ArrayRef,
        ]
    }
}

/// Convert a list of FileInfo objects to an Arrow RecordBatch
pub fn files_to_batch(files: &[FileInfo]) -> Result<RecordBatch> {
    let schema = build_file_catalog_schema()?;
    let arrow_schema = Arc::new(iceberg::arrow::schema_to_arrow_schema(&schema)?);

    let mut builder = BatchColumnsBuilder::with_capacity(files.len());
    for f in files {
        builder.push(f)?;
    }

    let batch = RecordBatch::try_new(arrow_schema, builder.build_arrays())?;
    Ok(batch)
}

// ==================== Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::FileCategory;
    use chrono::Utc;
    use std::path::PathBuf;

    /// Create a minimal FileInfo for testing
    fn make_test_file_info(name: &str) -> FileInfo {
        FileInfo {
            id: Uuid::new_v4(),
            source_path: PathBuf::from(format!("/test/{}", name)),
            filename: name.to_string(),
            extension: "txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            category: FileCategory::Document,
            size_bytes: 1024,
            content_hash: None,
            partial_hash: None,
            created_at: Some(Utc::now()),
            modified_at: Some(Utc::now()),
            scanned_at: Utc::now(),
            object_uri: None,
            ingested_at: None,
            suggested_name: None,
            name_reason: None,
            is_duplicate: false,
            duplicate_of: None,
            parent_dir: "test".to_string(),
            group_id: None,
        }
    }

    #[test]
    fn test_files_to_batch_empty() {
        let files: Vec<FileInfo> = vec![];
        let result = files_to_batch(&files);
        assert!(result.is_ok());
        let batch = result.unwrap();
        assert_eq!(batch.num_rows(), 0);
    }

    #[test]
    fn test_files_to_batch_single_file() {
        let files = vec![make_test_file_info("test.txt")];
        let result = files_to_batch(&files);
        assert!(result.is_ok());
        let batch = result.unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 20); // 20 columns in schema
    }

    #[test]
    fn test_files_to_batch_multiple_files() {
        let files = vec![
            make_test_file_info("file1.txt"),
            make_test_file_info("file2.txt"),
            make_test_file_info("file3.txt"),
        ];
        let result = files_to_batch(&files);
        assert!(result.is_ok());
        let batch = result.unwrap();
        assert_eq!(batch.num_rows(), 3);
    }

    #[test]
    fn test_files_to_batch_with_optional_fields() {
        let mut file = make_test_file_info("test.txt");
        file.content_hash = Some(crate::domain::ContentHash::new("abc123def456".to_string()));
        file.suggested_name = Some("better_name.txt".to_string());
        file.name_reason = Some("exif_datetime".to_string());
        file.duplicate_of = Some(Uuid::new_v4());
        file.group_id = Some(Uuid::new_v4());

        let files = vec![file];
        let result = files_to_batch(&files);
        assert!(result.is_ok());
        let batch = result.unwrap();
        assert_eq!(batch.num_rows(), 1);
    }

    #[test]
    fn test_batch_columns_builder_capacity() {
        let builder = BatchColumnsBuilder::with_capacity(100);
        // Just verify it doesn't panic and has the right initial state
        assert_eq!(builder.source_paths.capacity(), 100);
        assert_eq!(builder.filenames.capacity(), 100);
    }
}
