#!/usr/bin/env python3
"""CZ Control Panel helper.

Serves the control panel web app and relays its commands to Counter-Strike:
Condition Zero over RCON. Talks to the Game Mode Menu plugin (gamemodes.amxx),
which writes its live state to data/gamemodes/status.json.

    python3 panel.py           # this Mac only:  http://127.0.0.1:8027
    python3 panel.py --lan     # also your phone (prints a private link)

Only the Python standard library is used. Thumbnails are converted with the
macOS `sips` tool and cached next to this file.
"""

import argparse
import json
import os
import re
import secrets
import signal
import socket
import struct
import subprocess
import sys
import threading
import time
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

from getmaps import MapStore
from sharing import Sharing

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_HL = os.path.expanduser(
    "~/Library/Application Support/CrossOver/Bottles/Steam/drive_c/"
    "Program Files (x86)/Steam/steamapps/common/Half-Life"
)

# Commands the panel may send. Everything else is refused.
ALLOWED = {
    "gm_mode", "gm_map", "gm_set", "gm_addon", "gm_addonset", "gm_restartmap",
    "gm_bots", "gm_match", "gm_player", "gm_nav", "gm_pause", "gm_team",
    "gm_preset", "gm_presetsave", "gm_presetdel", "gm_pos", "gm_crash", "gm_do", "gm_announce",
}
MAP_LISTS = {"fav": "favorites.txt", "hidden": "hidden_maps.txt", "recent": "recent.txt"}
METAMOD_DLL = r"addons\metamod\dlls\metamod.dll"
SAFE_ARG = re.compile(r"^[A-Za-z0-9_.\-]{1,64}$")

# Names people search for that the file names don't contain
MAP_TITLES = {
    "de_cbble": "Cobblestone", "de_cbble_cz": "Cobblestone", "cs_747": "Boeing 747 hijack",
    "as_oilrig": "Oil rig", "de_dust2": "Dust II", "de_dust2_cz": "Dust II",
    "cs_militia": "Militia", "de_chateau": "Chateau",
}

FUN_PREFIXES = ("fy_", "aim_", "awp_", "ka_", "35hp", "scout", "he_", "gg_", "zm_", "dm_",
                "mg_", "fun_", "knife", "arena", "pool_", "deathrun", "jail", "bhop", "kz_")


def map_category(name):
    n = name.lower()
    if n.startswith("de_"):
        return "defuse"
    if n.startswith("cs_"):
        return "hostage"
    if n.startswith(("as_", "es_")):
        return "objective"
    if n.startswith("surf"):
        return "surf"
    if n.startswith(FUN_PREFIXES):
        return "arena"
    return "other"


class Rcon:
    """GoldSrc RCON over UDP (challenge, then the command)."""

    PREFIX = b"\xff\xff\xff\xff"

    def __init__(self, password, host="127.0.0.1", port=27015):
        self.password, self.host, self.port = password, host, port
        self.lock = threading.Lock()

    def send(self, command, timeout=1.5):
        with self.lock:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            try:
                s.settimeout(timeout)
                s.sendto(self.PREFIX + b"challenge rcon\n", (self.host, self.port))
                data, _ = s.recvfrom(4096)
                parts = data[4:].decode("latin-1").split()
                if len(parts) < 3 or parts[0] != "challenge":
                    raise ConnectionError("unexpected reply from the game")
                msg = 'rcon %s "%s" %s\n' % (parts[2], self.password, command)
                s.sendto(self.PREFIX + msg.encode("latin-1"), (self.host, self.port))

                singles, splits = [], {}
                s.settimeout(0.4)
                while True:
                    try:
                        data, _ = s.recvfrom(65535)
                    except socket.timeout:
                        break
                    if data[:4] == b"\xfe\xff\xff\xff":
                        splits.setdefault(data[4:8], {})[data[8] >> 4] = data[9:]
                    else:
                        singles.append(data)
                for chunks in splits.values():
                    singles.append(b"".join(chunks[i] for i in sorted(chunks)))
                out = []
                for d in singles:
                    if d.startswith(self.PREFIX):
                        d = d[5:]
                    out.append(d.decode("latin-1", "replace"))
                text = "".join(out)
                if "Bad rcon_password" in text:
                    raise PermissionError("the game rejected the panel password - restart the map once")
                return text
            finally:
                s.close()


