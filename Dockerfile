FROM rust:1.92-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
    

WORKDIR /app
COPY . .

ENV SQLX_OFFLINE=true

RUN cargo build --release 

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*


COPY --from=builder /app/assets ./assets
COPY --from=builder /app/target/release/mill-valley-sausalito-bikepath-flood-alert ./flood-alert

ENTRYPOINT ["./flood-alert"]
