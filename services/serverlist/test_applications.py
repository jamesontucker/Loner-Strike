#!/usr/bin/env python3
# Tests for "Apply for a role" (applications.py through modbot.py), with forged, signed
# interactions:  cd services/serverlist && python3 -m unittest test_applications
import importlib.util
import json
import os
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import applications  # noqa: E402
import community as cm  # noqa: E402
import modbot  # noqa: E402
from test_modbot import (DEV_ROLE, GUILD, MOD_ROLE, OWNER, PUB, ROLE_IDS, FakeBot, FakeFiles, FakeGB, buttons,  # noqa: E402
                         member, signed)

APPS = "1570700000000000001"
GMAN = "1570702999754379264"
_n = iter(range(10 ** 6))


def make():
    gb, files = FakeGB(), FakeFiles()
    gb.files = files
    c = cm.Community(cm.Store(os.path.join(tempfile.mkdtemp(), "c.db")), token="t" * 40, get=gb, get_range=files)
    bot = FakeBot()
    mod = modbot.Mod(c, bot, PUB.hex(), GUILD, [MOD_ROLE, DEV_ROLE], OWNER, background=False,
                     apply_channel=APPS, apply_roles=[MOD_ROLE, DEV_ROLE, GMAN])
    return c, mod, bot


def interact(mod, kind, custom_id, who, **data):
    d = {"type": kind, "id": f"a{next(_n)}", "guild_id": GUILD, "member": who, "data": dict(custom_id=custom_id, **data)}
    return mod.interaction(*signed(d))[1]


def applicant(uid="555", roles=(), name="Sam @everyone"):
    m = member(uid, roles=list(roles))
    m["user"]["global_name"] = name
    m["joined_at"] = "2026-10-01T10:00:00+00:00"
    return m


def apply(mod, who, key="map_maker", why="I've made six maps for our servers and want to share them.", links="https://gamebanana.com/members/1"):
    start = interact(mod, 3, mod.custom_id("apply", 0), who)
    pick_id = start["data"]["components"][0]["components"][0]["custom_id"]
    form = interact(mod, 3, pick_id, who, values=[key])
    if form["type"] != 9:
        return form
    fields = [{"type": 1, "components": [{"type": 4, "custom_id": "why", "value": why}]},
              {"type": 1, "components": [{"type": 4, "custom_id": "links", "value": links}]}]
    return interact(mod, 5, form["data"]["custom_id"], who, components=fields)


def card_buttons(bot):
    return buttons(bot.posts[-1])


class Applying(unittest.TestCase):
    def test_anyone_can_apply_and_the_team_gets_a_card(self):
        c, mod, bot = make()
        who = applicant()
        start = interact(mod, 3, mod.custom_id("apply", 0), who)
        self.assertEqual((start["type"], start["data"]["flags"]), (4, 64), "a private message")
        options = start["data"]["components"][0]["components"][0]["options"]
        self.assertEqual([o["label"] for o in options], [n for _, n in applications.ROLES])
        ans = apply(mod, who)
        self.assertIn("Thanks", ans["data"]["content"])
        post = bot.calls[-1]
        self.assertEqual(post[1], f"/channels/{APPS}/messages", "the private applications channel")
        card = bot.posts[-1]
        self.assertEqual(card["allowed_mentions"], {"parse": []})
        text = json.dumps(card)
        self.assertIn("Map Maker", text)
        self.assertNotIn("@everyone", text.replace("@\\u200beveryone", ""))
        self.assertEqual(list(card_buttons(bot)), ["Approve", "Deny"])
        a = c.store.application(1)
        self.assertEqual((a["user_id"], a["role_key"], a["state"]), ("555", "map_maker", "pending"))

    def test_one_open_application_per_role_and_none_for_a_role_you_have(self):
        c, mod, bot = make()
        apply(mod, applicant())
        again = apply(mod, applicant())
        self.assertIn("waiting", again["data"]["content"])
        self.assertIn("Thanks", apply(mod, applicant(), key="playtester")["data"]["content"], "another role is fine")
        has = apply(mod, applicant("777", roles=[ROLE_IDS["Playtester"]]), key="playtester")
        self.assertIn("already have", has["data"]["content"])

    def test_the_form_is_checked(self):
        c, mod, bot = make()
        short = apply(mod, applicant(), why="pls")
        self.assertIn("a little more", short["data"]["content"])
        who = applicant()
        start = interact(mod, 3, mod.custom_id("apply", 0), who)
        pick_id = start["data"]["components"][0]["components"][0]["custom_id"]
        self.assertIn("Pick one", interact(mod, 3, pick_id, who, values=["admin"])["data"]["content"])
        self.assertIn("isn't one of ours", interact(mod, 5, "ls1:applyform:9:" + "0" * 20, who)["data"]["content"].lower() + "isn't one of ours")
        long = apply(mod, applicant("888"), why="x" * 5000, links="y" * 5000)
        self.assertIn("Thanks", long["data"]["content"])
        a = c.store.application(1)
        self.assertEqual((len(a["why"]), len(a["links"])), (applications.WHY_MAX, applications.LINKS_MAX))

    def test_a_day_limit(self):
        c, mod, bot = make()
        mod.apps.per_day = cm.Window(1, 86400)
        apply(mod, applicant(), key="playtester")
        self.assertIn("tomorrow", apply(mod, applicant(), key="bug_hunter")["data"]["content"])


