//! Ingest module - Upload files to the lakehouse
//!
//! Implements content-addressed storage with Iceberg catalog integration.

mod output;
mod state;
mod upload;

use crate::cli::{IngestArgs, IngestOutputFormat};
use crate::domain::observation::normalize_relative_path;
use crate::domain::{ContentHash, FileCategory, FileInfo, ObservationStatus};
use crate::file_hash;
use crate::lakehouse::{writer, LakehouseConfig};
use crate::scan::scan_file;
use crate::storage;
use anyhow::{Context, Result};
use chrono::Utc;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use opendal::Operator;
use output::{
    catalog_commit_from_result, outcome_error, print_human_report, print_json_report, IngestCounts,
    IngestMode, IngestSummary,
};
use std::collections::HashSet;
use std::path::Path;
use uuid::Uuid;
use walkdir::WalkDir;

/// Whether the content-addressed blob had to be stored (ADR-009 blob axis).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlobState {
    /// Uploaded in ingest mode, or "would upload" in preview modes.
    Uploaded,
    /// Already in the store and verified against the local file.
    Existing,
}

/// Result of processing a single file during ingest. The two ADR-009 axes are
/// independent: a path can be newly observed while its blob already exists
/// (identical bytes at a second path), and vice versa.
enum IngestOutcome {
    /// A `present` observation row is (or would be) appended for this path.
    Observed(Box<FileInfo>, BlobState),
    /// The last observation of this path already records this content; no
    /// row. The blob was verified (and restored if it had gone missing).
    Unchanged(BlobState),
}

/// Run the ingest command
pub async fn run(args: IngestArgs) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    let config = LakehouseConfig::default();
    let json_output = args.format == IngestOutputFormat::Json;
    let mode = IngestMode::from_flags(args.dry_run, args.offline);
    let ctx = RunContext::new(&path, args.source.as_deref());
    tracing::info!(run_id = %ctx.run_id, source_id = %ctx.source_id, mode = mode.label(), "Starting ingest run");

    if !json_output {
        println!();
        println!(
            "{}",
            style("═══════════════════════════════════════════════════════════════").cyan()
        );
        println!("  📤 Anti-Entropator Ingest");
        println!(
            "{}",
            style("═══════════════════════════════════════════════════════════════").cyan()
        );
        println!();
        println!("  Path:    {}", path.display());
        println!("  Source:  {}", ctx.source_id);
        println!("  Target:  {}", config.warehouse);
        println!("  Mode:    {}", mode.label());
        println!("  Run:     {}", ctx.run_id);
        println!();
    }

    // Check lakehouse connectivity first (skipped only by --dry-run --offline)
    if mode.is_connected() {
        if !json_output {
            print!("  Checking lakehouse connectivity... ");
        }
        match check_connectivity(&config).await {
            Ok(_) => {
                if !json_output {
                    println!("{}", style("OK").green());
                }
            }
            Err(e) => {
                if !json_output {
                    println!("{}", style("FAILED").red());
                }
                anyhow::bail!(
                    "Cannot connect to lakehouse: {}. Run `docker compose up -d`",
                    e
                );
            }
        }
    }

    // Read the current catalog state for this source (ADR-009 "unchanged →
    // no row"). Connected modes fail closed here: without this state the run
    // cannot tell an unchanged path from a changed one. Offline previews have
    // no state and report every candidate as "would observe".
    let current_state = if mode.is_connected() {
        if !json_output {
            print!("  Reading catalog state for source... ");
        }
        match load_state(&config, &ctx.source_id).await {
            Ok(s) => {
                if !json_output {
                    println!("{}", style(format!("{} known paths", s.len())).green());
                }
                Some(s)
            }
            Err(e) => {
                if !json_output {
                    println!("{}", style("FAILED").red());
                }
                return Err(e.context(
                    "Cannot read current catalog state; refusing to guess which paths are unchanged",
                ));
            }
        }
    } else {
        None
    };

    // Collect files to ingest
    let files = collect_files(&path, &args)?;
    tracing::info!(
        candidates = files.len(),
        mode = mode.label(),
        "Collected ingest candidates"
    );
    if !json_output {
        println!("  Found {} files to ingest", files.len());
    }

    if files.is_empty() {
        return finalize_ingest(
            IngestSummary::build(
                mode,
                ctx.run_id,
                ctx.source_id.clone(),
                IngestCounts::default(),
                &[],
                catalog_commit_from_result(&None),
            ),
            None,
            args.format,
        );
    }

    // Create progress bar (stderr). Hide it in JSON mode so stdout stays one document.
    let pb = if json_output {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(files.len() as u64);
        let pb_style = ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .context("Invalid ingest progress bar template")?
            .progress_chars("#>-");
        pb.set_style(pb_style);
        pb
    };

    let mut tally = Tally::default();
    let mut errors = Vec::new();

    // Create OpenDAL operator (needed for existence checks and uploads)
    let operator = if mode.is_connected() {
        Some(storage::create_operator(&config)?)
    } else {
        None
    };

    for file_path in &files {
        pb.inc(1);
        let file_name = file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        pb.set_message(file_name.clone());

        let ctx_for_file = FileContext {
            root: &path,
            config: &config,
            operator: operator.as_ref(),
            mode,
            run: &ctx,
            state: current_state.as_ref(),
        };
        match process_file(file_path, &ctx_for_file).await {
            Ok(outcome) => tally.record(outcome),
            Err(e) => {
                errors.push(format!("{}: {}", file_path.display(), e));
            }
        }
    }

    pb.finish_and_clear();

    let Tally {
        mut counts,
        observations,
    } = tally;
    counts.candidates = files.len() as u64;

    // Commit to Iceberg only in ingest mode and only when something was observed
    let commit_result = if mode.writes() && !observations.is_empty() {
        if !json_output {
            print!("  Committing metadata to Iceberg catalog... ");
        }
        match writer::commit_files(observations, &config).await {
            Ok(_) => {
                if !json_output {
                    println!("{}", style("OK").green());
                }
                Some(Ok(()))
            }
            Err(e) => {
                if !json_output {
                    println!("{}", style("FAILED").red());
                }
                Some(Err(e))
            }
        }
    } else {
        None
    };

    let catalog_commit = catalog_commit_from_result(&commit_result);
    finalize_ingest(
        IngestSummary::build(
            mode,
            ctx.run_id,
            ctx.source_id.clone(),
            counts,
            &errors,
            catalog_commit,
        ),
        commit_result,
        args.format,
    )
}

