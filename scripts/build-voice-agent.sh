#!/usr/bin/env bash
# build-voice-agent.sh
#
# Builds ls-voice (app/voice-agent), Advanced Bot Interaction's speech-to-text helper, for
# servers on a Linux machine: one Linux x86_64 program, in Docker. Whisper (C++) and Opus are
# built into it and its C++ runtime too, so it needs only glibc 2.31 or newer: Ubuntu 22.04
# and up, Debian 11 and up. The app ships it and puts it on the server over SSH when voice is
# turned on there, checked against the sha256 written here.
#
# Out: app/src-tauri/resources/linux/ls-voice/ls-voice and ls-voice.sha256
# Needs: Docker (Docker Desktop on a Mac runs the x86_64 image). The release workflow runs
# this too (.github/workflows/release.yml, job server-agent).
#
# Usage: ./scripts/build-voice-agent.sh
set -euo pipefail
cd "$(dirname "$0")/.."

# Rust's official image on Debian 11 (glibc 2.31: the oldest glibc it then needs), pinned by
# digest (`docker buildx imagetools inspect rust:<tag>` shows a tag's digest)
IMAGE="rust:1.96-bullseye"
IMAGE_DIGEST="sha256:21d112adb6ab370d39eef8f861f10b42342419b9569e7a05dac15e19eb65a3b4"
OUT=app/src-tauri/resources/linux/ls-voice

command -v docker >/dev/null || { echo "Docker is needed: https://docs.docker.com/get-docker/" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker isn't running. Start Docker Desktop and try again." >&2; exit 1; }
mkdir -p "$OUT"
ref="$IMAGE@$IMAGE_DIGEST"

docker run --rm --platform linux/amd64 \
  -v "$PWD/app/voice-agent:/src/app/voice-agent:ro" \
  -v "$PWD/$OUT:/out" \
  -v loner-strike-voice-cargo:/usr/local/cargo/registry \
  -v loner-strike-voice-target:/target \
  -e CARGO_TARGET_DIR=/target \
  -e RUSTFLAGS="-C link-arg=-static-libstdc++ -C link-arg=-static-libgcc --remap-path-prefix=/src=/src --remap-path-prefix=/usr/local/cargo=/cargo" \
  -w /src/app/voice-agent \
  "$ref" \
  sh -euc "
    # (cmake builds Whisper and Opus; clang is for the bindings to Whisper)
    # (Debian 11's packages live on its archive now)
    printf 'deb http://archive.debian.org/debian bullseye main\ndeb http://archive.debian.org/debian-security bullseye-security main\n' > /etc/apt/sources.list
    rm -f /etc/apt/sources.list.d/*
    apt-get -o Acquire::Check-Valid-Until=false update -qq >/dev/null && apt-get install -y -qq cmake clang libclang-dev >/dev/null
    cargo build --release --locked
    cp /target/release/ls-voice /out/ls-voice.new
    chmod 755 /out/ls-voice.new
    # (it runs with nothing else installed: usage, and exit 2)
    /out/ls-voice.new >/dev/null 2>&1 || [ \$? -eq 2 ]
    mv -f /out/ls-voice.new /out/ls-voice
  "

head -c 20 "$OUT/ls-voice" | od -An -tx1 | tr -d ' \n' | grep -q '^7f454c46020101' || { echo "ls-voice isn't a 64-bit Linux program." >&2; exit 1; }
if command -v sha256sum >/dev/null; then sum=$(sha256sum "$OUT/ls-voice" | cut -c1-64); else sum=$(shasum -a 256 "$OUT/ls-voice" | cut -c1-64); fi
printf '%s  ls-voice\n' "$sum" > "$OUT/ls-voice.sha256"
echo "Built $OUT/ls-voice ($(wc -c < "$OUT/ls-voice" | tr -d ' ') bytes, sha256 $sum)"
