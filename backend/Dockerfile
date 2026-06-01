FROM rust:1.95 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin wara-backend --bin temporal_worker --bin wara-migrate

FROM debian:bookworm-slim
# Versions intentionally unpinned: stable base utilities from debian:bookworm-slim;
# pinning them breaks on routine base-image security updates.
# hadolint ignore=DL3008
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssh-client docker-cli && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/wara-backend /usr/local/bin/wara-backend
COPY --from=builder /app/target/release/temporal_worker /usr/local/bin/temporal_worker
COPY --from=builder /app/target/release/wara-migrate /usr/local/bin/wara-migrate
# Migration files + config so `wara-migrate migration apply` works at runtime.
WORKDIR /app
COPY --from=builder /app/Toasty.toml /app/Toasty.toml
COPY --from=builder /app/toasty /app/toasty
CMD ["wara-backend"]

