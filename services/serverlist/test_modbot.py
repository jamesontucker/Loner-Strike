#!/usr/bin/env python3
# Tests for the mod channel's cards and buttons (modbot.py), offline, with forged interactions
# signed by a test key:  cd services/serverlist && python3 -m unittest test_modbot
import json
import os
import sys
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import community as cm  # noqa: E402
import modbot  # noqa: E402
import serverlist as sl  # noqa: E402
from test_community import MAP, SPRAY, SPRAY_ITEM, FakeFiles, FakeGB, Recorder, body, map_item, report  # noqa: E402

SECRET = bytes(range(32))
PUB = modbot.public_key(SECRET)
GUILD, CHANNEL, MOD_ROLE, DEV_ROLE, OWNER = "1568421500636233748", "1570640821320159252", "1568425985504968768", "1568425984288362637", "310619637727363072"


class FakeBot(modbot.Bot):
    def __init__(self, fail=False):
        super().__init__("bot-token-not-real", CHANNEL, send=self.send)
        self.fail, self.posts, self.edits, self.n = fail, [], [], 0

    def send(self, method, path, b):
        if self.fail:
            return 500, {}
        if method == "POST":
            self.n += 1
            self.posts.append(b)
            return 200, {"id": str(9000 + self.n)}
        self.edits.append((path, b))
        return 200, {}


def make(fail=False):
    hook = Recorder()
    gb, files = FakeGB(), FakeFiles()
    gb.files = files
    c = cm.Community(cm.Store(os.path.join(tempfile.mkdtemp(), "c.db")), token="t" * 40, get=gb, get_range=files)
    c.verify_md5 = False
    bot = FakeBot(fail)
    mod = modbot.Mod(c, bot, PUB.hex(), GUILD, [MOD_ROLE, DEV_ROLE], OWNER,
                     webhook="https://discord.com/api/webhooks/1/abc", post=hook, background=False)
    return c, mod, bot, hook


_ids = iter(range(10 ** 6))


def member(uid="111", roles=(MOD_ROLE,)):
    return {"user": {"id": uid, "username": f"user{uid}"}, "roles": list(roles)}


def signed(payload, ts=None, secret=SECRET):
    raw = json.dumps(payload).encode()
    ts = str(int(time.time()) if ts is None else ts)
    return {"X-Signature-Ed25519": modbot.sign_pure(secret, ts.encode() + raw).hex(), "X-Signature-Timestamp": ts}, raw


def click(mod, custom_id, who=None, guild=GUILD, iid=None):
    d = {"type": 3, "id": iid or f"i{next(_ids)}", "guild_id": guild, "member": who or member(), "data": {"custom_id": custom_id}}
    return mod.interaction(*signed(d))


def modal(mod, custom_id, reason, who=None):
    d = {"type": 5, "id": f"i{next(_ids)}", "guild_id": GUILD, "member": who or member(),
         "data": {"custom_id": custom_id, "components": [{"type": 1, "components": [{"type": 4, "custom_id": "reason", "value": reason}]}]}}
    return mod.interaction(*signed(d))


def buttons(payload):
    return {b["label"]: b for b in payload["components"][0]["components"]}


class Ed25519(unittest.TestCase):
    # RFC 8032 section 7.1, tests 1-3
    VECTORS = [
        ("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60", "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "",
         "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"),
        ("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb", "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c", "72",
         "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"),
        ("c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7", "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025", "af82",
         "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a"),
    ]

    def test_rfc8032_vectors(self):
        for sk, pk, msg, sig in self.VECTORS:
            sk, pk, msg, sig = (bytes.fromhex(x) for x in (sk, pk, msg, sig))
            self.assertEqual(modbot.public_key(sk), pk)
            self.assertEqual(modbot.sign_pure(sk, msg), sig)
            for verify in {modbot.verify_pure, modbot.verify}:
                self.assertTrue(verify(pk, msg, sig))
                self.assertFalse(verify(pk, msg + b"x", sig))
                self.assertFalse(verify(pk, msg, sig[:63] + bytes([sig[63] ^ 1])))

    def test_malleable_or_malformed_signatures(self):
        sk, pk, msg, sig = (bytes.fromhex(x) for x in self.VECTORS[0])
        s = int.from_bytes(sig[32:], "little") + modbot._L  # same point, s not reduced: must be refused
        self.assertFalse(modbot.verify_pure(pk, msg, sig[:32] + s.to_bytes(32, "little")))
        self.assertFalse(modbot.verify_pure(pk[:31], msg, sig))
        self.assertFalse(modbot.verify_pure(b"\xff" * 32, msg, sig))