class Deciding(unittest.TestCase):
    def setUp(self):
        self.c, self.mod, self.bot = make()
        apply(self.mod, applicant())
        self.ok = card_buttons(self.bot)["Approve"]["custom_id"]
        self.no = card_buttons(self.bot)["Deny"]["custom_id"]

    MAPPER = ROLE_IDS["🗺️・Map Maker"]

    def click(self, cid, who=None):
        return interact(self.mod, 3, cid, who or member("111", roles=[MOD_ROLE]))

    def test_only_the_team_can_decide(self):
        for who in (applicant("555"), member("222", roles=["999"])):
            self.assertEqual(self.click(self.ok, who)["data"]["content"], "You can't do that.")
        self.assertEqual(self.c.store.application(1)["state"], "pending")
        self.assertEqual(self.click(self.ok, member("333", roles=[GMAN, self.MAPPER]))["type"], 7, "G-Man may")

    def test_approve_gives_the_role_once(self):
        ans = self.click(self.ok)
        self.assertEqual(ans["type"], 7)
        put = next(c for c in self.bot.calls if c[0] == "PUT")
        self.assertEqual(put[1], f"/guilds/{GUILD}/members/555/roles/{ROLE_IDS['🗺️・Map Maker']}")
        self.assertIn("Application #1", put[3])
        self.assertIn("Approved", json.dumps(ans["data"]))
        self.assertEqual(list(buttons(ans["data"])), ["Undo"])
        self.assertTrue(any("Audit: Application #1" in (p.get("content") or "") for p in self.bot.posts))
        again = self.click(self.ok, member("444", roles=[DEV_ROLE, self.MAPPER]))
        self.assertIn("Already approved by user111", again["data"]["content"])
        self.assertEqual(sum(1 for c in self.bot.calls if c[0] == "PUT"), 1)

    def test_a_role_above_the_bot(self):
        self.bot.role_reply = (403, {"code": 50013, "message": "Missing Permissions"})
        ans = self.click(self.ok)
        text = json.dumps(ans["data"])
        self.assertIn("ABOVE Map Maker", text)
        self.assertIn("Loner-Strike Setup", text)
        self.assertEqual(self.c.store.application(1)["state"], "pending", "still waiting: fix the roles, press again")
        self.assertEqual(list(buttons(ans["data"])), ["Approve", "Deny"])
        self.bot.role_reply = (204, {})
        self.assertEqual(self.c.store.application(1)["state"], "pending")
        self.click(self.ok)
        self.assertEqual(self.c.store.application(1)["state"], "approved")
        self.assertNotIn("ABOVE", json.dumps(self.mod.apps.card(1)))

    def test_a_role_that_doesnt_exist(self):
        del self.bot.roles["🗺️・Map Maker"]
        self.mod.apps._roles = {}
        ans = self.click(self.ok)
        self.assertIn("no role called Map Maker", json.dumps(ans["data"]))
        self.assertEqual(self.c.store.application(1)["state"], "pending")

    def deny(self, reason):
        form = self.click(self.no)
        self.assertEqual(form["type"], 9)
        rows = [{"type": 1, "components": [{"type": 4, "custom_id": "reason", "value": reason}]}]
        return interact(self.mod, 5, form["data"]["custom_id"], member("111", roles=[MOD_ROLE, self.MAPPER]), components=rows)

    def test_deny_tells_them_politely_and_starts_a_wait(self):
        ans = self.deny("Show us a finished map first @here")
        self.assertIn("Denied", json.dumps(ans["data"]))
        dm = self.bot.dms[-1]
        self.assertIn("Thanks for applying for Map Maker", dm["content"])
        self.assertIn("finished map", dm["content"])
        self.assertEqual(dm["allowed_mentions"], {"parse": []})
        again = apply(self.mod, applicant())
        self.assertIn("again from", again["data"]["content"])

    def test_closed_dms_are_noted(self):
        self.bot.dm_reply = (403, {"code": 50007})
        ans = self.deny("")
        self.assertIn("DMs are closed", json.dumps(ans["data"]))
        self.assertEqual(self.c.store.application(1)["state"], "denied")

    def test_undo(self):
        ans = self.click(self.ok)
        ans = self.click(buttons(ans["data"])["Undo"]["custom_id"])
        self.assertTrue(any(c[0] == "DELETE" and c[1].endswith(f"/members/555/roles/{ROLE_IDS['🗺️・Map Maker']}") for c in self.bot.calls))
        self.assertEqual(self.c.store.application(1)["state"], "pending")
        self.assertEqual(list(buttons(ans["data"])), ["Approve", "Deny"])
        ans = self.deny("no")
        ans = self.click(buttons(ans["data"])["Undo"]["custom_id"])
        self.assertIn("weren't told again", json.dumps(ans["data"]))

    def test_forged_and_replayed(self):
        action, n, mac = self.ok.split(":")[1:]
        self.assertIn("isn't one of ours", self.click(f"ls1:appok:{int(n) + 1}:{mac}")["data"]["content"].lower() + "isn't one of ours")
        d = {"type": 3, "id": "same", "guild_id": GUILD, "member": member("111", roles=[MOD_ROLE, self.MAPPER]), "data": {"custom_id": self.ok}}
        self.assertEqual(self.mod.interaction(*signed(d))[1]["type"], 7)
        self.assertEqual(self.mod.interaction(*signed(d))[1]["data"]["content"], "Already done.")

    def test_no_one_decides_on_their_own_application(self):
        apply(self.mod, applicant("111", roles=[MOD_ROLE]), key="playtester")
        mine = card_buttons(self.bot)
        tester = ROLE_IDS["Playtester"]
        for b in ("Approve", "Deny"):
            ans = self.click(mine[b]["custom_id"], member("111", roles=[MOD_ROLE, tester]))
            self.assertIn("your own application", ans["data"]["content"])
        self.assertEqual(self.c.store.application(2)["state"], "pending")
        self.assertEqual(self.click(mine["Approve"]["custom_id"], member("444", roles=[DEV_ROLE, tester]))["type"], 7, "someone else may")

    def test_earned_roles_from_any_decider(self):
        for who in (member("444", roles=[DEV_ROLE]), member("333", roles=[GMAN])):
            c, mod, bot = make()
            apply(mod, applicant())
            ok = card_buttons(bot)["Approve"]["custom_id"]
            ans = interact(mod, 3, ok, who)
            self.assertEqual(ans["type"], 7)
            self.assertEqual(c.store.application(1)["state"], "approved")
            interact(mod, 3, buttons(ans["data"])["Undo"]["custom_id"], who)
            self.assertEqual(c.store.application(1)["state"], "pending", "and undo them")

    def test_privileged_roles_can_be_added(self):
        c, mod, bot = make()
        mod.apps = applications.Applications(mod, APPS, [MOD_ROLE, DEV_ROLE, GMAN], ["Bug Hunter"])
        apply(mod, applicant(), key="bug_hunter")
        ok = card_buttons(bot)["Approve"]["custom_id"]
        self.assertIn("Only someone who has the Bug Hunter role", interact(mod, 3, ok, member("444", roles=[DEV_ROLE]))["data"]["content"])
        self.assertEqual(interact(mod, 3, ok, member("445", roles=[DEV_ROLE, ROLE_IDS["Bug Hunter"]]))["type"], 7)

    def test_moderator_applications(self):
        apply(self.mod, applicant("999"), key="moderator")
        ok = card_buttons(self.bot)["Approve"]["custom_id"]
        for who in (member("444", roles=[DEV_ROLE]), member("333", roles=[GMAN])):
            self.assertIn("Only someone who has the Moderator role", self.click(ok, who)["data"]["content"],
                          "a Dev or G-Man can't make a Moderator")
        ans = self.click(ok, member("111", roles=[MOD_ROLE]))
        self.assertEqual(ans["type"], 7, "a Moderator can")
        self.assertEqual(self.c.store.application(2)["state"], "approved")
        undo = buttons(ans["data"])["Undo"]["custom_id"]
        self.assertIn("Only someone", self.click(undo, member("444", roles=[DEV_ROLE]))["data"]["content"], "Undo follows the same rule")
        self.assertEqual(self.c.store.application(2)["state"], "approved")
        self.click(undo, member(OWNER, roles=[]))
        self.assertEqual(self.c.store.application(2)["state"], "pending", "the owner can undo")

    def test_a_moderator_application_says_what_it_gives(self):
        apply(self.mod, applicant("999"), key="moderator")
        self.assertIn("Moderator role only", json.dumps(self.bot.posts[-1]))
        ok = card_buttons(self.bot)["Approve"]["custom_id"]
        self.click(ok, member(OWNER, roles=[]))
        puts = [c for c in self.bot.calls if c[0] == "PUT"]
        self.assertEqual(len(puts), 1)
        self.assertTrue(puts[0][1].endswith(f"/roles/{ROLE_IDS['Moderator']}"), "exactly the Moderator role, nothing more")


