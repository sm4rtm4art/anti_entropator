//! Ingest summary output.
//!
//! Human mode keeps the existing operator report. JSON mode writes one versioned
//! document to stdout so callers can parse counts and outcome without scraping
//! banners.

use anyhow::{Context, Result};
use console::style;
use serde::Serialize;
use uuid::Uuid;

/// Version history:
/// - 1: initial summary.
/// - 2: added `run_id` (ADR-009 run identity).
/// - 3: added `source_id`, `observed`, `unchanged` (ADR-009 observation per
///   path). `uploaded` / `already_exists` keep their blob meaning.
/// - 4: batched commits: added `committed`, `batches_committed`,
///   `batches_failed`, `skipped`.
pub const INGEST_SUMMARY_FORMAT_VERSION: u32 = 4;

/// How an ingest invocation is allowed to interact with the lakehouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestMode {
    /// `--dry-run --offline`: candidate preview with no network and no
    /// existence checks.
    DryRunOffline,
    /// `--dry-run`: connected preview with connectivity and existence checks,
    /// no writes or commits.
    DryRun,
    /// Full ingest: upload new objects and commit catalog metadata.
    Ingest,
}

impl IngestMode {
    /// Map the CLI flags onto a mode.
    ///
    /// clap enforces `--offline` requires `--dry-run`; an `offline` without
    /// `dry_run` is treated as a plain ingest only so the mapping stays total.
    pub fn from_flags(dry_run: bool, offline: bool) -> Self {
        match (dry_run, offline) {
            (false, _) => Self::Ingest,
            (true, true) => Self::DryRunOffline,
            (true, false) => Self::DryRun,
        }
    }

    /// True when the lakehouse is contacted (connectivity + existence checks).
    pub fn is_connected(self) -> bool {
        !matches!(self, Self::DryRunOffline)
    }

    /// True when objects are uploaded and metadata is committed.
    pub fn writes(self) -> bool {
        matches!(self, Self::Ingest)
    }

    /// Operator-facing label for the run header.
    pub fn label(self) -> &'static str {
        match self {
            Self::DryRunOffline => "dry-run, offline (store not checked)",
            Self::DryRun => "dry-run (store checked, nothing written)",
            Self::Ingest => "ingest",
        }
    }

    /// Flags that switch this preview mode off; `None` for a real ingest.
    fn preview_flags(self) -> Option<&'static str> {
        match self {
            Self::DryRunOffline => Some("--dry-run --offline"),
            Self::DryRun => Some("--dry-run"),
            Self::Ingest => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestStatus {
    Success,
    Incomplete,
    CommitFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogCommitStatus {
    NotAttempted,
    Succeeded,
    Failed,
}

/// Per-run tallies on two independent axes (ADR-009):
/// paths (`observed` / `unchanged`) and blobs (`uploaded` / `already_exists`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct IngestCounts {
    /// Files selected by the filters.
    pub candidates: u64,
    /// Paths for which a `present` observation row was (or would be) appended.
    pub observed: u64,
    /// Paths whose last observation already records this content; no row.
    pub unchanged: u64,
    /// Blobs uploaded (or that would be) to the content-addressed store.
    pub uploaded: u64,
    /// Blobs that already existed in the store and were verified.
    pub already_exists: u64,
    /// Bytes of the uploaded blobs.
    pub bytes: u64,
    /// Candidates never started because the run stopped after a commit
    /// failure. Always 0 on a run that did not stop early.
    pub skipped: u64,
}

/// What the writer stage committed. A run commits in batches; the first
/// failed batch stops the run, so `batches_failed` is 0 or 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CommitCounts {
    /// Observation rows whose commit the catalog acknowledged. After a failed
    /// batch, `observed - committed` is an upper bound on missing rows: the
    /// failed batch may have landed server-side; the next run reconciles.
    pub committed: u64,
    /// Batches (one Parquet file and one snapshot each) acknowledged.
    pub batches_committed: u64,
    /// Batches whose commit returned an error (outcome unknown until
    /// reconciled).
    pub batches_failed: u64,
}

