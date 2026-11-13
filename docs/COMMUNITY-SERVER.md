# Community creations: the server side

The app's Community browser (docs/COMMUNITY.md) talks to two places we run:

- **list.lonerstrike.com** — the existing server list service on the Hetzner box (Python, standard
  library, systemd, behind the Cloudflare tunnel). It now also takes registrations and reports,
  checks them against GameBanana, keeps the moderation state in SQLite and posts cards with buttons
  to the private Discord channel 🛡️・community-reports.
- **dl.lonerstrike.com/community/index.json** — the static index the app reads (R2), built from that
  state and uploaded by `lsmod.py publish --upload`.

Nothing here is live until the steps in section 6 are run.

## 1. Files

| File | What |
|---|---|
| `services/serverlist/serverlist.py` | The list service. `--community-db` turns the community part on; routes `/community/*` and `/discord/interactions` to it. |
| `services/serverlist/community.py` | Registrations, checks, reports, moderation, re-check, index builder, admin API. |
| `services/serverlist/modbot.py` | Discord cards and buttons (bot posts, Ed25519-checked interactions, Undo, slash commands). |
| `services/serverlist/lsmod.py` | Moderation and publishing from a terminal (Mac, or on the box against 127.0.0.1). |
| `services/serverlist/discord-setup.py` | One-time: registers `/community` and sets the Interactions Endpoint URL. Dry run unless `--apply`. |
| `services/serverlist/install.sh`, `set-env.sh`, `community.env.example` | Install/update on the box; set one secret from stdin; the env file's template. |
| `services/serverlist/lonerstrike-list.service` | The unit (now with `EnvironmentFile=` and `--community-db`). |
| `services/serverlist/lonerstrike-community-publish.{service,timer}` | Optional: publish from the box every 5 minutes when the index changed. Installed, not enabled. |
| `scripts/r2-community.sh` | Uploads `index.json` + `index.json.sha256` to R2 with the aws CLI, like `r2-navs.sh`. Dry run unless `--upload`. |
| `services/serverlist/test_community.py`, `test_modbot.py`, `testdata/gamebanana/` | Offline tests with recorded GameBanana answers and forged Discord interactions. |

Tests: `cd services/serverlist && python3 -m unittest test_community test_modbot test_serverlist`
(108 tests). A live check that a real GameBanana zip can be listed by range requests:
`python3 -c "import community as c; print(c.package_names({'id': 1886785, 'name': 'cage.zip', 'size': 278384}))"`.

## 2. How it works

**A registration** (`POST /community/submit`, sent by the app after Publish). The body says only
which page and file: `{"schema": 1, "app": "<version>", "page": {"model": "Mod"|"Spray", "id": 123,
"file": 456}}` (an older app's `{"item": {...}}` is read for its `model`, `gb` and `file` only).
Anyone may register any tagged page — the tag is the creator's say-so — because nothing in the
request ends up in the listing: a stranger can at most make an entry match the page and package again.

1. Rate limits per visitor: 10 an hour, 30 a day, and a share of 10 of the 30 GameBanana checks a
   minute, so one source can't use them all up. A visitor is an IPv4 address or an IPv6 /64 (from
   Cloudflare's `CF-Connecting-IP` only when the request comes from the tunnel on 127.0.0.1, like
   `/register`); never a GameBanana member, so nobody can use up a creator's quota. The in-memory
   tables keep at most 20,000 visitors, dropping the longest-quiet first. Body at most 64 KB, by
   `Content-Length` or chunked (Cloudflare may forward either).
2. A pulled page answers 409. A retry reads the page again (is there a newer file?) but not an
   unchanged package.
