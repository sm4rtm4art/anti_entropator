//! Bounded ingest pipeline (ADR-009 slice 3c).
//!
//! Two stages joined by a bounded channel:
//!
//! ```text
//! files ──► worker pool (≤ concurrency in flight) ──► rows ──► writer (commit every batch_size)
//! ```
//!
//! Workers run [`IngestOutcome`]-producing futures on the tokio runtime via a
//! [`JoinSet`] that never holds more than `concurrency` tasks. Observation
//! rows flow through an `mpsc` channel with capacity `batch_size`, so at most
//! two batches of rows are ever buffered: one in the channel, one being
//! written.
//!
//! A run commits in batches, so a commit failure in batch *k* leaves batches
//! 1..k-1 in the catalog. The writer then raises `stop`: no new file is
//! started, files already in flight finish (their blobs are content-addressed
//! and safe), and their rows are counted as produced but not committed. The
//! caller reports `observed - committed` and `skipped` so nothing is lost
//! silently.

use super::output::{CommitCounts, IngestCounts};
use super::{BlobState, IngestOutcome};
use crate::domain::FileInfo;
use anyhow::Result;
use std::future::Future;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::Instrument;

/// Commits one batch of observation rows to the catalog.
///
/// The pipeline is generic over this so the batch and failure semantics can
/// be tested without a lakehouse; the production impl is
/// [`super::IcebergCommitter`].
pub(crate) trait BatchCommitter {
    async fn commit(&self, rows: Vec<FileInfo>) -> Result<()>;
}

/// Operator-facing bounds. Both are validated non-zero by the CLI.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PipelineConfig {
    /// Files processed at the same time.
    pub concurrency: NonZeroUsize,
    /// Observation rows per Parquet file / snapshot.
    pub batch_size: NonZeroUsize,
}

/// Everything the caller needs to build the summary.
#[derive(Debug)]
pub(crate) struct PipelineReport {
    /// `candidates` is left 0; the caller knows the total.
    pub counts: IngestCounts,
    pub commit: CommitCounts,
    /// Per-file errors, in completion order.
    pub errors: Vec<String>,
    /// The error of the batch that stopped the run, if any.
    pub commit_failure: Option<anyhow::Error>,
}

/// Run `worker` over `files` with bounded concurrency and, when `committer`
/// is `Some`, commit the produced rows in batches. `on_file_done` is called
/// once per finished file, for progress display.
pub(crate) async fn run_pipeline<W, Fut, C>(
    files: Vec<PathBuf>,
    cfg: PipelineConfig,
    worker: W,
    committer: Option<&C>,
    mut on_file_done: impl FnMut(&Path),
) -> PipelineReport
where
    W: Fn(PathBuf) -> Fut,
    Fut: Future<Output = Result<IngestOutcome>> + Send + 'static,
    C: BatchCommitter,
{
    let stop = Arc::new(AtomicBool::new(false));
    let batch_size = cfg.batch_size.get();

    // The channel exists only when rows are going to be committed; preview
    // modes count rows and drop them.
    let (tx, rx) = match committer {
        Some(_) => {
            let (tx, rx) = mpsc::channel::<FileInfo>(batch_size);
            (Some(tx), Some(rx))
        }
        None => (None, None),
    };

    let producer = async {
        let mut counts = IngestCounts::default();
        let mut errors = Vec::new();
        let mut set = JoinSet::new();
        let mut pending = files.into_iter();
        let concurrency = cfg.concurrency.get();

        loop {
            while set.len() < concurrency && !stop.load(Ordering::Acquire) {
                let Some(path) = pending.next() else { break };
                let span = tracing::info_span!("ingest.file", path = %path.display());
                let fut = worker(path.clone()).instrument(span);
                set.spawn(async move { (path, fut.await) });
            }

            let Some(joined) = set.join_next().await else {
                break;
            };
            let (path, result) = match joined {
                Ok(pair) => pair,
                Err(join_err) => {
                    errors.push(format!("worker task failed: {join_err}"));
                    continue;
                }
            };
            on_file_done(&path);

            match result {
                Ok(outcome) => {
                    let row = tally(&mut counts, outcome);
                    if let (Some(row), Some(tx)) = (row, tx.as_ref()) {
                        // Backpressure: blocks while two batches are buffered.
                        // Fails only after the writer stopped; the row is then
                        // one of the `observed - committed`.
                        let _ = tx.send(row).await;
                    }
                }
                Err(e) => errors.push(format!("{}: {e}", path.display())),
            }
        }

        counts.skipped = pending.count() as u64;
        // Dropping `tx` closes the channel; the writer flushes the tail.
        drop(tx);
        (counts, errors)
    };

    let writer = async {
        match (rx, committer) {
            (Some(rx), Some(committer)) => writer_stage(rx, batch_size, committer, &stop).await,
            _ => (CommitCounts::default(), None),
        }
    };

    let ((counts, errors), (commit, commit_failure)) = tokio::join!(producer, writer);

    PipelineReport {
        counts,
        commit,
        errors,
        commit_failure,
    }
}

