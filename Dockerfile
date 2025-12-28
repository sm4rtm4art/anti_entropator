# Anti-Entropator Container Image
# Multi-stage build for minimal image size

# Build stage
FROM rust:1.92 AS builder

WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source and rebuild
COPY src ./src
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/anti_entropator /usr/local/bin/

# Create non-root user
RUN useradd -m -u 1000 anti
USER anti

ENTRYPOINT ["anti_entropator"]
CMD ["--help"]

