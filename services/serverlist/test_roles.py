#!/usr/bin/env python3
"""Community roles (roles.py): claims, links, Polar supporters, VIP, server moderator applications.
Made-up tokens and secrets only; Discord and the game servers are fakes."""
import base64
import hashlib
import hmac
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

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import community as cm  # noqa: E402
import modbot  # noqa: E402
import roles  # noqa: E402
import serverlist as sl  # noqa: E402
from test_modbot import CHANNEL, GUILD, MOD_ROLE, OWNER, PUB, FakeBot, member, signed  # noqa: E402

POLAR = "polar_whs_made_up_for_tests_0123456789"
APPS = "1570000000000000001"
HOST_DISCORD = "420000000000000001"
PLAYER = "430000000000000002"
OTHER = "440000000000000003"
STEAM = "STEAM_0:1:12345"


class FakeServers:
    """clean_address and A2S: every address is fine; rules are what the test sets."""

    def __init__(self):
        self.rules = {}
        self.on = ["Zoë", "Player"]

    def players(self, address):
        return None if self.on is None else list(self.on)

    def clean(self, text):
        return (text.strip().lower(), ("203.0.113.5", 27015)) if isinstance(text, str) and text.strip() else None

    def ask(self, address):
        return ({"name": f"Server at {address}"}, dict(self.rules.get(address, {})))


_ids = iter(range(10 ** 6))


def make():
    c = cm.Community(cm.Store(os.path.join(tempfile.mkdtemp(), "c.db")), token="t" * 40)
    bot = FakeBot()
    bot.roles["Server Moderator"] = "900099"
    mod = modbot.Mod(c, bot, PUB.hex(), GUILD, [MOD_ROLE], OWNER, background=False, apply_channel=APPS, apply_roles=[MOD_ROLE])
    fs = FakeServers()
    r = roles.Roles(roles.Store(os.path.join(tempfile.mkdtemp(), "r.db")), clean=fs.clean, asker=fs.ask, players=fs.players, polar_secret=POLAR,
                    mod=mod, deciders=[MOD_ROLE])
    c.rolesvc = r
    return r, mod, bot, fs, c


def claim(r, fs, address="play.example.com:27015"):
    st = r.claim_start({"address": address}, "198.51.100.1")
    fs.rules[address] = {"ls_claim": st["nonce"]}
    done = r.claim_finish({"claim": st["claim"]}, "198.51.100.1")
    return done, r.server_by_token(done["token"])


def command(mod, name, uid, code=None, sub=None):
    opts = [{"name": "code", "value": code}] if code is not None else []
    data = {"name": name, "options": [{"name": sub, "type": 1, "options": opts}] if sub else opts}
    d = {"type": 2, "id": f"c{next(_ids)}", "guild_id": GUILD, "member": member(uid, ()), "data": data}
    return mod.interaction(*signed(d))[1]["data"]["content"]


def component(mod, custom_id, uid, values=None, guild=GUILD, roles_=(), fields=None, kind=3):
    d = {"type": kind, "id": f"i{next(_ids)}", "data": {"custom_id": custom_id}}
    if values is not None:
        d["data"]["values"] = values
    if fields:
        d["data"]["components"] = [{"type": 1, "components": [{"type": 4, "custom_id": k, "value": v}]} for k, v in fields.items()]
    if guild:
        d["guild_id"] = guild
        d["member"] = member(uid, roles_)
    else:
        d["user"] = {"id": uid, "username": f"user{uid}"}
    return mod.interaction(*signed(d))[1]


def polar(r, payload, secret=POLAR, wid=None, ts=None):
    body = json.dumps(payload).encode()
    wid = wid or f"msg_{next(_ids)}"
    ts = str(int(time.time()) if ts is None else ts)
    sig = base64.b64encode(hmac.new(secret.encode(), f"{wid}.{ts}.".encode() + body, hashlib.sha256).digest()).decode()
    return r.webhook({"webhook-id": wid, "webhook-timestamp": ts, "webhook-signature": f"v1,{sig}"}, body, "198.51.100.9")


