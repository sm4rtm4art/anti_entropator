//! External tool enrichers for file metadata
//!
//! Wrappers around ffprobe, exiftool, and pdfinfo to extract metadata.

use std::path::Path;
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;

/// Timeout for external tool subprocess execution.
/// Prevents hangs on malformed files.
const TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// Run a command with a timeout and kill-on-drop guarantee.
/// Returns `None` if the command fails to spawn, times out, or exits non-zero.
async fn run_tool(mut cmd: Command) -> Option<Output> {
    let child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let output = tokio::time::timeout(TOOL_TIMEOUT, child.wait_with_output())
        .await
        .ok()?
        .ok()?;
    if output.status.success() {
        Some(output)
    } else {
        None
    }
}

/// Extract datetime from image EXIF using exiftool
pub async fn exiftool_datetime(path: &Path) -> Option<(String, String)> {
    let mut cmd = Command::new("exiftool");
    cmd.arg("-DateTimeOriginal")
        .arg("-CreateDate")
        .arg("-s3")
        .arg(path);
    let output = run_tool(cmd).await?;

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
    let mut cmd = Command::new("ffprobe");
    cmd.arg("-v")
        .arg("quiet")
        .arg("-print_format")
        .arg("json")
        .arg("-show_entries")
        .arg("format_tags=creation_time")
        .arg(path);
    let output = run_tool(cmd).await?;

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
    let mut cmd = Command::new("pdfinfo");
    cmd.arg(path);
    let output = run_tool(cmd).await?;

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

    if date_parts.len() != 3 || time_parts.len() < 2 || time_parts.len() > 3 {
        return None;
    }

    // Validate date parts are numeric
    for part in &date_parts {
        if part.parse::<u32>().is_err() {
            return None;
        }
    }

    // Validate time parts are numeric
    for part in &time_parts {
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
/// Handles the common external-tool ISO 8601 formats:
/// - `2024-01-15T14:30:45Z`
/// - `2024-01-15T14:30:45.123456Z`
/// - `2024-01-15T14:30:45+02:00`
/// - `2024-01-15T14:30:45-05:00`
/// - `2024-01-15T14:30:45.123456-05:00`
///
/// Timezone decision: wall-clock date/time from the metadata string is
/// preserved and timezone suffixes (Z, +HH:MM, -HH:MM) are stripped without
/// conversion. No normalization to UTC or any fixed offset. These values drive
/// suggested filenames only, not catalog timestamp fields.
///
/// Validates shape and numeric fields only. Calendar and range validation
/// (e.g., month 1-12, hour 0-23) is deferred post-v0.3.
fn parse_iso_datetime(s: &str) -> Option<String> {
    let s = s.trim();
    let t_pos = s.find('T')?;
    let date = &s[..t_pos];
    let time_part = &s[t_pos + 1..];

    // Validate date: must be 3 numeric dash-separated parts
    let date_parts: Vec<&str> = date.split('-').collect();
    if date_parts.len() != 3 {
        return None;
    }
    for part in &date_parts {
        if part.parse::<u32>().is_err() {
            return None;
        }
    }

    // Strip milliseconds (everything after first '.')
    let time_no_millis = time_part.split('.').next()?;
    // Strip trailing 'Z'
    let time_no_tz = time_no_millis.split('Z').next()?;
    // Strip +HH:MM or -HH:MM offset using rfind to avoid confusion with date dashes
    let time_clean = if let Some(plus_pos) = time_no_tz.rfind('+') {
        let suffix = &time_no_tz[plus_pos + 1..];
        if !is_offset_shape(suffix) {
            return None;
        }
        &time_no_tz[..plus_pos]
    } else if let Some(minus_pos) = time_no_tz.rfind('-') {
        if minus_pos > 0 {
            let suffix = &time_no_tz[minus_pos + 1..];
            if !is_offset_shape(suffix) {
                return None;
            }
            &time_no_tz[..minus_pos]
        } else {
            time_no_tz
        }
    } else {
        time_no_tz
    };

    // Validate time is exactly HH:MM or HH:MM:SS (2-3 colon-separated numeric parts)
    let time_parts: Vec<&str> = time_clean.split(':').collect();
    if time_parts.len() < 2 || time_parts.len() > 3 {
        return None;
    }
    for part in &time_parts {
        if part.parse::<u32>().is_err() {
            return None;
        }
    }

    let formatted_time = time_clean.replace(':', "-");
    Some(format!("{}_{}", date, formatted_time))
}

/// Validate that a timezone offset suffix looks like "HH:MM" (digits:digits).
fn is_offset_shape(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    parts.len() == 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
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
        assert_eq!(parse_exif_datetime("2024:01:15 aa:bb:cc"), None);
    }

    #[test]
    fn parse_exif_single_time_component() {
        assert_eq!(parse_exif_datetime("2024:01:15 12"), None);
    }

    #[test]
    fn parse_exif_empty_time_part() {
        assert_eq!(parse_exif_datetime("2024:01:15 12::45"), None);
    }

    #[test]
    fn parse_exif_extra_time_components() {
        assert_eq!(parse_exif_datetime("2024:01:15 12:34:56:78"), None);
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
        assert_eq!(
            parse_iso_datetime("2024-01-15T14:30:45-05:00"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_iso_malformed_with_t() {
        assert_eq!(parse_iso_datetime("not-a-dateThello"), None);
    }

    #[test]
    fn parse_iso_just_t() {
        assert_eq!(parse_iso_datetime("T"), None);
    }

    #[test]
    fn parse_iso_no_date_digits() {
        assert_eq!(parse_iso_datetime("abcT12:00:00"), None);
    }

    #[test]
    fn parse_iso_single_time_component() {
        assert_eq!(parse_iso_datetime("2024-01-15T14"), None);
    }

    #[test]
    fn parse_iso_millis_with_negative_offset() {
        assert_eq!(
            parse_iso_datetime("2024-01-15T14:30:45.123456-05:00"),
            Some("2024-01-15_14-30-45".to_string())
        );
    }

    #[test]
    fn parse_iso_garbage_after_plus() {
        assert_eq!(parse_iso_datetime("2024-01-15T14:30:45+garbage"), None);
    }

    #[test]
    fn parse_iso_garbage_after_minus() {
        assert_eq!(parse_iso_datetime("2024-01-15T14:30:45-hello"), None);
    }

    #[test]
    fn parse_iso_extra_time_components() {
        assert_eq!(parse_iso_datetime("2024-01-15T12:34:56:78Z"), None);
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

    // ── run_tool ──

    #[tokio::test]
    async fn run_tool_missing_command_returns_none() {
        let cmd = Command::new("nonexistent_tool_xyz_12345");
        assert!(run_tool(cmd).await.is_none());
    }

    #[tokio::test]
    async fn run_tool_nonzero_exit_returns_none() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("exit 1");
        assert!(run_tool(cmd).await.is_none());
    }

    #[tokio::test]
    async fn run_tool_success_returns_output() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello");
        let output = run_tool(cmd).await;
        assert!(output.is_some());
        assert!(String::from_utf8_lossy(&output.unwrap().stdout).contains("hello"));
    }
}
