# Patched Ham Sandwich (Linux servers)

`hamsandwich_amxx_i386.so` is AMX Mod X's Ham Sandwich module for Linux, built from
[alliedmodders/amxmodx](https://github.com/alliedmodders/amxmodx) at commit
`8fc12670ea86470a0bfed4c35db5de73d3a39e8a` (1.10.0-git5487, the build setup installs) with
`specialbot-queue.patch` applied: the same fix as the Windows module, see
`../../windows/hamsandwich/README.md` for the bug. The app puts it on dedicated servers on
Linux machines each time one starts (remote.rs, `ensure_ham`), in place of the official
module, which has the same bug.

sha256: `dd398607f0dd15d22493dce920eb59f5bc62d5f6710205bff8bbd42c1202510e`

## Building

`build.sh`, in an x86 Ubuntu 18.04 container (the old glibc keeps it loading on any server:
it needs glibc 2.4 and libdl, nothing newer), with this folder at /work and an output folder
at /out:

    docker run --rm --platform linux/amd64 -v "$PWD":/work -v "$PWD/out":/out ubuntu:18.04 \
      bash -c 'mkdir -p /src && bash /work/build.sh'

AMBuild with gcc -m32, `--enable-optimize --no-mysql`, debug info stripped. Its exports match
the official module's (Meta_*, AMXX_*, GetEntityAPI2 and the rest).

AMX Mod X is GPL-3.0 (with its stated exceptions); this file and the patch are under the same
terms.
