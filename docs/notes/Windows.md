# Windows audit (agent MCP, 2026-11-14)

Nearly all testing so far ran on a Mac with CrossOver; most users will be on Windows. This is a code pass (no Windows
machine) over everything that behaves differently there: paths, Steam detection, setup and repair, the Ham Sandwich
fix, local servers, the updater, file locks, the voice agent, MCP's stdio bridge, WebView2 in the editor and the
Studio, and installer leftovers. Fixed items have tests that run on any OS. What needs a real Windows machine is the
"H. Windows" area of docs/notes/QA4.md, with exact steps.

## Fixed (this commit)
| # | Where | What went wrong on Windows | Fix | Test |
|---|---|---|---|---|
| 1 | sys.rs `rename_over` | Only "PermissionDenied" was retried; a file in use (error 32, an antivirus or OneDrive scan) is "uncategorized" in Rust, so saves failed at once | Retry on errors 5, 32 and 33 too (`transient_lock`) | `lock_errors_that_clear_by_themselves` |
| 2 | setup/record.rs `copy2` | Copying over a read-only file fails (CopyFileEx), and the copy took the source's read-only flag (players mark config.cfg read-only; archives keep the flag) | Clear the flag before and after the copy | `read_only_files_can_be_made_writable` (sys.rs) |
| 3 | hamguard.rs / app_api.rs | Play while the game is open: a loaded DLL can't be replaced, so the fix failed silently after retries, on the async thread | With the game open, say so in a quiet note ("goes back in the next time the game starts…"); the work runs off the async thread | hamguard tests |
| 4 | sys.rs `same_game_dir` | Case compared for ASCII only and "/" vs "\\" not normalized: `D:\Игры\Steam` vs `d:\игры\steam` read as "a different copy of the game is running" | `same_windows_path`: case-blind for every letter, slashes alike, `\\?\` dropped | `windows_paths_compare_like_windows` |
| 5 | steam.rs `libraries` | Older libraryfolders.vdf files (`"1" "D:\\Lib"`) were missed: games in those libraries weren't found | `library_paths` reads both formats | `both_libraryfolders_formats` |
| 6 | setup/record.rs `inside` | Setup paths could name CON/NUL/COM1, hold ":" (an NTFS stream) or end in a dot or space | Refused (`windows_unsafe`) | setup tests |
| 7 | mcp/conn.rs | The Claude Code terminal line had the app's path JSON-escaped (`C:\\Users\\…`): cmd and PowerShell keep the doubled backslashes | Plain double quotes on Windows, single quotes elsewhere (`shell_arg`) | `the_terminal_line_has_the_path_as_typed`, `setup_commands` |
| 8 | app_api.rs Remove everything | It deleted `%LOCALAPPDATA%\gg.lonerstrike.app` whole, including the open window's WebView2 data (`EBWebView`, locked): always a failure list | Leaves `EBWebView` for the uninstaller | `the_windows_webview_folder_is_left_for_the_uninstaller` |
| 9 | ui windowsGuards.ts (main.tsx) | WebView2 acts like a browser: unhandled F5/Ctrl+R reload the app (in the editor's fly mode, in text fields, Plugin Studio, Spray), Ctrl+P prints, Ctrl+F/F3 find bar, F7 caret browsing; a file dropped from Explorer replaces the app; right-click shows Back/Refresh/Print | Guards that run last: such keys, outside-file drops and the browser menu (except in text and selections) are stopped when nothing in the app handled them | scripts/test-windows.mjs |
| 10 | Play.tsx | (from the Ham work) the note is shown | quiet toast | — |

## Fixed, round 2 (2026-11-14)
The Windows-only code (procmatch's Win32 calls, SetErrorMode) was type-checked against the x86_64-pc-windows-msvc
target in a scratch crate (the app itself can't be checked for Windows on a Mac: ring's C build).
| # | Where | What went wrong on Windows | Fix | Test |
|---|---|---|---|---|
| 11 | sys.rs `quiet_crash_dialogs`, main.rs | A crashed hlds.exe sat behind "has stopped working" until clicked (the watcher killed it after 120 + 90 s) | `SetErrorMode(SEM_FAILCRITICALERRORS \| SEM_NOGPFAULTERRORBOX \| SEM_NOOPENFILEERRORBOX)` at app start; every child inherits it (no launch uses CREATE_DEFAULT_ERROR_MODE). Valve's own fatal MessageBox still shows | Windows PC (QA4 H) |
| 12 | setup/mod.rs `run_step`, record.rs `couldnt` | The running-game check ran once, before minutes of downloads; a game started meanwhile broke the install with "os error 32" | Every step checks first; while the game runs it pauses with "Close Counter-Strike to continue. Setup goes on by itself once it's closed." (15 minutes, then "…still open. Close it, then click Retry"); error 32/33 says to close the game and Retry | `setup_pauses_while_the_game_runs` |
| 13 | procmatch.rs, servers.rs `end_leftover` | A leftover server was ended by pid (only "hlds.exe" in the name checked): Windows reuses pids, so another server or program could be ended | The pid file holds the start time; a leftover is ended only when the pid, its full executable path and its start time (±15 s) all match (Windows: QueryFullProcessImageNameW + GetProcessTimes; Mac: ps etime + command) | `ends_only_the_process_it_started` (a real child), pid file, start-time and etime tests |
| 14 | servers.rs start, sys.rs `udp_port_free` | Only the app's own servers were compared; Windows HLDS quietly took the next port and RCON then failed in a restart loop | UDP test-bind on all addresses and on the server's own (or 127.0.0.1) before starting: "Port … is in use by another program on this computer…" | `a_port_another_program_holds_isnt_free` |
| 15 | tauri.conf.json, release.yml | The MSI was built too: NSIS hooks don't run for it, it installs per machine, and an update could leave two installs | **NSIS only** (the user's decision): `bundle.targets` without "msi", the manual-run artifacts without it. The mirror and Polar already took only `*_x64-setup.exe`; the site and docs never linked an MSI. latest.json's Windows entry is now the setup.exe; 0.1.2's NSIS installs (currentUser, passive updates) update in place | release build (not run here) |
| 17 | main.rs, sys.rs | The startup update dialog could update during a setup or restore (cutting steamcmd off); WebView2 asked before a page read the clipboard (editor/Studio paste from other programs); a read-only file couldn't be replaced by a save | The dialog checks the same rule as Settings' Install (and says "Update later" when busy); both windows get `enable_clipboard_access()`; `rename_over` clears the read-only flag first on Windows | settings rule shared; Windows PC for the clipboard |
| 16 | site/docs.html, Servers.tsx | Firewall: the user keeps Defender's own prompt (their decision, no elevated rule) | "Friends can't join" explains the prompt per server and how to fix a Cancel (Windows Security › Allow an app through firewall › hlds.exe, Private and Public); "A server won't start" names the new port message | — |

## Decided by the user (2026-11-14)
- Firewall: keep Defender's prompt; a plain hint where it helps (done, #16).
- Installer: NSIS only (done, #15). Not pushed, tagged or released.

## Open: needs a decision or a Windows machine
High:
- **WebView2 accelerator keys at the source**: also turn them off in Rust (`ICoreWebView2Settings3::
  SetAreBrowserAcceleratorKeysEnabled(false)` via `with_webview`, needs the webview2-com crate), and add
  `.on_navigation` so the window never leaves the app's own address. The page guard (#9) covers the keys today.
Medium:
- **Record keys case-sensitive** (setup/record.rs): two components shipping `Sound/x.wav` and `sound/x.wav` make two
  entries and the second backs up the first's file as the player's. Look keys up case-blind.
- **Alt key** in the editor (snap off while held, Alt+right-click wrap, Alt-drag): Windows may enter menu mode or
  ding; if so, swallow SC_KEYMENU in a window subclass.
- **Ctrl+1-9** in the editor are camera bookmarks, so on Windows they can't switch app sections (⌘1-9 on the Mac).
- **Fonts** (fixed): Barlow Semi Condensed and Big Shoulders Stencil ship in `app/ui/public/fonts` (OFL, `fonts.css`),
  so the app never asks Google Fonts and looks the same offline; Bahnschrift SemiCondensed is in the text stack and
  Consolas in every monospace stack (`ui-monospace` isn't on Windows Chromium).
- **Roaming AppData**: `app_dir()` is `%APPDATA%` (Roaming) and holds steamcmd, voice and chat models (GBs). Move big
  data to `%LOCALAPPDATA%`.
Low: `game_running` fails open when tasklist is slow; HLTV has no pid file; the uninstaller's `taskkill /IM
playitd.exe` hits a user's own playit; uninstall leaves firewall rules, game-folder mods, `%SystemDrive%\Loner-Strike
\Servers` and `%USERPROFILE%\.loner-strike\downloads`; ⌘ shown in some Studio labels; Win key and AltGr count as
shortcut modifiers (AltGr+8/9 on German/French layouts); Ctrl+Space toggles CJK input; the spray font list reads every
font file whole (slow first open); the wheel ignores `deltaMode`; pointer lock's Esc.

## Checked and fine
No `wmic` anywhere (tasklist/taskkill/PowerShell); `--mcp-stdio` runs before Tauri and gets piped handles in a
GUI-subsystem exe; the single-instance plugin is safe with restarts; `sys::canonical` strips `\\?\` and every
canonicalize + starts_with check canonicalizes both sides; UNC game folders; registry SteamPath and WOW6432Node;
7-Zip with `-sns-`; archive listings drop drive letters; long paths; the voice agent (no microphone in the UI, no
console windows, `ls-voice.exe` and `ls-intent.exe` ended with `taskkill /T`, whisper.cpp opens models with wide paths).
