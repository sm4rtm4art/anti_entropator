//! Directory scanner for profiling

use crate::domain::stats::{DuplicateEstimate, DuplicateGroup, ProfileError, ProfileResult};
use crate::domain::FileCategory;
use crate::profile::ScanOptions;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use indicatif::ProgressBar;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

/// Name patterns to detect (for "bad filenames" analysis)
struct NamePatterns {
    screenshot: Regex,
    download: Regex,
    untitled: Regex,
    image_generic: Regex,
    video_generic: Regex,
    uuid_like: Regex,
    long_hex: Regex,
    querystring_like: Regex,
    numbered_suffix: Regex,
}

impl NamePatterns {
    fn new() -> Result<Self> {
        Ok(Self {
            screenshot: Regex::new(r"(?i)^screenshot\b").context("Invalid regex: screenshot")?,
            download: Regex::new(r"(?i)^download(\s*\(\d+\))?(\b|\.)")
                .context("Invalid regex: download")?,
            untitled: Regex::new(r"(?i)^untitled(\b|\.)").context("Invalid regex: untitled")?,
            image_generic: Regex::new(r"(?i)^(img|image|photo)[-_ ]?\d+")
                .context("Invalid regex: image_generic")?,
            video_generic: Regex::new(r"(?i)^(vid|video)[-_ ]?\d+")
                .context("Invalid regex: video_generic")?,
            uuid_like: Regex::new(
                r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            )
            .context("Invalid regex: uuid_like")?,
            long_hex: Regex::new(r"(?i)^[0-9a-f]{20,}").context("Invalid regex: long_hex")?,
            querystring_like: Regex::new(r".*[?&]=.+")
                .context("Invalid regex: querystring_like")?,
            numbered_suffix: Regex::new(r"[-_ ]\(\d+\)\.[^.]+$")
                .context("Invalid regex: numbered_suffix")?,
        })
    }

    fn check(&self, filename: &str) -> Vec<&'static str> {
        let mut matches = Vec::new();

        if self.screenshot.is_match(filename) {
            matches.push("screenshot");
        }
        if self.download.is_match(filename) {
            matches.push("download");
        }
        if self.untitled.is_match(filename) {
            matches.push("untitled");
        }
        if self.image_generic.is_match(filename) {
            matches.push("image_generic");
        }
        if self.video_generic.is_match(filename) {
            matches.push("video_generic");
        }
        if self.uuid_like.is_match(filename) {
            matches.push("uuid_like");
        }
        if self.long_hex.is_match(filename) {
            matches.push("long_hex");
        }
        if self.querystring_like.is_match(filename) {
            matches.push("querystring_like");
        }
        if self.numbered_suffix.is_match(filename) {
            matches.push("numbered_suffix");
        }

        matches
    }
}

/// Get file extension, handling compound extensions
fn get_extension(filename: &str) -> String {
    let lower = filename.to_lowercase();

    // Check for compound extensions
    for compound in &[".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".tar.br"] {
        if lower.ends_with(compound) {
            return compound.to_string();
        }
    }

    // Get simple extension
    Path::new(filename)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
        .unwrap_or_else(|| "(none)".to_string())
}

/// Quick hash the first N bytes of a file
fn quick_hash(path: &Path, block_size: usize) -> Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; block_size];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    let result = hasher.finalize();

    Ok(format!("{:x}", result))
}

/// Detect MIME type from file content
fn detect_mime(path: &Path) -> Option<String> {
    infer::get_from_path(path)
        .ok()
        .flatten()
        .map(|t| t.mime_type().to_string())
}

/// Get file timestamps
#[allow(dead_code)] // Will be used when we add timestamp columns to output
fn get_timestamps(metadata: &fs::Metadata) -> (Option<DateTime<Utc>>, Option<DateTime<Utc>>) {
    let modified = metadata.modified().ok().map(DateTime::<Utc>::from);
    let created = metadata.created().ok().map(DateTime::<Utc>::from);
    (modified, created)
}

