//! File information types

use super::observation::{observation_id, ObservationStatus};
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

    // ── ADR-009 observation fields (catalog columns 21-25, all optional) ──
    /// Stable identifier of the ingest root this observation belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,

    /// Path relative to the ingest root, `/`-separated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,

    /// Ingest run that produced this observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<Uuid>,

    /// Whether the path was observed present or deleted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_status: Option<ObservationStatus>,

    /// When the observation was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
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
            source_id: None,
            relative_path: None,
            run_id: None,
            observation_status: None,
            observed_at: None,
        }
    }

    /// Attach the ADR-009 observation identity and derive the deterministic
    /// row `id` from `(source_id, relative_path, content_hash, status)`.
    ///
    /// Call after the content hash is final; the `id` incorporates it.
    pub fn with_observation(
        mut self,
        source_id: String,
        relative_path: String,
        run_id: Uuid,
        status: ObservationStatus,
        observed_at: DateTime<Utc>,
    ) -> Self {
        self.id = observation_id(
            &source_id,
            &relative_path,
            self.content_hash.as_ref().map(|h| h.0.as_str()),
            status,
        );
        self.source_id = Some(source_id);
        self.relative_path = Some(relative_path);
        self.run_id = Some(run_id);
        self.observation_status = Some(status);
        self.observed_at = Some(observed_at);
        self
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
    #[allow(dead_code)] // Kept for planned duplicate-group workflows (post-v0.3).
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
    #[allow(dead_code)] // Kept for planned duplicate-group workflows (post-v0.3).
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
#[allow(dead_code)] // Kept as lightweight profile DTO scaffold (post-v0.3).
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
    #[allow(dead_code)] // Used by planned profile output extensions.
    pub fn category(&self) -> FileCategory {
        if let Some(ref mime) = self.mime_type {
            FileCategory::from_mime(mime)
        } else {
            FileCategory::from_extension(&self.extension)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info() -> FileInfo {
        FileInfo::new(
            PathBuf::from("/test/photo.jpg"),
            "photo.jpg".to_string(),
            ".jpg".to_string(),
            2048,
            None,
            None,
        )
    }

    // ── FileInfo::new ──

    #[test]
    fn new_sets_category_from_extension() {
        let info = make_info();
        assert_eq!(info.category, FileCategory::Image);
    }

    #[test]
    fn new_sets_basic_fields() {
        let info = make_info();
        assert_eq!(info.filename, "photo.jpg");
        assert_eq!(info.extension, ".jpg");
        assert_eq!(info.size_bytes, 2048);
        assert_eq!(info.source_path, PathBuf::from("/test/photo.jpg"));
    }

    #[test]
    fn new_initializes_optional_fields_to_none() {
        let info = make_info();
        assert!(info.mime_type.is_none());
        assert!(info.content_hash.is_none());
        assert!(info.partial_hash.is_none());
        assert!(info.object_uri.is_none());
        assert!(info.ingested_at.is_none());
        assert!(info.suggested_name.is_none());
        assert!(info.name_reason.is_none());
        assert!(info.duplicate_of.is_none());
        assert!(info.group_id.is_none());
    }

    #[test]
    fn new_defaults_not_duplicate() {
        let info = make_info();
        assert!(!info.is_duplicate);
    }

    #[test]
    fn new_generates_unique_ids() {
        let a = make_info();
        let b = make_info();
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn new_unknown_extension() {
        let info = FileInfo::new(
            PathBuf::from("/test/data.xyz"),
            "data.xyz".to_string(),
            ".xyz".to_string(),
            100,
            None,
            None,
        );
        assert_eq!(info.category, FileCategory::Other);
    }

    #[test]
    fn new_no_extension() {
        let info = FileInfo::new(
            PathBuf::from("/test/Makefile"),
            "Makefile".to_string(),
            "(none)".to_string(),
            100,
            None,
            None,
        );
        assert_eq!(info.category, FileCategory::Other);
    }

    // ── with_* builders ──

    #[test]
    fn with_mime_type_overrides_category() {
        let info = make_info().with_mime_type("audio/mpeg".to_string());
        assert_eq!(info.category, FileCategory::Audio);
        assert_eq!(info.mime_type, Some("audio/mpeg".to_string()));
    }

    #[test]
    fn with_parent_dir() {
        let info = make_info().with_parent_dir("photos/2024".to_string());
        assert_eq!(info.parent_dir, "photos/2024");
    }

    #[test]
    fn with_group_id() {
        let gid = Uuid::new_v4();
        let info = make_info().with_group_id(gid);
        assert_eq!(info.group_id, Some(gid));
    }

    #[test]
    fn with_content_hash() {
        let hash = ContentHash::new("abc123".to_string());
        let info = make_info().with_content_hash(hash.clone());
        assert_eq!(info.content_hash.unwrap().as_str(), "abc123");
    }

    #[test]
    fn with_partial_hash() {
        let hash = PartialHash::new("def456".to_string());
        let info = make_info().with_partial_hash(hash.clone());
        assert_eq!(info.partial_hash.unwrap().as_str(), "def456");
    }

    #[test]
    fn mark_as_duplicate() {
        let original_id = Uuid::new_v4();
        let info = make_info().mark_as_duplicate(original_id);
        assert!(info.is_duplicate);
        assert_eq!(info.duplicate_of, Some(original_id));
    }

    #[test]
    fn with_suggested_name() {
        let info = make_info().with_suggested_name(
            "2024-01-15_vacation.jpg".to_string(),
            "exif_datetime".to_string(),
        );
        assert_eq!(info.suggested_name.unwrap(), "2024-01-15_vacation.jpg");
        assert_eq!(info.name_reason.unwrap(), "exif_datetime");
    }

    #[test]
    fn builder_chaining() {
        let gid = Uuid::new_v4();
        let dup_id = Uuid::new_v4();

        let info = make_info()
            .with_mime_type("image/png".to_string())
            .with_parent_dir("album".to_string())
            .with_content_hash(ContentHash::new("hash1".to_string()))
            .with_partial_hash(PartialHash::new("hash2".to_string()))
            .with_group_id(gid)
            .mark_as_duplicate(dup_id)
            .with_suggested_name("better.png".to_string(), "metadata".to_string());

        assert_eq!(info.category, FileCategory::Image);
        assert_eq!(info.parent_dir, "album");
        assert!(info.content_hash.is_some());
        assert!(info.partial_hash.is_some());
        assert_eq!(info.group_id, Some(gid));
        assert!(info.is_duplicate);
        assert_eq!(info.duplicate_of, Some(dup_id));
        assert_eq!(info.suggested_name.unwrap(), "better.png");
    }

    // ── with_observation ──

    #[test]
    fn with_observation_sets_fields_and_deterministic_id() {
        let run = Uuid::new_v4();
        let at = Utc::now();
        let a = make_info()
            .with_content_hash(ContentHash::new("h1".to_string()))
            .with_observation(
                "/src".to_string(),
                "photo.jpg".to_string(),
                run,
                ObservationStatus::Present,
                at,
            );
        let b = make_info()
            .with_content_hash(ContentHash::new("h1".to_string()))
            .with_observation(
                "/src".to_string(),
                "photo.jpg".to_string(),
                Uuid::new_v4(), // a different run must not change the id
                ObservationStatus::Present,
                Utc::now(),
            );

        assert_eq!(a.id, b.id);
        assert_eq!(a.source_id.as_deref(), Some("/src"));
        assert_eq!(a.relative_path.as_deref(), Some("photo.jpg"));
        assert_eq!(a.run_id, Some(run));
        assert_eq!(a.observation_status, Some(ObservationStatus::Present));
        assert_eq!(a.observed_at, Some(at));
    }

    #[test]
    fn with_observation_id_depends_on_content_hash() {
        let obs = |hash: &str| {
            make_info()
                .with_content_hash(ContentHash::new(hash.to_string()))
                .with_observation(
                    "/src".to_string(),
                    "photo.jpg".to_string(),
                    Uuid::nil(),
                    ObservationStatus::Present,
                    Utc::now(),
                )
                .id
        };
        assert_ne!(obs("h1"), obs("h2"));
    }

    #[test]
    fn observation_fields_are_omitted_from_json_when_unset() {
        let json = serde_json::to_value(make_info()).unwrap();
        assert!(json.get("source_id").is_none());
        assert!(json.get("run_id").is_none());
        assert!(json.get("observation_status").is_none());
    }

    // ── FileEntry ──

    #[test]
    fn file_entry_category_from_mime() {
        let entry = FileEntry {
            path: PathBuf::from("/test/file.dat"),
            filename: "file.dat".to_string(),
            extension: ".dat".to_string(),
            size_bytes: 100,
            modified_at: None,
            mime_type: Some("video/mp4".to_string()),
        };
        assert_eq!(entry.category(), FileCategory::Video);
    }

    #[test]
    fn file_entry_category_falls_back_to_extension() {
        let entry = FileEntry {
            path: PathBuf::from("/test/song.mp3"),
            filename: "song.mp3".to_string(),
            extension: ".mp3".to_string(),
            size_bytes: 100,
            modified_at: None,
            mime_type: None,
        };
        assert_eq!(entry.category(), FileCategory::Audio);
    }

    #[test]
    fn file_entry_category_mime_takes_precedence() {
        let entry = FileEntry {
            path: PathBuf::from("/test/file.txt"),
            filename: "file.txt".to_string(),
            extension: ".txt".to_string(),
            size_bytes: 100,
            modified_at: None,
            mime_type: Some("image/jpeg".to_string()),
        };
        // MIME says image, extension says document — MIME wins
        assert_eq!(entry.category(), FileCategory::Image);
    }
}
