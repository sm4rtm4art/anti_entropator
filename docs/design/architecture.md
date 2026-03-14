# Anti-Entropator Architecture

## Overview

Anti-Entropator is a **local data lakehouse** for file organization. It transforms a chaotic downloads folder into a queryable, organized data store using modern data engineering patterns.

## Architecture Diagram

```mermaid
flowchart LR
    subgraph UL["🖥️  User Layer"]
        direction TB
        CLI["⌨️  CLI (clap)"]
        REPL["💬  SQL REPL (rustyline)"]
    end

    subgraph OR["🔀  Orchestration"]
        direction TB
        SWITCH{{"⚙️  --engine"}}
        PROC["🔁  Procedural"]
        DFRS["🌊  dataflow-rs"]
        SWITCH --> PROC & DFRS
    end

    subgraph CP["⚡  Compute"]
        direction TB
        DF["🔥  DataFusion"]
        ICE["🧊  iceberg-rs"]
        CAT["🗂️  Lakekeeper"]
        PG[("🐘  Postgres")]
        ICE <--> CAT <--> PG
    end

    subgraph ST["🗄️  Object Storage"]
        direction TB
        IO["🔌  OpenDAL"]
        RFS["🦀  RustFS<br/><small>Parquet · Iceberg · Blobs</small>"]
        IO -->|S3 API| RFS
    end

    subgraph LZ["📂  Landing Zone"]
        Downloads["~/Downloads"]
    end

    CLI -->|"profile / scan"| Downloads
    CLI -->|"ingest / scan"| SWITCH
    CLI -->|query| DF
    REPL --> DF

    PROC & DFRS -->|"raw bytes"| IO
    PROC & DFRS -->|"commit snapshot"| ICE

    DF -->|"read / write"| IO
    ICE -->|manifests| IO
```

> **Note:** The dual-engine orchestration (procedural + dataflow-rs) is planned for v0.3.0 M4. The unified OpenDAL I/O boundary is implemented (M1 complete). See [Roadmap v0.3.0](../ROADMAP-v0.3.0.md) for details.

## Component Responsibilities

### CLI Layer

- **clap**: Command parsing and help generation
- **rustyline**: Interactive SQL REPL with history

### Core Commands

- **profile**: Read-only directory analysis (no Docker needed)
- **doctor**: Verify stack health and external tools
- **init**: Initialize lakehouse (bucket, warehouse, Iceberg table)
- **up**: Verify lakehouse services are running
- **scan**: Enrich file metadata without uploading
- **ingest**: Upload to RustFS + commit to Iceberg via Lakekeeper
- **query**: Execute SQL via DataFusion

### Orchestration Layer (v0.3.0)

- **Procedural engine**: Current sequential pipeline (default)
- **dataflow-rs engine**: Optional DAG-based orchestration behind `--engine dataflow`

### I/O Layer

- **OpenDAL**: Unified storage abstraction for all reads/writes/list/head/delete
- **object_store_opendal**: Adapter bridging DataFusion to OpenDAL (registered under `s3://`)

### Storage Layer

- **RustFS**: S3-compatible object storage (replaces MinIO)
- **Parquet**: Columnar file format with ZSTD compression
- **Iceberg**: Table format with schema evolution and time travel

### Catalog Layer

- **Lakekeeper**: Apache Iceberg REST Catalog (no JVM)
- **Postgres**: Backend storage for Lakekeeper catalog state

## Data Flow

### Ingest Pipeline

```mermaid
sequenceDiagram
    participant User
    participant CLI
    participant Engine as Engine<br/>(procedural / dataflow-rs)
    participant Scanner
    participant Hasher
    participant OpenDAL
    participant RustFS
    participant Lakekeeper

    User->>CLI: ingest ~/Downloads
    CLI->>Engine: dispatch pipeline
    Engine->>Scanner: traverse directory
    Scanner->>Hasher: compute SHA-256
    Hasher->>OpenDAL: put(sha256/ab/cd/hash)
    OpenDAL->>RustFS: S3 PutObject
    RustFS-->>Engine: object URI
    Engine->>Lakekeeper: append row to file_catalog (Iceberg snapshot commit)
    User->>CLI: query (validate snapshot / time travel)
```

> **Current state:** The procedural engine routes all I/O through OpenDAL (`src/storage/mod.rs`). DataFusion reads via `object_store_opendal`.

### Content-Addressed Storage

Files are stored with keys derived from their content hash:

```
s3://anti-entropator/warehouse/
└── sha256/
    ├── ab/
    │   └── cd/
    │       └── abcd1234...5678  (actual file bytes)
    └── ef/
        └── gh/
            └── efgh9012...3456
```

Benefits:

- **Idempotent uploads**: Same file always gets same key
- **Natural deduplication**: Identical files share storage
- **Verifiable**: Key proves content integrity

## Configuration

Configuration is loaded from (in order):

1. `--config` CLI flag
2. `ANTI_ENTROPATOR_CONFIG` environment variable
3. `./anti_entropator.toml`
4. `~/.config/anti_entropator/config.toml`

## Error Handling

Following the project's Rust rules:

- **Library code**: Uses `thiserror` for typed errors
- **Binary/CLI**: Uses `anyhow` for ergonomic error handling
- **No `.unwrap()` on I/O**: All filesystem operations use `?` or `match`

## Safety Guarantees

1. **Local files never deleted by default**: Ingest only copies, doesn't move
2. **Dry-run mandatory**: Every mutation has `--dry-run` / `--apply` flags
3. **Time travel workflow**: Every ingest produces an Iceberg snapshot that can be queried/rolled back
4. **Atomic operations**: Iceberg commits are all-or-nothing
