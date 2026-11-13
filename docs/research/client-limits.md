# What a stock CS 1.6 / CZ client can show (server-driven)

Scope: Steam GoldSrc client (2013+ builds, including the 25th anniversary update), no client mods. Everything here is driven from AMX Mod X / ReGameDLL / ReHLDS. Items marked *(verify)* are from community knowledge and have not been tested on our builds.

Primary sources:
- HL SDK `common/const.h` (TE_* layouts): https://github.com/ValveSoftware/halflife/blob/master/common/const.h
- AMXX `message_const.inc` (TE_* docs with arg lists): https://github.com/alliedmodders/amxmodx/blob/master/plugins/include/message_const.inc
- AMXX `amxmodx.inc` / `newmenus.inc`: https://github.com/alliedmodders/amxmodx/tree/master/plugins/include
- CS 1.6 user messages reference: https://wiki.alliedmods.net/Half-Life_1_Game_Events and https://wiki.alliedmods.net/Category:Half-Life_1_Messages
- ReGameDLL / ReHLDS sources: https://github.com/s1lentq/ReGameDLL_CS , https://github.com/dreamstalker/rehlds

## 1. Temp entities (SVC_TEMPENTITY)

Sent with `message_begin(MSG_ALL|MSG_ONE|MSG_PVS|MSG_BROADCAST…, SVC_TEMPENTITY)`. Use `MSG_ONE_UNRELIABLE` / `MSG_BROADCAST` for repeating visuals (unreliable = dropped instead of overflowing).

| TE | id | Args (after id) | Notes |
|---|---|---|---|
| TE_BEAMPOINTS | 0 | coord x,y,z (start), coord x,y,z (end), short sprite, byte startframe, byte framerate, byte life (0.1 s), byte width (0.1 u), byte noise (0.01), byte r,g,b, byte brightness, byte scroll | Straight beam between two points. Basis for drawing circles/lines as segments. |
| TE_BEAMENTPOINT | 1 | short entity, coord x,y,z, …same tail | Beam from entity to point (follows entity). |
| TE_BEAMENTS | 8 | short start ent, short end ent, …same tail | |
| TE_BEAMTORUS | 19 | coord center x,y,z, coord axis x,y,z, …same tail | Expanding torus. |
| TE_BEAMDISK | 20 | same as torus | Expanding filled disk. |
| TE_BEAMCYLINDER | 21 | same as torus | Expanding ring: radius grows from 0 to `axis` length over `life`. Classic "shockwave". **Cannot hold a static radius** – it always animates outward. |
| TE_BEAMFOLLOW | 22 | short entity, short sprite, byte life, byte width, byte r,g,b, byte brightness | Trail. |
| TE_BEAMRING | 24 | short start ent, short end ent, …same tail as BEAMPOINTS | Ring whose diameter is the line between two entities. Needs two entities. |
| TE_SPRITE | 17 | coord x,y,z, short sprite, byte scale (0.1), byte brightness | One-shot, plays the sprite once (life = sprite frames). |
| TE_GLOWSPRITE | 23 | coord x,y,z, short model, byte life (0.1 s), byte scale (0.1), byte brightness | Fading glow; good for markers. |
| TE_PLAYERATTACHMENT | 124 | byte player, coord vertical offset, short model, short life (0.1 s) | Sprite above a player's head (used for "radio" icons, team markers). |
| TE_KILLBEAM | 99 | short entity | Kills beams attached to an entity. |
| TE_TEXTMESSAGE | 29 | (HUD text, see §2) | What `set_hudmessage` uses under the hood. |

Limits and costs:
- `life` is a **byte in 0.1 s** -> max 25.5 s; beams with life 0 last ~1 frame (on some builds "forever" – avoid). Refresh repeating shapes every 0.5–1 s with overlapping life.
- Width byte in 0.1 units → max 25.5 units thick; brightness 0–255; noise 0–255 (0 = straight).
- Coords are sent as `coord` (short, 1/8 unit precision on the stock protocol), range ±4096 → fine for any map.
- Client caps (engine, from HL SDK/engine headers): ~500 temp entities (`MAX_TEMP_ENTITIES`) and a fixed beam pool (128 beams in the classic engine) *(verify on 2013+ build)*; extra beams are silently dropped. One BEAMPOINTS message is ~30 bytes.
- Visibility: temp entities have no PVS culling on the client once received; beams are drawn regardless of distance (no LOD), but **MSG_PVS/MSG_PAS** sends only to clients whose PVS contains the origin – use MSG_ALL/MSG_BROADCAST or per-player MSG_ONE for map-wide zones. Beams are clipped by walls only through normal depth (they do not draw through walls).
- Network: per-client datagram ≈ 1400 bytes per frame on the unreliable channel; the reliable buffer overflows at ~4000 bytes and **kicks the client** ("Reliable channel overflowed"). A 32-segment ring = ~1 KB, so send it unreliable and at most ~1/s.
- Sprite indices come from `precache_model("sprites/xxx.spr")` (see §9).

## 2. HUD text

