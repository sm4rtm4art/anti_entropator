//! Unified storage I/O boundary via OpenDAL.
//!
//! Every storage operation (read, write, list, head, delete) goes through
//! the [`Operator`] returned by [`create_operator`]. This is the single
//! I/O boundary described in ADR-006.

use crate::lakehouse::LakehouseConfig;
use anyhow::{Context, Result};
use opendal::{services::S3, Operator};

/// Build an OpenDAL [`Operator`] configured for the project's S3-compatible store.
///
/// All modules (`ingest`, `lakehouse`, `query`) should use this factory
/// instead of constructing their own storage clients.
pub fn create_operator(config: &LakehouseConfig) -> Result<Operator> {
    let builder = S3::default()
        .endpoint(&config.s3_endpoint)
        .bucket(&config.bucket)
        .region("us-east-1")
        .access_key_id(&config.s3_access_key)
        .secret_access_key(&config.s3_secret_key);

    let op = Operator::new(builder)
        .context("Failed to build OpenDAL S3 operator")?
        .finish();

    Ok(op)
}

/// Build an OpenDAL [`Operator`] backed by in-memory storage (for tests).
#[cfg(test)]
pub fn create_memory_operator() -> Result<Operator> {
    use opendal::services::Memory;

    let op = Operator::new(Memory::default())
        .context("Failed to build OpenDAL memory operator")?
        .finish();

    Ok(op)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn write_read_roundtrip() {
        let op = create_memory_operator().unwrap();

        op.write("test/hello.txt", "hello world")
            .await
            .expect("write failed");

        let data = op.read("test/hello.txt").await.expect("read failed");
        assert_eq!(data.to_vec(), b"hello world");
    }

    #[tokio::test]
    async fn is_exist_true_and_false() {
        let op = create_memory_operator().unwrap();

        assert!(!op.exists("absent.txt").await.unwrap());

        op.write("present.txt", "data").await.unwrap();
        assert!(op.exists("present.txt").await.unwrap());
    }

    #[tokio::test]
    async fn list_prefix() {
        let op = create_memory_operator().unwrap();

        op.write("prefix/a.txt", "a").await.unwrap();
        op.write("prefix/b.txt", "b").await.unwrap();
        op.write("other/c.txt", "c").await.unwrap();

        let entries: Vec<_> = op
            .list("prefix/")
            .await
            .expect("list failed")
            .into_iter()
            .filter(|e| !e.path().ends_with('/'))
            .collect();

        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn delete_object() {
        let op = create_memory_operator().unwrap();

        op.write("to_delete.txt", "bye").await.unwrap();
        assert!(op.exists("to_delete.txt").await.unwrap());

        op.delete("to_delete.txt").await.expect("delete failed");
        assert!(!op.exists("to_delete.txt").await.unwrap());
    }

    #[test]
    fn create_operator_builds_without_panic() {
        let config = LakehouseConfig::default();
        let op = create_operator(&config);
        assert!(op.is_ok());
    }

    #[test]
    fn create_operator_with_custom_config() {
        let config = LakehouseConfig {
            s3_endpoint: "http://custom:9000".to_string(),
            bucket: "test-bucket".to_string(),
            ..LakehouseConfig::default()
        };
        let op = create_operator(&config);
        assert!(op.is_ok());
    }
}
