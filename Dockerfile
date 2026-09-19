# syntax=docker/dockerfile:1.7

FROM rustlang/rust:nightly-bookworm AS builder

WORKDIR /src
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.128 --locked

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY public ./public

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --package zhorten --lib --release \
      --target-dir target/front --target wasm32-unknown-unknown \
      --no-default-features --features hydrate \
    && mkdir -p /output/site/pkg \
    && wasm-bindgen --target web --out-dir /output/site/pkg --out-name zhorten \
      target/front/wasm32-unknown-unknown/release/zhorten.wasm \
    && cp public/style.css /output/site/style.css \
    && cargo build --locked --package zhorten --bin zhorten --release \
      --no-default-features --features ssr \
    && cp target/release/zhorten /output/zhorten

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home zhorten \
    && mkdir -p /app/site /data \
    && chown -R zhorten:zhorten /app /data

COPY --from=builder --chown=zhorten:zhorten /output/zhorten /app/zhorten
COPY --from=builder --chown=zhorten:zhorten /output/site /app/site

USER zhorten
WORKDIR /app
ENV ZHORTEN_ADDR=0.0.0.0:3000 \
    ZHORTEN_DB=/data/zhorten.db \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_OUTPUT_NAME=zhorten
VOLUME ["/data"]
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:3000/ > /dev/null || exit 1
ENTRYPOINT ["/app/zhorten"]