def grant(account, gid="g1", cust="cus_1", sub=None, kind="benefit_grant.created", revoked=False):
    return {"type": kind, "data": {"id": gid, "customer_id": cust, "subscription_id": sub, "is_granted": not revoked,
                                   "is_revoked": revoked, "benefit": {"type": "discord"},
                                   "properties": {"account_id": account, "guild_id": GUILD}}}


def linked(r, fs, mod, uid=PLAYER, steam=STEAM, official=False):
    done, srv = claim(r, fs, f"s{next(_ids)}.example.com:27015")
    if official:
        r.set_official(srv["id"], True)
        srv = r.server_by_token(done["token"])
    code = r.link_code(srv, {"steamid": steam, "name": "Zoë"})["code"]
    self_text = command(mod, "link", uid, code)
    assert self_text.startswith("Linked"), self_text
    return srv


class Claims(unittest.TestCase):
    def test_claim_needs_the_cvar_and_a_new_claim_replaces_the_token(self):
        r, mod, bot, fs, _ = make()
        st = r.claim_start({"address": "play.example.com:27015"}, "1.2.3.4")
        with self.assertRaises(cm.Reject) as e:
            r.claim_finish({"claim": st["claim"]}, "1.2.3.4")
        self.assertIn("ls_claim", e.exception.message)
        fs.rules["play.example.com:27015"] = {"ls_claim": "lsc-wrong"}
        with self.assertRaises(cm.Reject):
            r.claim_finish({"claim": st["claim"]}, "1.2.3.4")
        fs.rules["play.example.com:27015"] = {"ls_claim": st["nonce"]}
        done = r.claim_finish({"claim": st["claim"]}, "1.2.3.4")
        self.assertTrue(done["token"].startswith("lsr1_"))
        srv = r.server_by_token(done["token"])
        self.assertEqual(srv["address"], "play.example.com:27015")
        # the token itself is never kept
        self.assertNotIn(done["token"], json.dumps(r.s.all("SELECT * FROM servers")))
        # one use
        with self.assertRaises(cm.Reject):
            r.claim_finish({"claim": st["claim"]}, "1.2.3.4")
        done2, srv2 = claim(r, fs)
        self.assertEqual(srv2["id"], srv["id"])
        self.assertIsNone(r.server_by_token(done["token"]))
        self.assertIsNone(r.server_by_token("lsr1_" + "x" * 43))
        self.assertIsNone(r.server_by_token(None))

    def test_owner_code_in_discord(self):
        r, mod, bot, fs, _ = make()
        done, srv = claim(r, fs)
        self.assertIn("isn't right", command(mod, "server", HOST_DISCORD, "ZZZZ-ZZZZ", sub="claim"))
        self.assertIn("You're the owner", command(mod, "server", HOST_DISCORD, done["owner_code"].lower(), sub="claim"))
        self.assertEqual(r.server_by_token(done["token"])["owner_discord"], HOST_DISCORD)
        # used once
        self.assertIn("isn't right", command(mod, "server", OTHER, done["owner_code"], sub="claim"))
        for _ in range(5):
            command(mod, "server", OTHER, "AAAA-AAAA", sub="claim")
        self.assertIn("Too many", command(mod, "server", OTHER, r.owner_code(srv)["owner_code"], sub="claim"))

    def test_claim_rate_limit(self):
        r, *_ = make()
        for _ in range(10):
            r.claim_start({"address": "a.example.com"}, "9.9.9.9")
        with self.assertRaises(cm.Reject) as e:
            r.claim_start({"address": "a.example.com"}, "9.9.9.9")
        self.assertEqual(e.exception.code, 429)


