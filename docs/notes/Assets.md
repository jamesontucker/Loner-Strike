# Assets: licences, tracked credits, model browsing, creature models (research + proposal)

2026-11-13. Research and a design proposal only: no app code was changed. Sources are linked where they're used.
GameBanana numbers come from the public apiv11 (the same API `gamebanana.rs` uses), sampled on 2026-11-13.

**Rule** below means something written down by GameBanana or Valve. **Custom** means what people usually do,
taken from the samples. **Inference** is my reading. None of this is legal advice.

---

## 1. Licensing norms on GameBanana

### 1.1 GameBanana's own rules (these are rules)

Site Rules, https://gamebanana.com/rules:
- **R.10 Copyright**: "Submissions or messages that violate intellectual property, or are derived from other
  copyrighted work without permission, are not allowed." Fix: "Obtain permission from the copyright holder(s)…"
- **R.23 Absent Credits**: "Submissions must mention all authors involved and those that lack credits to all authors,
  including original authors, are not allowed. … Credits should be listed in the Credits module, rather than in the
  body text." Fix: "Credit all authors involved in the current submission and those involved in previous versions or
  sub-components of the work." (Edited about 3 months ago, so the Credits module rule is recent.)
- **R.16 DMCA Takedown**: content with a verifiable takedown must be removed.

Terms of Service, https://gamebanana.com/wikis/334:
- "You certify that you own all intellectual property rights in Your Content." The uploader gives **GameBanana** a
  licence to host and distribute it "to users seeking to download". That licence is for GameBanana only. It gives
  other users no right to redistribute anything.
- "GameBanana does not pre-screen or moderate all Submitted Content." So a page being on GameBanana proves nothing
  about its rights.

