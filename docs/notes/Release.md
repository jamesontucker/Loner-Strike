# 1.0.0 release: local dry run of the draft recipe (P5, 2026-11-14)

The recipe itself is in P5.md ("test DRAFT recipe, updated after the public audit"). This is a local
run of it: **nothing was pushed, tagged or released.**

Orphan commit `d70a73d5` (tree `c66e5ce6`), built from tools `389ba573` with the recipe's steps
(internal docs removed, release.yml given the `draft_tag` input). It is only a local object, with no
ref pointing at it.

## Passed
- [x] No parent. Author and committer are both `21000066+jamesontucker@users.noreply.github.com`.
  No Co-Authored-By line.
- [x] None of the left-out docs are in the commit: docs/notes, docs/research, docs/polish*,
  docs/render-compare, docs/TOOLS-*.md. 1471 files remain, and docs/ keeps the 11 public ones.
- [x] No `/Users/jameson` anywhere. The `/Users/` hits are made-up test paths (`/Users/me`, `/Users/x`,
  `C:/Users/Zoë`) and mapsource's sanitizer patterns.
- [x] Value scan (`scratchpad/leakscan/scan.py <commit> <repo>`; it prints only a label and file:line,
  never a value). It checks for:
  - the runbook's IPs and Discord IDs;
  - the R2 env values;
  - the Discord bot token and webhooks;
  - the community admin token;
  - every non-noreply author email in the history and the global git email;
  - every panel_rcon.txt and server.cfg rcon/sv password under the Loner-Strike folders.

  **Nothing matched apart from the items below**: no emails, no rcon or server passwords, and no
  tokens, webhooks or R2 keys. The one "R2 env value" hit is the bucket name, which is public as the
  dl. domain.
- [x] The build reads nothing that was left out:
  - The only `include_str!` of docs are COMPONENTS, LONER-INC, CREATURES, bot-people, MCP and
    GAMEMODES-PLAN, all kept.
  - No build script, vite config, package.json or workflow reads docs/notes or TOOLS-*.
  - Release needs `CHANGELOG.md` with a `## 1.0.0` section (present) and version 1.0.0 in
    tauri.conf.json (present).
  - Every link from the app to a GitHub file points at something kept: CHANGELOG, CREDITS, LICENSE,
    docs/linux-server.md, docs/LONER-INC.md.
- [x] Local Mac build from an export of the commit: it built (see "Local build" below).

## To fix before the real run
1. **Must fix: the box's real IP in `services/serverlist/qa/live.py:36`** (7319babb, the live QA
   harness). The default is `LS_BOX` = `cs@<the box's IP>`. The runbook rule is to keep the address
   out of the public repo, so drop the default and require `LS_BOX`. The same file also hard-codes
   the guild and the owner's Discord user ID (lines 33-34); read those from the environment too.
   *Owner: whoever wrote the live harness.*
2. **gitleaks 8.21.2 finds 3 leaks, all made-up test fixtures.** The recipe needs "no leaks found",
   so add `// gitleaks:allow` at the end of each line. A .gitleaksignore won't work here, because its
   fingerprints include the commit SHA and the orphan commit gets a new one.
   - `app/src-tauri/src/diagnostics.rs:425`: the fake `KEY` (an OpenSSH key header; rule
     private-key).
   - `app/src-tauri/src/webstatus.rs:1247` and `:1272`: the fake tunnel `token`s (rule
     generic-api-key).
   *Owners: the credits/diagnostics test and the website-status tests.* The audit's 4 Discord-ID
   findings in community.env.example are gone.
3. Optional (audit checklist 4): the real Discord IDs in `services/serverlist/test_modbot.py:24`
   (guild, mod channel, Moderator/Dev roles, the owner's user ID) could become made-up ones. The
   guild ID in README.md:7, site/index.html:1086 and discord-setup.py:33 is the public server's, so
   it's fine.
4. Cosmetic: about 30 code comments point at docs/notes/*.md or TOOLS-*.md, which the public repo
   won't have, for example roles.rs, caps.rs and credits.rs. They're harmless, so leave them or trim
   them after 1.0.

## Local build
- What was built: `git archive d70a73d5` was unpacked into the scratchpad, with the empty
  `resources/linux/ls-voice` and `ls-intent` folders that CI fills, and nothing else from the working
  tree. Then `cargo tauri build --bundles app` ran with updater artifacts off, since there's no
  signing key locally.
- `npm ci && npm run build` and the release compile (6 m 48 s) both passed with no errors. The only
  warnings are 3 dead-code ones from other agents' uncommitted work that was in the commit.
- The result is Loner-Strike.app, version 1.0.0 (63 MB), with catalog, mods, plugins, linux and
  windows inside. Its binary has no repo or home paths except the usual
  `~/.cargo/registry`/`~/.rustup` panic locations of a local build. CI's builds carry the runner's
  paths instead.
- Not covered locally: Windows, the universal Mac build, signing and notarization, and the
  playitd/ls-voice/ls-intent sidecars. These are CI-only, and the 2026-11-10 tag run built them.
- The build and the export were deleted afterwards. Free disk stayed at 10 GB or more.
