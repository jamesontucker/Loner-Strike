# Site (agent: Site): the 1.0.0 website and docs

Owner of `site/` for the 1.0.0 overhaul. Release date on the site: 2026-11-14.

## What changed

- **Home page (`site/index.html`)**: told as four parts, Play, Build, Host and Share. There's a tab strip under the hero that jumps to each part, and each part opens with a big stencil heading.
  - Hero: the map editor in the window frame. The loader, kill feed and bot chat are kept.
  - New sections: Last Stand and Battle Royale cards with map renders, the map editor, Game Mode Studio, every tool (linked to its docs topic), AI agents (MCP) with the Claude Code line, and the Community browser with its rules.
  - The old 10-picture grid is now a **tabbed screenshot tour**: Play, Game modes, Map editor, Game Mode Studio, Tools, Hosting and Community.
    - Arrow keys, Home and End move between tabs (roving tabindex).
    - Arrow keys move along the thumbnail strip, and you can swipe the big picture on phones.
    - Clicking the picture opens the big viewer, which also takes arrow keys and swipes.
  - Meta tags, JSON-LD (features, screenshots, softwareVersion 1.0.0) and the FAQ (plus 4 new questions) are updated.
  - The download box says which file to take (.dmg or -setup.exe) for the visitor's computer. The download links are unchanged.