class Requests(unittest.TestCase):
    def test_signature_and_ping(self):
        c, mod, _, _ = make()
        h, raw = signed({"type": 1})
        self.assertEqual(mod.interaction(h, raw), (200, {"type": 1}))
        self.assertEqual(mod.interaction(h, raw + b" ")[0], 401, "the body was changed")
        self.assertEqual(mod.interaction(*signed({"type": 1}, secret=bytes(32)))[0], 401, "someone else's key")
        self.assertEqual(mod.interaction(*signed({"type": 1}, ts=int(time.time()) - 1000))[0], 401, "stale")
        self.assertEqual(mod.interaction({}, raw)[0], 401)
        self.assertEqual(mod.interaction({"X-Signature-Ed25519": "zz", "X-Signature-Timestamp": "1"}, raw)[0], 401)

    def test_only_moderators_in_our_server(self):
        c, mod, bot, _ = make()
        c.submit(body(map_item()), "a")
        pull = buttons(bot.posts[-1])["Pull"]["custom_id"]
        code, ans = click(mod, pull, who=member("222", roles=["999"]))
        self.assertEqual((ans["type"], ans["data"]["content"], ans["data"]["flags"]), (4, "You can't do that.", 64))
        self.assertIn("only works", click(mod, pull, guild="1")[1]["data"]["content"])
        self.assertEqual(click(mod, pull, who=member(OWNER, roles=[]))[1]["type"], 9, "the owner may, without a role")
        self.assertEqual(click(mod, pull, who=member("333", roles=[DEV_ROLE]))[1]["type"], 9)
        self.assertEqual(c.store.get("mod:739197")["state"], "listed", "nothing happened yet")

    def test_forged_buttons(self):
        c, mod, bot, _ = make()
        c.submit(body(map_item()), "a")
        pull = buttons(bot.posts[-1])["Pull"]["custom_id"]
        action, card, mac = pull.split(":")[1:]
        for forged in (f"ls1:pull:{int(card) + 1}:{mac}", f"ls1:keep:{card}:{mac}", "ls1:pull:1", "anything"):
            self.assertIn("isn't one of ours", click(mod, forged)[1]["data"]["content"], forged)
        keep = mod.custom_id("keep", int(card))  # signed, but not a button a listing card has
        self.assertIn("doesn't fit", click(mod, keep)[1]["data"]["content"])


