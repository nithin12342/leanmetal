# EdgeFlag production image (Linux, Valkey, no Redis).
# Builder caches deps first; runtime is non-root + HEALTHCHECK on /health.
FROM rust:1-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential clang llvm libclang-dev libssl-dev pkg-config cmake \
    iproute2 libelf-dev libxdp-dev zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/edgeflag
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN mkdir -p tests && cargo fetch
COPY . .
# Optional space-separated cargo features for verification builds,
# e.g. --build-arg RUST_FEATURES="af-xdp". Default image builds clean.
ARG RUST_FEATURES=""
RUN if [ -z "$RUST_FEATURES" ]; then cargo build --release --locked; else cargo build --release --locked --features "$RUST_FEATURES"; fi

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates curl libxdp1 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 10001 appuser

WORKDIR /app
COPY --from=builder /usr/src/edgeflag/target/release/edgeflag /app/edgeflag
RUN mkdir -p /app/data && chown -R appuser:appuser /app
USER appuser

ENV PORT=8080
ENV EDGEFLAG_DATA_DIR=/app/data
ENV RUST_LOG=info
EXPOSE 8080
VOLUME ["/app/data"]
HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
  CMD curl -sf http://127.0.0.1:8080/health || exit 1
CMD ["/app/edgeflag"]