class Setup(unittest.TestCase):
    def load(self):
        spec = importlib.util.spec_from_file_location("discord_setup", os.path.join(HERE, "discord-setup.py"))
        m = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(m)
        return m

    def fake(self):
        state = {"channels": [{"id": "c1", "type": 4, "name": "💬 Community"}, {"id": "c2", "type": 4, "name": "🛡 Staff"}],
                 "pins": {}, "made": [], "posted": [], "pinned": []}

        def ask(method, path, body=None):
            if path == "/users/@me":
                return {"id": "4242"}
            if path.endswith("/channels") and method == "GET":
                return state["channels"]
            if path.endswith("/channels") and method == "POST":
                ch = dict(body, id=f"new{len(state['made'])}")
                state["channels"].append(ch)
                state["made"].append(ch)
                return ch
            if path.endswith("/pins") and method == "GET":
                return state["pins"].get(path.split("/")[2], [])
            if path.endswith("/messages") and method == "POST":
                msg = dict(body, id="m1", author={"id": "4242"})
                state["posted"].append((path, msg))
                return msg
            if "/pins/" in path and method == "PUT":
                ch = path.split("/")[2]
                state["pins"].setdefault(ch, []).append(state["posted"][-1][1])
                state["pinned"].append(path)
                return {}
            if path.endswith("/roles"):
                return [{"id": "r1", "name": "Map Maker", "position": 5}, {"id": "bot", "name": "Loner-Strike Setup", "position": 3},
                        {"id": "r2", "name": "Playtester", "position": 1}]
            if "/members/" in path:
                return {"roles": ["bot"]}
            raise AssertionError(path)
        return state, ask

    def test_dry_run_changes_nothing(self):
        state, ask = self.fake()
        said = []
        self.assertIsNone(self.load().setup_apply("tok", ask, False, said.append))
        self.assertEqual((state["made"], state["posted"]), ([], []))
        self.assertTrue(any("Would make" in s for s in said))

    def test_apply_makes_channels_and_a_signed_pinned_button_once(self):
        m = self.load()
        state, ask = self.fake()
        said = []
        apps_id = m.setup_apply("tok", ask, True, said.append)
        apply_ch, private = state["made"]
        self.assertEqual((apply_ch["parent_id"], private["parent_id"]), ("c1", "c2"))
        everyone = {o["id"]: o for o in apply_ch["permission_overwrites"]}[GUILD]
        self.assertTrue(int(everyone["deny"]) & m.SEND and int(everyone["allow"]) & m.VIEW, "read-only for members")
        hidden = {o["id"]: o for o in private["permission_overwrites"]}
        self.assertTrue(int(hidden[GUILD]["deny"]) & m.VIEW, "private")
        self.assertTrue(all(r in hidden for r in m.TEAM_ROLES))
        self.assertEqual(apps_id, private["id"])
        button = state["posted"][0][1]["components"][0]["components"][0]
        self.assertEqual(modbot.Mod.parse_id(type("M", (), {"secret": modbot.button_secret("tok")})(), button["custom_id"]), ("apply", 0))
        self.assertEqual(len(state["pinned"]), 1)
        self.assertTrue(any("below Map Maker" in s for s in said), "warns about the role order")
        self.assertTrue(any("no role called Mode Maker" in s for s in said))
        m.setup_apply("tok", ask, True, said.append)
        self.assertEqual((len(state["made"]), len(state["posted"])), (2, 1), "a second run changes nothing")


