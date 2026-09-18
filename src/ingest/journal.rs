//! Durable ingest-run journal (ADR-009 slice 4a).
//!
//! One JSON object per run at `_runs/<run_id>.json` in the data bucket,
//! rewritten with its full history on every transition. Only runs that write
//! (ingest mode) are journaled; previews change nothing.
//!
//! States are written as they happen. `interrupted` is never written: a
//! journal whose last entry is not terminal *is* an interrupted run. That is
//! the only honest record a killed process can leave. A later run for the
//! same source closes such journals with `reconciled` (rows actually found in
//! the catalog for that `run_id`) and `superseded_by`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use opendal::Operator;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Reserved control prefix; never a content-addressed key.
pub const JOURNAL_PREFIX: &str = "_runs/";
pub const JOURNAL_FORMAT_VERSION: u32 = 1;

pub fn journal_key(run_id: Uuid) -> String {
    format!("{JOURNAL_PREFIX}{run_id}.json")
}

/// One lifecycle transition. Terminal states end the run's own writing;
/// closure states are appended by a *later* run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunState {
    /// Connectivity and catalog state established; files are being processed.
    Started,
    /// A batch commit is about to be attempted.
    Committing { batch: u64 },
    /// The catalog accepted the batch.
    BatchCommitted { batch: u64, rows: u64 },
    /// Terminal: every candidate processed, every batch committed.
    Completed,
    /// Terminal: batches committed, but `failed` files had per-file errors.
    Incomplete { failed: u64 },
    /// Terminal: the commit of `batch` failed and stopped the run.
    CommitFailed { batch: u64 },
    /// Closure by a later run: rows for this `run_id` found in the catalog.
    Reconciled { rows_in_catalog: u64 },
    /// Closure by a later run for the same source that completed.
    SupersededBy { run_id: Uuid },
}

impl RunState {
    pub fn is_closure(&self) -> bool {
        matches!(self, Self::Reconciled { .. } | Self::SupersededBy { .. })
    }

