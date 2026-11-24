#!/bin/bash
# Builds Metamod-P for Linux (i386) at the release setup installs (v1.21p109), with the
# listen-server engine fix. Runs inside an i386 Ubuntu 18.04 container (as Metamod-P's own
# "ubuntu1804" release build does); output in /out.
set -euxo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq git build-essential g++ make ca-certificates >/dev/null
mkdir -p /src && cd /src
git clone -q https://github.com/Bots-United/metamod-p
cd metamod-p
git checkout -q v1.21p109
git apply /work/listen-server-engine.patch
make -j4 -C metamod OPT=opt META_VERSION=109
cp metamod/opt.linux_i386/metamod.so /out/
strip --strip-debug /out/metamod.so
ls -la /out
