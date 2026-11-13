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

## Format checks (server only, 2026-11-12)
- Every registration's file is downloaded (<= 128 MB, MD5 must match the page) and each file in it
  checked by formats.py before listing; one at a time, niced, 180 s; results kept in the entry's
  item JSON as "checks" (not exported to the index) and shown on the Discord card.
- Corpus proof (corpus_check.py --hostile): 21,574 real files, 4 refused, all genuinely unusable
  (cs_ship.nav v9 Source nav; doublekill.wav/unstoppable.wav are MP3s; HLDS valve/models/doctor.mdl
  v9). 101,001 damaged variants: 100,942 refused, 0 crashes (the 59 are cuts in data the game
  ignores). Details in COMMUNITY-SERVER.md "Format checks".
- Relaxed after real files: BSP 29 accepted; lightofs/visofs == lump length; clip-node head ==
  count; quote-aware entity braces; only mip 0 of textures (other offsets are junk in real wads);
  model palette up to 768 bytes short and length off by a few bytes; float WAVs; trailing WAV
  chunks unchecked; MP3 zero padding; UTF-16 text; JSON lines; nav data after the areas.
- App-side install-time checks: NOT built (user: risk of false rejects on old GameBanana content).

## Tests
`cd services/serverlist && python3 -m unittest test_formats test_community test_modbot test_serverlist test_publish_check`
(125). App: `cd app/src-tauri && cargo test tools::community`.

## Open
- App Publish's refusal check is still a blocklist (the server's allow-list decides what's listed).
- A signed index (key pinned in the app); today the app checks index.json.sha256, which only
  catches damage.
- The workflow and the tunnel: Cloudflare Bot Fight Mode / WAF may challenge GitHub's runners on
  /community/publish/ (skip rule in COMMUNITY-SERVER.md step 6).

## Plan: a signed index (no code yet)
Goal: an app believes community/index.json only if it carries a valid signature by our key, so a
tampered file on R2 / the CDN (or a stolen R2 key) can't list or unpull anything.

- **Signature file**: `community/index.json.sig` beside the index: Ed25519 over the exact bytes of
  index.json (64 bytes, hex or base64 in a small text file with a key id line, e.g.
  `ls-community-1 <base64 sig>`), so the key can be rotated: the app knows key ids.
- **Who signs**: the publish workflow (community-publish.yml), after its checks and before upload:
  `python3 scripts/community-index-sign.py index.json > index.json.sig` (Ed25519 via the runner's
  python3-cryptography / `pip install cryptography` pinned by hash, or openssl 3 `pkeyutl -sign
  -rawin`). Upload order: index.json.sig first, then index.json, then .sha256 (an app that reads a
  new index with the old signature refuses it and keeps its cached copy for those seconds).
- **Where the private key lives**: a GitHub Actions secret `COMMUNITY_INDEX_SIGNING_KEY` (the raw
  32-byte seed, base64) used only by that workflow, in a GitHub **environment** ("community-publish")
  with a deployment branch rule (main only), so other workflows and PR runs can't read it. Never on
  the box, never in R2, never in the repo. An offline backup copy in the user's password manager.
  The list service and the box never see it (the box only builds the unsigned index).
- **The app**: the public key(s) baked into community.rs (`const INDEX_KEYS: [(&str, [u8; 32])]`);
  `index()` downloads index.json + .sig, verifies with the `ed25519-dalek` crate (or `ring`), and on
  a missing/bad signature treats it like an unreachable index: keeps the cached copy (which was
  verified when it was saved) and the page says "from earlier". The sha256 file stays for humans.
- **Rollout**: (1) add the signing step and publish .sig for a while; (2) ship an app that verifies
  but only warns when the sig is missing (old indexes); (3) next release refuses unsigned. Key
  rotation: ship the new public key in an app release first, then switch the secret.
- **Lost or leaked key**: rotate (new key in an app release, old key id removed in the following
  one); a leaked key alone can't publish (R2 keys are separate) but could sign a tampered file
  someone else uploads, so rotate promptly.