/// Fold per-file outcomes into summary counts and the rows to commit.
#[derive(Default)]
struct Tally {
    counts: IngestCounts,
    observations: Vec<FileInfo>,
}

impl Tally {
    fn record(&mut self, outcome: IngestOutcome) {
        let blob = match outcome {
            IngestOutcome::Observed(info, blob) => {
                self.counts.observed += 1;
                if blob == BlobState::Uploaded {
                    self.counts.bytes += info.size_bytes;
                }
                self.observations.push(*info);
                blob
            }
            IngestOutcome::Unchanged(blob) => {
                self.counts.unchanged += 1;
                blob
            }
        };
        match blob {
            BlobState::Uploaded => self.counts.uploaded += 1,
            BlobState::Existing => self.counts.already_exists += 1,
        }
    }
}

/// Build a DataFusion session and load the latest observation per path.
async fn load_state(config: &LakehouseConfig, source_id: &str) -> Result<state::CurrentState> {
    let session = crate::query::build_session(config).await?;
    state::load_current_state(&session, source_id).await
}

/// Finalize the ingest operation: print summary and determine command outcome.
fn finalize_ingest(
    summary: IngestSummary,
    commit_result: Option<Result<()>>,
    format: IngestOutputFormat,
) -> Result<()> {
    match format {
        IngestOutputFormat::Json => print_json_report(&summary)?,
        IngestOutputFormat::Human if summary.counts.candidates == 0 => {
            println!("\n  Nothing to ingest.");
        }
        IngestOutputFormat::Human => print_human_report(&summary),
    }

    outcome_error(&summary, commit_result)
}

/// Check lakehouse connectivity
async fn check_connectivity(config: &LakehouseConfig) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // Check RustFS
    client
        .get(&config.s3_endpoint)
        .send()
        .await
        .context("Cannot connect to RustFS")?;

    Ok(())
}

