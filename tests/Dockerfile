# syntax=docker/dockerfile:1.7
FROM rust:1.93-slim-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libasound2-dev \
        libx11-dev \
        libxrandr-dev \
        libxinerama-dev \
        libxcursor-dev \
        libxi-dev \
        libxkbcommon-dev \
        libxkbcommon-x11-0 \
        libwayland-dev \
        libgl1-mesa-dri \
        mesa-vulkan-drivers \
        xauth \
        xvfb \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

ARG OMNIATC_UID=1000
RUN mkdir /workspace && chown ${OMNIATC_UID}:${OMNIATC_UID} /workspace
RUN mkdir -p /usr/local/bin /usr/local/lib
WORKDIR /workspace
RUN useradd -u ${OMNIATC_UID} -d /workspace -m omniatc

RUN rustup component add cargo

COPY Cargo.toml Cargo.lock ./
COPY omniatc-client/Cargo.toml omniatc-client/Cargo.toml
COPY omniatc-core/Cargo.toml omniatc-core/Cargo.toml
COPY omniatc-math/Cargo.toml omniatc-math/Cargo.toml
COPY omniatc-maps/Cargo.toml omniatc-maps/Cargo.toml
COPY omniatc-store/Cargo.toml omniatc-store/Cargo.toml
COPY tests/diff-image/Cargo.toml tests/diff-image/Cargo.toml
COPY tests/client/Cargo.toml tests/client/Cargo.toml
RUN mkdir -p omniatc-client/src omniatc-core/src omniatc-math/src omniatc-maps/src omniatc-store/src tests/diff-image/src tests/client/src \
    && touch \
        omniatc-client/src/lib.rs \
        omniatc-client/src/main.rs \
        omniatc-core/src/lib.rs \
        omniatc-math/src/lib.rs \
        omniatc-maps/src/lib.rs \
        omniatc-store/src/lib.rs \
        tests/diff-image/src/lib.rs \
        tests/client/src/lib.rs
RUN cargo fetch

COPY . .
RUN --mount=type=cache,target=/workspace/target \
    cargo run -p omniatc-maps build-assets \
    && cargo build -p omniatc-client-test --bins \
    && cp target/debug/deps/libbevy_dylib-*.so /usr/local/lib/ \
    && for test in tests/client/src/bin/*.rs; do \
        name=$(basename "$test" .rs); \
        cp "target/debug/$name" "/usr/local/bin/client-test-$name"; \
    done
RUN printf '%s\n' \
        '#!/bin/sh' \
        'set -e' \
        "export LD_LIBRARY_PATH=/usr/local/lib:$(rustc --print target-libdir):$LD_LIBRARY_PATH" \
        'for bin in /usr/local/bin/client-test-*; do' \
        '  xvfb-run -a "$bin"' \
        'done' \
    > /usr/local/bin/run-client-tests \
    && chmod +x /usr/local/bin/run-client-tests

USER omniatc
ENTRYPOINT ["/usr/local/bin/run-client-tests"]
