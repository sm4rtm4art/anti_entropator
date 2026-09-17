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
        .stdout(predicate::str::contains("Would upload:    1 files"));
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
            "Remove --dry-run --offline to actually ingest",
        ));
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
        .stdout(predicate::str::contains("Would upload:    1 files"));
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

    assert_eq!(json["format_version"].as_u64(), Some(2));
    assert_eq!(json["mode"].as_str(), Some("dry_run_offline"));
    assert_eq!(json["status"].as_str(), Some("success"));
    assert_eq!(json["candidates"].as_u64(), Some(1));
    // Every run carries an identity, previews included (ADR-009 run_id).
    let run_id = json["run_id"].as_str().expect("run_id must be a string");
    assert!(
        uuid::Uuid::parse_str(run_id).is_ok(),
        "run_id must be a UUID, got {run_id}"
    );
    assert_eq!(json["uploaded"].as_u64(), Some(1));
    assert_eq!(json["already_exists"].as_u64(), Some(0));
    assert_eq!(json["failed"].as_u64(), Some(0));
    assert_eq!(json["bytes"].as_u64(), Some(7));
    assert_eq!(json["catalog_commit"].as_str(), Some("not_attempted"));
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
    assert_eq!(json["format_version"].as_u64(), Some(2));
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
    let ingest = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--format", "json"])
        .output()?;
    assert!(ingest.status.success());
    let ingest_json: serde_json::Value = serde_json::from_slice(&ingest.stdout)
        .expect("ingest --format json stdout must be a single JSON document");
    assert_eq!(ingest_json["mode"].as_str(), Some("ingest"));
    assert_eq!(ingest_json["uploaded"].as_u64(), Some(3));
    assert_eq!(ingest_json["catalog_commit"].as_str(), Some("succeeded"));
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

    // 5. Connected dry-run -- verifies all blobs in the store, uploads nothing
    let plan = cmd()?
        .arg("ingest")
        .arg(temp.path())
        .args(["--dry-run", "--format", "json"])
        .output()?;
    assert!(plan.status.success());
    let plan_json: serde_json::Value = serde_json::from_slice(&plan.stdout)?;
    assert_eq!(plan_json["mode"].as_str(), Some("dry_run"));
    assert_eq!(plan_json["candidates"].as_u64(), Some(3));
    assert_eq!(plan_json["uploaded"].as_u64(), Some(0));
    assert_eq!(plan_json["already_exists"].as_u64(), Some(3));
    assert_eq!(plan_json["catalog_commit"].as_str(), Some("not_attempted"));

    // 6. Re-ingest -- no new uploads (idempotent)
    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Uploaded:        0"));

    // 7. Query again -- still exactly 3 rows (no duplicates)
    cmd()?
        .arg("query")
        .arg(&query)
        .assert()
        .success()
        .stdout(predicate::str::contains("| 3        |"));

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
