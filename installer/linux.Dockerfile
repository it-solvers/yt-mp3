# Ubuntu용 build 환경. 지원하는 가장 오래된 Ubuntu에서 build해야 더 새 version에서도 실행됨
FROM ubuntu:22.04

ARG RUST_VERSION
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl pkg-config \
        libasound2-dev libfontconfig1-dev libwayland-dev libx11-dev libxkbcommon-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain "$RUST_VERSION"
