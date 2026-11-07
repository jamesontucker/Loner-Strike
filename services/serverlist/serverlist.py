#!/usr/bin/env python3
# Loner-Strike server list: the game servers Loner-Strike hosts chose to share, for the
# app's server browser (and the website).
#
# A server is on the list while it answers and its public ls_list setting is 1. The app
# sets that when the host turns on "Show on the Loner-Strike server list", so only someone
# who runs a server can list it, and turning it off takes it off the list. This service
# checks every server itself every couple of minutes; it never takes a host's word for
# the name, map or players.
#
# Listens on 127.0.0.1 only; a Cloudflare Tunnel (or Caddy) in front gives it HTTPS.
#   GET  /servers    {"servers": [...], "updated": <unix time>}, anyone may read it
#   POST /register   {"address": "host[:port]"}: checks the server now and lists it
# Nothing else is served. No visitor addresses are kept or logged beyond a short
# in-memory count for the rate limit.
import argparse
import ipaddress
import json
import os
import socket
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HEADER = b"\xff\xff\xff\xff"
SPLIT = b"\xfe\xff\xff\xff"
INFO = HEADER + b"TSource Engine Query\x00"
RULES = HEADER + b"V"
# how often every listed server is asked again
POLL_SECS = 120
# shown while it answered this recently
FRESH_SECS = 600
# taken off the list after not answering this long
GONE_SECS = 6 * 3600
MAX_SERVERS = 500
# sign-ups per visitor per hour
MAX_REGISTER = 20
DEFAULT_PORT = 27015


# ---- asking a game server (A2S, over UDP) ------------------------------------------------

def _text(raw):
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        return raw.decode("latin-1")


def _cstr(data, i):
    end = data.index(b"\x00", i)
    return _text(data[i:end]), end + 1


def parse_info(data):
    """An A2S_INFO reply (after the FF FF FF FF header) as a dict, or None. 'I' is the
    reply HLDS gives today; 'm' is GoldSrc's old one."""
    if not data:
        return None
    try:
        if data[0] == 0x49:
            i = 2
            name, i = _cstr(data, i)
            map_, i = _cstr(data, i)
            folder, i = _cstr(data, i)
            _, i = _cstr(data, i)
            i += 2
            players, max_players, bots = data[i], data[i + 1], data[i + 2]
            password, secure = data[i + 5], data[i + 6]
        elif data[0] == 0x6D:
            i = 1
            _, i = _cstr(data, i)
            name, i = _cstr(data, i)
            map_, i = _cstr(data, i)
            folder, i = _cstr(data, i)
            _, i = _cstr(data, i)
            players, max_players = data[i], data[i + 1]
            password = data[i + 5]
            j = i + 6
            if data[j] == 1:
                _, j = _cstr(data, j + 1)
                _, j = _cstr(data, j)
                j += 1 + 4 + 4 + 1 + 1
            else:
                j += 1
            secure, bots = data[j], data[j + 1]
        else:
            return None
    except (IndexError, ValueError):
        return None
    return {"name": name, "map": map_, "game": folder, "players": players, "bots": min(bots, players),
            "max_players": max_players, "password": bool(password), "secure": bool(secure)}


def parse_rules(data):
    """An A2S_RULES reply ('E', after the header) as a dict of setting: value, or None."""
    if not data or data[0] != 0x45:
        return None
    try:
        count = int.from_bytes(data[1:3], "little")
        out, i = {}, 3
        for _ in range(count):
            k, i = _cstr(data, i)
            v, i = _cstr(data, i)
            out[k] = v
        return out
    except (IndexError, ValueError):
        return None


def _receive(s):
    """One reply, put back together when GoldSrc split it over several packets."""
    data = s.recv(4096)
    if data[:4] != SPLIT:
        return data
    parts, total, rid = {}, None, data[4:8]
    while True:
        if data[:4] != SPLIT or data[4:8] != rid or len(data) < 9:
            return None
        # GoldSrc: the high four bits are this packet's number, the low four how many
        number, total = data[8] >> 4, data[8] & 0x0F
        parts[number] = data[9:]
        if total and len(parts) >= total:
            break
        data = s.recv(4096)
    if any(n not in parts for n in range(total)):
        return None
    return b"".join(parts[n] for n in range(total))


def ask(address, request, parse, timeout=2.0):
    """Sends an A2S request (answering a challenge if asked for one); the parsed reply, or None."""
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(timeout)
    try:
        s.connect(address)
        challenge = HEADER if request == RULES else b""
        for _ in range(3):
            s.send(request + challenge)
            data = _receive(s)
            if not data or data[:4] != HEADER:
                return None
            body = data[4:]
            if body[:1] == b"A" and len(body) >= 5:
                challenge = body[1:5]
                continue
            return parse(body)
    except OSError:
        return None
    finally:
        s.close()
    return None


# ---- addresses ----------------------------------------------------------------------------

def clean_address(text):
    """("host:port" as listed, (ip, port) to ask) for an address on the internet, or None.
    Private, local and other non-public addresses are refused."""
    text = (text or "").strip().lower()
    if not text or len(text) > 100:
        return None
    host, _, port = text.rpartition(":") if ":" in text else (text, "", str(DEFAULT_PORT))
    try:
        port = int(port)
    except ValueError:
        return None
    if not 1 <= port <= 65535 or not host or not all(c.isalnum() or c in ".-" for c in host):
        return None
    try:
        ip = socket.getaddrinfo(host, port, socket.AF_INET, socket.SOCK_DGRAM)[0][4][0]
    except (OSError, IndexError):
        return None
    if not ipaddress.ip_address(ip).is_global:
        return None
    return f"{host}:{port}", (ip, port)


