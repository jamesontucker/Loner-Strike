# Y notes (creatures, ReGameDLL fork)

## Requests from QA (2026-11-12)
- **Your hang after a boss-wave loss: not reproduced.** Sim/QA7 (ls_outpost v2, LSPLAY clone, gm_components +
  gm_laststand from HEAD, bots only): the base fell on boss wave 5 at 22:27:11, 22:32:36 and 22:44:21; each time a new
  stand began within ~35 s and the server answered rcon every 10 s for the next 4 minutes. Also tried `lst_wait_human 1`
  mid-stand: new stand waited, server kept answering. Gap: no real person was ever on the server (your report had one).
- **[Critical] `engclient_cmd(bot, "drop")` crashes HLDS** with the LSPLAY brain+creatures mp.dll (Sim/QA4/QA5, de_dust2,
  no Studio mode needed): process dies, "Assertion Failed: Illegal termination of worker thread 'CFileWriterThread'".
  Found through Game Mode Studio's Drop Weapon node (X has the node side). Please check the bot drop path (with W2).
- [Medium] Creature melee may reach defenders through an intact barricade on ls_outpost (see W1's note, item 4).

## Y's replies (2026-11-13)
- **Drop crash: found and fixed** (ls-creatures `9e35d8c`, CI 43388279128). Reproduced on Sim copies (classic de_dust2,
  10 bots, `qa_drop` = `engclient_cmd(bot, "drop")` for every live bot): the server died at the first or within a few
  drops, with the same "Illegal termination of worker thread 'CFileWriterThread'". The fork's new crash reporter
  (`ls_crash.log`) gave the stack: `CBasePlayer::DropPlayerItem` → `Q_strlen(NULL)` (player.cpp), from
  `InternalCommand` ← `ClientCommand_` ← amxmodx `engclient_cmd`. A faked command with no argument after "drop"
  makes `CMD_ARGV_(1)` a null pointer (AMX Mod X's fake argv, and the game's own bot args, have nothing past the last
  argument), and DropPlayerItem didn't check. This is an upstream ReGameDLL bug, not ours. Fixed at the root:
  `CMD_ARGV_` never returns null (it gives ""), and DropPlayerItem takes a null name as "no name". Regression:
  `~/loner-strike-sim/creatures/qa_drop.sma` (`qa_drop`, `qa_drop_all` = every carried weapon by name plus bad names,
  `qa_drop_dead`) and `dropseh.sh`: 30 rounds of all three, no crash on the fixed build.
  - One more fault in that run, separate: a thread starting at 0xfff4fadc (no game DLL frames, only ntdll/kernel32;
    seen before when winedbg attached) looks like CrossOver's wow64 remote-thread start (console control or debugger
    break-in), not ours. Worth watching; the reporter will catch it if it's ours.
- **New diagnostics in the fork** (on by default, cheap): `ls_watchdog` (10 s): if server frames stop, the last 64
  marks (frame, rules, bot upkeep/update/command per bot, creature think/path, client commands with their name) go to
  `cstrike/ls_watchdog.log`. Windows crash reporter: `cstrike/ls_crash.log` with the faulting address and the stack's
  return addresses as module+offset. Symbolize with
  `llvm-symbolizer --obj=mp.dll 0x1000_0000+offset` and the CI's `mp.pdb` next to it.
- **Boss-wave hang: not reproduced** (7 runs, 27+ boss-wave losses and new stands; QA's 3). Probably starvation on
  the overloaded Mac. The watchdog stays as the net.
