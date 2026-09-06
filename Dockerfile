# Anti-Entropator Container Image
# Single-arch build by default (multi-arch planned in S5).

# Build stage: keep explicit Rust tag on Bookworm to match runtime glibc.
# Bump alongside Rust dependency/MSRV needs, then rerun S5-B image validation.
FROM rust:1.98-bookworm AS builder

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

# Runtime stage - default Bookworm baseline
FROM debian:bookworm-slim AS runtime-bookworm

RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get -y upgrade \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/anti_entropator /usr/local/bin/

# Non-root user
RUN useradd -m -u 1000 anti
USER anti

ENTRYPOINT ["anti_entropator"]
CMD ["--help"]

# Runtime stage - distroless experiment candidate (S5-D B1)
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime-distroless

COPY --from=builder /app/target/release/anti_entropator /usr/local/bin/

ENTRYPOINT ["/usr/local/bin/anti_entropator"]
CMD ["--help"]

# Keep default local/CI image on Bookworm until S5-D chooses a baseline.
FROM runtime-bookworm AS runtime
