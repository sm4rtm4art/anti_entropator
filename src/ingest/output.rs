//! Ingest summary output.
//!
//! Human mode keeps the existing operator report. JSON mode writes one versioned
//! document to stdout so callers can parse counts and outcome without scraping
//! banners.

use anyhow::{Context, Result};
use console::style;
use serde::Serialize;

pub const INGEST_SUMMARY_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestMode {
    DryRun,
    Ingest,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IngestSummary {
    pub format_version: u32,
    pub mode: IngestMode,
    pub status: IngestStatus,
    pub candidates: u64,
    pub uploaded: u64,
    pub already_exists: u64,
    pub failed: u64,
    pub bytes: u64,
    pub catalog_commit: CatalogCommitStatus,
    pub errors: Vec<String>,
}

impl IngestSummary {
    pub fn build(
        dry_run: bool,
        candidates: u64,
        uploaded: u64,
        already_exists: u64,
        bytes: u64,
        errors: &[String],
        catalog_commit: CatalogCommitStatus,
    ) -> Self {
        let failed = errors.len() as u64;
        let status = if catalog_commit == CatalogCommitStatus::Failed {
            IngestStatus::CommitFailed
        } else if failed > 0 {
            IngestStatus::Incomplete
        } else {
            IngestStatus::Success
        };

        Self {
            format_version: INGEST_SUMMARY_FORMAT_VERSION,
            mode: if dry_run {
                IngestMode::DryRun
            } else {
                IngestMode::Ingest
            },
            status,
            candidates,
            uploaded,
            already_exists,
            failed,
            bytes,
            catalog_commit,
            errors: errors.to_vec(),
        }
    }
}

pub fn catalog_commit_from_result(commit_result: &Option<Result<()>>) -> CatalogCommitStatus {
    match commit_result {
        Some(Ok(())) => CatalogCommitStatus::Succeeded,
        Some(Err(_)) => CatalogCommitStatus::Failed,
        None => CatalogCommitStatus::NotAttempted,
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
    if summary.mode == IngestMode::DryRun {
        println!(
            "  Would upload:    {} files ({})",
            summary.uploaded,
            humansize::format_size(summary.bytes, humansize::BINARY)
        );
    } else {
        println!(
            "  Uploaded:        {} files ({})",
            summary.uploaded,
            humansize::format_size(summary.bytes, humansize::BINARY)
        );
    }
    println!("  Already in store: {} files", summary.already_exists);
    println!("  Errors:          {} files", summary.failed);
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
        IngestStatus::Success if summary.mode == IngestMode::DryRun => {
            println!(
                "{}",
                style("  Dry run - no files were uploaded. Remove --dry-run to actually ingest.")
                    .dim()
            );
            println!();
        }
        IngestStatus::Success => {
            println!("{}", style("  Files ingested successfully!").green());
            println!();
            println!("  Next steps:");
            println!("    1. Run `anti_entropator query` to explore your catalog");
            println!("    2. Run `anti_entropator duplicates` to find duplicate files");
            println!();
        }
        IngestStatus::Incomplete if summary.mode == IngestMode::DryRun => {
            println!(
                "{}",
                style("  Dry run - no files were uploaded. Remove --dry-run to actually ingest.")
                    .dim()
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
                style("  Ingest incomplete: metadata commit failed.").red()
            );
            println!("  Objects may have been uploaded but are not registered in the catalog.");
            println!();
        }
    }
}

pub fn outcome_error(summary: &IngestSummary, commit_result: Option<Result<()>>) -> Result<()> {
    match commit_result {
        Some(Err(e)) => Err(e.context("metadata commit failed")),
        _ if summary.failed > 0 => {
            if summary.mode == IngestMode::DryRun {
                Err(anyhow::anyhow!(
                    "ingest preview incomplete: {} file(s) failed",
                    summary.failed
                ))
            } else {
                Err(anyhow::anyhow!(
                    "ingest incomplete: {} file(s) failed",
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

    #[test]
    fn summary_success_status() {
        let summary =
            IngestSummary::build(false, 4, 3, 1, 1024, &[], CatalogCommitStatus::Succeeded);
        assert_eq!(summary.format_version, 1);
        assert_eq!(summary.mode, IngestMode::Ingest);
        assert_eq!(summary.status, IngestStatus::Success);
        assert_eq!(summary.candidates, 4);
        assert_eq!(summary.uploaded, 3);
        assert_eq!(summary.already_exists, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::Succeeded);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn summary_incomplete_from_errors() {
        let errors = sample_errors();
        let summary = IngestSummary::build(
            false,
            3,
            2,
            0,
            1024,
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
        let summary =
            IngestSummary::build(false, 3, 3, 0, 1024, &errors, CatalogCommitStatus::Failed);
        assert_eq!(summary.status, IngestStatus::CommitFailed);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::Failed);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn summary_dry_run_does_not_attempt_commit() {
        let summary =
            IngestSummary::build(true, 1, 1, 0, 12, &[], CatalogCommitStatus::NotAttempted);
        assert_eq!(summary.mode, IngestMode::DryRun);
        assert_eq!(summary.status, IngestStatus::Success);
        assert_eq!(summary.catalog_commit, CatalogCommitStatus::NotAttempted);
    }

    #[test]
    fn catalog_commit_from_result_maps_outcomes() {
        assert_eq!(
            catalog_commit_from_result(&Some(Ok(()))),
            CatalogCommitStatus::Succeeded
        );
        assert_eq!(
            catalog_commit_from_result(&Some(Err(anyhow::anyhow!("catalog down")))),
            CatalogCommitStatus::Failed
        );
        assert_eq!(
            catalog_commit_from_result(&None),
            CatalogCommitStatus::NotAttempted
        );
    }

    #[test]
    fn summary_serializes_stable_snake_case_keys() {
        let summary = IngestSummary::build(
            true,
            2,
            1,
            0,
            8,
            &sample_errors(),
            CatalogCommitStatus::NotAttempted,
        );
        let json = serde_json::to_value(&summary).unwrap();

        assert_eq!(json["format_version"], 1);
        assert_eq!(json["mode"], "dry_run");
        assert_eq!(json["status"], "incomplete");
        assert_eq!(json["candidates"], 2);
        assert_eq!(json["uploaded"], 1);
        assert_eq!(json["already_exists"], 0);
        assert_eq!(json["failed"], 1);
        assert_eq!(json["bytes"], 8);
        assert_eq!(json["catalog_commit"], "not_attempted");
        assert_eq!(json["errors"][0], "unreadable.txt: permission denied");
    }

    #[test]
    fn outcome_error_preserves_commit_failure() {
        let summary = IngestSummary::build(false, 3, 3, 0, 1024, &[], CatalogCommitStatus::Failed);
        let err = outcome_error(
            &summary,
            Some(Err(anyhow::anyhow!("catalog connection refused"))),
        )
        .unwrap_err();
        assert!(err.to_string().contains("metadata commit"));
    }

    #[test]
    fn outcome_error_reports_partial_ingest_failure() {
        let errors = sample_errors();
        let summary = IngestSummary::build(
            false,
            3,
            2,
            0,
            1024,
            &errors,
            CatalogCommitStatus::Succeeded,
        );
        let err = outcome_error(&summary, Some(Ok(()))).unwrap_err();
        assert!(err
            .to_string()
            .contains("ingest incomplete: 1 file(s) failed"));
    }

    #[test]
    fn outcome_error_reports_dry_run_preview_failure() {
        let errors = sample_errors();
        let summary =
            IngestSummary::build(true, 1, 0, 0, 0, &errors, CatalogCommitStatus::NotAttempted);
        let err = outcome_error(&summary, None).unwrap_err();
        assert!(err
            .to_string()
            .contains("ingest preview incomplete: 1 file(s) failed"));
    }

    #[test]
    fn outcome_error_success_is_ok() {
        let summary =
            IngestSummary::build(false, 3, 3, 0, 1024, &[], CatalogCommitStatus::Succeeded);
        assert!(outcome_error(&summary, Some(Ok(()))).is_ok());
    }
}