- **Banner**: "comes out November 14". The script switches it to "1.0.0 is out … Get it" from 2026-11-14 00:00 Pacific.
- **Docs (`site/docs.html`)**: still one page with one topic shown at a time, so every old `docs#…` link (the app uses several) keeps working. The client-side search covers every new topic. New groups and topics:
  - Basics: What's new in 1.0
  - Game modes: Last Stand, Battle Royale, Deathrun and Surf
  - Create: The Tools, Map editor, Compile and playtest, Hammer and J.A.C.K. maps, Game Mode Studio, Playtest and debug a mode, Plugin Studio (with loner.inc), Creature Studio
  - Map tools: Map doctor, Map simulator, Nav fixer, Callouts, Radar maker
  - Personalize: Crosshair & config, Bot people, Spray maker
  - Watch & inspect: Map viewer, Replay, Demos, Textures & models
  - Share: Community, Publishing, Community and your privacy
  - AI agents: Connect an AI agent (setup for each client)
  - Help: Tools, the editor and the studios
  - Existing topics got links to all of these (the app's pages, game modes, FAQ).
- **Other pages**: `privacy.html` now lists the app's new connections (dl./list.lonerstrike.com, SDHLT) and has a Community section. `llms.txt` and `llms-full.txt` cover 1.0, `sitemap.xml` dates are moved to 11-14, and `_redirects` has the short links /mcp, /tools and /whats-new.
- **Screenshots (`site/img/1.0/`)**:
  - Each shot is a 1440-wide `.webp` with a `.png` fallback (256 colours, at most 1200 wide), plus a 320-wide thumbnail in `thumb/`.
  - The app shots (`app-*.webp`) are webp copies of the older `img/gallery/*.jpg`. Those jpgs stay as their fallbacks.
  - All are real captures from `npm run tools:dev`, taken with my own Playwright script on ports 8137/5273, against a symlinked copy of the game folder that adds ls_outpost.
  - Map renders come from the viewer: ls_outpost, br_valley and de_dust2. The editor shots are ls_outpost.map imported via `LS_DEV_PICK`.

## Preview locally

The site uses clean URLs (`/docs`, as on Cloudflare Pages). This serves them like Cloudflare does:

```sh
cd site
python3 -c '
import http.server as h, os
class H(h.SimpleHTTPRequestHandler):
    def send_head(self):
        p = self.path.split("?")[0]
        if not os.path.exists(self.translate_path(p)) and os.path.exists(self.translate_path(p) + ".html"):
            self.path = p + ".html"
        return super().send_head()
h.test(H, port=8000, bind="127.0.0.1")'
```

Then open http://localhost:8000/ and http://localhost:8000/docs. Plain `python3 -m http.server 8000` also works if you open `docs.html` by its full name. Add `?drafts` to the home page to see where the in-game shots go.

## The upload zip

Same method as before: a list of files, zipped from inside `site/`.

```sh
cd site
find . -type f ! -name .DS_Store | sed 's|^\./||' | sort > /tmp/zipfiles.txt
rm -f ~/Desktop/lonerstrike-site-1.0.0.zip
zip -X ~/Desktop/lonerstrike-site-1.0.0.zip -@ < /tmp/zipfiles.txt
```

(`rm` first: zip adds to an existing archive instead of replacing it.)

## Homepage pass from the user's feedback (2026-11-13)

- **Evergreen copy**: no version numbers in the homepage copy (hero, Play, Build, Why, FAQ, og/twitter titles, JSON-LD; `softwareVersion` dropped). The release banner still names 1.0.0 on purpose: it's the dated launch notice, so take it out after launch week.
- **Hero**: "Then build your own maps and modes, try them out on the bots, and share them with everyone."
- **Modes**: "New in 1.0: two modes of our own" became **"Modes made here"**, a showcase that also points to Game Mode Studio (`#studio`) and Community. Last Stand says it's endless by default.
- **Build**: the map editor section is now **"Hammer's depth, a modern editor's feel"** ("inspired by Unreal Engine's editor", nothing more), covering the one editor bar, the Content Browser, live checks, F5 and import. Every tool in the box has a small Beta tag, a Content Browser entry and a fifth group, Share (Community, Credits). The groups sit 3 + 2 on desktop.
- **Cut**: the AI agents (MCP) section, its CSS, its FAQ (HTML and JSON-LD) and the MCP feature line. The MCP docs topic stays on the docs page.
- **Share**: no virus-scan claim. The section covers publishing, credits, remix, ownership, updates, bot ratings and moderation.
- **Clipping fixes**:
  - The chapter headings' block underline uses `background-repeat: space` with a 12 px tile, so it draws whole blocks only.
  - The loader bar's block count follows its width (ResizeObserver, `contain: inline-size`), so a resize never leaves half a block. It also no longer widens the hero on phones.
  - `.meter` blocks are `flex: none`.
- **Screenshot tour**:
  - The caption sits over the picture's lower edge on a gradient scrim, in a `figcaption` with `aria-live`.
  - Prev and next arrows are always visible on the picture.
  - The arrow keys work anywhere in the panel, and the strip scrolls by itself without scrolling the page. Smooth scrolling is off for reduced motion.
- **Docs anchors the homepage now uses**: `docs#content-browser` and `docs#credits`.

## Proposed homepage restructure, v2 (outline for review, NOT built yet; 2026-11-13)

The baseline is the homepage before the 1.0 overhaul: `840ca93d:site/index.html`, about 2,140 words. Today's page is about 3,000 words.

### The page before 1.0, in order (all original)

1. Top bar
2. Hero, with the map browser shot (42)
3. **Why this exists** (151)
4. Bots that play like people (96)
5. **How the bots play** (352), including **Pick the lobby**, the skill meters
6. Classic community modes (67)
7. Play with friends (75)
8. Dedicated servers (155)
9. **Everything in it** (312), a grid: Bots, Modes and maps, Friends, Servers, The app. The 1.0 overhaul removed it.
10. Screenshots (a grid)
11. Name your price / Download free (146)
12. Support the people who made this possible (116)
13. Community: our server and Discord (52)
14. Questions (11, 510)

### Added for 1.0 (all new, about 1,190 words)

- The Play/Build/Host/Share strip (46)
- Four big chapter headers (97)
- Modes made here: two cards and a Studio line (about 204, plus the original classics)
- The map editor (116)
- Game Mode Studio (118)
- Every tool in the box (288)
- The Community browser (90)
- The tabbed tour, replacing the original grid
- One more "Why" paragraph (about 35)
- 3 FAQ items: Beta, Hammer, Community (about 118)
- The SDHLT thanks card (23)

### Outline: words now → after

| # | Section | Status | Now | After |
|---|---|---|---|---|
| 1 | Top bar | Original, lightly updated: nav anchors are Play, Make, Host, Share, Screenshots, FAQ | 27 | 27 |
| 2 | Release banner | Launch week only, then removed | 27 | 0 |
| 3 | Hero: "Play Counter-Strike like it's 2004, alone or with the friends you don't have." plus one line "Then build your own maps and modes, try them out on the bots, and share them." | Original, lightly updated. The original map browser shot comes back (or the editor; user's pick) | 66 | 50 |
| 4 | Play/Build/Host/Share strip | New: cut (repeats the top nav) | 46 | 0 |
| 5 | Why this exists | **Original**: kept as it was, with the 1.0 paragraph cut to one sentence ("Then I wanted to make things again, so now it has the tools for that too.") | 185 | 160 |
| 6 | Chapter headers (giant Play/Build/Host/Share) | New: cut. The original page had none | 97 | 0 |
| 7 | Bots that play like people | Original, lightly updated ("They learn the rules, whatever the mode") | 101 | 95 |
| 8 | How the bots play, with Pick the lobby | **Original**, kept | 358 | 352 |
| 9 | Classic community modes: the original heading, lead and chip list. **Last Stand** and **Battle Royale** join the list as entries like the others, each with one line, a small "made here" tag and a link to its docs section. Chip text: "Last Stand · made here: hold an outpost against waves of creatures" (links to docs#last-stand); "Battle Royale · made here: drop in, loot up, outlast the zone" (links to docs#battle-royale) | Original, lightly updated. The separate mode cards and the "Modes made here" heading are cut | 271 | 95 |
| 10 | **Make your own** (one feature block like Play with friends: text plus one picture). Line: "The tools we wished we had in 2004." Bullets: map editor ("Hammer's depth, modern editor feel"), F5 to playtest with bots; Game Mode Studio, modes from nodes and wires; map simulator, nav fixer, spray maker and more (Beta). Links to docs#tools | New, condensed from the editor, Studio and tool catalog | 522 | 85 |
| 11 | Play with friends | Original, kept | 75 | 75 |
| 12 | Dedicated servers | Original, kept | 161 | 155 |
| 13 | **Share it** (same feature style plus the community picture). Line: "Publish what you make, and try what others made in a click." Bullets: Try it with bots; publish from the app with credits tracked; remix with credit. Links to docs#community | New, condensed | 107 | 50 |
| 14 | Everything in it | **Original**, restored and updated: its 5 groups plus "Make and share" (one line each, no tool catalog) | 0 | 200 |
| 15 | Screenshots | Original place and heading. Keeps the tabbed tour with captions and arrows | 11 | 11 |
| 16 | Name your price / Download free | Original, kept | 140 | 140 |
| 17 | Support the people… | Original, kept, plus the SDHLT card | 139 | 139 |
| 18 | Community: our server and Discord | Original, kept | 52 | 52 |
| 19 | Questions | Original 11 kept. Of the new ones, "What does Beta mean?" stays (one sentence); Hammer and Community move to the docs | 628 | 540 |
| | **Total** | | **about 3,000** | **about 2,225** |

That cuts about a quarter, not half. The original sections alone come to about 2,000 words. Halving would mean cutting originals too. Options for the user, in order of least loss:
- a. Move "And in every fight" (the 6-item list and the ideas note, about 140) to docs#how-bots-work.
- b. Make "Everything in it" one line per group (about 200 → 110).
- c. Move "What does a server cost?" and "Can people join without Loner-Strike?" to the docs (about 110).

Doing a, b and c brings it to about 1,865.

### Our modes are no longer singled out (user, 2026-11-13)
Besides row 9, the build also changes these:
- Play line (row 7 area): "The classic community mods, modes of our own, and…" goes back to the original wording, or drops "modes of our own".
- The bot bullet "They learn the rules, whatever the mode: the classics, ours and the ones you make" becomes "They learn the rules, whatever the mode".
- "They play the mode" goes back to the original: "Run and gun in Deathmatch, a team in a 5v5." The "hold the walls in Last Stand" clause is dropped.
- The Studio bullet "Start from Last Stand, Battle Royale or a blank mode" moves into the Make your own block as "start from a template or a blank mode".
- The og/twitter descriptions, JSON-LD description and feature list lose the separate "Last Stand and Battle Royale" item. Both stay in the mode list there.
- The screenshot tour keeps its Game modes tab (captions name the modes). That tab is the proof, not a callout.
- Pending the user's answer: the hero picture (map browser or editor) and the optional trims a, b and c.

### Built (2026-11-13)
The v2 outline is built with the user's answers.
- The hero has the original map browser picture.
- Only trim (b) is made: "Everything in it" has one line per group. "And in every fight" and the two server FAQ questions stay.
- Static HTML went from about 3,000 to about 2,280 words.

**Screenshots: inline, in each section's app window.** The separate tour is gone. Make your own (18 shots), Play with friends (2), Dedicated servers (3) and Share it (2, with one placeholder) each have a window you can flip through:
- the caption sits on a scrim, with arrows on the picture, a thumbnail strip when there are 3 or more, and the arrow keys;
- swipe works on phones, and a click opens the big viewer on that section's shots.

Draft in-game shots stay out of the public page. `?drafts` shows them as dashed, clearly labeled placeholders. Shot lists are `SETS` in the script; `INGAME` turns a taken shot on. The runner-up was the single tabbed tour (one big shot plus a strip), as in commit b59bafd7.

### The open questions: both sections were original
- **Why this exists** is original (151 words before 1.0). Recommendation: keep it whole with its DreamHack photo, and cut only the added 1.0 paragraph to one sentence. That's better than cutting it to 2–3 sentences.
- **Pick the lobby** (the skill meters) is original, inside "How the bots play". Recommendation: keep it as it was. Play's lead picture stays the Players page with live bot chat, as it was before 1.0. The `bots-site` in-game shot can replace that picture when it exists (marked as a placeholder).

## In-game shots to take (user)

Save each one as a JPEG at `site/img/1.0/ingame/<name>.jpg`, 16:10 if you can (for example 1680×1050 or 1280×800, or crop to it). Turn off net_graph and the console.

To turn one on, add its name to `const INGAME = new Set([...])` in `index.html`, then check it with `?drafts`. Until a name is in that list, its shot is left out of the tour; nothing broken shows.

| name | tab | framing |
|---|---|---|
| `bots-site` | Play | de_dust2, as a T, coming out of B tunnels behind 3–4 bot teammates pushing onto the site. HUD and radar on, crosshair on the site, some smoke or a flash if you get one. |
| `bots-chat` | Play | Buy time in T spawn. You've typed "someone buy me an awp", a bot answers in team chat, and its AWP is on the floor in front of you. The chat lines must be readable (bottom left). |
| `laststand-wave` | Game modes | ls_outpost, standing on the west wall's firing step looking down at the west gate barricade with 6+ creatures pushing it. The "WAVE n/10 – x left" banner is visible, and a bot defender beside you if possible. |
| `laststand-revive` | Game modes | ls_outpost courtyard between waves: reviving a downed teammate with the progress bar visible. The keep or the armory counter is in the background. |
| `br-dropin` | Game modes | br_valley at the start: mid-air, high, looking down at the town hall and the river, with other players or bots dropping in view. |
| `br-zone` | Game modes | br_valley late game near radio hill or the bridges: the zone edge visible, the round clock showing, 2–3 enemies or squad mates in frame. |
| `deathrun` | Game modes | A deathrun map with bot runners on a trap section (spikes, a moving floor). Ideally the activator bot is at its buttons too (a spectator angle works). |
| `surf` | Game modes | surf_ski_2 or similar: spectate a bot mid-ramp in chase cam, with the ramp running across the frame. |
| `community-browse` | Community | App, not game: Tools › Community › Browse once the first creations are listed, with a creation picked showing "Try it" and its badges. |

**Optional:** the two mode cards on the home page use map renders. To use in-game shots there instead, swap the `<picture>` sources marked `IN-GAME SHOT WANTED` in `index.html`.

**Optional, app shots to retake for 1.0:** the Play, Hosting and "Bots that play like people" pictures are still the 0.1 app shots (`img/gallery/*.jpg`, `img/play.jpg`, `img/friends.jpg`, `img/servers.jpg`). The 1.0 sidebar is grouped differently, so retake them in the app at 1280×950 if you want them current. Keep the same file names, then run `cwebp -q 78 x.jpg -o img/1.0/app-x.webp` (and make the thumbnail with `-resize 320 0`).

## Notes and things to check

- I didn't change `og.jpg` (the share image). It still shows the 0.1 look, which is fine but could be refreshed.
- The docs describe the "My library" (Studio) feature nowhere: it's uncommitted and not finished (per the Studio notes). Hammer/J.A.C.K. import is written about without promising the slanted-brush fixes; the docs ask people to check imports and keep their original file.
- Seen in tools-dev only (it may be the harness missing global styles): Community › Publish's tip boxes wrap very narrow, and its inputs are unstyled. Worth a look in the real app.
- Processes: my dev server (ports 8137/5273) and preview server (8462) are stopped when I finish.

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

### Round 5 (2026-11-13)
- Thanks is now 4 + 4: added Valve, Bots-United (Metamod-P) and the open-source tools (whisper.cpp, llama.cpp, Tauri, React).
- Make your own names four tools: the map simulator, nav fixer, callouts and the spray maker.
- Publishing wording: GameBanana is read-only, so it now reads "Package it in the app and upload it to GameBanana, with the credits filled in for you" on the homepage and in the docs.
- docs.html is tightened by about 30%, from 20,020 to 13,948 words. Every id and anchor is kept. The before and after counts per topic are in the scratchpad docs2/*.json.

## From P5 (2026-11-13): map editor keys and names for docs.html
- **Content drawer**: **Ctrl+Space** on Windows and Linux (as in Unreal); **⌘⇧Space** on a Mac (⌘Space is Spotlight,
  Ctrl+Space switches the input source). With TrenchBroom keys it's Ctrl+Shift+Space / ⌘⇧Space (Ctrl+Space maximizes
  a pane there). Also View > Content drawer.
- **Panels**: "Content" (Models, sprites, sounds, textures and prefabs; Content › Prefabs replaces Library › Prefabs) and
  "Gameplay" (Components and mode kits for game modes; was "Library"): #editor-library, the Studio's Map markers
  paragraph and the Community "Installed things show up…" paragraph still say Library.
- **Playtest**: F5 in every key set, Map > Playtest, and the Playtest button in the editor's one bar.
- **Budget panel** (new, View > Budget; a bottom tab after Checks): bars for how close the map is to GoldSrc's limits,
  green, yellow over 75%, red over 90%. *Precache*: models and sounds (512 each) = the game's own share (an estimate),
  what the map's entities load, and the creatures of the mode it's made for ("Last Stand with this roster: N sounds of
  headroom"). *As you build*: brush entities, entities, an edicts estimate, different textures. *Last compile*: the
  compiler's own planes, clipnodes, faces, leaves, marksurfaces, lighting… Click a bar for what uses most, the usual
  fix, and Pick to select it. Playtest and Publish ask first when a bar is red.