/// Main scan function
pub async fn scan(
    root: &Path,
    options: &ScanOptions,
    progress: Option<&ProgressBar>,
) -> Result<ProfileResult> {
    let mut result = ProfileResult::new(root.display().to_string());
    let patterns = NamePatterns::new()?;

    // For duplicate detection: group by size
    let mut by_size: HashMap<u64, Vec<String>> = HashMap::new();

    // Track largest files
    let mut largest: Vec<(u64, String)> = Vec::new();
    const MAX_LARGEST: usize = 50;

    // Walk the directory
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                result.errors.push(ProfileError {
                    path: e
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    error: e.to_string(),
                });
                continue;
            }
        };

        let path = entry.path();
        let path_str = path.display().to_string();

        if let Some(pb) = progress {
            pb.set_message(format!("Scanning: {} files", result.file_count));
        }

        // Handle symlinks
        if entry.path_is_symlink() {
            result.symlink_count += 1;
            continue;
        }

        // Handle directories
        if entry.file_type().is_dir() {
            result.dir_count += 1;
            continue;
        }

        // Handle files
        if entry.file_type().is_file() {
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    result.errors.push(ProfileError {
                        path: path_str.clone(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };

            let size = metadata.len();
            let filename = entry.file_name().to_string_lossy().to_string();
            let extension = get_extension(&filename);

            result.file_count += 1;
            result.total_bytes += size;

            if size == 0 {
                result.zero_byte_count += 1;
            }

            // Stats by extension
            result
                .by_extension
                .entry(extension.clone())
                .or_default()
                .add(size, &path_str);

            // Stats by category
            let category = FileCategory::from_extension(&extension);
            result
                .by_category
                .entry(category.to_string())
                .or_default()
                .add(size, &path_str);

            // MIME type detection
            if options.detect_mime {
                if let Some(mime) = detect_mime(path) {
                    result
                        .by_mime
                        .entry(mime.clone())
                        .or_default()
                        .add(size, &path_str);
                }
            }

            // Track no-extension files
            if extension == "(none)" && result.no_extension_examples.len() < 20 {
                result.no_extension_examples.push(path_str.clone());
            }

            // Track largest files
            largest.push((size, path_str.clone()));
            if largest.len() > MAX_LARGEST * 2 {
                largest.sort_by_key(|entry| std::cmp::Reverse(entry.0));
                largest.truncate(MAX_LARGEST);
            }

            // Name pattern analysis
            for pattern in patterns.check(&filename) {
                *result.name_patterns.entry(pattern.to_string()).or_insert(0) += 1;
            }

            // Duplicate candidate tracking
            if options.detect_duplicates && size > 0 {
                by_size.entry(size).or_default().push(path_str);
            }
        }
    }

    // Finalize largest files
    largest.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    largest.truncate(15);
    result.largest_files = largest;

    // Duplicate estimation via quick-hash
    if options.detect_duplicates {
        if let Some(pb) = progress {
            pb.set_message("Estimating duplicates...");
        }

        let mut estimate = DuplicateEstimate::default();

        // Get size candidate groups (same size = potential duplicate)
        let size_candidates: Vec<(u64, Vec<String>)> = by_size
            .into_iter()
            .filter(|(_, paths)| paths.len() > 1)
            .collect();

        estimate.size_candidate_groups = size_candidates.len() as u64;

        // Quick-hash candidates to confirm
        let mut quickhash_groups: HashMap<(u64, String), Vec<String>> = HashMap::new();
        let mut files_hashed = 0;
        let mut _hash_errors = 0;

        for (size, paths) in size_candidates.iter() {
            for path in paths {
                if files_hashed >= options.max_hash_files {
                    break;
                }

                match quick_hash(Path::new(path), 64 * 1024) {
                    Ok(hash) => {
                        quickhash_groups
                            .entry((*size, hash))
                            .or_default()
                            .push(path.clone());
                        files_hashed += 1;
                    }
                    Err(_) => {
                        _hash_errors += 1;
                    }
                }
            }

            if files_hashed >= options.max_hash_files {
                break;
            }
        }

        estimate.files_hashed = files_hashed as u64;

        // Confirmed duplicate groups
        let mut confirmed: Vec<((u64, String), Vec<String>)> = quickhash_groups
            .into_iter()
            .filter(|(_, paths)| paths.len() > 1)
            .collect();

        confirmed.sort_by(|a, b| {
            let count_cmp = b.1.len().cmp(&a.1.len());
            if count_cmp == std::cmp::Ordering::Equal {
                b.0 .0.cmp(&a.0 .0)
            } else {
                count_cmp
            }
        });

        estimate.quickhash_confirmed_groups = confirmed.len() as u64;

        // Calculate reclaimable bytes
        for ((size, _), paths) in &confirmed {
            estimate.reclaimable_bytes += (paths.len() as u64 - 1) * size;
        }

        // Top duplicate groups
        for ((size, _), paths) in confirmed.iter().take(10) {
            estimate.top_groups.push(DuplicateGroup {
                count: paths.len() as u64,
                size_bytes: *size,
                sample_paths: paths.iter().take(5).cloned().collect(),
            });
        }

        result.duplicate_estimate = estimate;
    }

    result.finalize();

    Ok(result)
}
