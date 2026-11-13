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
