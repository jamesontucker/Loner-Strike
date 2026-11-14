# Public exposure audit: Loner-Strike and the ReGameDLL fork (2026-11-14)

Read-only audit. Nothing public was changed: no push, no deletes, no settings changes.
Secret values are never quoted here; hints show at most 4 characters.

## Scope covered

| Surface | What was scanned |
|---|---|
| `origin/main` tree (171e4b63) | gitleaks + regex scan (IPs, emails, home paths, ssh users, key/token shapes, phones, SteamIDs, cfg passwords) |
| Public history | all 527 commits reachable from `origin/main`; tags v0.1.0, v0.1.1, v0.1.2 (all inside main); `ci-dry-1.0` (03148aa4, 1 commit; branch now deleted but the commit is **still served by SHA**); `refs/pull/1..5/head` (Dependabot workflow bumps, public forever) |
| Releases | v0.1.0 to v0.1.2: 8 assets each; latest.json, .sig files and release bodies read |
| Actions | 3 workflow files; logs of all 22 runs downloaded and scanned (gitleaks + regex); 12 unexpired artifacts listed |
| Repo settings | visibility, wiki, issues, PRs, Pages, secret scanning |
| Fork `jamesontucker/ReGameDLL_CS` | 30 branches + tags; the 481 commits not in upstream `rehlds/ReGameDLL_CS` scanned (gitleaks + regex); releases and runs listed |
| 1.0 merge preview | `git diff origin/main..tools` excluding docs/notes, plus the tools history (`origin/main..tools`, 799 commits) |
| Box reachability | polite TCP connects / one A2S query / SSH auth-method probe to 178.156.241.180 (no login attempted) |

Tools: gitleaks 8.21.2 (binary in the session scratchpad, not installed system-wide) and a custom regex scanner.

## Bottom line

- **No live secret was found anywhere public**: not in the tree, history, PR refs, tags, release assets, run logs or the fork. **Nothing needs rotating.**
- GitHub secret scanning is on for both repos with 0 alerts; push protection is on.
- The real exposures are **personal data in commit metadata** and **what the 1.0 merge would publish if it's not done carefully**.

## Findings, ranked

