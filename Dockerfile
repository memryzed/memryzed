# Dockerfile for Memryzed.
#
# Builds the `memryzed` binary from source and runs the MCP server over
# stdio. MCP-aware clients spawn the container and exchange protocol
# frames on stdin/stdout.
#
# Most users do not need this image; the install script at
# https://memryzed.com fetches a prebuilt binary for their platform.
# This Dockerfile is for containerized deployments and CI.

# ---- build ----
FROM rust:1.83-slim-bookworm AS build

# ONNX Runtime (via the `ort` crate) and SQLite need a C/C++ toolchain.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev clang cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY . .

RUN cargo build --release -p memryzed-cli \
    && cp target/release/memryzed /usr/local/bin/memryzed

# ---- runtime ----
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /usr/local/bin/memryzed /usr/local/bin/memryzed

# Keep all state inside the container by default. Mount a volume here to
# persist memory across container restarts.
ENV MEMRYZED_DATA_DIR=/data
RUN mkdir -p /data
VOLUME ["/data"]

# Run the MCP server over stdio.
ENTRYPOINT ["memryzed", "serve"]
