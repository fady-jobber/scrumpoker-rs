FROM rust:1.83-slim as builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/scrumpoker-rs /app/scrumpoker-rs
COPY templates ./templates
COPY public ./public

ENV ROCKET_ADDRESS=0.0.0.0

EXPOSE 8080

CMD sh -c "ROCKET_PORT=${PORT:-8080} /app/scrumpoker-rs"