### 1. MEDIUM: personal email in commit metadata (history, both repos)
- **Where:** author and committer of 515 of 527 commits on `main` (and therefore tags v0.1.x), and 458 commits across 15 `ls-*` branches plus `loner-strike` in the fork. The `tools` branch has 799 more (all of them).
- **What:** `jame***@gmail.com`. Anyone can read it with `git log` or `https://github.com/.../commit/<sha>.patch`.
- **Live secret?** No. Personal data, can't be rotated.
- **Fix:**
  - Now: `git config --global user.email 21000066+jamesontucker@users.noreply.github.com` (and in each repo/worktree); on GitHub, Settings > Emails: "Keep my email address private" and "Block command line pushes that expose my email".
  - 1.0 merge: squash-merge (see 2) with the noreply address, or rewrite only the unpushed `tools` commits with `git filter-repo --mailmap` before merging. Cheap, because nothing in `tools` is public yet.
  - Existing history: `git filter-repo --mailmap` on main and the fork, then force-push. Trade-offs: every SHA changes; tags v0.1.0 to v0.1.2 must be re-pointed (releases follow the tag, the updater's latest.json uses asset URLs so it keeps working); the fork's release tags and the app's pinned fork version (5.30.0.814-lsNN tags) need re-pointing too; anyone's clone diverges; the old commits stay reachable by SHA on GitHub until GitHub Support runs a GC, and copies already scraped (GH Archive, Software Heritage, mirrors) stay. The email has been public since 2026-11-06, so a rewrite reduces casual discovery but doesn't un-leak it. Owner's call; recommended minimum is the "now" and "1.0 merge" steps.

### 2. MEDIUM: internal documents and history the 1.0 merge would publish
The tree diff `origin/main..tools` (docs/notes excluded) still adds:
- `docs/TOOLS-CREW.md` (crew/agent brief: worktree rules, local paths), `docs/TOOLS-NOTES.md`, `docs/TOOLS-PLAN.md`, `docs/TOOLS-DEV.md`, `docs/TOOLS-EDITOR.md`, `docs/TOOLS-RENDER.md`, `docs/polish/` before/after screenshots. The ci-dry build left these out; the plain diff doesn't.
- `docs/GAMEMODES-PLAN.md` lines 8-9: `/Users/jameson/...` and the Nextcloud path (the ci-dry build did include this one).

If `tools` is merged **with its history** (a normal merge, not a squash), excluding docs/notes from the final tree is not enough:
- 46 docs/notes files touched in 309 commits become public history, including `ProdServer.md` (production restart investigation), `Legal.md`, `LegalReview.md`, `QA4.md`, review notes (BRmapReview*, StudioUXReview, PerfAudit), `P5.md`. The scan found no secrets in them, but they are internal.
- 799 commit messages, some internal ("Crew rules: reminders after today's slips ...", 9a521746).
- 799 more commits carrying the personal email (finding 1).
- **Live secret?** No.
- **Fix (recommended):** ship 1.0 as one squash commit built like ci-dry (tree minus docs/notes and docs/TOOLS-*, docs/polish*), authored with the noreply email; keep `tools` local/private for its history. Alternative if history matters: `git filter-repo --path docs/notes --path-glob 'docs/TOOLS-*' --path docs/polish --invert-paths --mailmap ...` on a copy of `tools`, then review commit messages before merging.

### 3. LOW: home-directory paths
- **Tree on tools (would ship at 1.0):** `app/src-tauri/src/tools/health.rs:2038` (`LS_TEST_SIM=/Users/jameson/loner-strike-sim`), `docs/GAMEMODES-PLAN.md:8-9`, and (if not excluded) `docs/TOOLS-CREW.md:57-63`, `docs/TOOLS-NOTES.md:237,457,630,1038`, `docs/TOOLS-PLAN.md:5-6`.
- **Current main tree/history:** none (only the dangling ci-dry commit 03148aa4 has the first two).
- **Fix:** replace with placeholders (`LS_TEST_SIM=<loner-strike-sim folder>`, `REPO`/`FORK` described generically) before the merge. No rewrite needed.

### 4. LOW: dangling ci-dry commit still served
- `03148aa4` ("CI dry build of 1.0.0 ...") is no longer on any branch but GitHub still serves it by SHA (and its cancelled run 43591429991 links to it). Content is what 1.0 will publish anyway (minus TOOLS docs); no secrets. It carries findings 3, 5-7 in their tools form.
- **Fix:** none required. If wanted, GitHub Support can purge unreachable objects; not worth it here.

### 5. LOW (accepted by owner): Hetzner box IP 178.156.241.180
- **Main tree:** `app/src-tauri/src/hltv.rs:923,926`; `app/src-tauri/src/servers.rs:4057-4058,4100-4102` (test fixtures, since b567bcfd).
- **History only:** `site/index.html` (added 4824b9e4, replaced by `play.lonerstrike.com` in a77b8cd3).
- **Not on tools:** the tools branch already swaps these for 203.0.113.10, so the 1.0 tree drops it.
- `play.lonerstrike.com` resolves straight to this IP (UDP game traffic can't go through Cloudflare), so it's public regardless. No rewrite.

### 6. LOW: Discord IDs in `services/serverlist/community.env.example`
- **Where:** tools tip and ci-dry 03148aa4, lines 22-30: guild, channel, role IDs (`1568***`, `1570***`) and the owner's Discord user ID (`3106***`).
- **Live secret?** No: snowflake IDs aren't credentials (gitleaks' `discord-client-id` hit is a false positive). The owner ID links the project to a personal Discord account.
- **Fix (optional):** leave these blank in the example, keep real values only in `/etc/lonerstrike-list/community.env`.

### 7. LOW: probable personal SteamID in a test fixture
- `app/src-tauri/src/community.rs:880,882` (main tree; 882/884 on tools): `STEAM_0:0:479***` named "nano". If it's the owner's account it ties the project to that Steam profile.
- **Fix (optional):** swap for `STEAM_0:1:12345` like the other fixtures. History rewrite not worth it.

### 8. INFO: infrastructure described in docs (no values)
- `docs/COMMUNITY-SERVER.md` (tools) names the list service on 127.0.0.1:8100 behind a Cloudflare tunnel, `/etc/lonerstrike-list/community.env`, `~/.config/loner-discord-token`, `~/.config/lonerstrike/community-admin.token`, the token model and lockouts. `docs/linux-server.md` (main) uses a generic `/home/cs` example. This is design documentation without secrets; security doesn't depend on hiding it. Optional: drop the Mac-local token file paths.

### 9. INFO: false positives checked
- `app/src-tauri/src/diagnostics.rs:420-425` (main and tools): `RCON`, `JOIN`, `HLTV_PW`, `TUNNEL`, `PHONE`, `HOME_IP` (178.62.14.201) and a truncated 171-char fake OpenSSH key are redaction-test fixtures.
- `app/src-tauri/src/webstatus.rs:943,968` (main) / `1247,1272` (tools): a dummy tunnel token in tests (`eyJh***`, decodes to placeholder JSON).
- `compile.rs`, `editor/assets.rs`: "BEGIN OPENSSH PRIVATE KEY" appears as a detection pattern, not a key.
- services/serverlist tests: `"t" * 40`, `nope***`, `shor***`, `not-***` test tokens.
- Other IPs are RFC 5737 documentation ranges, 1.1.1.1/8.8.8.8, MaxMind test IPs, version numbers, or the UI placeholder 146.235.20.10.
- Emails in third-party AMXX plugin headers (gore, shero) are original authors' credits.
- `cfg` files: no real `rcon_password`/`sv_password` values in main or tools.

## GitHub-side items

- **Releases:** assets are the installers, `.sig` files and `latest.json`. The signatures are minisign *signatures* (public by design), not the private key. Release bodies are clean. Binaries were built on GitHub runners (paths are `/Users/runner/...`).
- **Workflows:** all secrets are referenced as `secrets.*` (Apple signing, Tauri signing key, R2 keys, Polar token, community publish token). No `pull_request`/`pull_request_target` triggers, so fork PRs can't reach secrets. The publish token is passed to curl via a 0600 file, not argv.
- **Run logs (22 runs, public on a public repo):** every secret appears as `***`; gitleaks found nothing; no R2 endpoint/account ID, IP or email leaked. Polar's 403 error printed only the masked benefit ID.
- **Artifacts:** 12 unexpired (ls-control, ls-voice-linux, two early app builds), downloadable by any signed-in user; all built from public source. Fine.
- **Settings:** public; issues on (0 issues); 5 closed Dependabot PRs (workflow version bumps only); wiki enabled but empty; Pages off; secret scanning + push protection on; "non-provider patterns" and "validity checks" off. Suggest turning on non-provider patterns, and disabling the wiki if unused.
- **Missing tags:** runs exist for `v0.1.3` and `v1.0.0` tag pushes, but those tags are gone; their commits (f2cf5585, 890bbc39) are on main, so nothing extra is exposed.

## The ReGameDLL fork

- Public fork of rehlds/ReGameDLL_CS; 30 branches (15 `ls-*` plus `loner-strike`), releases 5.30.0.814-ls18 to -ls20.
- 481 non-upstream commits: gitleaks clean; no IPs, home paths, ssh hosts, tokens or internal docs; only CHANGELOG/README/LICENSE-TRANSITION outside source.
- Only issue: the personal email on 458 commits (finding 1).

## Box reachability (178.156.241.180, bypassing Cloudflare)

| Port | Result |
|---|---|
| TCP 22 | open: `OpenSSH_9.6p1 Ubuntu-3ubuntu13.20`. `cs@` and `root@`: "Permission denied (publickey)", so **password auth is off**. Root is still offered publickey; set `PermitRootLogin no` if root logins aren't needed. |
| UDP 27015 | answers A2S_INFO (challenge reply): the game server, expected |
| UDP 27016-27020 | no reply (HLTV/others not answering) |
| TCP 80, 443, 8080, 8090 (web status), 8100 (list service), 9090 (ls-control), 27015-27020, 27099 | closed/filtered |
| HTTP/HTTPS direct | no response |

No web service is reachable around Cloudflare.

## Before the 1.0 merge: checklist

1. Set git user.email to the noreply address everywhere.
2. Merge as a squash commit (ci-dry style), excluding docs/notes, docs/TOOLS-*, docs/polish*.
3. Replace `/Users/jameson` paths in `health.rs:2038` and `GAMEMODES-PLAN.md:8-9`.
4. Optionally blank the Discord IDs in `community.env.example` and swap the "nano" SteamID fixture.
5. Re-run gitleaks on the squash commit before pushing.
