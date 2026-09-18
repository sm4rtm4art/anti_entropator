//! CLI Integration Tests
//!
//! Tests the anti_entropator binary as a black box, verifying command-line behavior.

use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

/// Get a Command for the anti_entropator binary
fn cmd() -> Result<Command> {
    #[allow(deprecated)] // cargo_bin works fine for our use case
    Ok(Command::cargo_bin("anti_entropator")?)
}

// ==================== Help & Version Tests ====================

#[test]
fn cli_help_displays_usage() -> Result<()> {
    cmd()?
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("lakehouse"))
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"));
    Ok(())
}

#[test]
fn cli_version_displays_version() -> Result<()> {
    cmd()?
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("anti_entropator"));
    Ok(())
}

#[test]
fn cli_no_args_shows_help() -> Result<()> {
    // Running without subcommand should show help or error
    cmd()?
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
    Ok(())
}

// ==================== Profile Command Tests ====================

#[test]
fn profile_help_shows_options() -> Result<()> {
    cmd()?
        .args(["profile", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile a directory"))
        .stdout(predicate::str::contains("--out"))
        .stdout(predicate::str::contains("--format"));
    Ok(())
}

#[test]
fn profile_nonexistent_path_fails() -> Result<()> {
    cmd()?
        .args(["profile", "/nonexistent/path/that/does/not/exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
    Ok(())
}

#[test]
fn profile_file_instead_of_directory_fails() -> Result<()> {
    let temp = tempdir()?;
    let file_path = temp.path().join("test.txt");
    std::fs::write(&file_path, "test content")?;

    cmd()?
        .arg("profile")
        .arg(&file_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
    Ok(())
}

#[test]
fn profile_empty_directory_succeeds() -> Result<()> {
    let temp = tempdir()?;

    cmd()?
        .arg("profile")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Anti-Entropator"));
    Ok(())
}

#[test]
fn profile_directory_with_files_shows_stats() -> Result<()> {
    let temp = tempdir()?;

    // Create some test files
    std::fs::write(temp.path().join("doc.pdf"), "fake pdf content")?;
    std::fs::write(temp.path().join("image.jpg"), "fake jpg content")?;
    std::fs::write(temp.path().join("code.rs"), "fn main() {}")?;

    cmd()?
        .arg("profile")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Files:"))
        .stdout(predicate::str::contains("3")); // 3 files
    Ok(())
}

#[test]
fn profile_json_output_is_valid_json() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    let output = cmd()?
        .arg("profile")
        .arg(temp.path())
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output)?;
    // Should be valid JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&json_str).is_ok(),
        "Output should be valid JSON: {}",
        json_str
    );
    Ok(())
}

#[test]
fn profile_with_no_mime_flag_works() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    cmd()?
        .arg("profile")
        .arg(temp.path())
        .arg("--no-mime")
        .assert()
        .success();
    Ok(())
}

#[test]
fn profile_with_decimal_flag_works() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    cmd()?
        .arg("profile")
        .arg(temp.path())
        .arg("--decimal")
        .assert()
        .success();
    Ok(())
}

// ==================== Scan Command Tests ====================

#[test]
fn scan_help_shows_options() -> Result<()> {
    cmd()?
        .args(["scan", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan a directory"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--limit"));
    Ok(())
}

#[test]
fn scan_nonexistent_path_fails() -> Result<()> {
    cmd()?
        .args(["scan", "/nonexistent/path"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
    Ok(())
}

#[test]
fn scan_empty_directory_succeeds() -> Result<()> {
    let temp = tempdir()?;

    cmd()?
        .arg("scan")
        .arg(temp.path())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan"));
    Ok(())
}

#[test]
fn scan_with_limit_respects_limit() -> Result<()> {
    let temp = tempdir()?;

    // Create multiple files
    for i in 0..10 {
        std::fs::write(temp.path().join(format!("file{}.txt", i)), "content")?;
    }

    cmd()?
        .arg("scan")
        .arg(temp.path())
        .arg("--limit")
        .arg("3")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("scanning 3"));
    Ok(())
}

// Unix-only: requires chmod to create permission-denied directories
#[test]
#[cfg(unix)]
fn scan_partial_errors_exits_nonzero() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir()?;
    // One readable file so WalkDir starts scanning
    std::fs::write(temp.path().join("readable.txt"), b"hello")?;
    // One unreadable subdirectory so WalkDir produces Err entries
    let blocked = temp.path().join("blocked");
    std::fs::create_dir(&blocked)?;
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))?;

    let result = cmd()?
        .arg("scan")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("scan incomplete"));

    // Restore permissions so tempdir cleanup succeeds
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755))?;
    drop(result);
    Ok(())
}