/// Fold one outcome into the counts; return the row to commit, if any.
fn tally(counts: &mut IngestCounts, outcome: IngestOutcome) -> Option<FileInfo> {
    let (row, blob) = match outcome {
        IngestOutcome::Observed(info, blob) => {
            counts.observed += 1;
            if blob == BlobState::Uploaded {
                counts.bytes += info.size_bytes;
            }
            (Some(*info), blob)
        }
        IngestOutcome::Unchanged(blob) => {
            counts.unchanged += 1;
            (None, blob)
        }
    };
    match blob {
        BlobState::Uploaded => counts.uploaded += 1,
        BlobState::Existing => counts.already_exists += 1,
    }
    row
}

/// Accumulate rows and commit every `batch_size`. On the first failure, raise
/// `stop`, drop the receiver, and return; the producer's later sends fail
/// instead of blocking.
async fn writer_stage<C: BatchCommitter>(
    mut rx: mpsc::Receiver<FileInfo>,
    batch_size: usize,
    committer: &C,
    stop: &AtomicBool,
) -> (CommitCounts, Option<anyhow::Error>) {
    let mut counts = CommitCounts::default();
    let mut batch: Vec<FileInfo> = Vec::with_capacity(batch_size);

    while let Some(row) = rx.recv().await {
        batch.push(row);
        if batch.len() >= batch_size {
            if let Err(e) = flush(&mut batch, committer, &mut counts).await {
                stop.store(true, Ordering::Release);
                return (counts, Some(e));
            }
        }
    }
    if !batch.is_empty() {
        if let Err(e) = flush(&mut batch, committer, &mut counts).await {
            stop.store(true, Ordering::Release);
            return (counts, Some(e));
        }
    }
    (counts, None)
}

