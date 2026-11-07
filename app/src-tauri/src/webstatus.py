#!/usr/bin/env python3
# Loner-Strike website status: tells a website this game server's name, map and players,
# and can count visits. Written by the Loner-Strike app (changes here are replaced).
#
# Listens on 127.0.0.1 only; Caddy or a Cloudflare Tunnel in front gives it HTTPS.
#   GET /status  {"name", "map", "players", "bots", "max_players", "online": true}
#                or {"online": false} when the game doesn't answer
#   GET /visit   adds one and answers {"count": n} (only with --counter)
# Nothing else is served. No IP addresses are kept or logged, no cookies are set.
#
# With Discord webhooks (systemd credentials "discord-live" and "discord-busy"): keeps one
# message up to date with the server's status, and says when people come on to play.
import argparse
import json
import os
import socket
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HEADER = b"\xff\xff\xff\xff"
QUERY = HEADER + b"TSource Engine Query\x00"
# how long an answer from the game is reused
CACHE_SECS = 10
# visits counted per second at most (all visitors together); more are answered, not counted
MAX_VISITS_PER_SEC = 20
# how often the Discord status message is brought up to date
DISCORD_SECS = 60
# "people are on" at most this often
BUSY_GAP_SECS = 3600


def _text(raw):
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        return raw.decode("latin-1")


def _cstr(data, i):
    end = data.index(b"\x00", i)
    return _text(data[i:end]), end + 1


def parse_info(data):
    """An A2S_INFO reply (after the 4-byte FF FF FF FF header) as a dict, or None.

    'I' (0x49) is the reply HLDS gives today; 'm' (0x6D) is GoldSrc's old one."""
    if not data:
        return None
    kind = data[0]
    try:
        if kind == 0x49:
            # protocol, name, map, folder, game, app id (short), players, max, bots
            i = 2
            name, i = _cstr(data, i)
            map_, i = _cstr(data, i)
            _, i = _cstr(data, i)
            _, i = _cstr(data, i)
            i += 2
            players, max_players = data[i], data[i + 1]
            bots = data[i + 2] if len(data) > i + 2 else 0
        elif kind == 0x6D:
            # address, name, map, folder, game, players, max, protocol, type, os,
            # password, mod (and its details when 1), secure, bots
            i = 1
            _, i = _cstr(data, i)
            name, i = _cstr(data, i)
            map_, i = _cstr(data, i)
            _, i = _cstr(data, i)
            _, i = _cstr(data, i)
            players, max_players = data[i], data[i + 1]
            bots = 0
            try:
                j = i + 6
                if data[i + 6] == 1:
                    _, j = _cstr(data, j + 1)
                    _, j = _cstr(data, j)
                    j += 1 + 4 + 4 + 1 + 1
                else:
                    j += 1
                bots = data[j + 1]
            except (IndexError, ValueError):
                bots = 0
        else:
            return None
    except (IndexError, ValueError):
        return None
    bots = min(bots, players)
    return {"name": name, "map": map_, "players": players, "bots": bots, "max_players": max_players, "online": True}


