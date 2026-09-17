//! Mutation-safe content-addressed upload (ADR-009 slice 3a).
//!
//! The CAS invariant is: the bytes stored under `sha256/ab/cd/<hash>` hash to
//! `<hash>`. A file can change between the scan that computed its hash and the
//! upload that stores it, so the upload re-hashes the bytes it actually sends
//! and only lets the object become visible when they match:
//!
//! 1. open an OpenDAL writer on the CAS key with `if_not_exists`;
//! 2. stream the file in bounded chunks, hashing as it goes;
//! 3. at EOF compare hash and length with the expected values;
//!    - match: `close()` materializes the object (single PUT or
//!      `CompleteMultipartUpload`, both conditional);
//!    - mismatch: `abort()`; nothing is materialized and the caller gets
//!      [`SourceChanged`], which is retryable after a rescan.
//!
//! `ConditionNotMatch` on close means another writer stored the same key
//! first. By construction that object has identical bytes, so it is reported
//! as [`UploadOutcome::AlreadyExists`].
//!
//! Existing blobs are verified against the local file with [`verify_existing`]
//! before being trusted: size always, stored SHA-256 metadata when present.

use crate::domain::ContentHash;
use crate::file_hash::to_lower_hex;
use anyhow::{Context, Result};
use opendal::{ErrorKind, Operator};
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::io::AsyncReadExt;

/// User-metadata key holding the object's SHA-256 (lowercase hex).
pub const METADATA_SHA256: &str = "sha256";
/// User-metadata key holding the object's size in bytes.
pub const METADATA_SIZE: &str = "size";

/// Read buffer per iteration. Bounds per-file memory together with
/// [`UPLOAD_CHUNK_BYTES`].
const READ_BUFFER_BYTES: usize = 64 * 1024;
/// Part size handed to OpenDAL. Files smaller than this are a single PUT on
/// close; larger files become a multipart upload with parts of this size.
/// S3-compatible stores require parts of at least 5 MiB.
pub const UPLOAD_CHUNK_BYTES: usize = 8 * 1024 * 1024;

/// Result of a verified upload attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadOutcome {
    /// The object was materialized by this call.
    Uploaded { bytes: u64 },
    /// Another writer materialized identical bytes under this key first.
    AlreadyExists,
}

/// The source file's bytes no longer match the hash the caller expected.
/// Nothing was stored. Rescan the file and retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceChanged {
    pub expected_hash: String,
    pub actual_hash: Option<String>,
    pub expected_len: u64,
    pub actual_len: u64,
}

impl std::fmt::Display for SourceChanged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.actual_hash {
            Some(actual) => write!(
                f,
                "source changed during upload: expected sha256 {} ({} bytes), read {} ({} bytes); nothing stored",
                self.expected_hash, self.expected_len, actual, self.actual_len
            ),
            None => write!(
                f,
                "source changed before upload: expected {} bytes, file is now {} bytes; nothing stored",
                self.expected_len, self.actual_len
            ),
        }
    }
}

impl std::error::Error for SourceChanged {}

/// Stream `path` into the CAS key for `expected`, verifying the bytes on the
/// way. See the module docs for the protocol.
pub async fn upload_verified(
    op: &Operator,
    path: &Path,
    expected: &ContentHash,
    expected_len: u64,
) -> Result<UploadOutcome> {
    let key = expected.to_object_key();

    // Cheap early exit: the scan recorded the length, so a different length
    // now means the file changed and streaming it would be wasted work.
    let current_len = tokio::fs::metadata(path)
        .await
        .with_context(|| format!("Failed to stat source file {}", path.display()))?
        .len();
    if current_len != expected_len {
        return Err(SourceChanged {
            expected_hash: expected.0.clone(),
            actual_hash: None,
            expected_len,
            actual_len: current_len,
        }
        .into());
    }

    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("Failed to open source file {}", path.display()))?;

    let mut writer_builder = op
        .writer_with(&key)
        .chunk(UPLOAD_CHUNK_BYTES)
        .if_not_exists(true);
    if op.info().full_capability().write_with_user_metadata {
        writer_builder = writer_builder.user_metadata([
            (METADATA_SHA256.to_string(), expected.0.clone()),
            (METADATA_SIZE.to_string(), expected_len.to_string()),
        ]);
    }
    let mut writer = writer_builder
        .await
        .with_context(|| format!("Failed to open storage writer for {key}"))?;

    let mut hasher = Sha256::new();
    let mut actual_len: u64 = 0;
    let mut buf = vec![0u8; READ_BUFFER_BYTES];
    loop {
        let n = match file.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                abort_quietly(&mut writer, &key).await;
                return Err(e).with_context(|| format!("Failed to read {}", path.display()));
            }
        };
        hasher.update(&buf[..n]);
        actual_len += n as u64;
        if let Err(e) = writer.write(buf[..n].to_vec()).await {
            abort_quietly(&mut writer, &key).await;
            return Err(e).with_context(|| format!("Failed to stream {} to {key}", path.display()));
        }
    }

    let actual_hash = to_lower_hex(&hasher.finalize());
    if actual_hash != expected.0 || actual_len != expected_len {
        abort_quietly(&mut writer, &key).await;
        return Err(SourceChanged {
            expected_hash: expected.0.clone(),
            actual_hash: Some(actual_hash),
            expected_len,
            actual_len,
        }
        .into());
    }

    match writer.close().await {
        Ok(_) => Ok(UploadOutcome::Uploaded { bytes: actual_len }),
        Err(e) if e.kind() == ErrorKind::ConditionNotMatch => {
            tracing::debug!(key = %key, "Object materialized concurrently by another writer");
            Ok(UploadOutcome::AlreadyExists)
        }
        Err(e) => Err(e).with_context(|| format!("Failed to finalize upload of {key}")),
    }
}

