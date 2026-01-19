# Anti-Entropator

[![CI](https://github.com/sm4rtm4rt/anti_entropator/actions/workflows/ci.yml/badge.svg)](https://github.com/martinkaergell/anti_entropator/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org)

> _Fighting entropy, one file at a time_

A **local data lakehouse** for file organization, built in Rust. Transform your chaotic downloads folder into a queryable, organized data store using modern data engineering patterns.

## Why?

Your downloads folder is a data swamp. This project turns it into a lakehouse by:

- **Cataloging** every file with rich metadata (type, size, hash, MIME)
- **Detecting duplicates** via content hashing
- **Organizing** files by category with safe, reversible operations
- **Querying** your catalog with SQL (planned)

## Architecture

```mermaid
flowchart TD
    subgraph client [Local Host - Rust CLI]
        CLI[Anti-Entropator CLI]
        DF[Apache DataFusion]
    end

    subgraph stack [Docker Compose Stack]
        subgraph catalog [Catalog Layer]
            Lakekeeper[Lakekeeper Catalog]
            Iceberg[Apache Iceberg]
        end

        subgraph storage [Storage Layer]
            RustFS[(RustFS S3)]
        end
    end

    User((User)) --> CLI
    CLI --> DF
    DF <--> RustFS
    DF <--> Lakekeeper
    Lakekeeper -.-> Iceberg
    Iceberg -.-> RustFS
```

## Quick Start

### 1. Profile your downloads (no Docker needed)

```bash
cargo run --release -- profile ~/Downloads
```

Output:

```
═══════════════════════════════════════════════════════════════
  📊 Anti-Entropator Swamp Profile
═══════════════════════════════════════════════════════════════

  Path: /Users/you/Downloads
  Files: 4,548 | Dirs: 111 | Total size: 4.49 GiB

─── By Extension (top 25 by total size) ───────────────────────
╭───────────┬───────┬───────────┬──────────┬───────────╮
│ Extension │ Count │ Total     │ Avg      │ Max       │
├───────────┼───────┼───────────┼──────────┼───────────┤
│ .mp4      │ 811   │ 2.05 GiB  │ 2.59 MiB │ 53.81 MiB │
│ .pdf      │ 465   │ 1.11 GiB  │ 2.44 MiB │ 99.66 MiB │
...
```

### 2. Start the lakehouse stack

```bash
# Create directories with correct permissions
mkdir -p data/rustfs logs/rustfs data/postgres
chown -R 10001:10001 data/rustfs logs/rustfs

# Start services
docker compose up -d

# Verify health
cargo run -- doctor
```

### 3. Scan and ingest files

```bash
# Scan & enrich metadata (read-only)
cargo run -- scan ~/Downloads --dry-run

# Upload to object storage
cargo run -- ingest ~/Downloads --dry-run

# Remove --dry-run to actually upload
cargo run -- ingest ~/Downloads
```

## Features

| Feature      | Status | Description                                  |
| ------------ | ------ | -------------------------------------------- |
| `profile`    | ✅      | Read-only directory analysis                 |
| `doctor`     | ✅      | Stack health checks                          |
| `scan`       | ✅      | Metadata enrichment with external tools      |
| `ingest`     | ✅      | Upload to S3-compatible object storage       |
| `init`       | ✅      | Initialize lakehouse (bucket, warehouse, table) |
| `up`         | ✅      | Verify lakehouse services are running        |
| `query`      | 🚧      | SQL via DataFusion (in development)          |
| `sql`        | 🚧      | Interactive SQL REPL (in development)        |
| `duplicates` | 🚧      | Find duplicate files (in development)        |

**Legend:** ✅ Implemented | 🚧 In Development

## Stack Components

- **[RustFS](https://github.com/rustfs/rustfs)**: S3-compatible object storage (Apache 2.0)
- **[Lakekeeper](https://github.com/lakekeeper/lakekeeper)**: Apache Iceberg REST Catalog (Rust)
- **[Apache Iceberg](https://iceberg.apache.org/)**: Table format with time travel
- **[DataFusion](https://datafusion.apache.org/)**: SQL query engine

## Documentation

- [Getting Started](docs/manual/getting-started.md)
- [Architecture](docs/design/architecture.md)
- [ADRs](docs/adr/) - Why we made these technology choices

## Project Goals

1. **Clean my downloads folder** - Practical utility
2. **Learn Rust deeply** - Systems programming skills
3. **Demonstrate lakehouse patterns** - Portfolio piece
4. **Teach others** - Good documentation

## License

MIT
