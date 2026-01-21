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
        .stdout(predicate::str::contains("--types"))
        .stdout(predicate::str::contains("--max-size"));
    Ok(())
}

#[test]
fn ingest_nonexistent_path_fails() -> Result<()> {
    cmd()?
        .args(["ingest", "/nonexistent/path", "--dry-run"])
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
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run"))
        .stdout(predicate::str::contains("no files were uploaded"));
    Ok(())
}

// ==================== Unimplemented Commands Tests ====================

#[test]
fn sql_help_shows_options() -> Result<()> {
    cmd()?
        .args(["sql", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Open an interactive SQL REPL"));
    Ok(())
}

#[test]
fn query_help_shows_options() -> Result<()> {
    cmd()?
        .args(["query", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Execute a one-shot SQL query"));
    Ok(())
}

#[test]
fn duplicates_command_shows_not_implemented() -> Result<()> {
    cmd()?
        .arg("duplicates")
        .assert()
        .success()
        .stdout(predicate::str::contains("not yet implemented"));
    Ok(())
}

// ==================== Full Flow Tests ====================

#[test]
#[ignore] // Requires full Docker stack running
fn ingest_then_query_flow() -> Result<()> {
    let temp = tempdir()?;
    std::fs::write(temp.path().join("test.txt"), "hello world")?;

    // 1. Ingest
    cmd()?
        .arg("ingest")
        .arg(temp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("successfully"));

    // 2. Query
    cmd()?
        .args(["query", "SELECT count(*) FROM files"])
        .assert()
        .success()
        .stdout(predicate::str::contains("count"));

    Ok(())
}

// ==================== Global Flags Tests ====================

#[test]
fn verbose_flag_is_accepted() -> Result<()> {
    let temp = tempdir()?;

    cmd()?
        .arg("-v")
        .arg("profile")
        .arg(temp.path())
        .assert()
        .success();
    Ok(())
}

#[test]
fn config_flag_with_nonexistent_file_still_works() -> Result<()> {
    // Config file is optional, so nonexistent should not fail
    let temp = tempdir()?;

    cmd()?
        .arg("--config")
        .arg("/nonexistent/config.toml")
        .arg("profile")
        .arg(temp.path())
        .assert()
        .success();
    Ok(())
}
