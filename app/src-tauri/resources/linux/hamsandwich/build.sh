#!/bin/bash
# Builds AMX Mod X's Ham Sandwich for Linux (i386) at the commit setup installs, with the
# special-bot queue fix. Runs inside an x86 Ubuntu container; output in /out.
set -euxo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq git python3 python3-pip g++-multilib gcc-multilib clang nasm ca-certificates >/dev/null
cd /src
git clone -q https://github.com/alliedmodders/ambuild
pip3 install -q ./ambuild
git clone -q https://github.com/alliedmodders/metamod-hl1
git clone -q https://github.com/alliedmodders/hlsdk
git clone -q https://github.com/alliedmodders/amxmodx
cd amxmodx
git checkout -q 8fc12670ea86470a0bfed4c35db5de73d3a39e8a
git submodule update --init --recursive -q
git apply /work/specialbot-queue.patch
mkdir build && cd build
python3 ../configure.py --enable-optimize --metamod=/src/metamod-hl1 --hlsdk=/src/hlsdk --no-mysql
ambuild || true
find . -name 'hamsandwich_amxx_i386.so' -exec cp {} /out/ \;
strip --strip-debug /out/hamsandwich_amxx_i386.so
ls -la /out
