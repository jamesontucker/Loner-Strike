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

## Proposed homepage restructure (outline for review, NOT built yet; 2026-11-13)

### Audit of today's page (about 3,000 words)

| # | Block | Words | Job | Notes |
|---|---|---|---|---|
| 1 | Release banner | 27 | want | Dated launch notice |
| 2 | Hero (tagline, CTAs, loader, editor shot) | 66 | understand, download | Good. Hero shot is the editor, but the page leads with Play |
| 3 | Verbs strip (Play/Build/Host/Share) | 46 | navigate | Repeats the top nav and the chapter headers |
| 4 | Why this exists | 185 | trust | Good voice, but long and placed before any proof |
| 5 | Play header | 30 | understand | Repeats "server full of people" from 6 |
| 6 | Bots that play like people | 101 | want | Repeats team play from 7 |
| 7 | How the bots play (stats, unique, fight, ladder) | 358 | want | Deepest block on the page, and docs material |
| 8 | Modes made here + classics chips | 271 | want | Two cards with 4 bullets each |
| 9 | Build header | 24 | understand | |
| 10 | Map editor | 116 | want | |
| 11 | Game Mode Studio | 118 | want | |
| 12 | Every tool in the box | 288 | want | A catalog: docs#tools does this job |
| 13 | Host header | 26 | understand | |
| 14 | Play with friends | 75 | want | |
| 15 | Dedicated servers | 161 | want | Admin detail is docs material |
| 16 | Share header | 17 | understand | |
| 17 | Community browser | 90 | want, trust | |
| 18 | Our server + Discord cards | 52 | trust | |
| 19 | Screenshots tour | 11 | proof | Sits near the bottom: proof comes after 2,000 words |
| 20 | Name your price / Download | 140 | download | |
| 21 | Thanks (5 credits) | 139 | trust | |
| 22 | Questions (14) | 628 | trust | Half are docs topics (server cost, Hammer import, map learning, AI agent already gone) |

**Doing the same job twice:**
- Navigation: the top nav, the verbs strip and the chapter headers.
- "Feels like a server full of people": in 5 and 6.
- Bot team play: in 6 and 7.
- Beta: in 12 and the FAQ.
- "Friends don't need it": in the Host header and the FAQ.
- GameBanana: in 17 and 21.

### Proposed outline (target about 1,200 words, under half)

Top nav: **Play · Build · Host · Share · FAQ · [Download]**. Sticky, with anchors matching the sections below. The verbs strip goes.

1. **Hero** (#top, about 50 words)
   - H1 "Loner-Strike".
   - Tagline: "Play Counter-Strike like it's 2004, alone or with the friends you don't have."
   - Second line: "Then build your own maps and modes, try them out on the bots, and share them."
   - Primary button **Download free**, secondary **See it in action**.
   - Fine print: free, CS 1.6 and CZ, Windows or a Mac with CrossOver.
   - Visual: in-game bots shot `bots-site` (PLACEHOLDER: the Players page with live bot chat until it exists). The loader and kill feed stay as decoration.
2. **See it** (#screenshots, about 20 words)
   - The tour with caption overlays and arrows.
   - Tabs cut to **Play, Build, Host, Share** to match the pillars, with modes inside Play.
3. **Play** (#play)
   - Line: "A server full of bots that feels like a server full of people."
   - Bullets:
     - Bots that see, aim and chat like people, and remember you
     - Last Stand, Battle Royale and 20+ classic mods, all playable with bots
     - Pick the lobby: newbies to gods
   - Visual: `laststand-wave` (PLACEHOLDER: the ls_outpost render until it exists).
   - Links: "How the bots work", "Game modes" (docs).
4. **Build** (#build, small Beta tag)
   - Line: "The tools we wished we had in 2004."
   - Bullets:
     - Map editor: Hammer's depth, modern editor feel, F5 to playtest with bots
     - Game Mode Studio: modes from nodes and wires
     - A dozen more: map simulator, nav fixer, spray maker and the rest
   - Visual: editor.webp.
   - Link: "Every tool" (docs#tools).
5. **Host** (#host)
   - Line: "Your friends only need the game."
   - Bullets:
     - Host from your game: seven ways for friends to connect
     - Or an always-on server on a rented Linux machine, set up for you
     - Admins and moderators who help from their own app
   - Visual: servers.jpg.
   - Small row: Join our server · Join the Discord (the live cards slimmed to one line).
   - Link: docs#servers.
6. **Share** (#share)
   - Line: "Publish what you make, and try what others made in a click."
   - Bullets:
     - Publish from the app, credits tracked
     - Remix with credit
     - Updates between maps, never mid-game
   - Visual: `community-browse` (PLACEHOLDER: community.webp until it exists).
   - Link: docs#community.
7. **Questions** (#faq, 7 kept, about 300 words)
   - Kept: Is it really free? · Do I need to own Counter-Strike? · Does it work on a Mac? · Windows says it protected my PC · Will it mess up my game? · Do my friends need it too? · What does Beta mean?
   - Moved to the docs: server cost, joining without the app, why bots learn maps, Hammer maps, what can be shared, who made the mods.
   - **Why this exists** is cut to two or three sentences beside the DreamHack photo, as a short note above the FAQ. The alternative is to drop it to the docs; the user should choose.
8. **Download** (#get, about 100 words)
   - Name your price and Download free side by side, with the 3 steps.
9. **Thanks** (about 50 words)
   - One compact row: GameBanana, AlliedModders, ReHLDS, SDHLT, the mod makers, each a link with no paragraph.
   - Then the footer.

**Moved to the docs, not lost:**
- How the bots play: the stats, "What no other bot does", "In every fight" and the ladder (the ladder could stay as the Play visual if the user likes it).
- The mode cards' detail and the classics chips: Play links to Game modes.
- Every tool in the box.
- The dedicated server feature list.
- Seven FAQ answers.

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
