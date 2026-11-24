# Patched Metamod-P (the native Linux game)

`metamod.so` is Metamod-P for Linux built from
[Bots-United/metamod-p](https://github.com/Bots-United/metamod-p) at tag `v1.21p109` (the
release setup pins) with `listen-server-engine.patch` applied. Setup puts it in the game on
Linux (`gamebin::LINUX_FLAVOR.metamod_fixed_res`). Linux servers (remote.rs) keep the official
build: a dedicated server's engine is `engine_i486.so`, which the release already recognises.

sha256: `3401efe77b0e332d7347c860e851399e1872ce21091f329fa5c9ebb61de772e0`

## The bug

When the native Linux game hosts a match itself (Play), the engine is the game's own `hw.so`.
On Linux, Metamod-P's `EngineInfo::check_for_engine_module` only recognises `engine_*.so`
(on Windows it does recognise `hw.dll`/`sw.dll`), so it never finds the engine's code range.
The failure leaves the state at `STATE_NULL` rather than `STATE_INVALID`, so
`is_valid_code_pointer` says no to everything, and `determine_engine_interface_version` sets
every newer engine function to NULL: `pfnGetPlayerAuthId` and the 15 after it (GetFileSize,
QueryClientCvarValue, ...). Plugins get that table. AMX Mod X calls `GetPlayerAuthId` in
`C_ClientUserInfoChanged_Post` as a player joins, so the game crashed (segfault at address 0)
the moment you joined your own match; with `GetFileSize` gone the bots also reported every nav
file as made for a different version of the map.

Found 2026-11-24 on Ubuntu 22.04 (gdb: `#0 0x00000000`, `#1 C_ClientUserInfoChanged_Post`;
AMX Mod X's `g_engfuncs` filled up to slot 142, NULL from 143). Same crash with AMX Mod X 1.9
and with no AMX Mod X plugins loaded; stock CS and Metamod alone ran fine.

The patch makes the Linux check also accept `hw.so` and `sw.so`, as the Windows one does.

## Building

`build.sh`, in an i386 Ubuntu 18.04 container (Metamod-P's own "ubuntu1804" release build is
made the same way; the old glibc keeps it loading inside Steam's runtime on any system: it
needs glibc 2.4), with this folder at /work and an output folder at /out:

    docker run --rm --platform linux/386 -v "$PWD":/work -v "$PWD/out":/out i386/ubuntu:18.04 \
      bash /work/build.sh

Metamod-P is GPL-2.0-or-later; this file and the patch are under the same terms.
