//! Doctor module - Preflight checks for the lakehouse stack
//!
//! Verifies that all required services and tools are available and configured.

use anyhow::Result;
use console::{style, Emoji};
use std::process::Command;
use std::time::Duration;

static CHECK: Emoji<'_, '_> = Emoji("✅ ", "[OK] ");
static CROSS: Emoji<'_, '_> = Emoji("❌ ", "[FAIL] ");
static WARN: Emoji<'_, '_> = Emoji("⚠️  ", "[WARN] ");
static INFO: Emoji<'_, '_> = Emoji("ℹ️  ", "[INFO] ");

/// Check result
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

impl CheckResult {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            message: message.into(),
            suggestion: None,
        }
    }

    fn warn(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warning,
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    fn error(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Error,
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }
}

/// Evaluate doctor check results and return an error if any check failed.
///
/// This is the pure decision function: it does not print or perform I/O.
/// Use it to test doctor outcome logic without requiring Docker or network.
fn doctor_outcome(results: &[CheckResult]) -> Result<()> {
    let has_errors = results.iter().any(|r| r.status == CheckStatus::Error);
    if has_errors {
        anyhow::bail!("doctor: one or more checks failed");
    }
    Ok(())
}

/// Run all doctor checks
pub async fn run() -> Result<()> {
    println!();
    println!(
        "{}",
        style("═══════════════════════════════════════════════════════════════").cyan()
    );
    println!("  {} Anti-Entropator Doctor", Emoji("🩺", ""));
    println!(
        "{}",
        style("═══════════════════════════════════════════════════════════════").cyan()
    );
    println!();

    let mut results = Vec::new();

    // Check Docker
    println!("{} Checking Docker...", INFO);
    results.push(check_docker().await);

    // Check RustFS
    println!("{} Checking RustFS (S3)...", INFO);
    results.push(check_rustfs().await);

    // Check Catalog (Lakekeeper)
    println!("{} Checking Lakekeeper catalog...", INFO);
    results.push(check_catalog().await);

    // Check Port Availability
    println!("{} Checking port availability...", INFO);
    results.extend(check_ports().await);

    // Check external tools
    println!("{} Checking external tools...", INFO);
    results.extend(check_external_tools().await);

    println!();
    println!("─── Results ────────────────────────────────────────────────────");
    println!();

    let has_errors = results.iter().any(|r| r.status == CheckStatus::Error);
    let has_warnings = results.iter().any(|r| r.status == CheckStatus::Warning);

    for result in &results {
        let emoji = match result.status {
            CheckStatus::Ok => CHECK,
            CheckStatus::Warning => WARN,
            CheckStatus::Error => CROSS,
        };

        let status_style = match result.status {
            CheckStatus::Ok => style(&result.message).green(),
            CheckStatus::Warning => style(&result.message).yellow(),
            CheckStatus::Error => style(&result.message).red(),
        };

        println!("{}{}: {}", emoji, style(&result.name).bold(), status_style);

        if let Some(ref suggestion) = result.suggestion {
            println!("     └─ {}", style(suggestion).dim());
        }
    }

    println!();

    if has_errors {
        println!(
            "{}",
            style("Some checks failed. Please fix the issues above before proceeding.").red()
        );
    } else if has_warnings {
        println!(
            "{}",
            style("All critical checks passed, but some optional features are unavailable.")
                .yellow()
        );
    } else {
        println!(
            "{}",
            style("All checks passed! Your lakehouse stack is ready.").green()
        );
    }

    doctor_outcome(&results)
}

/// Check if Docker is running
async fn check_docker() -> CheckResult {
    match Command::new("docker").arg("info").output() {
        Ok(output) => {
            if output.status.success() {
                CheckResult::ok("Docker", "Docker daemon is running")
            } else {
                CheckResult::error(
                    "Docker",
                    "Docker daemon is not running",
                    "Start Docker Desktop or run: sudo systemctl start docker",
                )
            }
        }
        Err(_) => CheckResult::error(
            "Docker",
            "Docker is not installed",
            "Install Docker from https://docs.docker.com/get-docker/",
        ),
    }
}

/// Check if RustFS is reachable
async fn check_rustfs() -> CheckResult {
    let endpoint = std::env::var("ANTI_ENTROPATOR_S3_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:8200".to_string());

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return CheckResult::error(
                "RustFS",
                format!("Failed to create HTTP client: {}", e),
                "Check your network configuration",
            )
        }
    };

    match client.get(&endpoint).send().await {
        Ok(resp) => {
            if resp.status().is_success() || resp.status().as_u16() == 403 {
                // 403 is expected without auth, but means server is up
                CheckResult::ok("RustFS", format!("RustFS is reachable at {}", endpoint))
            } else {
                CheckResult::warn(
                    "RustFS",
                    format!("RustFS responded with status {}", resp.status()),
                    "Check RustFS configuration and credentials",
                )
            }
        }
        Err(e) => {
            if e.is_connect() {
                CheckResult::error(
                    "RustFS",
                    format!("Cannot connect to RustFS at {}", endpoint),
                    "Run: docker compose up -d rustfs",
                )
            } else {
                CheckResult::error(
                    "RustFS",
                    format!("RustFS check failed: {}", e),
                    "Check if RustFS is running and accessible",
                )
            }
        }
    }
}

