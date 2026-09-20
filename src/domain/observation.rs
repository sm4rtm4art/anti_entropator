//! File observation identity (ADR-009).
//!
//! A `file_catalog` row is one observation of one path in one source during one
//! ingest run. This module holds the pieces of that identity that are pure
//! data: the observation status and the deterministic row identifier.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of a file observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    /// The path was present with the recorded content hash.
    Present,
    /// The path was established as absent after a complete source scan.
    Deleted,
}

impl ObservationStatus {
    /// Catalog column value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Deleted => "deleted",
        }
    }
}

impl std::fmt::Display for ObservationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fixed UUIDv5 namespace for observation identifiers. Never change this
/// value: existing rows derive their `id` from it.
const OBSERVATION_NAMESPACE: Uuid = Uuid::from_u128(0x6f2a_1c3e_8d4b_4a57_9e0f_2b7c_5d1a_3e8f);

/// Deterministic row identifier for an observation.
///
/// Two ingest runs that observe the same `(source_id, relative_path)` with the
/// same content and status produce the same `id`, which makes the identifier
/// field a natural idempotency key. `content_hash` is `None` for deleted
/// observations.
pub fn observation_id(
    source_id: &str,
    relative_path: &str,
    content_hash: Option<&str>,
    status: ObservationStatus,
) -> Uuid {
    // Length-prefixed fields so no combination of values can collide by
    // shifting bytes between components.
    let mut name = Vec::with_capacity(
        source_id.len() + relative_path.len() + content_hash.map_or(0, str::len) + 32,
    );
    for part in [
        source_id,
        relative_path,
        content_hash.unwrap_or(""),
        status.as_str(),
    ] {
        name.extend_from_slice(&(part.len() as u64).to_be_bytes());
        name.extend_from_slice(part.as_bytes());
    }
    Uuid::new_v5(&OBSERVATION_NAMESPACE, &name)
}

/// A path component that is not valid UTF-8 cannot become a catalog identity.
///
/// `relative_path` is part of the observation identity, so it is stored as a
/// string byte-for-byte. A lossy conversion would map distinct on-disk names
/// (possible on ext4/xfs; APFS forbids them) to one identity and let two
/// files collide into one observation. Such a file is rejected instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonUtf8Path {
    /// Lossy rendering, for the error message only.
    pub display: String,
}

impl std::fmt::Display for NonUtf8Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "path is not valid UTF-8 and cannot be a catalog identity: {}",
            self.display
        )
    }
}

impl std::error::Error for NonUtf8Path {}

/// Normalize a path relative to the ingest root into the catalog form:
/// `/`-separated, no leading separator. Fails for any component that is not
/// valid UTF-8; valid paths are returned byte-for-byte.
pub fn normalize_relative_path(relative: &std::path::Path) -> Result<String, NonUtf8Path> {
    let mut parts: Vec<&str> = Vec::new();
    for c in relative.components() {
        if let std::path::Component::Normal(s) = c {
            parts.push(s.to_str().ok_or_else(|| NonUtf8Path {
                display: relative.display().to_string(),
            })?);
        }
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn status_strings_are_stable() {
        assert_eq!(ObservationStatus::Present.as_str(), "present");
        assert_eq!(ObservationStatus::Deleted.as_str(), "deleted");
        assert_eq!(ObservationStatus::Present.to_string(), "present");
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&ObservationStatus::Present).unwrap(),
            "\"present\""
        );
        assert_eq!(
            serde_json::from_str::<ObservationStatus>("\"deleted\"").unwrap(),
            ObservationStatus::Deleted
        );
    }

    #[test]
    fn observation_id_is_deterministic() {
        let a = observation_id("/src", "a/b.txt", Some("h1"), ObservationStatus::Present);
        let b = observation_id("/src", "a/b.txt", Some("h1"), ObservationStatus::Present);
        assert_eq!(a, b);
        assert_eq!(a.get_version(), Some(uuid::Version::Sha1));
    }

    #[test]
    fn observation_id_changes_with_each_component() {
        let base = observation_id("/src", "a/b.txt", Some("h1"), ObservationStatus::Present);
        assert_ne!(
            base,
            observation_id("/other", "a/b.txt", Some("h1"), ObservationStatus::Present)
        );
        assert_ne!(
            base,
            observation_id("/src", "a/c.txt", Some("h1"), ObservationStatus::Present)
        );
        assert_ne!(
            base,
            observation_id("/src", "a/b.txt", Some("h2"), ObservationStatus::Present)
        );
        assert_ne!(
            base,
            observation_id("/src", "a/b.txt", Some("h1"), ObservationStatus::Deleted)
        );
        assert_ne!(
            base,
            observation_id("/src", "a/b.txt", None, ObservationStatus::Present)
        );
    }

    #[test]
    fn observation_id_length_prefix_prevents_boundary_shifts() {
        // "ab" + "c" vs "a" + "bc" would collide under plain concatenation.
        let x = observation_id("ab", "c", None, ObservationStatus::Present);
        let y = observation_id("a", "bc", None, ObservationStatus::Present);
        assert_ne!(x, y);
    }

    #[test]
    fn observation_id_is_pinned() {
        // Guards the namespace constant and encoding: changing either would
        // silently break idempotency against rows already committed.
        let id = observation_id(
            "/Users/example/Downloads",
            "photos/2024/img_001.jpg",
            Some("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"),
            ObservationStatus::Present,
        );
        assert_eq!(id.to_string(), "5ee4a915-ef58-5dd4-95c6-932e6411c918");
    }

    #[test]
    fn normalize_relative_path_uses_forward_slashes_and_no_prefix() {
        let n = |p: &str| normalize_relative_path(Path::new(p)).unwrap();
        assert_eq!(n("a/b/c.txt"), "a/b/c.txt");
        assert_eq!(n("./a/b.txt"), "a/b.txt");
        assert_eq!(n("c.txt"), "c.txt");
        assert_eq!(n(""), "");
        // Non-ASCII UTF-8 is an identity like any other: byte-for-byte.
        assert_eq!(n("Übungen/Straße/日本語.txt"), "Übungen/Straße/日本語.txt");
    }

    #[cfg(unix)]
    #[test]
    fn normalize_relative_path_rejects_non_utf8_instead_of_collapsing_it() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        // Two distinct byte sequences that `to_string_lossy` would both turn
        // into "a/f\u{FFFD}.txt" and thus into one identity.
        let a = Path::new(OsStr::from_bytes(b"a/f\xff.txt"));
        let b = Path::new(OsStr::from_bytes(b"a/f\xfe.txt"));
        assert_eq!(
            a.to_string_lossy(),
            b.to_string_lossy(),
            "precondition: lossy conversion collides"
        );
        let ea = normalize_relative_path(a).unwrap_err();
        let eb = normalize_relative_path(b).unwrap_err();
        assert!(ea.to_string().contains("not valid UTF-8"), "{ea}");
        assert!(eb.to_string().contains("not valid UTF-8"), "{eb}");
    }
}