/// Collect files to ingest based on filters
fn collect_files(path: &Path, args: &IngestArgs) -> Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    let mut type_filter: Option<HashSet<String>> = None;

    // Parse glob patterns upfront (fail early on invalid syntax)
    let exclude_patterns: Vec<glob::Pattern> = args
        .exclude
        .iter()
        .map(|p| {
            glob::Pattern::new(p).with_context(|| format!("invalid exclude glob pattern: '{}'", p))
        })
        .collect::<Result<Vec<_>>>()?;
    let include_patterns: Vec<glob::Pattern> = args
        .include
        .iter()
        .map(|p| {
            glob::Pattern::new(p).with_context(|| format!("invalid include glob pattern: '{}'", p))
        })
        .collect::<Result<Vec<_>>>()?;

    // Parse type filter
    if !args.types.is_empty() {
        type_filter = Some(args.types.iter().map(|t| t.to_lowercase()).collect());
    }

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry?;

        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();
        let filename = file_path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();

        // Apply exclude patterns (match against filename)
        if exclude_patterns.iter().any(|pat| pat.matches(&filename)) {
            continue;
        }

        // Apply include patterns (match against filename)
        if !include_patterns.is_empty()
            && !include_patterns.iter().any(|pat| pat.matches(&filename))
        {
            continue;
        }

        // Apply type filter
        if let Some(ref types) = type_filter {
            let ext = file_path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            let category = FileCategory::from_extension(&ext).to_string();

            if !types.contains(&category) && !types.contains(&ext) {
                continue;
            }
        }

        // Apply size filter
        if let Some(max) = args.max_size {
            let metadata = entry.metadata().with_context(|| {
                format!(
                    "Failed to read file metadata for size filtering: {}",
                    file_path.display()
                )
            })?;
            if metadata.len() > max {
                continue;
            }
        }

        files.push(file_path.to_path_buf());

        // Apply limit
        if let Some(limit) = args.limit {
            if files.len() >= limit {
                break;
            }
        }
    }

    Ok(files)
}

/// Identity shared by every observation of one ingest run (ADR-009).
#[derive(Debug, Clone)]
struct RunContext {
    /// Identifies this logical ingest attempt; stamped on every committed row.
    run_id: Uuid,
    /// Stable identifier of the ingest root. Slice 3a derives it from the
    /// canonical root path; `--source` (slice 3b) will allow overriding it.
    source_id: String,
}

impl RunContext {
    /// `source` overrides the default `source_id` (the canonical root path).
    fn new(root: &Path, source: Option<&str>) -> Self {
        Self {
            run_id: Uuid::new_v4(),
            source_id: source
                .map(str::to_owned)
                .unwrap_or_else(|| root.to_string_lossy().into_owned()),
        }
    }
}

/// Everything `process_file` needs besides the file itself.
struct FileContext<'a> {
    root: &'a Path,
    config: &'a LakehouseConfig,
    /// `None` in `--dry-run --offline`.
    operator: Option<&'a Operator>,
    mode: IngestMode,
    run: &'a RunContext,
    /// Latest observation per path for this source; `None` when the catalog
    /// was not read (`--dry-run --offline`).
    state: Option<&'a state::CurrentState>,
}

/// How many times a file whose bytes changed mid-upload is rescanned and
/// retried before it is reported as a per-file error.
const SOURCE_CHANGED_RETRIES: usize = 1;

