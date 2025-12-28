# Anti-Entropator

> _Fighting entropy, one file at a time_

A **local data lakehouse** for file organization, built in Rust. Transform your chaotic downloads folder into a queryable, organized data store using modern data engineering patterns.

## Why?

Your downloads folder is a data swamp. This project turns it into a lakehouse by:

- **Cataloging** every file with rich metadata (type, size, hash, MIME)
- **Detecting duplicates** via content hashing
- **Organizing** files by category with safe, reversible operations
- **Querying** your catalog with SQL

## Architecture

```mermaid
flowchart TD
    subgraph client [Local Host - Rust CLI]
        CLI[Anti-Entropator CLI]
        DF[Apache DataFusion]
    end

    subgraph stack [Docker Compose Stack]
        subgraph catalog [Metadata Layer]
            Nessie[Nessie Catalog]
            Iceberg[Apache Iceberg]
        end

        subgraph storage [Storage Layer]
            RustFS[(RustFS S3)]
        end
    end

    User((User)) --> CLI
    CLI --> DF
    DF <--> RustFS
    DF <--> Nessie
    Nessie -.-> Iceberg
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

### 3. Query your catalog

```bash
# Interactive SQL
cargo run -- sql

# Find duplicates
cargo run -- query "
  SELECT sha256, COUNT(*) as copies
  FROM file_catalog
  GROUP BY sha256
  HAVING COUNT(*) > 1
"
```

## Features

| Feature      | Status | Description                  |
| ------------ | ------ | ---------------------------- |
| `profile`    | ✅     | Read-only directory analysis |
| `doctor`     | ✅     | Stack health checks          |
| `scan`       | 🔄     | Metadata enrichment          |
| `ingest`     | 🔄     | Upload to object storage     |
| `query`      | 🔄     | SQL via DataFusion           |
| `duplicates` | 🔄     | Find duplicate files         |

## Stack Components

- **[RustFS](https://github.com/rustfs/rustfs)**: S3-compatible object storage (Apache 2.0)
- **[Apache Iceberg](https://iceberg.apache.org/)**: Table format with time travel
- **[Nessie](https://projectnessie.org/)**: Git-like catalog for data
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