    /// Short label for tables and logs.
    pub fn label(&self) -> String {
        match self {
            Self::Started => "started".into(),
            Self::Committing { batch } => format!("committing batch {batch}"),
            Self::BatchCommitted { batch, rows } => {
                format!("batch {batch} committed ({rows} rows)")
            }
            Self::Completed => "completed".into(),
            Self::Incomplete { failed } => format!("incomplete ({failed} failed)"),
            Self::CommitFailed { batch } => format!("commit failed at batch {batch}"),
            Self::Reconciled { rows_in_catalog } => {
                format!("reconciled ({rows_in_catalog} rows in catalog)")
            }
            Self::SupersededBy { run_id } => format!("superseded by {run_id}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub state: RunState,
}

/// What the journal says about a run as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Completed,
    Incomplete,
    CommitFailed,
    /// Last own entry is not terminal and no later run has closed it.
    Interrupted,
    /// Was interrupted; a later run recorded what the catalog holds.
    Reconciled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunJournal {
    pub format_version: u32,
    pub run_id: Uuid,
    pub source_id: String,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub candidates: u64,
    pub history: Vec<Transition>,
}

impl RunJournal {
    pub fn new(run_id: Uuid, source_id: String, candidates: u64, now: DateTime<Utc>) -> Self {
        Self {
            format_version: JOURNAL_FORMAT_VERSION,
            run_id,
            source_id,
            started_at: now,
            updated_at: now,
            candidates,
            history: vec![Transition {
                at: now,
                state: RunState::Started,
            }],
        }
    }

    pub fn push(&mut self, state: RunState, now: DateTime<Utc>) {
        self.updated_at = now;
        self.history.push(Transition { at: now, state });
    }

    /// Last entry written by the run itself (closure entries excluded).
    pub fn last_own(&self) -> &RunState {
        self.history
            .iter()
            .rev()
            .map(|t| &t.state)
            .find(|s| !s.is_closure())
            .unwrap_or(&RunState::Started)
    }

    pub fn outcome(&self) -> RunOutcome {
        match self.last_own() {
            RunState::Completed => RunOutcome::Completed,
            RunState::Incomplete { .. } => RunOutcome::Incomplete,
            RunState::CommitFailed { .. } => RunOutcome::CommitFailed,
            _ if self.is_closed() => RunOutcome::Reconciled,
            _ => RunOutcome::Interrupted,
        }
    }

    /// Interrupted and not yet closed by a later run.
    pub fn is_open(&self) -> bool {
        self.outcome() == RunOutcome::Interrupted
    }

    fn is_closed(&self) -> bool {
        self.history.iter().any(|t| t.state.is_closure())
    }

    /// Rows the run itself recorded as committed.
    pub fn rows_committed(&self) -> u64 {
        self.history
            .iter()
            .filter_map(|t| match t.state {
                RunState::BatchCommitted { rows, .. } => Some(rows),
                _ => None,
            })
            .sum()
    }

    pub fn batches_committed(&self) -> u64 {
        self.history
            .iter()
            .filter(|t| matches!(t.state, RunState::BatchCommitted { .. }))
            .count() as u64
    }
}

// ==================== Storage ====================

/// Owns one run's journal and persists every transition.
pub struct JournalWriter {
    op: Operator,
    journal: RunJournal,
}

impl JournalWriter {
    /// Write the `started` entry. Fails closed: a run that cannot record its
    /// start must not start.
    pub async fn start(
        op: Operator,
        run_id: Uuid,
        source_id: String,
        candidates: u64,
    ) -> Result<Self> {
        let journal = RunJournal::new(run_id, source_id, candidates, Utc::now());
        save(&op, &journal)
            .await
            .context("Cannot write the run journal; refusing to start an unrecorded run")?;
        Ok(Self { op, journal })
    }

    /// Append and persist. A failed write here is logged, not fatal: the
    /// journal then under-reports progress, which reads as "interrupted",
    /// the safe direction.
    pub async fn record(&mut self, state: RunState) {
        self.journal.push(state.clone(), Utc::now());
        if let Err(e) = save(&self.op, &self.journal).await {
            tracing::warn!(
                run_id = %self.journal.run_id,
                state = %state.label(),
                error = %e,
                "Could not persist run journal transition"
            );
        }
    }
}

async fn save(op: &Operator, journal: &RunJournal) -> Result<()> {
    let body = serde_json::to_vec_pretty(journal).context("Failed to serialize run journal")?;
    op.write(&journal_key(journal.run_id), body)
        .await
        .with_context(|| format!("Failed to write {}", journal_key(journal.run_id)))?;
    Ok(())
}

pub async fn load(op: &Operator, run_id: Uuid) -> Result<RunJournal> {
    let key = journal_key(run_id);
    let bytes = op
        .read(&key)
        .await
        .with_context(|| format!("No journal for run {run_id}"))?;
    serde_json::from_slice(&bytes.to_vec()).with_context(|| format!("Malformed journal {key}"))
}

/// Every journal, newest first. Unreadable entries are skipped with a
/// warning so one bad object cannot hide the rest.
pub async fn list(op: &Operator) -> Result<Vec<RunJournal>> {
    let entries = op
        .list(JOURNAL_PREFIX)
        .await
        .context("Failed to list run journals")?;
    let mut journals = Vec::new();
    for entry in entries {
        let Some(id) = entry
            .name()
            .strip_suffix(".json")
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        match load(op, id).await {
            Ok(j) => journals.push(j),
            Err(e) => {
                tracing::warn!(key = %entry.path(), error = %e, "Skipping unreadable journal")
            }
        }
    }
    journals.sort_by_key(|j| std::cmp::Reverse(j.started_at));
    Ok(journals)
}

/// Open (interrupted, unclosed) runs for one source.
pub async fn open_runs_for(op: &Operator, source_id: &str) -> Result<Vec<RunJournal>> {
    Ok(list(op)
        .await?
        .into_iter()
        .filter(|j| j.source_id == source_id && j.is_open())
        .collect())
}

/// Append a closure entry to another run's journal.
pub async fn close(op: &Operator, journal: &mut RunJournal, state: RunState) -> Result<()> {
    debug_assert!(state.is_closure());
    journal.push(state, Utc::now());
    save(op, journal).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::create_memory_operator;

    fn t0() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-18T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn fresh_journal_is_interrupted_until_a_terminal_state() {
        let mut j = RunJournal::new(Uuid::nil(), "src".into(), 10, t0());
        assert_eq!(j.outcome(), RunOutcome::Interrupted);
        assert!(j.is_open());

        j.push(RunState::Committing { batch: 1 }, t0());
        assert_eq!(j.outcome(), RunOutcome::Interrupted);
        j.push(RunState::BatchCommitted { batch: 1, rows: 4 }, t0());
        assert_eq!(j.outcome(), RunOutcome::Interrupted, "still running");
        assert_eq!(j.rows_committed(), 4);

        j.push(RunState::Completed, t0());
        assert_eq!(j.outcome(), RunOutcome::Completed);
        assert!(!j.is_open());
    }

    #[test]
    fn terminal_states_map_to_outcomes() {
        for (state, outcome) in [
            (RunState::Completed, RunOutcome::Completed),
            (RunState::Incomplete { failed: 2 }, RunOutcome::Incomplete),
            (
                RunState::CommitFailed { batch: 3 },
                RunOutcome::CommitFailed,
            ),
        ] {
            let mut j = RunJournal::new(Uuid::nil(), "src".into(), 1, t0());
            j.push(state, t0());
            assert_eq!(j.outcome(), outcome);
            assert!(!j.is_open());
        }
    }

    #[test]
    fn closure_entries_turn_interrupted_into_reconciled_without_hiding_last_own_state() {
        let mut j = RunJournal::new(Uuid::nil(), "src".into(), 10, t0());
        j.push(RunState::BatchCommitted { batch: 1, rows: 3 }, t0());
        j.push(RunState::Committing { batch: 2 }, t0());
        // Killed here. A later run closes it.
        j.push(RunState::Reconciled { rows_in_catalog: 6 }, t0());
        assert_eq!(j.outcome(), RunOutcome::Reconciled);
        assert!(!j.is_open());
        // Batch 2 did land (6 > 3); the own history still says where it died.
        assert_eq!(j.last_own(), &RunState::Committing { batch: 2 });
        assert_eq!(j.rows_committed(), 3);
        assert_eq!(j.batches_committed(), 1);

        j.push(
            RunState::SupersededBy {
                run_id: Uuid::from_u128(7),
            },
            t0(),
        );
        assert_eq!(j.outcome(), RunOutcome::Reconciled);
    }

    #[test]
    fn closure_does_not_change_a_terminal_outcome() {
        let mut j = RunJournal::new(Uuid::nil(), "src".into(), 1, t0());
        j.push(RunState::Completed, t0());
        j.push(
            RunState::SupersededBy {
                run_id: Uuid::from_u128(1),
            },
            t0(),
        );
        assert_eq!(j.outcome(), RunOutcome::Completed);
    }

    #[test]
    fn json_is_flat_snake_case_and_round_trips() {
        let mut j = RunJournal::new(Uuid::nil(), "src".into(), 2, t0());
        j.push(RunState::Committing { batch: 1 }, t0());
        j.push(RunState::CommitFailed { batch: 1 }, t0());
        let v = serde_json::to_value(&j).unwrap();
        assert_eq!(v["format_version"], 1);
        assert_eq!(v["history"][0]["state"], "started");
        assert_eq!(v["history"][1]["state"], "committing");
        assert_eq!(v["history"][1]["batch"], 1);
        assert_eq!(v["history"][2]["state"], "commit_failed");
        let back: RunJournal = serde_json::from_value(v).unwrap();
        assert_eq!(back, j);
    }

    #[tokio::test]
    async fn writer_persists_every_transition_and_list_finds_it() {
        let op = create_memory_operator().unwrap();
        let id = Uuid::from_u128(42);
        let mut w = JournalWriter::start(op.clone(), id, "src".into(), 5)
            .await
            .unwrap();
        let stored = load(&op, id).await.unwrap();
        assert_eq!(stored.last_own(), &RunState::Started);

        w.record(RunState::Committing { batch: 1 }).await;
        w.record(RunState::BatchCommitted { batch: 1, rows: 5 })
            .await;
        let stored = load(&op, id).await.unwrap();
        assert_eq!(stored.history.len(), 3);
        assert_eq!(stored.rows_committed(), 5);
        assert!(stored.is_open(), "no terminal state yet");

        w.record(RunState::Completed).await;
        let all = list(&op).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].outcome(), RunOutcome::Completed);
        assert_eq!(
            journal_key(id),
            "_runs/00000000-0000-0000-0000-00000000002a.json"
        );
    }

    #[tokio::test]
    async fn open_runs_for_filters_by_source_and_openness() {
        let op = create_memory_operator().unwrap();
        // Interrupted, source A.
        JournalWriter::start(op.clone(), Uuid::from_u128(1), "A".into(), 1)
            .await
            .unwrap();
        // Completed, source A.
        let mut done = JournalWriter::start(op.clone(), Uuid::from_u128(2), "A".into(), 1)
            .await
            .unwrap();
        done.record(RunState::Completed).await;
        // Interrupted, source B.
        JournalWriter::start(op.clone(), Uuid::from_u128(3), "B".into(), 1)
            .await
            .unwrap();

        let open = open_runs_for(&op, "A").await.unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].run_id, Uuid::from_u128(1));

        // Closing it removes it from the open set.
        let mut j = open.into_iter().next().unwrap();
        close(&op, &mut j, RunState::Reconciled { rows_in_catalog: 0 })
            .await
            .unwrap();
        assert!(open_runs_for(&op, "A").await.unwrap().is_empty());
        assert_eq!(
            load(&op, Uuid::from_u128(1)).await.unwrap().outcome(),
            RunOutcome::Reconciled
        );
    }

    #[tokio::test]
    async fn list_skips_foreign_objects_under_the_prefix() {
        let op = create_memory_operator().unwrap();
        op.write("_runs/README.txt", b"not a journal".to_vec())
            .await
            .unwrap();
        op.write(
            "_runs/00000000-0000-0000-0000-000000000009.json",
            b"{ not json".to_vec(),
        )
        .await
        .unwrap();
        JournalWriter::start(op.clone(), Uuid::from_u128(5), "A".into(), 1)
            .await
            .unwrap();
        let all = list(&op).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].run_id, Uuid::from_u128(5));
    }
}
