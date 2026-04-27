# syntax=docker/dockerfile:1.6
# ----- Stage 1: build -----
# Rust 1.95 — latest stable as of 2026-04. Required because some transitive
# deps (e.g. getrandom) now use Cargo's edition2024 manifest feature
# (stabilized in Rust 1.85).
FROM rust:1.95-slim-bookworm AS builder

# protoc + build deps for tonic + native TLS roots compile
RUN apt-get update && apt-get install -y --no-install-recommends \
        protobuf-compiler \
        pkg-config \
        libssl-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache deps: copy only manifests first for better layer caching.
COPY Cargo.toml Cargo.lock* ./
COPY crates/ocpp-protocol/Cargo.toml        crates/ocpp-protocol/Cargo.toml
COPY crates/ocpp-transport/Cargo.toml       crates/ocpp-transport/Cargo.toml
COPY crates/ocpp-store/Cargo.toml           crates/ocpp-store/Cargo.toml
COPY crates/ocpp-adapter/Cargo.toml         crates/ocpp-adapter/Cargo.toml
COPY crates/ocpp-internal-mqtt/Cargo.toml   crates/ocpp-internal-mqtt/Cargo.toml
COPY crates/ocpp-internal-grpc/Cargo.toml   crates/ocpp-internal-grpc/Cargo.toml
COPY crates/ocpp-gateway/Cargo.toml         crates/ocpp-gateway/Cargo.toml

# Now copy real sources and build (cache deps via BuildKit mounts).
COPY crates ./crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p ocpp-gateway && \
    cp /app/target/release/ocpp-gateway /usr/local/bin/ocpp-gateway

# ----- Stage 2: runtime -----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 ocpp

WORKDIR /app
COPY --from=builder /usr/local/bin/ocpp-gateway /usr/local/bin/ocpp-gateway

# Pre-create the data directory owned by the unprivileged user so the
# named volume mounted at /app/data inherits the right ownership.
RUN mkdir -p /app/data && chown -R ocpp:ocpp /app

USER ocpp
ENV RUST_LOG=info
EXPOSE 50051

ENTRYPOINT ["ocpp-gateway"]
CMD ["--config", "/app/gateway.yaml"]