class Store(unittest.TestCase):
    def test_retention(self):
        """Two years of history; a block's history while it's in place, then two years from its lifting;
        delisted entries, their uid records and old cards go; reports 90 days, interaction ids 1 day."""
        _c, mod, _bot = make()
        s, day, t = mod.c.store, 86400, cm.now()
        old, recent = t - 800 * day, t - 30 * day
        def entry(key, state, updated, sid):
            row = cm.new_row("Mod", int(key[4:]), updated)
            row.update(kind="map", state=state, submitter_id=sid, submitter=f"m{sid}", item="{}", updated=updated)
            s.put(row, "test")
        entry("mod:1", "pulled", old, 10)      # out of the list for over two years: goes
        entry("mod:2", "listed", old, 20)      # listed: stays however old
        entry("mod:3", "rejected", recent, 30)  # turned down a month ago: stays
        s.db.execute("UPDATE log SET at=?", (old,))
        s.db.execute("INSERT INTO log (key, at, action, detail) VALUES (?,?,?,?)", ("mod:2", recent, "list", ""))
        s.set_blocked(40, "blocked one", "mod", "spam", True)       # still blocked: history stays
        s.set_blocked(50, "lifted long ago", "mod", "x", True)
        s.set_blocked(50, "", "mod", "", False)                      # lifted
        s.set_blocked(60, "lifted lately", "mod", "x", True)
        s.set_blocked(60, "", "mod", "", False)
        s.db.execute("UPDATE log SET at=? WHERE key IN ('member:40', 'member:50')", (old,))
        s.db.execute("UPDATE log SET at=? WHERE key='member:60' AND action='block'", (old,))  # blocked long ago, lifted lately
        s.db.execute("INSERT INTO uids (uid, owner_id, owner, key, at, by) VALUES ('u7', 1, 'x', '', ?, 'lsmod')", (recent,))
        s.db.execute("INSERT INTO uids (uid, owner_id, owner, key, at, by) VALUES ('u8', 1, 'x', '', ?, 'lsmod')", (old,))
        for uid, key in (("u1", "mod:1"), ("u2", "mod:2"), ("u9", "mod:999")):
            s.db.execute("INSERT INTO uids (uid, owner_id, owner, key, at, by) VALUES (?,?,?,?,?,?)", (uid, 1, "x", key, old, "t"))
        s.db.execute("INSERT INTO uid_conflicts (uid, holder, other, why, at) VALUES ('u2', 'mod:2', 'mod:3', '', ?)", (old,))
        s.db.execute("INSERT INTO uid_conflicts (uid, holder, other, why, at) VALUES ('u2', 'mod:2', 'mod:3', '', ?)", (recent,))
        s.db.execute("INSERT INTO cards (kind, key, at) VALUES ('listing', 'mod:2', ?)", (old,))
        s.db.execute("INSERT INTO cards (kind, key, at) VALUES ('listing', 'mod:2', ?)", (recent,))
        s.db.execute("INSERT INTO reports (key, at, reason, note, reporter, app) VALUES ('mod:2', ?, 'other', '', 'r', '')", (t - 91 * day,))
        s.db.execute("INSERT INTO interactions (id, at) VALUES ('i1', ?)", (t - 2 * day,))
        gone = s.housekeeping()
        keys = lambda q: [r[0] for r in s.db.execute(q)]
        self.assertEqual(sorted(keys("SELECT key FROM entries")), ["mod:2", "mod:3"])
        self.assertEqual(sorted(keys("SELECT uid FROM uids")), ["u2", "u7"], "uid records of gone entries go; a recent moderator assignment stays")
        self.assertEqual(len(keys("SELECT id FROM uid_conflicts")), 1)
        self.assertEqual(len(keys("SELECT id FROM cards")), 1)
        log = keys("SELECT key FROM log")
        self.assertIn("member:40", log, "a block in place keeps its history")
        self.assertNotIn("member:50", log, "lifted over two years ago: gone")
        self.assertEqual(log.count("member:60"), 2, "lifted lately: its whole history counts from the lifting")
        self.assertEqual(log.count("mod:2"), 1, "only the recent line of a listed entry")
        self.assertNotIn("mod:1", log)
        self.assertEqual((gone["reports"], gone["interactions"]), (1, 1))

    def test_applications_go_after_180_days(self):
        """The privacy page: deleted 180 days after the decision, or after sending when undecided."""
        _c, mod, _bot = make()
        s = mod.c.store
        day = 86400
        old_decided = s.add_application(user_id="1", role_key="playtester", state="denied", why="a", at=cm.now() - 400 * day, decided_at=cm.now() - 181 * day)
        recent_decision = s.add_application(user_id="2", role_key="playtester", state="granted", why="b", at=cm.now() - 400 * day, decided_at=cm.now() - 10 * day)
        old_pending = s.add_application(user_id="3", role_key="playtester", state="pending", why="c", at=cm.now() - 181 * day)
        new_pending = s.add_application(user_id="4", role_key="playtester", state="pending", why="d", at=cm.now() - 5 * day)
        s.add_application(user_id="5", role_key="playtester", state="pending", why="e")  # adding runs the housekeeping
        self.assertIsNone(s.application(old_decided))
        self.assertIsNone(s.application(old_pending))
        self.assertIsNotNone(s.application(recent_decision), "decided 10 days ago: kept")
        self.assertIsNotNone(s.application(new_pending))
        s.db.execute("UPDATE applications SET decided_at=? WHERE id=?", (cm.now() - 200 * day, recent_decision))
        self.assertEqual(s.prune_applications(), 1)
        self.assertIsNone(s.application(recent_decision))

    def test_migrated_to_newest(self):
        s = cm.Store(os.path.join(tempfile.mkdtemp(), "c.db"))
        self.assertEqual(s.db.execute("PRAGMA user_version").fetchone()[0], len(cm.MIGRATIONS))
        i = s.add_application(user_id="1", role_key="playtester", state="pending", why="because")
        self.assertEqual(s.application(i)["state"], "pending")


if __name__ == "__main__":
    unittest.main()