3. The page is fetched from `https://gamebanana.com/apiv11/<Mod|Spray>/<id>/ProfilePage` (the only
   host; ids from validated integers; no redirects; 4 MB cap; 30 fetches/minute in all). It must:
   exist and not be trashed/private/withheld/flagged; be Counter-Strike 1.6 (4254); carry
   `Software Used: LonerStrike`; be in a category we take (its kind comes from it: Maps 5474,
   Prefabs 451, Textures 681, AMXX 3521 for Studio modes, Sprays).
   **The server picks the file**, not the request (its file id is only a hint): the page's newest
   file (GameBanana's date, then id) that passes every check here and below, scanned clean, 1 byte
   to 1 GB; up to three are tried. For a page already registered only a file newer than the one it
   has counts, so nobody can switch a listing back to an old version; if a newer file fails, the
   listed one stays and the answer and the entry's check note say why. Going back to an older file
   is a moderator's `lsmod.py use-file <page> <file id>`. A file the team turned down (same MD5)
   answers 409.
4. **The package**, read with HTTP range requests from `gamebanana.com/dl/<file>` (redirects only to
   `files.gamebanana.com` / `filecacheN.gamebanana.com`), at most 1 MB read in all, and only two
   packages at a time (others get a 429 "try again in a minute", which the app retries):
   - the zip's end record first: more than 5000 entries, a directory over 1 MB, split or zip64
     zips are refused before anything else is read;
   - the directory and the entries must agree: the first entry at byte 0, each next one at most
     96 bytes after the previous one's data (room for a local extra field and data descriptor, not
     for a hidden entry), the directory right after the last, and up to 16 local headers spread
     over the zip compared with the directory (name, method, size);
   - every name must be one the app's installer would write (no backslashes, `..`, empty or dot
     parts, NUL, trailing dots/spaces, nothing outside content.rs `safe_part`'s characters), no
     two names differing only in case;
   - **an allow-list per kind**, mirroring the app's installer (archive.rs `content_exts`,
     content.rs `allowed`): `README.txt`, `loner-strike.json`, and for maps `maps/*.bsp .nav .txt
     .res .lsv`, `gfx/*.tga .bmp`, `models/*.mdl`, `sound/*.wav .mp3`, `sprites/*.spr`,
     `overviews/*.txt .bmp .tga`, custom `.wad`s at the root (not the game's own); prefabs
     `prefabs/<id>.map .json .png` + wads; textures root wads; sprays `tempdecal.wad`, root wads,
     `logos/*.bmp`, a root picture; Game Mode Studio modes `modes/<id>/**.json .ini .txt`,
     `addons/amxmodx/configs/gamemodes/<id>.ini`, plus content (provisional until the Studio's
     package format is fixed). Anything else is refused, with clear messages for code (plugins get
     "Plugin modes can't be shared yet…") and for game settings (`.cfg`, `.gam`, `liblist.gam`,
     `plugins*.ini`);
   - `loner-strike.json` (one entry, at most 64 KB packed and unpacked whatever the header claims,
     stored or deflated, not encrypted; a damaged stream is a clean 422). Maps, modes and prefabs
     need one, of the page's kind; a lone `.wad`/picture (sprays, textures) needs none.
5. **The entry is built only from verified sources**: title, author and pictures from the page; the
   file id and MD5 from the page; summary, games, tags, map modes and mode details from the
   package's manifest (checked with the same strict rules); the map's `.bsp` must be in the zip
   (`bspSize` from it), and `nav`, `places`, `lsv`, `res`, `overview` count only when those files
   ship. Health numbers are dropped (unverified) until we run checks ourselves.
6. **Listed at once**. A card goes to the mod channel (title, kind, page id, thumbnail, link,
   **[Pull]**). If the team blocked that member, or hid or turned down this page, the new file
   waits instead (card with **[Approve] [Reject]**). An update to a listed page replaces it at once.

Answers: `200 {"ok": true, "key": "mod:123", "state": "listed" | "pending" | "hidden"}`; refusals
`400/409/413/422 {"error": "<what to fix>"}`; `429` rate limit; `502` GameBanana didn't answer;
`503` busy. The app treats a 4xx other than 429 as final and shows the reason; 429/5xx stay
"waiting" and are retried.

**Who can undo what.** Each entry records who took it out of the list (`hidden_by`: `team`,
`recheck` or empty). A creator's new registration never reverses a moderator: a page the team
pulled or hid, or turned down, stays so; the new file waits for a moderator (pulled pages refuse
it). A re-check hide (the listed file vanished) is lifted by a new registration that passes every
check; a re-check rejection doesn't hold either. Pulls by anyone stay until a moderator restores
them. Blocks only change by a moderator. The full table is at the top of community.py's
migrations, tested in `test_community.Transitions`.

**Where a registration stands** (`GET /community/submission/<mod|spray>/<id>`, public, CORS `*`):
`{"key", "state": "unknown|pending|listed|hidden|rejected|pulled", "updated", "md5"?, "waiting"?, "reason"?}`.

**A report** (`POST /community/report`, the app's Report button, body
`{"schema": 1, "app": "<version>", "model": "Mod"|"Spray", "id": 123, "reason":
"broken|malware|stolen|offensive|not-loner-strike|other", "note": "<up to 500 characters>"}`):
5 per visitor per hour, one per visitor per page per day (more are thanked and dropped), 300 a day in
all. Kept 90 days with the reason, the note and a **reporter hash** (HMAC of the address with a salt
that lives only in memory: the same reporter reads the same until a restart; no address is stored).
Any GameBanana page can be reported, listed or not. The channel gets a card (reason, escaped note,
reporter hash, reports this week, **[Pull] [Keep] [Block creator]**), at most one per page per half hour.

**The mod channel.** Cards are posted by the bot application ("Loner-Strike Setup") with
`allowed_mentions: {parse: []}` and outside text escaped and capped, so a page name or note can't
ping anyone. Clicks arrive at `POST /discord/interactions`:
- Discord's Ed25519 signature over timestamp + body is checked with the app's public key
  (`cryptography` when installed, else the RFC 8032 verifier in modbot.py); timestamps older than
  5 minutes are refused; PING is answered.
- Only our guild, and only members with the Moderator or Dev role or the owner; anyone else gets an
  ephemeral "You can't do that."
- Button ids are opaque and signed (`ls1:<action>:<card>:<hmac>`, keyed from the bot token), so a
  forged id can't name another card or action. Each interaction id is done once (Discord may
  resend); a second click on a done card says who already did it.