class Game:
    def __init__(self, hl_dir, game="czero"):
        self.hl = hl_dir
        self.cz = os.path.join(hl_dir, game)      # czero or cstrike
        self.data = os.path.join(self.cz, "addons", "amxmodx", "data", "gamemodes")
        self.cache = os.path.join(HERE, "cache")
        os.makedirs(self.cache, exist_ok=True)
        os.makedirs(self.data, exist_ok=True)
        pw_file = os.path.join(self.data, "panel_rcon.txt")
        if not os.path.exists(pw_file) or not open(pw_file).read().strip():
            with open(pw_file, "w") as f:
                f.write(secrets.token_urlsafe(18) + "\n")
        self.rcon = Rcon(open(pw_file).read().strip())
        self._maps = None
        self.store = MapStore(self, HERE)
        self.sharing = Sharing(HERE, self)

    # ---- live state -----------------------------------------------------
    def state(self):
        try:
            reply = self.rcon.send("gm_status")
        except PermissionError as e:
            return {"online": False, "reason": str(e)}
        except (OSError, ConnectionError):
            return {"online": False, "reason": "Condition Zero isn't running, or no match is loaded."}

        lines = []
        path = os.path.join(self.data, "status.json")
        if "GM status written" in reply and os.path.exists(path):
            with open(path, encoding="latin-1") as f:
                lines = f.read().splitlines()
        else:  # plugin version that prints its status instead of writing it
            lines = [l[3:] for l in reply.splitlines() if l.startswith("GM {")]
        if not lines:
            return {"online": False, "reason": "The game is running but the Game Mode Menu plugin didn't answer."}

        out = {"online": True, "modes": [], "settings": [], "addons": [], "addonsets": [], "players": [],
               "presets": [], "events": [], "me": {}}
        for line in lines:
            try:
                obj = json.loads(line)
            except ValueError:
                continue
            kind = obj.pop("t", None)
            if kind == "state":
                out["state"] = obj
            elif kind == "me":
                out["me"] = obj
            elif kind in ("mode", "setting", "addon", "addonset", "player", "preset", "event"):
                out[kind + "s"].append(obj)
        return out

    def command(self, cmd, args):
        if cmd not in ALLOWED:
            raise ValueError("unknown command")
        if cmd == "gm_announce":
            # free text, so keep only characters that can't end or extend the console command
            text = re.sub(r'[^\w .,!?\'()+\-:#@&%/]', "", " ".join(args), flags=re.ASCII)[:120].strip()
            if not text:
                raise ValueError("empty message")
            self.rcon.send('gm_announce "%s"' % text)
            return []
        if not all(SAFE_ARG.match(a) for a in args):
            raise ValueError("invalid argument")
        reply = self.rcon.send(" ".join([cmd] + args))
        return [l.strip() for l in reply.splitlines() if l.strip()]

    # ---- maps -----------------------------------------------------------
    def maps(self, refresh=False):
        if self._maps is None or refresh:
            self._maps = self._scan_maps()
        fav = {m.lower() for m in self.read_list("fav")}
        hidden = {m.lower() for m in self.read_list("hidden")}
        recent = [m.lower() for m in self.read_list("recent")]
        out = []
        for m in self._maps:
            k = m["name"].lower()
            out.append(dict(m, fav=k in fav, hidden=k in hidden, recent=recent.index(k) if k in recent else -1))
        return out

    def read_list(self, which):
        path = os.path.join(self.data, MAP_LISTS[which])
        if not os.path.exists(path):
            return []
        with open(path, encoding="latin-1") as f:
            return [l.strip() for l in f if l.strip()]

    def set_flag(self, which, name, on):
        if which not in ("fav", "hidden") or not SAFE_ARG.match(name):
            raise ValueError("invalid map")
        items = [m for m in self.read_list(which) if m.lower() != name.lower()]
        if on:
            items.insert(0, name)
        with open(os.path.join(self.data, MAP_LISTS[which]), "w", encoding="latin-1") as f:
            f.write("".join(m + "\n" for m in items))

    def _scan_maps(self):
        found = {}
        for game in ("czero", "cstrike"):
            d = os.path.join(self.hl, game, "maps")
            if not os.path.isdir(d):
                continue
            for f in os.listdir(d):
                if f.lower().endswith(".bsp"):
                    found.setdefault(f[:-4].lower(), f[:-4])
        navs = set()
        for game in ("czero", "cstrike"):
            d = os.path.join(self.hl, game, "maps")
            if os.path.isdir(d):
                navs.update(f[:-4].lower() for f in os.listdir(d) if f.lower().endswith(".nav"))
        by = self.map_sources()
        by.update(self.store.authors())
        maps = []
        for key in sorted(found):
            name = found[key]
            kind, _ = self.thumb_source(name)
            maps.append({"name": name, "cat": map_category(name), "nav": key in navs, "thumb": kind,
                         "by": by.get(key, "Other"), "title": self.map_title(name)})
        return maps

    def map_title(self, name):
        """The map's own name for itself (worldspawn "message"), for search."""
        if name.lower() in MAP_TITLES:
            return MAP_TITLES[name.lower()]
        for game in ("czero", "cstrike"):
            path = os.path.join(self.hl, game, "maps", name + ".bsp")
            try:
                with open(path, "rb") as f:
                    header = f.read(12)
                    if len(header) < 12:
                        continue
                    offset, length = struct.unpack_from("<ii", header, 4)
                    f.seek(offset)
                    head = f.read(min(length, 4096)).decode("latin-1", "replace")
            except OSError:
                continue
            m = re.search(r'"message"\s+"([^"]*)"', head.split("}")[0])
            if not m:
                return ""
            title = re.sub(r"\\n.*|\s*\(press fire.*", "", m.group(1), flags=re.I).strip(" -")
            return "" if title.lower() == name.lower() else title[:60]
        return ""

    def map_sources(self):
        """Who made each map, from map_sources.json next to this file."""
        try:
            with open(os.path.join(HERE, "map_sources.json"), encoding="utf-8") as f:
                groups = json.load(f).get("groups", [])
        except (OSError, ValueError):
            return {}
        return {m.lower(): g["name"] for g in groups for m in g.get("maps", [])}

    # ---- health check ---------------------------------------------------
    def health(self, repair=True):
        """Checks that Metamod, AMX Mod X and the menu are wired up, fixing
        what a Steam update or file verification tends to undo."""
        checks, fixed = [], []
        am = os.path.join(self.cz, "addons", "amxmodx")

        def check(name, ok, detail=""):
            checks.append({"name": name, "ok": bool(ok), "detail": detail})

        liblist = os.path.join(self.cz, "liblist.gam")
        try:
            with open(liblist, encoding="latin-1", newline="") as f:
                text = f.read()
            m = re.search(r'^gamedll\s+"([^"]*)"', text, re.M)
            if m and m.group(1).lower() == METAMOD_DLL.lower():
                check("Game loads Metamod", True)
            elif m and repair:
                stamp = time.strftime("%Y%m%d-%H%M%S")
                with open(liblist + ".steam-" + stamp, "w", encoding="latin-1", newline="") as f:
                    f.write(text)
                text = text[:m.start(1)] + METAMOD_DLL + text[m.end(1):]
                with open(liblist, "w", encoding="latin-1", newline="") as f:
                    f.write(text)
                fixed.append("Steam had reset liblist.gam, so the mods weren't loading. Fixed it. Restart Condition Zero.")
                check("Game loads Metamod", True, "repaired")
            else:
                check("Game loads Metamod", False, "liblist.gam doesn't point at Metamod")
        except OSError:
            check("Game loads Metamod", False, "liblist.gam is missing")

        check("Metamod installed", os.path.exists(os.path.join(self.cz, "addons", "metamod", "dlls", "metamod.dll")))
        mm_ini = os.path.join(self.cz, "addons", "metamod", "plugins.ini")
        mm_text = open(mm_ini, encoding="latin-1").read() if os.path.exists(mm_ini) else ""
        if "amxmodx_mm.dll" in mm_text.lower():
            check("Metamod loads AMX Mod X", True)
        elif repair and os.path.isdir(os.path.dirname(mm_ini)):
            with open(mm_ini, "w", encoding="latin-1", newline="") as f:
                f.write("win32 addons\\amxmodx\\dlls\\amxmodx_mm.dll\r\n")
            fixed.append("Metamod's plugin list was missing AMX Mod X. Fixed it. Restart Condition Zero.")
            check("Metamod loads AMX Mod X", True, "repaired")
        else:
            check("Metamod loads AMX Mod X", False)
        check("AMX Mod X installed", os.path.exists(os.path.join(am, "dlls", "amxmodx_mm.dll")))
        check("Game Mode Menu installed", os.path.exists(os.path.join(am, "plugins", "gamemodes.amxx")))

        pl_ini = os.path.join(am, "configs", "plugins.ini")
        pl_text = open(pl_ini, encoding="latin-1").read() if os.path.exists(pl_ini) else ""
        if re.search(r"^\s*gamemodes\.amxx", pl_text, re.M):
            check("Game Mode Menu enabled", True)
        elif repair and pl_text:
            with open(pl_ini, "a", encoding="latin-1", newline="") as f:
                f.write("\r\ngamemodes.amxx\t\t; Game Mode Menu\r\n")
            fixed.append("The Game Mode Menu had been removed from plugins.ini. Added it back. Restart the map.")
            check("Game Mode Menu enabled", True, "repaired")
        else:
            check("Game Mode Menu enabled", False)

        cfg = os.path.join(self.cz, "config.cfg")
        binds = open(cfg, encoding="latin-1").read() if os.path.exists(cfg) else ""
        check("F3 and F4 keys bound", '"gamemenu"' in binds and '"gamequick"' in binds,
              "" if '"gamemenu"' in binds else 'type  bind F3 gamemenu  in the console')
        return {"ok": all(c["ok"] for c in checks), "checks": checks, "fixed": fixed}

    def thumb_source(self, name):
        """('shot' | 'radar' | None, path) for a map's picture."""
        for game in ("czero", "cstrike"):
            for sub in ("gfx/thumbnails/maps_wide", "gfx/thumbnails/maps"):
                p = self._find(os.path.join(self.hl, game, sub), name, (".tga", ".bmp"))
                if p:
                    return "shot", p
        # a GameBanana screenshot (saved or looked up) beats the radar overview
        shot = self.store.shot_for(name) or self.store.find_shot(name)
        if shot:
            return "shot", shot
        for game in ("czero", "cstrike"):
            p = self._find(os.path.join(self.hl, game, "overviews"), name, (".bmp", ".tga"))
            if p:
                return "radar", p
        return None, None

    @staticmethod
    def _find(directory, name, exts):
        if not os.path.isdir(directory):
            return None
        want = {name.lower() + e for e in exts}
        for f in os.listdir(directory):
            if f.lower() in want:
                return os.path.join(directory, f)
        return None

    def thumbnail(self, name):
        kind, src = self.thumb_source(name)
        if not src:
            return None
        out = os.path.join(self.cache, "%s-%s.png" % (kind, name.lower()))
        if not os.path.exists(out) or os.path.getmtime(out) < os.path.getmtime(src):
            args = ["sips", "-s", "format", "png", src, "--out", out]
            if kind == "radar":
                args[1:1] = ["-Z", "512"]
            subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        return out if os.path.exists(out) else None


