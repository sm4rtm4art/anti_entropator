//! Domain module - Core types following "Anti-Primitive Obsession" principle
//!
//! These types ensure compile-time guarantees about data validity.

#![allow(dead_code)] // Domain types scaffolding - will be fully used in later phases

use std::path::PathBuf;
use thiserror::Error;

pub mod file_info;
pub mod stats;

pub use file_info::FileInfo;

/// Errors that can occur in domain operations
#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A validated raw path that exists on the filesystem
#[derive(Debug, Clone)]
pub struct RawPath(PathBuf);

impl RawPath {
    /// Create a new RawPath, validating that it exists
    pub fn new(path: PathBuf) -> Result<Self, DomainError> {
        if !path.exists() {
            return Err(DomainError::PathNotFound(path));
        }
        Ok(Self(path))
    }

    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }

    pub fn into_inner(self) -> PathBuf {
        self.0
    }
}

impl AsRef<std::path::Path> for RawPath {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

/// A content hash (SHA-256)
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn new(hash: String) -> Self {
        Self(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the S3 object key for content-addressed storage
    /// Format: sha256/ab/cd/<full_hash>
    pub fn to_object_key(&self) -> String {
        let hash = &self.0;
        if hash.len() >= 4 {
            format!("sha256/{}/{}/{}", &hash[0..2], &hash[2..4], hash)
        } else {
            format!("sha256/{}", hash)
        }
    }
}

impl std::fmt::Display for ContentHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A partial hash (first N bytes) for quick duplicate candidate detection
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PartialHash(String);

impl PartialHash {
    pub fn new(hash: String) -> Self {
        Self(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// File category for organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileCategory {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Code,
    Font,
    Data,
    Other,
}

impl FileCategory {
    /// Determine category from MIME type
    pub fn from_mime(mime: &str) -> Self {
        let mime_lower = mime.to_lowercase();

        if mime_lower.starts_with("image/") {
            FileCategory::Image
        } else if mime_lower.starts_with("video/") {
            FileCategory::Video
        } else if mime_lower.starts_with("audio/") {
            FileCategory::Audio
        } else if mime_lower.starts_with("text/")
            || mime_lower.contains("pdf")
            || mime_lower.contains("document")
            || mime_lower.contains("msword")
            || mime_lower.contains("spreadsheet")
            || mime_lower.contains("presentation")
            || mime_lower.contains("epub")
        {
            FileCategory::Document
        } else if mime_lower.contains("zip")
            || mime_lower.contains("tar")
            || mime_lower.contains("gzip")
            || mime_lower.contains("bzip")
            || mime_lower.contains("xz")
            || mime_lower.contains("rar")
            || mime_lower.contains("7z")
            || mime_lower.contains("compressed")
            || mime_lower.contains("archive")
        {
            FileCategory::Archive
        } else if mime_lower.contains("javascript")
            || mime_lower.contains("json")
            || mime_lower.contains("xml")
            || mime_lower.contains("html")
            || mime_lower.contains("css")
            || mime_lower.contains("python")
            || mime_lower.contains("rust")
            || mime_lower.contains("java")
        {
            FileCategory::Code
        } else if mime_lower.contains("font")
            || mime_lower.contains("woff")
            || mime_lower.contains("ttf")
            || mime_lower.contains("otf")
        {
            FileCategory::Font
        } else if mime_lower.contains("parquet")
            || mime_lower.contains("csv")
            || mime_lower.contains("sqlite")
            || mime_lower.contains("sql")
        {
            FileCategory::Data
        } else {
            FileCategory::Other
        }
    }

    /// Determine category from file extension (fallback)
    pub fn from_extension(ext: &str) -> Self {
        let ext_lower = ext.to_lowercase();
        let ext_clean = ext_lower.trim_start_matches('.');

        match ext_clean {
            // Images
            "jpg" | "jpeg" | "png" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" | "heic"
            | "heif" | "raw" | "cr2" | "nef" | "eps" => FileCategory::Image,
            // Video
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
            | "3gp" => FileCategory::Video,
            // Audio
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "aiff" => {
                FileCategory::Audio
            }
            // Documents
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "xlsm" | "ppt" | "pptx" | "odt" | "ods"
            | "odp" | "rtf" | "txt" | "md" | "epub" | "mobi" => FileCategory::Document,
            // Archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "dmg" | "iso" | "pkg" | "deb"
            | "rpm" => FileCategory::Archive,
            // Code
            "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "c" | "cpp" | "h" | "hpp"
            | "go" | "rb" | "php" | "swift" | "kt" | "scala" | "html" | "css" | "scss" | "sass"
            | "less" | "json" | "yaml" | "yml" | "toml" | "xml" | "sh" | "bash" | "zsh" | "sql" => {
                FileCategory::Code
            }
            // Fonts
            "ttf" | "otf" | "woff" | "woff2" | "eot" => FileCategory::Font,
            // Data
            "csv" | "parquet" | "avro" | "orc" | "sqlite" | "db" | "arrow" => FileCategory::Data,
            // Other
            _ => FileCategory::Other,
        }
    }
}

impl std::fmt::Display for FileCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileCategory::Image => write!(f, "image"),
            FileCategory::Video => write!(f, "video"),
            FileCategory::Audio => write!(f, "audio"),
            FileCategory::Document => write!(f, "document"),
            FileCategory::Archive => write!(f, "archive"),
            FileCategory::Code => write!(f, "code"),
            FileCategory::Font => write!(f, "font"),
            FileCategory::Data => write!(f, "data"),
            FileCategory::Other => write!(f, "other"),
        }
    }
}