def check(listed):
    """Asks a server for its details and its ls_list setting: (info or None, rules or None)."""
    found = clean_address(listed)
    if not found:
        return None, None
    _, addr = found
    info = ask(addr, INFO, parse_info)
    rules = ask(addr, RULES, parse_rules) if info else None
    return info, rules


# ---- the list -----------------------------------------------------------------------------

class Registry:
    def __init__(self, path):
        self.path = path
        self.lock = threading.Lock()
        self.servers = {}
        try:
            with open(path) as f:
                self.servers = json.load(f).get("servers", {})
        except (OSError, ValueError):
            pass

    def _save(self):
        tmp = self.path + ".tmp"
        try:
            with open(tmp, "w") as f:
                json.dump({"servers": self.servers}, f)
            os.replace(tmp, self.path)
        except OSError:
            pass

    def update(self, listed, info, rules, now):
        """Takes in what a server said. True while it's on the list."""
        with self.lock:
            entry = self.servers.get(listed)
            if info and rules is not None and rules.get("ls_list") != "1":
                # the host turned the list off (or never turned it on)
                if entry:
                    del self.servers[listed]
                    self._save()
                return False
            if info and rules is not None:
                if not entry:
                    if len(self.servers) >= MAX_SERVERS:
                        return False
                    entry = {"since": now}
                    self.servers[listed] = entry
                entry.update(info=info, seen=now)
                self._save()
                return True
            if entry and now - entry.get("seen", 0) > GONE_SECS:
                del self.servers[listed]
                self._save()
            return False

    def public(self, now):
        with self.lock:
            out = [dict(e["info"], address=a, since=e["since"]) for a, e in self.servers.items() if now - e.get("seen", 0) <= FRESH_SECS]
        out.sort(key=lambda s: (-(s["players"] - s["bots"]), -s["players"], s["name"].lower()))
        return out

    def addresses(self):
        with self.lock:
            return list(self.servers)

    def poll(self):
        while True:
            started = time.time()
            with ThreadPoolExecutor(max_workers=16) as pool:
                for listed, (info, rules) in zip(self.addresses(), pool.map(check, self.addresses())):
                    self.update(listed, info, rules, time.time())
            time.sleep(max(5, POLL_SECS - (time.time() - started)))


class RateLimit:
    def __init__(self, per_hour):
        self.per_hour = per_hour
        self.lock = threading.Lock()
        self.hits = {}

    def allow(self, who, now):
        with self.lock:
            recent = [t for t in self.hits.get(who, []) if now - t < 3600]
            if len(recent) >= self.per_hour:
                self.hits[who] = recent
                return False
            recent.append(now)
            self.hits[who] = recent
            # forget visitors from more than an hour ago
            if len(self.hits) > 10000:
                self.hits = {k: v for k, v in self.hits.items() if v and now - v[-1] < 3600}
            return True


def make_handler(registry, limit, asker=check):
    class Handler(BaseHTTPRequestHandler):
        server_version = "list"
        sys_version = ""
        timeout = 10

        def log_message(self, *args):
            pass

        def _send(self, code, body, cache="no-store"):
            data = json.dumps(body).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", cache)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(data)

        def _visitor(self):
            # behind Cloudflare or Caddy on this machine: the visitor is in a header
            peer = self.client_address[0]
            if peer == "127.0.0.1":
                return self.headers.get("CF-Connecting-IP") or (self.headers.get("X-Forwarded-For") or peer).split(",")[0].strip()
            return peer

        def do_GET(self):
            if self.path.split("?", 1)[0] == "/servers":
                now = time.time()
                self._send(200, {"servers": registry.public(now), "updated": int(now)}, "public, max-age=30")
            else:
                self._send(404, {"error": "not found"})

        do_HEAD = do_GET

        def do_POST(self):
            if self.path.split("?", 1)[0] != "/register":
                return self._send(404, {"error": "not found"})
            if not limit.allow(self._visitor(), time.time()):
                return self._send(429, {"error": "Too many sign-ups from here. Try again in an hour."})
            try:
                size = int(self.headers.get("Content-Length") or 0)
                body = json.loads(self.rfile.read(min(size, 1024)) or b"{}") if 0 < size <= 1024 else {}
            except ValueError:
                body = {}
            found = clean_address(body.get("address") if isinstance(body, dict) else None)
            if not found:
                return self._send(400, {"error": "That isn't an address on the internet, like play.example.com:27015."})
            listed = found[0]
            info, rules = asker(listed)
            if not info:
                return self._send(200, {"ok": False, "error": "The server didn't answer from the internet. Check it's running and its port is open."})
            if rules is None:
                return self._send(200, {"ok": False, "error": "The server answered but wouldn't say its settings. Try again in a minute."})
            if rules.get("ls_list") != "1":
                return self._send(200, {"ok": False, "error": "The server isn't set to be listed yet. Its change takes effect from the next map."})
            if not registry.update(listed, info, rules, time.time()):
                return self._send(200, {"ok": False, "error": "The list is full right now."})
            self._send(200, {"ok": True, "address": listed})

        def do_OPTIONS(self):
            self.send_response(204)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "GET")
            self.send_header("Access-Control-Max-Age", "86400")
            self.send_header("Content-Length", "0")
            self.end_headers()

    return Handler


def main():
    p = argparse.ArgumentParser(description="Loner-Strike server list")
    p.add_argument("--listen", type=int, default=8100, help="port on 127.0.0.1")
    p.add_argument("--state", required=True, help="file that keeps the list between restarts")
    a = p.parse_args()
    registry = Registry(a.state)
    threading.Thread(target=registry.poll, daemon=True).start()
    server = ThreadingHTTPServer(("127.0.0.1", a.listen), make_handler(registry, RateLimit(MAX_REGISTER)))
    server.daemon_threads = True
    server.serve_forever()


if __name__ == "__main__":
    main()