class Links(unittest.TestCase):
    def test_link_relink_unlink(self):
        r, mod, bot, fs, _ = make()
        done, srv = claim(r, fs)
        for bad in ("BOT", "STEAM_ID_LAN", "STEAM_ID_PENDING", "VALVE_0:1:2", "STEAM_0:1:1; quit"):
            with self.assertRaises(cm.Reject):
                r.link_code(srv, {"steamid": bad, "name": "Zoë"})
        code = r.link_code(srv, {"steamid": STEAM, "name": "Zoë"})["code"]
        self.assertRegex(code, r"^[0-9A-Z]{4}-[0-9A-Z]{4}$")
        self.assertIn("not linked", command(mod, "link", PLAYER))
        # typed loosely: lower case, no hyphen, O for 0
        self.assertTrue(command(mod, "link", PLAYER, code.replace("-", "").lower().replace("0", "o")).startswith("Linked"))
        self.assertIn(STEAM, command(mod, "link", PLAYER))
        self.assertIn("isn't right", command(mod, "link", PLAYER, code))
        # someone else can't take that SteamID
        c2 = r.link_code(srv, {"steamid": STEAM, "name": "Zoë"})["code"]
        self.assertIn("another Discord account", command(mod, "link", OTHER, c2))
        # relink to another SteamID replaces the link
        c3 = r.link_code(srv, {"steamid": "STEAM_0:0:999", "name": "Zoë"})["code"]
        self.assertTrue(command(mod, "link", PLAYER, c3).startswith("Linked"))
        self.assertIsNone(r.s.one("SELECT * FROM links WHERE steamid=?", (STEAM,)))
        self.assertEqual(command(mod, "unlink", PLAYER), "Unlinked.")
        self.assertEqual(command(mod, "unlink", PLAYER), "You weren't linked.")
        log = [x["action"] for x in r.s.all("SELECT action FROM roles_log")]
        self.assertEqual(log.count("link"), 2)
        self.assertIn("unlink", log)

    def test_codes_run_out_and_wrong_codes_lock_out(self):
        r, mod, bot, fs, _ = make()
        done, srv = claim(r, fs)
        code = r.link_code(srv, {"steamid": STEAM, "name": "Zoë"})["code"]
        r.s.run("UPDATE link_codes SET expires=?", (cm.now() - 1,))
        self.assertIn("ran out", command(mod, "link", PLAYER, code))
        for _ in range(5):
            self.assertIn("isn't right", command(mod, "link", OTHER, "AAAA-AAAA"))
        good = r.link_code(srv, {"steamid": "STEAM_0:0:5", "name": "Zoë"})["code"]
        self.assertIn("Too many", command(mod, "link", OTHER, good))
        # codes per SteamID: three in ten minutes, the fourth refused; a newer code replaces the older one
        for _ in range(3):
            r.link_code(srv, {"steamid": "STEAM_0:0:7", "name": "Zoë"})
        with self.assertRaises(cm.Reject) as e:
            r.link_code(srv, {"steamid": "STEAM_0:0:7", "name": "Zoë"})
        self.assertEqual(e.exception.code, 429)
        self.assertEqual(len(r.s.all("SELECT * FROM link_codes WHERE steamid='STEAM_0:0:7'")), 1)

    def test_codes_are_random_and_only_hashed(self):
        r, mod, bot, fs, _ = make()
        done, srv = claim(r, fs)
        codes = set()
        for i in range(40):
            r.steams_per_server.clear()
            codes.add(r.link_code(srv, {"steamid": f"STEAM_0:1:{i}", "name": "Zoë"})["code"])
        self.assertEqual(len(codes), 40)
        stored = json.dumps(r.s.all("SELECT * FROM link_codes"))
        self.assertFalse(any(c in stored or c.replace("-", "") in stored for c in codes))
        self.assertEqual(roles.norm_code("abcd-efgh"), "ABCDEFGH")
        self.assertEqual(roles.norm_code("O1IL-0000"), "0111" + "0000")
        self.assertIsNone(roles.norm_code("ABCD-EFG"))
        self.assertIsNone(roles.norm_code("ABCD-EFGU"))


