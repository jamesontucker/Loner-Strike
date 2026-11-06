"""The foundation (Metamod-P, AMX Mod X, ReGameDLL) and Loner-Strike's own plugins.

Every version here is the one Loner-Strike is tested with, pinned by checksum.
"""

import glob
import os
import re
import subprocess
import tempfile

from . import fetch

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

FOUNDATION = {
    "metamod": ("https://github.com/Bots-United/metamod-p/releases/download/v1.21p109/metamod-p-v1.21p109-win32.tar.xz",
                "564838d9a42032ee3bc44eeb2616896e0789c3ce8d7a033d81f3f795b67e31ed"),
    "amxx_base": ("https://www.amxmodx.org/amxxdrop/1.10/amxmodx-1.10.0-git5487-base-windows.zip",
                  "ca13b474741f5f0b035507969cb853b7eac9052614402114801e21c4dc2f2ebd"),
    "amxx_cstrike": ("https://www.amxmodx.org/amxxdrop/1.10/amxmodx-1.10.0-git5487-cstrike-windows.zip",
                     "47f358b4dd40c0259576bbde9790442dee4b642fe31bfed0421180b81429d4aa"),
    "regamedll": ("https://github.com/rehlds/ReGameDLL_CS/releases/download/5.30.0.814/regamedll-bin-5.30.0.814.zip",
                  "457f5c96a4d10280fcad47f106cbd5be86363a47df1ccab91c514e9d1bc6fd18"),
}
METAMOD_DLL = r"addons\metamod\dlls\metamod.dll"

# AMX Mod X files a player may have customised: never replaced if present
KEEP_IF_PRESENT = ("addons/amxmodx/configs/",)

# Modes that need nothing beyond Loner-Strike's own plugins
CORE_MODES = ("classic", "csdm", "csdm_ffa", "knife", "pistols", "deagle", "awp", "scoutz", "surf")
CORE_ADDONS = ("quakesounds",)          # miscstats.amxx comes with AMX Mod X
OUR_PLUGINS = ("gamemodes", "gm_humanbots", "gm_pubserver", "gm_deathmatch")
ALWAYS_ON = ("gamemodes", "gm_humanbots", "gm_pubserver")   # listed in plugins.ini


def _tree(root):
    for d, _, files in os.walk(root):
        for f in files:
            full = os.path.join(d, f)
            yield full, os.path.relpath(full, root).replace(os.sep, "/")


def install_foundation(rec, game, say):
    """Metamod-P, AMX Mod X (base + Counter-Strike addon) and ReGameDLL."""
    rec.begin("foundation")
    with tempfile.TemporaryDirectory() as tmp:
        # Metamod-P
        say("Metamod-P 1.21p109")
        mm = fetch.unpack(fetch.download(*FOUNDATION["metamod"]), os.path.join(tmp, "mm"))
        rec.copy(os.path.join(mm, "addons/metamod/dlls/metamod.dll"), "addons/metamod/dlls/metamod.dll")
        plugins_ini = rec.read("addons/metamod/plugins.ini") or ""
        if "amxmodx_mm.dll" not in plugins_ini.lower():
            rec.write("addons/metamod/plugins.ini", plugins_ini.rstrip("\r\n") +
                      ("\r\n" if plugins_ini.strip() else "") + "win32 addons\\amxmodx\\dlls\\amxmodx_mm.dll\r\n")

        # AMX Mod X
        say("AMX Mod X 1.10 (build 5487) with the Counter-Strike addon")
        for key in ("amxx_base", "amxx_cstrike"):
            src = fetch.unpack(fetch.download(*FOUNDATION[key]), os.path.join(tmp, key))
            for full, rel in _tree(src):
                if not rel.startswith("addons/"):
                    continue
                if rel.startswith(KEEP_IF_PRESENT) and rec.exists(rel) and rel not in rec.data["files"]:
                    continue
                rec.copy(full, rel)

        # ReGameDLL replaces the game's own server code (mp.dll); the original is backed up
        say("ReGameDLL 5.30")
        rg = fetch.unpack(fetch.download(*FOUNDATION["regamedll"]), os.path.join(tmp, "rg"))
        rec.copy(os.path.join(rg, "bin/win32/cstrike/dlls/mp.dll"), "dlls/mp.dll")
        for cfg in ("game.cfg", "game_init.cfg"):
            if not rec.exists(cfg):
                rec.copy(os.path.join(rg, "bin/win32/cstrike", cfg), cfg)

    # Point the game at Metamod
    lib = rec.read("liblist.gam")
    if lib is None:
        raise SystemExit("%s has no liblist.gam - is this really the %s folder?" % (rec.game, game))
    m = re.search(r'^gamedll\s+"([^"]*)"', lib, re.M)
    if not m:
        lib = lib.rstrip("\r\n") + '\r\ngamedll "%s"\r\n' % METAMOD_DLL
    elif m.group(1).lower() != METAMOD_DLL.lower():
        lib = lib[:m.start(1)] + METAMOD_DLL + lib[m.end(1):]
    else:
        lib = None
    if lib is not None:
        rec.write("liblist.gam", lib)
    rec.done("foundation")


