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
  - Map tools: Map doctor, Map health check, Nav fixer, Callouts, Radar maker
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