class Squatting(unittest.TestCase):
    def test_an_official_link_wins_over_an_unverified_one(self):
        r, mod, bot, fs, _ = make()
        linked(r, fs, mod, OTHER, STEAM)  # a host squatting STEAM on their own server
        done, off = claim(r, fs, "official.example.com:27015")
        r.set_official(off["id"], True)
        off = r.server_by_token(done["token"])
        code = r.link_code(off, {"steamid": STEAM, "name": "Zoë"})["code"]
        self.assertTrue(command(mod, "link", PLAYER, code).startswith("Linked"))
        self.assertEqual(r.s.one("SELECT discord_id, official FROM links WHERE steamid=?", (STEAM,)), {"discord_id": PLAYER, "official": 1})
        self.assertIn("was replaced", bot.dms[-1]["content"])
        self.assertIn("evict", [x["action"] for x in r.s.all("SELECT action FROM roles_log")])
        r.code_per_steam = cm.Window(10, 600)
        # two official links: the second needs /unlink
        code = r.link_code(off, {"steamid": STEAM, "name": "Zoë"})["code"]
        self.assertIn("another Discord account", command(mod, "link", OTHER, code))
        # an unverified link never replaces an official one
        _, host = claim(r, fs, "host2.example.com:27015")
        code = r.link_code(host, {"steamid": STEAM, "name": "Zoë"})["code"]
        self.assertIn("another Discord account", command(mod, "link", OTHER, code))

    def test_the_player_has_to_be_on_the_server(self):
        r, mod, bot, fs, _ = make()
        _, srv = claim(r, fs)
        with self.assertRaises(cm.Reject) as e:
            r.link_code(srv, {"steamid": STEAM, "name": "Nobody here"})
        self.assertIn("don't seem to be on this server", e.exception.message)
        with self.assertRaises(cm.Reject):
            r.link_code(srv, {"steamid": STEAM})
        fs.on = None
        with self.assertRaises(cm.Reject) as e:
            r.link_code(srv, {"steamid": STEAM, "name": "Zoë"})
        self.assertIn("couldn't see who's on", e.exception.message)

    def test_twenty_steamids_per_server_a_day(self):
        r, mod, bot, fs, _ = make()
        _, srv = claim(r, fs)
        for i in range(roles.STEAMS_PER_DAY):
            r.link_code(srv, {"steamid": f"STEAM_0:0:{i}", "name": "Zoë"})
        with self.assertRaises(cm.Reject) as e:
            r.link_code(srv, {"steamid": "STEAM_0:0:999", "name": "Zoë"})
        self.assertIn("20 players today", e.exception.message)
        r.link_code(srv, {"steamid": "STEAM_0:0:1", "name": "Zoë"})  # one it already had
        _, other = claim(r, fs, "other.example.com:27015")
        r.link_code(other, {"steamid": "STEAM_0:0:999", "name": "Zoë"})
        self.assertTrue(r.new_steamid_ok(srv["id"], "STEAM_0:0:999", t=time.time() + 86401))

    def test_moderators_evict_a_link(self):
        r, mod, bot, fs, c = make()
        linked(r, fs, mod, OTHER, STEAM)
        with self.assertRaises(cm.Reject):
            r.evict("not-a-steamid")
        self.assertEqual(r.admin("POST", ["unlink"], {"steamid": STEAM, "reason": "squatted"}, {})["discord_id"], OTHER)
        self.assertIsNone(r.s.one("SELECT * FROM links WHERE steamid=?", (STEAM,)))
        last = r.s.all("SELECT * FROM roles_log ORDER BY id DESC LIMIT 1")[0]
        self.assertEqual((last["action"], last["actor"], last["subject"]), ("evict", "moderator", STEAM))
        self.assertIn("squatted", last["detail"])
        with self.assertRaises(cm.Reject):
            r.evict(STEAM)


