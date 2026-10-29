#!/usr/bin/env bash
# Runs the desktop clippy and tests on Ubuntu 24.04 in Docker, as the Linux CI jobs do, so Linux-only code can be
# checked from Windows or macOS. The toolchain, the crates and the build stay in one Docker volume between runs.
set -euo pipefail

image=openscl4-desktop-linux-check
cache=openscl4-desktop-linux
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Docker Desktop on Windows takes the Windows path, which Git Bash gives with -W
if windows_root="$(cd "$root" && pwd -W 2>/dev/null)"; then
    root="$windows_root"
fi

# the packages of the Rust test job in .github/workflows/test.yml, and what its runner already has
docker build --quiet --tag "$image" - >/dev/null <<'DOCKERFILE'
FROM ubuntu:24.04
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl dbus pkg-config \
        libdbus-1-dev libfontconfig1-dev libsqlite3-dev libxkbcommon-dev \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/cache/rustup CARGO_HOME=/cache/cargo CARGO_TARGET_DIR=/cache/target PATH=/cache/cargo/bin:$PATH
DOCKERFILE

# MSYS_NO_PATHCONV keeps Git Bash from turning the container's paths into Windows ones
MSYS_NO_PATHCONV=1 docker run --rm \
    --mount "type=bind,source=$root,target=/src,readonly" \
    --volume "$cache:/cache" \
    --workdir /src \
    "$image" bash -c '
        set -euo pipefail
        if ! command -v rustup >/dev/null; then
            curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs \
                | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none
        fi
        # the toolchain of rust-toolchain.toml
        rustup toolchain install
        cargo clippy --locked -p openscq30-desktop --all-targets -- -D warnings
        dbus-run-session -- cargo test --locked -p openscq30-desktop
    '
