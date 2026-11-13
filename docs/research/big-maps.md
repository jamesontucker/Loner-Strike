# Big GoldSrc maps for Battle Royale / Extraction (20-40 players)

Status: desk research, 2026-11-12. Web search found **no actual CS 1.6 battle royale map on GameBanana or ModDB**. Treat every candidate below as "to verify by download". Spawn counts, file sizes and licences must be checked against the BSP (`ripent`/`bspinfo`) and the readme. Nothing below has been confirmed.

## The hard limit: ±4096 units
- The GoldSrc world is capped at ±4096 units per axis, so 8192 x 8192 x 8192 in total (about 156 m per side at roughly 1.9 cm/unit). The cap is built into the engine and client and cannot be patched for Steam clients. ReHLDS only changes the server.
- Things that go past it get culled or snapped by the network code, show up as invisible entities, or fail to compile. VHLT can compile past the limit, but stock clients still break.
- How big maps live with it:
  1. **Use the full cube.** About 8k x 8k already feels big with 20-40 players and gives a 1-2 minute run edge to edge.
  2. **Stack zones vertically.** Put separate areas on different Z levels and link them with teleports, like the Half-Life Battle Royale mod (it uses 4 vertical slices). The CS:GO map br_electrified got around Source's limit the same way.
  3. **Scale down.** Use smaller buildings and props, narrower roads and a lot of fog (`env_fog` / sky + `mp_` fog via plugin), so the map reads as larger than it is.
  4. **Watch the other limits.** Big open terrain runs into r_speeds/wpoly, MAX_MAP_CLIPNODES (32767), AllocBlock/lightmaps, 512 models and 8 MB of textures. Open terrain needs heavy VIS hinting or `func_detail` (VHLT).
  5. **Go multi-map.** For Extraction, link several mid-size maps with a changelevel (each map is a "zone"). This is easier than one huge BSP.

## Candidates
| Map | Where | Style | Extra assets | Notes / test-map verdict |
|---|---|---|---|---|
| Half-Life Battle Royale (mod) | https://www.moddb.com/mods/half-life-battle-royale | HL1 BR, 4 vertical sections | HL mod content | Unreleased. Useful as a reference for the stacking technique, not as a test map. |
| zm_ large open zombie maps (e.g. zm_western, zm_cbble_kamp) | https://counter-strike-boost.com/map/zm_western , https://counter-strike-boost.com/map/zm_cbble_kamp | Town/western, many buildings | Often custom WAD/models/sky | Built for 32p Zombie Plague, so lots of spawns. Good candidates for a first test, but verify licence (often none stated). |
| zm_fun | https://gamebanana.com/mods/download/88406 | Zombie map | Unknown | Size unknown. Verify. |
| fy_oilrig | https://www.moddb.com/addons/fy-oilrig | Multi-level rig | Unknown | Vertical rather than wide. Better as an Extraction sub-zone. |
| HL1 DM maps (crossfire, stalkyard, datacore, bounce, snark_pit) | Ship with Half-Life | Industrial, multi-area | Needs halflife.wad and HL models (CS ships halflife.wad, but some map models/sounds may be missing in CZ) | Run in CS if you add info_player_start/deathmatch → CT/T spawns via ripent or a spawn plugin. Small-to-mid size. Fine for quick bot/nav tests, too small for BR. |
| Sven Co-op maps | svencoop.com | Large co-op campaigns | Need Sven entities/WADs | **Usually not compatible.** Sven-specific entities and its raised limits (Sven's own engine build) break in CS. Avoid. |
| Stock CS/CZ largest (de_cbble, de_aztec, cs_militia, de_airstrip, cs_havana) | Stock | Mixed | None | Already have navs and no licence issues. Too small for 40p BR, but **the best immediate test maps** for the mode's logic (zone shrink, loot, extraction trigger). |

## Licence notes
- GameBanana/ModDB maps are mostly "free to use on servers" by convention. Redistributing them in a public repo needs explicit author permission. Do not bundle them. Download via FastDL or link to the source.
- Stock Valve maps can be used on servers but not redistributed. HL1 maps fall under the same rule.

## Recommendation
1. Prototype BR/Extraction rules on stock maps (de_cbble, cs_militia) with plugin-added spawns.
2. Download 3-5 large zm_/town maps, check dimensions (`bspinfo`, look for extents near ±4096), spawn counts and licences, and pick one as the 20-40p test map.
3. For 1.0.0-quality BR, plan a **custom 8k x 8k map** (VHLT, func_detail, fog, stacked zones) or a multi-map Extraction chain, and own the licence.

## Sources
- https://www.moddb.com/mods/half-life-battle-royale
- https://www.pcgamer.com/go-4-the-kill-mod-brings-battle-royale-combat-to-counter-strike-global-offensive
- https://www.moddb.com/addons/fy-oilrig
- https://gamebanana.com/mods/download/88406
- https://counter-strike-boost.com/map/zm_western
- https://counter-strike-boost.com/map/zm_cbble_kamp
- https://forums.alliedmods.net/showthread.php?p=1257039