/// Check if the Iceberg REST catalog (Lakekeeper) is reachable
async fn check_catalog() -> CheckResult {
    let endpoint = std::env::var("ANTI_ENTROPATOR_CATALOG_ENDPOINT")
        .or_else(|_| std::env::var("ANTI_ENTROPATOR_LAKEKEEPER_ENDPOINT"))
        .unwrap_or_else(|_| "http://localhost:8100".to_string());

    // Lakekeeper exposes a Swagger UI (useful stable HTTP surface for reachability checks).
    let base = endpoint.trim_end_matches('/');
    let swagger_url = format!("{}/swagger-ui/", base);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return CheckResult::error(
                "Lakekeeper",
                format!("Failed to create HTTP client: {}", e),
                "Check your network configuration",
            )
        }
    };

    match client.get(&swagger_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                CheckResult::ok(
                    "Lakekeeper",
                    format!("Lakekeeper catalog is reachable at {}", endpoint),
                )
            } else {
                CheckResult::warn(
                    "Lakekeeper",
                    format!("Lakekeeper responded with status {}", resp.status()),
                    "Check Lakekeeper configuration",
                )
            }
        }
        Err(e) => {
            if e.is_connect() {
                CheckResult::error(
                    "Lakekeeper",
                    "Cannot connect to Lakekeeper catalog",
                    "Run: docker compose up -d lakekeeper",
                )
            } else {
                CheckResult::error(
                    "Lakekeeper",
                    format!("Lakekeeper check failed: {}", e),
                    "Check if Lakekeeper is running and accessible",
                )
            }
        }
    }
}

/// Check for external enrichment tools
async fn check_external_tools() -> Vec<CheckResult> {
    let tools = [
        (
            "ffprobe",
            "Video/audio metadata extraction",
            "Install FFmpeg: brew install ffmpeg",
        ),
        (
            "exiftool",
            "Image EXIF metadata extraction",
            "Install: brew install exiftool",
        ),
        (
            "pdfinfo",
            "PDF metadata extraction",
            "Install Poppler: brew install poppler",
        ),
    ];

    let mut results = Vec::new();

    for (tool, description, install_hint) in tools {
        let result = match Command::new("which").arg(tool).output() {
            Ok(output) => {
                if output.status.success() {
                    // Try to get version
                    let version_output = Command::new(tool)
                        .arg(if tool == "exiftool" {
                            "-ver"
                        } else {
                            "-version"
                        })
                        .output();

                    let version_info = version_output
                        .ok()
                        .and_then(|o| String::from_utf8(o.stdout).ok())
                        .map(|s| s.lines().next().unwrap_or("").to_string())
                        .unwrap_or_default();

                    CheckResult::ok(
                        tool,
                        format!(
                            "{} - {}",
                            description,
                            if version_info.is_empty() {
                                "installed".to_string()
                            } else {
                                version_info.trim().to_string()
                            }
                        ),
                    )
                } else {
                    CheckResult::warn(
                        tool,
                        format!("{} - not installed", description),
                        install_hint,
                    )
                }
            }
            Err(_) => CheckResult::warn(
                tool,
                format!("{} - not installed", description),
                install_hint,
            ),
        };

        results.push(result);
    }

    results
}

/// Check if required ports are available
async fn check_ports() -> Vec<CheckResult> {
    use std::net::TcpListener;

    let ports = [
        (8100, "Lakekeeper Catalog"),
        (8200, "RustFS API"),
        (8210, "RustFS Console"),
        (8300, "Postgres Backend"),
    ];

    let mut results = Vec::new();

    for (port, description) in ports {
        match TcpListener::bind(format!("127.0.0.1:{}", port)) {
            Ok(_) => {
                // Port is free (or we were able to bind to it)
                // However, if the docker container is already running, bind will fail.
                // We want to detect if *other* processes are using it.
                results.push(CheckResult::ok(
                    format!("Port {}", port),
                    format!("{} is available or in use by our stack", description),
                ));
            }
            Err(_) => {
                // Let's check if it's reachable via HTTP - if it is, maybe it's already our stack?
                results.push(CheckResult::warn(
                    format!("Port {}", port),
                    format!("{} is busy", description),
                    "This is OK if the stack is already running. If not, another process is using this port.",
                ));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_outcome_all_ok() {
        let results = vec![
            CheckResult::ok("Docker", "running"),
            CheckResult::ok("RustFS", "reachable"),
        ];
        assert!(doctor_outcome(&results).is_ok());
    }

    #[test]
    fn doctor_outcome_warning_only() {
        let results = vec![
            CheckResult::ok("Docker", "running"),
            CheckResult::warn("exiftool", "not installed", "brew install exiftool"),
        ];
        assert!(doctor_outcome(&results).is_ok());
    }

    #[test]
    fn doctor_outcome_has_error() {
        let results = vec![
            CheckResult::ok("Docker", "running"),
            CheckResult::error("RustFS", "cannot connect", "run docker compose up"),
        ];
        let err = doctor_outcome(&results).unwrap_err();
        assert!(err.to_string().contains("failed"));
    }
}
