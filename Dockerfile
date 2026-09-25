# syntax=docker/dockerfile:1

FROM rust:bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --uid 10001 chainwatch
WORKDIR /app
COPY --from=builder /app/target/release/chainwatch-rs /usr/local/bin/chainwatch-rs
USER chainwatch
ENV RUST_LOG=info
EXPOSE 8080
CMD ["chainwatch-rs"]
