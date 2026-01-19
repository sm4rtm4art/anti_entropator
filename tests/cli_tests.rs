//! CLI Integration Tests
//!
//! Tests the anti_entropator binary as a black box, verifying command-line behavior.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

/// Get a Command for the anti_entropator binary
fn cmd() -> Command {
    #[allow(deprecated)] // cargo_bin works fine for our use case
    Command::cargo_bin("anti_entropator").unwrap()
}

// ==================== Help & Version Tests ====================

#[test]
fn cli_help_displays_usage() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("lakehouse"))
        .stdout(predicate::str::contains("Usage:"))
        .stdout(predicate::str::contains("Commands:"));
}

#[test]
fn cli_version_displays_version() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("anti_entropator"));
}

#[test]
fn cli_no_args_shows_help() {
    // Running without subcommand should show help or error
    cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"));
}

// ==================== Profile Command Tests ====================

#[test]
fn profile_help_shows_options() {
    cmd()
        .args(["profile", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile a directory"))
        .stdout(predicate::str::contains("--out"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn profile_nonexistent_path_fails() {
    cmd()
        .args(["profile", "/nonexistent/path/that/does/not/exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn profile_file_instead_of_directory_fails() {
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("test.txt");
    std::fs::write(&file_path, "test content").unwrap();

    cmd()
        .args(["profile", file_path.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a directory"));
}

#[test]
fn profile_empty_directory_succeeds() {
    let temp = tempdir().unwrap();

    cmd()
        .args(["profile", temp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Anti-Entropator"));
}

#[test]
fn profile_directory_with_files_shows_stats() {
    let temp = tempdir().unwrap();

    // Create some test files
    std::fs::write(temp.path().join("doc.pdf"), "fake pdf content").unwrap();
    std::fs::write(temp.path().join("image.jpg"), "fake jpg content").unwrap();
    std::fs::write(temp.path().join("code.rs"), "fn main() {}").unwrap();

    cmd()
        .args(["profile", temp.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Files:"))
        .stdout(predicate::str::contains("3")); // 3 files
}

#[test]
fn profile_json_output_is_valid_json() {
    let temp = tempdir().unwrap();
    std::fs::write(temp.path().join("test.txt"), "content").unwrap();

    let output = cmd()
        .args(["profile", temp.path().to_str().unwrap(), "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json_str = String::from_utf8(output).unwrap();
    // Should be valid JSON
    assert!(
        serde_json::from_str::<serde_json::Value>(&json_str).is_ok(),
        "Output should be valid JSON: {}",
        json_str
    );
}

#[test]
fn profile_with_no_mime_flag_works() {
    let temp = tempdir().unwrap();
    std::fs::write(temp.path().join("test.txt"), "content").unwrap();

    cmd()
        .args(["profile", temp.path().to_str().unwrap(), "--no-mime"])
        .assert()
        .success();
}

#[test]
fn profile_with_decimal_flag_works() {
    let temp = tempdir().unwrap();
    std::fs::write(temp.path().join("test.txt"), "content").unwrap();

    cmd()
        .args(["profile", temp.path().to_str().unwrap(), "--decimal"])
        .assert()
        .success();
}

// ==================== Scan Command Tests ====================

#[test]
fn scan_help_shows_options() {
    cmd()
        .args(["scan", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan a directory"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--limit"));
}

#[test]
fn scan_nonexistent_path_fails() {
    cmd()
        .args(["scan", "/nonexistent/path"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn scan_empty_directory_succeeds() {
    let temp = tempdir().unwrap();

    cmd()
        .args(["scan", temp.path().to_str().unwrap(), "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan"));
}

#[test]
fn scan_with_limit_respects_limit() {
    let temp = tempdir().unwrap();

    // Create multiple files
    for i in 0..10 {
        std::fs::write(temp.path().join(format!("file{}.txt", i)), "content").unwrap();
    }

    cmd()
        .args([
            "scan",
            temp.path().to_str().unwrap(),
            "--limit",
            "3",
            "--dry-run",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("scanning 3"));
}

// ==================== Doctor Command Tests ====================

#[test]
fn doctor_help_shows_description() {
    cmd()
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("preflight"));
}

// Note: doctor command makes network calls, so we just test it runs
// In a real CI environment, you might skip this or mock the services
#[test]
#[ignore] // Ignore by default as it requires Docker services
fn doctor_runs_checks() {
    cmd()
        .arg("doctor")
        .assert()
        .stdout(predicate::str::contains("Docker"));
}

// ==================== Ingest Command Tests ====================

#[test]
fn ingest_help_shows_options() {
    cmd()
        .args(["ingest", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ingest files"))
        .stdout(predicate::str::contains("--dry-run"))
        .stdout(predicate::str::contains("--types"))
        .stdout(predicate::str::contains("--max-size"));
}

#[test]
fn ingest_nonexistent_path_fails() {
    cmd()
        .args(["ingest", "/nonexistent/path", "--dry-run"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn ingest_dry_run_does_not_upload() {
    let temp = tempdir().unwrap();
    std::fs::write(temp.path().join("test.txt"), "content").unwrap();

    cmd()
        .args(["ingest", temp.path().to_str().unwrap(), "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("no files were uploaded"));
}

// ==================== Unimplemented Commands Tests ====================

#[test]
fn sql_command_shows_not_implemented() {
    cmd()
        .arg("sql")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn query_command_shows_not_implemented() {
    cmd()
        .args(["query", "SELECT * FROM files"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

#[test]
fn duplicates_command_shows_not_implemented() {
    cmd()
        .arg("duplicates")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
}

// ==================== Global Flags Tests ====================

#[test]
fn verbose_flag_is_accepted() {
    let temp = tempdir().unwrap();

    cmd()
        .args(["-v", "profile", temp.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn config_flag_with_nonexistent_file_still_works() {
    // Config file is optional, so nonexistent should not fail
    let temp = tempdir().unwrap();

    cmd()
        .args([
            "--config",
            "/nonexistent/config.toml",
            "profile",
            temp.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}