// ==================== Doctor Command Tests ====================

#[test]
fn doctor_help_shows_description() -> Result<()> {
    cmd()?
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("preflight"));
    Ok(())
}

// Note: doctor command makes network calls, so we just test it runs
// In a real CI environment, you might skip this or mock the services
#[test]
#[ignore] // Ignore by default as it requires Docker services
fn doctor_runs_checks() -> Result<()> {
    cmd()?
        .arg("doctor")
        .assert()
        .stdout(predicate::str::contains("Docker"));
    Ok(())
}

// ==================== Ingest Command Tests ====================

#[test]
fn ingest_help_shows_options() -> Result<()> {
    cmd()?
        .args(["ingest", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ingest files"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--offline"))
        .stdout(predicate::str::contains("--types"))
        .stdout(predicate::str::contains("--max-size"))
        .stdout(predicate::str::contains("--format"));
    Ok(())
}

#[test]
fn ingest_nonexistent_path_fails() -> Result<()> {
    cmd()?
        .args(["ingest", "/nonexistent/path", "--dry-run", "--offline"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
    Ok(())
}

#[test]
fn ingest_dry_run_does_not_upload() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Would upload"))
        .stdout(predicate::str::contains("no files were uploaded"))
        .stdout(predicate::str::contains("Uploaded:").not());
    Ok(())
}

#[test]
fn ingest_rejects_invalid_max_size() -> Result<()> {
    let temp = tempdir()?;

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--max-size", "100TB", "--dry-run", "--offline"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid size '100TB'"))
        .stderr(predicate::str::contains("B, KB, MB, or GB"));
    Ok(())
}

#[test]
fn ingest_max_size_filters_files() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("small.txt"), b"small")?;
    std::fs::write(temp.path().join("large.txt"), vec![0_u8; 2048])?;

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--max-size", "1KB", "--dry-run", "--offline"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 1 files to ingest"))
        .stdout(predicate::str::contains("Would upload:    1 blobs"));
    Ok(())
}

#[test]
#[cfg(unix)]
fn ingest_partial_errors_exit_nonzero() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir()?;
    std::fs::write(temp.path().join("readable.txt"), b"hello")?;
    let blocked = temp.path().join("unreadable.txt");
    std::fs::write(&blocked, b"blocked")?;
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))?;

    let result = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Errors:          1 files"))
        .stderr(predicate::str::contains(
            "ingest preview incomplete: 1 file(s) failed",
        ));

    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o644))?;
    drop(result);
    Ok(())
}

/// An endpoint on a closed local port: connection refused, no Docker required.
const UNREACHABLE_S3_ENDPOINT: &str = "http://127.0.0.1:9";

#[test]
fn ingest_offline_dry_run_reports_store_not_checked() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Mode:    dry-run, offline (store not checked)",
        ))
        .stdout(predicate::str::contains(
            "Already in store: not checked (--offline)",
        ))
        .stdout(predicate::str::contains(
            "Unchanged:       not checked (--offline)",
        ))
        .stdout(predicate::str::contains(
            "Would observe:   1 paths (catalog not read)",
        ))
        .stdout(predicate::str::contains(
            "Remove --dry-run --offline to actually ingest",
        ));
    Ok(())
}

#[test]
fn ingest_source_flag_overrides_source_id_and_defaults_to_canonical_root() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    // Default: canonical absolute path of the ingest root.
    let output = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline", "--format", "json"])
        .output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        json["source_id"].as_str(),
        Some(temp.path().canonicalize()?.to_string_lossy().as_ref())
    );

    // Override: trimmed name, echoed in JSON and in the human header.
    let output = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args([
            "--source",
            "  downloads  ",
            "--dry-run",
            "--offline",
            "--format",
            "json",
        ])
        .output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["source_id"].as_str(), Some("downloads"));

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--source", "downloads", "--dry-run", "--offline"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Source:  downloads"))
        .stdout(predicate::str::contains("Source:          downloads"));
    Ok(())
}