/// Process a single file: scan and hash it, decide whether the path needs a
/// new observation, then depending on `mode` stop (`--dry-run --offline`),
/// verify the blob only (`--dry-run`), or verify and upload (ingest).
async fn process_file(path: &Path, fc: &FileContext<'_>) -> Result<IngestOutcome> {
    let mut info = prepare_file(path, fc.root, fc.config, fc.run).await?;

    let Some(op) = fc.operator else {
        // --dry-run --offline: no catalog, no store. Every candidate is
        // "would observe" and "would upload".
        return Ok(IngestOutcome::Observed(Box::new(info), BlobState::Uploaded));
    };

    let mut attempts_left = SOURCE_CHANGED_RETRIES;
    loop {
        let hash = info
            .content_hash
            .clone()
            .expect("prepare_file always sets the content hash");
        let relative_path = info
            .relative_path
            .clone()
            .expect("prepare_file always sets the relative path");
        let decision = state::decide(fc.state, &relative_path, &hash.0);

        // Existing blobs are verified (size, and SHA-256 metadata when present),
        // not merely checked for existence. A mismatch is a per-file error,
        // never an overwrite.
        let blob = match upload::verify_existing(op, &hash, info.size_bytes).await? {
            upload::BlobVerdict::Verified | upload::BlobVerdict::Legacy => BlobState::Existing,
            upload::BlobVerdict::Missing if !fc.mode.writes() => BlobState::Uploaded,
            upload::BlobVerdict::Missing => {
                match upload::upload_verified(op, path, &hash, info.size_bytes).await {
                    Ok(upload::UploadOutcome::Uploaded { .. }) => BlobState::Uploaded,
                    Ok(upload::UploadOutcome::AlreadyExists) => BlobState::Existing,
                    Err(e) if e.is::<upload::SourceChanged>() && attempts_left > 0 => {
                        attempts_left -= 1;
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Source changed during upload; rescanning and retrying"
                        );
                        info = prepare_file(path, fc.root, fc.config, fc.run).await?;
                        continue;
                    }
                    Err(e) => {
                        return Err(e).with_context(|| {
                            format!(
                                "upload failed after {} retr{}",
                                SOURCE_CHANGED_RETRIES,
                                if SOURCE_CHANGED_RETRIES == 1 {
                                    "y"
                                } else {
                                    "ies"
                                }
                            )
                        });
                    }
                }
            }
        };

        return Ok(match decision {
            state::PathDecision::Unchanged => {
                if blob == BlobState::Uploaded && fc.mode.writes() {
                    tracing::warn!(
                        path = %relative_path,
                        "Blob for an unchanged path was missing from the store; restored"
                    );
                }
                IngestOutcome::Unchanged(blob)
            }
            state::PathDecision::Observe => {
                if fc.mode.writes() {
                    info.ingested_at = Some(Utc::now());
                }
                IngestOutcome::Observed(Box::new(info), blob)
            }
        });
    }
}

