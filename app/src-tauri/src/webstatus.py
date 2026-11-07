#!/usr/bin/env python3
# Loner-Strike website status: tells a website this game server's name, map and players,
# and can count visits. Written by the Loner-Strike app (changes here are replaced).
#
# Listens on 127.0.0.1 only; Caddy or a Cloudflare Tunnel in front gives it HTTPS.
#   GET /status  {"name", "map", "players", "bots", "max_players", "online": true}
#                or {"online": false} when the game doesn't answer
#   GET /visit   adds one and answers {"count": n} (only with --counter)
# Nothing else is served. No IP addresses are kept or logged, no cookies are set.
import argparse
import json
import os
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HEADER = b"\xff\xff\xff\xff"
QUERY = HEADER + b"TSource Engine Query\x00"
# how long an answer from the game is reused
CACHE_SECS = 10
# visits counted per second at most (all visitors together); more are answered, not counted
MAX_VISITS_PER_SEC = 20


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
    a = p.parse_args()
    origins = {o.strip().rstrip("/").lower() for o in a.origins.split(",") if o.strip()}
    counter = Counter(a.counter) if a.counter else None
    server = ThreadingHTTPServer(("127.0.0.1", a.listen), make_handler(Status(a.game), counter, origins))
    server.daemon_threads = True
    server.serve_forever()


if __name__ == "__main__":
    main()