/// Best-effort abort; the caller's own error is the one worth reporting.
async fn abort_quietly(writer: &mut opendal::Writer, key: &str) {
    if let Err(e) = writer.abort().await {
        tracing::warn!(key = %key, error = %e, "Failed to abort upload; an incomplete multipart upload may remain");
    }
}

/// What [`verify_existing`] found under a CAS key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobVerdict {
    /// No object under the key.
    Missing,
    /// Size and stored SHA-256 metadata both match.
    Verified,
    /// Size matches; the object predates SHA-256 metadata so only size could
    /// be checked.
    Legacy,
}

/// The stored object does not match the local file. The object is left
/// untouched; this is a per-file error, never a silent overwrite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityMismatch {
    pub key: String,
    pub field: &'static str,
    pub stored: String,
    pub expected: String,
}

impl std::fmt::Display for IntegrityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CAS integrity mismatch for {}: stored {} is {}, local file has {}; object left untouched",
            self.key, self.field, self.stored, self.expected
        )
    }
}

impl std::error::Error for IntegrityMismatch {}

/// Check whether the CAS object for `expected` exists and matches the local
/// file's size and, when the object carries it, its SHA-256 metadata.
pub async fn verify_existing(
    op: &Operator,
    expected: &ContentHash,
    expected_len: u64,
) -> Result<BlobVerdict> {
    let key = expected.to_object_key();
    let meta = match op.stat(&key).await {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(BlobVerdict::Missing),
        Err(e) => return Err(e).with_context(|| format!("Failed to stat {key}")),
    };

    if meta.content_length() != expected_len {
        return Err(IntegrityMismatch {
            key,
            field: "size",
            stored: meta.content_length().to_string(),
            expected: expected_len.to_string(),
        }
        .into());
    }

    match meta.user_metadata().and_then(|m| m.get(METADATA_SHA256)) {
        Some(stored) if stored == &expected.0 => Ok(BlobVerdict::Verified),
        Some(stored) => Err(IntegrityMismatch {
            key,
            field: "sha256",
            stored: stored.clone(),
            expected: expected.0.clone(),
        }
        .into()),
        None => Ok(BlobVerdict::Legacy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_hash::full_sha256;
    use crate::storage::create_memory_operator;
    use tempfile::tempdir;

    fn hash_of(bytes: &[u8]) -> ContentHash {
        let mut h = Sha256::new();
        h.update(bytes);
        ContentHash::new(to_lower_hex(&h.finalize()))
    }

    async fn write_temp(dir: &Path, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        tokio::fs::write(&path, bytes).await.unwrap();
        path
    }

    #[tokio::test]
    async fn uploads_when_bytes_match_expected_hash() {
        let op = create_memory_operator().unwrap();
        let dir = tempdir().unwrap();
        let content = b"stable content";
        let path = write_temp(dir.path(), "a.bin", content).await;
        let hash = ContentHash::new(full_sha256(&path).unwrap());

        let outcome = upload_verified(&op, &path, &hash, content.len() as u64)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            UploadOutcome::Uploaded {
                bytes: content.len() as u64
            }
        );
        let stored = op.read(&hash.to_object_key()).await.unwrap().to_vec();
        assert_eq!(stored, content);
    }

    #[tokio::test]
    async fn mutated_bytes_store_nothing_under_either_key() {
        // Scan saw `original`; by upload time the file holds `replaced` of the
        // same length, so the cheap length check cannot catch it.
        let op = create_memory_operator().unwrap();
        let dir = tempdir().unwrap();
        let original = b"version-one";
        let replaced = b"version-two";
        assert_eq!(original.len(), replaced.len());
        let path = write_temp(dir.path(), "m.bin", replaced).await;
        let stale_hash = hash_of(original);

        let err = upload_verified(&op, &path, &stale_hash, original.len() as u64)
            .await
            .unwrap_err();

        let changed = err
            .downcast_ref::<SourceChanged>()
            .expect("error must be SourceChanged");
        assert_eq!(changed.expected_hash, stale_hash.0);
        assert_eq!(
            changed.actual_hash.as_deref(),
            Some(hash_of(replaced).0.as_str())
        );
        assert!(!op.exists(&stale_hash.to_object_key()).await.unwrap());
        assert!(!op.exists(&hash_of(replaced).to_object_key()).await.unwrap());
    }

    #[tokio::test]
    async fn changed_length_fails_before_streaming() {
        let op = create_memory_operator().unwrap();
        let dir = tempdir().unwrap();
        let path = write_temp(dir.path(), "grew.bin", b"now much longer than before").await;
        let stale_hash = hash_of(b"short");

        let err = upload_verified(&op, &path, &stale_hash, 5)
            .await
            .unwrap_err();

        let changed = err.downcast_ref::<SourceChanged>().unwrap();
        assert_eq!(changed.expected_len, 5);
        assert_eq!(changed.actual_len, 27);
        assert!(changed.actual_hash.is_none(), "must not have streamed");
        assert!(!op.exists(&stale_hash.to_object_key()).await.unwrap());
    }

    #[tokio::test]
    async fn concurrent_identical_writer_is_reported_as_exists_and_body_is_untouched() {
        let op = create_memory_operator().unwrap();
        let dir = tempdir().unwrap();
        let content = b"same bytes from two sources";
        let path = write_temp(dir.path(), "dup.bin", content).await;
        let hash = hash_of(content);

        // Simulate the other writer winning the race.
        op.write(&hash.to_object_key(), content.to_vec())
            .await
            .unwrap();

        let outcome = upload_verified(&op, &path, &hash, content.len() as u64)
            .await
            .unwrap();

        assert_eq!(outcome, UploadOutcome::AlreadyExists);
        let stored = op.read(&hash.to_object_key()).await.unwrap().to_vec();
        assert_eq!(stored, content);
    }

    #[tokio::test]
    async fn multi_chunk_file_round_trips() {
        // Larger than the read buffer so the loop runs many iterations; the
        // memory service accepts every chunk as a part.
        let op = create_memory_operator().unwrap();
        let dir = tempdir().unwrap();
        let content: Vec<u8> = (0..(READ_BUFFER_BYTES * 3 + 17))
            .map(|i| (i % 251) as u8)
            .collect();
        let path = write_temp(dir.path(), "big.bin", &content).await;
        let hash = hash_of(&content);

        let outcome = upload_verified(&op, &path, &hash, content.len() as u64)
            .await
            .unwrap();

        assert_eq!(
            outcome,
            UploadOutcome::Uploaded {
                bytes: content.len() as u64
            }
        );
        assert_eq!(
            op.read(&hash.to_object_key()).await.unwrap().to_vec(),
            content
        );
    }

    #[tokio::test]
    async fn verify_reports_missing_when_absent() {
        let op = create_memory_operator().unwrap();
        let verdict = verify_existing(&op, &hash_of(b"nothing"), 7).await.unwrap();
        assert_eq!(verdict, BlobVerdict::Missing);
    }

    #[tokio::test]
    async fn verify_reports_legacy_when_size_matches_without_metadata() {
        let op = create_memory_operator().unwrap();
        let content = b"legacy blob";
        let hash = hash_of(content);
        op.write(&hash.to_object_key(), content.to_vec())
            .await
            .unwrap();

        let verdict = verify_existing(&op, &hash, content.len() as u64)
            .await
            .unwrap();
        assert_eq!(verdict, BlobVerdict::Legacy);
    }

    #[tokio::test]
    async fn verify_rejects_size_mismatch_and_leaves_object_untouched() {
        let op = create_memory_operator().unwrap();
        let stored = b"what is actually stored";
        let hash = hash_of(b"what the local file claims");
        op.write(&hash.to_object_key(), stored.to_vec())
            .await
            .unwrap();

        let err = verify_existing(&op, &hash, 26).await.unwrap_err();

        let mismatch = err.downcast_ref::<IntegrityMismatch>().unwrap();
        assert_eq!(mismatch.field, "size");
        assert_eq!(mismatch.stored, stored.len().to_string());
        assert_eq!(mismatch.expected, "26");
        assert_eq!(
            op.read(&hash.to_object_key()).await.unwrap().to_vec(),
            stored
        );
    }

    #[test]
    fn source_changed_messages_distinguish_pre_and_in_flight_detection() {
        let pre = SourceChanged {
            expected_hash: "abc".into(),
            actual_hash: None,
            expected_len: 5,
            actual_len: 9,
        };
        assert!(pre.to_string().contains("before upload"));
        assert!(pre.to_string().contains("nothing stored"));

        let mid = SourceChanged {
            expected_hash: "abc".into(),
            actual_hash: Some("def".into()),
            expected_len: 5,
            actual_len: 5,
        };
        assert!(mid.to_string().contains("during upload"));
        assert!(mid.to_string().contains("def"));
    }

    #[test]
    fn integrity_mismatch_message_names_key_and_field() {
        let m = IntegrityMismatch {
            key: "sha256/ab/cd/abcd".into(),
            field: "sha256",
            stored: "x".into(),
            expected: "y".into(),
        };
        let s = m.to_string();
        assert!(s.contains("sha256/ab/cd/abcd"));
        assert!(s.contains("left untouched"));
    }
}
