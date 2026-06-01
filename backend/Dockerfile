FROM rust:1.95 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin wara-backend --bin temporal_worker

FROM debian:bookworm-slim
# hadolint ignore=DL3008
# Versions are intentionally unpinned: these are stable base utilities from the
# debian:bookworm-slim repositories, and pinning them breaks on routine security
# updates to the base image.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssh-client docker-cli && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/wara-backend /usr/local/bin/wara-backend
COPY --from=builder /app/target/release/temporal_worker /usr/local/bin/temporal_worker
CMD ["wara-backend"]

