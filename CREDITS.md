# Credits

Loner-Strike only exists because of these projects and their authors. The game modes' and add-ons' source code ships with the app (in `mods/`), patched for Condition Zero's bots and compiled on your computer; their models, sounds and other resources are downloaded from the original sources at install time, pinned to one commit or release and checked by sha256. Our patches are under the same licenses as the originals.

## Loner-Strike's own code
The app, the control panel, the website and the AMX Mod X plugins in `plugins/` (Game Mode Menu, bot styles, bot chat, bot navigation, bot pings, pub server, deathmatch, match mode, VIP, community, server extras) are ours, under the [GPL-3.0](LICENSE). So are the bot names (`regular_names.ini`), the bot chat lines (`botchat.ini`), the bot profile made for Counter-Strike 1.6 and the bot style files, the app icon and logo, and the icons in the app and on the website.

| Adapted from | Authors | Where |
|---|---|---|
| Ping Faker | MeRcyLeZZ (AlliedModders) | Bot pings on the scoreboard (gm_botping) send the pings message the way Ping Faker does |
| CSDM spawn files | BAILOPAN and the CSDM team (AlliedModders) | Deathmatch (gm_deathmatch) reads CSDM's `<map>.spawns.cfg` files when a map has one. CSDM itself is not used |

## Foundations
Downloaded at setup, one pinned version each.