#[test]
fn ingest_source_flag_rejects_empty_name() -> Result<()> {
    let temp = tempdir()?;
    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--source", "   ", "--dry-run", "--offline"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("source name must not be empty"));
    Ok(())
}

#[test]
fn ingest_offline_dry_run_never_contacts_the_store() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    // If --offline touched the network this would fail with a connection error.
    cmd()?
        .env("ANTI_ENTROPATOR_S3_ENDPOINT", UNREACHABLE_S3_ENDPOINT)
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Would upload:    1 blobs"));
    Ok(())
}

#[test]
fn ingest_dry_run_fails_closed_when_store_unreachable() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    // Plain --dry-run is a connected preview: it must not silently degrade to
    // an offline "everything is new" answer.
    cmd()?
        .env("ANTI_ENTROPATOR_S3_ENDPOINT", UNREACHABLE_S3_ENDPOINT)
        .arg("ingest")
        .arg(temp.path())
        .arg("--dry-run")
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "Mode:    dry-run (store checked, nothing written)",
        ))
        .stderr(predicate::str::contains("Cannot connect to lakehouse"))
        .stdout(predicate::str::contains("Would upload").not());
    Ok(())
}

#[test]
fn ingest_offline_requires_dry_run() -> Result<()> {
    let temp = tempdir()?;

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .arg("--offline")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--dry-run"));
    Ok(())
}

#[test]
fn ingest_dry_run_json_is_valid_summary() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    let output = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline", "--format", "json"])
        .output()?;

    assert!(output.status.success());
    let json_str = String::from_utf8(output.stdout)?;
    let json: serde_json::Value = serde_json::from_str(&json_str)?;

    assert_eq!(json["format_version"].as_u64(), Some(4));
    assert_eq!(json["mode"].as_str(), Some("dry_run_offline"));
    assert_eq!(json["status"].as_str(), Some("success"));
    assert_eq!(json["candidates"].as_u64(), Some(1));
    // Every run carries an identity, previews included (ADR-009 run_id).
    let run_id = json["run_id"].as_str().expect("run_id must be a string");
    assert!(
        uuid::Uuid::parse_str(run_id).is_ok(),
        "run_id must be a UUID, got {run_id}"
    );
    // Offline: no catalog state, so every candidate is "would observe" and
    // "would upload"; nothing can be unchanged or already stored.
    assert_eq!(json["observed"].as_u64(), Some(1));
    assert_eq!(json["unchanged"].as_u64(), Some(0));
    assert_eq!(json["uploaded"].as_u64(), Some(1));
    assert_eq!(json["already_exists"].as_u64(), Some(0));
    assert_eq!(json["failed"].as_u64(), Some(0));
    assert_eq!(json["bytes"].as_u64(), Some(7));
    assert!(json["source_id"].is_string());
    assert_eq!(json["catalog_commit"].as_str(), Some("not_attempted"));
    // Previews never commit, so the batch accounting is all zero.
    assert_eq!(json["committed"].as_u64(), Some(0));
    assert_eq!(json["batches_committed"].as_u64(), Some(0));
    assert_eq!(json["batches_failed"].as_u64(), Some(0));
    assert_eq!(json["skipped"].as_u64(), Some(0));
    assert_eq!(json["errors"].as_array().map(|e| e.len()), Some(0));
    assert!(
        !json_str.contains("Would upload"),
        "JSON stdout must not include the human report: {json_str}"
    );
    Ok(())
}

/// Tracing diagnostics must go to stderr so `--format json` stdout stays a
/// single parseable document. The default `tracing_subscriber::fmt` writer is
/// stdout; this test guards the explicit stderr routing in `main.rs`.
#[test]
fn ingest_json_stdout_stays_pure_when_tracing_is_enabled() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "content")?;

    let output = cmd()?
        .env("RUST_LOG", "anti_entropator=info")
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline", "--format", "json"])
        .output()?;

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {stdout}"));
    assert_eq!(json["candidates"].as_u64(), Some(1));
    assert!(
        stderr.contains("Collected ingest candidates"),
        "expected the tracing line on stderr, got: {stderr}"
    );
    assert!(
        !stdout.contains("Collected ingest candidates"),
        "tracing must not leak into stdout: {stdout}"
    );
    Ok(())
}