def compile_plugin(rec, install, src_rel, out_name, include_rel=None, say=print):
    """Compiles addons/amxmodx/scripting/<src_rel> into plugins/<out_name>.amxx."""
    am = os.path.join(rec.game, "addons", "amxmodx")
    exe = os.path.join(am, "scripting", "amxxpc.exe")
    src = os.path.join(am, "scripting", src_rel)
    out_rel = "addons/amxmodx/plugins/%s.amxx" % out_name
    rec._prepare(out_rel)
    if rec.dry:
        return True
    args = [src, "-o" + os.path.join(rec.game, out_rel)]
    if include_rel:
        args.append("-i" + os.path.join(am, "scripting", include_rel))
    r = subprocess.run(install.windows_command(exe, *args), capture_output=True, text=True)
    out = (r.stdout or "") + (r.stderr or "")
    if "Done." not in out:
        errors = [l for l in out.splitlines() if "error" in l.lower()][:5]
        say("    couldn't compile %s:\n      %s" % (src_rel, "\n      ".join(errors) or out[-300:]))
        return False
    return True


def _filter_lines(text, keep):
    """Keeps comment lines and lines whose first word (an id) passes keep(id)."""
    out = []
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith((";", "//")) or keep(s.split()[0]):
            out.append(line)
    return "\r\n".join(out) + "\r\n"


def install_core(rec, install, game, modes, addons, say):
    """Loner-Strike's plugins, the Game Mode Menu configs, bot styles and keys."""
    rec.begin("core")
    plugins_src = os.path.join(REPO, "plugins")
    say("Loner-Strike plugins")
    for name in OUR_PLUGINS:
        rec.copy(os.path.join(plugins_src, "scripting", name + ".sma"), "addons/amxmodx/scripting/%s.sma" % name)
    ok = all([compile_plugin(rec, install, name + ".sma", name, say=say) for name in OUR_PLUGINS])

    pl = rec.read("addons/amxmodx/configs/plugins.ini") or ""
    add = [n for n in ALWAYS_ON if not re.search(r"^\s*%s\.amxx" % n, pl, re.M)]
    if add:
        rec.write("addons/amxmodx/configs/plugins.ini", pl.rstrip("\r\n") + "\r\n" +
                  "".join("%s.amxx\t\t; Loner-Strike\r\n" % n for n in add))

    say("Game modes and settings")
    cfg_src = os.path.join(plugins_src, "configs", "gamemodes")
    mode_ok = lambda m: m in modes
    for full, rel in _tree(cfg_src):
        dest = "addons/amxmodx/configs/gamemodes/" + rel
        base = rel.split("/")[-1]
        mode_id = base.split(".")[0]
        if rel.startswith("maps/") and base != "README.txt" and not mode_ok(mode_id):
            continue
        if rel.startswith("addons/") and mode_id not in addons:
            continue
        if base.endswith((".plugins.ini",)) or (base.endswith(".cfg") and "/" not in rel and base != "base.cfg"):
            if not mode_ok(mode_id):
                continue
        with open(full, encoding="latin-1", newline="") as f:
            text = f.read()
        if base == "modes.ini":
            text = _filter_lines(text, mode_ok)
        elif base == "addons.ini":
            text = _filter_lines(text, lambda a: a in addons)
        elif base == "mappresets.ini":
            text = "\r\n".join(l for l in text.splitlines()
                               if not l.strip() or l.lstrip().startswith(";") or l.split()[-1] in modes) + "\r\n"
        elif base == "presets.ini":
            def preset_ok(line):
                m = re.findall(r'"([^"]*)"', line)
                return len(m) < 3 or m[2] in modes
            text = "\r\n".join(l for l in text.splitlines()
                               if not l.strip() or l.lstrip().startswith(";") or preset_ok(l)) + "\r\n"
        rec.write(dest, text)

    # Bot styles: personality files made from the game's bot profile
    say("Bot styles")
    if game == "cstrike" and not rec.exists("BotProfile.db"):
        from . import botprofile
        rec.write("BotProfile.db", botprofile.generate())
    import importlib.util
    spec = importlib.util.spec_from_file_location("botstyles", os.path.join(REPO, "panel", "botstyles.py"))
    bs = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bs)
    base = rec.read("BotProfileEnhanced.db") or rec.read("BotProfile.db")
    if base is None and rec.dry:
        base = ""
    if base is None:
        say("    no bot profile found, so the bot styles keep CZ's own personalities")
    else:
        for style in bs.STYLES:
            rec.write("BotProfile_%s.db" % style,
                      "// Made by Loner-Strike for the %s bot style\r\n" % style + bs.reshape(base, style))

    # F3 opens the Game Mode Menu, F4 the quick menu
    cfg = rec.read("config.cfg") or ""
    binds = [b for b in ('bind "F3" "gamemenu"', 'bind "F4" "gamequick"') if b.split('"')[3] not in cfg]
    if binds:
        rec.write("config.cfg", cfg.rstrip("\r\n") + "\r\n" + "\r\n".join(binds) + "\r\n")
    rec.done("core")
    return ok
