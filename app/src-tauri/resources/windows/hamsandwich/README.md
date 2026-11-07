# Patched Ham Sandwich (Windows only)

`hamsandwich_amxx.dll` is AMX Mod X's Ham Sandwich module built from
[alliedmodders/amxmodx](https://github.com/alliedmodders/amxmodx) at commit
`8fc12670ea86470a0bfed4c35db5de73d3a39e8a` (1.10.0-git5487, the build setup installs) with
`specialbot-queue.patch` applied. Setup puts it over the official one on native Windows
installs only; CrossOver installs on the Mac keep the official module.

sha256: `d1ebbf716f3a7a65dbc59eacb4da4a00ab7eb84b1308a2a8b79338ea4b3ebdeb`

## The bug

`RegisterHamPlayer` / `RegisterHam(..., "player", ..., true)` before any bot has joined is
queued by `CHamSpecialBotHandler` (it needs a bot to find the bots' vtable). The queue keeps
the plugin's `AMX*` and a `Forward*` it holds no reference to. When plugins unload at a map
change, the forward is freed but the queue is not cleared, so the first bot on a later map
makes `RegisterChecked` call `AddRef()` on freed memory.

gamemodes.amxx reloads the first map before any bot joins (`bot_join_after_player`), which
triggers it every session. On Windows the freed block is reused by AMX Mod X itself (Trie
storage), so the stray increment corrupts the heap and hl.exe crashes at a later map change
(`0xc0000374`, or an access violation in amxmodx_mm.dll / mp.dll). Wine's heap doesn't reuse
the block, which is why the same setup never crashed under CrossOver. Found with full page
heap (`PageHeapFlags 0x13`): the faulting write is `inc dword ptr [eax]` in
`CHamSpecialBotHandler::RegisterChecked` on a block freed by the module.

The fix empties the queue in `OnPluginsUnloaded` (the queued items own no forward reference,
so they are just deleted).

## Building

MSVC (x86), same flags as AMX Mod X's AMBuildScript for Windows modules (`/MT /Ox /Oy- /EHsc
/GR-`, defines `NDEBUG WIN32 _WINDOWS AMX_NOPROPLIST PAWN_CELL_SIZE=32 AMXMODX_BUILD
AMXX_USE_VERSIONLIB HAVE_STDINT_H`), with
[metamod-hl1](https://github.com/alliedmodders/metamod-hl1) and
[hlsdk](https://github.com/alliedmodders/hlsdk), sources as listed in
`modules/hamsandwich/AMBuilder`, linked with
`/EXPORT:GiveFnptrsToDll=_GiveFnptrsToDll@8,@1 /SECTION:.data,RW`.

AMX Mod X is GPL-3.0 (with its stated exceptions); this file and the patch are under the same
terms.