- Pull and Reject ask for a reason in a small form (needed to reject). Approve doesn't re-check the
  page (buttons must answer in 3 seconds; the checks passed when it came in). Block creator blocks
  that GameBanana member's next submissions.
- The card is edited ("Pulled by @name · reason") with **[Undo]**, and an audit line is posted.
  Undo puts the entry back exactly as it was, unless it changed since (then: use lsmod.py).
- Slash commands (guild): `/community pull|keep|block|status <page> [reason]`.
- If the bot can't post, the same news goes to the channel's webhook as text (no buttons).

**Pulls reach apps when the index is published again** (`lsmod.py publish --upload` on the Mac, or
the optional timer on the box). Pulled pages stay in the index with `status: "revoked"` and in the
top-level `revoked` list, so apps hide them even when GameBanana still lists the tag.

**The daily re-check** (a thread in the service, first run 10 minutes after start): each listed,
hidden and waiting page is fetched again (2 s apart). Gone, untagged, moved out of CS 1.6 or flagged:
pulled (waiting ones turned down). The listed file's MD5 no longer on the page: hidden ("replaced or
removed"; a new registration brings it back). Flagged by the virus scan: pulled. Network trouble
changes nothing. **Guard**: when more than 3 entries, or 2 or more making over a quarter of them,
would leave the list (pull, hide or turn down) in one run, nothing changes, the channel is told,
and the publish timer doesn't publish until a person has looked (GameBanana having a bad day, or
its API changing). A single change always goes through.

