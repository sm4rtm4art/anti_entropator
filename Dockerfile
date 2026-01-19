# Anti-Entropator Container Image
# Multi-arch build - detects host architecture

# Build stage
FROM rust:latest AS builder

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

# Runtime stage - minimal Debian
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/anti_entropator /usr/local/bin/

# Non-root user
RUN useradd -m -u 1000 anti
USER anti

ENTRYPOINT ["anti_entropator"]
CMD ["--help"]
