# Roadmap

## 1. Installer (next)
One interactive script, `setup.py`, for Mac (CrossOver or native Steam) and Windows:
1. Finds Steam and the game folders: `czero` (Condition Zero), `cstrike` (CS 1.6), or both.
2. Backs up anything it replaces and records every file it installs, so uninstalling is clean.
3. Installs the foundation: Metamod-P, AMX Mod X, ReGameDLL and the Loner-Strike plugins and panel.
4. Asks which modes and add-ons you want, with an "everything" option and a "just the basics" option.
5. Downloads each piece from its original source (see `catalog/components.json`), applies the bot patches and compiles the plugins.

## 2. "Get more" in the panel
The panel reads the same catalog, so anything skipped during setup can be installed or removed later with a button, like maps already are.

## 3. CS 1.6 support
Steam CS 1.6 has had the same bots as Condition Zero since 2013 and ReGameDLL supports both, so nearly everything works as is. To do:
- the panel stops assuming `czero` and lets you pick the game
- copy a bot profile file into `cstrike`
- use the right player models for each game

## 4. Windows polish
Replace the two Mac-only commands in the panel: `sips` for thumbnails and `open` for the browser. Test the playit agent's Windows build and the Tailscale path.
