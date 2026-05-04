//! External tool enrichers for file metadata
//!
//! Wrappers around ffprobe, exiftool, and pdfinfo to extract metadata.

use std::path::Path;
use tokio::process::Command;

/// Extract datetime from image EXIF using exiftool
pub async fn exiftool_datetime(path: &Path) -> Option<(String, String)> {
    let output = Command::new("exiftool")
        .arg("-DateTimeOriginal")
        .arg("-CreateDate")
        .arg("-s3")
        .arg(path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse EXIF datetime format: "2024:01:15 14:30:45"
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Try to parse EXIF datetime
        if let Some(formatted) = parse_exif_datetime(trimmed) {
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                .unwrap_or_default();

            return Some((
                format!("{}{}", formatted, ext),
                "exif_datetime_original".to_string(),
            ));
        }
    }

    None
}

/// Extract datetime from video/audio using ffprobe
pub async fn ffprobe_datetime(path: &Path) -> Option<(String, String)> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_entries")
        .arg("format_tags=creation_time")
        .arg(path)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse JSON output
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
        if let Some(creation_time) = json
            .get("format")
            .and_then(|f| f.get("tags"))
            .and_then(|t| t.get("creation_time"))
            .and_then(|c| c.as_str())
        {
            // Parse ISO datetime: "2024-01-15T14:30:45.000000Z"
            if let Some(formatted) = parse_iso_datetime(creation_time) {
                let ext = path
                    .extension()
                    .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                    .unwrap_or_default();

                return Some((
                    format!("{}{}", formatted, ext),
                    "ffprobe_creation_time".to_string(),
                ));
            }
        }
    }

    None
}

/// Extract title from PDF using pdfinfo
pub async fn pdfinfo_title(path: &Path) -> Option<(String, String)> {
    let output = Command::new("pdfinfo").arg(path).output().await.ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if line.starts_with("Title:") {
            let title = line.trim_start_matches("Title:").trim();

            if !title.is_empty() && title.len() > 3 && !is_useless_title(title) {
                // Sanitize for filename
                let sanitized = sanitize_filename(title);

                if !sanitized.is_empty() && sanitized.len() > 3 {
                    return Some((format!("{}.pdf", sanitized), "pdfinfo_title".to_string()));
                }
            }
        }
    }

    None
}

/// Parse EXIF datetime format "2024:01:15 14:30:45"
fn parse_exif_datetime(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let date_parts: Vec<&str> = parts[0].split(':').collect();
    let time_parts: Vec<&str> = parts[1].split(':').collect();

    if date_parts.len() != 3 || time_parts.len() < 2 {
        return None;
    }

    // Validate parts are numeric
    for part in &date_parts {
        if part.parse::<u32>().is_err() {
            return None;
        }
    }

    Some(format!(
        "{}-{}-{}_{}-{}-{}",
        date_parts[0],
        date_parts[1],
        date_parts[2],
        time_parts[0],
        time_parts[1],
        time_parts.get(2).unwrap_or(&"00")
    ))
}

/// Parse ISO datetime format "2024-01-15T14:30:45.000000Z"
///
/// TODO: This parser is fragile — it strips timezone by splitting on `-` which
/// only works because ISO 8601 time components use colons. Replace with `chrono`
/// parsing and add unit tests covering positive/negative offsets, milliseconds,
/// and edge cases.
fn parse_iso_datetime(s: &str) -> Option<String> {
    // Simple parsing - just extract the date and time parts
    let s = s.trim();

    if let Some(t_pos) = s.find('T') {
        let date = &s[..t_pos];
        let time_part = &s[t_pos + 1..];

        // Remove timezone and milliseconds
        let time = time_part
            .split('.')
            .next()?
            .split('Z')
            .next()?
            .split('+')
            .next()?
            .split('-')
            .next()?;

        let time_clean = time.replace(':', "-");

        return Some(format!("{}_{}", date, time_clean));
    }

    None
}

