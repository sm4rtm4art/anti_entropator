# Getting Started with Anti-Entropator

## Prerequisites

- **Rust 1.88+**: Install via [rustup](https://rustup.rs/)
- **Docker**: For running the lakehouse stack
- **Optional tools**: `ffprobe`, `exiftool`, `pdfinfo` for richer metadata

## Installation

### From Source

```bash
git clone https://github.com/YOUR_USERNAME/anti_entropator.git
cd anti_entropator
cargo build --release
```

The binary will be at `target/release/anti_entropator`.

### Add to PATH

```bash
# Add to ~/.zshrc or ~/.bashrc
export PATH="$PATH:/path/to/anti_entropator/target/release"
```

## Quick Start

### 1. Profile Your Downloads (No Docker Needed)

First, understand what you're dealing with:

```bash
anti_entropator profile ~/Downloads
```

This shows:

- File type distribution (by extension and MIME)
- Size statistics with percentiles
- Duplicate estimation
- Name quality patterns (generic names, UUIDs, etc.)

### 2. Start the Lakehouse Stack

```bash
# Create data directories with correct permissions
mkdir -p data/rustfs logs/rustfs data/postgres
chown -R 10001:10001 data/rustfs logs/rustfs

# Start services
docker compose up -d

# Verify everything is running
anti_entropator doctor
```

### 3. Initialize the Catalog

```bash
anti_entropator init
```

This creates:

- The `anti-entropator` bucket in RustFS
- The `file_catalog` Iceberg table in Nessie

### 4. Ingest Files

```bash
# Preview what would be ingested
anti_entropator ingest ~/Downloads --dry-run

# Actually ingest (creates a branch for review)
anti_entropator ingest ~/Downloads
```

### 5. Query Your Catalog

```bash
# Interactive SQL REPL
anti_entropator sql

# One-shot queries
anti_entropator query "SELECT category, COUNT(*) FROM file_catalog GROUP BY category"
```

### 6. Find Duplicates

```bash
anti_entropator duplicates
```

## Command Reference

| Command          | Description                              |
| ---------------- | ---------------------------------------- |
| `profile <path>` | Analyze directory (read-only, no Docker) |
| `doctor`         | Check stack health and external tools    |
| `up`             | Verify lakehouse services are running    |
| `init`           | Initialize bucket and Iceberg table      |
| `scan <path>`    | Enrich metadata without uploading        |
| `ingest <path>`  | Upload files and commit to catalog       |
| `sql`            | Interactive SQL REPL                     |
| `query <sql>`    | Execute one-shot SQL                     |
| `duplicates`     | Find and report duplicate files          |
| `merge <branch>` | Merge ingest branch to main              |

## Environment Variables

| Variable                          | Default                          | Description       |
| --------------------------------- | -------------------------------- | ----------------- |
| `ANTI_ENTROPATOR_S3_ENDPOINT`     | `http://localhost:9000`          | RustFS endpoint   |
| `ANTI_ENTROPATOR_NESSIE_ENDPOINT` | `http://localhost:19120/api/v1`  | Nessie API        |
| `ANTI_ENTROPATOR_BUCKET`          | `anti-entropator`                | S3 bucket name    |
| `ANTI_ENTROPATOR_WAREHOUSE`       | `s3://anti-entropator/warehouse` | Iceberg warehouse |

## Troubleshooting

### "Docker daemon is not running"

Start Docker Desktop or:

```bash
sudo systemctl start docker
```

### "Cannot connect to RustFS"

```bash
docker compose up -d rustfs
docker compose logs rustfs
```

### Permission denied on RustFS volumes

RustFS runs as UID 10001:

```bash
chown -R 10001:10001 data/rustfs logs/rustfs
```

### External tools not detected

Install optional enrichment tools:

```bash
# macOS
brew install ffmpeg exiftool poppler

# Ubuntu
apt install ffmpeg libimage-exiftool-perl poppler-utils
```
