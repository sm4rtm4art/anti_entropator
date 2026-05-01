//! Statistics types for profiling

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aggregated statistics for a group of files
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupStats {
    pub count: u64,
    pub total_bytes: u64,
    pub min_bytes: u64,
    pub max_bytes: u64,
    pub largest_path: String,
    sizes: Vec<u64>,
}

impl GroupStats {
    pub fn new() -> Self {
        Self {
            count: 0,
            total_bytes: 0,
            min_bytes: u64::MAX,
            max_bytes: 0,
            largest_path: String::new(),
            sizes: Vec::new(),
        }
    }

    pub fn add(&mut self, size: u64, path: &str) {
        self.count += 1;
        self.total_bytes += size;
        self.sizes.push(size);

        if size < self.min_bytes {
            self.min_bytes = size;
        }
        if size > self.max_bytes {
            self.max_bytes = size;
            self.largest_path = path.to_string();
        }
    }

    pub fn avg_bytes(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_bytes as f64 / self.count as f64
        }
    }

    /// Calculate percentile (0-100)
    pub fn percentile(&self, p: u8) -> u64 {
        if self.sizes.is_empty() {
            return 0;
        }

        let mut sorted = self.sizes.clone();
        sorted.sort_unstable();

        let idx = ((p as f64 / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        sorted.get(idx).copied().unwrap_or(0)
    }

    pub fn p50(&self) -> u64 {
        self.percentile(50)
    }

    pub fn p90(&self) -> u64 {
        self.percentile(90)
    }

    pub fn p99(&self) -> u64 {
        self.percentile(99)
    }

    /// Finalize min_bytes if no files were added
    pub fn finalize(&mut self) {
        if self.count == 0 {
            self.min_bytes = 0;
        }
    }
}

/// Profile result containing all statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileResult {
    /// Path that was profiled
    pub path: String,

    /// Total file count
    pub file_count: u64,

    /// Total directory count
    pub dir_count: u64,

    /// Total symlink count
    pub symlink_count: u64,

    /// Total bytes across all files
    pub total_bytes: u64,

    /// Zero-byte file count
    pub zero_byte_count: u64,

    /// Stats by extension
    pub by_extension: HashMap<String, GroupStats>,

    /// Stats by MIME type
    pub by_mime: HashMap<String, GroupStats>,

    /// Stats by category
    pub by_category: HashMap<String, GroupStats>,

    /// Top N largest files
    pub largest_files: Vec<(u64, String)>,

    /// Files with no extension
    pub no_extension_examples: Vec<String>,

    /// Name pattern statistics
    pub name_patterns: HashMap<String, u64>,

    /// Duplicate estimation
    pub duplicate_estimate: DuplicateEstimate,

    /// Errors encountered
    pub errors: Vec<ProfileError>,
}

/// Duplicate estimation results
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DuplicateEstimate {
    /// Groups with same file size
    pub size_candidate_groups: u64,

    /// Groups confirmed by quick-hash
    pub quickhash_confirmed_groups: u64,

    /// Files that were quick-hashed
    pub files_hashed: u64,

    /// Estimated bytes reclaimable
    pub reclaimable_bytes: u64,

    /// Top duplicate groups (count, size, sample paths)
    pub top_groups: Vec<DuplicateGroup>,
}

/// A group of potential duplicates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub count: u64,
    pub size_bytes: u64,
    pub sample_paths: Vec<String>,
}

/// An error encountered during profiling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileError {
    pub path: String,
    pub error: String,
}

impl ProfileResult {
    pub fn new(path: String) -> Self {
        Self {
            path,
            file_count: 0,
            dir_count: 0,
            symlink_count: 0,
            total_bytes: 0,
            zero_byte_count: 0,
            by_extension: HashMap::new(),
            by_mime: HashMap::new(),
            by_category: HashMap::new(),
            largest_files: Vec::new(),
            no_extension_examples: Vec::new(),
            name_patterns: HashMap::new(),
            duplicate_estimate: DuplicateEstimate::default(),
            errors: Vec::new(),
        }
    }

    /// Finalize all stats after collection
    pub fn finalize(&mut self) {
        for stats in self.by_extension.values_mut() {
            stats.finalize();
        }
        for stats in self.by_mime.values_mut() {
            stats.finalize();
        }
        for stats in self.by_category.values_mut() {
            stats.finalize();
        }

        // Sort largest files
        self.largest_files
            .sort_by_key(|entry| std::cmp::Reverse(entry.0));
        self.largest_files.truncate(15);
    }
}
