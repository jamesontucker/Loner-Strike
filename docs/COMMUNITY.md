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
| What GameBanana can't say | The **Loner-Strike index**, `https://dl.lonerstrike.com/community/index.json` | Modes a map supports, ships a nav and finished map knowledge, health check results, game mode plugins and their review, pulled pages, featured picks. A static file on the same download server as the nav pack. |
| Registrations from the app | `https://list.lonerstrike.com/community/submit` (not built yet, see section 6) | Next to the server list service. Until it exists, registrations wait in the app. |

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
      "kind": "mode", "model": "Mod", "gb": 740001, "name": "Jetpacks",
      "mode": {
        "id": "jet",
        "plugins": [ { "file": "gm_jet.amxx", "sha256": "…64 hex…", "source": "scripting/gm_jet.sma" } ],
        "review": "reviewed",
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
| `mode` | For game modes: its mode id, its plugins (file, sha256 of the .amxx, where the source is), the review state (`none`, `verified`, `reviewed`), maps it's made for. No plugins = a data-only mode. |
| `health` | The newest Loner-Strike health check (tools/health.rs) of that map version. |
| `screenshots` | https links on `images.gamebanana.com` or `dl.lonerstrike.com` only. |
| `featured` | A team pick; listed first. |
| `status` | `listed`; `hidden` (kept out of lists, still openable by link); `revoked` (never offered or installed). |
| `revoked` (top) | GameBanana pages pulled even though they carry the tag (the page may not be in `items`). |
| `verifiedAuthors` | GameBanana names whose modes may be marked `verified`. Informational for the app; the index's `review` field is what counts. |

What the app does with a bad entry: drops it, never the whole list. Dropped: non-positive or
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

The rules are code (`community::trust`) with tests; the page shows the result as a label, a
sentence and, for plugins, a question before installing.

| What | Level | Installs from the app |
|---|---|---|
| Pulled (`revoked`, top-level or per entry) | blocked | Never; not listed. |
| Maps, prefabs, sprays, textures | content | Yes, one click, whoever made them. The installer copies only content (gamebanana.rs/archive.rs allow-lists: maps, wads, models, sounds, sprites, gfx, overviews) and never replaces files you have. |
| A game mode with no plugins (settings, graphs, components) | data | Yes, one click. |
| A mode with plugins, every plugin with a sha256 and source, `review: reviewed` | reviewed | Yes, after a confirm naming the plugins ("The Loner-Strike team read their source"). |
| Same, `review: verified` (known author, source shipped, not read) | verified | Yes, after a stronger warning ("Plugins can do anything a server can. Only install it if you trust its author"). |
| A mode with plugins lacking source or a full hash, `review: none`, or a tagged mode the index doesn't know | unreviewed | No. The page explains why and links to GameBanana. |

Today Try it works for maps, sprays and textures (the installer sections that exist). Modes wait
for the Game Mode Studio's installer and prefabs for the editor's prefab library; their pages
say so. The metadata and the UI for plugin trust are in place for when they land.

When the mode installer is built, it must also:
- check each plugin's sha256 against the index before copying, and refuse a mismatch;
- for `reviewed`, prefer building from the shipped source with the bundled amxxpc and compare;
- install into the mode's own plugin list (`<id>.plugins.ini`), never `plugins.ini`, so removing
  the mode removes its plugins;
- refuse Metamod DLLs and anything outside `addons/amxmodx/{plugins,configs,data}` and the
  mode's content.

**Moderation** (team side, no in-app reporting beyond a link):
- New registrations land in an inbox; a person lists them (sets `status: listed`), possibly after
  a look. Content can be listed fast; modes with plugins need a review first.
- Pull a page by adding `mod:<id>` to `revoked` (takes effect within 15 minutes for running apps;
  installed copies stay, as with any GameBanana download, and can be removed in Get more).
- Reports: GameBanana's own report button, and the Loner-Strike Discord.
- Featured picks are a field, not an algorithm.

## 5. Try it

1. The page asks `/api/tools/community/try {model, id, confirmed}`. The server re-reads the
   GameBanana page (tag still there?), applies the trust rules (and the confirm), picks the file
   (asked, else the index's, else the newest clean supported one; GameBanana-flagged files
   never) and starts the existing installer job (`MapStore::start_section_install`).
2. The page follows the job on `/api/gb/jobs` (the same list Get more uses).
3. Done: **Play with bots** starts the game on that map (`/api/app/launch {map}`), and one
   button per game mode the map is made for that you have (`/api/app/launch {map, mode}`).
   Modes it's made for that you don't have are named, pointing at Mods.

## 6. Publish

1. **Pick a map** (not one of the game's own).
2. **Check** (`GET /api/tools/community/publish/:map`): the map doctor's `diagnose` (damaged
   file, missing textures and files, nav and knowledge fit) and the newest health check report
   for that map. Blockers stop it (a stock map, a damaged file); warnings are shown (missing
   files, no nav, a bad health check); tips nudge (name places, overview, run a health check).
3. **Fill in** name, author (remembered), a sentence or two, the modes it's made for (the
   map's preset mode and the health check's mode pre-ticked) and up to 6 screenshots (PNG/JPEG,
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
   checks the game, the tag and that one of its files has the package's MD5, then builds the
   index entry (page id, file id, MD5, the GameBanana author name, its pictures) and sends it
   to the inbox. If the inbox doesn't answer (it doesn't exist yet) the entry is kept in
   `<app dir>/tools/community/mine.json` as "waiting", retried each time Yours opens, and the
   page offers the entry as text to post in the Discord.

