# syntax=docker/dockerfile:1.7

FROM rust:1.88-bookworm AS builder

ARG DEBIAN_FRONTEND=noninteractive

WORKDIR /workspace/KittenTTS_rust_server

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates clang cmake pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY README.md LICENSE ./
COPY config ./config
COPY src ./src
COPY tests ./tests
COPY --from=kitten_tts_rs / ./../kitten_tts_rs

RUN cargo build --release --features real-backend

FROM debian:bookworm-slim AS onnxruntime

ARG DEBIAN_FRONTEND=noninteractive
ARG ONNXRUNTIME_VERSION=1.24.2

WORKDIR /tmp

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates curl tar \
    && rm -rf /var/lib/apt/lists/*

RUN curl -L --fail -o onnxruntime.tgz "https://github.com/microsoft/onnxruntime/releases/download/v${ONNXRUNTIME_VERSION}/onnxruntime-linux-x64-${ONNXRUNTIME_VERSION}.tgz" \
    && mkdir -p /onnxruntime \
    && tar -xzf onnxruntime.tgz --strip-components=1 -C /onnxruntime \
        "onnxruntime-linux-x64-${ONNXRUNTIME_VERSION}/lib"

FROM debian:bookworm-slim AS runtime

ARG DEBIAN_FRONTEND=noninteractive
ARG ONNXRUNTIME_VERSION=1.24.2

ENV HF_HOME=/home/elkitten/.cache/huggingface \
    KITTENTTS_SERVER_HOST=0.0.0.0 \
    KITTENTTS_SERVER_PORT=8008 \
    RUST_LOG=info

WORKDIR /app

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates curl espeak-ng gosu libgomp1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system elkitten \
    && useradd --system --gid elkitten --create-home --home-dir /home/elkitten --shell /usr/sbin/nologin elkitten \
    && install -d -o elkitten -g elkitten /app/config /home/elkitten/.cache/huggingface /home/elkitten/.config/pulse /home/elkitten/.local/share/onnxruntime/${ONNXRUNTIME_VERSION}

COPY --from=builder /workspace/KittenTTS_rust_server/target/release/kittentts-server-rs /usr/local/bin/kittentts-server-rs
COPY --from=onnxruntime /onnxruntime/lib/libonnxruntime.so /home/elkitten/.local/share/onnxruntime/${ONNXRUNTIME_VERSION}/libonnxruntime.so
COPY --from=onnxruntime /onnxruntime/lib/libonnxruntime.so.1 /home/elkitten/.local/share/onnxruntime/${ONNXRUNTIME_VERSION}/libonnxruntime.so.1
COPY --from=onnxruntime /onnxruntime/lib/libonnxruntime.so.${ONNXRUNTIME_VERSION} /home/elkitten/.local/share/onnxruntime/${ONNXRUNTIME_VERSION}/libonnxruntime.so.${ONNXRUNTIME_VERSION}
COPY --chown=elkitten:elkitten config ./config
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN chmod 0755 /usr/local/bin/docker-entrypoint.sh

EXPOSE 8008

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:8008/healthz >/dev/null || exit 1

ENTRYPOINT ["/bin/sh", "/usr/local/bin/docker-entrypoint.sh"]
CMD ["kittentts-server-rs"]