class Supporters(unittest.TestCase):
    def test_signature_replay_and_staleness(self):
        r, *_ = make()
        self.assertEqual(polar(r, grant(PLAYER))[0], 202)
        self.assertEqual(polar(r, grant(PLAYER), secret="not-the-secret")[0], 401)
        self.assertEqual(polar(r, grant(PLAYER), ts=int(time.time()) - 3600)[0], 401)
        code, ans = polar(r, grant(PLAYER, gid="g2"), wid="msg_same")
        self.assertEqual(code, 202)
        self.assertTrue(polar(r, grant(PLAYER, gid="g3"), wid="msg_same")[1].get("again"))
        self.assertIsNone(r.s.one("SELECT * FROM polar_grants WHERE grant_id='g3'"))
        off = roles.Roles(roles.Store(os.path.join(tempfile.mkdtemp(), "r.db")))
        self.assertEqual(polar(off, grant(PLAYER))[0], 503)
        # Standard Webhooks' whsec_ secrets work too
        key = base64.b64encode(b"k" * 24).decode()
        self.assertEqual(roles.webhook_key("whsec_" + key), b"k" * 24)

    def test_monthly_and_one_time_stacking(self):
        r, mod, bot, fs, _ = make()
        self.assertFalse(r.is_supporter(PLAYER))
        polar(r, grant(PLAYER, gid="gm", cust="cus_m", sub="sub_1"))
        self.assertTrue(r.is_supporter(PLAYER))
        polar(r, grant(PLAYER, gid="gm", cust="cus_m", sub="sub_1", kind="benefit_grant.revoked", revoked=True))
        self.assertFalse(r.is_supporter(PLAYER))
        # a donation that comes before Polar says whose Discord it is
        polar(r, {"type": "order.paid", "data": {"id": "ord_1", "customer_id": "cus_o", "subscription_id": None}})
        self.assertFalse(r.is_supporter(OTHER))
        polar(r, grant(OTHER, gid="go", cust="cus_o"))
        self.assertTrue(r.is_supporter(OTHER))
        polar(r, {"type": "order.paid", "data": {"id": "ord_2", "customer_id": "cus_o"}})
        polar(r, {"type": "order.paid", "data": {"id": "ord_2", "customer_id": "cus_o"}})  # the same order again
        until = r.s.one("SELECT until FROM supporters WHERE discord_id=?", (OTHER,))["until"]
        self.assertAlmostEqual(until - cm.now(), 60 * 86400, delta=120)
        self.assertTrue(r.is_supporter(OTHER, at=cm.now() + 59 * 86400))
        self.assertFalse(r.is_supporter(OTHER, at=cm.now() + 61 * 86400))
        # not a Discord benefit, or no account: nothing
        polar(r, {"type": "benefit_grant.created", "data": {"id": "gx", "customer_id": "c", "benefit": {"type": "license_keys"},
                                                           "properties": {"account_id": "450000000000000009"}}})
        self.assertFalse(r.is_supporter("450000000000000009"))

    def test_vip_lists(self):
        r, mod, bot, fs, _ = make()
        polar(r, grant(PLAYER, gid="g", cust="c", sub="s"))
        polar(r, grant(OTHER, gid="g2", cust="c2", sub="s2"))
        linked(r, fs, mod, PLAYER, STEAM)
        done, host = claim(r, fs, "host.example.com:27015")
        with self.assertRaises(cm.Reject):
            r.vip(host)
        with self.assertRaises(cm.Reject):
            r.vip_check(host, {"steamids": [STEAM]})
        host = r.set_settings(host, {"settings": {"honor_supporters": True}}) and r.server_by_token(done["token"])
        self.assertEqual(r.vip_check(host, {"steamids": [STEAM, "STEAM_0:0:1"]}), {"steamids": [STEAM]})
        with self.assertRaises(cm.Reject):
            r.vip_check(host, {"steamids": ["STEAM_0:0:1"] * 65})
        with self.assertRaises(cm.Reject):
            r.vip_check(host, {"steamids": ["nope"]})
        r.set_official(host["id"], True)
        self.assertEqual(r.vip(r.server_by_token(done["token"]))["steamids"], [STEAM])  # OTHER isn't linked