def query(port, timeout=1.5):
    """Asks the game on this machine; None if it doesn't answer."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(timeout)
    try:
        s.connect(("127.0.0.1", port))
        req = QUERY
        for _ in range(3):
            s.send(req)
            data = s.recv(4096)
            if data[:4] != HEADER:
                return None
            body = data[4:]
            # a challenge (S2C_CHALLENGE): ask again with it
            if body[:1] == b"A" and len(body) >= 5:
                req = QUERY + body[1:5]
                continue
            return parse_info(body)
    except OSError:
        return None
    finally:
        s.close()
    return None


class Status:
    def __init__(self, port):
        self.port = port
        self.lock = threading.Lock()
        self.at = None
        self.value = {"online": False}

    def get(self):
        with self.lock:
            now = time.monotonic()
            if self.at is None or now - self.at >= CACHE_SECS:
                self.value = query(self.port) or {"online": False}
                self.at = time.monotonic()
            return self.value


class Counter:
    def __init__(self, path):
        self.path = path
        self.lock = threading.Lock()
        self.second = None
        self.in_second = 0
        try:
            with open(path) as f:
                self.count = max(0, int(f.read().strip() or 0))
        except (OSError, ValueError):
            self.count = 0

    def hit(self):
        with self.lock:
            sec = int(time.monotonic())
            if sec != self.second:
                self.second, self.in_second = sec, 0
            if self.in_second < MAX_VISITS_PER_SEC:
                self.in_second += 1
                self.count += 1
                self._save()
            return self.count

    def _save(self):
        tmp = self.path + ".tmp"
        try:
            with open(tmp, "w") as f:
                f.write(str(self.count))
                f.flush()
                os.fsync(f.fileno())
            os.replace(tmp, self.path)
        except OSError:
            pass


def _credential(name):
    """A systemd credential's text (LoadCredential=), or "" without it."""
    folder = os.environ.get("CREDENTIALS_DIRECTORY")
    if not folder:
        return ""
    try:
        with open(os.path.join(folder, name)) as f:
            return f.read().strip()
    except OSError:
        return ""


def _discord(method, url, body=None):
    """A call to a Discord webhook: (status, answer as a dict)."""
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    # Discord turns away requests without a User-Agent of this shape
    req.add_header("User-Agent", "DiscordBot (https://lonerstrike.com, 1)")
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        return e.code, {}
    except (OSError, ValueError):
        return 0, {}


def _people(n):
    return "1 person" if n == 1 else f"{n} people"