Porting Whitelist and Guidelines, https://gamebanana.com/wikis/755 (last edited 2023):
- Ported content is handled "via DMCA takedowns only". The whitelist is "a rough guideline … nothing more".
- **Confirmed** (they allow porting if it's not for profit, credit is given and their conditions are met): includes
  Valve "subject to conditions", Tripwire, id Software, Crytek, Ubisoft, Epic (a disclaimer is required), and others.
- **Grey Area** (not endorsed, but they usually don't act on non-commercial use): includes **Nexon**, Activision,
  Capcom, Blizzard, SEGA, Konami, Rockstar, Nintendo.
- **Definite No** (never allowed on GameBanana): Bohemia Interactive (outside their own games), **Cry of Fear**,
  Battlestate Games, America's Army.
- The "Guidelines to Porting" are rules: no defamatory use, no inappropriate use, and "proper credit must be given at
  all times … do not claim credit for anything you didn't explicitly make".

Valve's conditions, "Porting update - Valve", https://gamebanana.com/blogs/17948 (2015): non-profit, credit Valve,
nothing defamatory, and Valve can ask for removal at any time. Also: "This is an informal agreement … it does not
… allow you to use Valve assets on other websites or services."

Valve's Half-Life 1 SDK licence, https://github.com/ValveSoftware/halflife/blob/master/LICENSE: you may develop "a
modified Valve game running on the Half-Life 1 engine" and distribute it "but only for free". That's why stock HL and
CS content is fine *in* a mod people play with the game. It is not a licence to rehost Valve's files. This matches
the project rule in CREATURES.md: Valve's `.mdl`s never go on FastDL.

### 1.2 The permission fields on each submission, and what the API exposes

Every Mod and Sound `ProfilePage` (`/apiv11/Mod/<id>/ProfilePage`, `/apiv11/Sound/<id>/ProfilePage`) has:

| field | what it is |
|---|---|
| `_aLicenseChecklist` | `{yes: [...], ask: [...], only_if_same: [...], no: [...]}`: lines of plain text |
| `_sLicense` | free HTML the uploader wrote (often the Creative Commons badge) |
| `_aCredits` | `[{_sGroupName, _aAuthors: [{_sName, _sRole?, _idRow?/_sProfileUrl?}]}]`: the Credits module |
| `_bIsPorted` | the uploader's own "ported" tick box |
| `_bCreatedBySubmitter` | the uploader's own "I made this" tick box |
| `_aSubmitter` | `_idRow`, `_sName`, `_sProfileUrl`, and `_aDefaultLicenseChecklist` (that member's default) |
| `_aContributingStudios` | studios, usually empty |

The page's **License** tab (for example https://gamebanana.com/mods/license/223124) shows the four buckets under these
headings: **"You are free to"** (`yes`), **"Ask if you want to"** (`ask`), **"You may, if you offer the same
rights"** (`only_if_same`) and **"You may not"** (`no`). After them comes the `_sLicense` text and then
"**BY DOWNLOADING YOU AGREE TO THESE TERMS**". The checklist always has the same 8 lines. Only the bucket each line is
in changes, and the noun changes with the item type ("this Mod", "this Sound"):

1. Download and install this Mod
2. Redistribute this Mod on other sites
3. Modify this Mod and distribute the modified Mod on GameBanana
4. Modify this Mod and distribute the modified Mod on another site
5. **Use parts of this Mod in another Mod and distribute the Mod on GameBanana** (a map that ships the model)
6. **Use parts of this Mod in another Mod and distribute the Mod on another site** (our index/mirror, FastDL)
7. Use this Mod or components of this Mod in adult/NSFW content
8. Use this Mod or components of this Mod commercially

The app already fetches this exact response. `gamebanana.rs::details()` calls `/{kind}/{id}/ProfilePage` and reads
only the name, submitter, text, images and files. So reading licences and credits costs no extra request.

### 1.3 How common each permission is (real sample)

Sample: 400 CS 1.6 submissions, 80 from each group. In each group, 40 are the newest and 40 the most downloaded:
player models (cat 12761), all skins/models (8310), map props (8338), sprites (1247) and CS 1.6 sounds (game 4254).
Bucket per checklist line (y = yes, a = ask, n = no):

| line | yes | ask | only if same | no |
|---|---|---|---|---|
| install | 400 | 0 | 0 | 0 |
| redistribute on other sites | 9 | 378 | 0 | 13 |
| modify, distribute on GameBanana | 10 | 379 | 1 | 10 |
| use parts in another mod, on GameBanana | 10 | 379 | 1 | 10 |
| use parts in another mod, other site | 7 | 374 | 2 | 17 |
| NSFW | 3 | 376 | 0 | 21 |
| commercial | 3 | 4 | 0 | 393 |

- **357 of 400 (89%) have exactly the default shape** (install yes, everything else ask, commercial no). 13 more are
  the default plus NSFW no. 352 have the default `_sLicense`, *CC BY-NC-ND 4.0*. Items from 2008–2012 have it too, and
  the submitter object carries the same `_aDefaultLicenseChecklist`. So it's a default that GameBanana applies (and
  backfilled). Most uploaders never chose it. **Inference:** "ask" usually means "nobody said", not "the author wants
  to be asked". It still binds you: the page says "by downloading you agree".
- The two can contradict each other. Mod 90672 says "modify … if you offer the same rights" in the checklist, but its
  licence text is CC BY-NC-**ND** (no derivatives). Where they conflict, the stricter one should win.
- `_bIsPorted` was ticked on 136/400 (34%). Player models were highest at 47/80. **The flag is self-reported and
  unreliable**: Sound 21584 "CS:GO Full Weapons Sounds For CS 1.6" isn't flagged ported, yet its own credits say
  "Valve Corporation / Counter-Strike: Global Offensive". At least 17 more (names/credits saying CSO, CS:GO, Left 4
  Dead…) aren't flagged either. Credits groups such as "Original Authors" (137 of 400) with a company name are a better
  signal than the flag.
- An open checklist proves nothing either. Mod 180823 "Alien Zombie" allows *commercial* use, but the submitter isn't
  the creator and the credits say "Original Authors: Valve". **An uploader can't license what they don't own.**
- Every one of the 400 had a Credits module (`_aCredits`), and GameBanana's own form needs one. Only 48 descriptions
  mention credit at all, and 10 mention asking for permission.

Maps (100 CS 1.6 maps, 50 newest + 50 most downloaded): 85 have the default checklist. **62 credit someone besides
the uploader** in the Credits module, and roles name what was used ("textures" 14, "sprites", "models", "sounds",
"skybox texture"). Only 13 say it in the description.

### 1.4 What GoldSrc mappers usually do (custom, not rules)

- **Credit, rarely ask.** From the map sample: credits in the Credits module (62/100) are normal. Asking first is
  rare in practice, even though 89% of the assets say "ask". Enforcement is after the fact: a report under R.23
  (missing credit) or R.10/DMCA (the owner complains).
- **Readme.** Older map zips carry a `readme.txt` with "Credits / Thanks to". TWHL's mod packaging guide says to ship
  every file the .res lists except stock Half-Life ones, and to include a readme
  (https://twhl.info/wiki/page/Tutorial%3A_Setting_up_a_Mod%3A_Addendum/7595).
- **Authors set their own terms in the text.** A typical example from GameBanana's "Free Map Projects/Files" wiki
  (https://gamebanana.com/wikis/980): "Feel free to do anything you wish with these files, just remember that if you
  use them in a finished map to include 'logan dougall' into the credits otherwise … your submission will probably
  get deleted". Others say "feel free to use in projects or mods as long as I am made aware" (Mod 546193). The app
  can't read that text reliably. It should show it.
- **Rips are normal in CS 1.6 zombie mods** (CSO/Nexon, Killing Floor, L4D). On GameBanana they live under the
  porting whitelist. Nexon is grey area. Tripwire is confirmed only for non-profit use with credit.

### 1.5 What this means for a Loner-Strike map that ships a GameBanana model

- Publishing **on GameBanana** with the file inside needs line 5 ("use parts … on GameBanana") = yes. The default
  is "ask". Line 6 ("… on another site") covers our community index download, a mirror and FastDL. Under the
  default that's also "ask".
- In every case: credit every author in the Credits module (rule R.23) and in the readme (custom).
- Stock Valve files the player already has are never packaged. `doctor::is_stock` already leaves them out of the
  .res and the package, so they need no credit line. Valve does get credited when a *custom* file is derived from
  Valve content.

---

## 2. A tracked credits list (design)

### 2.1 What the app already knows (and what it doesn't)

| record | where | knows | doesn't know |
|---|---|---|---|
| Get more content (skins, player models, sounds, sprites, textures, sprays) | `<app dir>/content/record.json` via `content.rs` (key `Mod:<id>:<game tag>`), written in `gamebanana.rs::install_content` | page `url`, `name`, `author` (= submitter name), the `files` it put in the game (`{path,…}`); `Content::owners(game)` gives path → (key, name) | the credits list, the licence, ported, the submitter id; a file's hash |
| Map downloads | `<app dir>/installed_maps.json` (`gamebanana.rs::install_into`, `record_copy`) | page, name, author, `maps`, `files` (incl. the map's models/sounds), `shared` | the same gaps |
| Your own model files | `<app dir>/tools/imported_models.json` (assets.rs `imported_record`) | that it came from a file you picked | where it came from |
| Community creations | `communitylib::Installed` (`uid`, `owner_id`, page) and `Credit {uid, model, gb, name, author}` for remixes; `origins()` → mine / community / remix / file | creator and page of maps/modes/prefabs | the assets *inside* them |
| Prefabs | `PrefabMeta.origin` (`local` / `community` page + uploader / `file`) (P5) | where the prefab came from | the assets its entities name |
| Map editor assets | `tools/editor/assets.rs`: every model/sprite/sound/sky the map names, where each was found; *Copy into the game* / *next to the project* / *Find missing files* | the files and their locations | where each file came from. Copies change the path, so path matching breaks |
| Publish | `tools/community.rs::readiness` (`r.files` = custom files shipped), `build_package` (README.txt, `loner-strike.json` Item), `gamebanana_form` (text for the GB form) | which custom files ship | who made them |

### 2.2 Capture: a "rights" snapshot at download time

When `install_content` or `install_into` records an install, also store what the page said that day. The ProfilePage
JSON is already in hand (`details()` / `source()` fetch it):

```json
"rights": {
  "page": "https://gamebanana.com/mods/227562", "model": "Mod", "id": 227562,
  "submitter": {"id": 12345, "name": "DimeX"},
  "credits": [{"group": "Original Authors", "authors": [{"name": "DimeX", "role": "", "url": ""}]}],
  "ported": false, "createdBySubmitter": true,
  "checklist": {"install": "yes", "redistribute": "ask", "modify_gb": "ask", "modify_other": "ask",
                "parts_gb": "ask", "parts_other": "ask", "nsfw": "ask", "commercial": "no"},
  "licence": "This work is licensed under … CC BY-NC-ND 4.0 …",   // plain text, ≤ 1 KB
  "fetched": 1763050000
}
```

- Normalise the checklist by line *prefix*, so it works whatever the noun is (Mod, Sound, Spray). The scratch script
  used `startswith("Redistribute")`, `"Modify"` + `endswith("GameBanana")`, `"Use parts"` …, `"adult"`, `"commercial"`.
  An unknown line is kept as text and treated as "ask".
- Keep the snapshot: the terms you downloaded under are the ones that count, and pages change. Older records with no
  `rights` get them on first need: one ProfilePage call by `content::key_id(key)`, cached, through the 4 MB JSON cap.
- **File fingerprints**: add `sha256` to each installed file entry in both records. P5's editor copies files into
  `<game>_downloads` and next to the project, and map packages carry files at new paths. A hash index (sha256 →
  record key) finds the source wherever the file has gone. Computing the hashes is cheap at install, when the bytes
  are already being copied.

### 2.3 Resolve: where did each asset of my map come from?

New `tools/credits.rs`, pure functions plus one route:
`resolve(rel, bytes_hash) -> Source`, tried in this order:
1. **stock**: `doctor::is_stock` → no credit needed;
2. **GameBanana download**: path match via `Content::owners(game)` (and the map records' `files`), else the hash
   index → `{record key, rights}`;
3. **community creation**: a file that came in with an installed community map/prefab package. Its package
   manifest's own `credits` list (2.4) is carried along, so credits chain from creator to remixer;
4. **your own**: `imported_models.json`, or files under the project made by the app (the spray maker's WADs, your
   compiled maps);
5. **manual**: a credit the mapper typed (below);
6. **unknown**: shown as "Where's this from?" with an *Add credit* button.

The editor project keeps manual credits and permission notes beside the map, the same way P5 keeps `ls_modes`. It's
a list in the `.lsmap` document, not worldspawn (BSP entity text is precious):
`credits: [{files: ["models/x.mdl"], page?, authors: [{name, role}], permission: "granted" | "", note, date}]`.
`permission: "granted"` with a note ("asked on Discord, 2026-11-10") is how a mapper records an *ask* answered yes.

Route: `GET /api/tools/editor/credits` → `{assets: [{rel, kind, source, page, authors, checklist, verdict, why}],
summary}`. It reuses `assets.rs`'s named-file list, so the files are the same ones Playtest copies.

### 2.4 Carry it everywhere the map goes

- **Editor**: the Project panel's "Sounds, models and sky" rows get a *From* column (page link, authors) and a
  permission badge. **Map > Map properties > Credits** lists the auto credits plus your manual ones and free
  "thanks" text.
- **README.txt** (`community.rs::readme`): a `Credits` block, grouped by asset, each with its page URL, the authors by
  credits group, and "Used with permission (note)" where the mapper recorded one.
- **GameBanana form** (`gamebanana_form`): rule R.23 wants credits in the Credits module, so add
  `credits: [{group, authors: [{name, url}]}]` to the form JSON. The Publish page shows it as rows to copy into
  GameBanana's Credits module. The `text` gets a short "Uses assets by …" line too.
- **Community index** (`Item`): add `credits: Vec<AssetCredit>` (`{files, page, authors[], permission}`; ≤ 64
  entries, short strings; the list service checks sizes the way it does `modeRefs`). Browse cards show "Uses: Civilian
  Zombies by DimeX …". Installing a community map records these credits, so reusing its assets keeps the chain.
- **Prefabs**: a prefab's export carries the credits of the assets its entities name. A placed prefab adds them to
  the map, so P5's prefab origin plus this gives prefab → asset → author.

### 2.5 Warn, plainly, at the right moments

Verdict per asset, for the way it's about to be shared (Publish to GameBanana + our index = lines 5 and 6):

| verdict | when | what the mapper sees |
|---|---|---|
| ok | both lines `yes` (or `only_if_same` and the map is shared on the same terms) | credit line added automatically |
| ask | `ask`, missing checklist, unknown source, or `only_if_same` the map can't offer | warning in live checks (`asset-permission`) and Publish: "Civilian Zombies by DimeX says *ask first* before using it in a map you share. Ask them (link to their GameBanana profile), then mark it *Permission granted* with a note." |
| no | either line `no` | Publish **refuses** until the asset is removed or *Permission granted* is recorded with a note (a stated "no" needs more than a warning; still the mapper's call) |
| ported | `_bIsPorted`, or a credits group/role naming a company (Valve, Nexon, Tripwire…) | info, plus warning when the company is in GameBanana's "Definite No" list: "Ported from Killing Floor (Tripwire): allowed on GameBanana only non-profit with credit." |

- Show warnings first in Map Doctor and Publish (`readiness().warnings`). Live checks show them as information, not
  errors: a map that never leaves the computer needs no permission.
- Get more and the asset browser show the licence **before download** (2.6), so nobody learns at Publish time that
  their favourite model said "no".
- The rule stays honest: "ask" is GameBanana's default, so say "This page uses GameBanana's default terms: ask
  first", not "the author forbids".

### 2.6 Before download

`details()` adds `licence: {checklist, text, ported, credits}` to its answer. The Get more dialog (and the asset
browser's GameBanana tab) shows a small block: *Free to install* ✓ · *Use in a map you share*: Ask first / Yes / No
· *Ported from …*, plus the full credits. Browse lists can't show it (the Index API has no licence fields). The detail
view is the place.

---

## 3. Browsing models in the editor

### 3.1 What exists today

- **Entity form** (`editor/EntityForm.tsx`): `model`, `sprite`, `noise`/`message` and the other keys are **plain
  text fields**. The only datalists are entity names (target/killtarget/master). `AssetPreview` adds a **Play**
  button for `.wav`/`.mp3` (through `/api/tools/editor/sound`) and a **3D look** for `.mdl` (`ModelPreview` → the
  asset browser's model route). **Sprites get no preview**: `assetKind` knows sound and model only. There's no
  picker, so you have to know the path.
- **Viewport** (`EntityModels.tsx`): point entities with a model key, hostages, spawns and armoury weapons draw their
  real model. `env_sprite` draws as a glow billboard, not the sprite.
- **Creatures**: `ls_creature`'s Type shows the type's model with a preview. `ls_wave_spawner` has type toggles.
- **Tools > Textures & models** (`views/tools/assets/`, agent H): `Models.tsx` lists every model in cstrike,
  czero, valve and the `_downloads` folders (kinds player/view/carried/world/hostage/prop, search, `imported` flag),
  `ModelView` in 3D with skins, body parts and animations (`/api/tools/assets/model{,/info,/tex,/anim}`), plus
  *Get more* (GameBanana skins, player models and textures through `/api/gb/*`) and *Add your own*. There's no
  sprite or sound tab, and no SPR decoder in `goldsrc/` (only a magic-byte check in assets.rs/mapcopy.rs).
- **Placing**: drag and drop from the Library (`ComponentPanel`, `KitPanel` → `extras.ts::placeComponentAt`, ids
  `kit:<mode>:<id>`).

So **no, there's no way to browse models from the editor today**: only to preview one you typed.

### 3.2 Proposal: one asset picker, three entry points

**A. "Choose…" beside asset keys.** In EntityForm, a key with FGD type `studio` / `sprite` / `sound` (or a value
ending .mdl/.spr/.wav) gets a *Choose…* button. It opens the **Asset picker** dialog (its own lazy chunk) on the
right tab, filtered to what fits:
- **Models**: reuses `Models.tsx`'s list and `ModelView` (search, kind filter, skins/anims, size and polycount from
  `/model/info`). A *Source* filter: Game (stock) / Downloaded (Get more record) / Yours (imported) / This map.
- **Sprites** (new): needs `goldsrc/spr.rs` (SPR v2 header, palette, frames → RGBA → PNG, through the same caps as
  `decode_picture`) and `/api/tools/assets/sprites`, `/sprite?folder&path&frame`. It shows animated frames and
  render mode hints (additive/alphatest), which matter for `env_sprite`'s `rendermode`.
- **Sounds** (new): lists `sound/**/*.wav|mp3` in the game folders (the same depth and count caps as models) and
  plays them through the existing `/api/tools/editor/sound`. It shows length, rate and mono/stereo, and warns about
  what GoldSrc can't play (not 8/16-bit PCM, > 44.1 kHz).
- Every row shows the **source badge** and the **permission badge** from section 2 (stock / ok / ask / no).

**B. Library > Assets tab** (beside Prefabs, Components, Mode kits): the same picker docked as a panel. **Drag onto
the map** places the right entity through `placeComponentAt` with a new `asset:<kind>:<path>` id:
- `.mdl` → `cycler_sprite` with `model`, `framerate 1`, `renderamt`/`rendermode` defaults. It's the usual static
  prop in CS 1.6. CS has no `env_model`, and `cycler` takes damage and blocks. The sequence and skin come from the
  picker's choice.
- `.spr` → `env_sprite` with `model`, `rendermode 5` (additive), `renderamt 255`, `scale 1`, `framerate 10`, and
  spawnflag "Start on".
- `.wav` → `ambient_generic` with `message` (path without `sound/`), `health 10`, a radius flag from the picker
  (small/medium/large/everywhere) and "Start silent" off.
- Dropping an asset onto a *selected* entity sets its key instead of placing a new one.
- A precache note: each new model/sprite/sound counts toward the engine's 512, as the creature budget does.

**C. Get more from inside the editor.** The picker's last tab is GameBanana, through the existing `/api/gb/*` (same
checks, records, backups, caps). The detail view shows **credits and the permission block before the Download
button** (2.6). After install, the item is selected in the picker, so the next click places it, and its rights
snapshot is already recorded for the credits list. The current sections have no map-prop models outside "Skins /
Models > Map Props/Decals" (cat 8338). Add that to `content::SECTIONS` as a **Props** section, plus Half-Life
"NPCs & Enemies" (7739) for creature models (installed into the CS game's `models/`, as Get more does now).

Everything stays within the asset rules P5 already enforces (`clean_rel`, magic bytes, size caps, copies only into
the project or `<game>_downloads`).

---

## 4. Creature models for Last Stand

### 4.1 How the search was done

- Every Half-Life **NPCs & Enemies** item (cat 7739, 495 items) and the most downloaded hits for 24 keywords (zombie,
  monster, mutant, creature, alien, demon, ghoul, undead, infected, beast, crawler, spitter, brute, boss, headcrab,
  skeleton, golem, hound, spider, abomination, …) in CS 1.6 (game 4254) and Half-Life (game 34): 621 ProfilePages.
- Plus Sven Co-op NPCs (cat 7280) and Sven/HL player models for the same keywords: 83 more.
- Each page was checked against: the checklist, `_sLicense`, `_bIsPorted`, `_bCreatedBySubmitter`, the credits
  groups and roles, and the description, for game/company names (CSO/Nexon, Killing Floor, L4D, Black Mesa, HL2,
  Valve, Gearbox, …).

### 4.2 The honest result

**There are almost no zombie or monster models on GameBanana that are both original and clearly cleared for
redistribution.** What's there falls into three groups:
1. **Rips** from commercial games: CSO/Nexon zombies (most CS 1.6 zombie player models), Killing Floor (D-Wanderer's
   Stalker/Patriarch), Resident Evil, L4D, Black Mesa, HL2. They're out by the user's rule.
2. **Edits of Valve's Half-Life models** (almost every HL NPC entry: reskins, "HD" remakes, rigs on Valve skeletons
   with Valve animations). The uploader's licence covers only their own changes. The rest is Valve's, and under
   Valve's informal deal it can't be rehosted outside GameBanana (1.1). Legally that's no better than the stock
   `zombie.mdl` we already use, which players already have.
3. **Original meshes**: few, and nearly all on GameBanana's default terms (**ask** before redistributing).

One more catch: **every CS player model reuses Valve's player skeleton and animations** (that's how it plays as a CS
player). The creature framework can drive player models (Y.md: `anim_idle = ref_aim_knife_idle1` …). That is
tolerated everywhere and fine under the SDK licence *inside* the game, but the animation data is still Valve's.

### 4.3 Candidates (best first)

"Ask" = GameBanana's default checklist (redistribute / use parts on GameBanana / another site: all ask; commercial: no;
licence text CC BY-NC-ND 4.0) unless noted. Size: **player** = a CS/HL player model (fits the 32×72 hull, plays
through player sequences), **NPC** = a Half-Life monster model (its own sequences, played by activity).

| # | page | author(s) | permission (as listed) | original? | size / quality | fits |
|---|---|---|---|---|---|---|
| 1 | [Scary OP Zombie](https://gamebanana.com/mods/355790) (CS 1.6 player model, 2022) | Ghost1726877 ("Owner", role Modeler) | ask | original mesh, claimed by the author; CS player animations (Valve) | player; v1 3k polys, v2 2k polys, own hands + zombie bomb | zombie, runner |
| 2 | [Civilian Zombies](https://gamebanana.com/mods/227562) (CS 1.6, 2014) | DimeX | ask | **edit of CS `vip.mdl`** (Valve) + new animations by the author; 7 body variants | player; one model, 7 submodels (variety for the horde) | zombie |
| 3 | [HD Houndeye](https://gamebanana.com/mods/180986) (HL, 2010) | Sproily (model & texture) | ask | new four-legged model by the author (sequences likely follow Valve's houndeye) | NPC, small; HD textures | runner (fast low creature) |
| 4 | [Sproily's Controller](https://gamebanana.com/mods/180974) (HL, 2008) | Sproily (model, skin), DiamonD (rig, compile) | ask | new mesh (replaces the alien controller) | NPC, flying | spitter (ranged) |
| 5 | [Green Pre-Alpha Houndeye 1997](https://gamebanana.com/mods/526333) (HL, 2024) | serecky | ask | the author's own recreation of a Valve **concept design**; **sources included** | NPC, small | runner |
| 6 | [Unidentified Creature Over Alien Slave](https://gamebanana.com/mods/583479) (HL, 2025) | GhostSynth (Modeler) | redistribute **yes**, parts on GB/other **yes**; NSFW and commercial no; licence "Use with credits added and non-commercial/non-profit." | the author's model of a Valve concept drawing (Chuck Jones, "Quiver" era); replaces the vortigaunt (Valve skeleton/anims likely) | NPC, player height | runner / spitter |
| 7 | [Maestra Fénix and LeonelC's Alien Controller](https://gamebanana.com/mods/180971) (HL, 2018) | Maestra_Fenix, LeonelC | all yes incl. commercial; **CC BY 4.0** | **derived** from Valve's controller + Gearbox PS2 arms (credited); their CC BY covers their changes only | NPC, flying; raised polys, new UVs | spitter |
| 8 | [Female Alien Grunt](https://gamebanana.com/mods/569553) (HL, 2025) | Goob, Ghost2189959 | modify and parts on GB/other **yes**; redistribute ask | edit of Valve's alien grunt | NPC, large | brute (**but** sexualised design: not for the default roster) |
| 9 | [Half Life Zombie For CS](https://gamebanana.com/mods/442824) (CS 1.6, 2023) | MG_Pro | ask | Valve's HL zombie rigged as a CS player model | player | zombie (no gain over stock `zombie.mdl`, which already plays) |
| 10 | [Heehee-lanth](https://gamebanana.com/mods/408181) (HL, 2022) | kimilil (retexture) | all yes except commercial; licence "Models and original textures are (c) Valve" | Valve's Nihilanth, retextured | NPC, huge | boss (joke skin: no) |
| 11 | [Uboa Zombie Playermodel](https://gamebanana.com/mods/420415) (Sven Co-op) | enuze (creator) | licence "Use it however you like."; checklist default ask | fan model of a *Yume Nikki* character (third-party IP) | player | runner (odd look; IP risk) |
| 12 | [Zombie Assassin](https://gamebanana.com/mods/654600) (HL, 2026) | zorrouwu | everything yes | **not the uploader's**: "the model used in Zombie Edition and Residual Life" (other mods' work) | NPC | ✗ example of an open checklist you can't rely on |
| 13 | [Alien Zombie](https://gamebanana.com/mods/180823) (HL, 2013) | Pyromancer12 (not the creator) | everything yes incl. commercial | credits "Original Authors: Valve" | NPC | ✗ same: the uploader can't grant this |
| 14 | [KF Stalker](https://gamebanana.com/mods/227556) / [KF Patriarch](https://gamebanana.com/mods/227555) (CS 1.6) | D-Wanderer | parts on GB/other **no** | **ported from Killing Floor** (Tripwire) | player | ✗ rip; listed because it's popular for zombie mods |

Nothing fits **brute** or **boss** cleanly. The original candidates are small aliens or zombies. Large originals are
all edits of Valve's garg/agrunt or rips.

### 4.4 What to do with them

- Treat **1 (Scary OP Zombie)**, **3–4 (Sproily)** and **5 (serecky, with sources)** as "ask the author" candidates:
  a GameBanana PM asking for "redistribute on other sites" and "use in another mod on another site" for Last Stand,
  credited, non-commercial. A written yes, stored as the permission note from 2.2, makes them shippable.
  *Inference:* the authors of 3–4 (2008–2010) may not answer.
- **6** is the only original-*looking* entry with redistribution already "yes", but its design is a Valve concept on
  (very likely) a Valve skeleton. Fine to *offer* through the opt-in look; not as our own content.
- Player-sized models (1, 2, 9, 11) work through the Y.md player-sequence path today. The `anim_speeds` key Y noted
  would stop the foot sliding. NPC-sized ones (3–8) play by activity like `houndeye.mdl`. Check the hull (≤ 32 wide,
  CREATURES.md §2) and the HL25 sequence-order issue (CREATURES.md §7c) before trusting a sequence number.
- **Better route for the default roster: original models made for us, or CC0 sources compiled to MDL.** Quaternius
  lists *Ultimate Monsters* (about 50 animated monsters, CC0 per its Sketchfab listing) and *LowPoly Animated
  Monsters* (CC0 per itch.io). The *Zombie Apocalypse Kit* says "free to use in personal and commercial projects":
  check its licence file. Links: https://sketchfab.com/3d-models/ultimate-monsters-pack-fd72e114d119488da71fe3a16f216c4f,
  https://quaternius.itch.io/lowpoly-animated-monsters, https://quaternius.com/packs/zombieapocalypsekit.html.
  Turning them into GoldSrc models is real work: ≤ 128 bones, 8-bit 256-colour BMP textures, our own idle/walk/run/
  attack/flinch/die sequences with linear movement, compiled with studiomdl. The result is ours to ship on FastDL.

---

## 5. Recommendation

1. **Capture rights at download now** (2.2). The app already fetches the ProfilePage. Storing `credits`, the
   normalised checklist, the licence text, `ported`, the submitter id and file hashes in the two install records is
   small, and everything else builds on it.
2. **Credits on Publish next** (2.3–2.4): resolve each shipped custom file to its source, then write the README
   Credits block, the GameBanana Credits-module rows (rule R.23) and `Item.credits`. Warn on "ask", refuse on "no"
   unless the mapper records permission (2.5). Word it truthfully: "ask" is usually GameBanana's default, not the
   author's choice.
3. **Asset picker in the editor** (3.2): *Choose…* on model/sprite/sound keys and a Library > Assets tab with
   drag-to-place (`cycler_sprite` / `env_sprite` / `ambient_generic`), reusing the asset browser's models list and
   viewer. Add an SPR decoder, a sounds list and a Props section in Get more. Show licence and credits *before*
   download.
4. **Last Stand creatures:** keep Half-Life's own models as the default (players own them; nothing is rehosted).
   Don't ship anything from GameBanana in the default roster. Ask Ghost1726877 (Scary OP Zombie) and serecky
   (Green-Eye houndeye, sources included) for written permission. Those, and the Base Builder look, go in as an
   **opt-in** look. For a shippable 1.0.0 roster of our own, build the five creatures from CC0 sources (verify each
   licence file) or commission them.

Scratch data (not committed): `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/gb/`
(rows.json = the 400-item sample, creat.json + sven.json = creature candidates).
