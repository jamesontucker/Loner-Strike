# Compatibility across 1.x: editor projects, shared creations, compiled modes (W2)

Sibling of docs/notes/StudioCompat.md (X: Studio graphs). Same goal: a 1.x update never breaks
anyone's work, and a file from a newer app is refused whole with the version to update to.

## The rules (every format here)

1. **A format version is written and checked.** A file without one is the 1.0 format (files made
   before versions were written stay valid).
2. **Newer is refused whole, never half-loaded.** The message names what it is and the app
   version to update to: `compat::newer(what, saved_by)` (app/src-tauri/src/compat.rs). Files
   carry the app version that wrote them (`saved_by` / `app` / `minApp`) so the message can say
   "Update Loner-Strike to 1.4.2 or newer". Without it: "update to the newest version".
3. **Names never change.** Field names, keys, pin ids, native and forward names and signatures,
   cvar and command names stay as 1.0 shipped them.
4. **New fields are optional with defaults** (`#[serde(default)]`, a missing key means the old
   behavior). Readers ignore fields they don't know. A change an older app would get wrong (not
   merely miss) raises the format version.
5. **Removed things are deprecated and kept**: still read (and mapped to the new thing on load,
   like graph.rs `proj_spawn_pins` / `vol_spawn_pins`), still declared (natives), never reused
   for something else.
6. **The 1.0 samples are permanent** (app/src-tauri/tests/compat/1.0). Tests load or install them
   in every 1.x; never edit them to make a test pass. A new format version adds its own folder
   (tests/compat/1.1/...) next to them.

## 1. Map editor projects

| Format | Version | Before this pass | Added |
|---|---|---|---|
| Map project (`MapDocument`, editor/doc.rs; the project's `.json` and its autosave, `_edit` saves) | `version` = `DOC_VERSION` 1 | written and checked; newer refused ("update the app"), version 0 refused | `saved_by` written on save; the refusal names it |
| Prefab (`PrefabMeta.format`, editor/prefab.rs) | `PREFAB_FORMAT` 1 | written and checked | `saved_by`; the refusal names it |

Fixture: `editor/project.json` (a floor, a func_door brush in a group, info_player_start, WADs,
sky). Tests: it opens with everything there and saves again in the current format; a newer
version is refused with "Update Loner-Strike to 1.7.0 or newer" (and the general advice without
`saved_by`).

## 2. Shared creations (Community, Publish)

| Format | Version | Before | Added |
|---|---|---|---|
| Community index (tools/community.rs `Index`) | `schema` 1 | checked; newer: "update" (and the kept copy stays) | optional `minApp` (the list writes the oldest app that reads its format); the message names it. An unknown item `kind` is dropped, not an error (serde `other`) |
| Package manifest `loner-strike.json` | `schema` + `app` | written; read by the list service, not the app's installer | nothing (the service keeps it) |
| Mode map kit (Studio, `loner-strike/mode-kit`, tools/studio/kit.rs) | `version` 1 | written; the check refused anything but exactly 1 (a missing version too) | missing = 1; only newer refused, naming the kit's `app` (now written) |
| Map kit (editor, tools/editor/kits.rs) | `version` 1 | checked; missing refused | missing = 1; newer names `app` |
| Creature pack (`creatures/creatures.ini` in a package, tools/creaturepack.rs) | none | no version | `; format: 1 (Loner-Strike x.y.z)` written; a bigger format is refused whole, naming the version (the list's checker strips `;` comments) |
| Mode graphs, snippets, libraries | | X: StudioCompat.md | |

Fixtures: `community/index.json` (a map, a mode, a creature), `community/mode-kit.json` (from the
markers sample), `community/creatures.ini`. Tests: the index reads all three and a newer schema is
refused; the kit installs and a newer one names its app version; the pack installs into
creatures.ini and a format-2 pack is refused.

## 3. Compiled modes and the server

| Part | Before | Added |
|---|---|---|
| Natives and forwards a generated or hand-written plugin calls (loner.inc, ls_components.inc) | `LS_COMPONENTS_VERSION`, `ls_components_version` cvar; a missing native silently did nothing (loner.inc's native filter) | **Snapshot** `server/plugin_api.txt` (269 declarations); a test fails if any 1.0 native or forward is gone or its signature changed. At run time, a missing `ls_*` native is now logged once per native: "<plugin> needs a newer gm_components than this server's (version N has no X): update the server's Loner-Strike files..." (or "isn't running") |
| The bot brain (`ls_mode_ver`) | requirements.rs knew versions 1-2 (`modeVer` in a mode's needs; `brain_here` read mp.dll) | versions 3 (use offers) and 4 (Bots Avoid Area, pickup offers) in `FEATURES` and `brain_here`, so a mode using them says it needs a newer brain before install. gm_components already holds those hints back from an older brain (pickup offers below 4, the free "box" instead of "use" below 3) |
| gm_* plugins' cvars and commands generated modes use | | rule 3; not snapshotted (Studio's codegen tests cover what it writes) |

Fixture: `server/mode_avoid.graph.json` (needs `modeVer` 4). Regenerating the API snapshot is a
1.0-only step (`LS_COMPAT_SNAPSHOT=1 cargo test compat_write -- --ignored`); after 1.0 the file
only grows by hand when a format version adds a new frozen sample folder.
