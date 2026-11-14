# Game Mode Studio: UX/UI review (2026-11-14)

This review only assesses the Studio. It changes no code. I'm a fresh reviewer who didn't build it.
How I tested: the `tools` branch at b782545b, with the dev servers the specs use (`serve_tools_dev` on 8141 and Vite on 5241).
Saved graphs went to a scratch `LS_APP_DIR`. Compile, Test run, Playtest, Install and the debugger routes were answered in the
page, the way `studio-v3.spec.ts` does it, so nothing was installed and no sim ran. Test run, Playtest, the debugger and live
tweaks are judged from the code (`StudioPage.tsx`, `Results.tsx`, `DebugPanel.tsx`) and the specs.
I used Chromium at 1440×900 and 1024×680.

The screenshots are in `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/studio-ux/`
(called `shots/` below). That's a scratch folder: copy the shots out if you want to keep them.

## High impact

**H1. The examples and the tour open zoomed out too far to read.**
Every example opens at the minimum zoom of 0.2, and even then the graph doesn't fit. Node titles, pins and the "notes on every
part" are unreadable, and the tour's step 2 tells you to read them.
Where: `Canvas.tsx` (fitView with `minZoom 0.2`).
Shots: `shots/02-tour-2.png`, `shots/44-example-1.png`.
Fix: open a big graph at 100% on its first comment, or on a saved "Start here" view. Then add a "Next part ›" button, or let
the tour step through the comment boxes. Effort: small to medium.

**H2. The Mode details can't be reached again.**
After you click any item under My mode (a custom event, function, enum and so on), the Details panel stays on it. Clicking
empty canvas, pressing Escape or picking and then clearing a node doesn't bring back "This mode". That hides the name,
description, Game settings and Requirements until you reopen the mode.
Where: `store.ts` (`select([])` never clears `part`) and `Inspector.tsx:637`.
Shot: `shots/38-details-stuck.png`.
Fix: clear `part` when the canvas selection is cleared. Also add a permanent **Mode settings** button to the toolbar, like
Unreal's Class Settings. Effort: small.

**H3. Requirements has no styling and holds the whole map list.**
"Default map" is a raw select. "Recommended maps" is a bulleted list of 366 checkbox rows, so the Details panel scrolls to
about 15,000 px and scrolls sideways too. The canvas tips at the bottom of Details end up buried under it. There's no line
saying what recommended maps are for.
Where: `Requirements.tsx`. There's no `.st-req*` CSS in `studio.css`.
Shots: `shots/41-requirements.png`, `shots/37a.png`.
Fix: make it a searchable multi-select (chips plus a search box), base-game maps first. Add one line explaining what it's for
and fix the sideways scroll. Effort: medium.

**H4. The canvas is tiny in small windows, and the side panels can't be collapsed.**
The panels are fixed at 230 px and 290 px (`.st-main`). At 1024×680 the canvas is 306×285 px, about 13% of the window. Even in
Focus it's 556×385, and the zoom controls cover part of it. The node list is cut off by the canvas: it's 440 px tall inside a
283 px canvas, so you see about 5 rows.
Shots: `shots/51-small-editor.png`, `shots/52-small-focus.png`, `shots/54-small-palette.png`.
Fix: make My mode and Details collapsible and resizable, like Unreal's tabs. Below about 1200 px wide, collapse Details into a
drawer. Draw the node list in a portal so it isn't clipped. Hide the minimap by default. Effort: medium.

**H5. Pressing Enter after a search in the node list doesn't place a node.**
After you type, the cursor sits on the first folder header, so Enter folds the folder. Results aren't ranked either:
"killed" lists Bot Trait (it matches "skilled") right under Player Killed. In Unreal, you type and press Enter.
Where: `Palette.tsx:191` and `:244`.
Shots: `shots/07-palette-killed.png`, `shots/08-player-killed.png`.
Fix: while searching, put the cursor on the first item. Rank title matches first (prefix, then whole word), then help text.
Effort: small.

**H6. You can't type a value on an unwired input pin.**
In Unreal you type a pin's default right beside it. Here you get a read-only "Amount: 500" line on the node and have to edit it
in Details. For a non-coder that's a hidden step on almost every node.
Where: `Nodes.tsx` (`st-node-params`).
Shot: `shots/11-give-money-details.png`.
Fix: add small inline fields (number, yes/no, short text, a dropdown for enums and weapons) next to unwired inputs. Details
keeps the long text. Effort: large (fields, widths and layout, but worth it).

**H7. A blank Event Graph gives you nowhere to start.**
A Blank mode opens to an empty grid and a big empty minimap. There's no hint, and the Quick check says "no problems" for a
mode that does nothing.
Shot: `shots/05-blank-editor.png`.
Fix: put an empty-state card in the canvas ("Right-click or press Space to add your first event", with buttons for Round
Started and Player Killed), or start with greyed-out ghost events like Unreal's BeginPlay. The Quick check should say "Nothing
happens yet: add an event". Effort: small.

