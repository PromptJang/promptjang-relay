FROM node:22-bookworm-slim AS web
WORKDIR /src/web
COPY web/package*.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.89-bookworm AS rust
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY migrations ./migrations
COPY docs ./docs
COPY src ./src
RUN cargo build --release --bin promptjang-relay

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust /src/target/release/promptjang-relay /usr/local/bin/promptjang-relay
COPY --from=web /src/web/dist /app/ui
ENV PJ_BIND=0.0.0.0:8080 PJ_STATIC_DIR=/app/ui
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --start-period=20s --retries=5 CMD curl -fsS http://127.0.0.1:8080/ready || exit 1
ENTRYPOINT ["promptjang-relay"]
