# Community creations: browse, try, publish

Loner-Strike's promise is build, play locally with human-like bots, host, share. This is the
"share" part: a **Community** browser for maps, game modes, prefabs, sprays and textures made
with Loner-Strike, where one click installs a creation and starts a game with bots, and a
**Publish** flow that turns a map you made into a ready-to-upload package.

Code: `app/src-tauri/src/tools/community.rs` (routes under `/api/tools/community/*`, app window
only) and `app/ui/src/views/tools/community/` (Tools > Share > Community, Beta).

## 1. Where things live

| What | Where | Why |
|---|---|---|
| The files | **GameBanana** | The app already browses and installs from it safely (gamebanana.rs, content.rs). Creators already have accounts there, it scans uploads for viruses, hosts pictures, comments and likes, and pays nothing to us in bandwidth. |
| Which pages are Loner-Strike creations | A **tag** on the GameBanana page | GameBanana's search can find pages by their attributes. |
| What GameBanana can't say | The **Loner-Strike index**, `https://dl.lonerstrike.com/community/index.json` | Modes a map supports, ships a nav and finished map knowledge, map simulator (health check) results, game mode plugins and their review, pulled pages, featured picks. A static file on the same download server as the nav pack. |
| Registrations and reports from the app | `https://list.lonerstrike.com/community/submit` and `/community/report` (section 7, docs/COMMUNITY-SERVER.md) | In the server list service on the Hetzner box. When it doesn't answer, registrations wait in the app. |

### GameBanana's API (what we use and what it can't do)

- Public read API, `https://gamebanana.com/apiv11` (the one the website itself uses; no key).
  - `Mod/Index` filters: `Generic_Category`, `Generic_Game`, `Generic_Name`, `Generic_Submitter`,
    `Generic_Studio`. There is **no tag filter** (`Generic_Tag` is "not recognized").
  - `Util/Search/Results?_idGameRow=4254&_csvFields=attribs&_sSearchString=<word>&_sOrder=date`
    searches attributes (tags) across item types (Mods, Sprays, WiPs), 15 a page. `_sOrder` takes
    `best_match`, `date`, `udate`, `popularity`.
  - The search splits on hyphens and ORs the words: `Loner-Strike` matches every page with
    "Strike" in a tag (680 pages). Hence the one-word tag below.
  - `<Model>/<id>/ProfilePage`: tags as `{_sTitle, _sValue}`, files with `_sMd5Checksum`,
    `_sAvResult` (virus scan), `_nFilesize`, the game, category, submitter, studios.
  - List records carry tags as `"Title: Value"` strings.
- CS 1.6 is game 4254. The categories we use: Maps 5474 (and its subcategories), Prefabs 451,
  Textures 681, Resources > AMXX Plugins & Mods 3521 (game modes), Resources > Navigation Files
  3526, Resources > Overviews 3527. Sprays are their own item type (`Spray`), with ids separate
  from Mods.
- **No public upload or submission API.** Uploading needs a signed-in browser session, and the
  write endpoints are undocumented. The app never signs in to GameBanana and never uses
  undocumented write calls. Publishing is "the app prepares, you upload on the site, the app
  checks the result".

## 2. GameBanana conventions

1. **The tag.** On the page's Attributes, add `Software Used: LonerStrike`. "Software Used" is
   GameBanana's own attribute group (mappers already list Hammer or Trenchbroom there), so it
   reads naturally. The value is one word because of the hyphen splitting above. When reading
   pages the app also accepts `Loner-Strike`, but only the one-word tag is found by the search.
   *Check with the first real upload that the search finds it (unverified: no page carries it
   yet).*
2. **The game and category.** Counter-Strike 1.6 (Condition Zero maps go there too), in the
   category for the kind (maps by type: Bomb/Defuse, Surf, ...; prefabs; textures; AMXX
   Plugins & Mods for game modes; sprays as Sprays).
