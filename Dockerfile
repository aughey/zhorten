# syntax=docker/dockerfile:1.7

FROM rustlang/rust:nightly-bookworm AS builder

WORKDIR /src
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.128 --locked

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY public ./public

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --package zhorten-app --lib --release \
      --target-dir target/front --target wasm32-unknown-unknown \
      --no-default-features --features csr \
    && mkdir -p /output/site/assets/pkg \
    && wasm-bindgen --target web --out-dir /output/site/assets/pkg --out-name zhorten_app \
      target/front/wasm32-unknown-unknown/release/zhorten_app.wasm \
    && cp public/index.html /output/site/ \
    && cp public/assets/bootstrap.js public/assets/style.css public/assets/favicon.svg /output/site/assets/ \
    && cargo build --locked --package zhorten-server --bin zhorten --release \
    && cp target/release/zhorten /output/zhorten \
    && mkdir -p /output/data

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder --chown=10001:10001 /output/zhorten /app/zhorten
COPY --from=builder --chown=10001:10001 /output/site /app/site
COPY --from=builder --chown=10001:10001 /output/data /data

USER 10001:10001
WORKDIR /app
ENV ZHORTEN_ADDR=0.0.0.0:3000 \
    ZHORTEN_DB=/data/zhorten.db \
    ZHORTEN_CACHE_CAPACITY=67108864 \
    ZHORTEN_SITE_ROOT=/app/site
VOLUME ["/data"]
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD ["/app/zhorten", "--healthcheck"]
ENTRYPOINT ["/app/zhorten"]
