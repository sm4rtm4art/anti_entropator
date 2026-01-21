//! File information types

use super::{ContentHash, FileCategory, PartialHash};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Complete information about a scanned file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    /// Unique identifier
    pub id: Uuid,

    /// Original source path
    pub source_path: PathBuf,

    /// Filename only
    pub filename: String,

    /// File extension (lowercase, with dot)
    pub extension: String,

    /// MIME type if detected
    pub mime_type: Option<String>,

    /// File category
    pub category: FileCategory,

    /// Size in bytes
    pub size_bytes: u64,

    /// Full content hash (SHA-256)
    pub content_hash: Option<ContentHash>,

    /// Partial hash for quick duplicate detection
    pub partial_hash: Option<PartialHash>,

    /// File creation time
    pub created_at: Option<DateTime<Utc>>,

    /// File modification time
    pub modified_at: Option<DateTime<Utc>>,

    /// When this file was scanned
    pub scanned_at: DateTime<Utc>,

    /// S3 object URI (once ingested)
    pub object_uri: Option<String>,

    /// When ingested to object storage
    pub ingested_at: Option<DateTime<Utc>>,

    /// Suggested new name based on metadata
    pub suggested_name: Option<String>,

    /// Reason for the suggested name
    pub name_reason: Option<String>,

    /// Whether this file is a duplicate
    pub is_duplicate: bool,

    /// Reference to the "original" file if this is a duplicate
    pub duplicate_of: Option<Uuid>,

    /// Parent directory relative to ingest root
    pub parent_dir: String,

    /// Group ID for related files (fuzzy duplicates, different formats)
    pub group_id: Option<Uuid>,
}

impl FileInfo {
    /// Create new FileInfo from basic scan data
    pub fn new(
        source_path: PathBuf,
        filename: String,
        extension: String,
        size_bytes: u64,
        modified_at: Option<DateTime<Utc>>,
        created_at: Option<DateTime<Utc>>,
    ) -> Self {
        let category = FileCategory::from_extension(&extension);

        Self {
            id: Uuid::new_v4(),
            source_path,
            filename,
            extension,
            mime_type: None,
            category,
            size_bytes,
            content_hash: None,
            partial_hash: None,
            created_at,
            modified_at,
            scanned_at: Utc::now(),
            object_uri: None,
            ingested_at: None,
            suggested_name: None,
            name_reason: None,
            is_duplicate: false,
            duplicate_of: None,
            parent_dir: String::new(),
            group_id: None,
        }
    }

    /// Update category based on MIME type
    pub fn with_mime_type(mut self, mime: String) -> Self {
        self.category = FileCategory::from_mime(&mime);
        self.mime_type = Some(mime);
        self
    }

    /// Set parent directory
    pub fn with_parent_dir(mut self, dir: String) -> Self {
        self.parent_dir = dir;
        self
    }

    /// Set group id
    pub fn with_group_id(mut self, id: Uuid) -> Self {
        self.group_id = Some(id);
        self
    }

    /// Set content hash
    pub fn with_content_hash(mut self, hash: ContentHash) -> Self {
        self.content_hash = Some(hash);
        self
    }

    /// Set partial hash
    pub fn with_partial_hash(mut self, hash: PartialHash) -> Self {
        self.partial_hash = Some(hash);
        self
    }

    /// Mark as duplicate of another file
    pub fn mark_as_duplicate(mut self, original_id: Uuid) -> Self {
        self.is_duplicate = true;
        self.duplicate_of = Some(original_id);
        self
    }

    /// Set suggested name
    pub fn with_suggested_name(mut self, name: String, reason: String) -> Self {
        self.suggested_name = Some(name);
        self.name_reason = Some(reason);
        self
    }
}

/// Lightweight file entry for profiling (minimal memory)
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub filename: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at: Option<DateTime<Utc>>,
    pub mime_type: Option<String>,
}

impl FileEntry {
    pub fn category(&self) -> FileCategory {
        if let Some(ref mime) = self.mime_type {
            FileCategory::from_mime(mime)
        } else {
            FileCategory::from_extension(&self.extension)
        }
    }
}
