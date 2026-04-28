# UCFP Rust server — multistage build for Cloudflare Containers (or any Linux host).
#
# Build:    docker build -t ucfp:latest .
# Run:      docker run -p 8080:8080 \
#               -e UCFP_TOKEN=changeme \
#               -e UCFP_BIND=0.0.0.0:8080 \
#               -v ucfp-data:/data \
#               ucfp:latest
#
# Cloudflare Containers binds storage via R2; pair with scripts/redb-snapshot.sh
# (sidecar cron) to back up /data/ucfp.redb to R2 every N minutes.

# ── Builder ────────────────────────────────────────────────────────────────
FROM rust:latest AS builder

WORKDIR /build

# Cache deps separately from source.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin && \
    echo 'fn main() {}' > src/bin/ucfp.rs && \
    echo '' > src/lib.rs && \
    cargo fetch --locked

# Now bring in real source.
COPY src ./src
COPY docs ./docs

# Build with the production feature umbrella.
RUN cargo build --release --features full --bin ucfp

# ── Runtime ────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

RUN useradd --system --uid 1001 --home /home/ucfp --create-home ucfp
USER ucfp

WORKDIR /home/ucfp
COPY --from=builder /build/target/release/ucfp /usr/local/bin/ucfp

ENV UCFP_BIND=0.0.0.0:8080 \
    UCFP_DATA_DIR=/data

VOLUME ["/data"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s \
    CMD wget -qO- http://127.0.0.1:8080/healthz || exit 1

ENTRYPOINT ["/usr/local/bin/ucfp"]