3. **The file.** The package Publish made, uploaded unchanged. The app matches it by MD5, which
   GameBanana lists for each file. Layout: game paths from the archive root (`maps/x.bsp`,
   `maps/x.nav`, `maps/x.lsv`, `maps/x.res`, `overviews/x.bmp`, `mytex.wad`, `gfx/env/...`,
   `models/...`, `sound/...`), plus `README.txt` and `loner-strike.json` (the index entry to be,
   section 3). That's GameBanana's usual layout, so people without the app can unzip it into
   cstrike/.
4. **Pictures.** Screenshots go in GameBanana's picture upload (Publish saves them beside the
   package). The index points at GameBanana's image host, so we host no pictures.
5. **Later: a Loner-Strike studio.** A GameBanana Studio that creators can join. `Generic_Studio`
   is a real filter, so a studio would give a second, curated way to list pages. Not needed for
   the first version.

## 3. The index (`community/index.json`)

JSON, UTF-8, at most 8 MB, at most 5000 items. Fields are camelCase. A reader ignores fields it
doesn't know, so new fields never break older apps. `schema` is the major version: an app that
reads 1 refuses 2 and says "update Loner-Strike".

```json
{
  "schema": 1,
  "updated": 1794000000,
  "revoked": ["mod:900", "spray:45"],
  "verifiedAuthors": ["SomeModder"],
  "items": [
    {
      "kind": "map",
      "model": "Mod", "gb": 739741, "file": 1879994, "md5": "50f71f9d253d5a4021f178ebbc6b1323",
      "name": "de_lonely", "author": "jam", "summary": "A quiet bomb map with two long sightlines.",
      "games": ["cstrike"],
      "map": {
        "name": "de_lonely", "bspSize": 2394112, "bspSha256": "…64 hex…",
        "modes": ["classic", "csdm"],
        "nav": true, "places": true, "lsv": true, "res": true, "overview": true
      },
      "health": { "verdict": "good", "when": 1793990000, "mode": "classic", "minutes": 5, "bots": 12,
                  "stuckPct": 0.8, "falls": 0, "killsPerBotMin": 1.2, "coveragePct": 61.0 },
      "screenshots": ["https://images.gamebanana.com/img/ss/mods/6af63370ed972.jpg"],
      "tags": ["night", "small"],
      "featured": false,
      "added": 1793990000, "updated": 1793990000,
      "status": "listed"
    },
    {
      "kind": "mode", "model": "Mod", "gb": 740001, "name": "Jetpacks (a Game Mode Studio mode)",
      "mode": {
        "id": "jet",
        "plugins": [],
        "review": "none",
        "maps": ["de_lonely"]
      }
    }
  ]
}
```

