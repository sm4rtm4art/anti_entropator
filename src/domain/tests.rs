//! Unit tests for domain types
//!
//! Tests for FileCategory, ContentHash, and PartialHash.

use super::*;
use std::collections::HashSet;

// ==================== FileCategory::from_extension tests ====================

#[test]
fn file_category_from_extension_images() {
    let image_exts = [
        "jpg", "jpeg", "png", "gif", "webp", "svg", "bmp", "ico", "heic", "raw",
    ];
    for ext in image_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Image,
            "Extension '{}' should be Image",
            ext
        );
        // Also test with dot prefix
        assert_eq!(
            FileCategory::from_extension(&format!(".{}", ext)),
            FileCategory::Image,
            "Extension '.{}' should be Image",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_videos() {
    let video_exts = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v"];
    for ext in video_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Video,
            "Extension '{}' should be Video",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_audio() {
    let audio_exts = ["mp3", "wav", "flac", "aac", "ogg", "wma", "m4a", "opus"];
    for ext in audio_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Audio,
            "Extension '{}' should be Audio",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_documents() {
    let doc_exts = [
        "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "epub",
    ];
    for ext in doc_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Document,
            "Extension '{}' should be Document",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_archives() {
    let archive_exts = ["zip", "tar", "gz", "bz2", "xz", "7z", "rar", "dmg", "iso"];
    for ext in archive_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Archive,
            "Extension '{}' should be Archive",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_code() {
    let code_exts = [
        "rs", "py", "js", "ts", "java", "go", "rb", "html", "css", "json", "yaml",
    ];
    for ext in code_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Code,
            "Extension '{}' should be Code",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_fonts() {
    let font_exts = ["ttf", "otf", "woff", "woff2", "eot"];
    for ext in font_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Font,
            "Extension '{}' should be Font",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_data() {
    let data_exts = ["csv", "parquet", "avro", "orc", "sqlite", "db", "arrow"];
    for ext in data_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Data,
            "Extension '{}' should be Data",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_unknown() {
    let unknown_exts = ["xyz", "unknown", "foo", ""];
    for ext in unknown_exts {
        assert_eq!(
            FileCategory::from_extension(ext),
            FileCategory::Other,
            "Extension '{}' should be Other",
            ext
        );
    }
}

#[test]
fn file_category_from_extension_case_insensitive() {
    assert_eq!(FileCategory::from_extension("PDF"), FileCategory::Document);
    assert_eq!(FileCategory::from_extension("Jpg"), FileCategory::Image);
    assert_eq!(FileCategory::from_extension("MP4"), FileCategory::Video);
}

// ==================== FileCategory::from_mime tests ====================

#[test]
fn file_category_from_mime_images() {
    let image_mimes = [
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "image/svg+xml",
    ];
    for mime in image_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Image,
            "MIME '{}' should be Image",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_videos() {
    let video_mimes = [
        "video/mp4",
        "video/webm",
        "video/x-msvideo",
        "video/quicktime",
    ];
    for mime in video_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Video,
            "MIME '{}' should be Video",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_audio() {
    let audio_mimes = ["audio/mpeg", "audio/wav", "audio/ogg", "audio/flac"];
    for mime in audio_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Audio,
            "MIME '{}' should be Audio",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_documents() {
    let doc_mimes = [
        "application/pdf",
        "text/plain",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "application/vnd.ms-excel",
        "application/epub+zip",
    ];
    for mime in doc_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Document,
            "MIME '{}' should be Document",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_archives() {
    let archive_mimes = [
        "application/zip",
        "application/x-tar",
        "application/gzip",
        "application/x-7z-compressed",
        "application/x-rar-compressed",
    ];
    for mime in archive_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Archive,
            "MIME '{}' should be Archive",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_code() {
    // Note: text/html and text/css are caught by text/ prefix → Document
    // Code category only catches application/* types with code keywords
    let code_mimes = [
        "application/javascript",
        "application/json",
        "application/xml",
        "application/xhtml+xml",
    ];
    for mime in code_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Code,
            "MIME '{}' should be Code",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_text_types_are_documents() {
    // text/* MIME types are classified as documents (text/html, text/css, text/plain)
    let text_mimes = ["text/html", "text/css", "text/plain", "text/markdown"];
    for mime in text_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Document,
            "MIME '{}' should be Document (text/* types)",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_fonts() {
    let font_mimes = [
        "font/ttf",
        "font/otf",
        "font/woff",
        "font/woff2",
        "application/font-woff",
    ];
    for mime in font_mimes {
        assert_eq!(
            FileCategory::from_mime(mime),
            FileCategory::Font,
            "MIME '{}' should be Font",
            mime
        );
    }
}

#[test]
fn file_category_from_mime_unknown() {
    assert_eq!(
        FileCategory::from_mime("application/octet-stream"),
        FileCategory::Other
    );
}

#[test]
fn file_category_from_mime_case_insensitive() {
    assert_eq!(FileCategory::from_mime("IMAGE/JPEG"), FileCategory::Image);
    assert_eq!(
        FileCategory::from_mime("Application/PDF"),
        FileCategory::Document
    );
}

// ==================== ContentHash tests ====================

#[test]
fn content_hash_new_and_as_str() {
    let hash = ContentHash::new("abc123def456".to_string());
    assert_eq!(hash.as_str(), "abc123def456");
}

#[test]
fn content_hash_display() {
    let hash = ContentHash::new("abc123".to_string());
    assert_eq!(format!("{}", hash), "abc123");
}

#[test]
fn content_hash_to_object_key_normal() {
    let hash = ContentHash::new("abcd1234567890".to_string());
    assert_eq!(hash.to_object_key(), "sha256/ab/cd/abcd1234567890");
}

#[test]
fn content_hash_to_object_key_realistic_sha256() {
    // A realistic 64-char SHA-256 hash
    let hash = ContentHash::new(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
    );
    assert_eq!(
        hash.to_object_key(),
        "sha256/e3/b0/e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn content_hash_to_object_key_short_hash() {
    // Edge case: hash shorter than 4 chars
    let hash = ContentHash::new("ab".to_string());
    assert_eq!(hash.to_object_key(), "sha256/ab");
}

#[test]
fn content_hash_to_object_key_exactly_4_chars() {
    let hash = ContentHash::new("abcd".to_string());
    assert_eq!(hash.to_object_key(), "sha256/ab/cd/abcd");
}

#[test]
fn content_hash_equality() {
    let hash1 = ContentHash::new("abc123".to_string());
    let hash2 = ContentHash::new("abc123".to_string());
    let hash3 = ContentHash::new("xyz789".to_string());

    assert_eq!(hash1, hash2);
    assert_ne!(hash1, hash3);
}

#[test]
fn content_hash_hashable() {
    let mut set = HashSet::new();
    set.insert(ContentHash::new("abc".to_string()));
    set.insert(ContentHash::new("abc".to_string())); // Duplicate
    set.insert(ContentHash::new("def".to_string()));

    assert_eq!(set.len(), 2);
}

// ==================== PartialHash tests ====================

#[test]
fn partial_hash_new_and_as_str() {
    let hash = PartialHash::new("partial123".to_string());
    assert_eq!(hash.as_str(), "partial123");
}

#[test]
fn partial_hash_equality() {
    let hash1 = PartialHash::new("abc".to_string());
    let hash2 = PartialHash::new("abc".to_string());
    assert_eq!(hash1, hash2);
}

#[test]
fn partial_hash_hashable() {
    let mut set = HashSet::new();
    set.insert(PartialHash::new("abc".to_string()));
    set.insert(PartialHash::new("def".to_string()));
    assert_eq!(set.len(), 2);
}

// ==================== FileCategory Display tests ====================

#[test]
fn file_category_display_all_variants() {
    assert_eq!(format!("{}", FileCategory::Image), "image");
    assert_eq!(format!("{}", FileCategory::Video), "video");
    assert_eq!(format!("{}", FileCategory::Audio), "audio");
    assert_eq!(format!("{}", FileCategory::Document), "document");
    assert_eq!(format!("{}", FileCategory::Archive), "archive");
    assert_eq!(format!("{}", FileCategory::Code), "code");
    assert_eq!(format!("{}", FileCategory::Font), "font");
    assert_eq!(format!("{}", FileCategory::Data), "data");
    assert_eq!(format!("{}", FileCategory::Other), "other");
}
