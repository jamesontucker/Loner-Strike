# Archive extraction on Windows (review, 2026-11-13)

Scope: `app/src-tauri/src/archive.rs` after the Caps commits (0ef33d0, 36d422a), and its callers
(gamebanana.rs map/content installs and local files, sounds.rs, tools/community.rs).

## Earlier Windows fixes, all still in place
- b02e06e (merged in ad08461, replacing the shipped UnRAR of b032147/aa4718a): 7-Zip unpacks what
  Windows 10's tar.exe can only list (RAR with filters); its listing refuses links and over-budget
  archives first; dest is cleared before 7-Zip runs; links are stripped after; 7-Zip errors are
  reported. Bundled copy: `resources/windows/7zip/` (7-Zip 26.05 x64) -> `<exe dir>\7zip\7z.exe`.
- tar is `%SystemRoot%\System32\tar.exe` (bsdtar), then PATH; the zip crate reads .zip first.
- Caps: the 7-Zip listing's entry count goes to bomb_check; the watcher stops any unpacker past
  65,536 files or the size budget.

## Checked with the real 7-Zip (bundled 26.05 run under CrossOver's Wine)
- `7z l -slt`: the archive's own block ("--", "Path = <archive>", Type...) comes before the
  "----------" line, entries after; CRLF lines; warnings ("There are data after the end of
  archive") sit in the header block and exit 0. Names print in the OEM code page when piped
  (lossy-decoded; only the ASCII keys are read). Samples are in the tests (.7z, .rar, .zip).
- 7-Zip **does** create symbolic links on extraction by default (zip Unix links and tar links:
  "ERROR: Cannot create symbolic link" under Wine, which can't make them; real Windows can with
  Developer Mode or admin). The old comment said it didn't. A zip's Unix link has no
  "Symbolic Link =" line in the listing, only "Attributes =  lrwxrwxrwx".
- With `-snl- -snh- -sns-` link entries are skipped (exit 0) and ':' becomes '_'.
- Paths stay inside `-o`: `..` dropped, `C:\abs` -> `C_\abs`, `\\server\share` -> `server\share`,
  `CON`/`aux.txt`/`NUL.bsp` -> `_CON`/`_aux.txt`/`_NUL.bsp`, `x.bsp. ` -> `x.bsp__`.
- `-o` with spaces and non-ASCII (ü, 日本) and archive names with them work; unicode entry
  names are kept on disk.

## Fixed (this commit)
- 7-Zip runs with `-snl- -snh- -sns-` (no links, no alternate streams - those would be invisible
  to the size watcher); `-o` is built as an OsString (was `display()`, lossy).
- The 7-Zip listing also counts a Unix mode starting with `l` (zip/tar) and "Copy Link" as links.
- tar's listing showing links is refused before 7-Zip/unar run (7-Zip could unpack a zip whose
  links its own listing doesn't show).
- The bundled 7-Zip is used before an installed one (an old install may not take -bso0).
- 7-Zip errors show its last ERROR line ("7-Zip: Open ERROR: Cannot open the file as [7z] archive").
- The watcher checks the file count once more after the unpacker exits (one done in < 0.5 s).
- An inner archive that fails is removed (no half-unpacked inner archive gets installed).
- A leading drive in a listed path ("C:\maps\x.bsp") is dropped; any part with ':' is skipped.

## Type check
`cargo check --target x86_64-pc-windows-msvc` stops in ring's build script (no MSVC headers on
the Mac: `assert.h`), so the crate was checked with `--target x86_64-pc-windows-gnu
--all-targets` (same cfg(windows) code, no target_env cfgs in the crate): clean.

## Still needs a real Windows run
- Windows' own tar.exe (libarchive) on the same samples; Developer Mode on, to see links really
  skipped; the watcher stopping 7z.exe mid-run (TerminateProcess); long paths over 260.
- Single file size: there's no RLIMIT_FSIZE on Windows, so one big file can overshoot the budget
  by what's written in 0.5 s before the watcher stops it (then refused and removed).
