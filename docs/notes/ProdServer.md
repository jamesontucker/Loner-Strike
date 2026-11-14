# Production server "Alone Together": restarts on 2026-11-14 (investigation notes)

Status: root cause narrowed down, not yet pinned to one component. No fix committed, nothing
deployed. The box was only read (no restarts, no file changes).

## What the box runs (built and deployed 2026-11-13)
- Engine: Loner-Strike ReHLDS fork, "Exe build 00:41:43 Nov 12 2026 (4597)" (engine_i486.so etc.
  dated Nov 13 04:11 UTC). The only engine change since build 4595 (which ran cleanly Nov 10-12) is
  06431db (skip entities with no classname). Valve's engine is kept in `.valve-engine/`.
- Game DLL: cs.so "ReGameDLL 5.30.0.1047-dev" = fork tag `5.30.0.814-ls20` (ea4bfa6, Nov 11 22:25 PST).
  Before that the box ran ls19 (954) with no crashes. ls19..ls20 is 93 commits, about 7.8k lines
  (map knowledge LSV v2-v6, sightlines, breakable floors, cover, team play).
- AMX Mod X 1.10.0.5487 and Metamod-P 1.21p109 (unchanged since Oct/May).
- relocalizebugfix_mm_i386.so: new on this box on Nov 13 04:12 (metamod plugins.ini changed then).
- Plugins: dated Nov 13 22:44 UTC, built from the **tools branch** (not main, not a release).
  gamemodes.amxx matches `3f96494d` (same code size and strings); gm_humanbots.amxx matches the
  current tools HEAD. Released app is v0.1.2 (no v0.1.3 tag); v0.1.2 ships Valve's engine, upstream
  ReGameDLL 5.30.0.814 and older plugins, so released users don't run this combination.

## Timeline (journal, kernel log, AMXX logs)
- 01:40:57 start after the OS-update reboot (planned).
- 01:59 map vote (mapchooser, picks cs_havana) -> 02:01:14 bots dropped "on level change" ->
  **SIGSEGV** (core-dump result). Kernel: segfault in amxmodx_mm_i386.so +0x21ebd.
- 02:21:34 same (vote at 02:19, SEGV at the change), same address.
- 02:41:55 same, at +0x242e7.
- 02:42:00 restart. 1 s after the map loaded ("map knowledge for de_dust2 loaded" 02:42:03.236),
  from 02:42:03.275: flood of `Precaching not allowed` / `Run time error 10 ... precache_model`
  from gamemodes.amxx then gm_humanbots.amxx, every frame, up to ~500 pairs a second
  (93,318 errors, 27 MB error_20261114.log). 02:46:52 **watchdog**: "hasn't answered for 90 s",
  SIGKILL. So restart 4 is a freeze caught by the watchdog, caused by the flood.
- 03:07:12 SEGV again at the change after the vote (+0x21ebd); 03:27:38 another restart.
  The server is in a ~20-minute crash loop: it always restarts on de_dust2 (+map de_dust2), so the
  vote's map is never reached.
- Nov 11 on the previous build: 60+ map changes, including de_dust2 -> cs_havana, no crash.

## Where it crashes (amxmodx_mm_i386.so from the box; file offset = reported offset + 0x1d000)
- +0x21ebd -> `CLangMngr::CLang::Clear()` (5 of 6 server crashes)
- +0x242e7 -> `THash<ke::AString, keytbl_val>::_Clear()` (the lang key table)
- Both are called from AMXX's `C_ServerDeactivate_Post` -> `g_langMngr.Clear()`, which runs when
  AMXX unloads plugins at every map change. plugin_end had already run (session.txt says "clean",
  and there's no "Last session ended unexpectedly" after the SEGVs). The pointers read were small
  junk values (0xb20e7, 0xb4d7d...), so AMXX's heap data was overwritten earlier: **heap corruption**.
- Other short-lived hlds_linux processes (pids 1357, 1970, 3043, 4178, 5182, 6676, not this unit,
  probably manual test runs; /tmp/dumps created 01:42) crashed in the same places (+0x242e7 and
  `amxx_DynaCallback` +0x2dde1 with a dead plugin pointer), some within 17 s of starting. So the
  corruption happens early and doesn't need 20 minutes of play.

## The precache errors are a symptom, not the cause
- Source: gamemodes.sma and gm_humanbots.sma call precache_model **only inside plugin_precache**.
  Checked in the deployed .amxx too (disassembled: the only `precache_model` sysreq in each is in
  `plugin_precache`, and nothing calls plugin_precache).
- AMXX only runs the `plugin_precache` forward in C_Spawn, with precaching allowed. The errors
  (gamemodes then gm_humanbots, in plugins.ini order, every frame) mean AMXX's forward for
  plugin_precache was being run by something else. That only fits AMXX's forward table being
  damaged, which is the same heap corruption as the SEGVs.
- Guarding plugin_precache (run once per map) would stop the log flood and the freeze, but not the
  crashes.

## What the corruption probably is
- Linux only: a Windows sim copy (Sim/PCX from B: the box's exact plugins, plugins.ini and
  gamemodes configs; engine build 4597; game DLL 5.30.0.1051; relocalizebugfix) survived 3 map
  changes, one after a mapchooser vote, with 19 bots and no errors.
- On Windows mp.dll has its own CRT heap, so a game-DLL heap bug can't touch AMXX there. On Linux
  everything shares glibc malloc. **Main suspect: a heap overflow or use-after-free in the ls20
  game DLL (new bot-brain code), hitting AMXX's lang and forward data.**
- relocalizebugfix: looked at it, probably not the cause. Its only hook is ClientCommand for
  say/say_team from real clients: it copies the text back through CMD_ARGS, at most 190 bytes.
  AMXX's engclient_cmd goes straight to the game DLL, so bot chat never reaches it, and there was
  no chat with % or #.
- Engine 4597: the only change is the no-classname entity parse. Unlikely.

## Next steps (need the user's OK, or a Linux test box)
1. A/B on Linux (not production): the box's build with cs.so swapped to ls19 (954). Then a vote and
   a map change with bots, or just `changelevel` 30 s after the map starts.
2. Or on production with the user's OK: roll cs.so back to the ls19 build (it ran cleanly Nov 10-12).
   That's the quickest way to stop the crash loop. Taking relocalizebugfix out of
   addons/metamod/plugins.ini is the second thing to try.
3. Once the component is known: an ASan/valgrind build of the game DLL on Linux, with a sim map change.
4. Harmless hardening for the plugins: make plugin_precache idempotent (no flood if the forward
   misfires). Not done yet.

Sim copy used: Sim/PCX (port 27400), stopped. Scratch compile dir: drive_c/Loner-Strike/PCfix.