/// Check if PDF title is useless (generic, placeholder, etc.)
fn is_useless_title(title: &str) -> bool {
    let lower = title.to_lowercase();

    lower == "untitled"
        || lower == "document"
        || lower == "microsoft word"
        || lower.starts_with("slide")
        || lower.starts_with("page")
        || lower.len() < 4
        || title
            .chars()
            .all(|c| c.is_ascii_digit() || c == '-' || c == '_')
}

/// Sanitize string for use as filename
fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            '\n' | '\r' | '\t' => '_',
            _ if c.is_control() => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(100) // Limit length
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_exif_datetime ──

    #[test]
    fn parse_exif_valid() {
        assert_eq!(
            parse_exif_datetime("2024:01:15 14:30:45"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_exif_missing_seconds() {
        assert_eq!(
            parse_exif_datetime("2024:01:15 14:30"),
            Some("2024-01-15_14-30-00".to_string())
        );
    }

    #[test]
    fn parse_exif_empty() {
        assert_eq!(parse_exif_datetime(""), None);
    }

    #[test]
    fn parse_exif_garbage() {
        assert_eq!(parse_exif_datetime("not a date"), None);
    }

    #[test]
    fn parse_exif_non_numeric_date() {
        assert_eq!(parse_exif_datetime("abcd:ef:gh 12:34:56"), None);
    }

    #[test]
    fn parse_exif_non_numeric_time() {
        // Known weakness: time parts are not validated. Hardening deferred to S4 (triage #12).
        assert_eq!(
            parse_exif_datetime("2024:01:15 aa:bb:cc"),
            Some("2024-01-15_aa-bb-cc".to_string())
        );
    }

    // ── parse_iso_datetime ──

    #[test]
    fn parse_iso_basic() {
        assert_eq!(
            parse_iso_datetime("2024-01-15T14:30:45Z"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_iso_with_millis() {
        assert_eq!(
            parse_iso_datetime("2024-01-15T14:30:45.123456Z"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_iso_positive_offset() {
        assert_eq!(
            parse_iso_datetime("2024-01-15T14:30:45+02:00"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_iso_negative_offset() {
        // The parser chain: split('.') -> split('Z') -> split('+') -> split('-').
        // For "14:30:45-05:00": no '.', no 'Z', no '+', split('-') gives "14:30:45".
        // This happens to produce correct output for this case, but the approach is
        // fragile for edge cases. Hardening deferred to S4 (triage #12).
        assert_eq!(
            parse_iso_datetime("2024-01-15T14:30:45-05:00"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_iso_malformed_with_t() {
        // Known weakness: parser accepts any string containing `T`, no date/time validation.
        // Hardening deferred to S4 (triage #12).
        assert_eq!(
            parse_iso_datetime("not-a-dateThello"),
            Some("not-a-date_hello".to_string())
        );
    }

    #[test]
    fn parse_iso_no_t() {
        assert_eq!(parse_iso_datetime("2024-01-15 14:30:45"), None);
    }

    #[test]
    fn parse_iso_empty() {
        assert_eq!(parse_iso_datetime(""), None);
    }

    // ── is_useless_title ──

    #[test]
    fn useless_title_untitled() {
        assert!(is_useless_title("Untitled"));
    }

    #[test]
    fn useless_title_document() {
        assert!(is_useless_title("document"));
    }

    #[test]
    fn useless_title_short() {
        assert!(is_useless_title("ab"));
    }

    #[test]
    fn useless_title_numeric() {
        assert!(is_useless_title("123-456"));
    }

    #[test]
    fn useless_title_real() {
        assert!(!is_useless_title("Annual Report 2024"));
    }

    // ── sanitize_filename ──

    #[test]
    fn sanitize_replaces_slashes() {
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    }

    #[test]
    fn sanitize_limits_length() {
        let long = "a".repeat(200);
        let result = sanitize_filename(&long);
        assert_eq!(result.len(), 100);
    }

    #[test]
    fn sanitize_preserves_normal() {
        assert_eq!(sanitize_filename("photo_2024.jpg"), "photo_2024.jpg");
    }
}
