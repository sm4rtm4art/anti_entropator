//! `runs` command: read-only view of ingest run journals (ADR-009 slice 4a).
//!
//! Reads `_runs/<run_id>.json` objects through the shared OpenDAL operator and
//! reports what each run recorded about itself. It never writes.

use crate::cli::{IngestOutputFormat, RunsArgs, RunsCommand};
use crate::ingest::journal::{self, RunJournal, RunOutcome};
use crate::lakehouse::LakehouseConfig;
use crate::storage;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use comfy_table::{presets::UTF8_FULL, Cell, Color, ContentArrangement, Table};
use serde::Serialize;
use uuid::Uuid;

/// One row of `runs list`; `runs show` adds the history.
#[derive(Debug, Serialize)]
struct RunView {
    run_id: Uuid,
    source_id: String,
    started_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    candidates: u64,
    outcome: RunOutcome,
    last_state: String,
    rows_committed: u64,
    batches_committed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<Vec<journal::Transition>>,
}

impl RunView {
    fn summary(j: &RunJournal) -> Self {
        Self {
            run_id: j.run_id,
            source_id: j.source_id.clone(),
            started_at: j.started_at,
            updated_at: j.updated_at,
            candidates: j.candidates,
            outcome: j.outcome(),
            last_state: j.last_own().label(),
            rows_committed: j.rows_committed(),
            batches_committed: j.batches_committed(),
            history: None,
        }
    }

    fn full(j: &RunJournal) -> Self {
        Self {
            history: Some(j.history.clone()),
            ..Self::summary(j)
        }
    }
}

pub async fn run(args: RunsArgs) -> Result<()> {
    let config = LakehouseConfig::default();
    let op = storage::create_operator(&config)?;

    match args.command {
        RunsCommand::List {
            source,
            open,
            format,
        } => {
            let journals = journal::list(&op).await?;
            let views: Vec<RunView> = journals
                .iter()
                .filter(|j| source.as_deref().is_none_or(|s| j.source_id == s))
                .filter(|j| !open || j.is_open())
                .map(RunView::summary)
                .collect();
            match format {
                IngestOutputFormat::Json => print_json(&views),
                IngestOutputFormat::Human => print_list(&views),
            }
        }
        RunsCommand::Show { run_id, format } => {
            let j = journal::load(&op, run_id)
                .await
                .with_context(|| format!("Run {run_id} has no journal"))?;
            let view = RunView::full(&j);
            match format {
                IngestOutputFormat::Json => print_json(&view),
                IngestOutputFormat::Human => print_show(&view),
            }
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).context("Failed to serialize runs output")?
    );
    Ok(())
}

fn outcome_cell(outcome: RunOutcome) -> Cell {
    let (text, color) = match outcome {
        RunOutcome::Completed => ("completed", Color::Green),
        RunOutcome::Incomplete => ("incomplete", Color::Yellow),
        RunOutcome::CommitFailed => ("commit failed", Color::Red),
        RunOutcome::Interrupted => ("interrupted", Color::Red),
        RunOutcome::Reconciled => ("reconciled", Color::Yellow),
    };
    Cell::new(text).fg(color)
}

fn print_list(views: &[RunView]) -> Result<()> {
    if views.is_empty() {
        println!("  No ingest runs recorded.");
        return Ok(());
    }
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec![
            "Run",
            "Source",
            "Started (UTC)",
            "Outcome",
            "Last state",
            "Rows",
            "Files",
        ]);
    for v in views {
        table.add_row(vec![
            Cell::new(v.run_id),
            Cell::new(&v.source_id),
            Cell::new(v.started_at.format("%Y-%m-%d %H:%M:%S")),
            outcome_cell(v.outcome),
            Cell::new(&v.last_state),
            Cell::new(v.rows_committed),
            Cell::new(v.candidates),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn print_show(v: &RunView) -> Result<()> {
    println!();
    println!("  Run:        {}", v.run_id);
    println!("  Source:     {}", v.source_id);
    println!("  Candidates: {}", v.candidates);
    println!(
        "  Committed:  {} rows in {} batch(es)",
        v.rows_committed, v.batches_committed
    );
    println!("  Outcome:    {}", outcome_cell(v.outcome).content());
    println!("  Last state: {}", v.last_state);
    println!();
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["At (UTC)", "State"]);
    for t in v.history.as_deref().unwrap_or_default() {
        table.add_row(vec![
            Cell::new(t.at.format("%Y-%m-%d %H:%M:%S%.3f")),
            Cell::new(t.state.label()),
        ]);
    }
    println!("{table}");
    if v.outcome == RunOutcome::Interrupted {
        println!();
        println!(
            "  This run never reached a terminal state. Re-run the same ingest: rows it did not"
        );
        println!(
            "  commit are observed again, and the catalog count for this run is recorded here."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::journal::RunState;

    fn journal() -> RunJournal {
        let t = DateTime::parse_from_rfc3339("2026-09-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let mut j = RunJournal::new(Uuid::from_u128(9), "downloads".into(), 12, t);
        j.push(RunState::BatchCommitted { batch: 1, rows: 5 }, t);
        j.push(RunState::Committing { batch: 2 }, t);
        j
    }

    #[test]
    fn summary_view_reports_interrupted_run_without_history() {
        let v = RunView::summary(&journal());
        assert_eq!(v.outcome, RunOutcome::Interrupted);
        assert_eq!(v.last_state, "committing batch 2");
        assert_eq!(v.rows_committed, 5);
        assert_eq!(v.batches_committed, 1);
        let json = serde_json::to_value(&v).unwrap();
        assert_eq!(json["outcome"], "interrupted");
        assert!(json.get("history").is_none(), "list omits history");
    }

    #[test]
    fn full_view_includes_history_in_order() {
        let v = RunView::full(&journal());
        let json = serde_json::to_value(&v).unwrap();
        let h = json["history"].as_array().unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0]["state"], "started");
        assert_eq!(h[1]["state"], "batch_committed");
        assert_eq!(h[1]["rows"], 5);
        assert_eq!(h[2]["state"], "committing");
    }

    #[test]
    fn human_printers_do_not_panic_on_empty_and_full_input() {
        print_list(&[]).unwrap();
        print_list(&[RunView::summary(&journal())]).unwrap();
        print_show(&RunView::full(&journal())).unwrap();
    }
}
