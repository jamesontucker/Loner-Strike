#!/usr/bin/env bash
# build-server-agent.sh
#
# Builds ls-control (app/server-agent), Loner-Strike's shared control service for servers
# on a Linux machine, as one static Linux x86_64 program (musl), in Docker, so it runs on
# any Ubuntu or Debian server without anything else installed. The app ships it and puts
# it on the server over SSH (control/agent.rs), checked against the sha256 written here.
#
# Out: app/src-tauri/resources/linux/ls-control/ls-control and ls-control.sha256
# Needs: Docker (Docker Desktop on a Mac runs the x86_64 image). The release workflow runs
# this too (.github/workflows/release.yml, job server-agent).
#
# Usage: ./scripts/build-server-agent.sh
set -euo pipefail
cd "$(dirname "$0")/.."

# Rust's official image, pinned by digest (a new one: change both, after checking it builds;
# `docker buildx imagetools inspect rust:<tag>` shows a tag's digest)
IMAGE="rust:1.96-bookworm"
IMAGE_DIGEST="sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663"
TARGET=x86_64-unknown-linux-musl
OUT=app/src-tauri/resources/linux/ls-control

command -v docker >/dev/null || { echo "Docker is needed: https://docs.docker.com/get-docker/" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker isn't running. Start Docker Desktop and try again." >&2; exit 1; }
mkdir -p "$OUT"
ref="$IMAGE@$IMAGE_DIGEST"

# Only what the build reads goes in, read-only: the crate, the app's shared control rules
# (compiled in with #[path]) and the app's Cargo.toml (its version is ls-control's).
# Downloads and build output stay in Docker volumes between runs.
docker run --rm --platform linux/amd64 \
  -v "$PWD/app/server-agent:/src/app/server-agent:ro" \
  -v "$PWD/app/src-tauri/Cargo.toml:/src/app/src-tauri/Cargo.toml:ro" \
  -v "$PWD/app/src-tauri/src/control:/src/app/src-tauri/src/control:ro" \
  -v "$PWD/$OUT:/out" \
  -v loner-strike-agent-cargo:/usr/local/cargo/registry \
  -v loner-strike-agent-target:/target \
  -e CARGO_TARGET_DIR=/target \
  -e RUSTFLAGS="-C target-feature=+crt-static --remap-path-prefix=/src=/src --remap-path-prefix=/usr/local/cargo=/cargo" \
  -w /src/app/server-agent \
  "$ref" \
  sh -euc "
    rustup target add $TARGET >/dev/null 2>&1
    cargo build --release --locked --target $TARGET
    cp /target/$TARGET/release/ls-control /out/ls-control.new
    chmod 755 /out/ls-control.new
    # it runs, on its own (no libraries), and says its version
    /out/ls-control.new version
    mv -f /out/ls-control.new /out/ls-control
  "

# x86_64 ELF, statically linked (no program interpreter)
head -c 20 "$OUT/ls-control" | od -An -tx1 | tr -d ' \n' | grep -q '^7f454c46020101' || { echo "ls-control isn't a 64-bit Linux program." >&2; exit 1; }
if command -v sha256sum >/dev/null; then sum=$(sha256sum "$OUT/ls-control" | cut -c1-64); else sum=$(shasum -a 256 "$OUT/ls-control" | cut -c1-64); fi
printf '%s  ls-control\n' "$sum" > "$OUT/ls-control.sha256"
echo "Built $OUT/ls-control ($(wc -c < "$OUT/ls-control" | tr -d ' ') bytes, sha256 $sum)"