Other tools open Publish with a map: `go("tools", "community:publish:<map>")`. Other addresses:
`community:yours`, `community:mod:<id>`, `community:spray:<id>`.

## 7. Server-side work on dl.lonerstrike.com / list.lonerstrike.com (not done here)

1. **Host the index** at `https://dl.lonerstrike.com/community/index.json` (R2, like
   `navs/list.json`), `Content-Type: application/json`, short cache (`max-age=300`) so a pull
   spreads within minutes. Start with `{"schema": 1, "items": []}`.
2. **The inbox**: `POST https://list.lonerstrike.com/community/submit`, body
   `{"schema": 1, "app": "<version>", "item": {…}}`, in the list service on the Hetzner box.
   It should: rate-limit per IP; re-fetch the GameBanana page itself and re-check the tag, the
   game and the MD5 (never trust the app's copy); store the entry as pending; answer 2xx (the
   app then marks it sent) or 4xx with `{error}`. Optionally post "new creation waiting" to a
   team Discord channel (the status service already talks to Discord).
3. **Building the index**: a small job (cron or a button in a team page) that merges approved
   entries, re-checks every listed page daily (gone, tag removed, file replaced: MD5 no longer
   on the page → hide and tell the team), writes the file and uploads it.
4. **Health and screenshots from our side (optional, later)**: run the health check on a
   submitted map on a team machine to fill `health` independently of the author's numbers, and
   render viewer screenshots to `dl.lonerstrike.com/community/shots/` (already an allowed
   screenshot host).
5. **Discord**: a #creations channel the "waiting" fallback can point at, and a feed of newly
   listed creations.

## 8. Requests to other tools (also in TOOLS-NOTES.md)

- **Installer (archive.rs, owner of gamebanana/content):** allow `.lsv` in `maps/` (the bot
  brain's map knowledge, plain data). Today a package's `.lsv` is skipped on install, so bots
  rebuild knowledge on first play instead of having it ready.
- **Map editor / nav fixer:** a "Publish…" action that does `go("tools", "community:publish:<map>")`
  after a successful compile or save.
- **Map viewer:** "Use as a screenshot for Publish" (the viewer's `shot()` exists for dev); could
  hand a PNG to the Publish form.
- **Spray maker:** "Share on GameBanana" for sprays: the spray image + the tag; register the same way.
- **Game Mode Studio:** a mode package format (mode.ini + cfg + plugins.ini + modes/<id>/ +
  sources) and its installer following section 4; Publish then gets a "game mode" path.

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
beside it when there; a bare GameBanana `.map` or a zip of them works too, centred on install).
WADs in the package go into the game folder only when no file of that name is there. The page then
offers "Open the map editor" (`go("tools", "editor:prefabs")`).
