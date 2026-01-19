# Getting Started with Anti-Entropator

## Prerequisites

- **Rust 1.85+**: Install via [rustup](https://rustup.rs/)
- **Docker**: For running the lakehouse stack
- **Optional tools**: `ffprobe`, `exiftool`, `pdfinfo` for richer metadata extraction

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

### 3. Initialize the Lakehouse

```bash
anti_entropator init
```

This creates:

- The `anti-entropator` bucket in RustFS (S3-compatible storage)
- The `anti-entropator` warehouse in Lakekeeper
- The `anti_entropator` Iceberg namespace
- The `file_catalog` Iceberg table (with schema for file metadata)

The command is idempotent - run it multiple times safely.

### 4. Ingest Files

```bash
# Preview what would be ingested
anti_entropator ingest ~/Downloads --dry-run

# Actually ingest (uploads to object storage)
anti_entropator ingest ~/Downloads
```

### 5. Query Your Catalog (In Development)

> **Note:** The `sql` and `query` commands are in development. They will enable SQL queries via DataFusion once Iceberg catalog integration is complete.

```bash
# Interactive SQL REPL (coming soon)
anti_entropator sql

# One-shot queries (coming soon)
anti_entropator query "SELECT category, COUNT(*) FROM file_catalog GROUP BY category"
```

### 6. Find Duplicates (In Development)

> **Note:** The `duplicates` command is in development.

```bash
anti_entropator duplicates
```

## Command Reference

| Command          | Status | Description                              |
| ---------------- | ------ | ---------------------------------------- |
| `profile <path>` | ✅      | Analyze directory (read-only, no Docker) |
| `doctor`         | ✅      | Check stack health and external tools    |
| `up`             | ✅      | Verify lakehouse services are running    |
| `init`           | ✅      | Initialize lakehouse (bucket, warehouse, table) |
| `scan <path>`    | ✅      | Enrich metadata without uploading        |
| `ingest <path>`  | ✅      | Upload files to object storage           |
| `sql`            | 🚧      | Interactive SQL REPL (in development)    |
| `query <sql>`    | 🚧      | Execute one-shot SQL (in development)    |
| `duplicates`     | 🚧      | Find duplicate files (in development)    |
| `merge <branch>` | 🚧      | Merge ingest branch (in development)     |

**Legend:** ✅ Implemented | 🚧 In Development

## Environment Variables

| Variable                           | Default                  | Description              |
| ---------------------------------- | ------------------------ | ------------------------ |
| `ANTI_ENTROPATOR_S3_ENDPOINT`      | `http://localhost:19000` | RustFS endpoint          |
| `ANTI_ENTROPATOR_CATALOG_ENDPOINT` | `http://localhost:8181`  | Lakekeeper API           |
| `ANTI_ENTROPATOR_BUCKET`           | `anti-entropator`        | S3 bucket name           |
| `ANTI_ENTROPATOR_WAREHOUSE`        | `anti-entropator`        | Lakekeeper warehouse name|

RustFS credentials are read from (first match wins):

- `RUSTFS_ACCESS_KEY` / `RUSTFS_SECRET_KEY`
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`

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