#[test]
fn ingest_json_empty_directory_emits_success_summary() -> Result<()> {
    let temp = tempdir()?;

    let output = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline", "--format", "json"])
        .output()?;

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["status"].as_str(), Some("success"));
    assert_eq!(json["candidates"].as_u64(), Some(0));
    assert_eq!(json["uploaded"].as_u64(), Some(0));
    assert_eq!(json["failed"].as_u64(), Some(0));
    Ok(())
}

#[test]
#[cfg(unix)]
fn ingest_partial_errors_json_exits_nonzero() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempdir()?;
    std::fs::write(temp.path().join("readable.txt"), b"hello")?;
    let blocked = temp.path().join("unreadable.txt");
    std::fs::write(&blocked, b"blocked")?;
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000))?;

    let output = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline", "--format", "json"])
        .output()?;

    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o644))?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ingest preview incomplete: 1 file(s) failed"),
        "stderr should keep the existing failure contract: {stderr}"
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["format_version"].as_u64(), Some(4));
    assert_eq!(json["mode"].as_str(), Some("dry_run_offline"));
    assert_eq!(json["status"].as_str(), Some("incomplete"));
    assert_eq!(json["candidates"].as_u64(), Some(2));
    assert_eq!(json["failed"].as_u64(), Some(1));
    assert_eq!(json["catalog_commit"].as_str(), Some("not_attempted"));
    Ok(())
}

#[test]
fn ingest_rejects_unsupported_format() -> Result<()> {
    let temp = tempdir()?;

    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--offline", "--format", "table"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'table'"));
    Ok(())
}

// ==================== Query Command Tests ====================

