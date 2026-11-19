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

    def test_names_of_63_bytes(self):
        """Steam's server name limit is 64 bytes with the NUL: a 63-byte UTF-8 name (the app's
        cap) comes through the A2S reply, the list and a claim's name whole."""
        name = "Café " + "x" * 57
        self.assertEqual(len(name.encode()), 63)
        body = b"I\x30" + name.encode() + b"\x00de_dust2\x00cstrike\x00Counter-Strike\x00\x0a\x00\x05\x20\x03dl\x01\x01\x01"
        info = sl.parse_info(body)
        self.assertEqual(info["name"], name)
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.assertTrue(r.update("a:1", info, {"ls_list": "1"}, 1000))
        self.assertEqual(r.public(1000)[0]["name"], name)
        import community as cm
        self.assertEqual(cm.one_line(name, 64), name, "a claim keeps it (roles.py: one_line(name, 64))")
        emoji = "a" * 59 + "\U0001F642"
        self.assertEqual(cm.one_line(emoji, 64), emoji)

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
        self.assertEqual(sorted(r.addresses()), ["a:1", "b:1"], "one poll saying off isn't enough (a server just started says 0)")
        self.assertFalse(r.update("b:1", info, {"ls_list": "0"}, 1400))
        self.assertEqual(r.addresses(), ["a:1"], "the host turned it off: two polls in a row")
        r.update("a:1", None, None, 1000 + 2 * 24 * 3600)
        self.assertEqual(r.addresses(), ["a:1"], "down for two days (a machine restarted): still known")
        self.assertEqual(r.public(1000 + 2 * 24 * 3600), [], "but not shown while it doesn't answer")
        self.assertTrue(r.update("a:1", info, {"ls_list": "1"}, 1000 + 2 * 24 * 3600 + 5))
        self.assertEqual([s["address"] for s in r.public(1000 + 2 * 24 * 3600 + 5)], ["a:1"], "back on its own when it answers")
        r.update("a:1", None, None, 1000 + 2 * 24 * 3600 + 5 + sl.GONE_SECS + 1)
        self.assertEqual(r.addresses(), [], "gone for a week: off the list")
        again = sl.Registry(r.path)
        self.assertEqual(again.addresses(), [], "kept on disk")

    def test_a_restart_doesnt_drop_a_listed_server(self):
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        info = sl.parse_info(INFO_BODY)
        self.assertTrue(r.update("a:1", info, {"ls_list": "1"}, 1000))
        # it restarts: one poll lands before friends.cfg has set ls_list 1 again
        self.assertFalse(r.update("a:1", info, {"ls_list": "0"}, 1120))
        self.assertTrue(r.update("a:1", info, {"ls_list": "1"}, 1240))
        # the count starts again after a poll that says on
        self.assertFalse(r.update("a:1", info, {"ls_list": "0"}, 1360))
        self.assertEqual(r.addresses(), ["a:1"])
        again = sl.Registry(r.path)
        self.assertEqual(again.servers["a:1"]["off"], 1, "the count is kept over a restart of the list")

    def test_one_server_by_name_and_ip_is_shown_once(self):
        info = sl.parse_info(INFO_BODY)
        on = {"ls_list": "1"}
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.assertTrue(r.update("8.8.8.8:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertTrue(r.update("play.example.com:27015", info, on, 1010, "8.8.8.8:27015"))
        self.assertEqual(sorted(r.addresses()), ["8.8.8.8:27015", "play.example.com:27015"], "both kept as registered")
        self.assertEqual([(s["address"], s["since"]) for s in r.public(1020)], [("8.8.8.8:27015", 1000)], "one row: the IP's")
        # another port on the same machine is another server
        self.assertTrue(r.update("8.8.8.8:27016", info, on, 1030, "8.8.8.8:27016"))
        self.assertEqual(len(r.public(1040)), 2)
        # one name per ip:port (a second name is refused; names never fold together)
        n = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.assertTrue(n.update("a.example.com:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertFalse(n.update("b.example.com:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertEqual([s["address"] for s in n.public(1000)], ["a.example.com:27015"])

    def test_a_name_pointed_at_someone_elses_server_changes_nothing_of_theirs(self):
        on = {"ls_list": "1"}
        victim = dict(sl.parse_info(INFO_BODY), name="Victim")
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.assertTrue(r.update("8.8.8.8:27015", victim, on, 1000, "8.8.8.8:27015"))
        before = json.loads(json.dumps(r.servers["8.8.8.8:27015"]))
        # the attacker's name resolves to the victim's IP
        self.assertTrue(r.update("evil.example.com:27015", victim, on, 1005, "8.8.8.8:27015"))
        self.assertEqual(r.servers["8.8.8.8:27015"], before, "the victim's entry is untouched")
        self.assertEqual([(s["address"], s["name"], s["since"]) for s in r.public(1010)], [("8.8.8.8:27015", "Victim", 1000)])
        # then points it at its own server: its own row, the victim's still the same
        mine = dict(victim, name="Evil")
        self.assertTrue(r.update("evil.example.com:27015", mine, on, 1130, "9.9.9.9:27015"))
        self.assertTrue(r.update("8.8.8.8:27015", victim, on, 1130, "8.8.8.8:27015"))
        rows = {s["address"]: s["name"] for s in r.public(1140)}
        self.assertEqual(rows, {"8.8.8.8:27015": "Victim", "evil.example.com:27015": "Evil"})

    def test_a_name_that_moves_between_polls_isnt_folded(self):
        info = sl.parse_info(INFO_BODY)
        on = {"ls_list": "1"}
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        r.update("8.8.8.8:27015", info, on, 1000, "8.8.8.8:27015")
        r.update("play.example.com:27015", info, on, 1000, "8.8.8.8:27015")
        self.assertEqual(len(r.public(1000)), 1)
        # next poll the name answers from elsewhere: two rows again
        r.update("play.example.com:27015", info, on, 1120, "9.9.9.9:27015")
        r.update("8.8.8.8:27015", info, on, 1120, "8.8.8.8:27015")
        self.assertEqual(len(r.public(1130)), 2)
        # the name points back, but the IP's last answer is from an older poll: not folded
        r.update("play.example.com:27015", info, on, 1500, "8.8.8.8:27015")
        self.assertEqual(len(r.public(1510)), 2)

    def test_names_pointed_at_one_server_cant_fill_the_list(self):
        info = sl.parse_info(INFO_BODY)
        on = {"ls_list": "1"}
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.assertTrue(r.update("8.8.8.8:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertTrue(r.update("a.example.com:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertFalse(r.update("b.example.com:27015", info, on, 1000, "8.8.8.8:27015"), "a third for one ip:port")
        self.assertFalse(r.update("c.example.com:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertEqual(sorted(r.addresses()), ["8.8.8.8:27015", "a.example.com:27015"])
        self.assertTrue(r.update("a.example.com:27015", info, on, 1100, "8.8.8.8:27015"), "the ones there keep answering")
        self.assertTrue(r.update("d.example.com:27015", info, on, 1100, "9.9.9.9:27015"), "another server is fine")

    def test_names_registered_first_cant_squat_the_servers_own_ip(self):
        info = sl.parse_info(INFO_BODY)
        on = {"ls_list": "1"}
        r = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.assertTrue(r.update("evil.example.com:27015", info, on, 1000, "8.8.8.8:27015"))
        self.assertFalse(r.update("evil2.example.com:27015", info, on, 1000, "8.8.8.8:27015"), "one name per ip:port")
        self.assertTrue(r.update("8.8.8.8:27015", info, on, 1000, "8.8.8.8:27015"), "the server's own IP always gets on")

    def test_check_asks_the_address_it_keeps(self):
        srv = FakeServer({"ls_list": "1"})
        try:
            calls = []
            real = sl.socket.getaddrinfo
            sl.socket.getaddrinfo = lambda *a, **k: calls.append(a) or [(0, 0, 0, "", srv.addr)]
            old_global = sl.ipaddress.IPv4Address.is_global
            sl.ipaddress.IPv4Address.is_global = property(lambda self: True)
            try:
                info, rules, ip = sl.check_at("play.example.com:%d" % srv.addr[1])
            finally:
                sl.socket.getaddrinfo = real
                sl.ipaddress.IPv4Address.is_global = old_global
            self.assertEqual(len(calls), 1, "one lookup")
            self.assertEqual(ip, "%s:%d" % srv.addr)
            self.assertEqual(rules.get("ls_list"), "1")
        finally:
            srv.on = False

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