**The index**: `GET /community/admin/index` builds `community/index.json` from the database —
exactly community.rs's `Index`/`Item` (camelCase, every field typed as the app reads it; checked
by compiling the app's structs and `parse_index` against a generated index: all entries kept).
`schema: 1`, `updated` = last moderation change (same database, same bytes), `revoked`, an empty
`verifiedAuthors`, items featured first, newest first, at most 5000 and 8 MB. Modes always carry
`plugins: []`, `review: "none"` (the fields stay for later). An `index.json.sha256` goes beside it,
and the app refuses an index that doesn't match it (keeping the copy from before). That catches a
damaged or half-finished upload; a signature with a key pinned in the app would also stop a
tampered one (request in COMMUNITY.md section 8).

## 3. Admin API and lsmod.py

All under `/community/admin/`, `Authorization: Bearer <COMMUNITY_ADMIN_TOKEN>` (32+ characters;
compared as SHA-256 digests with `hmac.compare_digest`; 10 wrong tokens from one visitor lock it out
for an hour; no CORS headers). **They answer only on the box itself** (requests from 127.0.0.1 without
Cloudflare's visitor header): from a Mac, open a tunnel first, `ssh -N -L 8100:127.0.0.1:8100
cs@178.156.241.180`, and lsmod.py's default `--server http://127.0.0.1:8100` goes through it.
`--admin-remote` on the service opens them through the Cloudflare tunnel too; if you ever want that,
put a Cloudflare Access application (your own login) in front of `list.lonerstrike.com/community/admin/*`
as well as the token. A second, publish-only token (`COMMUNITY_PUBLISH_TOKEN`) may only read the
index and status and say what was published (the publish timer uses it).

`lsmod.py` (Mac: token in `~/.config/lonerstrike/community-admin.token`, chmod 600, or
`LS_COMMUNITY_TOKEN`; it refuses plain http to anything but 127.0.0.1): `queue [--state …]`, `show`, `pull --reason`, `restore`, `hide`, `approve`,
`reject --reason`, `block`, `unblock`, `blocked`, `reports [page]`, `feature on|off`,
`use-file <page> <file id>`, `recheck [page]`, `status` (counts; "CHANGED, run publish" when the index differs from what was
last published), `publish [--out DIR] [--upload] [--if-changed] [--auto]` (`--auto`: the timer's
mode, also skipping after a held-back re-check). Pages as `mod:123`, `spray:45` or the GameBanana link.

## 4. Configuration

`/etc/lonerstrike-list/community.env` (root, 0600, read by systemd before it drops to the
service's dynamic user; secrets are removed from the process environment once read; none are
logged):

| Variable | |
|---|---|
| `COMMUNITY_ADMIN_TOKEN` | lsmod.py's token. install.sh makes one. Without it, moderation answers 503. |
| `COMMUNITY_PUBLISH_TOKEN` | The publish timer's read-only token. install.sh makes one (and puts it in `publish.env`). |
| `DISCORD_BOT_TOKEN` | The bot application's token (from `~/.config/loner-discord-token`). Without it: no cards, the webhook gets text. |
| `DISCORD_APP_ID`, `DISCORD_PUBLIC_KEY` | Public; printed by `discord-setup.py`. Without the key, `/discord/interactions` answers 503. |
| `DISCORD_GUILD_ID`, `DISCORD_CHANNEL_ID`, `DISCORD_MOD_ROLES`, `DISCORD_OWNER_ID` | Filled in by the template: guild 1568421500636233748, channel 1570640821320159252, roles Moderator 1568425985504968768 + Dev 1568425984288362637, owner 310619637727363072. |
| `COMMUNITY_MOD_WEBHOOK` | Fallback: the channel's webhook (from `~/.config/lonerstrike/community-webhook`). Only `https://discord.com/api/webhooks/…` is ever posted to. |

`/etc/lonerstrike-list/publish.env` (root, 0600) is the publish timer's own file, with only what
it needs: `LS_COMMUNITY_TOKEN` (the publish token) and `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`,
`R2_ENDPOINT`, `R2_BUCKET` (fill in an R2 token for the bucket if you turn the timer on). The list
service never sees the R2 keys, and the timer never sees the bot token or the admin token.

The unit allows 192 MB (`MemoryMax`): the list, the database and two package checks at 1 MB each.
Discord signatures use `python3-cryptography` (install.sh installs it; without it a slower
built-in RFC 8032 check is used and the service says so in its log at start).

The database is `/var/lib/lonerstrike-list/community.db` (systemd StateDirectory). Migrations run
at start (`PRAGMA user_version`; version 1 creates everything; version 2 adds `hidden_by`, with
rows already hidden, pulled or turned down counted as the team's).

## 5. Security notes

- Outbound requests: only `gamebanana.com` (API) and its file hosts (range requests), `discord.com`
  (bot channel calls and the webhook). Exact host match, https only, no proxies from the
  environment, redirects followed only for downloads and only to GameBanana's file hosts.
- Inbound bodies capped (64 KB submit/admin, 4 KB report); JSON shape and types checked; text
  stripped of control/direction characters; malformed Discord commands get a polite answer.
- The public status endpoint is rate-limited (60 a minute per visitor) and shows the reason the
  moderators gave (written for the author), never who gave it; Discord pulls without a reason say
  "Pulled by the moderators".
- No visitor addresses stored or logged; logs say what happened to which page, never tokens or
  webhook addresses.
- Not covered: DNS for gamebanana.com resolving somewhere private (the host is fixed, so this would
  need DNS compromise); a tampered index on the download server together with its checksum (needs
  a signature).

## 6. Go-live steps (for you to run; nothing here has been run)

Box: `cs@178.156.241.180` (Ubuntu 24.04, sudo). Run from the tools checkout
(`cd ~/loner-strike-tools`, or the main checkout once merged).

1. **Tests**: `! cd services/serverlist && python3 -m unittest test_community test_modbot test_serverlist`
2. **Copy and install** (keeps the list running; adds the community part; makes the env file with
   a fresh admin token):
   ```
   ! scp -r services/serverlist scripts/r2-community.sh cs@178.156.241.180:/tmp/ls-new/
   ! ssh cs@178.156.241.180 'sudo sh /tmp/ls-new/serverlist/install.sh'
   ```
3. **Secrets into the env file** (from your Mac's files over stdin; never shown, never on a command line):
   ```
   ! ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh DISCORD_BOT_TOKEN' < ~/.config/loner-discord-token
   ! ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh COMMUNITY_MOD_WEBHOOK' < ~/.config/lonerstrike/community-webhook
   ```
4. **The admin token to your Mac** (straight into the file, not the screen):
   ```
   ! (umask 077; ssh cs@178.156.241.180 'sudo sed -n "s/^COMMUNITY_ADMIN_TOKEN=//p" /etc/lonerstrike-list/community.env' > ~/.config/lonerstrike/community-admin.token)
   ```
5. **The app's id and public key** (public values):
   ```
   ! python3 services/serverlist/discord-setup.py
   ! echo <DISCORD_PUBLIC_KEY it printed> | ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh DISCORD_PUBLIC_KEY'
   ! echo <DISCORD_APP_ID it printed> | ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh DISCORD_APP_ID'
   ! ssh cs@178.156.241.180 'sudo systemctl restart lonerstrike-list && sleep 2 && journalctl -u lonerstrike-list -n 5 --no-pager -o cat'
   ```
6. **Cloudflare**: no new tunnel route is needed — `list.lonerstrike.com` already sends every path to
   `http://localhost:8100`, so `/community/*` and `/discord/interactions` arrive. Check:
   `! curl -s https://list.lonerstrike.com/community/health` → `{"ok": true}`, and a POST with a body
   reaches the service:
   `! curl -s -X POST https://list.lonerstrike.com/community/submit -H 'Content-Type: application/json' -d '{"schema": 1}'`
   → `{"error": "The registration doesn't say which GameBanana page it is."}` (a Cloudflare page or an
   empty answer means the tunnel or a WAF rule is in the way; chunked bodies, which Cloudflare may
   forward instead of `Content-Length`, are read too). If the zone has Bot
   Fight Mode, a WAF challenge or rate limiting rules, add a skip rule for
   `http.host eq "list.lonerstrike.com" and (starts_with(http.request.uri.path, "/discord/interactions") or starts_with(http.request.uri.path, "/community/"))`
   — Discord and the app can't solve challenges.
7. **Discord buttons and the slash command** (Discord PINGs the endpoint while saving it; step 5
   must be done first):
   ```
   ! python3 services/serverlist/discord-setup.py --apply
   ```
   (Or by hand: developer portal → the app → General Information → Interactions Endpoint URL =
   `https://list.lonerstrike.com/discord/interactions`.) Then in the channel's server: Server
   Settings → Integrations → Loner-Strike Setup → `/community`: allow Moderator and Dev if they
   lack Manage Messages (the service checks roles itself either way).
8. **Check it**. Moderation answers only on the box, so open the admin tunnel first (it stays in the
   background; `! pkill -f "8100:127.0.0.1:8100"` closes it):
   `! ssh -f -N -L 8100:127.0.0.1:8100 cs@178.156.241.180`
   then `! python3 services/serverlist/lsmod.py status` (counts, index sha) and in Discord
   `/community status mod:1` (answers "unknown", ephemeral). The first real registration from the
   app posts a card with [Pull].
9. **The first index**: `! python3 services/serverlist/lsmod.py publish` (writes
   `community-index/index.json` + `.sha256`, uploads nothing), look at it, then
   `! python3 services/serverlist/lsmod.py publish --upload` (aws CLI + `~/.config/lonerstrike/r2.env`,
   as for the nav pack). Check:
   `! curl -sI https://dl.lonerstrike.com/community/index.json` → 200, `content-type: application/json`,
   `cache-control: public, max-age=300`.
10. **Optional — publish from the box** (pulls reach apps within ~5 minutes without the Mac; never
    after a held-back re-check): make a separate R2 API token for the bucket (Object Read & Write)
    and put its four values into `publish.env` (the publish token is already there), one at a time
    from your clipboard (`pbpaste` gives what you copied; nothing is shown):
    ```
    ! pbpaste | ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh R2_ACCESS_KEY_ID publish'
    ! pbpaste | ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh R2_SECRET_ACCESS_KEY publish'
    ! pbpaste | ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh R2_ENDPOINT publish'
    ! pbpaste | ssh cs@178.156.241.180 'sudo /opt/lonerstrike-list/set-env.sh R2_BUCKET publish'
    ! ssh cs@178.156.241.180 'sudo snap install aws-cli --classic && sudo systemctl enable --now lonerstrike-community-publish.timer'
    ! ssh cs@178.156.241.180 'sudo systemctl start lonerstrike-community-publish; journalctl -u lonerstrike-community-publish -n 10 --no-pager -o cat'
    ```
    Unverified: the snap aws CLI inside a DynamicUser unit (the unit sets HOME and PATH for it).

**Rollback**: copy the previous `services/serverlist` (main checkout) and run its install.sh; its
unit has no `--community-db`, so the community part stops answering (404) and `community.db` stays
where it is. In Discord, clear the Interactions Endpoint URL.

## 7. Open items

- The tag search finding `Software Used: LonerStrike` is unverified until the first real upload.
- Only `.zip` packages (and lone .bsp/.wad/images) can be looked inside; a creator re-packing as
  `.rar` is refused with a clear message.
- The re-check doesn't read package contents again (a different file means a new MD5, which hides it).
- The app doesn't verify `index.json.sha256` (or a signature) yet.