| Project | Version | Authors | License |
|---|---|---|---|
| [AMX Mod X](https://github.com/alliedmodders/amxmodx) (base and Counter-Strike addon) | 1.10.0 build 5487 | AMX Mod X Development Team (AlliedModders) | GPL-3.0-or-later, with the Half-Life linking exception |
| [Metamod-P](https://github.com/Bots-United/metamod-p) | 1.21p109 | Will Day, Jussi Kivilinna, and Bots-United | GPL-2.0-or-later |
| [ReGameDLL_CS](https://github.com/rehlds/ReGameDLL_CS), in Loner-Strike's fork ([jamesontucker/ReGameDLL_CS](https://github.com/jamesontucker/ReGameDLL_CS), branch loner-strike: the bot brain) | 5.30.0.814 | s1lent and the ReGameDLL_CS contributors; the bot changes by Loner-Strike | MIT |
| [ReHLDS](https://github.com/rehlds/ReHLDS), in Loner-Strike's fork ([jamesontucker/ReHLDS](https://github.com/jamesontucker/ReHLDS), branch loner-strike: the voice feed), for dedicated servers | 3.15.0.896 | dreamstalker, s1lent and the ReHLDS contributors | MIT |
| [ReAPI](https://github.com/rehlds/ReAPI), for dedicated servers | 5.29.0.358 | s1lent, asmodai and the ReAPI contributors | GPL-3.0 |
| [relocalizebugfix](https://github.com/rehlds/relocalizebugfix), for dedicated servers | 2.8 | the ReHLDS team | MIT |

## Game modes
| Mode | Authors | Source | License |
|---|---|---|---|
| Gun Game 2.13c | Avalanche | [gungame-amxx.org](https://gungame-amxx.org/), [AlliedModders](https://forums.alliedmods.net/showthread.php?t=49983) | Not stated in the source |
| Zombie Plague 4.3 Fix5a | MeRcyLeZZ | [AlliedModders](https://forums.alliedmods.net/showthread.php?t=72505); models and sounds from [ecks3/zp508-similar-to-zod](https://github.com/ecks3/zp508-similar-to-zod) | GPL-3.0-or-later |
| Zombie Plague 5.0.8 (Advanced) | MeRcyLeZZ and the ZP Dev Team; CS APIs by WiLS | [MultiModServer](https://github.com/evandrocoan/MultiModServer); Sniper and Assassin, models and resources from [ecks3/zp508-similar-to-zod](https://github.com/ecks3/zp508-similar-to-zod) | GPL |
| Warcraft 3: Frozen Throne | Geesu, Avanderik, YamiKaitou, and the [wc3mods](https://github.com/wc3mods/war3ft) team | [GitHub](https://github.com/wc3mods/war3ft) | GPL-2.0-or-later |
| SuperHero Mod 1.2.1 | {HOJ} Batman, JTP10181, vittu, AssKicR and the SuperHero Team; each hero by its author (named in its plugin) | [MultiModServer](https://github.com/evandrocoan/MultiModServer); hero models and sounds from [OneAimlessRobot/my_amxx_plugins](https://github.com/OneAimlessRobot/my_amxx_plugins) | Not stated in the source |
| HideNSeek 2.8 | Exolent | [AlliedModders](https://forums.alliedmods.net/showthread.php?t=65370), via [MultiModServer](https://github.com/evandrocoan/MultiModServer) | Not stated in the source |
| Paintball Elimination 2016 | Sneaky.amxx (Dias), GlobalModders | [GitHub](https://github.com/GlobalModders/CS1.6-AMXX-PaintBall-Elimination-2016-By-Sneaky.amxx) | GPL-3.0 |
| ProKreedz 2.31 | nucLeaR, p4ddY | [AlliedModders](https://forums.alliedmods.net/showthread.php?t=130417) | Not stated in the source |
| Deathrun Manager 3.0.3, trigger_hurt fix, Reset Buttons | xPaw, coderiz | AlliedModders, via [MultiModServer](https://github.com/evandrocoan/MultiModServer) | Not stated in the source |
| Base Builder TURBO | KoRrNiK, with Albertd and R3X (barrier); based on Tirant's Base Builder; napalm grenades after MeRcyLeZZ | [GitHub](https://github.com/KoRrNiK/BaseBuilder-TURBO) | Not stated |
| SoccerJam 2.07a | OneEyed | [OneAimlessRobot/my_amxx_plugins](https://github.com/OneAimlessRobot/my_amxx_plugins) | Not stated in the source |

Classic, Deathmatch, Free for all, Knife, Pistols, Deagle, AWP, Scoutz & Knivez, Surf and Match are ours. Kreedz's no-bots helper, Hide and Seek's bot stealth and Deathrun's bot traps are ours too.

The MultiModServer collection as a whole is published under GPL-3.0. Warcraft 3's sounds (also used for Base Builder's frost grenades) come from the War3FT repository.

## Add-ons
| Add-on | Authors | Source | License |
|---|---|---|---|
| D.I.C.E. (Roll the Dice) | OciXCrom | [GitHub](https://github.com/OciXCrom/DICE); Loner-Strike ships only its changes and applies them to the original at install | Not stated upstream |
| Round-end music | based on REMusic 2.3.1 by OciXCrom | [GitHub](https://github.com/OciXCrom/REMusic) | Not stated upstream |
| Ultimate Gore Enhancement 1.6 | mike_cao, fizzarum, jtp10181 | [jtpage.net](http://www.jtpage.net), as published on [Pastebin](https://pastebin.com/raw/vUSkR2e6) | GPL-2.0-or-later |
| CSB Grappling Hook 1.0.0 | counter-strike-boost.com | [counter-strike-boost.com](https://counter-strike-boost.com/plugins/csb-grappling-hook) | GPL-3.0-or-later |
| Quake sounds (miscstats) | AMX Mod X Development Team | Comes with AMX Mod X | GPL-3.0-or-later |
| Quake sounds' announcer | Hattrick (ClaudiuHKS) | The announcer sounds from [Advanced Quake Sounds](https://github.com/ClaudiuHKS/AdvancedQuakeSounds), downloaded at install from a fixed commit and checked | MIT |

Round-end music ships with freely licensed tracks from OpenGameArt by Tri-Tachyon, marcelofg55 and OwlishMedia (CC BY 4.0) and Emma_MA, Umplix and Zane Little Music (CC0); each track's author, license and link is listed in `sound/roundsound/CREDITS.txt`.

## Sound packs
The Sounds page offers these packs from [GameBanana](https://gamebanana.com). They are downloaded only when you pick one, straight from GameBanana, and belong to their creators.

| Pack | Author |
|---|---|
| [Rammstein CT and T win pack](https://gamebanana.com/sounds/14773) | K3PT3L3N |
| [End Round Music Cues](https://gamebanana.com/sounds/82836) | OffTheVoid |
| [End Round sound](https://gamebanana.com/sounds/57946) | opera |
| [Kill sounds pack (UT recreation)](https://gamebanana.com/sounds/90672) | Mike Isakov |
| [CS:GO CT and T End Round Sounds](https://gamebanana.com/sounds/36543) | MatasM82 |
| [CT Win Radio Sound (SWAT Team Wins)](https://gamebanana.com/sounds/21823) | Ghost968373 |
| [SWAT Radio Voices](https://gamebanana.com/sounds/25095) | Xander9393 |
| [CS:GO Radio & Bot Chatter](https://gamebanana.com/sounds/31143) | hellmonja |

## Maps
Maps are downloaded from [GameBanana](https://gamebanana.com) and belong to their creators. The panel shows each map's author. Zombie Plague Advanced suggests zm_silent_hill, zm_zod_labtest, zm_ice_attack3 and zm_deko2, by their authors.

## From your own game
Setup's music (Half-Life's menu theme and pieces of its soundtrack) and the app's button sounds are played from your own copy of Half-Life; Loner-Strike doesn't include them. The game and its files are Valve's.

## Dedicated servers
For a server on this computer or on a Linux machine, the app installs Valve's [SteamCMD](https://developer.valvesoftware.com/wiki/SteamCMD) and the Half-Life Dedicated Server (with HLTV) from Steam, under Valve's terms. For a server's status website it can set up [Caddy](https://github.com/caddyserver/caddy) (Apache-2.0) from the Linux distribution or Caddy's own package repository, or [cloudflared](https://github.com/cloudflare/cloudflared) (Apache-2.0) from Cloudflare's.

## Play with friends
| Service | Notes |
|---|---|
| [playit.gg](https://playit.gg) | Tunnels run through playit.gg's service with playit's own agent, [playit-agent](https://github.com/playit-cloud/playit-agent) v1.2.0 by Developed Methods (BSD-2-Clause), shipped inside the app: built from its source for Mac, and playit's own signed build for Windows. Copyright 2022 Developed Methods LLC |
| [7-Zip](https://www.7-zip.org) | 7-Zip by Igor Pavlov (GNU LGPL, with the unRAR licence restriction for its RAR code), shipped inside the Windows app to unpack map downloads Windows 10's own tar can't open. Unchanged, with its licence beside it |
| [Ham Sandwich](https://github.com/alliedmodders/amxmodx) | AMX Mod X's Ham Sandwich module (GPL-3.0), rebuilt for Windows and for Linux servers with a fix for a crash on map changes; the patch and how each was built are in app/src-tauri/resources/windows/hamsandwich and resources/linux/hamsandwich |
| [Tailscale](https://tailscale.com) | The app only reads Tailscale's status for this computer's address |

## Advanced Bot Interaction
| Part | Notes |
|---|---|
| [whisper.cpp](https://github.com/ggml-org/whisper.cpp) | Speech to text in ls-voice, by Georgi Gerganov and contributors (MIT), built in through [whisper-rs](https://github.com/tazz4843/whisper-rs) (Unlicense). The model is OpenAI's Whisper base.en (MIT), whisper.cpp's own copy, downloaded when the add-on is installed |
| [Opus](https://opus-codec.org) | Decoding players' voice in ls-voice: libopus (BSD-3-Clause) through the [opus](https://github.com/SpaceManiac/opus-rs) crate (MIT/Apache-2.0) |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | Runs the understanding model in ls-intent (MIT), through [llama-cpp-2](https://github.com/utilityai/llama-cpp-rs) (MIT/Apache-2.0) |
| [Gemma](https://ai.google.dev/gemma) | The understanding model is Loner-Strike's fine-tune of Google's Gemma 3 270M, downloaded when the add-on is installed. Gemma is provided under and subject to the Gemma Terms of Use found at ai.google.dev/gemma/terms, and the Gemma Prohibited Use Policy |

## Website
| Image | Author | License |
|---|---|---|
| [DreamHack, winter 2004](https://commons.wikimedia.org/wiki/File:Winter_2004_DreamHack_LAN_Party.jpg) | Toffelginkgo | [CC BY-SA 3.0](https://creativecommons.org/licenses/by-sa/3.0/) |

The radio, in the app and on the website, plays the stations' own streams; nothing is copied or stored.

| Station | Notes |
|---|---|
| [Bassdrive](https://www.bassdrive.com/) | Streamed from Bassdrive |
| [Kniteforce Radio](https://kniteforce-radio.com/) | Streamed from Kniteforce Radio |
| [TechnoBase.FM](https://www.technobase.fm/) and [HardBase.FM](https://www.hardbase.fm/) | Streamed from the We aRe One webradios |
| [SomaFM: The Trip](https://somafm.com/thetrip/), [DEF CON Radio](https://somafm.com/defcon/) and [Groove Salad](https://somafm.com/groovesalad/) | Streamed from SomaFM, listener supported |
| [PsyMusic UK: PsyStream](https://www.psymusic.co.uk/) | Streamed from PsyMusic UK's psytrance station |
| [SceneSat Radio](https://scenesat.com/) | Streamed from SceneSat |
| [Rainwave](https://rainwave.cc/) | Streamed from Rainwave's game music channel |
| [0nlineradio: 2000s](https://www.0nlineradio.com/) | Streamed from 0nlineradio |
| [Radio Bob: Nu Metal](https://www.radiobob.de/) | Streamed from Radio Bob |

## Fonts
| Font | Author | License |
|---|---|---|
| [Counter-Strike](https://www.dafont.com/counter-strike.font) (titles in the app and on the website) | SoJa | 100% free (dafont) |
| [Barlow Semi Condensed](https://github.com/jpt/barlow) | Jeremy Tribby | SIL Open Font License 1.1 |
| [Big Shoulders Stencil Display](https://github.com/xotypeco/big_shoulders) (fallback for titles in the app) | Patric King (XO Type) | SIL Open Font License 1.1 |

The website serves its own copies of the fonts; the app loads Barlow Semi Condensed and Big Shoulders from Google Fonts.

## Libraries
The app is built with these, among many smaller ones. Every Rust crate and npm package, with its version, is in `app/src-tauri/Cargo.lock` and `app/ui/package-lock.json`, and each one's license is in its package.

| Library | License |
|---|---|
| [Tauri](https://tauri.app) and its plugins | Apache-2.0 or MIT |
| [axum](https://github.com/tokio-rs/axum), [Tokio](https://tokio.rs) | MIT |
| [reqwest](https://github.com/seanmonstar/reqwest), [rustls](https://github.com/rustls/rustls), [ring](https://github.com/briansmith/ring) | MIT or Apache-2.0; rustls also ISC; ring Apache-2.0 and ISC |
| [ssh2](https://crates.io/crates/ssh2), with bundled [libssh2](https://libssh2.org) and [OpenSSL](https://www.openssl.org) | MIT or Apache-2.0; libssh2 BSD-3-Clause; OpenSSL Apache-2.0 |
| [zip](https://github.com/zip-rs/zip2), [image](https://github.com/image-rs/image), [qrcode](https://github.com/kennytm/qrcode-rust), [igd-next](https://github.com/dariusc93/rust-igd) | MIT, or MIT or Apache-2.0 |
| [React](https://react.dev) | MIT |
| [TanStack Query](https://tanstack.com/query) | MIT |
| [zustand](https://github.com/pmndrs/zustand) | MIT |
| [Vite](https://vite.dev), [TypeScript](https://www.typescriptlang.org) (build only) | MIT; Apache-2.0 |

## Not in this release: Smarter chat
Smarter chat (a local model writing bots' replies) is in the code but hidden in this release, so nothing below is downloaded. When it returns, it will download these only when you pick a model.

| Project | Authors | License |
|---|---|---|
| [llama.cpp](https://github.com/ggml-org/llama.cpp) (llama-server, release b11722) | Georgi Gerganov and contributors | MIT |
| [Llama 3.2 1B](https://huggingface.co/meta-llama/Llama-3.2-1B), GGUF by [QuantFactory](https://huggingface.co/QuantFactory/Llama-3.2-1B-GGUF) | Meta | Llama 3.2 Community License |
| [Discord-Micae-Hermes-3-3B](https://huggingface.co/mookiezi/Discord-Micae-Hermes-3-3B), GGUF by [mradermacher](https://huggingface.co/mradermacher/Discord-Micae-Hermes-3-3B-GGUF) | mookiezi, on Hermes 3 (Nous Research) and Llama 3.2 (Meta) | Llama 3 Community License |
| [Gemma4NPC-E2B](https://huggingface.co/chimbiwide/Gemma4NPC-E2B-GGUF) | chimbiwide (npcLM), on Gemma 4 (Google) | Apache-2.0 |
