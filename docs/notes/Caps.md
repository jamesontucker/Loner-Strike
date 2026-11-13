# Caps: size limits on everything people hand the app (audit 2026-11-13)

Rule: generous caps that only stop hostile or broken files, never real content. Every new
refusal says what was too big and what the most is ("That picture is too big: 9000×9000 — the
most is 8192×8192."). New code is marked `[Caps]`. Shared helpers: `app/src-tauri/src/caps.rs`
(`read_file`/`read_text`: size checked before reading; `read_up_to`/`text_up_to`: network and
child answers; `decode_picture`: pixel size from the header before any decoding, plus
`image::Limits`). Tests: `cargo test caps::` and `archive::`, `spray::`, and
`services/serverlist/test_caps.py`.

## Real files measured (what the caps were checked against)

| Type | Largest real file found | Its cap |
|---|---|---|
| .bsp | 13.1 MB (zm_silent_hill) of 596 maps | 64 MB (goldsrc MAX_BSP) |
| .wad | 37.9 MB (valve/halflife.wad) of 111 | 256 MB (MAX_WAD); custom WAD 64 MB |
| .mdl | 4.9 MB (a player model) of 2261 | 64 MB (MAX_MDL); map assets 32 MB |
| .wav | 6.9 MB of 5590 | 32 MB (map assets, inspector play); 8 MB per added music track |
| .spr | 3.0 MB of 876 | 8 MB (map assets) |
| .dem | 66.6 MB (TopFrag-DUST2only.dem) | 1 GB (scan, serve, SFTP read) |
| .nav | 2.9 MB (de_piranesi_cz) of 316 | 32 MB (MAX_NAV) |
| .lsv | 0.36 MB (cs_rats2) | 512 MB (MAX_LSV) |
| .map | 2.3 MB (maps-src/br_valley) | 64 MB (editor open); 8 MB per prefab |
| graph .json | 136 KB (every_node test graph) | 2 MB |
| overview/picture | 1024×768, 2.3 MB (all 2230 TGA/BMP in the game) | 8192×8192 pixels, 64 MB file |
| zip packing ratio | de_dust2.bsp 2.1x, halflife.wad 1.4x, a .dem 4.0x, a GameBanana map zip 1.6x | refuse only >200x on archives over 256 MB |
| GameBanana package | 132.7 MB (biggest CS 1.6 item found: a multi-map "mega mappack"; single maps < 50 MB) | 256 MB (community check and Publish) |

GameBanana's own upload limit (forum answers from staff, not an official page, may be dated):
per section — about 1.46 GB for Mods, 500 MB for Sounds, 200 MB for WIPs; very large uploads
often time out. So our 256 MB Publish/check cap is the binding one, well under GameBanana's.
Sources: gamebanana.com/questions/44998, gamebanana.com/questions/64498, gamebanana.com/threads/220563.

## What was missing, and what's set now

| Where | Input | Was | Risk | Now |
|---|---|---|---|---|
| archive.rs `extract_zip`, bsdtar, unar, 7-Zip paths | any archive (GameBanana, sounds, picked files, prefab/community packages) | 3 GB total only; **no entry count, no ratio** | inode/CPU exhaustion (millions of tiny files); a 1 MB download filling 3 GB | `bomb_check`: ≤ 65,536 files; > 200:1 refused once over 256 MB unpacked (archive.rs `MAX_ENTRIES`, `MAX_RATIO`) |
| maps.rs `convert` | game/GameBanana TGA/BMP/JPEG shots and overviews | **no file cap, no pixel cap** | memory (decompression bomb) | 64 MB file, 8192×8192, 300 MB decode |
| tools/community.rs `decode_shot` | Publish screenshots | 8 MB base64; **no pixel cap** | memory | 8192×8192 via `decode_picture` |
| tools/community.rs publish route | Publish request with screenshots | **axum's 2 MB default** — real screenshots (up to 6 × 8 MB, the page's own limit) were refused with a bare 413 | broken feature | `DefaultBodyLimit` = `PUBLISH_BODY` (6 × base64 8 MB + 64 KB ≈ 64 MB) |
| tools/community.rs `build_package` | the finished package, read whole for MD5 | 1 GB uncompressed sum; **zip read back with no cap** | memory up to 1 GB; a package the list can never check | finished zip ≤ 256 MB (`MAX_PACKAGE_ZIP`, matches the list's CHECK_MAX), checked before reading; too-big zip deleted |
| tools/overview.rs `current_rgb` | installed overview picture | 32 MB file; **no pixel cap** | memory | 8192×8192 (and TGA read as TGA) |
| editor/prefab.rs `check_png` | prefab thumbnails (imports, community) | 1 MB file; dims checked **after** decoding | memory (1 MB PNG can be 30000²) | 1024×1024 checked from the header first |
| tools/editor/prefabs.rs | package's prefab .json / .png sidecars; WADs in a package | **none**; WAD read whole then checked | memory | 1 MB json, 1 MB png; WAD size by metadata, only 4 header bytes read |
| tools/overview.rs, health.rs, navfix.rs | .nav reads; navfix's .lsv | **none** (other .nav reads use 32 MB) | memory | 32 MB nav, 512 MB lsv |
| tools/plugins.rs open, rename | .sma/.inc | read whole, **then** 2 MB check; rename none | memory | checked before reading (2 MB / 8 MB) |
| tools/editor.rs inspector sound | .wav/.mp3 | read whole, **then** 32 MB check | memory | checked before reading |
| sounds.rs pool file serve | music pool | read whole, **then** 32 MB check | memory | checked before reading |
| tools/doctor.rs copy fix | game files copied between folders | **none** (whole file in RAM) | memory | 256 MB (MAX_WAD) |
| tools/communitylib.rs remix / to server | installed community graphs | **none** (2 MB at install) | memory | 2 MB |
| tools/spray.rs designs | saved designs | 200 designs, 256 MB total; open uncapped | — | **no count limit** (user), **512 MB total** (user), 24 MB each unchanged; open capped at 24 MB |
| hltv.rs demo download | local .dem | **none** | memory | 1 GB (still whole in memory; streaming would be better) |
| remote.rs `read_file`/`read_file_opt`, `exec` | SFTP files, ssh output from the person's server | **none** | memory | 1 GB per file, 32 MB per command output |
| rcon.rs UDP reply | game server answer | 1.5 s per packet; **no total** — a server that keeps sending held the lock forever | hang + memory | 4 MB and 20 s per answer |
| webstatus.rs, control/client.rs, control/agent.rs | status page (typed domain), invite redeem, ping | **none** (`.text()`/`.json()`) | memory | 1 MB / 1 MB / 64 KB |
| gamebanana.rs API pages, archive listing | GameBanana JSON | **none** | memory (trusted host, low) | 4 MB (same as the community service) |
| mcp/stdio.rs | lines from the agent; the app's answer | **none** (unbounded line, `read_to_end`) | memory | 4 MB a line (over: JSON-RPC error, the bridge carries on); 64 MB an answer |
| services/serverlist/formats_runner.py | everything in a package together | 256 MB per entry; **no total** | temp disk/CPU (many 256 MB entries) | 1 GB declared total (entries can't exceed what they declare) |
| services/serverlist/community.py `CHECK_MAX` | package downloaded for checking | 128 MB | real packs refused | **256 MB** (user). One check at a time: ≤ 256 MB package + ≤ 256 MB entry in PrivateTmp; the child's 256 MB address space is unaffected (streamed); 180 s for download + check needs ~2 MB/s from GameBanana's file hosts |

## Already capped (verified, unchanged)

| Where | Input | Cap |
|---|---|---|
| server.rs / all `Json`/`Bytes` routes | HTTP bodies | axum 0.7 default 2 MB (no global override); overrides: screenshot ≈64 MB (shot.rs:77), spray use 8 MB, design save 24 MB (spray.rs), Publish ≈64 MB (new) |
| mcp/mod.rs `/mcp` | MCP request body | 2 MB default; tool output trimmed to ~48 KB (catalog.rs, paged; kept per user), pictures 4 MB |
| mcp/catalog.rs | in-process route answers | 512 MB (the app's own output, then trimmed) — left as is |
| gamebanana.rs | map/model/sound download; picked local file | 1.5 GB streamed to disk, host allowlist, 1 h; unpack 3 GB |
| setup/fetch.rs | component downloads + unpack | 2 GB, sha256-pinned (no entry count: sources are pinned) |
| smartchat/runtime.rs | model download, tar unpack | 5 GB / 1 GB, sha256-pinned |
| sounds.rs | sound pack download / unpack; added tracks | 64 MB / 256 MB; 8 MB per track |
| navpack.rs | nav pack | 64 MB |
| backup.rs | backup import | 20 MB JSON, 50 MB per inner JSON, 2 MB configs, 2 GB per file, 500k entries |
| control/client.rs | proxied owner reply | 32 MB |
| goldsrc/* parsers | BSP, WAD, MDL, nav, lsv, lightmap, mesh | every count/length checked against the file or a MAX_* before allocating (no small-file huge-allocation found) |
| editor/mapfile.rs, rmf.rs, tools/editor.rs | .map/.rmf/.jmf open | 64 MB file; rmf counts ≤ 1M, depth 64 |
| tools/editor/components.rs, kits.rs | component packs, kits | 4 MB, 256 KB |
| tools/studio | graphs, snippets, library imports | 2 MB |
| tools/creatures.rs, playercfg.rs, people.rs | Creature Studio .ini, configs | 1 MB, 1 MB, 4 MB |
| tools/spray.rs | picked picture; drawn PNG | 40 MB + 8192²; 8 MB + 2048² |
| tools/render.rs | sky pictures | 32 MB, 2048², 64 MB decode |
| tools/demos.rs | .dem parsing | 1 GB streamed, 128 KB per message |
| tools/replay.rs | traces | 200 MB, 4 KB lines |
| tools/community.rs | community index; GameBanana JSON; prefab/mode download | 8 MB; 4 MB; 128 / 64 MB (unpack 512 MB) |
| JSON depth | every serde_json parse | serde_json's built-in 128-level recursion limit |
| services/serverlist | all POST bodies, zip directory, manifest, rate limits, UDP | 64 KB body, 1 MB directory (kept), 64 KB manifest (kept), 5000 entries, 4 MB GameBanana pages; see community.py constants |

## Left as they are (low risk, noted)

- GameBanana map installs read nested archives with the remaining 3 GB budget; prefab/mode
  downloads unpack 512 MB, then nested archives get the rest of 3 GB.
- browser.rs / sharing.rs / smartchat local llama answers: `.text()` with timeouts only (our own
  list service, playit, localhost).
- servers.rs SteamCMD output: no timeout (Valve's tool).
- compile.rs reads its own .pts/.bsp output whole (the compiler's own files).
- services/serverlist: the `log` table has no row limit (rate limits slow it); Discord API
  answers read without a cap (trusted host).
- Demo download is still read whole (≤ 1 GB); streaming it is the better fix later.
