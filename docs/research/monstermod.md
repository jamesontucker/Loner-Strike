# MonsterMod and HL1 monster models (research, 2026-11-12)

## 1. MonsterMod projects

| Project | Status | Platforms | Licence |
|---|---|---|---|
| botman "Monster" Metamod plugin (2002) | Dead. Three updates, last on 2002-07-30. Supported TFC, CS and others. The 2002 source does not compile on modern compilers and has missing files. | Win/Linux (old) | GPL-2.0 plus HLSDK code |
| Rick90 "Monster Mod" (ModDB) | Last public binary 2007-09-29, private until 2014 | Win | unclear |
| [JulianR0/monstermod-redo](https://github.com/JulianR0/monstermod-redo) (Giegue's rewrite, the main maintained fork) | Active milestones. Tier 0 (Linux) and Windows tiers are marked done. **No GitHub releases are published**, so you build it yourself. | `monster_mm.dll` (Windows) / `monster_mm_i386.so` (Linux) in `addons/monstermod/` | GPL-2.0 |
| [QuakerLife/monstermod-redo](https://github.com/QuakerLife/monstermod-redo) | Fork of the above (39 commits, older README that says "Linux only") | Linux | GPL-2.0 |
| [tjanok/monstermod-redux](https://github.com/tjanok/monstermod-redux) | Small (7 commits). Reduces the precache to avoid edict and precache crashes. Adds commands to spawn monsters at a location or from a cfg. Has a VS .sln. | Win (likely) | "originally GPL v2" |

I found no project named "MonsterMod Reloaded". The redo project is the one people mean.

### Compatibility with CS 1.6, ReHLDS, Metamod and ReGameDLL
- It is a Metamod plugin with its own monster code (it does not rely on the game DLL's monster classes). That is why the original ran on CS and TFC. It should therefore load under Metamod-P or Metamod-R alongside ReGameDLL. **None of the repos explicitly confirm CS 1.6 or ReGameDLL.** This needs testing.
- The redo README recommends **ReHLDS** so you can use `rescount` and `reslist model|sound` to track precache usage.
- Linux: binaries built with GCC 5 or later can fail on stock HLDS with libstdc++ `CXXABI` errors. Either build with g++ 4.8 or delete the libstdc++ bundled with HLDS.

### Configuration
- Global config is in key → value format. There are per-monster config files in the release `cfg/` folder, and the wiki has details. Keyvalues include `displayname` (HUD name) and `classify` (relationship class).
- The redo supports spawning from per-map cfg files. It also supports `info_node` / `info_node_air` for navigation (a node viewer is included). Without nodes, monsters move poorly on CS maps.

### Known issues for CS
- **Precache limit (512 models/sounds):** each monster type adds many models and sounds. Large CS maps combined with AMXX plugins can crash at load. Keep the set of monster types small (redux trims its precache for this reason).
- **Bots:** CS/CZ/ReGameDLL bots only target players. They will not shoot monsters unless the bot brain is changed (our ReGameDLL fork could add this). Monsters will attack bots.
- **Hostages:** hostages are a game-DLL entity (`hostage_entity`). Monster relationships with hostages are untested and may be ignored or attacked.
- Redo bugs listed in its README: stukabats hover, and monsters gib during death animations.

### Licence quote
The repos are GPL-2.0, but they derive from Valve's HL SDK. The HLSDK file header reads:
> "Use, distribution, and modification of this source code and/or resulting object code is restricted to non-commercial enhancements to products from Valve LLC. All other use, distribution, or modification is prohibited without written permission from Valve LLC."

In practice: non-commercial redistribution is fine, and Loner-Strike is free.

## 2. Do CS 1.6-only owners have the HL1 monster models?
- Steam app 10 (Counter-Strike) includes **shared depot 1 "Base Goldsrc Shared Content" from app 70**, plus depot 11 (CS base). See the [Valve Steam App IDs wiki](https://developer.valvesoftware.com/wiki/Steam_Application_IDs) and [SteamDB app 10 depots](https://steamdb.info/app/10/depots). The `valve/` folder that a CS-only owner gets therefore comes from depot 1. Community reports say "all of the HL content is still installed" ([AlliedModders](https://forums.alliedmods.net/showthread.php?p=2751959)).
- Local check: HLDS (appid 90) installs in our CrossOver bottle (`Loner-Strike Servers/.base/valve/models/`) contain `zombie.mdl` and `hgrunt.mdl`. The client's Half-Life folder has 15 matching monster models. On this machine depot 1 is recorded under app 70 because Half-Life is owned.
- **Likely yes, but not verified on a CS-only account.** To verify, check depot 1's manifest or a clean CS-only install. Monster models live on the server. Clients load them from their own `valve/` or download them through sv_downloadurl/FastDL if they are missing. Do not redistribute Valve `.mdl` files on FastDL unless that is needed.

### Free alternatives
- I found no ready-made CS-oriented HL1 monster pack on GameBanana. Most results are HL2 or HL:A ports (for example [Volkolak's HL:A Zombies](https://gamebanana.com/mods/484084), which is Source and not usable). Browse [GameBanana Half-Life models](https://gamebanana.com/mods/cats/1890?gameid=34) and [ModDB HL addons](https://www.moddb.com/games/half-life/addons) for GoldSrc `.mdl` reskins. Check each author's permission terms.
- Stock HL models are the simplest choice, because MonsterMod's skeletons and animations expect them.

Sources: links above, plus [TCRF HL SDK monster models](https://tcrf.net/Development:Half-Life_(Windows)/Half-Life_SDK/Monster_Models).