## Medium impact

**M1. The minimap covers nodes.**
The minimap is large, sits on top of pins in the lower right, and shows even on empty graphs.
Shots: `shots/08b-player-killed.png`, `shots/30-new-function.png`.
Fix: make it smaller, semi-transparent, off by default, and toggled from a View menu. Effort: small.

**M2. Variable drop menu ignores Escape and outside clicks.**
When you drop a variable with no modifier, the Get/Set choice ignores Escape and clicks outside it. It stays open after you
drop more nodes. It also has no keyboard focus.
Where: `Canvas.tsx:820`.
Shot: `shots/21-var-get.png`.
Fix: reuse the context menu's close and keyboard logic. Effort: small.

**M3. Promote to Variable puts the new Get on top of another node.**
Get Amount landed over Player Killed's header.
Shot: `shots/25-versions.png`.
Fix: place the node left of the pin's node, in free space. Effort: small.

**M4. Get and Set nodes always show Player and Team pins.**
They show even for a whole-mode variable, and the node says "Get Variable 1" in a full header. That's noisy and misleading.
Shot: `shots/21-var-get.png`.
Fix: show Player and Team only when the scope needs them. Use Unreal's compact "pill" Get (just the name and the output).
Effort: medium.

**M5. A new node isn't selected, and new items don't start in rename.**
A placed node doesn't open its help in Details. A new variable or function doesn't open with its name ready to type.
Shots: `shots/08b-player-killed.png`, `shots/17-add-variable.png`.
Fix: select the node you just placed. Focus the name field (select-all) on a new variable, function, custom event, Timeline or
table. Effort: small.

**M6. The toolbar is crowded and wraps in the wrong place.**
At 1440 px the map, the bots field and Playtest (the main action) wrap to a second row, apart from Compile and Test run. The
bots field is a bare "9" with no visible label. Bookmarks (nine identical "Keep this view as N" items) and Versions take
top-level buttons. The page header ("History" + "Focus" + the BETA line) is a second history that sits above "Versions".
Shots: `shots/13c-toolbar.png`, `shots/27-bookmarks.png`.
Fix: use one row of Undo/Redo · Find · View (Tidy up, Bookmarks, Minimap) · Versions, and on the right Compile ✓ · Test run ·
[map ▾] [9 bots] **Playtest**. Label it "bots". Let users name bookmarks, and list the saved ones in the menu.
Effort: medium.

**M7. Custom events don't put a node on the graph.**
Adding one under My mode creates nothing on the canvas. Its Details then explains how to find the node yourself.
Shot: `shots/31-custom-event.png`.
Fix: place the Custom Event node at the view's center when you add one, as Unreal does. Effort: small.

**M8. The Timeline editor feels cramped.**
- Lanes are 96 px tall with no value axis, so it's hard to shape curves.
- The ruler's "0s" is cut off to "s".
- With Length 3 s, the first view only shows about 2.1 s until you press Fit.
- Picking a key adds a row above the lanes, so the lane jumps under the mouse (my second click missed it).
- A tiny unlabeled "×" removes a track with no undo hint.
- Details still shows the last item you picked, not the Timeline.

Shots: `shots/34-tl-keys.png`, `shots/35-tl-custom.png`.
Fix: fit the length on open, make lanes resizable with a value axis, keep the key row's space fixed, and label the track
remove button. Effort: medium.

**M9. Yes/no game settings take a typed 1 or 0.**
"Free for all (1 on)" and "Team balance (1 on)" are a dropdown plus a number box. Long names are cut off in the dropdown.
Shot: `shots/44-example-1.png`.
Fix: use a switch for yes/no settings and a stepper with units for numbers. Show the full name as a label. Effort: small.

**M10. Opening an example makes you name a copy first.**
"Open one to see how it's made" goes straight to "What's the mode called?". The start screen also has two groups: unlabeled
template buttons (the blurb is only a tooltip, with Rifle Reward styled as the main button) and example cards with blurbs.
Shots: `shots/01-start.png`, `shots/43-example-name.png`.
Fix: open examples read-only with a "Make it mine" button. Show templates as cards with their blurbs. Make Blank mode or the
tour the main button for first-timers. Effort: medium.

**M11. Copy problems.**
- The Playtest step says "Compiling (the first time under CrossOver takes longer)", which makes no sense on Windows.
- Test run is "about a minute and a half" in its tooltip but "about two minutes" in the tour and docs.
- "Full check: compiling…." has an extra period.
- British "round" for "around" in "Let's look round", "joined up round them" (DeleteDialog) and "walk round".
- "+4 −0 ~0" in Versions is cryptic. Use "4 added".
- The Problems link says "Set Variable" but the node title is "Set Variable 1".
- The Timeline key range message is garbled: "its time 0 to the length, its value the track's kind".

