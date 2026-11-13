# Community server notes (agent: community server)

Design: docs/COMMUNITY.md (app side) and docs/COMMUNITY-SERVER.md (server side, go-live steps).

## State (2026-11-12)
- Live on the box (installed by the coordinator from `tools`): list service with `--community-db`,
  Discord interactions endpoint set, first empty index on dl.lonerstrike.com/community/.
- Publishing: GitHub Actions `.github/workflows/community-publish.yml` every 10 minutes (and by
  hand), from `GET https://list.lonerstrike.com/community/publish/index` with the publish token, to
  R2 with the repo's existing R2 secrets. The box never holds R2 keys (the bucket also holds app
  updates); install.sh removes the earlier box timer and publish.env, set-env.sh refuses R2_/AWS_ names.
- Needs `main` for the schedule (scheduled workflows run from the default branch only).

## Rules worth remembering
- A registration names only a page (+ file id, a hint). Everything listed comes from GameBanana
  (title, author, pictures, file, MD5) and the package's own loner-strike.json and file list.
  The server picks the newest file that passes; never older than the listed one (moderators:
  `lsmod.py use-file`).
- Package allow-list per kind mirrors the app's installer (archive.rs content_exts, content.rs
  allowed). Game Mode Studio mode paths are provisional until the Studio's package format exists.
- A creator's action never undoes a moderator's (hidden_by team/recheck; table in community.py).
- Re-check guard: >3, or >=2 and >25%, leaving at once -> held back; the workflow then doesn't
  publish until `lsmod.py release-hold`. The workflow has the same rule against the live index
  (override: Run workflow with force).
- Health numbers aren't listed (unproven) until we run checks ourselves.

## Tests
`cd services/serverlist && python3 -m unittest test_community test_modbot test_serverlist test_publish_check`
(113). App: `cd app/src-tauri && cargo test tools::community`.

## Open
- App Publish's refusal check is still a blocklist (the server's allow-list decides what's listed).
- A signed index (key pinned in the app); today the app checks index.json.sha256, which only
  catches damage.
- The workflow and the tunnel: Cloudflare Bot Fight Mode / WAF may challenge GitHub's runners on
  /community/publish/ (skip rule in COMMUNITY-SERVER.md step 6).
