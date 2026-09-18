//! Current catalog state per path (ADR-009).
//!
//! `file_catalog` is an append-only observation log, so "what is the last
//! known content of this path in this source" is a reduction: the `present`
//! observation with the greatest `observed_at` per `relative_path`. Ingest
//! computes it once at run start to apply the ADR-009 rule "repeated
//! unchanged ingest appends no observation".
//!
//! Rows committed before the observation columns existed have no
//! `source_id` and are not matched; their paths are re-observed once.

use crate::query::qualified_table_name;
use anyhow::{Context, Result};
use arrow::array::{Array, AsArray, RecordBatch};
use arrow::compute::cast;
use arrow::datatypes::{DataType, TimeUnit};
use chrono::{DateTime, Utc};
use datafusion::prelude::SessionContext;
use datafusion::scalar::ScalarValue;
use std::collections::HashMap;
use uuid::Uuid;

/// Last known content of one path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownPath {
    pub content_hash: String,
    pub observed_at: DateTime<Utc>,
}

/// Latest `present` observation per `relative_path` for one source.
pub type CurrentState = HashMap<String, KnownPath>;

/// One observation row as read from the catalog, before reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationRow {
    pub relative_path: String,
    pub content_hash: String,
    pub observed_at: DateTime<Utc>,
}

/// What to do with a scanned path given the current state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathDecision {
    /// Append a `present` observation: the path is unknown, or its content
    /// differs from the last observation.
    Observe,
    /// The last observation already records this content; append nothing.
    Unchanged,
}

/// Decide for one path. `state` is `None` when the catalog was not read
/// (`--dry-run --offline`), in which case every path is observed.
pub fn decide(
    state: Option<&CurrentState>,
    relative_path: &str,
    content_hash: &str,
) -> PathDecision {
    match state.and_then(|s| s.get(relative_path)) {
        Some(known) if known.content_hash == content_hash => PathDecision::Unchanged,
        _ => PathDecision::Observe,
    }
}

/// Reduce observation rows to the latest per path by `observed_at`.
/// Ties keep the first row seen; they cannot occur for a single writer with
/// microsecond timestamps.
pub fn reduce_latest(rows: impl IntoIterator<Item = ObservationRow>) -> CurrentState {
    let mut state = CurrentState::new();
    for row in rows {
        let candidate = KnownPath {
            content_hash: row.content_hash,
            observed_at: row.observed_at,
        };
        match state.get(&row.relative_path) {
            Some(existing) if existing.observed_at >= candidate.observed_at => {}
            _ => {
                state.insert(row.relative_path, candidate);
            }
        }
    }
    state
}

/// Read every `present` observation for `source_id` and reduce it.
///
/// Fails when the catalog cannot be read; the caller must not guess about
/// unchanged paths without this state.
pub async fn load_current_state(ctx: &SessionContext, source_id: &str) -> Result<CurrentState> {
    let sql = format!(
        "SELECT relative_path, content_hash, observed_at FROM {} \
         WHERE source_id = $1 AND observation_status = 'present' \
         AND relative_path IS NOT NULL AND content_hash IS NOT NULL AND observed_at IS NOT NULL",
        qualified_table_name()
    );
    let batches = ctx
        .sql(&sql)
        .await
        .context("Failed to plan current-state query")?
        .with_param_values(vec![ScalarValue::Utf8(Some(source_id.to_string()))])
        .context("Failed to bind current-state query parameter")?
        .collect()
        .await
        .context("Failed to read current catalog state")?;

    let rows = rows_from_batches(&batches)?;
    tracing::info!(
        source_id = %source_id,
        observations = rows.len(),
        "Loaded catalog state for source"
    );
    Ok(reduce_latest(rows))
}

/// How many observation rows the catalog holds for one run. Used to resolve
/// an interrupted run's unknown commit outcome (ADR-009 slice 4).
///
/// The `source_id` predicate is not redundant: data files written before the
/// observation columns existed have no `run_id`, and iceberg-rust 0.10 cannot
/// null-fill a `FixedSizeBinary(16)` column for them. `source_id = …` prunes
/// those files by their (absent) column statistics before they are read.
pub async fn count_rows_for_run(
    ctx: &SessionContext,
    source_id: &str,
    run_id: Uuid,
) -> Result<u64> {
    // `run_id` is a 16-byte UUID column; compare its hex form to a bound
    // string so neither parameter reaches the SQL text.
    let sql = format!(
        "SELECT COUNT(*) FROM {} WHERE source_id = $1 AND encode(run_id, 'hex') = $2",
        qualified_table_name()
    );
    let batches = ctx
        .sql(&sql)
        .await
        .context("Failed to plan run row-count query")?
        .with_param_values(vec![
            ScalarValue::Utf8(Some(source_id.to_string())),
            ScalarValue::Utf8(Some(run_id.simple().to_string())),
        ])
        .context("Failed to bind run row-count parameter")?
        .collect()
        .await
        .context("Failed to count rows for run")?;
    count_from_batches(&batches)
}

