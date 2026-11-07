#!/usr/bin/env python3
# Tests for the server list: python3 -m unittest services/serverlist/test_serverlist.py
import json
import os
import socket
import sys
import tempfile
import threading
import unittest
import urllib.request
from http.server import ThreadingHTTPServer

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import serverlist as sl  # noqa: E402

H = sl.HEADER
INFO_BODY = b"I\x30Pub \xc3\xa9\x00de_dust2\x00cstrike\x00Counter-Strike\x00\x0a\x00\x05\x20\x03dl\x01\x01\x01"


def rules_body(rules):
    out = b"E" + len(rules).to_bytes(2, "little")
    for k, v in rules.items():
        out += k.encode() + b"\x00" + v.encode() + b"\x00"
    return out


class FakeServer:
    """A GoldSrc server on 127.0.0.1 that asks for a challenge and splits its rules reply
    over three packets, sent out of order."""

    def __init__(self, rules):
        self.rules = rules
        self.s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.s.bind(("127.0.0.1", 0))
        self.addr = self.s.getsockname()
        self.s.settimeout(0.2)
        self.on = True
        threading.Thread(target=self.run, daemon=True).start()

    def run(self):
        while self.on:
            try:
                data, peer = self.s.recvfrom(1500)
            except OSError:
                continue
            if data.startswith(sl.INFO):
                if data[len(sl.INFO):] != b"CHAL":
                    self.s.sendto(H + b"ACHAL", peer)
                else:
                    self.s.sendto(H + INFO_BODY, peer)
            elif data.startswith(sl.RULES):
                if data[5:] != b"CHAL":
                    self.s.sendto(H + b"ACHAL", peer)
                    continue
                whole = H + rules_body(self.rules)
                third = len(whole) // 3 + 1
                parts = [whole[i * third:(i + 1) * third] for i in range(3)]
                for n in (2, 0, 1):
                    self.s.sendto(sl.SPLIT + b"\x07\x00\x00\x00" + bytes([(n << 4) | 3]) + parts[n], peer)

    def stop(self):
        self.on = False
        self.s.close()


class Tests(unittest.TestCase):
    def test_asks_with_challenges_and_split_replies(self):
        many = {f"mp_setting{i}": str(i) for i in range(80)}
        many["ls_list"] = "1"
        f = FakeServer(many)
        try:
            info = sl.ask(f.addr, sl.INFO, sl.parse_info)
            self.assertEqual(info, {"name": "Pub é", "map": "de_dust2", "game": "cstrike", "players": 5, "bots": 3, "max_players": 32, "password": True, "secure": True})
            rules = sl.ask(f.addr, sl.RULES, sl.parse_rules)
            self.assertEqual(rules["ls_list"], "1")
            self.assertEqual(len(rules), 81)
        finally:
            f.stop()

    def test_nothing_answering(self):
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.bind(("127.0.0.1", 0))
        addr = s.getsockname()
        try:
            self.assertIsNone(sl.ask(addr, sl.INFO, sl.parse_info, timeout=0.3))
        finally:
            s.close()

    def test_addresses(self):
        self.assertEqual(sl.clean_address("8.8.8.8"), ("8.8.8.8:27015", ("8.8.8.8", 27015)))
        self.assertEqual(sl.clean_address(" 8.8.8.8:27016 ")[0], "8.8.8.8:27016")
        for bad in ["", "127.0.0.1:27015", "10.0.0.5", "192.168.1.2:27015", "169.254.1.1", "0.0.0.0", "8.8.8.8:0", "8.8.8.8:99999", "8.8.8.8:x", "a b:1", "x" * 101, "localhost:27015"]:
            self.assertIsNone(sl.clean_address(bad), bad)

    def test_the_list(self):
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        info = sl.parse_info(INFO_BODY)
        self.assertFalse(r.update("a:1", info, {"ls_list": "0"}, 1000), "not opted in")
        self.assertTrue(r.update("a:1", info, {"ls_list": "1"}, 1000))
        self.assertTrue(r.update("b:1", dict(info, players=9, bots=1), {"ls_list": "1"}, 1000))
        self.assertEqual([s["address"] for s in r.public(1100)], ["b:1", "a:1"], "most people first")
        self.assertFalse(r.update("a:1", None, None, 1200), "didn't answer this time")
        self.assertEqual(len(r.public(1200)), 2, "a missed answer isn't the end")
        self.assertEqual([s["address"] for s in r.public(1000 + sl.FRESH_SECS + 1)], [], "quiet too long: hidden")
        self.assertFalse(r.update("b:1", info, {"ls_list": "0"}, 1300))
        self.assertEqual(r.addresses(), ["a:1"], "the host turned it off")
        r.update("a:1", None, None, 1000 + sl.GONE_SECS + 1)
        self.assertEqual(r.addresses(), [], "gone for hours: off the list")
        again = sl.Registry(r.path)
        self.assertEqual(again.addresses(), [], "kept on disk")

    def test_rate_limit(self):
        lim = sl.RateLimit(2)
        self.assertTrue(lim.allow("x", 0) and lim.allow("x", 1))
        self.assertFalse(lim.allow("x", 2))
        self.assertTrue(lim.allow("y", 2))
        self.assertTrue(lim.allow("x", 3700), "an hour later")

    def test_http(self):
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        answers = {"8.8.8.8:27015": (sl.parse_info(INFO_BODY), {"ls_list": "1"}), "8.8.4.4:27015": (sl.parse_info(INFO_BODY), {})}
        srv = ThreadingHTTPServer(("127.0.0.1", 0), sl.make_handler(r, sl.RateLimit(3), lambda a: answers.get(a, (None, None))))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        base = f"http://127.0.0.1:{srv.server_address[1]}"

        def post(body):
            req = urllib.request.Request(base + "/register", data=json.dumps(body).encode(), method="POST")
            try:
                with urllib.request.urlopen(req) as res:
                    return res.status, json.load(res)
            except urllib.error.HTTPError as e:
                return e.code, json.load(e)

        try:
            self.assertEqual(post({"address": "8.8.8.8"}), (200, {"ok": True, "address": "8.8.8.8:27015"}))
            self.assertIn("isn't set to be listed", post({"address": "8.8.4.4"})[1]["error"])
            self.assertEqual(post({"address": "192.168.0.2"})[0], 400)
            self.assertEqual(post({"address": "1.1.1.1"})[0], 429, "rate limited after three")
            with urllib.request.urlopen(base + "/servers") as res:
                self.assertEqual(res.headers["Access-Control-Allow-Origin"], "*")
                body = json.load(res)
            self.assertEqual([s["address"] for s in body["servers"]], ["8.8.8.8:27015"])
            self.assertEqual(body["servers"][0]["map"], "de_dust2")
            with self.assertRaises(urllib.error.HTTPError):
                urllib.request.urlopen(base + "/other")
        finally:
            srv.shutdown()


if __name__ == "__main__":
    unittest.main()
