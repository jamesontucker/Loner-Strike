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

## Open: needs a decision or a Windows machine
High:
- **Firewall** (servers.rs start, hltv.rs, app_api phone access, control): each server's own hlds.exe/hltv.exe gets a
  Defender prompt; Cancel or a "Public" Wi-Fi network quietly blocks friends; nothing explains it, nothing cleans the
  rules. Proposal: a note before a server's first start and one elevated `netsh advfirewall firewall add rule
  name="Loner-Strike <id>" dir=in action=allow program=<hlds.exe> profile=private,public` (+ hltv.exe); removed by
  name in Remove everything and the uninstaller. Needs the user's OK (an elevation prompt).
- **Crash dialogs keep a dead hlds.exe alive** (servers.rs crash watch): "has stopped working" and Valve's fatal
  MessageBox hold the process until clicked; the watcher kills it after 120 + 90 s. Proposal: `SetErrorMode(...
  SEM_NOGPFAULTERRORBOX ...)` at start (children inherit it; windows-sys feature `Win32_System_Diagnostics_Debug`).
- **Setup checks for a running game only at the start** (setup_api.rs): a game started during the downloads makes
  the copies fail with "os error 32" mid-install. Check again before the first write; map 32/5 to "Close
  Counter-Strike".
- **WebView2 accelerator keys at the source**: also turn them off in Rust (`ICoreWebView2Settings3::
  SetAreBrowserAcceleratorKeysEnabled(false)` via `with_webview`, needs the webview2-com crate), and add
  `.on_navigation` so the window never leaves the app's own address. The page guard (#9) covers the keys today.
Medium:
- **MSI and NSIS both built** (tauri.conf.json `targets`): the NSIS hooks (stop helpers, clean up) don't run for the
  MSI, which installs per machine; updates could leave two installs. Proposal: Windows builds NSIS only. Release
  owner's call.
- **The startup update dialog skips the safety checks** (main.rs check_for_update): it can update during a setup or
  restore (killing steamcmd). Share `api_update_install`'s guard.
- **Leftover hlds.exe clean-up by PID** (servers.rs end_leftover, voice.rs, sharing.rs): Windows reuses PIDs; check the
  image's full path (QueryFullProcessImageNameW) before killing.
- **Port checks** (servers.rs start, hltv.rs, voice.rs): only the app's own list is compared; Windows HLDS quietly
  moves to the next port and RCON then fails in a restart loop. Test-bind the exact address first.
- **Record keys case-sensitive** (setup/record.rs): two components shipping `Sound/x.wav` and `sound/x.wav` make two
  entries and the second backs up the first's file as the player's. Look keys up case-blind.
- **Read-only rename targets** (sys.rs rename_over, backup.rs restore): clear the flag first (as content.rs does).
- **Clipboard read prompts** (main.rs window builders): add `.enable_clipboard_access()` so editor/Studio paste from
  outside works without a permission prompt.
- **Alt key** in the editor (snap off while held, Alt+right-click wrap, Alt-drag): Windows may enter menu mode or
  ding; if so, swallow SC_KEYMENU in a window subclass.
- **Ctrl+1-9** in the editor are camera bookmarks, so on Windows they can't switch app sections (⌘1-9 on the Mac).
- **Fonts**: Barlow comes from Google Fonts; offline Windows falls back to Arial (wide labels). Ship Barlow locally
  and add Bahnschrift SemiCondensed; add Consolas to the monospace stacks (`ui-monospace` isn't on Windows Chromium).
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