impl CommitCounts {
    pub fn status(&self) -> CatalogCommitStatus {
        if self.batches_failed > 0 {
            CatalogCommitStatus::Failed
        } else if self.batches_committed > 0 {
            CatalogCommitStatus::Succeeded
        } else {
            CatalogCommitStatus::NotAttempted
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestSummary {
    pub format_version: u32,
    pub mode: IngestMode,
    /// Identity of this ingest attempt; every row it commits carries it.
    pub run_id: Uuid,
    /// Logical source the observations belong to.
    pub source_id: String,
    pub status: IngestStatus,
    #[serde(flatten)]
    pub counts: IngestCounts,
    pub failed: u64,
    pub catalog_commit: CatalogCommitStatus,
    #[serde(flatten)]
    pub commit: CommitCounts,
    pub errors: Vec<String>,
}

impl IngestSummary {
    pub fn build(
        mode: IngestMode,
        run_id: Uuid,
        source_id: String,
        counts: IngestCounts,
        commit: CommitCounts,
        errors: &[String],
    ) -> Self {
        let failed = errors.len() as u64;
        let catalog_commit = commit.status();
        let status = if catalog_commit == CatalogCommitStatus::Failed {
            IngestStatus::CommitFailed
        } else if failed > 0 {
            IngestStatus::Incomplete
        } else {
            IngestStatus::Success
        };

        Self {
            format_version: INGEST_SUMMARY_FORMAT_VERSION,
            mode,
            run_id,
            source_id,
            status,
            counts,
            failed,
            catalog_commit,
            commit,
            errors: errors.to_vec(),
        }
    }

    /// Rows this run produced but did not get into the catalog.
    pub fn uncommitted_rows(&self) -> u64 {
        self.counts.observed.saturating_sub(self.commit.committed)
    }
}

pub fn print_json_report(summary: &IngestSummary) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(summary).context("Failed to serialize ingest summary")?
    );
    Ok(())
}

pub fn print_human_report(summary: &IngestSummary) {
    println!();
    println!("─── Ingest Results ─────────────────────────────────────────────");
    println!();
    let c = &summary.counts;
    let bytes = humansize::format_size(c.bytes, humansize::BINARY);
    if summary.mode.writes() {
        println!(
            "  Observed:        {} paths (catalog rows appended)",
            c.observed
        );
        println!(
            "  Unchanged:       {} paths (already in catalog)",
            c.unchanged
        );
        println!("  Uploaded:        {} blobs ({bytes})", c.uploaded);
        println!("  Already in store: {} blobs", c.already_exists);
        let k = &summary.commit;
        println!(
            "  Committed:       {} rows in {} batch(es)",
            k.committed, k.batches_committed
        );
        if k.batches_failed > 0 {
            println!(
                "  Commit failed:   batch {}; {} rows not acknowledged",
                k.batches_committed + 1,
                summary.uncommitted_rows()
            );
        }
        if c.skipped > 0 {
            println!("  Not started:     {} files (run stopped)", c.skipped);
        }
    } else if summary.mode.is_connected() {
        println!("  Would observe:   {} paths (catalog rows)", c.observed);
        println!(
            "  Unchanged:       {} paths (already in catalog)",
            c.unchanged
        );
        println!("  Would upload:    {} blobs ({bytes})", c.uploaded);
        println!("  Already in store: {} blobs", c.already_exists);
    } else {
        println!("  Would observe:   {} paths (catalog not read)", c.observed);
        println!("  Unchanged:       not checked (--offline)");
        println!("  Would upload:    {} blobs ({bytes})", c.uploaded);
        println!("  Already in store: not checked (--offline)");
    }
    println!("  Errors:          {} files", summary.failed);
    println!("  Run:             {}", summary.run_id);
    println!("  Source:          {}", summary.source_id);
    println!();

    if !summary.errors.is_empty() {
        println!("  Errors:");
        for err in summary.errors.iter().take(5) {
            println!("    - {}", err);
        }
        if summary.errors.len() > 5 {
            println!("    ... and {} more", summary.errors.len() - 5);
        }
        println!();
    }

    match summary.status {
        IngestStatus::Success | IngestStatus::Incomplete if !summary.mode.writes() => {
            let flags = summary
                .mode
                .preview_flags()
                .expect("preview modes always have flags");
            println!(
                "{}",
                style(format!(
                    "  Dry run - no files were uploaded. Remove {flags} to actually ingest."
                ))
                .dim()
            );
            println!();
        }
        IngestStatus::Success => {
            println!("{}", style("  Files ingested successfully!").green());
            println!();
            println!("  Next steps:");
            println!("    1. Run `anti_entropator query` to explore your catalog");
            println!(
                "    2. Group duplicate content: SELECT content_hash, COUNT(*) FROM files GROUP BY content_hash HAVING COUNT(*) > 1"
            );
            println!();
        }
        IngestStatus::Incomplete => {
            println!(
                "{}",
                style("  Ingest incomplete: one or more files failed.").red()
            );
            println!();
        }
        IngestStatus::CommitFailed => {
            println!(
                "{}",
                style(format!(
                    "  Ingest incomplete: metadata commit failed after {} acknowledged batch(es).",
                    summary.commit.batches_committed
                ))
                .red()
            );
            println!(
                "  Up to {} observed row(s) may be missing from the catalog; {} file(s) were not started.",
                summary.uncommitted_rows(),
                summary.counts.skipped
            );
            println!(
                "  The failed batch may still have landed. Uploaded blobs are safe. Re-run the same"
            );
            println!(
                "  ingest: it records what the catalog holds for this run and observes the rest."
            );
            println!();
        }
    }
}

/// `commit_failure` is the error of the batch that stopped the run, if any.
pub fn outcome_error(summary: &IngestSummary, commit_failure: Option<anyhow::Error>) -> Result<()> {
    match commit_failure {
        Some(e) => Err(e.context("metadata commit failed")),
        _ if summary.failed > 0 => {
            if summary.mode.writes() {
                Err(anyhow::anyhow!(
                    "ingest incomplete: {} file(s) failed",
                    summary.failed
                ))
            } else {
                Err(anyhow::anyhow!(
                    "ingest preview incomplete: {} file(s) failed",
                    summary.failed
                ))
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_errors() -> Vec<String> {
        vec!["unreadable.txt: permission denied".to_string()]
    }

    fn counts(
        candidates: u64,
        observed: u64,
        unchanged: u64,
        uploaded: u64,
        already_exists: u64,
        bytes: u64,
    ) -> IngestCounts {
        IngestCounts {
            candidates,
            observed,
            unchanged,
            uploaded,
            already_exists,
            bytes,
            skipped: 0,
        }
    }

    /// Commit counts that yield the given status: `Succeeded` commits every
    /// observed row in one batch, `Failed` commits nothing and fails batch 1.
    fn commit_for(status: CatalogCommitStatus, observed: u64) -> CommitCounts {
        match status {
            CatalogCommitStatus::Succeeded => CommitCounts {
                committed: observed,
                batches_committed: 1,
                batches_failed: 0,
            },
            CatalogCommitStatus::Failed => CommitCounts {
                committed: 0,
                batches_committed: 0,
                batches_failed: 1,
            },
            CatalogCommitStatus::NotAttempted => CommitCounts::default(),
        }
    }

    fn build(
        mode: IngestMode,
        c: IngestCounts,
        errors: &[String],
        commit: CatalogCommitStatus,
    ) -> IngestSummary {
        IngestSummary::build(
            mode,
            Uuid::nil(),
            "/src".to_string(),
            c,
            commit_for(commit, c.observed),
            errors,
        )
    }

    #[test]
    fn summary_success_status() {
        let summary = build(
            IngestMode::Ingest,
            counts(4, 3, 1, 2, 2, 1024),
            &[],
            CatalogCommitStatus::Succeeded,
        );
        assert_eq!(summary.format_version, 4);
        assert_eq!(summary.mode, IngestMode::Ingest);
        assert_eq!(summary.run_id, Uuid::nil());
        assert_eq!(summary.source_id, "/src");
        assert_eq!(summary.status, IngestStatus::Success);
        assert_eq!(summary.counts.candidates, 4);
        assert_eq!(summary.counts.observed, 3);
        assert_eq!(summary.counts.unchanged, 1);
        assert_eq!(summary.counts.uploaded, 2);
        assert_eq!(summary.counts.already_exists, 2);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::Succeeded);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn summary_incomplete_from_errors() {
        let errors = sample_errors();
        let summary = build(
            IngestMode::Ingest,
            counts(3, 2, 0, 2, 0, 1024),
            &errors,
            CatalogCommitStatus::Succeeded,
        );
        assert_eq!(summary.status, IngestStatus::Incomplete);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.errors, errors);
    }

    #[test]
    fn summary_commit_failed_takes_precedence() {
        let errors = sample_errors();
        let summary = build(
            IngestMode::Ingest,
            counts(3, 3, 0, 3, 0, 1024),
            &errors,
            CatalogCommitStatus::Failed,
        );
        assert_eq!(summary.status, IngestStatus::CommitFailed);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::Failed);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn summary_dry_run_does_not_attempt_commit() {
        let summary = build(
            IngestMode::DryRun,
            counts(1, 1, 0, 1, 0, 12),
            &[],
            CatalogCommitStatus::NotAttempted,
        );
        assert_eq!(summary.mode, IngestMode::DryRun);
        assert_eq!(summary.status, IngestStatus::Success);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::NotAttempted);
    }

    #[test]
    fn commit_counts_status_prefers_failure_over_partial_success() {
        let c = |committed, batches_committed, batches_failed| CommitCounts {
            committed,
            batches_committed,
            batches_failed,
        };
        assert_eq!(c(0, 0, 0).status(), CatalogCommitStatus::NotAttempted);
        assert_eq!(c(10, 2, 0).status(), CatalogCommitStatus::Succeeded);
        // Batch 1 committed, batch 2 failed: the run is a failure even
        // though rows landed.
        assert_eq!(c(1000, 1, 1).status(), CatalogCommitStatus::Failed);
        assert_eq!(c(0, 0, 1).status(), CatalogCommitStatus::Failed);
    }

    #[test]
    fn partial_commit_failure_reports_uncommitted_rows_and_skipped_files() {
        // 3 batches of 2 planned; batch 2 failed. Batch 1 (2 rows) landed,
        // 4 rows were produced but 2 of them are lost, and 3 candidates were
        // never started.
        let mut c = counts(9, 4, 2, 4, 2, 4096);
        c.skipped = 3;
        let summary = IngestSummary::build(
            IngestMode::Ingest,
            Uuid::nil(),
            "/src".to_string(),
            c,
            CommitCounts {
                committed: 2,
                batches_committed: 1,
                batches_failed: 1,
            },
            &[],
        );
        assert_eq!(summary.status, IngestStatus::CommitFailed);
        assert_eq!(summary.uncommitted_rows(), 2);
        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["committed"], 2);
        assert_eq!(json["batches_committed"], 1);
        assert_eq!(json["batches_failed"], 1);
        assert_eq!(json["skipped"], 3);
        assert_eq!(json["catalog_commit"], "failed");
        // Flattened, not nested.
        assert!(json.get("commit").is_none());
    }

    #[test]
    fn summary_serializes_stable_flat_snake_case_keys() {
        let summary = build(
            IngestMode::DryRun,
            counts(2, 1, 1, 1, 0, 8),
            &sample_errors(),
            CatalogCommitStatus::NotAttempted,
        );
        let json = serde_json::to_value(&summary).unwrap();

        assert_eq!(json["format_version"], 4);
        assert_eq!(json["mode"], "dry_run");
        assert_eq!(json["run_id"], "00000000-0000-0000-0000-000000000000");
        assert_eq!(json["source_id"], "/src");
        assert_eq!(json["status"], "incomplete");
        // Counts are flattened: no nested `counts` object.
        assert!(json.get("counts").is_none());
        assert_eq!(json["candidates"], 2);
        assert_eq!(json["observed"], 1);
        assert_eq!(json["unchanged"], 1);
        assert_eq!(json["uploaded"], 1);
        assert_eq!(json["already_exists"], 0);
        assert_eq!(json["failed"], 1);
        assert_eq!(json["bytes"], 8);
        assert_eq!(json["skipped"], 0);
        assert_eq!(json["catalog_commit"], "not_attempted");
        assert_eq!(json["committed"], 0);
        assert_eq!(json["batches_committed"], 0);
        assert_eq!(json["batches_failed"], 0);
        assert_eq!(json["errors"][0], "unreadable.txt: permission denied");
    }

    #[test]
    fn outcome_error_preserves_commit_failure() {
        let summary = build(
            IngestMode::Ingest,
            counts(3, 3, 0, 3, 0, 1024),
            &[],
            CatalogCommitStatus::Failed,
        );
        let err = outcome_error(
            &summary,
            Some(anyhow::anyhow!("catalog connection refused")),
        )
        .unwrap_err();
        assert!(err.to_string().contains("metadata commit"));
        assert!(format!("{err:#}").contains("catalog connection refused"));
    }

    #[test]
    fn outcome_error_reports_partial_ingest_failure() {
        let errors = sample_errors();
        let summary = build(
            IngestMode::Ingest,
            counts(3, 2, 0, 2, 0, 1024),
            &errors,
            CatalogCommitStatus::Succeeded,
        );
        let err = outcome_error(&summary, None).unwrap_err();
        assert!(err
            .to_string()
            .contains("ingest incomplete: 1 file(s) failed"));
    }

    #[test]
    fn outcome_error_reports_dry_run_preview_failure() {
        let errors = sample_errors();
        let summary = build(
            IngestMode::DryRun,
            counts(1, 0, 0, 0, 0, 0),
            &errors,
            CatalogCommitStatus::NotAttempted,
        );
        let err = outcome_error(&summary, None).unwrap_err();
        assert!(err
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }

    #[test]
    fn outcome_error_success_is_ok() {
        let summary = build(
            IngestMode::Ingest,
            counts(3, 3, 0, 3, 0, 1024),
            &[],
            CatalogCommitStatus::Succeeded,
        );
        assert!(outcome_error(&summary, None).is_ok());
    }

    #[test]
    fn unchanged_only_run_is_success_without_commit() {
        // Re-ingest of an unchanged source: nothing observed, nothing
        // uploaded, no commit attempted, still a success.
        let summary = build(
            IngestMode::Ingest,
            counts(3, 0, 3, 0, 3, 0),
            &[],
            CatalogCommitStatus::NotAttempted,
        );
        assert_eq!(summary.status, IngestStatus::Success);
        assert!(outcome_error(&summary, None).is_ok());
    }

    // ── IngestMode ──

    #[test]
    fn mode_from_flags_maps_each_flag() {
        assert_eq!(
            IngestMode::from_flags(true, true),
            IngestMode::DryRunOffline
        );
        assert_eq!(IngestMode::from_flags(true, false), IngestMode::DryRun);
        assert_eq!(IngestMode::from_flags(false, false), IngestMode::Ingest);
        // clap forbids this combination; the mapping still stays total.
        assert_eq!(IngestMode::from_flags(false, true), IngestMode::Ingest);
    }

    #[test]
    fn mode_capabilities_are_ordered() {
        // dry-run --offline: no network, no writes
        assert!(!IngestMode::DryRunOffline.is_connected());
        assert!(!IngestMode::DryRunOffline.writes());
        // dry-run: connected, no writes
        assert!(IngestMode::DryRun.is_connected());
        assert!(!IngestMode::DryRun.writes());
        // ingest: connected and writes
        assert!(IngestMode::Ingest.is_connected());
        assert!(IngestMode::Ingest.writes());
    }

    #[test]
    fn mode_serializes_snake_case_for_every_variant() {
        for (mode, expected) in [
            (IngestMode::DryRunOffline, "dry_run_offline"),
            (IngestMode::DryRun, "dry_run"),
            (IngestMode::Ingest, "ingest"),
        ] {
            let summary = build(
                mode,
                IngestCounts::default(),
                &[],
                CatalogCommitStatus::NotAttempted,
            );
            let json = serde_json::to_value(&summary).unwrap();
            assert_eq!(json["mode"], expected);
        }
    }

    #[test]
    fn connected_dry_run_summary_keeps_both_axes_and_never_commits() {
        let summary = build(
            IngestMode::DryRun,
            counts(3, 2, 1, 1, 2, 12),
            &[],
            CatalogCommitStatus::NotAttempted,
        );
        assert_eq!(summary.mode, IngestMode::DryRun);
        assert_eq!(summary.status, IngestStatus::Success);
        assert_eq!(summary.counts.observed, 2);
        assert_eq!(summary.counts.unchanged, 1);
        assert_eq!(summary.counts.uploaded, 1);
        assert_eq!(summary.counts.already_exists, 2);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::NotAttempted);
    }

    #[test]
    fn outcome_error_reports_offline_preview_failure() {
        let errors = sample_errors();
        let summary = build(
            IngestMode::DryRunOffline,
            counts(1, 0, 0, 0, 0, 0),
            &errors,
            CatalogCommitStatus::NotAttempted,
        );
        let err = outcome_error(&summary, None).unwrap_err();
        assert!(err
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }
}
