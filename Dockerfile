# @faridguzman91: Multi-stage Dockerfile for the engage relay server.
#
# Stage 1 — builder: compile the Rust binary with musl for a fully static binary.
# Stage 2 — runtime: minimal Debian-slim image (~30 MB) with just the binary.
#
# Build:  docker build -t engage-server .
# Run:    docker run -p 3000:3000 --env-file .env engage-server

# ── Stage 1: builder ──────────────────────────────────────────────────────────
FROM rust:1.96-slim-bookworm AS builder

WORKDIR /build

# Install musl tools for a statically linked binary
RUN apt-get update && apt-get install -y --no-install-recommends \
    musl-tools \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

# Cache dependencies — copy manifests first so Docker layer caches the build
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl \
    && rm -rf src

# Build the real binary
COPY src ./src
RUN touch src/main.rs \
    && cargo build --release --target x86_64-unknown-linux-musl

# ── Stage 2: runtime ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# @faridguzman91: curl is only present for the Docker healthcheck — remove if not needed
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --no-create-home --shell /bin/false engage

COPY --from=builder \
    /build/target/x86_64-unknown-linux-musl/release/engage-server \
    /usr/local/bin/engage-server

RUN mkdir /data && chown engage:engage /data

USER engage
WORKDIR /data

EXPOSE 3000

ENV PORT=3000
ENV DATABASE_PATH=/data/engage-server.db
ENV RUST_LOG=engage_server=info,tower_http=warn

ENTRYPOINT ["/usr/local/bin/engage-server"]