class Applications(unittest.TestCase):
    def setup_server(self, official=False):
        r, mod, bot, fs, c = make()
        done, srv = claim(r, fs, "mod.example.com:27015")
        command(mod, "server", HOST_DISCORD, done["owner_code"], sub="claim")
        r.set_settings(srv, {"settings": {"applications": True}})
        if official:
            r.set_official(srv["id"], True)
        return r, mod, bot, fs, c, done, r.server_by_token(done["token"])

    def apply(self, r, mod, srv, uid=PLAYER):
        ans = component(mod, mod.custom_id("applypick", 0), uid, values=["server_mod"])
        self.assertEqual(ans["type"], 4, ans)
        opts = ans["data"]["components"][0]["components"][0]["options"]
        self.assertIn(str(srv["id"]), [o["value"] for o in opts])
        form = component(mod, mod.custom_id("smpick", 0), uid, values=[str(srv["id"])])
        self.assertEqual(form["type"], 9, form)
        done = component(mod, form["data"]["custom_id"], uid, kind=5,
                         fields={"availability": "Weekends, 18-23 CET", "why": "I play here every day and know the regulars well.",
                                 "experience": "Two years on a jailbreak server", "other": ""})
        self.assertIn("went to its owner", done["data"]["content"])
        return r.s.one("SELECT * FROM sm_apps ORDER BY id DESC")

    def test_needs_a_link_and_official_needs_an_official_link(self):
        r, mod, bot, fs, c, done, srv = self.setup_server(official=True)
        ans = component(mod, mod.custom_id("applypick", 0), PLAYER, values=["server_mod"])
        self.assertIn("Link your SteamID first", ans["data"]["content"])
        linked(r, fs, mod)  # on a community server
        ans = component(mod, mod.custom_id("applypick", 0), PLAYER, values=["server_mod"])
        self.assertIn("official servers need a link made on an official server", ans["data"]["content"])
        ans = component(mod, mod.custom_id("smpick", 0), PLAYER, values=[str(srv["id"])])
        self.assertIn("official server", ans["data"]["content"])
        command(mod, "unlink", PLAYER)
        linked(r, fs, mod, official=True)
        self.apply(r, mod, srv)

    def test_end_to_end(self):
        r, mod, bot, fs, c, done, srv = self.setup_server()
        linked(r, fs, mod)
        a = self.apply(r, mod, srv)
        self.assertEqual(a["state"], "pending")
        # the card went to the owner by DM (not the team channel: the server didn't hand deciding to them)
        self.assertEqual(len(bot.dms), 1)
        card = bot.dms[-1]
        self.assertIn(STEAM, card["embeds"][0]["description"])
        self.assertIn("unverified", card["embeds"][0]["description"])
        ok = card["components"][0]["components"][0]["custom_id"]
        # someone else (even a Discord moderator, while the switch is off) can't decide
        self.assertIn("can't decide", component(mod, ok, OTHER, roles_=(MOD_ROLE,))["data"]["content"])
        # nor the applicant
        self.assertIn("can't decide", component(mod, ok, PLAYER, guild=None)["data"]["content"])
        # the owner, in their DMs
        ans = component(mod, ok, HOST_DISCORD, guild=None)
        self.assertEqual(ans["type"], 7)
        self.assertEqual(r.s.one("SELECT state FROM sm_apps WHERE id=?", (a["id"],))["state"], "approved")
        self.assertIn("approved", bot.dms[-1]["content"])
        self.assertIn("Already approved", component(mod, ok, HOST_DISCORD, guild=None)["data"]["content"])
        # the app: the list, the invite (DMed), the redemption (Discord role), the revoke
        apps = r.applications(srv)["applications"]
        self.assertEqual((apps[0]["state"], apps[0]["link"]), ("approved", "unverified"))
        with self.assertRaises(cm.Reject):
            r.send_invite(srv, a["id"], {"invite": "not an invite"})
        out = r.send_invite(srv, a["id"], {"invite": "LS1.eyJhIjpbXX0", "expires": cm.now() + 3600})
        self.assertTrue(out["dm"])
        self.assertIn("LS1.eyJhIjpbXX0", bot.dms[-1]["content"])
        self.assertNotIn("LS1.eyJhIjpbXX0", json.dumps(r.s.all("SELECT * FROM sm_apps")))
        with self.assertRaises(cm.Reject):
            r.send_invite(srv, a["id"], {"invite": "LS1.again"})
        r.redeemed(srv, a["id"], {"member": "m1"})
        self.assertTrue(any(m == "PUT" and p.endswith("/roles/900099") for m, p, *_ in bot.calls))
        self.assertEqual(r.applications(srv)["applications"][0]["state"], "active")
        # while moderating, the link can't move
        self.assertIn("Ask its owner", command(mod, "unlink", PLAYER))
        r.revoke(srv, a["id"], {"by": "owner"})
        self.assertTrue(any(m == "DELETE" and p.endswith("/roles/900099") for m, p, *_ in bot.calls))
        self.assertEqual(command(mod, "unlink", PLAYER), "Unlinked.")
        acts = [x["action"] for x in r.s.all("SELECT action FROM roles_log ORDER BY id")]
        for want in ("apply", "approve", "invite", "redeem", "revoke"):
            self.assertIn(want, acts)
        # another server's token can't touch it
        _, other = claim(r, fs, "other.example.com:27015")
        with self.assertRaises(cm.Reject):
            r.revoke(other, a["id"], {})

    def test_discord_moderators_when_allowed_and_deny_with_reason(self):
        r, mod, bot, fs, c, done, srv = self.setup_server()
        r.set_settings(srv, {"settings": {"discord_mods_decide": True}})
        srv = r.server_by_token(done["token"])
        linked(r, fs, mod)
        a = self.apply(r, mod, srv)
        self.assertEqual(len(bot.posts), 1)  # the team channel's copy
        no = bot.posts[-1]["components"][0]["components"][1]["custom_id"]
        form = component(mod, no, OTHER, roles_=(MOD_ROLE,))
        self.assertEqual(form["type"], 9)
        ans = component(mod, form["data"]["custom_id"], OTHER, roles_=(MOD_ROLE,), kind=5, fields={"reason": "Too new here"})
        self.assertEqual(ans["type"], 7)
        self.assertEqual(r.s.one("SELECT state, reason FROM sm_apps WHERE id=?", (a["id"],)), {"state": "denied", "reason": "Too new here"})
        self.assertIn("Too new here", bot.dms[-1]["content"])
        # the owner's DM copy was edited too
        self.assertTrue(any("/channels/dm-1/messages/" in p for p, _ in bot.edits))
        # 14 days before the same server again
        ans = component(mod, mod.custom_id("smpick", 0), PLAYER, values=[str(srv["id"])])
        self.assertIn("again from", ans["data"]["content"])

    def test_invites_run_out(self):
        r, mod, bot, fs, c, done, srv = self.setup_server()
        linked(r, fs, mod)
        a = self.apply(r, mod, srv)
        component(mod, bot.dms[-1]["components"][0]["components"][0]["custom_id"], HOST_DISCORD, guild=None)
        r.send_invite(srv, a["id"], {"invite": "LS1.x", "expires": cm.now() + 60})
        r.s.run("UPDATE sm_apps SET invite_expires=?", (cm.now() - 1,))
        self.assertEqual(r.applications(srv)["applications"][0]["state"], "approved")
        self.assertTrue(r.send_invite(srv, a["id"], {"invite": "LS1.y"})["ok"])