`set_hudmessage(r,g,b, x,y, effects, fxtime, holdtime, fadein, fadeout, channel)` + `show_hudmessage`:
- **4 channels** (1–4); `-1` = AMXX auto-picks. A new message on a channel replaces the old one. Plugins share these 4 – use `CreateHudSyncObj` / `ShowSyncHudMsg` to avoid fighting other plugins.
- x/y are 0.0–1.0, `-1.0` = centre. effects 0 fade, 1 flicker, 2 typewriter.
- Text limit: engine TE_TEXTMESSAGE text is ~512 bytes; AMXX formats into a buffer and the client shows ~479 visible chars; keep it short. Uses the HUD font only, no colour per word.
- Source: https://www.amxmodx.org/api/amxmodx/set_hudmessage

Director HUD (`set_dhudmessage` / `show_dhudmessage`, AMXX 1.8.3+, uses SVC_DIRECTOR / DRC_CMD_MESSAGE):
- Bigger, bolder font, not clipped by HUD channels.
- **8 slots per client** (client-side ring buffer); the 9th overwrites the oldest. Text max **128 chars**.
- Source: https://www.amxmodx.org/api/amxmodx/set_dhudmessage

Also: `client_print(id, print_center|print_chat|print_console, …)` (TextMsg, 192-byte user-message limit), `client_print_color` (SayText with ^1–^4 colours, team colour from a player index).

User-message payload limit: **192 bytes** per user message (`MAX_USER_MSG_DATA`).

## 3. Menus

- `show_menu(id, keys, text, time, title)` → ShowMenu message: key bitmask of 10 keys (1–9, 0 = bit 9). Text up to ~512 chars; engine splits into 175-char chunks.
- Only **10 selectable slots on screen**. Colours via `\r \y \w \d` and `\R` right-align.
- newmenus (`menu_create` etc.): 7 items per page by default with Back/Next/Exit; `MPROP_PERPAGE` 0 = no pagination (max 10 items incl. exit), otherwise ≤7. Total item count is effectively unlimited (paged).
- Only one menu visible per player; opening another replaces it. Bots never see menus.
- Source: https://www.amxmodx.org/api/newmenus

## 4. StatusText / StatusValue (the "Friend: name Health: 100%" line)

- `StatusValue` (byte index 1–3?, short value) sets variables; `StatusText` (byte line 0, string) sets a format with `%p1` (player name by index in value 1), `%i2` (integer), `%h:` prefix etc. Max string ~ 128.
- CS normally rewrites it when you aim at players (mp_playerid); custom text gets overwritten unless you block the native updates (`set_msg_block`/register_message).
- Source: https://wiki.alliedmods.net/Half-Life_1_Game_Events#StatusText

## 5. Scoreboard: ScoreAttrib, ScoreInfo, TeamInfo

- `ScoreAttrib` (byte player, byte flags): 1 = DEAD, 2 = BOMB, 4 = VIP. Shows "Dead"/"Bomb"/"VIP" in the scoreboard; VIP flag also gives a "VIP" tag. Only for the receiving client's team visibility rules (bomb only shown to Ts by CS logic; message itself can be sent to anyone).
- `ScoreInfo` (byte id, short frags, short deaths, short classid, short teamid) – can show arbitrary numbers in the kills/deaths columns (e.g. "zone kills").
- `TeamScore` (string team, short score) – team header score.
- Source: https://wiki.alliedmods.net/Counter-Strike_1.6_Events (CS events list)

## 6. Radar