/// Scan and hash one file and attach its observation identity. Pure with
/// respect to the store; safe to call again after a `SourceChanged` retry.
async fn prepare_file(
    path: &Path,
    root_path: &Path,
    config: &LakehouseConfig,
    ctx: &RunContext,
) -> Result<FileInfo> {
    // 1. Scan file for initial metadata
    let mut info = scan_file(path).await?;

    // 2. Path identity relative to the ingest root
    let relative = path.strip_prefix(root_path).unwrap_or(path);
    if let Some(parent) = relative.parent() {
        info = info.with_parent_dir(parent.to_string_lossy().to_string());
    }
    let relative_path = normalize_relative_path(relative);

    // 3. Content hash (scan_file only hashes small files inline)
    if info.content_hash.is_none() {
        let hash_path = path.to_path_buf();
        let h = tokio::task::spawn_blocking(move || file_hash::full_sha256(&hash_path))
            .await
            .context("Blocking hash task panicked")??;
        info.content_hash = Some(ContentHash::new(h));
    }
    let content_hash = info.content_hash.clone().expect("content hash set above");

    info.object_uri = Some(format!(
        "s3://{}/{}",
        config.bucket,
        content_hash.to_object_key()
    ));

    // 4. Observation identity; derives the deterministic row id from the hash.
    Ok(info.with_observation(
        ctx.source_id.clone(),
        relative_path,
        ctx.run_id,
        ObservationStatus::Present,
        Utc::now(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{IngestArgs, IngestOutputFormat};
    use std::path::PathBuf;

    fn default_args(path: PathBuf) -> IngestArgs {
        IngestArgs {
            path,
            include: vec![],
            exclude: vec![],
            types: vec![],
            max_size: None,
            limit: None,
            source: None,
            dry_run: true,
            offline: true,
            format: IngestOutputFormat::Human,
        }
    }

    fn finalize(
        commit_result: Option<Result<()>>,
        uploaded_count: u64,
        exists_count: u64,
        errors: &[String],
        total_bytes: u64,
        mode: IngestMode,
    ) -> Result<()> {
        finalize_with_format(
            commit_result,
            uploaded_count,
            exists_count,
            errors,
            total_bytes,
            mode,
            IngestOutputFormat::Human,
        )
    }

    fn finalize_with_format(
        commit_result: Option<Result<()>>,
        uploaded_count: u64,
        exists_count: u64,
        errors: &[String],
        total_bytes: u64,
        mode: IngestMode,
        format: IngestOutputFormat,
    ) -> Result<()> {
        let candidates = uploaded_count + exists_count + errors.len() as u64;
        let catalog_commit = catalog_commit_from_result(&commit_result);
        finalize_ingest(
            IngestSummary::build(
                mode,
                Uuid::nil(),
                "/test".to_string(),
                IngestCounts {
                    candidates,
                    observed: uploaded_count,
                    unchanged: exists_count,
                    uploaded: uploaded_count,
                    already_exists: exists_count,
                    bytes: total_bytes,
                },
                errors,
                catalog_commit,
            ),
            commit_result,
            format,
        )
    }

    // ── collect_files ──

    fn make_test_tree(dir: &std::path::Path) {
        std::fs::write(dir.join("photo.jpg"), b"fake jpg").unwrap();
        std::fs::write(dir.join("readme.txt"), b"hello world").unwrap();
        std::fs::write(dir.join("data.csv"), b"a,b,c").unwrap();
        std::fs::write(dir.join("debug.log"), b"log line").unwrap();
        let sub = dir.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("nested.rs"), b"fn main() {}").unwrap();
    }

    #[test]
    fn collect_files_no_filters() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let args = default_args(dir.path().to_path_buf());

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn collect_files_exclude_pattern() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.exclude = vec!["*.log".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 4);
        assert!(!files.iter().any(|f| f.to_string_lossy().contains(".log")));
    }

    #[test]
    fn collect_files_include_pattern() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["*.txt".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("readme.txt"));
    }

    #[test]
    fn collect_files_limit() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.limit = Some(2);

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn collect_files_max_size() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("small.txt"), b"hi").unwrap();
        std::fs::write(dir.path().join("big.txt"), vec![0u8; 2048]).unwrap();
        let mut args = default_args(dir.path().to_path_buf());
        args.max_size = Some(1024);

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("small.txt"));
    }

    #[test]
    fn collect_files_type_filter_by_category() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.types = vec!["image".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("photo.jpg"));
    }

    #[test]
    fn collect_files_type_filter_by_extension() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.types = vec!["rs".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("nested.rs"));
    }

    #[test]
    fn collect_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let args = default_args(dir.path().to_path_buf());
        let files = collect_files(dir.path(), &args).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn collect_files_combined_filters() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.exclude = vec!["*.rs".to_string()];
        args.limit = Some(3);

        let files = collect_files(dir.path(), &args).unwrap();
        assert!(files.len() <= 3);
        assert!(!files.iter().any(|f| f.to_string_lossy().ends_with(".rs")));
    }

    #[test]
    fn collect_files_include_glob_star() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["*.jpg".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("photo.jpg"));
    }

    #[test]
    fn collect_files_exclude_glob_star() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.exclude = vec!["*.rs".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 4);
        assert!(!files.iter().any(|f| f.to_string_lossy().ends_with(".rs")));
    }

    #[test]
    fn collect_files_include_matches_nested_filename() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["*.rs".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("nested.rs"));
    }

    #[test]
    fn collect_files_glob_does_not_substring_match() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["files".to_string()];

        let files = collect_files(dir.path(), &args).unwrap();
        assert!(
            files.is_empty(),
            "bare word should not substring-match filenames"
        );
    }

    #[test]
    fn collect_files_invalid_glob_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        make_test_tree(dir.path());
        let mut args = default_args(dir.path().to_path_buf());
        args.include = vec!["[invalid".to_string()];

        let result = collect_files(dir.path(), &args);
        assert!(result.is_err());
    }

    // ── IngestOutcome tally tests ──

    fn make_test_info(name: &str, size: u64) -> FileInfo {
        let mut info = FileInfo::new(
            std::path::PathBuf::from(format!("/test/{}", name)),
            name.to_string(),
            ".txt".to_string(),
            size,
            None,
            None,
        );
        info.ingested_at = Some(chrono::Utc::now());
        info
    }

    fn tally(outcomes: Vec<IngestOutcome>) -> Tally {
        let mut t = Tally::default();
        for o in outcomes {
            t.record(o);
        }
        t
    }

    #[test]
    fn tally_counts_both_axes_independently() {
        let t = tally(vec![
            // new path, new blob
            IngestOutcome::Observed(Box::new(make_test_info("a.txt", 100)), BlobState::Uploaded),
            // new path, blob already there (identical bytes elsewhere)
            IngestOutcome::Observed(Box::new(make_test_info("b.txt", 200)), BlobState::Existing),
            // unchanged path, blob there
            IngestOutcome::Unchanged(BlobState::Existing),
        ]);
        assert_eq!(t.counts.observed, 2);
        assert_eq!(t.counts.unchanged, 1);
        assert_eq!(t.counts.uploaded, 1);
        assert_eq!(t.counts.already_exists, 2);
        // Only uploaded bytes count; b.txt's blob was not transferred.
        assert_eq!(t.counts.bytes, 100);
        assert_eq!(t.observations.len(), 2);
    }

    #[test]
    fn tally_unchanged_paths_commit_nothing() {
        let t = tally(vec![
            IngestOutcome::Unchanged(BlobState::Existing),
            IngestOutcome::Unchanged(BlobState::Existing),
            IngestOutcome::Unchanged(BlobState::Existing),
        ]);
        assert_eq!(t.counts.unchanged, 3);
        assert_eq!(t.counts.already_exists, 3);
        assert_eq!(t.counts.observed, 0);
        assert_eq!(t.counts.uploaded, 0);
        assert!(t.observations.is_empty());
    }

    #[test]
    fn tally_restored_blob_for_unchanged_path_counts_as_upload_without_row() {
        let t = tally(vec![IngestOutcome::Unchanged(BlobState::Uploaded)]);
        assert_eq!(t.counts.unchanged, 1);
        assert_eq!(t.counts.uploaded, 1);
        assert!(t.observations.is_empty());
    }

    #[test]
    fn tally_candidates_is_set_by_caller_not_by_record() {
        let t = tally(vec![IngestOutcome::Unchanged(BlobState::Existing)]);
        assert_eq!(t.counts.candidates, 0);
    }

    // ── RunContext ──

    #[test]
    fn run_context_defaults_source_id_to_root_path() {
        let ctx = RunContext::new(Path::new("/data/downloads"), None);
        assert_eq!(ctx.source_id, "/data/downloads");
    }

    #[test]
    fn run_context_uses_source_override() {
        let ctx = RunContext::new(Path::new("/data/downloads"), Some("downloads"));
        assert_eq!(ctx.source_id, "downloads");
    }

    #[test]
    fn run_context_generates_distinct_run_ids() {
        let a = RunContext::new(Path::new("/x"), None);
        let b = RunContext::new(Path::new("/x"), None);
        assert_ne!(a.run_id, b.run_id);
    }

    // ── finalize_ingest tests ──

    #[test]
    fn finalize_commit_success() {
        let result = finalize(Some(Ok(())), 3, 1, &[], 1024, IngestMode::Ingest);
        assert!(result.is_ok());
    }

    #[test]
    fn finalize_commit_failure() {
        let err = anyhow::anyhow!("catalog connection refused");
        let result = finalize(Some(Err(err)), 3, 0, &[], 1024, IngestMode::Ingest);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata commit"));
    }

    #[test]
    fn finalize_no_commit() {
        let result = finalize(None, 0, 5, &[], 0, IngestMode::Ingest);
        assert!(result.is_ok());
    }

    #[test]
    fn finalize_partial_processing_failure() {
        let errors = vec!["unreadable.txt: permission denied".to_string()];
        let result = finalize(Some(Ok(())), 2, 0, &errors, 1024, IngestMode::Ingest);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest incomplete: 1 file(s) failed"));
    }

    #[test]
    fn finalize_complete_processing_failure() {
        let errors = vec![
            "unreadable-a.txt: permission denied".to_string(),
            "unreadable-b.txt: permission denied".to_string(),
        ];
        let result = finalize(None, 0, 0, &errors, 0, IngestMode::Ingest);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest incomplete: 2 file(s) failed"));
    }

    #[test]
    fn finalize_dry_run_processing_failure() {
        let errors = vec!["unreadable.txt: permission denied".to_string()];
        let result = finalize(None, 0, 0, &errors, 0, IngestMode::DryRunOffline);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }

    #[test]
    fn finalize_json_commit_failure_still_exits_nonzero() {
        let err = anyhow::anyhow!("catalog connection refused");
        let result = finalize_with_format(
            Some(Err(err)),
            3,
            0,
            &[],
            1024,
            IngestMode::Ingest,
            IngestOutputFormat::Json,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("metadata commit"));
    }

    #[test]
    fn finalize_connected_dry_run_processing_failure() {
        let errors = vec!["unreadable.txt: permission denied".to_string()];
        let result = finalize(None, 1, 2, &errors, 0, IngestMode::DryRun);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }

    #[test]
    fn finalize_connected_dry_run_success_without_commit_is_ok() {
        let result = finalize(None, 1, 2, &[], 12, IngestMode::DryRun);
        assert!(result.is_ok());
    }
}