#[test]
fn query_help_shows_options() -> Result<()> {
    cmd()?
        .args(["query", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Execute a one-shot SQL query"));
    Ok(())
}

// ==================== Removed Placeholder Commands ====================

/// `sql`, `duplicates`, and `merge` were placeholder subcommands that only
/// exited non-zero. They are removed from the binary until implemented; the
/// CLI must reject them as unknown rather than advertise them in `--help`.
#[test]
fn placeholder_commands_are_not_advertised_or_accepted() -> Result<()> {
    for name in ["sql", "duplicates", "merge"] {
        cmd()?
            .arg(name)
            .assert()
            .failure()
            .stderr(predicate::str::contains("unrecognized subcommand"));
    }

    cmd()?
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("\n  sql").not())
        .stdout(predicate::str::contains("\n  duplicates").not())
        .stdout(predicate::str::contains("\n  merge").not());
    Ok(())
}

// ==================== Full Flow Tests ====================

#[test]
#[ignore] // Requires: docker compose up -d && source .env
fn ingest_then_query_flow() -> Result<()> {
    // 1. Init (idempotent). On a table created before the ADR-009 columns the
    //    first call adds them; every later call must report the schema as
    //    up to date without touching it.
    cmd()?.arg("init").assert().success();
    cmd()?
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Checking table schema... up to date",
        ));

    // 2. Create temp dir with unique marker filenames. `big` exceeds the
    //    upload part size so the conditional multipart path is exercised.
    let temp = tempdir()?;
    let marker = &uuid::Uuid::new_v4().to_string()[..8];
    let file_a = format!("s2b_{}_a.txt", marker);
    let file_b = format!("s2b_{}_b.txt", marker);
    let file_big = format!("s2b_{}_big.bin", marker);
    std::fs::write(temp.path().join(&file_a), format!("hello-{marker}"))?;
    std::fs::write(temp.path().join(&file_b), format!("world-{marker}"))?;
    let big: Vec<u8> = (0..(9 * 1024 * 1024usize))
        .map(|i| (i as u32).wrapping_mul(2654435761) as u8)
        .chain(marker.bytes())
        .collect();
    std::fs::write(temp.path().join(&file_big), &big)?;

    // 3. Ingest with --format json -- should upload 3 files and commit.
    //    stdout must be exactly one JSON document even on the upload+commit
    //    path (the Iceberg writer used to print progress lines to stdout).
    //    `--batch-size 2` makes the 3 rows land as two real Iceberg commits
    //    and `--concurrency 2` runs the pipeline with more than one worker.
    let ingest = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args([
            "--format",
            "json",
            "--batch-size",
            "2",
            "--concurrency",
            "2",
        ])
        .output()?;
    assert!(ingest.status.success());
    let ingest_json: serde_json::Value = serde_json::from_slice(&ingest.stdout)
        .expect("ingest --format json stdout must be a single JSON document");
    assert_eq!(ingest_json["mode"].as_str(), Some("ingest"));
    assert_eq!(ingest_json["observed"].as_u64(), Some(3));
    assert_eq!(ingest_json["unchanged"].as_u64(), Some(0));
    assert_eq!(ingest_json["uploaded"].as_u64(), Some(3));
    assert_eq!(ingest_json["catalog_commit"].as_str(), Some("succeeded"));
    assert_eq!(ingest_json["committed"].as_u64(), Some(3));
    assert_eq!(ingest_json["batches_committed"].as_u64(), Some(2));
    assert_eq!(ingest_json["batches_failed"].as_u64(), Some(0));
    assert_eq!(ingest_json["skipped"].as_u64(), Some(0));
    let run_id = ingest_json["run_id"]
        .as_str()
        .expect("run_id in summary")
        .to_string();

    // 4. Query with marker to isolate this run's rows
    let query = format!(
        "SELECT count(*) FROM files WHERE filename LIKE 's2b_{}%'",
        marker
    );
    cmd()?
        .arg("query")
        .arg(&query)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 3        |"));

    // 4b. Every committed row carries the run identity and the path identity
    //     (ADR-009 observation columns), and the canonical source root.
    let source_root = temp.path().canonicalize()?;
    let run_id_hex = run_id.replace('-', "");
    let by_run = format!(
        "SELECT count(*) FROM files WHERE encode(run_id, 'hex') = '{run_id_hex}' AND observation_status = 'present' AND source_id = '{}' AND relative_path = filename",
        source_root.display()
    );
    cmd()?
        .arg("query")
        .arg(&by_run)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 3        |"));

    // 5. Connected dry-run -- reads catalog state, verifies all blobs,
    //    uploads nothing, appends nothing: every path is unchanged.
    let plan = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--format", "json"])
        .output()?;
    assert!(plan.status.success());
    let plan_json: serde_json::Value = serde_json::from_slice(&plan.stdout)?;
    assert_eq!(plan_json["mode"].as_str(), Some("dry_run"));
    assert_eq!(plan_json["candidates"].as_u64(), Some(3));
    assert_eq!(plan_json["observed"].as_u64(), Some(0));
    assert_eq!(plan_json["unchanged"].as_u64(), Some(3));
    assert_eq!(plan_json["uploaded"].as_u64(), Some(0));
    assert_eq!(plan_json["already_exists"].as_u64(), Some(3));
    assert_eq!(plan_json["catalog_commit"].as_str(), Some("not_attempted"));

    // 6. Re-ingest unchanged -- no rows, no uploads, no commit (idempotent)
    let again = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--format", "json"])
        .output()?;
    assert!(again.status.success());
    let again_json: serde_json::Value = serde_json::from_slice(&again.stdout)?;
    assert_eq!(again_json["observed"].as_u64(), Some(0));
    assert_eq!(again_json["unchanged"].as_u64(), Some(3));
    assert_eq!(again_json["uploaded"].as_u64(), Some(0));
    assert_eq!(again_json["catalog_commit"].as_str(), Some("not_attempted"));

    // 7. Query again -- still exactly 3 rows (no duplicates)
    cmd()?
        .arg("query")
        .arg(&query)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 3        |"));

    // 8. ADR-009 transitions in one run:
    //    - identical bytes at a second path -> new row, blob already exists;
    //    - changed bytes at an existing path -> new row, new blob;
    //    - the untouched file -> unchanged.
    let file_copy = format!("s2b_{}_copy_of_a.txt", marker);
    std::fs::copy(temp.path().join(&file_a), temp.path().join(&file_copy))?;
    std::fs::write(temp.path().join(&file_b), format!("world-v2-{marker}"))?;

    let third = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--format", "json"])
        .output()?;
    assert!(third.status.success());
    let third_json: serde_json::Value = serde_json::from_slice(&third.stdout)?;
    assert_eq!(third_json["candidates"].as_u64(), Some(4));
    assert_eq!(third_json["observed"].as_u64(), Some(2));
    assert_eq!(third_json["unchanged"].as_u64(), Some(2));
    assert_eq!(third_json["uploaded"].as_u64(), Some(1));
    assert_eq!(third_json["already_exists"].as_u64(), Some(3));
    assert_eq!(third_json["catalog_commit"].as_str(), Some("succeeded"));

    // 5 rows total: 3 original + copy + new version of b.
    cmd()?
        .arg("query")
        .arg(&query)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 5        |"));

    // The copy and the original share one blob (two rows, one content_hash).
    let shared_blob = format!(
        "SELECT count(*) FROM files WHERE filename IN ('{file_a}', '{file_copy}') GROUP BY content_hash"
    );
    cmd()?
        .arg("query")
        .arg(&shared_blob)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 2        |"));

    // Current state of b: the latest present observation carries the new hash.
    let b_history = format!(
        "SELECT count(DISTINCT content_hash) FROM files WHERE filename = '{file_b}' AND observation_status = 'present'"
    );
    cmd()?
        .arg("query")
        .arg(&b_history)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 2 "));

    // 9. A different --source is a different logical source: everything is
    //    re-observed, nothing is re-uploaded.
    let other_source = format!("s2b-other-{marker}");
    let other = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--source", &other_source, "--format", "json"])
        .output()?;
    assert!(other.status.success());
    let other_json: serde_json::Value = serde_json::from_slice(&other.stdout)?;
    assert_eq!(
        other_json["source_id"].as_str(),
        Some(other_source.as_str())
    );
    assert_eq!(other_json["observed"].as_u64(), Some(4));
    assert_eq!(other_json["uploaded"].as_u64(), Some(0));
    assert_eq!(other_json["already_exists"].as_u64(), Some(4));

    Ok(())
}

