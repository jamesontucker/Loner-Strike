# SDHLT for Linux (Loner-Strike's build)

The map editor compiles with seedee's SDHLT v1.3.1 ([seedee/SDHLT](https://github.com/seedee/SDHLT)),
downloaded from its official release. That release's Linux programs were built on Ubuntu 24.04
and need glibc 2.38 and a current libstdc++, so they don't run on Ubuntu 22.04, Mint 21 or
Debian 12. On Linux the app puts this build of the same tag's source, unmodified, over them
(`tools::compile::SDHLT_LINUX_URL`): made on Ubuntu 20.04 with the C++ runtime linked in, it
needs glibc 2.29 (Ubuntu 20.04, Debian 11, Mint 20 or newer, and every current system).

Hosted on the download mirror (not in this repo):
`https://dl.lonerstrike.com/tools/sdhlt-v1.3.1-linux-x64.tar.xz`
sha256 `51871fa72b0c94ab881752c29a5a677dcf501788da53a79db4d4f1acfd65ac79`
(`Linux/` with sdHLCSG, sdHLBSP, sdHLVIS, sdHLRAD, sdRIPENT and SDHLT's LICENSE.md).

Checked 2026-11-24: compiles ls_outpost (CSG, BSP, VIS, RAD) on Ubuntu 22.04 to the same
.bsp the official build makes on 24.04.

## Building

`build.sh` in an x86_64 Ubuntu 20.04 container, output in /out:

    docker run --rm --platform linux/amd64 -v "$PWD":/work -v "$PWD/out":/out ubuntu:20.04 \
      bash /work/build.sh

Then upload `out/sdhlt-v1.3.1-linux-x64.tar.xz` to the mirror's `tools/` folder (scripts/r2-sync.sh
leaves folders alone) and pin its sha256 in compile.rs. The tarball isn't byte-for-byte
reproducible (file times), so a rebuild means a new sum.

SDHLT is GPL v2 (with the Valve SDK 2.3 license taking priority where needed, and parts under
the id Quake 2 licence); its source is the tag above, unchanged.