def make_handler(game, token, hosts):
    index = os.path.join(HERE, "web", "index.html")

    class Handler(BaseHTTPRequestHandler):
        server_version = "CZPanel/1.0"

        def log_message(self, fmt, *args):
            pass

        def _authorized(self):
            # A web page can point its own domain name at 127.0.0.1 ("DNS rebinding");
            # the browser then still sends that name, so only our own addresses get in.
            if (self.headers.get("Host") or "").lower() not in hosts:
                return False
            if token is None:
                return True
            q = parse_qs(urlparse(self.path).query)
            if q.get("t", [""])[0] == token:
                return True
            cookie = self.headers.get("Cookie", "")
            return ("czpanel=" + token) in cookie

        def _send(self, code, body, ctype="application/json", extra=None):
            if isinstance(body, (dict, list)):
                body = json.dumps(body).encode()
            elif isinstance(body, str):
                body = body.encode()
            self.send_response(code)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Cache-Control", "max-age=3600" if ctype == "image/png" else "no-store")
            for k, v in (extra or {}).items():
                self.send_header(k, v)
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if not self._authorized():
                return self._send(HTTPStatus.FORBIDDEN, "Open the link printed by panel.py.", "text/plain")
            url = urlparse(self.path)
            if url.path in ("/", "/index.html"):
                extra = {"Set-Cookie": "czpanel=%s; Path=/; SameSite=Strict" % token} if token else None
                with open(index, "rb") as f:
                    return self._send(200, f.read(), "text/html; charset=utf-8", extra)
            if url.path == "/api/state":
                return self._send(200, game.state())
            if url.path == "/api/share":
                return self._send(200, game.sharing.status())
            if url.path == "/api/health":
                return self._send(200, game.health())
            if url.path == "/api/maps":
                refresh = "refresh" in parse_qs(url.query)
                return self._send(200, game.maps(refresh))
            if url.path.startswith("/api/gb/"):
                return self._gb_get(url)
            m = re.match(r"^/thumb/([A-Za-z0-9_.\-]{1,64})\.png$", url.path)
            if m:
                p = game.thumbnail(m.group(1))
                if not p:
                    return self._send(404, "no picture", "text/plain")
                with open(p, "rb") as f:
                    return self._send(200, f.read(), "image/png")
            self._send(404, {"error": "not found"})

        def _gb_get(self, url):
            q = {k: v[0] for k, v in parse_qs(url.query).items()}
            try:
                if url.path == "/api/gb/browse":
                    return self._send(200, game.store.browse(int(q.get("cat", 5474)), q.get("sort", "popular"),
                                                             q.get("q", ""), int(q.get("page", 1))))
                if url.path == "/api/gb/jobs":
                    return self._send(200, game.store.job_list())
                m = re.match(r"^/api/gb/mod/(\d{1,9})$", url.path)
                if m:
                    return self._send(200, game.store.details(m.group(1)))
            except ValueError as e:
                return self._send(400, {"error": str(e)})
            except OSError:
                return self._send(502, {"error": "Couldn't reach GameBanana. Check your internet connection."})
            self._send(404, {"error": "not found"})

        def do_POST(self):
            if not self._authorized():
                return self._send(HTTPStatus.FORBIDDEN, {"error": "forbidden"})
            # Only the panel sends this header; other websites can't add it without
            # a CORS preflight, which this server never answers.
            if self.headers.get("X-Panel") != "1":
                return self._send(HTTPStatus.FORBIDDEN, {"error": "forbidden"})
            path = urlparse(self.path).path
            if path not in ("/api/cmd", "/api/mapflag", "/api/gb/install", "/api/gb/uninstall", "/api/share"):
                return self._send(404, {"error": "not found"})
            try:
                length = min(int(self.headers.get("Content-Length", 0)), 4096)
                req = json.loads(self.rfile.read(length) or b"{}")
                if path == "/api/gb/install":
                    return self._send(200, {"ok": True, "job": game.store.start_install(req.get("mod"), req.get("file"))})
                if path == "/api/gb/uninstall":
                    return self._send(200, {"ok": True, "maps": game.store.uninstall(req.get("mod"))})
                if path == "/api/share":
                    sh, action = game.sharing, req.get("action")
                    if action == "server":
                        sh.set_server(str(req.get("name", "")), str(req.get("password", "")))
                    elif action == "method":
                        sh.set_method(str(req.get("method", "")))
                    elif action in ("connect", "cancel", "disconnect", "start", "stop"):
                        getattr(sh, action)()
                    else:
                        raise ValueError("unknown action")
                    return self._send(200, dict(sh.status(), ok=True))
                if path == "/api/mapflag":
                    game.set_flag(str(req.get("flag", "")), str(req.get("map", "")), bool(req.get("on")))
                    return self._send(200, {"ok": True})
                lines = game.command(str(req.get("cmd", "")), [str(a) for a in req.get("args", [])])
                self._send(200, {"ok": True, "output": lines})
            except ValueError as e:
                self._send(400, {"ok": False, "error": str(e)})
            except PermissionError as e:
                self._send(503, {"ok": False, "error": str(e)})
            except (OSError, ConnectionError):
                self._send(503, {"ok": False, "error": "Condition Zero isn't running, or no match is loaded."})

    return Handler