// ==================== Run Journal Tests ====================

#[test]
fn runs_help_lists_list_and_show() -> Result<()> {
    cmd()?
        .args(["runs", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\n  list"))
        .stdout(predicate::str::contains("\n  show"));
    cmd()?
        .args(["runs", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--source"))
        .stdout(predicate::str::contains("--open"))
        .stdout(predicate::str::contains("--format"));
    Ok(())
}

#[test]
fn runs_show_rejects_non_uuid() -> Result<()> {
    cmd()?
        .args(["runs", "show", "not-a-run-id"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
    Ok(())
}

/// Run a `COUNT(*)` query and read the single number out of the table output.
fn query_count(sql: &str) -> Result<u64> {
    let out = cmd()?.arg("query").arg(sql).output()?;
    assert!(out.status.success(), "query failed: {sql}");
    let stdout = String::from_utf8(out.stdout)?;
    stdout
        .lines()
        .filter_map(|l| l.strip_prefix('|').and_then(|l| l.strip_suffix('|')))
        .filter_map(|cell| cell.trim().parse::<u64>().ok())
        .next()
        .ok_or_else(|| anyhow::anyhow!("no count in query output:\n{stdout}"))
}

/// ADR-009 slice 4 / roadmap criterion 4: a run killed between upload and
/// commit is never reported as success, its state is identifiable afterwards,
/// and the next run for the source reconciles and supersedes it.
#[test]
#[ignore] // Requires: docker compose up -d && source .env
fn interrupted_ingest_is_recorded_and_reconciled() -> Result<()> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;

    cmd()?.arg("init").assert().success();

    // 40 distinct 2 MiB files: enough work that the run is still going when
    // the first batch has been committed.
    let temp = tempdir()?;
    let marker = &uuid::Uuid::new_v4().to_string()[..8];
    let total = 40u64;
    for i in 0..total {
        let body: Vec<u8> = (0..(2 * 1024 * 1024usize))
            .map(|k| ((k as u64).wrapping_mul(2654435761).wrapping_add(i * 7919)) as u8)
            .chain(marker.bytes())
            .collect();
        std::fs::write(temp.path().join(format!("s4a_{marker}_{i:02}.bin")), body)?;
    }
    let source = format!("s4a-kill-{marker}");

    // 1. Start an ingest that commits one row per batch and kill it right
    //    after the first `Batch committed` event on stderr (SIGKILL: no
    //    chance to write a terminal journal state).
    #[allow(deprecated)]
    let bin = assert_cmd::cargo::cargo_bin("anti_entropator");
    let mut child = std::process::Command::new(bin)
        .arg("ingest")
        .arg(temp.path())
        .args([
            "--source",
            &source,
            "--batch-size",
            "1",
            "--concurrency",
            "1",
        ])
        .args(["--format", "json"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child.stderr.take().expect("piped stderr");
    let mut saw_commit = false;
    for line in BufReader::new(stderr).lines() {
        if line?.contains("Batch committed") {
            saw_commit = true;
            break;
        }
    }
    child.kill()?;
    let status = child.wait()?;
    assert!(saw_commit, "run ended before its first batch commit");
    assert!(!status.success(), "a killed run must not exit 0");

    // 2. The journal shows exactly one open run for this source.
    let list = cmd()?
        .args(["runs", "list", "--source", &source, "--open"])
        .args(["--format", "json"])
        .output()?;
    assert!(list.status.success());
    let open: Vec<serde_json::Value> = serde_json::from_slice(&list.stdout)?;
    assert_eq!(open.len(), 1, "one interrupted run expected: {open:?}");
    let killed = &open[0];
    assert_eq!(killed["outcome"], "interrupted");
    let killed_id = killed["run_id"].as_str().expect("run_id").to_string();
    let journal_rows = killed["rows_committed"].as_u64().expect("rows_committed");
    assert!(journal_rows >= 1, "journal must record the committed batch");
    assert!(journal_rows < total, "run must not have finished");

    let show = cmd()?
        .args(["runs", "show", &killed_id, "--format", "json"])
        .output()?;
    assert!(show.status.success());
    let show_json: serde_json::Value = serde_json::from_slice(&show.stdout)?;
    let history = show_json["history"].as_array().expect("history");
    assert_eq!(history[0]["state"], "started");
    let last = history.last().unwrap()["state"].as_str().unwrap();
    assert!(
        matches!(last, "started" | "committing" | "batch_committed"),
        "last own state must be non-terminal, got {last}"
    );

    // 3. The catalog holds at least what the journal recorded (the kill may
    //    have landed between a commit and its journal entry). The `source_id`
    //    predicate prunes pre-ADR-009 data files, which have no `run_id`
    //    column (see "Known limitations" in the manual).
    let hex = killed_id.replace('-', "");
    let in_catalog = query_count(&format!(
        "SELECT count(*) FROM files WHERE source_id = '{source}' AND encode(run_id, 'hex') = '{hex}'"
    ))?;
    assert!(in_catalog >= journal_rows);
    assert!(in_catalog <= journal_rows + 1);

    // 4. The next run for the source warns, reconciles the killed run against
    //    the catalog, observes exactly the paths that never got a row, and
    //    supersedes its predecessor when it completes.
    let again = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--source", &source, "--format", "json"])
        .output()?;
    assert!(again.status.success());
    let stderr = String::from_utf8_lossy(&again.stderr);
    assert!(
        stderr.contains("did not finish"),
        "second run must warn about the interrupted one; stderr:\n{stderr}"
    );
    let again_json: serde_json::Value = serde_json::from_slice(&again.stdout)?;
    assert_eq!(again_json["status"], "success");
    assert_eq!(again_json["unchanged"].as_u64(), Some(in_catalog));
    assert_eq!(again_json["observed"].as_u64(), Some(total - in_catalog));
    // Blob states cover every candidate (unchanged paths are verified too).
    // A blob the killed run uploaded but never committed is found in the
    // store and only re-observed, never re-uploaded; with concurrency 1 at
    // most one file was in that window.
    let uploaded = again_json["uploaded"].as_u64().unwrap();
    let already = again_json["already_exists"].as_u64().unwrap();
    assert_eq!(uploaded + already, total);
    let upload_without_commit = already - in_catalog;
    assert!(
        upload_without_commit <= 1,
        "at most one upload-without-commit: {upload_without_commit}"
    );
    let new_id = again_json["run_id"].as_str().unwrap();

    let closed = cmd()?
        .args(["runs", "show", &killed_id, "--format", "json"])
        .output()?;
    let closed_json: serde_json::Value = serde_json::from_slice(&closed.stdout)?;
    assert_eq!(closed_json["outcome"], "reconciled");
    let states: Vec<&serde_json::Value> =
        closed_json["history"].as_array().unwrap().iter().collect();
    let reconciled = states
        .iter()
        .find(|t| t["state"] == "reconciled")
        .expect("reconciled entry");
    assert_eq!(reconciled["rows_in_catalog"].as_u64(), Some(in_catalog));
    let superseded = states
        .iter()
        .find(|t| t["state"] == "superseded_by")
        .expect("superseded_by entry");
    assert_eq!(superseded["run_id"].as_str(), Some(new_id));

    let open_after = cmd()?
        .args(["runs", "list", "--source", &source, "--open"])
        .args(["--format", "json"])
        .output()?;
    let open_after: Vec<serde_json::Value> = serde_json::from_slice(&open_after.stdout)?;
    assert!(open_after.is_empty(), "no open runs left: {open_after:?}");

    // 5. The completed run's own journal is terminal.
    let done = cmd()?
        .args(["runs", "show", new_id, "--format", "json"])
        .output()?;
    let done_json: serde_json::Value = serde_json::from_slice(&done.stdout)?;
    assert_eq!(done_json["outcome"], "completed");
    assert_eq!(
        done_json["rows_committed"].as_u64(),
        Some(total - in_catalog)
    );

    Ok(())
}

// ==================== Removed Flag Rejection Tests ====================

#[test]
fn removed_config_flag_rejected() -> Result<()> {
    let temp = tempdir()?;
    cmd()?
        .arg("--config")
        .arg("/nonexistent/config.toml")
        .arg("profile")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
    Ok(())
}

#[test]
fn removed_global_verbose_flag_rejected() -> Result<()> {
    let temp = tempdir()?;
    cmd()?
        .arg("-v")
        .arg("profile")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
    Ok(())
}

#[test]
fn removed_since_flag_rejected() -> Result<()> {
    let temp = tempdir()?;
    cmd()?
        .arg("ingest")
        .arg("--since")
        .arg("7d")
        .arg("--dry-run")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
    Ok(())
}

#[test]
fn removed_auto_merge_flag_rejected() -> Result<()> {
    let temp = tempdir()?;
    cmd()?
        .arg("ingest")
        .arg("--auto-merge")
        .arg("--dry-run")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
    Ok(())
}

#[test]
fn removed_ingest_verbose_flag_rejected() -> Result<()> {
    let temp = tempdir()?;
    cmd()?
        .arg("ingest")
        .arg("-v")
        .arg("--dry-run")
        .arg(temp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
    Ok(())
}

// ==================== Profile E2E Tests ====================

#[test]
fn profile_json_exact_counts() -> Result<()> {
    let dir = tempdir()?;
    std::fs::write(dir.path().join("a.txt"), b"hello")?; // 5 bytes
    std::fs::write(dir.path().join("b.txt"), b"world")?; // 5 bytes
    std::fs::write(dir.path().join("c.jpg"), vec![0u8; 100])?; // 100 bytes

    let output = cmd()?
        .arg("profile")
        .arg(dir.path())
        .arg("--format")
        .arg("json")
        .arg("--no-mime")
        .arg("--no-duplicates")
        .output()?;

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(json["file_count"].as_u64(), Some(3));
    assert_eq!(json["total_bytes"].as_u64(), Some(110));
    assert_eq!(json["by_extension"][".txt"]["count"].as_u64(), Some(2));
    assert_eq!(json["by_extension"][".jpg"]["count"].as_u64(), Some(1));
    assert_eq!(json["by_category"]["document"]["count"].as_u64(), Some(2));
    assert_eq!(json["by_category"]["image"]["count"].as_u64(), Some(1));

    Ok(())
}

#[test]
fn profile_out_flag_creates_files() -> Result<()> {
    let dir = tempdir()?;
    std::fs::write(dir.path().join("test.txt"), b"data")?;
    let out_dir = tempdir()?;

    cmd()?
        .arg("profile")
        .arg(dir.path())
        .arg("--out")
        .arg(out_dir.path())
        .arg("--no-mime")
        .arg("--no-duplicates")
        .assert()
        .success();

    assert!(out_dir.path().join("profile.json").exists());
    assert!(out_dir.path().join("profile.md").exists());

    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(
        out_dir.path().join("profile.json"),
    )?)?;
    assert_eq!(json["file_count"].as_u64(), Some(1));

    Ok(())
}
