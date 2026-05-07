use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Compute SHA-256 of the full file using streaming reads.
pub(crate) fn full_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute SHA-256 of only the first `block_size` bytes.
pub(crate) fn quick_sha256(path: &Path, block_size: usize) -> Result<String> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0u8; block_size];
    let bytes_read = file.read(&mut buffer)?;
    buffer.truncate(bytes_read);

    let mut hasher = Sha256::new();
    hasher.update(&buffer);

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_hash_known_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("known.txt");
        std::fs::write(&path, b"hello world").expect("write test data");

        let hash = full_sha256(&path).expect("hash known content");
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn full_hash_empty_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("empty.bin");
        std::fs::write(&path, b"").expect("write test data");

        let hash = full_sha256(&path).expect("hash empty file");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn full_hash_deterministic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("det.bin");
        std::fs::write(&path, b"reproducible content").expect("write test data");

        let hash1 = full_sha256(&path).expect("first hash");
        let hash2 = full_sha256(&path).expect("second hash");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn full_hash_matches_reference_sha256() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ref.bin");
        let payload = b"same-streaming-input-for-scan-and-ingest";
        std::fs::write(&path, payload).expect("write test data");

        let shared = full_sha256(&path).expect("shared hash");
        let mut hasher = Sha256::new();
        hasher.update(payload);
        let reference = format!("{:x}", hasher.finalize());

        assert_eq!(shared, reference);
    }

    #[test]
    fn quick_hash_deterministic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("quick.bin");
        std::fs::write(&path, b"hello world, this is test content").expect("write test data");

        let h1 = quick_sha256(&path, 1024).expect("first quick hash");
        let h2 = quick_sha256(&path, 1024).expect("second quick hash");
        assert_eq!(h1, h2);
    }

    #[test]
    fn quick_hash_reads_only_first_block() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("large.bin");
        let content = vec![0xABu8; 2048];
        std::fs::write(&path, &content).expect("write test data");

        let block_size = 512;
        let hash = quick_sha256(&path, block_size).expect("quick hash");

        let mut hasher = Sha256::new();
        hasher.update(&content[..block_size]);
        let expected = format!("{:x}", hasher.finalize());

        assert_eq!(hash, expected);
    }
}