Where: `StudioPage.tsx:493`, `Tour.tsx:41`, `DeleteDialog.tsx:109`, `VersionHistory.tsx`, `Results.tsx`.
Effort: small.

**M12. The shortcut list is incomplete.**
The "?" list leaves out the quick-add keys (B, S, D, F, L, G, O, N, R, P, W + click), F1, bookmarks (Ctrl/⌘+1–9), and Ctrl/Alt
drags. Notation is mixed: "⌘Shift+1" next to "⇧⌘Z".
Shot: `shots/29-shortcuts.png`.
Fix: add a "Quick add" section and use one notation (Ctrl on Windows). Effort: small.

## Low impact

- **L1. Two events look the same.** Events with two Player Killed nodes list "Player Killed" twice. Add "(2)" or the comment
  they sit in. `shots/02-tour-2.png`.
- **L2. Node help ignores Escape after F1.** F1 opens node help, but focus stays on the canvas, so Escape doesn't close it.
  Move focus to the panel. `NodeHelp.tsx:123`.
- **L3. Variables are far down My mode.** My mode has 13 sections, and Variables is 8th (below the fold at 900 px). Order them
  Graphs, Functions, Variables, Custom events, then the rest.
- **L4. Pin menu.** The pin menu only has Promote. Add Break links and Select linked here, as Unreal does.
- **L5. Breakpoint dot.** The hollow circle on every node title looks like a radio button. Show it on hover only, or put it in
  the gutter, as Unreal does.
- **L6. Version history panel.** It's 380 px wide and mostly empty, and it covers the changed nodes it's pointing at. Make it
  narrower, or dock it in Results.
- **L7. Data table header.** The header cells look like data cells. The column remove "×" sits right beside the type
  dropdown. `shots/36-data-table.png`.
- **L8. Accessibility.** Nodes are `role=button` with no label, and pins have no labels. Give nodes `aria-label="<title>
  node"`. Contrast checked fine: muted text 4.4–5.8:1, pins 13.8:1, and the focus ring shows (`shots/53-focus-ring.png`).
- **L9. The context menu shows options that can't apply.** On a variable, "Save to my library…" appears with a note that it
  can't apply. Hide it instead.

## What's already great

- The quick check's warnings are plain and tied to their nodes, with a link that frames the node and a badge on the node.
- Busy feedback is consistent: "Compiling…", "Test run…" and Playtest's step label with seconds and Cancel all have spinners.
  The save bar says Saved or Unsaved changes.
- The delete question for a variable in use (Replace with, Remove with nodes, Keep nodes, Find uses, one Undo) is better than
  Unreal's.
- Context-sensitive search on a dropped wire works, with Favorites and Recently used. Quick-add with B + click works.
  Promote to Variable names the variable after the pin.
- Node help with the pins and settings explained, an example and the Quick-add hint is excellent learning material.
- Version history with highlighted added and changed nodes, plus Restore, works smoothly.
- The tour's steps really do highlight the real controls, and it's easy to skip or replay.
- Docs (`site/docs.html`) are thorough. "Why won't it compile?" is especially good.

## Top 10 before 1.0

1. **H2**: get back to Mode settings, and add a Mode settings button (small).
2. **H1**: examples and the tour open readable, at a "Start here" view (small to medium).
3. **H5**: Enter places the top search match, and results are ranked (small).
4. **H7**: an empty-canvas starter card for a blank mode (small).
5. **H3**: restyle Requirements and make the map picker searchable (medium).
6. **H4**: collapsible side panels, a node list that isn't clipped, and the minimap off by default (medium; covers M1).
7. **M2 + M3 + M5**: the drop menu closes, Promote places the node clear of others, new things are selected and renamable
   (small).
8. **M6**: the toolbar on one row, with Playtest next to its map and bots (medium).
9. **M11 + M12**: copy fixes, the CrossOver line, US English, and quick-add keys in the shortcut list (small).
10. **H6**: inline pin values. It's the biggest Blueprint-feel gap. Start with number and yes/no (large).

## Who fixes what (X, 2026-11-14)
X takes: **H2** (store.ts select/part, a Mode settings button), **H5** (Palette.tsx search: Enter, ranking), **H7**
(empty-canvas card in Canvas.tsx, Quick check notice in graph.rs), **H6** (inline pin values: Nodes.tsx + a small
pinValue helper; last), **M4** (Get/Set pins by scope: Nodes.tsx / registry pins), **M11** Playtest's CrossOver line
(StudioPage.tsx:493 only) and "Full check: compiling…." (Results.tsx), then L9 if X2 doesn't want it. X2 has the rest.
Lines I'll touch outside those: Canvas.tsx only its empty-state render; StudioPage.tsx only the Playtest step text and
the toolbar's new Mode settings button. X2: say here if you're in those spots and I'll wait.
