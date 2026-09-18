# Getting Started with Anti-Entropator

## Prerequisites

- **Rust 1.94+**: Install via [rustup](https://rustup.rs/)
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

# Create local directories (RustFS data lives in a named Docker volume)
mkdir -p logs/rustfs data/postgres
chown -R 10001:10001 logs/rustfs

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
# Dry run: read catalog state, check which blobs exist, upload nothing
anti_entropator ingest ~/Downloads --dry-run

# Offline dry run: list candidates without contacting the lakehouse at all
anti_entropator ingest ~/Downloads --dry-run --offline

# Machine-readable summary (one JSON document on stdout) works in every mode
anti_entropator ingest ~/Downloads --dry-run --format json

# Actually ingest (uploads to object storage + commits to the catalog)
anti_entropator ingest ~/Downloads

# Name the source so it survives moving the folder
anti_entropator ingest ~/Downloads --source downloads
```

Ingest has three modes; `--offline` requires `--dry-run`:

| Mode | Contacts lakehouse | Reads catalog state / checks blobs | Uploads / commits | JSON `mode` |
|---|---|---|---|---|
| `--dry-run` | yes (fails if unreachable) | yes — `unchanged` and `already_exists` are accurate | no | `dry_run` |
| `--dry-run --offline` | no | no — every candidate is "would observe" and "would upload" | no | `dry_run_offline` |
| default | yes | yes | yes | `ingest` |

`--format json` writes a versioned summary (`format_version` 3) to stdout with
`mode`, `run_id`, `source_id`, `status`, `catalog_commit`, and these counts:

| Count | Axis | Meaning |
|---|---|---|
| `candidates` | — | files selected by the filters |
| `observed` | paths | a `present` row was appended (new path, or content changed) |
| `unchanged` | paths | last observation already records this content; no row |
| `uploaded` | blobs | content uploaded to the store (`bytes` counts these) |
| `already_exists` | blobs | content was already stored and verified |

The axes are independent: a second copy of a file is `observed` with
`already_exists`, because the path is new but the bytes are not.
Human output remains the default.
Partial and complete failures exit non-zero in every mode; JSON mode keeps that exit status and prints the summary before the error on stderr.

What a `file_catalog` row means (ADR-009): one row is one observation of one
path in one source during one run, appended-only. Ingest reads the latest
observation per path for the source at run start and appends a row only when
a path is new or its content changed; re-running an unchanged ingest appends
nothing and attempts no commit. If the catalog cannot be read, the run stops
rather than guess.

- `source_id` defaults to the canonical absolute path of the ingested
  directory. Pass `--source <name>` to keep observing the same logical source
  after moving the folder; a different `source_id` is a different source and
  every path is observed again (blobs are not re-uploaded).
- Deleted and renamed files are not yet recorded: a rename appears as a new
  row at the new path and the old path's row stays. `observation_status` is
  always `present` today.

How an upload is made safe:

- Files are streamed in bounded chunks and re-hashed while they are sent; the
  object is stored under `sha256/<aa>/<bb>/<hash>` only if the bytes still
  match the hash from the scan. A file that changes mid-upload is aborted
  (nothing is stored), rescanned, and retried once before it counts as a
  per-file error.
- An object that already exists is verified against the local file (size, and
  the object's `sha256` metadata when present). A mismatch is reported as an
  error for that file; the stored object is never overwritten.

Tables created before the observation columns existed are upgraded in place
the next time you run `anti_entropator init`; until then `ingest` stops with
`run anti_entropator init to upgrade it`. Older rows keep `NULL` in the new
columns and are observed again once.

### 5. Query Your Catalog

> **Note:** `query` is a one-shot command; there is no interactive SQL mode.
>
> Lakekeeper registers Iceberg table `file_catalog` (namespace
> `anti_entropator`). The `query` command rewrites `FROM files` /
> `JOIN files` to `iceberg.anti_entropator.file_catalog`. Prefer the
> Iceberg name in docs; `files` is optional CLI sugar.

```bash
# One-shot query (shorthand)
anti_entropator query "SELECT category, COUNT(*) FROM files GROUP BY category"

# Same query with the canonical Iceberg table reference
anti_entropator query "SELECT category, COUNT(*) FROM iceberg.anti_entropator.file_catalog GROUP BY category"

# Duplicate content today: group by hash in SQL
anti_entropator query "SELECT content_hash, COUNT(*) AS copies FROM iceberg.anti_entropator.file_catalog GROUP BY content_hash HAVING COUNT(*) > 1"

# Current state of one source: the latest present observation per path
anti_entropator query "SELECT relative_path, content_hash, observed_at FROM (SELECT *, ROW_NUMBER() OVER (PARTITION BY relative_path ORDER BY observed_at DESC) AS rn FROM iceberg.anti_entropator.file_catalog WHERE source_id = 'downloads' AND observation_status = 'present') WHERE rn = 1"
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

Every command in the binary is implemented. Interactive SQL, a duplicate
workflow, and branch merge are roadmap items and do not exist as commands yet.

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

### Permission denied on the RustFS log directory

RustFS runs as UID 10001:

```bash
chown -R 10001:10001 logs/rustfs
```

### Upgrading from RustFS `1.0.0-beta.2` (bind mount) to `1.0.0` (named volume)

RustFS 1.0.0 does not start on a `1.0.0-beta.2` data directory and does not
support Docker Desktop bind mounts (it logs `Unsupported filesystem type ...
(FUSE)` and fails writes with `Bad file descriptor`). The stack therefore
moved RustFS data to the named volume `rustfs-data`. There is no in-place
migration; the catalog in Postgres references objects by path, so both must be
reset together:

```bash
docker compose down
rm -rf data/rustfs data/postgres   # local lakehouse contents are discarded
docker compose up -d
anti_entropator init                # recreates bucket, project, warehouse, table
anti_entropator ingest <path>       # re-ingest from your source directories
```

Source files are never modified by ingest, so a re-ingest rebuilds the
lakehouse from them. A full reset later is `docker compose down -v` plus
`rm -rf data/postgres`.

### External tools not detected

Install optional enrichment tools:

```bash
# macOS
brew install ffmpeg exiftool poppler

# Ubuntu
apt install ffmpeg libimage-exiftool-perl poppler-utils
```