class PlayerList(unittest.TestCase):
    def test_a2s_player_reply(self):
        import struct
        body = b"D\x02" + b"\x00Zo\xc3\xab\x00" + struct.pack("<lf", 3, 12.5) + b"\x01[BOT] Kyle\x00" + struct.pack("<lf", 0, 1.0)
        self.assertEqual(sl.parse_players(body), ["Zoë", "[BOT] Kyle"])
        self.assertIsNone(sl.parse_players(b"E\x00\x00"))
        self.assertIsNone(sl.parse_players(b"D\x02\x00Zo"))


class Http(unittest.TestCase):
    def test_through_the_service(self):
        r, mod, bot, fs, c = make()
        srv = ThreadingHTTPServer(("127.0.0.1", 0), sl.make_handler(sl.Registry(os.path.join(tempfile.mkdtemp(), "l.json")), sl.RateLimit(3), community=c))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        base = f"http://127.0.0.1:{srv.server_address[1]}"

        def call(method, path, body=None, token=None, headers=None):
            h = dict(headers or {}, **{"Content-Type": "application/json"})
            if token:
                h["Authorization"] = f"Bearer {token}"
            data = body if isinstance(body, bytes) else (json.dumps(body).encode() if body is not None else None)
            req = urllib.request.Request(base + path, data=data, method=method, headers=h)
            try:
                with urllib.request.urlopen(req, timeout=5) as resp:
                    return resp.status, json.loads(resp.read() or b"{}")
            except urllib.error.HTTPError as e:
                with e:
                    return e.code, json.loads(e.read() or b"{}")
        try:
            code, st = call("POST", "/roles/claim/start", {"address": "web.example.com:27015"})
            self.assertEqual(code, 200)
            fs.rules["web.example.com:27015"] = {"ls_claim": st["nonce"]}
            code, done = call("POST", "/roles/claim/finish", {"claim": st["claim"]})
            tok = done["token"]
            self.assertEqual(call("GET", "/roles/server")[0], 401)
            self.assertEqual(call("GET", "/roles/server", token="lsr1_wrong")[0], 401)
            code, view = call("GET", "/roles/server", token=tok)
            self.assertEqual((code, view["settings"]["honor_supporters"]), (200, False))
            code, view = call("POST", "/roles/server", {"settings": {"honor_supporters": True}}, token=tok)
            self.assertTrue(view["settings"]["honor_supporters"])
            self.assertEqual(call("POST", "/roles/server", {"settings": {"honor_supporters": "yes"}}, token=tok)[0], 400)
            code, lc = call("POST", "/roles/link/code", {"steamid": STEAM, "name": "Zoë"}, token=tok)
            self.assertEqual(code, 200)
            self.assertEqual(call("POST", "/roles/vip/check", {"steamids": [STEAM]}, token=tok), (200, {"steamids": []}))
            self.assertEqual(call("GET", "/roles/vip", token=tok)[0], 403)
            self.assertEqual(call("GET", "/roles/applications", token=tok), (200, {"applications": []}))
            # the webhook through HTTP
            body = json.dumps(grant(PLAYER)).encode()
            ts = str(int(time.time()))
            sig = base64.b64encode(hmac.new(POLAR.encode(), f"m1.{ts}.".encode() + body, hashlib.sha256).digest()).decode()
            self.assertEqual(call("POST", "/roles/polar/webhook", body, headers={"webhook-id": "m1", "webhook-timestamp": ts,
                                                                                "webhook-signature": f"v1,{sig}"})[0], 202)
            self.assertEqual(call("POST", "/roles/polar/webhook", body, headers={"webhook-id": "m2", "webhook-timestamp": ts,
                                                                                "webhook-signature": f"v1,{sig}"})[0], 401)
            self.assertEqual(call("DELETE", "/roles/server", {}, token=tok), (200, {"ok": True}))
            self.assertEqual(call("GET", "/roles/server", token=tok)[0], 401)
            self.assertEqual(call("GET", "/roles/nothing")[0], 401)
        finally:
            srv.shutdown()


if __name__ == "__main__":
    unittest.main()