class Cards(unittest.TestCase):
    def test_listing_pull_and_undo(self):
        c, mod, bot, _ = make()
        c.submit(body(map_item()), "a")
        card = bot.posts[-1]
        self.assertEqual(card["allowed_mentions"], {"parse": []})
        self.assertEqual(card["embeds"][0]["url"], "https://gamebanana.com/mods/739197")
        self.assertTrue(card["embeds"][0]["thumbnail"]["url"].startswith("https://images.gamebanana.com/"))
        pull = buttons(card)["Pull"]["custom_id"]
        form = click(mod, pull)[1]
        self.assertEqual(form["type"], 9)
        code, ans = modal(mod, form["data"]["custom_id"], "Stolen from @everyone's map")
        self.assertEqual(ans["type"], 7)
        desc = ans["data"]["embeds"][0]["description"]
        self.assertIn("Pulled** by <@111>", desc)
        self.assertNotIn("@everyone", desc)
        self.assertEqual(list(buttons(ans["data"])), ["Undo"])
        self.assertEqual(c.store.get("mod:739197")["state"], "pulled")
        self.assertIn("mod:739197", json.loads(cm.build_index(c.store)[0])["revoked"])
        self.assertTrue(bot.posts[-1]["content"].startswith("Audit: Pulled `mod:739197`"))
        # a second moderator clicking the old Pull (a stale card) and a replayed delivery
        self.assertIn("Already pulled by user111", click(mod, pull, who=member("444"))[1]["data"]["content"])
        undo = buttons(ans["data"])["Undo"]["custom_id"]
        d = {"type": 3, "id": "same-one", "guild_id": GUILD, "member": member(), "data": {"custom_id": undo}}
        self.assertEqual(mod.interaction(*signed(d))[1]["type"], 7)
        self.assertEqual(mod.interaction(*signed(d))[1]["data"]["content"], "Already done.")
        self.assertEqual(c.store.get("mod:739197")["state"], "listed")
        self.assertEqual(list(buttons(mod.payload(1))), ["Pull"], "the card is back as it was")
        self.assertIn("Undid pulled", bot.posts[-1]["content"])
        self.assertIn("Nothing to undo", click(mod, undo)[1]["data"]["content"])

    def test_undo_refuses_when_the_entry_changed(self):
        c, mod, bot, _ = make()
        c.submit(body(map_item()), "a")
        form = click(mod, buttons(bot.posts[-1])["Pull"]["custom_id"])[1]
        ans = modal(mod, form["data"]["custom_id"], "")[1]
        self.assertEqual(c.store.get("mod:739197")["reason"], "Pulled by the moderators", "neutral, no moderator's name")
        c.moderate("mod:739197", "restore", {})
        self.assertIn("changed since", click(mod, buttons(ans["data"])["Undo"]["custom_id"])[1]["data"]["content"])

    def test_report_keep_block_and_the_queue(self):
        c, mod, bot, _ = make()
        c.submit(body(SPRAY_ITEM), "a")
        c.report(report(model="Spray", gb=82585, reason="offensive", note="<@1> look"), "9.9.9.9")
        card = bot.posts[-1]
        self.assertIn("Report: offensive", card["embeds"][0]["description"])
        self.assertNotIn("9.9.9.9", json.dumps(card))
        self.assertEqual(list(buttons(card)), ["Pull", "Keep", "Block creator"])
        ans = click(mod, buttons(card)["Keep"]["custom_id"])[1]
        self.assertIn("Kept** by", ans["data"]["embeds"][0]["description"])
        self.assertEqual(c.store.get("spray:82585")["state"], "listed")
        # another report, and this time the creator is blocked
        c.report_said = cm.Window(10, 1)
        c.report(report(model="Spray", gb=82585, reason="offensive"), "8.8.8.8")
        ans = click(mod, buttons(bot.posts[-1])["Block creator"]["custom_id"])[1]
        self.assertIn("Creator blocked", ans["data"]["embeds"][0]["description"])
        self.assertTrue(c.store.is_blocked(SPRAY["_aSubmitter"]["_idRow"]))
        # their next upload waits, with Approve / Reject
        page = json.loads(json.dumps(SPRAY))
        page["_aFiles"].append(dict(page["_aFiles"][0], _idRow=2000000, _sMd5Checksum="1" * 32))
        c.get.pages[("Spray", 82585)] = page
        self.assertEqual(c.submit(body(dict(SPRAY_ITEM, file=2000000, md5="1" * 32)), "a")[1]["state"], "pending")
        q = bot.posts[-1]
        self.assertEqual(list(buttons(q)), ["Approve", "Reject"])
        form = click(mod, buttons(q)["Reject"]["custom_id"])[1]
        self.assertTrue(form["data"]["components"][0]["components"][0]["required"])
        modal(mod, form["data"]["custom_id"], "Same picture again.")
        self.assertIsNone(c.store.get("spray:82585")["proposed"])
        self.assertEqual(json.loads(cm.build_index(c.store)[0])["items"][0]["md5"], "26600d11dcffa32874195d9a32bcc010")
        # undo the block from its card
        undo = buttons(ans["data"])["Undo"]["custom_id"]
        click(mod, undo)
        self.assertFalse(c.store.is_blocked(SPRAY["_aSubmitter"]["_idRow"]))

    def test_approve_from_the_queue(self):
        c, mod, bot, _ = make()
        c.block(SPRAY["_aSubmitter"]["_idRow"], "x", "test", "")
        c.submit(body(SPRAY_ITEM), "a")
        c.get.pages[("Spray", 82585)] = OSError("GameBanana slow")  # approving doesn't wait on it
        ans = click(mod, buttons(bot.posts[-1])["Approve"]["custom_id"])[1]
        self.assertEqual(ans["type"], 7)
        self.assertEqual(c.store.get("spray:82585")["state"], "listed")
        click(mod, buttons(ans["data"])["Undo"]["custom_id"])
        self.assertEqual(c.store.get("spray:82585")["state"], "pending")

    def test_unregistered_page_reports_cant_block(self):
        c, mod, bot, _ = make()
        c.report(report(gb=77), "a")
        b = buttons(bot.posts[-1])
        self.assertTrue(b["Block creator"]["disabled"])
        form = click(mod, b["Pull"]["custom_id"])[1]
        modal(mod, form["data"]["custom_id"], "bad")
        self.assertEqual(c.store.get("mod:77")["state"], "pulled")
        click(mod, buttons(mod.payload(1))["Undo"]["custom_id"])
        self.assertIsNone(c.store.get("mod:77"), "undo takes away the entry the pull made")

    def test_webhook_when_the_bot_cant_post(self):
        c, mod, bot, hook = make(fail=True)
        err = __import__("io").StringIO()
        with __import__("contextlib").redirect_stderr(err):
            c.submit(body(map_item()), "a")
        self.assertIn("Listed map", hook.sent[-1][1]["content"])
        self.assertEqual(hook.sent[-1][1]["allowed_mentions"], {"parse": []})
        self.assertNotIn("bot-token-not-real", err.getvalue())

    def test_junk_commands_get_a_clean_answer(self):
        c, mod, bot, _ = make()
        for data in ({"name": "community", "options": "x"}, {"name": "community", "options": [{"name": "pull", "options": [1, None, {"name": 5}]}]},
                     {"name": "community", "options": [None]}, {"name": "other"}, "x"):
            d = {"type": 2, "id": f"j{next(_ids)}", "guild_id": GUILD, "member": member(), "data": data}
            code, ans = mod.interaction(*signed(d))
            self.assertEqual((code, ans["type"]), (200, 4), data)

    def test_slash_commands(self):
        c, mod, bot, _ = make()
        c.submit(body(map_item()), "a")

        def cmd(sub, page, reason="", who=None):
            opts = [{"name": "page", "value": page}] + ([{"name": "reason", "value": reason}] if reason else [])
            d = {"type": 2, "id": f"c{next(_ids)}", "guild_id": GUILD, "member": who or member(),
                 "data": {"name": "community", "options": [{"name": sub, "type": 1, "options": opts}]}}
            return mod.interaction(*signed(d))[1]["data"]["content"]
        self.assertIn("listed", cmd("status", "mod:739197"))
        self.assertEqual(cmd("pull", "mod:739197", who=member("5", roles=[])), "You can't do that.")
        self.assertIn("Pulled", cmd("pull", "https://gamebanana.com/mods/739197", "malware"))
        self.assertEqual(c.store.get("mod:739197")["reason"], "malware")
        self.assertIn("next submissions wait", cmd("block", "mod:739197"))
        self.assertIn("Name the page", cmd("status", "nope"))


class Http(unittest.TestCase):
    def test_through_the_service(self):
        c, mod, bot, _ = make()
        srv = ThreadingHTTPServer(("127.0.0.1", 0), sl.make_handler(sl.Registry(os.path.join(tempfile.mkdtemp(), "l.json")), sl.RateLimit(3), community=c))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        try:
            url = f"http://127.0.0.1:{srv.server_address[1]}/discord/interactions"
            h, raw = signed({"type": 1})
            req = urllib.request.Request(url, data=raw, method="POST", headers=dict(h, **{"Content-Type": "application/json"}))
            with urllib.request.urlopen(req, timeout=5) as r:
                self.assertEqual(json.loads(r.read()), {"type": 1})
                self.assertNotIn("Access-Control-Allow-Origin", r.headers)
            req = urllib.request.Request(url, data=raw + b"x", method="POST", headers=h)
            with self.assertRaises(urllib.error.HTTPError) as e:
                urllib.request.urlopen(req, timeout=5)
            self.assertEqual(e.exception.code, 401)
            e.exception.close()
        finally:
            srv.shutdown()


if __name__ == "__main__":
    unittest.main()