/// Read the single scalar of a `COUNT(*)` result.
pub fn count_from_batches(batches: &[RecordBatch]) -> Result<u64> {
    let batch = batches
        .iter()
        .find(|b| b.num_rows() > 0)
        .context("count query returned no rows")?;
    let col = cast(batch.column(0), &DataType::Int64).context("count column")?;
    let col = col.as_primitive::<arrow::datatypes::Int64Type>();
    u64::try_from(col.value(0)).context("negative count")
}

/// Convert result batches (`relative_path`, `content_hash`, `observed_at`)
/// to rows. Columns are cast to canonical Arrow types first so the exact
/// physical encoding chosen by the reader does not matter.
pub fn rows_from_batches(batches: &[RecordBatch]) -> Result<Vec<ObservationRow>> {
    let mut rows = Vec::new();
    for batch in batches {
        if batch.num_columns() != 3 {
            anyhow::bail!(
                "current-state query returned {} columns, expected 3",
                batch.num_columns()
            );
        }
        let paths = cast(batch.column(0), &DataType::Utf8).context("relative_path column")?;
        let hashes = cast(batch.column(1), &DataType::Utf8).context("content_hash column")?;
        let ats = cast(
            batch.column(2),
            &DataType::Timestamp(TimeUnit::Microsecond, None),
        )
        .context("observed_at column")?;
        let paths = paths.as_string::<i32>();
        let hashes = hashes.as_string::<i32>();
        let ats = ats.as_primitive::<arrow::datatypes::TimestampMicrosecondType>();

        for i in 0..batch.num_rows() {
            if paths.is_null(i) || hashes.is_null(i) || ats.is_null(i) {
                continue;
            }
            let observed_at = DateTime::<Utc>::from_timestamp_micros(ats.value(i))
                .context("observed_at out of range")?;
            rows.push(ObservationRow {
                relative_path: paths.value(i).to_string(),
                content_hash: hashes.value(i).to_string(),
                observed_at,
            });
        }
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{StringArray, TimestampMicrosecondArray};
    use arrow::datatypes::{Field, Schema};
    use chrono::TimeZone;
    use std::sync::Arc;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    fn row(path: &str, hash: &str, secs: i64) -> ObservationRow {
        ObservationRow {
            relative_path: path.to_string(),
            content_hash: hash.to_string(),
            observed_at: at(secs),
        }
    }

    // ── reduce_latest ──

    #[test]
    fn reduce_keeps_latest_observation_per_path() {
        let state = reduce_latest([
            row("a.txt", "h1", 10),
            row("a.txt", "h2", 20),
            row("b.txt", "h3", 5),
        ]);
        assert_eq!(state.len(), 2);
        assert_eq!(state["a.txt"].content_hash, "h2");
        assert_eq!(state["a.txt"].observed_at, at(20));
        assert_eq!(state["b.txt"].content_hash, "h3");
    }

    #[test]
    fn reduce_is_order_independent() {
        let forward = reduce_latest([row("a", "h1", 10), row("a", "h2", 20)]);
        let backward = reduce_latest([row("a", "h2", 20), row("a", "h1", 10)]);
        assert_eq!(forward, backward);
        assert_eq!(forward["a"].content_hash, "h2");
    }

    #[test]
    fn reduce_a_b_a_yields_a() {
        // A path that returns to earlier content: the latest row wins even
        // though its deterministic id equals the first row's id.
        let state = reduce_latest([row("a", "hA", 10), row("a", "hB", 20), row("a", "hA", 30)]);
        assert_eq!(state["a"].content_hash, "hA");
        assert_eq!(state["a"].observed_at, at(30));
    }

    #[test]
    fn reduce_empty_is_empty() {
        assert!(reduce_latest(std::iter::empty()).is_empty());
    }

    // ── decide ──

    fn known(entries: &[(&str, &str)]) -> CurrentState {
        entries
            .iter()
            .map(|(p, h)| {
                (
                    p.to_string(),
                    KnownPath {
                        content_hash: h.to_string(),
                        observed_at: at(1),
                    },
                )
            })
            .collect()
    }

    #[test]
    fn decide_without_state_always_observes() {
        assert_eq!(decide(None, "a", "h1"), PathDecision::Observe);
    }

    #[test]
    fn decide_unknown_path_observes() {
        let state = known(&[("other", "h1")]);
        assert_eq!(decide(Some(&state), "a", "h1"), PathDecision::Observe);
    }

    #[test]
    fn decide_same_hash_is_unchanged() {
        let state = known(&[("a", "h1")]);
        assert_eq!(decide(Some(&state), "a", "h1"), PathDecision::Unchanged);
    }

    #[test]
    fn decide_different_hash_observes() {
        let state = known(&[("a", "h1")]);
        assert_eq!(decide(Some(&state), "a", "h2"), PathDecision::Observe);
    }

    #[test]
    fn decide_same_bytes_at_other_path_observes() {
        // Identical content at a second path is a new observation of that
        // path, regardless of the blob already existing.
        let state = known(&[("a", "h1")]);
        assert_eq!(
            decide(Some(&state), "copy-of-a", "h1"),
            PathDecision::Observe
        );
    }

    // ── rows_from_batches ──

    fn batch(rows: &[(Option<&str>, Option<&str>, Option<i64>)]) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("relative_path", DataType::Utf8, true),
            Field::new("content_hash", DataType::Utf8, true),
            Field::new(
                "observed_at",
                DataType::Timestamp(TimeUnit::Microsecond, Some("+00:00".into())),
                true,
            ),
        ]));
        let paths = StringArray::from(rows.iter().map(|r| r.0).collect::<Vec<_>>());
        let hashes = StringArray::from(rows.iter().map(|r| r.1).collect::<Vec<_>>());
        let ats = TimestampMicrosecondArray::from(rows.iter().map(|r| r.2).collect::<Vec<_>>())
            .with_timezone("+00:00");
        RecordBatch::try_new(
            schema,
            vec![Arc::new(paths), Arc::new(hashes), Arc::new(ats)],
        )
        .unwrap()
    }

    #[test]
    fn rows_from_batches_reads_typed_columns() {
        let b = batch(&[
            (Some("a.txt"), Some("h1"), Some(10_000_000)),
            (Some("b.txt"), Some("h2"), Some(20_000_000)),
        ]);
        let rows = rows_from_batches(&[b]).unwrap();
        assert_eq!(rows, vec![row("a.txt", "h1", 10), row("b.txt", "h2", 20)]);
    }

    #[test]
    fn rows_from_batches_skips_rows_with_nulls() {
        let b = batch(&[
            (Some("a.txt"), None, Some(10_000_000)),
            (None, Some("h2"), Some(20_000_000)),
            (Some("c.txt"), Some("h3"), None),
            (Some("d.txt"), Some("h4"), Some(40_000_000)),
        ]);
        let rows = rows_from_batches(&[b]).unwrap();
        assert_eq!(rows, vec![row("d.txt", "h4", 40)]);
    }

    #[test]
    fn rows_from_batches_rejects_wrong_shape() {
        let schema = Arc::new(Schema::new(vec![Field::new("x", DataType::Utf8, true)]));
        let b = RecordBatch::try_new(schema, vec![Arc::new(StringArray::from(vec!["x"]))]).unwrap();
        let err = rows_from_batches(&[b]).unwrap_err().to_string();
        assert!(err.contains("expected 3"), "{err}");
    }

    #[test]
    fn rows_from_batches_handles_multiple_batches() {
        let rows = rows_from_batches(&[
            batch(&[(Some("a"), Some("h1"), Some(1_000_000))]),
            batch(&[(Some("b"), Some("h2"), Some(2_000_000))]),
        ])
        .unwrap();
        assert_eq!(rows.len(), 2);
    }

    fn count_batch(values: Vec<i64>) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "count(*)",
            DataType::Int64,
            false,
        )]));
        RecordBatch::try_new(
            schema,
            vec![Arc::new(arrow::array::Int64Array::from(values))],
        )
        .unwrap()
    }

    #[test]
    fn count_from_batches_reads_first_non_empty_batch() {
        assert_eq!(
            count_from_batches(&[count_batch(vec![]), count_batch(vec![7])]).unwrap(),
            7
        );
        assert_eq!(count_from_batches(&[count_batch(vec![0])]).unwrap(), 0);
    }

    #[test]
    fn count_from_batches_rejects_empty_and_negative() {
        assert!(count_from_batches(&[]).is_err());
        assert!(count_from_batches(&[count_batch(vec![])]).is_err());
        assert!(count_from_batches(&[count_batch(vec![-1])]).is_err());
    }
}