- Stock CS radar shows **teammates only** (positions updated by the `Radar` user message: byte player, coord x,y,z, sent ~1/s by the game DLL), the **bomb** (`BombDrop`: coord x,y,z, byte flag; `BombPickup`), **hostages** (`HostagePos`: byte flag, byte id, coord x,y,z; `HostageK`), and the VIP for CTs.
- Plugins can send `Radar` with any player index and any coordinates → a dot at an arbitrary point (marker trick). `HostagePos` gives up to MAX_HOSTAGES (~20 ids) fake "hostage" dots — usable as objective markers *(verify they render without a real hostage)*. Dots flash red for `BombDrop` planted flag=1.
- Radar dots are drawn in the radar circle (relative to the player); enemies are never shown unless you send their position (ReGameDLL `mp_radar_mode` / forcecamera options don't add that natively *(verify)*).
- No way to draw shapes (circles/zones) on the radar.

## 7. Overview (the full-screen map, `overview_mode`/spectator, and the CZ/1.6 overview background)

File `cstrike/overviews/<map>.txt` (CZ: `czero/overviews/`):
```
// overview description file for de_dust2.bsp
global
{
	ZOOM	1.2
	ORIGIN	-220 -880 0
	ROTATED	0
}
layer
{
	IMAGE	"overviews/de_dust2.bmp"
	HEIGHT	-100
}
```
- `ZOOM`: scale; at zoom 1.0 the image covers 8192 world units across (4096 / zoom per half-screen) – generated by `dev_overview 1` in a listen server, which prints ZOOM/ORIGIN.
- `ORIGIN`: world point at image centre. `ROTATED 1` swaps axes (for maps longer in Y).
- Multiple `layer` blocks with different `HEIGHT` are allowed, but the stock client uses only the first/closest layer in practice.
- Image: **8-bit indexed BMP (256 colours) or 24-bit TGA, 1024×768** (stock files are 1024×768 BMP; TGA works on Steam builds *(verify)*).
- Loaded client-side when entering spectator overview / at map load from the local filesystem (searches `cstrike_downloads` too).
- Server push: overview files are not auto-sent, but a `maps/<map>.res` listing `overviews/<map>.txt` and `overviews/<map>.bmp` makes clients with `cl_allowdownload 1` download them (works with FastDL `sv_downloadurl`). Common practice on custom-map servers.
- Used by spectators (overview mode) only; it does **not** replace the in-game radar background in stock 1.6 (radar is a plain circle). The CZ client is the same.
- Source: https://developer.valvesoftware.com/wiki/Level_Overviews_(GoldSrc) and https://twhl.info/wiki/page/Overviews

## 8. BarTime, fade, shake

- `BarTime` (short seconds) – the bomb-defuse style progress bar; 0 hides it.
- `BarTime2` (short seconds, short start percent) – resume from a percent.
- `ScreenFade` (short duration, short holdtime, short flags, byte r,g,b,a): durations in **1/4096 s units (1<<12 = 1 s)**, short max ≈ 16 s. Flags: 0 FFADE_IN, 1 OUT, 2 MODULATE, 4 STAYOUT.
- `ScreenShake` (short amplitude, short duration, short frequency) – all `<<12` fixed point.
- Also `Damage` HUD arrows, `SetFOV` (byte), `Crosshair`/`HideWeapon` (byte flags: 1 weapons, 2 flashlight, 4 all, 8 radar/health/armor, 16 timer, 32 money, 64 crosshair) – HideWeapon can hide radar, timer, money.
- `RoundTime` (short seconds) – sets the round timer display (useful as zone countdown).

## 9. Custom sprites / sounds / models

- Precache in `plugin_precache`: `precache_model` (models + sprites), `precache_sound`, `precache_generic`.
- Engine limits: **512 models (incl. sprites and brush submodels), 512 sounds**, generic ~512; going over crashes the server ("PF_precache_model_I: Model overflow"). Stock CS maps already use ~100–200 model slots.
- Clients download missing files on connect if `sv_allowdownload 1` (slow, in-engine) or via `sv_downloadurl` FastDL (HTTP). File types allowed: .mdl .spr .wav .bmp .tga .txt .res .wad etc. (no .dll/.exe/.cfg blocked list in ReHLDS).
- Sounds: WAV PCM 8/16-bit mono, 11/22/44 kHz; MP3 via `mp3 play` (client cmd, needs file present).
- Sprites: .spr (rendermodes additive/indexalpha/alphatest), power-of-2 dimensions recommended, ≤256×256 per frame safest.

## 10. How existing 1.6 mods show a shrinking zone / objective markers

- **Ring of beams**: most battle-royale / "zone" plugins draw the safe circle as N `TE_BEAMPOINTS` segments (16–48) refreshed every ~1 s with life ≈ 1.2 s, optionally two rings (ground + head height). Sent unreliable per player to only those near the edge to save bandwidth.
- **Expanding cylinder pulses**: `TE_BEAMCYLINDER` from center outward to show the radius periodically (the "shockwave" look) — cheap, one message.
- **Entity-based wall**: a custom .mdl cylinder (e.g. "zone" model) with `pev->scale` doesn't scale models in GoldSrc *(only sprites scale)*; mods instead ship several pre-sized models or use `env_sprite`/`env_beam` entities. Persistent `env_beam` entities stay networked without resending.
- **Objective markers**: `TE_PLAYERATTACHMENT` / `env_sprite` with `kRenderTransAdd` + `kRenderFxNoDissipation`, visible through walls only if `rendermode` glow (`kRenderGlow` draws through walls, size-constant with distance) – classic "waypoint" trick. Plus DHUD distance text, fake radar dots (§6).
- **Damage outside zone**: ScreenFade red pulse + `Damage` message.
- Example sources: AlliedModders "Battle Royale" threads https://forums.alliedmods.net/search.php?searchid=&q=battle+royale+cs+1.6 , "Zone" / "Safe Zone" plugins; ZP (Zombie Plague) uses TE_BEAMCYLINDER for grenades: https://github.com/zombieplague/zombieplague (verify specific repo).

## Takeaways for Loner-Strike
1. Zone: beam-segment ring (unreliable, ≤1/s) + RoundTime/DHUD countdown + red ScreenFade outside.
2. Objectives: glow sprites (through walls) + fake radar dots + TE_PLAYERATTACHMENT for team icons.
3. Ship custom overviews via `.res` + FastDL; they help spectators only.
4. Stay within 4 HUD channels (use sync objects) and 8 DHUD slots; keep every user message < 192 bytes.