def status_message(info, address):
    """The live status message's embed for an A2S answer (or {"online": False})."""
    join = f"`connect {address}`" if address else ""
    if not info.get("online"):
        embed = {"title": "Server offline", "description": "The server isn't answering right now.", "color": 0x6B6B6B}
    else:
        people, bots = info["players"] - info["bots"], info["bots"]
        if people:
            text = f"**{_people(people)}** and {bots} bots playing **{info['map']}**."
        elif bots:
            text = f"Only bots right now, {bots} of them on **{info['map']}**. Join and take one's place."
        else:
            text = f"Empty right now, on **{info['map']}**."
        embed = {"title": info["name"][:256], "description": text, "color": 0x8FA24A if people else 0x4F6B3A}
        embed["fields"] = [{"name": "Players", "value": f"{info['players']}/{info['max_players']}", "inline": True}]
        if join:
            embed["fields"].append({"name": "Join", "value": join, "inline": True})
    embed["footer"] = {"text": "Updated every minute"}
    embed["timestamp"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    return {"embeds": [embed], "allowed_mentions": {"parse": []}}


def busy_message(info, address):
    """What's said when people come on to play."""
    people = info["players"] - info["bots"]
    verb = "is" if people == 1 else "are"
    text = f"{_people(people)} {verb} playing on **{info['map']}** right now."
    if address:
        text += f" Join with `connect {address}`."
    return {"content": text, "allowed_mentions": {"parse": []}}


class DiscordStatus:
    """Keeps the live message up to date and says when people come on. Its message id and
    when it last said so are kept in `state` (JSON), so a restart edits the same message."""

    def __init__(self, status, live, busy, address, state, busy_at):
        self.status, self.live, self.busy, self.address = status, live, busy, address
        self.state, self.busy_at = state, busy_at
        self.saved = {}
        try:
            with open(state) as f:
                self.saved = json.load(f)
        except (OSError, ValueError):
            pass
        self.was_busy = True  # nothing is said for people already on when it starts

    def _save(self):
        tmp = self.state + ".tmp"
        try:
            with open(tmp, "w") as f:
                json.dump(self.saved, f)
            os.replace(tmp, self.state)
        except OSError:
            pass

    def tick(self, now):
        info = self.status.get()
        if self.live:
            body = status_message(info, self.address)
            mid = self.saved.get("message")
            code = _discord("PATCH", f"{self.live}/messages/{mid}", body)[0] if mid else 404
            # never posted, or deleted since: a new one
            if code == 404:
                code, answer = _discord("POST", self.live + "?wait=true", body)
                if code == 200 and answer.get("id"):
                    self.saved["message"] = answer["id"]
                    self._save()
        if self.busy:
            busy = info.get("online") and info["players"] - info["bots"] >= self.busy_at
            if busy and not self.was_busy and ("busy_said" not in self.saved or now - self.saved["busy_said"] >= BUSY_GAP_SECS):
                if _discord("POST", self.busy, busy_message(info, self.address))[0] in (200, 204):
                    self.saved["busy_said"] = now
                    self._save()
            self.was_busy = bool(busy)

    def run(self):
        while True:
            try:
                self.tick(time.time())
            except Exception:
                pass  # Discord or the network having a moment: next minute
            time.sleep(DISCORD_SECS)


def origin_allowed(origin, allowed):
    """Whether a site may read the answers: an exact entry, or an https://*.suffix entry
    matching any subdomain (one or more labels) of that suffix over https."""
    if not origin or origin in allowed:
        return bool(origin)
    if not origin.startswith("https://"):
        return False
    rest = origin[len("https://"):]
    host, _, port = rest.partition(":")
    for a in allowed:
        if not a.startswith("https://*."):
            continue
        suffix, _, aport = a[len("https://*."):].partition(":")
        if port == aport and host.endswith("." + suffix) and len(host) > len(suffix) + 1 and "/" not in rest:
            return True
    return False


def make_handler(status, counter, origins):
    class Handler(BaseHTTPRequestHandler):
        server_version = "status"
        sys_version = ""
        timeout = 10

        def log_message(self, *args):
            # nothing logged: no visitor addresses kept, not even in the journal
            pass

        def _cors(self):
            origin = (self.headers.get("Origin") or "").rstrip("/").lower()
            if origin_allowed(origin, origins):
                self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")

        def _send(self, code, body, cache):
            data = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", cache)
            self.send_header("X-Content-Type-Options", "nosniff")
            self._cors()
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(data)

        def do_GET(self):
            path = self.path.split("?", 1)[0]
            if path == "/status":
                self._send(200, status.get(), "max-age=10")
            elif path == "/visit" and counter is not None:
                self._send(200, {"count": counter.hit()}, "no-store")
            else:
                self._send(404, {"error": "not found"}, "no-store")

        def do_HEAD(self):
            path = self.path.split("?", 1)[0]
            if path == "/status":
                self._send(200, status.get(), "max-age=10")
            else:
                self._send(404, {"error": "not found"}, "no-store")

        def do_OPTIONS(self):
            self.send_response(204)
            self._cors()
            self.send_header("Access-Control-Allow-Methods", "GET")
            self.send_header("Access-Control-Max-Age", "86400")
            self.send_header("Content-Length", "0")
            self.end_headers()

    return Handler


def main():
    p = argparse.ArgumentParser(description="Loner-Strike website status")
    p.add_argument("--listen", type=int, required=True, help="port on 127.0.0.1")
    p.add_argument("--game", type=int, required=True, help="the game server's port on this machine")
    p.add_argument("--origins", default="", help="sites allowed to read the answers, comma separated")
    p.add_argument("--counter", default="", help="file that keeps the visit count (no counter without it)")
    p.add_argument("--address", default="", help="the server's public address, for joining from Discord")
    p.add_argument("--discord-state", default="", help="file that keeps the Discord message id")
    p.add_argument("--busy-at", type=int, default=1, help="people (not bots) on before Discord is told")
    a = p.parse_args()
    origins = {o.strip().rstrip("/").lower() for o in a.origins.split(",") if o.strip()}
    counter = Counter(a.counter) if a.counter else None
    status = Status(a.game)
    live, busy = _credential("discord-live"), _credential("discord-busy")
    if (live or busy) and a.discord_state:
        discord = DiscordStatus(status, live, busy, a.address, a.discord_state, max(1, a.busy_at))
        threading.Thread(target=discord.run, daemon=True).start()
    server = ThreadingHTTPServer(("127.0.0.1", a.listen), make_handler(status, counter, origins))
    server.daemon_threads = True
    server.serve_forever()


if __name__ == "__main__":
    main()