async fn flush<C: BatchCommitter>(
    batch: &mut Vec<FileInfo>,
    committer: &C,
    counts: &mut CommitCounts,
) -> Result<()> {
    let rows = std::mem::take(batch);
    let n = rows.len() as u64;
    let index = counts.batches_committed + counts.batches_failed + 1;
    let span = tracing::info_span!("ingest.commit", batch = index, rows = n);
    match committer.commit(rows).instrument(span).await {
        Ok(()) => {
            counts.committed += n;
            counts.batches_committed += 1;
            tracing::info!(batch = index, rows = n, "Batch committed");
            Ok(())
        }
        Err(e) => {
            counts.batches_failed += 1;
            tracing::error!(batch = index, rows = n, error = %e, "Batch commit failed; stopping run");
            Err(e.context(format!("batch {index} ({n} rows)")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::output::CatalogCommitStatus;
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;
    use std::time::Duration;

    fn cfg(concurrency: usize, batch_size: usize) -> PipelineConfig {
        PipelineConfig {
            concurrency: NonZeroUsize::new(concurrency).unwrap(),
            batch_size: NonZeroUsize::new(batch_size).unwrap(),
        }
    }

    fn paths(n: usize) -> Vec<PathBuf> {
        (0..n)
            .map(|i| PathBuf::from(format!("/src/f{i}")))
            .collect()
    }

    fn info(path: &Path, size: u64) -> Box<FileInfo> {
        Box::new(FileInfo::new(
            path.to_path_buf(),
            path.file_name().unwrap().to_string_lossy().into_owned(),
            String::new(),
            size,
            None,
            None,
        ))
    }

    async fn observe_all(path: PathBuf) -> Result<IngestOutcome> {
        Ok(IngestOutcome::Observed(
            info(&path, 10),
            BlobState::Uploaded,
        ))
    }

    /// Records every batch; fails the batch at `fail_at` (1-based).
    struct FakeCommitter {
        batches: Mutex<Vec<usize>>,
        fail_at: Option<usize>,
    }

    impl FakeCommitter {
        fn ok() -> Self {
            Self {
                batches: Mutex::new(vec![]),
                fail_at: None,
            }
        }
        fn failing_at(n: usize) -> Self {
            Self {
                batches: Mutex::new(vec![]),
                fail_at: Some(n),
            }
        }
        fn batches(&self) -> Vec<usize> {
            self.batches.lock().unwrap().clone()
        }
    }

    impl BatchCommitter for FakeCommitter {
        async fn commit(&self, rows: Vec<FileInfo>) -> Result<()> {
            let mut b = self.batches.lock().unwrap();
            b.push(rows.len());
            if self.fail_at == Some(b.len()) {
                anyhow::bail!("catalog unavailable");
            }
            Ok(())
        }
    }

    /// Never called; for preview modes.
    struct NoCommitter;
    impl BatchCommitter for NoCommitter {
        async fn commit(&self, _: Vec<FileInfo>) -> Result<()> {
            panic!("preview modes must not commit");
        }
    }

    #[tokio::test]
    async fn rows_are_committed_in_batches_with_a_partial_tail() {
        let committer = FakeCommitter::ok();
        let report = run_pipeline(paths(7), cfg(2, 3), observe_all, Some(&committer), |_| {}).await;

        assert_eq!(committer.batches(), vec![3, 3, 1]);
        assert_eq!(report.counts.observed, 7);
        assert_eq!(report.counts.uploaded, 7);
        assert_eq!(report.counts.bytes, 70);
        assert_eq!(report.counts.skipped, 0);
        assert_eq!(report.commit.committed, 7);
        assert_eq!(report.commit.batches_committed, 3);
        assert_eq!(report.commit.batches_failed, 0);
        assert!(report.commit_failure.is_none());
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn commit_failure_in_batch_two_keeps_batch_one_and_stops_the_run() {
        // 100 files, batches of 2, batch 2 fails. Batch 1 must be committed,
        // no third commit may be attempted, and the accounting must add up.
        let committer = FakeCommitter::failing_at(2);
        let total = 100;
        let report = run_pipeline(
            paths(total),
            cfg(4, 2),
            observe_all,
            Some(&committer),
            |_| {},
        )
        .await;

        assert_eq!(
            committer.batches(),
            vec![2, 2],
            "no commit after the failure"
        );
        assert_eq!(report.commit.committed, 2);
        assert_eq!(report.commit.batches_committed, 1);
        assert_eq!(report.commit.batches_failed, 1);
        let failure = report.commit_failure.expect("failure surfaced");
        assert!(format!("{failure:#}").contains("batch 2 (2 rows)"));
        assert!(format!("{failure:#}").contains("catalog unavailable"));

        // Rows in the failed batch plus anything in flight are lost; the run
        // stopped before starting the rest.
        let lost = report.counts.observed - report.commit.committed;
        assert!(lost >= 2, "at least the failed batch is uncommitted");
        assert!(
            report.counts.skipped > 0,
            "run must stop early: skipped={}",
            report.counts.skipped
        );
        assert_eq!(
            report.counts.observed + report.counts.skipped,
            total as u64,
            "every candidate is either observed or skipped"
        );
    }

    #[tokio::test]
    async fn concurrency_is_bounded_and_actually_used() {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let worker = {
            let in_flight = in_flight.clone();
            let peak = peak.clone();
            move |path: PathBuf| {
                let in_flight = in_flight.clone();
                let peak = peak.clone();
                async move {
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    Ok(IngestOutcome::Observed(info(&path, 1), BlobState::Existing))
                }
            }
        };
        let committer = FakeCommitter::ok();
        let report = run_pipeline(paths(12), cfg(3, 100), worker, Some(&committer), |_| {}).await;

        let peak = peak.load(Ordering::SeqCst);
        assert!(peak <= 3, "bound violated: peak={peak}");
        assert!(peak >= 2, "no concurrency observed: peak={peak}");
        assert_eq!(report.counts.observed, 12);
        assert_eq!(report.counts.already_exists, 12);
        assert_eq!(committer.batches(), vec![12]);
    }

    #[tokio::test]
    async fn preview_mode_counts_rows_without_a_committer() {
        let report = run_pipeline(
            paths(5),
            cfg(2, 2),
            observe_all,
            None::<&NoCommitter>,
            |_| {},
        )
        .await;
        assert_eq!(report.counts.observed, 5);
        assert_eq!(report.commit, CommitCounts::default());
        assert!(report.commit_failure.is_none());
    }

    #[tokio::test]
    async fn per_file_errors_are_collected_and_do_not_stop_the_run() {
        let worker = |path: PathBuf| async move {
            if path.ends_with("f1") {
                anyhow::bail!("permission denied");
            }
            Ok(IngestOutcome::Unchanged(BlobState::Existing))
        };
        let committer = FakeCommitter::ok();
        let report = run_pipeline(paths(3), cfg(1, 10), worker, Some(&committer), |_| {}).await;

        assert_eq!(
            report.errors,
            vec!["/src/f1: permission denied".to_string()]
        );
        assert_eq!(report.counts.unchanged, 2);
        assert_eq!(report.counts.observed, 0);
        assert!(committer.batches().is_empty(), "no rows, no commit");
        assert_eq!(report.commit.status(), CatalogCommitStatus::NotAttempted);
    }

    #[tokio::test]
    async fn progress_callback_fires_once_per_file() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let committer = FakeCommitter::ok();
        let seen_cb = seen.clone();
        run_pipeline(
            paths(4),
            cfg(2, 10),
            observe_all,
            Some(&committer),
            move |p| {
                seen_cb.lock().unwrap().push(p.to_path_buf());
            },
        )
        .await;
        let mut got = seen.lock().unwrap().clone();
        got.sort();
        assert_eq!(got, paths(4));
    }

    #[test]
    fn tally_counts_both_axes_independently() {
        let mut c = IngestCounts::default();
        let a = tally(
            &mut c,
            IngestOutcome::Observed(info(Path::new("/a"), 100), BlobState::Uploaded),
        );
        let b = tally(
            &mut c,
            IngestOutcome::Observed(info(Path::new("/b"), 200), BlobState::Existing),
        );
        let u = tally(&mut c, IngestOutcome::Unchanged(BlobState::Existing));
        let r = tally(&mut c, IngestOutcome::Unchanged(BlobState::Uploaded));

        assert!(a.is_some() && b.is_some());
        assert!(u.is_none() && r.is_none(), "unchanged paths produce no row");
        assert_eq!(c.observed, 2);
        assert_eq!(c.unchanged, 2);
        assert_eq!(c.uploaded, 2, "includes the restored blob");
        assert_eq!(c.already_exists, 2);
        assert_eq!(c.bytes, 100, "only transferred bytes count");
    }
}
