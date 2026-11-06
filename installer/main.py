"""Loner-Strike setup: interactive, or driven by flags (python3 setup.py --help)."""

import argparse
import os
import stat
import sys

from . import core, steam
from .components import install_component, load_catalog
from .fetch import FetchError
from .record import Record

TAGLINE = "Re-live the golden days of Counter-Strike locally or with the friends you don't have."


def say(text):
    print("  " + text)


def ask(question, default=True):
    hint = "Y/n" if default else "y/N"
    while True:
        a = input("%s [%s] " % (question, hint)).strip().lower()
        if not a:
            return default
        if a in ("y", "yes", "n", "no"):
            return a.startswith("y")


def choose(question, options, default=0):
    """options: list of (value, label). Returns the chosen value."""
    print(question)
    for i, (_, label) in enumerate(options, 1):
        print("  %d) %s" % (i, label))
    while True:
        a = input("Choose 1-%d [%d]: " % (len(options), default + 1)).strip()
        if not a:
            return options[default][0]
        if a.isdigit() and 1 <= int(a) <= len(options):
            return options[int(a) - 1][0]


def pick_install(args):
    if args.hl_dir:
        return steam.install_at(args.hl_dir, args.bottle)
    found = [i for i in steam.find_installs() if i.games]
    if not found:
        if sys.platform == "darwin" and steam.native_mac_hl():
            raise SystemExit("Found the Mac version of Half-Life, which mods can't run on. Install the Windows version of "
                             "Steam in CrossOver, install the game there, and run setup again.")
        raise SystemExit("Couldn't find Counter-Strike or Condition Zero. Use --hl-dir to point at the Half-Life folder.")
    if len(found) == 1 or args.yes:
        return found[0]
    return choose("Which install?", [(i, i.describe()) for i in found])


def pick_games(args, install):
    if args.game:
        games = install.games if args.game == "both" else [args.game]
        missing = [g for g in games if g not in install.games]
        if missing:
            raise SystemExit("%s isn't installed in %s" % (", ".join(missing), install.hl))
        return games
    if len(install.games) == 1 or args.yes:
        return install.games
    return choose("Which game?", [([g], steam.GAMES[g]) for g in install.games] + [(install.games, "Both")],
                  len(install.games))


def pick_components(args, catalog, games):
    usable = [c for c in catalog if c.get("kind") in ("mode", "addon") and c.get("installable")
              and any(g in c.get("games", steam.GAMES) for g in games)]
    if args.components:
        want = set(args.components.split(","))
        return [c for c in usable if c["id"] in want or c["id"].split("-", 1)[-1] in want]
    if args.basics:
        return []
    if args.all or args.yes:
        return usable
    how = choose("\nWhich game modes and add-ons?", [
        ("all", "Everything (%d modes and add-ons)" % len(usable)),
        ("basics", "Just the basics (the built-in modes: Classic, Deathmatch, weapon modes, Surf)"),
        ("pick", "Let me pick")])
    if how == "all":
        return usable
    if how == "basics":
        return []
    print()
    return [c for c in usable if ask("  %s - %s?" % (c["name"], c.get("description", "")))]


def write_launcher(install, game):
    """A double-click launcher for the control panel in the repo folder."""
    panel = os.path.join(core.REPO, "panel", "panel.py")
    if sys.platform == "win32":
        path = os.path.join(core.REPO, "Start Loner-Strike.bat")
        text = '@echo off\r\ncd /d "%s"\r\npython "%s" --game-dir "%s" --game %s\r\npause\r\n' % (
            os.path.dirname(panel), panel, install.hl, game)
    else:
        path = os.path.join(core.REPO, "Start Loner-Strike.command")
        text = '#!/bin/sh\ncd "%s" && exec python3 panel.py --game-dir "%s" --game %s\n' % (
            os.path.dirname(panel), install.hl, game)
    with open(path, "w", newline="") as f:
        f.write(text)
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return path


def run(args):
    print("\nLoner-Strike setup\n%s\n" % TAGLINE)
    install = pick_install(args)
    games = pick_games(args, install)
    catalog = load_catalog()

    if args.uninstall:
        for g in games:
            rec = Record(install.game_dir(g), args.dry_run)
            if not rec.data["files"]:
                say("Nothing from Loner-Strike in %s." % steam.GAMES[g])
                continue
            if args.yes or ask("Remove Loner-Strike from %s and restore the original files?" % steam.GAMES[g]):
                maps = [mod for mods in rec.data.get("maps", {}).values() for mod in mods]
                rec.uninstall()
                if maps and not args.dry_run:
                    from .components import map_store
                    store = map_store(install)
                    for mod in maps:
                        try:
                            store.uninstall(mod)
                        except ValueError:
                            pass      # already removed from the panel
                    say("Removed %d map download%s that came with the modes." % (len(maps), "" if len(maps) == 1 else "s"))
                say("Removed from %s." % steam.GAMES[g])
        return

    chosen = pick_components(args, catalog, games)
    print("\nInstalling into %s%s\n" % (install.hl, " (dry run: nothing is written)" if args.dry_run else ""))
    if not args.yes and not ask("Close the game first if it's running. Continue?"):
        return
    failed = []
    for g in games:
        print("\n== %s" % steam.GAMES[g])
        rec = Record(install.game_dir(g), args.dry_run)
        try:
            if not rec.installed("foundation") or args.repair:
                core.install_foundation(rec, g, say)
            for comp in chosen:
                if g in comp.get("games", steam.GAMES) and (args.repair or not rec.installed(comp["id"])):
                    if not install_component(rec, install, comp, say):
                        failed.append(comp["name"])
            modes = set(core.CORE_MODES)
            addons = set(core.CORE_ADDONS)
            for comp in catalog:
                if rec.installed(comp["id"]) or (args.dry_run and comp in chosen):
                    modes.update(comp.get("modes", []))
                    addons.update(comp.get("addons", []))
            if not core.install_core(rec, install, g, modes, addons, say):
                failed.append("Loner-Strike plugins")
        except FetchError as e:
            raise SystemExit("\nSetup stopped: %s\nNothing else was changed; run setup again to retry." % e)
    if not args.dry_run:
        launcher = write_launcher(install, games[0])
        print("\nDone. Start the game, then open the control panel with:\n  %s" % launcher)
    if failed:
        print("\nThese didn't compile, so they won't load: %s" % ", ".join(failed))


def main(argv=None):
    ap = argparse.ArgumentParser(prog="setup.py", description="Install Loner-Strike. " + TAGLINE)
    ap.add_argument("--hl-dir", help="the Half-Life folder (found automatically if left out)")
    ap.add_argument("--bottle", help="CrossOver bottle name, when --hl-dir is outside a bottle")
    ap.add_argument("--game", choices=["czero", "cstrike", "both"])
    ap.add_argument("--all", action="store_true", help="install every mode and add-on")
    ap.add_argument("--basics", action="store_true", help="only the built-in modes")
    ap.add_argument("--components", help="comma-separated ids, e.g. war3,zp5,dice")
    ap.add_argument("--repair", action="store_true", help="reinstall what's already installed")
    ap.add_argument("--uninstall", action="store_true", help="remove Loner-Strike and restore the original files")
    ap.add_argument("--dry-run", action="store_true", help="show what would happen without changing anything")
    ap.add_argument("--yes", "-y", action="store_true", help="don't ask questions; use the defaults")
    try:
        run(ap.parse_args(argv))
    except KeyboardInterrupt:
        print("\nStopped.")
