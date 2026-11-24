#!/bin/bash
# Builds SDHLT v1.3.1's Linux tools from seedee's source, unmodified, in an x86_64 Ubuntu 20.04
# container: its glibc 2.31 and the C++ runtime linked in (-static-libstdc++ -static-libgcc)
# (with -pthread: glibc 2.31 still has threads in libpthread) keep them running on older systems than the release's own Linux build (made on Ubuntu 24.04,
# needs glibc 2.38). Output in /out: the five programs and sdhlt-v1.3.1-linux-x64.tar.xz.
set -euxo pipefail
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq git g++ make python3-pip ca-certificates xz-utils >/dev/null
pip3 install -q 'cmake>=3.20,<4'
mkdir -p /src && cd /src
git clone -q https://github.com/seedee/SDHLT
cd SDHLT
git checkout -q v1.3.1
cmake -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_CXX_STANDARD=17 -DCMAKE_EXE_LINKER_FLAGS="-static-libstdc++ -static-libgcc -pthread"
cmake --build build -j4
mkdir -p /out/Linux
for t in sdHLCSG sdHLBSP sdHLVIS sdHLRAD sdRIPENT; do cp "tools/$t" /out/Linux/; strip /out/Linux/$t; done
cp LICENSE.md /out/Linux/
cd /out && tar -cJf sdhlt-v1.3.1-linux-x64.tar.xz Linux
sha256sum sdhlt-v1.3.1-linux-x64.tar.xz Linux/*
