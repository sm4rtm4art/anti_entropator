# Getting Started with Anti-Entropator

## Prerequisites

- **Rust 1.85+**: Install via [rustup](https://rustup.rs/)
- **Docker**: For running the lakehouse stack
- **Optional tools**: `ffprobe`, `exiftool`, `pdfinfo` for richer metadata extraction

## Installation

### From Source

```bash
git clone https://github.com/sm4rtm4art/anti_entropator.git
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
# Create local environment file from template
cp env.example .env

# Edit .env and replace all CHANGE_ME values first

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
- A Lakekeeper project (ID persisted to the platform data directory)
- The `anti-entropator` warehouse in Lakekeeper
- The `anti_entropator` Iceberg namespace
- The `file_catalog` Iceberg table (with schema for file metadata)

The command is idempotent - run it multiple times safely. The project ID is stored in the platform data directory (`~/.local/share/anti_entropator/` on Linux, `~/Library/Application Support/` on macOS) so subsequent commands (ingest, query) reuse the same project. A legacy `.lakehouse_state.json` in the working directory is also checked as a fallback.

### 4. Ingest Files

```bash
# Preview what would be ingested
anti_entropator ingest ~/Downloads --dry-run

# Actually ingest (uploads to object storage)
anti_entropator ingest ~/Downloads
```

### 5. Query Your Catalog

> **Note:** `query` is implemented as a one-shot command.
> `sql` exits with an error indicating the interactive REPL is planned but not yet implemented.

```bash
# One-shot query (basic implementation)
anti_entropator query "SELECT category, COUNT(*) FROM file_catalog GROUP BY category"

# Interactive SQL REPL (placeholder)
anti_entropator sql
```

### 6. Find Duplicates (Placeholder)

> **Note:** The `duplicates` command is a placeholder and does not execute duplicate handling yet.

```bash
anti_entropator duplicates
```

### 7. Merge Branches (Placeholder)

> **Note:** The `merge` command is currently a placeholder command.

```bash
anti_entropator merge
```

## Command Reference

| Command          | Status | Description                                  |
| ---------------- | ------ | -------------------------------------------- |
| `profile <path>` | ✅      | Analyze directory (read-only, no Docker)     |
| `doctor`         | ✅      | Check stack health and external tools        |
| `up`             | ✅      | Verify lakehouse services are running        |
| `init`           | ✅      | Initialize lakehouse (bucket, warehouse, table) |
| `scan <path>`    | ✅      | Enrich metadata without uploading            |
| `ingest <path>`  | ✅      | Upload files & commit metadata to Iceberg    |
| `query <sql>`    | ✅      | Execute one-shot SQL via DataFusion (basic)  |
| `sql`              | 🚧      | Interactive SQL REPL (planned, not yet implemented)      |
| `duplicates`       | 🚧      | Duplicate finder workflow (planned, not yet implemented) |
| `merge`            | 🚧      | Ingest branch merge workflow (planned, not yet implemented) |

**Legend:** ✅ Implemented | 🚧 Planned (not yet implemented)

## Environment Variables

| Variable                           | Default                  | Description              |
| ---------------------------------- | ------------------------ | ------------------------ |
| `ANTI_ENTROPATOR_S3_ENDPOINT`      | `http://localhost:8200`  | RustFS endpoint          |
| `ANTI_ENTROPATOR_CATALOG_ENDPOINT` | `http://localhost:8100`  | Lakekeeper API           |
| `ANTI_ENTROPATOR_S3_REGION`        | `eu-central-1`           | S3 signing/storage region |
| `ANTI_ENTROPATOR_BUCKET`           | `anti-entropator`        | S3 bucket name           |
| `ANTI_ENTROPATOR_WAREHOUSE`        | `anti-entropator`        | Lakekeeper warehouse name|
| `ANTI_ENTROPATOR_PROJECT_ID`       | _(auto-generated)_       | Lakekeeper project UUID (override auto-detection) |

RustFS credentials are read from (first match wins):

- `RUSTFS_ACCESS_KEY` / `RUSTFS_SECRET_KEY`
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`

For local development only:

- `LAKEKEEPER_AUTHZ_BACKEND=allowall`

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
