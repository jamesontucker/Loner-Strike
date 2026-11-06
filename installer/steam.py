"""Finds Half-Life installs (with czero and/or cstrike) on this computer.

Supported:
  - Windows Steam (any library folder)
  - Steam inside a CrossOver bottle on a Mac

Native Mac Steam is detected only to explain that it can't be used: AMX Mod X
and ReGameDLL have no builds for the macOS version of the game.
"""

import glob
import os
import re
import sys

GAMES = {"czero": "Counter-Strike: Condition Zero", "cstrike": "Counter-Strike 1.6"}
CROSSOVER_WINE = "/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"


class Install:
    """One Half-Life folder and how to run Windows programs for it."""

    def __init__(self, hl_dir, kind, bottle=None):
        self.hl = hl_dir
        self.kind = kind            # "windows" or "crossover"
        self.bottle = bottle        # CrossOver bottle name
        self.games = [g for g in GAMES if os.path.isfile(os.path.join(hl_dir, g, "liblist.gam"))]

    def game_dir(self, game):
        return os.path.join(self.hl, game)

    def windows_command(self, exe, *args):
        """argv that runs a Windows program (e.g. the plugin compiler) here."""
        if self.kind == "crossover":
            return [CROSSOVER_WINE, "--bottle", self.bottle, "--wait-children", exe] + list(args)
        return [exe] + list(args)

    def describe(self):
        where = "CrossOver bottle \"%s\"" % self.bottle if self.kind == "crossover" else "Steam"
        return "%s (%s): %s" % (where, self.hl, ", ".join(GAMES[g] for g in self.games) or "no supported games")


def _library_paths(steam_root):
    """Every Steam library listed in libraryfolders.vdf, plus the main one."""
    paths = [steam_root]
    vdf = os.path.join(steam_root, "steamapps", "libraryfolders.vdf")
    try:
        with open(vdf, encoding="utf-8", errors="replace") as f:
            for m in re.finditer(r'"path"\s+"([^"]+)"', f.read()):
                paths.append(m.group(1).replace("\\\\", "\\"))
    except OSError:
        pass
    return paths


def _hl_in(library):
    p = os.path.join(library, "steamapps", "common", "Half-Life")
    return p if os.path.isdir(p) else None


def find_windows():
    roots = []
    try:
        import winreg
        for hive, key in ((winreg.HKEY_CURRENT_USER, r"Software\Valve\Steam"),
                          (winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Valve\Steam")):
            try:
                with winreg.OpenKey(hive, key) as k:
                    for name in ("SteamPath", "InstallPath"):
                        try:
                            roots.append(os.path.normpath(winreg.QueryValueEx(k, name)[0]))
                        except OSError:
                            pass
            except OSError:
                pass
    except ImportError:
        pass
    roots.append(r"C:\Program Files (x86)\Steam")
    found, seen = [], set()
    for root in roots:
        for lib in _library_paths(root):
            hl = _hl_in(lib)
            if hl and hl.lower() not in seen:
                seen.add(hl.lower())
                found.append(Install(hl, "windows"))
    return found


def find_crossover():
    found = []
    pattern = os.path.expanduser("~/Library/Application Support/CrossOver/Bottles/*/drive_c")
    for drive_c in glob.glob(pattern):
        bottle = os.path.basename(os.path.dirname(drive_c))
        for steam in ("Program Files (x86)/Steam", "Program Files/Steam"):
            root = os.path.join(drive_c, steam)
            if not os.path.isdir(root):
                continue
            for lib in _library_paths(root):
                if re.match(r"^[A-Za-z]:\\", lib):      # a Windows path inside the bottle
                    if lib[0].lower() != "c":
                        continue
                    lib = os.path.join(drive_c, lib[3:].replace("\\", "/"))
                hl = _hl_in(lib)
                if hl and all(i.hl != hl for i in found):
                    found.append(Install(hl, "crossover", bottle))
    return found


def native_mac_hl():
    p = os.path.expanduser("~/Library/Application Support/Steam/steamapps/common/Half-Life")
    return p if os.path.isdir(p) else None


def find_installs():
    if sys.platform == "win32":
        return find_windows()
    if sys.platform == "darwin":
        return find_crossover()
    return []


def install_at(path, bottle=None):
    """An Install for a folder given by hand (--hl-dir). On a Mac the bottle
    is taken from the path, or given with --bottle (for a test copy elsewhere)."""
    path = os.path.abspath(os.path.expanduser(path))
    if sys.platform == "darwin":
        m = re.search(r"/CrossOver/Bottles/([^/]+)/drive_c/", path)
        if not m and not bottle:
            raise SystemExit("On a Mac the game has to be inside a CrossOver bottle (or pass --bottle).")
        return Install(path, "crossover", bottle or m.group(1))
    return Install(path, "windows")