def lan_ip():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(("10.255.255.255", 1))
        return s.getsockname()[0]
    except OSError:
        return "127.0.0.1"
    finally:
        s.close()


def main():
    ap = argparse.ArgumentParser(description="CZ Control Panel")
    ap.add_argument("--lan", action="store_true", help="allow other devices on your network (phone)")
    ap.add_argument("--port", type=int, default=8027)
    ap.add_argument("--game-dir", default=os.environ.get("CZ_HL_DIR", DEFAULT_HL),
                    help="the Half-Life folder that contains czero or cstrike")
    ap.add_argument("--game", choices=("czero", "cstrike"), default="czero",
                    help="Condition Zero (czero) or Counter-Strike 1.6 (cstrike)")
    ap.add_argument("--no-browser", action="store_true")
    args = ap.parse_args()

    if not os.path.isdir(os.path.join(args.game_dir, args.game)):
        sys.exit("Couldn't find %s in %s (use --game-dir)" % (args.game, args.game_dir))

    game = Game(args.game_dir, args.game)
    token = secrets.token_urlsafe(12) if args.lan else None
    host = "0.0.0.0" if args.lan else "127.0.0.1"
    try:
        hosts = {"%s:%d" % (h, args.port) for h in ("127.0.0.1", "localhost")}
        if args.lan:
            hosts.add("%s:%d" % (lan_ip(), args.port))
        server = ThreadingHTTPServer((host, args.port), make_handler(game, token, hosts))
    except OSError:
        url = "http://127.0.0.1:%d/" % args.port
        print("The panel is already running (it may have started at login): %s" % url)
        if not args.no_browser:
            subprocess.run(["open", url], check=False)
        return

    report = game.health()
    for line in report["fixed"]:
        print("Repaired: " + line)

    local = "http://127.0.0.1:%d/" % args.port
    print("CZ Control Panel running at %s" % local)
    if token:
        print("On your phone (same Wi-Fi):  http://%s:%d/?t=%s" % (lan_ip(), args.port, token))
        local += "?t=" + token
    print("Press Ctrl+C to stop.")
    if not args.no_browser:
        subprocess.run(["open", local], check=False)
    # playit only runs while the panel does
    def quit_(*_):
        raise KeyboardInterrupt
    signal.signal(signal.SIGTERM, quit_)
    signal.signal(signal.SIGHUP, quit_)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        game.sharing.stop()


if __name__ == "__main__":
    main()