| Field | Meaning |
|---|---|
| `kind` | `map`, `mode`, `prefab`, `spray`, `texture`. Unknown kinds are skipped. |
| `model`, `gb` | The GameBanana page (`Mod` default, or `Spray`) and its id. `model`+`gb` is the key. |
| `file`, `md5` | The file on the page this entry describes, and its MD5 as GameBanana lists it. With `file`, Try it installs that file (if it's still clean); else the newest clean one. |
| `map` | For maps: the exact .bsp version (`bspSize`, `bspSha256`) the nav, knowledge and checks were made for, the modes it's made for, and what it ships. |
| `mode` | For game modes: its mode id, its plugins (file, sha256 of the .amxx, where the source is), the review state (`none`, `verified`, `reviewed`), maps it's made for. For 1.0 `plugins` is always empty and `review` always `none` (only Game Mode Studio modes are shared); the fields stay for a later version. |
| `health` | The newest Loner-Strike map simulation (the map simulator, tools/health.rs) of that map version. Not filled by the list service for now: an author's numbers can't be proven (see section 7, health from our side). |
| `screenshots` | https links on `images.gamebanana.com` or `dl.lonerstrike.com` only. |
| `featured` | A team pick; listed first. |
| `status` | `listed`; `hidden` (kept out of lists, still openable by link); `revoked` (never offered or installed). |
| `revoked` (top) | GameBanana pages pulled even though they carry the tag (the page may not be in `items`). |
| `verifiedAuthors` | GameBanana names whose modes may be marked `verified`. Informational for the app; the index's `review` field is what counts. |

What the app does with a bad entry: drops it, never the whole list. An unknown `status` reads
as `hidden` and an unknown `review` as `none`, so a newer index can add values without older
apps dropping the list (other fields must keep their types: a string where a number belongs
still fails the whole file). Dropped: non-positive or
huge ids, duplicate keys, unknown kinds, map names that aren't plain (`maps::safe_arg`), mode
ids that aren't letters/digits/_, entries without a name. Cleaned: text to one line and a
length cap, MD5 to 32 hex or nothing, screenshots to allowed https hosts, games to cstrike/czero.

**When the index can't be reached:** the last copy that parsed is kept at
`<app dir>/tools/community/index.json` and used (the page says "from earlier"). With no copy,
the list is GameBanana's tag search alone: no badges, modes can't be installed, content still
can. A download that doesn't parse (an HTML error page) never replaces the kept copy. The
index is cached 15 minutes in memory, GameBanana's search 10 minutes; Refresh skips both.

The list merges both sources: every tagged GameBanana page (with the index's extras when it's
listed), plus index entries the search didn't return (GameBanana slow to index, or offline).
Featured first, then listed, then tag-only pages; newest, most liked or by name within each.

## 4. Trust and moderation

**For 1.0 the community shares content and Game Mode Studio modes only**: maps, prefabs,
textures, sprays, component setups and Studio graphs, all plain data. Game Mode Studio builds a
mode's code on each player's computer, so nothing that runs code is ever downloaded. Plugin
modes (hand-written or compiled AMX Mod X plugins) aren't shared at all; a later version may
add them behind an automatic native-allowlist check.

The rules are code (`community::trust`) with tests; the page shows the result as a label and a
sentence.

| What | Level | Installs from the app |
|---|---|---|
| Pulled (`revoked`, top-level or per entry) | blocked | Never; not listed. |
| Maps, prefabs, sprays, textures | content | Yes, one click, whoever made them. The installer copies only content (gamebanana.rs/archive.rs allow-lists: maps, wads, models, sounds, sprites, gfx, overviews) and never replaces files you have. |
| A game mode with no plugins (a Game Mode Studio mode: settings, graphs, components) | data | Yes, one click. |
| A mode with plugins, whatever its `review`, or a tagged mode the index doesn't know | unreviewed | No. The page says plugin modes can't be installed from the community yet and links to GameBanana. |

**What can't be published.** Publish (`build_package`), registering (`check_page`) and the list
service all refuse the same things, with the same list kept in both places
(`community.rs` `CODE_EXT`/`ARCHIVE_EXT`, `services/serverlist/community.py`):
a mode with plugins ("Plugin modes can't be shared yet. Make it in Game Mode Studio, which
builds a mode's code on each player's computer.", and the page offers "Open in Game Mode
Studio"), and any file that runs code or hides other files: `.amxx .amx .sma .inc .dll .so
.dylib .exe .com .scr .msi .bat .cmd .sh .ps1 .py .pyc .js .vbs .jar .lua .pl .rb .php .app
.command`, archives inside the package (`.zip .rar .7z .tar .gz .tgz .bz2 .xz .cab`), anything
under `addons/metamod/` or a `dlls/` folder. The list service reads the uploaded zip's file list
itself (range requests to GameBanana's file hosts), so a hand-made zip is checked too.

When the mode installer is built, it must also refuse plugins (until a later version allows a
checked kind), install into the mode's own files, never `plugins.ini`, and refuse Metamod DLLs
and anything outside the mode's content and settings.

**Moderation** (docs/COMMUNITY-SERVER.md has the commands and setup):
- A registration that passes the list service's automatic checks (page in CS 1.6, the tag, the
  kind, the file's MD5, GameBanana's virus check, nothing that runs code in the package) is
  **listed at once**, sprays included. The team's private Discord channel gets a card for each
  new listing with a Pull button.
- **Report** (every creation's page): a reason (doesn't work, harmful, someone else's work,
  offensive, not a Loner-Strike creation, something else) and an optional note (500
  characters), sent through `POST /api/tools/community/report` to the list service, which
  rate-limits it and posts a card to the moderators' channel with Pull, Keep and Block creator.
  Nothing about the player is sent; the service tells reporters apart only by a salted hash.
- **Block creator** sends that GameBanana member's future registrations to a manual queue
  (Approve / Reject cards) instead of listing them at once.
- A pull adds the page to `revoked` (and marks its entry `revoked`); it reaches running apps
  when the index is next published (within 15 minutes of that). Installed copies stay, as with
  any GameBanana download, and can be removed in Get more.
- Also: GameBanana's own report button, and the Loner-Strike Discord. Featured picks are a
  field, not an algorithm.

### Index integrity

The list service publishes `community/index.json.sha256` beside the index, and the app refuses a
download that doesn't match it (`community::checked_index`; the copy from before is used, as when
the index can't be reached). That catches a damaged or half-finished upload. A checksum from the
same server can't catch a tampered one; a signature the app verifies (a key pinned in the app)
would. Request in section 8.

## 5. Try it

1. The page asks `/api/tools/community/try {model, id, confirmed}`. The server re-reads the
   GameBanana page (tag still there?), refuses a mode with plugins outright, applies the trust
   rules (and the confirm, unused while only content and data modes install), picks the file
   (asked, else the index's, else the newest clean supported one; GameBanana-flagged files
   never) and starts the existing installer job (`MapStore::start_section_install`).
2. The page follows the job on `/api/gb/jobs` (the same list Get more uses).
3. Done: **Play with bots** starts the game on that map (`/api/app/launch {map}`), and one
   button per game mode the map is made for that you have (`/api/app/launch {map, mode}`).
   Modes it's made for that you don't have are named, pointing at Mods.

## 6. Publish

1. **Pick a map** (not one of the game's own).
2. **Check** (`GET /api/tools/community/publish/:map`): the map doctor's `diagnose` (damaged
   file, missing textures and files, nav and knowledge fit) and the newest map simulation report
   for that map. Blockers stop it (a stock map, a damaged file); warnings are shown (missing
   files, no nav, a bad simulation); tips nudge (name places, overview, run the map simulator).
3. **Fill in** name, author (remembered), a sentence or two, the modes it's made for (the
   map's preset mode and the simulation's mode pre-ticked) and up to 6 screenshots (PNG/JPEG,
   8 MB each, decoded on the server to be sure).
4. **Make the package** (`POST .../publish/:map`): `<app dir>/tools/community/packages/<map>-loner-strike.zip`
   with the map, its nav and finished knowledge (only when they fit), a fresh `.res`, the
   overview, every custom file it uses where the game looks (no Valve or stock files, nothing
   from outside the game folder, at most 1 GB), `README.txt` and `loner-strike.json`.
   Screenshots go in `<map>-loner-strike-screenshots/`. A `<map>-loner-strike.json` beside the
   zip keeps the details for registering.
5. **Upload on GameBanana**: the page shows the folder, opens the submit page, and has copy
   buttons for the name, the description (with the bot and health facts written out), the
   attribute and the category to pick.
6. **Paste the link** (`POST /api/tools/community/register {url}`): the app reads the page and
   checks the game, the tag, that one of its files has the package's MD5 and that the package
   carries nothing that runs code, then builds the index entry (page id, file id, MD5, the
   GameBanana author name, its pictures) and sends it to the list service, which checks it all
   again itself. Its answer, kept in `<app dir>/tools/community/mine.json` and shown in Yours:
   - 200 `{"ok": true, "key": "mod:123", "state": "listed"}`: listed (shows up once the index is
     next published), or `"state": "pending"` when the creator is blocked and a moderator looks
     first;
   - 4xx (not 429) `{"error": "..."}`: **refused**, final, with what to fix (409 pulled or turned
     down, 422 a failed check); not sent again;
   - 429, 5xx or no answer: **waiting**, retried each time Yours opens; the page also offers the
     entry as text to post in the Discord.
   A mode with plugins is refused before anything is sent, with "Open in Game Mode Studio".

Other tools open Publish with a map: `go("tools", "community:publish:<map>")`. Other addresses:
`community:yours`, `community:mod:<id>`, `community:spray:<id>`.

## 7. The server side (list.lonerstrike.com / dl.lonerstrike.com)

Built in `services/serverlist/` (community.py inside the server list service); how it works,
its settings and the deploy steps are in **docs/COMMUNITY-SERVER.md**. What the app talks to:

| Endpoint | Body / answer |
|---|---|
| `POST https://list.lonerstrike.com/community/submit` | `{"schema": 1, "app": "<version>", "page": {"model": "Mod" \| "Spray", "id", "file"}}` — only which page and file; the list takes the title, author and pictures from GameBanana and everything Loner-Strike from the package's own `loner-strike.json` and file list (nav/knowledge/overview only when they ship; health numbers aren't listed, as nothing proves them) → 200 `{"ok", "key", "state": "listed" \| "pending"}`, or 4xx/5xx `{"error"}` (section 6) |
| `GET https://list.lonerstrike.com/community/submission/<mod\|spray>/<id>` | `{"key", "state": "listed" \| "pending" \| "hidden" \| "rejected" \| "pulled" \| "unknown", "reason"?}` |
| `POST https://list.lonerstrike.com/community/report` | `{"schema": 1, "app", "model": "Mod" \| "Spray", "id", "reason", "note"}` → 200 `{"ok": true}`, 400 bad, 413 too long, 429 too many, 503 busy, each with `{"error"}` |
| `GET https://dl.lonerstrike.com/community/index.json` (+ `.sha256`) | the index (section 3), published from the list service's database by the team |

Later, optional: run the map simulator on a submitted map on a team machine to fill `health`
independently of the author's numbers, and render viewer screenshots to
`dl.lonerstrike.com/community/shots/` (already an allowed screenshot host).

## 8. Requests to other tools (also in TOOLS-NOTES.md)

- **App / list service: a signed index.** Pin a public key in the app and have it verify a
  signature published beside `community/index.json` (Ed25519 or minisign) before using a
  download; the app already checks `index.json.sha256`, which only catches damage. Until then a tampered index on
  the download server would be believed (the trust rules still stop plugins and the installer's
  allow-lists still apply).

- **Installer (archive.rs, owner of gamebanana/content):** allow `.lsv` in `maps/` (the bot
  brain's map knowledge, plain data). Today a package's `.lsv` is skipped on install, so bots
  rebuild knowledge on first play instead of having it ready.
- **Map editor / nav fixer:** a "Publish…" action that does `go("tools", "community:publish:<map>")`
  after a successful compile or save.
- **Map viewer:** "Use as a screenshot for Publish" (the viewer's `shot()` exists for dev); could
  hand a PNG to the Publish form.
- **Spray maker:** "Share on GameBanana" for sprays: the spray image + the tag; register the same way.
- **Game Mode Studio:** a mode package format for Studio modes (settings, graphs, components; no
  compiled plugins or sources: the app builds the code on the player's computer) and its
  installer following section 4; Publish then gets a "game mode" path, run through the same
  `package_refusal` check.

## 9. Prefab packages (map editor, agent P4)

A prefab package is a zip made by the editor's Library > Prefabs > Share:
`prefabs/<id>.map` (a plain Valve 220 .map: any GoldSrc editor can paste it), `prefabs/<id>.json`
(`format`, `name`, `tags`, `author`, `description`, `size`, `brushes`, `entities`, `textures`, `wads`,
`materials` with texture sizes), `prefabs/<id>.png` (thumbnail), optional `*.wad`, `README.txt` and
`loner-strike.json` (`{"schema": 1, "item": {"kind": "prefab", ...}}`). Upload it to GameBanana's
Prefabs category with the tag, like any creation.

**Try it** (`community::try_prefab`, no installer job): downloads the file (GameBanana hosts only,
MD5 checked against the page, 128 MB cap), unpacks it, and `tools::editor::prefabs::install_download`
adds every `.map` it finds to the player's library (`<app dir>/editor/prefabs/`, the `.json`/`.png`
beside it when there; a bare GameBanana `.map` or a zip of them works too, centered on install).
WADs in the package go into the game folder only when no file of that name is there. The page then
offers "Open the map editor" (`go("tools", "editor:prefabs")`).

## 10. The community library: installed creations in their usual places

Code: `app/src-tauri/src/tools/communitylib.rs`; UI `views/tools/community/{Installed,chip,Hooks,ServerContent}.tsx`.

**What's kept.** Every Try it is noted in `<app dir>/community/<mod|spray>-<id>/meta.json`: kind,
page, file and MD5 installed, name, author (GameBanana's), the game folder, and where it went
(map names, prefab ids, a Studio mode's id). A Studio mode's graph is kept beside it. The files
themselves stay where they always go, installed by the installer that owns them:

| Kind | Installed by | Shows up in |
|---|---|---|
| map | gamebanana.rs job (`maps` section; noted when the job ends) | Maps page and Play map lists (Community tag on the card, "From Community" in Show, a chip with a link on the map's sheet) |
| Game Mode Studio mode | downloaded zip (`modes/<id>/<id>.graph.json`, checked: one graph, the Studio's checks, nothing that runs code), built here with the Studio's own compiler (`studio::compile`), refused when the id clashes with a mode you have, your own Studio graph or another community mode | Play mode cards (Community line), Mods page "Modes from Community" (Details / Remove), Game Mode Menu |
| prefab | `editor::prefabs::install_download` | the editor's prefab library (chip on the row) |
| spray, textures | gamebanana.rs/content.rs job (`sprays`/`textures` sections, replaced files backed up) | the spray maker's "Sprays from Community" |

Component setups have no community kind yet; when the editor's component packs become shareable
they get a kind and a row here like prefabs.

**Routes** (app window only; MCP: library and remix are Read/Change, the rest asked first):
`GET /api/tools/community/library` (items with `update` and `pulled`, plus lookups by map name,
mode id, prefab id and page for the chips), `POST .../uninstall {key}`, `.../update {key}`,
`.../remix {key}`, `GET|POST .../server/:id`, `GET|POST .../publish-mode/:id`.

**Removing** goes back through the installer that put it in (its records say which files are
its; replaced files come back), and the safety net (History & restore) keeps a copy of each file
first. A Studio mode is taken out of modes.ini with its plugin and files (`studio::uninstall`,
only modes the Studio made). Servers carrying it drop it too.

**Updates.** A newer file is "available" when the index lists a newer file id for the page (or,
for a page the index doesn't list, when the page itself has a newer clean file). Update removes
the installed version first (the installer never writes over existing files), then installs the
new one through exactly the same checks as Try it. A page the team pulled since shows a note.

**Remix** (Studio modes): copies the community graph into your Game Mode Studio graphs as
`<id>_remix` (`_remix2`, … never over your copies or an installed mode), named "<name> remix",
and notes the credit in `<app dir>/community/remixes.json` (your graph id → original page, name
and author). It opens in the Studio (`studio:<id>`). Publishing it (Publish > A game mode) puts
`remixOf` in the index entry and "A remix of X by Y" in the package's README and the GameBanana
description. The credit lives beside the graph rather than in it, so the Studio's graph format
didn't change.

**Publishing a Studio mode** (Publish > A game mode): the graph must pass the Studio's checks;
the package is `modes/<id>/<id>.graph.json`, README.txt and loner-strike.json (kind `mode`,
`plugins: []`, maps it's made for, `remixOf`), uploaded to Resources > AMXX Plugins & Mods with
the tag. Registering works as for maps. Other tools can open it with
`go("tools", "community:publish:mode:<id>")`.

**Servers** (a server's Library > Community content tab, this computer only): tick the community
maps and Studio modes a server carries; the choice is kept in `<app dir>/community/servers/<id>.json`.
- Maps are copied with the app's map copy job (`mapcopy::start`: the map and its files, recorded in
  the server's map records under the GameBanana id; Linux servers are uploaded with the usual SSH
  sync). FastDL serves the server's game folder and lists what its records hold, so players
  download them with nothing more to do.
- Modes are built on this computer first (the Studio's compiler, so a server only ever gets what
  the graph makes), then `studio_<id>.amxx`, `<id>.plugins.ini`/`.cfg` and the modes.ini line go
  into the server's folder (`studio::register_mode`), then a Linux server is synced.
- Unticking takes them off: maps through the server's map records (FastDL stops offering them),
  modes with `studio::uninstall` on the server's folder, then a sync.
