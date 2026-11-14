#!/usr/bin/env python3
# Tests for community creations, offline:
#   cd services/serverlist && python3 -m unittest test_community test_serverlist
# GameBanana answers come from testdata/gamebanana/: real ProfilePage answers recorded
# 2026-11-12 (long text, mod manager and credit lists cut), with the LonerStrike tag added
# (no real page carries it yet) and the map's and spray's file names changed to what Publish
# uploads (a .zip; a single .wad); mod_739197_untagged.json is otherwise the page as recorded.
# Package listings come from zips made here (FakeFiles); a live check of the real thing:
#   LS_LIVE=1 python3 -m unittest test_community.Live
import copy
import hashlib
import io
import json
import os
import sqlite3
import struct
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.error
import urllib.request
import zipfile
from contextlib import redirect_stderr
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import community as cm  # noqa: E402
import serverlist as sl  # noqa: E402

DATA = os.path.join(HERE, "testdata", "gamebanana")
TOKEN = "t" * 40


def fixture(name):
    with open(os.path.join(DATA, name)) as f:
        return json.load(f)


MAP = fixture("mod_739197_map.json")
MODE = fixture("mod_739703_mode.json")
PREFAB = fixture("mod_706629_prefab.json")
SPRAY = fixture("spray_82585.json")


class FakeGB:
    """GameBanana's API from fixtures: pages[(model, id)] = page dict, None (no such page) or
    an exception (network trouble). Counts calls."""

    def __init__(self, pages=None):
        self.pages = pages if pages is not None else {
            ("Mod", 739197): MAP, ("Mod", 739703): MODE, ("Mod", 706629): PREFAB, ("Spray", 82585): SPRAY}
        self.calls = []

    def __call__(self, url, limit, **kw):
        self.calls.append(url)
        assert url.startswith("https://gamebanana.com/apiv11/") and url.endswith("/ProfilePage"), url
        model, gb = url.split("/")[4], int(url.split("/")[5])
        page = self.pages.get((model, gb), None)
        if isinstance(page, Exception):
            raise page
        if page is None:
            return 404, json.dumps(fixture("not_found.json")).encode()
        page = copy.deepcopy(page)
        # the size GameBanana lists for a file is the size of the zip the fake file host serves
        for f in page.get("_aFiles", []):
            data = getattr(self, "files", None) and self.files.files.get(f["_idRow"])
            if isinstance(data, bytes):
                f["_nFilesize"] = len(data)
        return 200, json.dumps(page).encode()


def manifest(item):
    return json.dumps({"schema": 1, "app": "1.0.0", "item": item}).encode()


MAP_MANIFEST = {"kind": "map", "gb": 0, "name": "de_marakesh (from the package)", "summary": "A market town.", "games": ["cstrike"],
                "tags": ["desert"], "map": {"name": "de_marakesh", "bspSize": 1, "bspSha256": "ab" * 32, "modes": ["classic", "csdm"],
                                            "nav": True, "places": True, "lsv": True, "res": True, "overview": True},
                "health": {"verdict": "good", "when": 1793990000, "mode": "classic", "minutes": 5, "bots": 12,
                           "stuckPct": 0.8, "falls": 0, "killsPerBotMin": 1.2, "coveragePct": 61.0}}


def tiny_bsp():
    """The smallest map formats.py takes: a world model and an entity list."""
    ents = b'{\n"classname" "worldspawn"\n}\n\0'
    lumps = [(0, 0)] * 15
    lumps[0] = (124, len(ents))
    lumps[14] = (124 + len(ents), 64)
    head = struct.pack("<i", 30) + b"".join(struct.pack("<ii", o, n) for o, n in lumps)
    return head + ents + bytes(64)


def sample(name):
    """Valid content for a file of this name (what the format checks expect)."""
    ext = name.lower().rsplit(".", 1)[-1]
    if ext == "bsp":
        return tiny_bsp()
    if ext == "nav":
        return struct.pack("<IIIHI", 0xFEEDFACE, 5, 0, 0, 0)
    if ext == "lsv":
        return struct.pack("<10If", 0x3256534C, 6, 0, 0, 4, 0, 0, 0, 0, 0, 0.0)
    if ext == "wav":
        pcm = bytes(100)
        return b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, 22050, 22050, 1, 8) \
            + b"data" + struct.pack("<I", len(pcm)) + pcm
    if ext == "wad":
        return b"WAD3" + struct.pack("<ii", 0, 12)
    if ext == "bmp":  # 1x1, 24-bit
        return b"BM" + struct.pack("<IHHI", 58, 0, 0, 54) + struct.pack("<IiiHHIIiiII", 40, 1, 1, 1, 24, 0, 4, 0, 0, 0, 0) + bytes(4)
    if ext == "tga":  # 1x1, uncompressed 24-bit
        return struct.pack("<BBBHHBHHHHBB", 0, 0, 2, 0, 0, 0, 0, 0, 1, 1, 24, 0) + bytes(3)
    if ext == "json":
        return b"{}"
    if ext == "png":
        return b"\x89PNG\r\n\x1a\n" + bytes(32)
    return b"text " * 20


def zip_of(names, files=None):
    """A zip with these paths (each with valid sample content) and `files` as given."""
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w", zipfile.ZIP_DEFLATED) as z:
        for n in names:
            z.writestr(n, sample(n))
        for n, data in (files or {}).items():
            z.writestr(n, data)
    return b.getvalue()


def map_zip(extra=(), man=None):
    return zip_of(["maps/de_marakesh.bsp", "maps/de_marakesh.nav", "maps/de_marakesh.lsv", "README.txt", *extra],
                  {"loner-strike.json": manifest(man or MAP_MANIFEST)})


class FakeFiles:
    """GameBanana's download hosts for range requests: files[file id] = zip bytes."""

    def __init__(self):
        self.files = {
            1885247: map_zip(),
            1886785: zip_of(["addons/amxmodx/configs/gamemodes/cage.ini", "modes/cage/graph.json"],
                            {"loner-strike.json": manifest({"kind": "mode", "name": "Cage", "mode": {"id": "cage", "maps": ["de_dust2"], "plugins": []}})}),
            1873526: sample("tempdecal.wad"),
            2000000: sample("tempdecal.wad"),  # a newer spray file (map tests put a zip here)
            1789868: zip_of(["prefabs/can.map", "prefabs/can.json", "prefabs/can.png"],
                            {"loner-strike.json": manifest({"kind": "prefab", "name": "Can", "tags": ["props"]})}),
        }
        self.calls = 0

    def __call__(self, url, start, end):
        self.calls += 1
        fid = int(url.rsplit("/", 1)[1])
        data = self.files[fid]
        if isinstance(data, Exception):
            raise data
        return data[start:end + 1], url


def map_item(**kw):
    it = {"kind": "map", "model": "Mod", "gb": 739197, "file": 1885247, "md5": "bb48b1faf89c4daad7545c4f3a2ce681",
          "name": "de_marakesh", "author": "someone else", "summary": "A market town.", "games": ["cstrike"],
          "map": {"name": "de_marakesh", "bspSize": 2394112, "bspSha256": "ab" * 32, "modes": ["classic", "csdm"],
                  "nav": True, "places": True, "lsv": True, "res": True, "overview": False},
          "health": {"verdict": "good", "when": 1793990000, "mode": "classic", "minutes": 5, "bots": 12,
                     "stuckPct": 0.8, "falls": 0, "killsPerBotMin": 1.2, "coveragePct": 61.0},
          "screenshots": ["https://evil.example/x.png"], "tags": ["desert", "small"],
          "featured": True, "status": "listed", "added": 1, "updated": 2}
    it.update(kw)
    return it


def mode_item(plugins=False, **kw):
    it = {"kind": "mode", "model": "Mod", "gb": 739703, "file": 1886785, "md5": "77f263451516f3d9be3934e329640472",
          "name": "Cage", "mode": {"id": "cage", "review": "reviewed", "maps": ["de_dust2"],
                                   "plugins": [{"file": "gm_cage.amxx", "sha256": "cd" * 32, "source": "scripting/gm_cage.sma"}] if plugins else []}}
    it.update(kw)
    return it


def body(item, schema=1):
    return json.dumps({"schema": schema, "app": "0.1.3", "item": item}).encode()


class Recorder:
    def __init__(self):
        self.sent = []

    def __call__(self, url, b):
        self.sent.append((url, b))
        return 204


SPRAY_ITEM = {"kind": "spray", "model": "Spray", "gb": 82585, "file": 1873526, "md5": "26600d11dcffa32874195d9a32bcc010", "name": "Chen"}


def block(c, page):
    """Blocks the GameBanana member who made a fixture page: their submissions then wait."""
    c.block(page["_aSubmitter"]["_idRow"], page["_aSubmitter"]["_sName"], "test", "testing")


def make(gb=None, token=TOKEN, hook="https://discord.com/api/webhooks/1/abc", files=None):
    tmp = tempfile.mkdtemp()
    rec = Recorder()
    gb, files = gb or FakeGB(), files or FakeFiles()
    gb.files = files
    c = cm.Community(cm.Store(os.path.join(tmp, "c.db")), token=token, notifier=cm.Notifier(hook, post=rec),
                     get=gb, get_range=files)
    c.verify_md5 = False  # the fixture pages list the real files' MD5s, not these zips' (FileChecks tests it)
    # notifications in tests go straight through
    c.notifier.say = (lambda say: (lambda text, wait=False: say(text, wait=True)))(c.notifier.say)
    return c, rec


# ---- input checks ---------------------------------------------------------------------------

class Validation(unittest.TestCase):
    def test_good_items_are_typed_and_server_fields_dropped(self):
        it = cm.clean_item(map_item())
        self.assertEqual(it["model"], "Mod")
        self.assertNotIn("author", it)
        self.assertNotIn("screenshots", it)
        self.assertNotIn("featured", it)
        self.assertEqual(it["map"]["bspSize"], 2394112)
        self.assertIsInstance(it["health"]["stuckPct"], float)
        m = cm.clean_item(mode_item())
        self.assertEqual((m["mode"]["review"], m["mode"]["plugins"]), ("none", []), "the author can't review their own mode")

    def test_uids(self):
        u = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"
        m = cm.clean_item(dict(mode_item(), uid=u.upper(), remixOf={"model": "Mod", "gb": 5, "uid": u, "name": "Lava", "author": "jam"}))
        self.assertEqual((m["uid"], m["remixOf"]["uid"], m["remixOf"]["gb"]), (u, u, 5))
        mo = dict(mode_item()["mode"], uid=u)
        self.assertEqual(cm.clean_item(dict(mode_item(), mode=mo))["mode"]["uid"], u)
        old = cm.clean_item(mode_item())
        self.assertNotIn("uid", old, "entries without one keep working")
        self.assertEqual(old["mode"]["uid"], "")
        mp = dict(map_item()["map"], modeRefs=[{"uid": u, "id": "lava", "name": "Floor is lava"}, {"id": "classic"}])
        refs = cm.clean_item(dict(map_item(), map=mp))["map"]["modeRefs"]
        self.assertEqual([(r["uid"], r["id"]) for r in refs], [(u, "lava"), ("", "classic")])
        for bad in [dict(mode_item(), uid="not-a-uuid"), dict(mode_item(), uid=5),
                    dict(map_item(), map=dict(map_item()["map"], modeRefs=[{"uid": "x", "id": "a"}])),
                    dict(map_item(), map=dict(map_item()["map"], modeRefs=[{"id": "bad id"}])),
                    dict(mode_item(), remixOf={"model": "Nope", "gb": 1})]:
            with self.assertRaises(cm.Reject):
                cm.clean_item(bad)

    def test_asset_credits(self):
        good = [{"files": ["models/z.mdl", "sound/z/roar.wav"], "page": "https://gamebanana.com/mods/227562", "name": "Civilian Zombies",
                 "authors": [{"name": "DimeX", "role": "Original Authors", "url": "https://gamebanana.com/members/599043"}],
                 "permission": "granted", "note": "asked on Discord"}]
        it = cm.clean_item(dict(map_item(), credits=good))
        self.assertEqual(it["credits"][0]["authors"][0]["name"], "DimeX")
        self.assertEqual(it["credits"][0]["permission"], "granted")
        self.assertNotIn("credits", cm.clean_item(map_item()))
        for bad in ([dict(good[0], files=["../evil"])], [dict(good[0], page="https://evil.example/x")], [dict(good[0], permission="maybe")],
                    [dict(good[0], files=[])], [dict(good[0], authors=[{"name": "x", "url": "javascript:1"}])], [good[0]] * 65, ["x"]):
            with self.assertRaises(cm.Reject):
                cm.clean_item(dict(map_item(), credits=bad))
        row = cm.new_row("Mod", 739197, 1)
        row.update(kind="map", state="listed", submitter_id=1, submitter="x", item=json.dumps(dict(it, author="x")))
        self.assertEqual(cm.index_item(row)["credits"][0]["page"], "https://gamebanana.com/mods/227562")

    def test_block_with_pull(self):
        c, _ = make()
        c.submit(body(map_item()), "1.2.3.4")
        sid = MAP["_aSubmitter"]["_idRow"]
        pulled = c.block(sid, "x", "test", "Repeated copyright notices.", pull=True)
        self.assertEqual(pulled, ["mod:739197"])
        self.assertEqual((c.store.get("mod:739197")["state"], c.store.get("mod:739197")["reason"]), ("pulled", "Repeated copyright notices."))
        self.assertEqual(c.block(999999, "y", "test", ""), [], "a plain block pulls nothing")

    def test_bot_rating(self):
        self.assertEqual(cm.clean_item(dict(mode_item(), bots="ok"))["bots"], "ok")
        self.assertNotIn("bots", cm.clean_item(dict(map_item(), bots="ok")), "only modes carry it")
        self.assertNotIn("bots", cm.clean_item(mode_item()))
        with self.assertRaises(cm.Reject):
            cm.clean_item(dict(mode_item(), bots="superb"))

    def test_mode_requirements_are_kept_and_cleaned(self):
        it = cm.clean_item(mode_item(mode={"id": "cage", "maps": [], "requirements": {
            "defaultMap": "de_dust2", "maps": ["de_dust2", "a b", "de_dust2"],
            "content": [{"path": "Models/Crate.mdl", "link": {"model": "Mod", "id": 77, "name": "Crates"}},
                        {"path": "../../x.mdl"}, {"path": "sound/beep.wav", "link": {"model": "Skin", "id": 1}}]}}))
        r = it["mode"]["requirements"]
        self.assertEqual(r["maps"], ["de_dust2"])
        self.assertEqual(r["content"], [{"path": "models/crate.mdl", "kind": "model", "link": {"model": "Mod", "id": 77, "name": "Crates"}},
                                        {"path": "sound/beep.wav", "kind": "sound"}])
        with self.assertRaises(cm.Reject):
            cm.clean_item(mode_item(mode={"id": "cage", "requirements": {"content": "nope"}}))

    def test_mode_needs_are_kept_and_cleaned(self):
        # what a mode needs beyond the game (QA4-G F6): components, creatures, the bot brain's versions
        it = cm.clean_item(mode_item(mode={"id": "cage", "maps": [], "requirements": {"maps": [], "content": [], "needs": {
            "componentsPlugin": True, "components": ["ls_wave_spawner", "../x", "ls_wave_spawner"], "creatures": ["zombie", "a b"],
            "modeVer": 2, "creatureVer": 99,
            "features": [{"id": "influence", "name": "Personality  influence", "brain": "mode", "ver": 2},
                         {"id": "x y", "name": "n", "brain": "mode", "ver": 1}, {"id": "f", "name": "n", "brain": "kernel", "ver": 1}]}}}))
        n = it["mode"]["requirements"]["needs"]
        self.assertEqual(n, {"componentsPlugin": True, "components": ["ls_wave_spawner"], "creatures": ["zombie"], "modeVer": 2, "creatureVer": 9,
                             "features": [{"id": "influence", "name": "Personality influence", "brain": "mode", "ver": 2}]})
        # needing nothing: not kept
        it = cm.clean_item(mode_item(mode={"id": "cage", "maps": [], "requirements": {"needs": {"components": [], "features": []}}}))
        self.assertNotIn("needs", it["mode"]["requirements"])
        for bad in ("nope", {"modeVer": "2"}, {"modeVer": -1}, {"features": ["x"]}):
            with self.assertRaises(cm.Reject):
                cm.clean_item(mode_item(mode={"id": "cage", "requirements": {"needs": bad}}))

    def test_mode_capabilities_are_kept_and_cleaned(self):
        it = cm.clean_item(mode_item(mode={"id": "cage", "maps": [], "capabilities": ["kicks_players", "server_commands", "format_disk", "kicks_players"]}))
        self.assertEqual(it["mode"]["capabilities"], ["kicks_players", "server_commands"])
        self.assertNotIn("capabilities", cm.clean_item(mode_item())["mode"])
        with self.assertRaises(cm.Reject):
            cm.clean_item(mode_item(mode={"id": "cage", "capabilities": "server_commands"}))

    def test_plugin_modes_cant_be_shared(self):
        with self.assertRaises(cm.Reject) as e:
            cm.clean_item(mode_item(plugins=True))
        self.assertEqual(e.exception.code, 422)
        self.assertIn("Game Mode Studio", e.exception.message)

    def test_junk_is_refused(self):
        bad = [
            None, [], {"kind": "virus"}, map_item(kind="spray"), map_item(model="Spray"), map_item(gb=0), map_item(gb="1"),
            map_item(gb=True), map_item(file=0), map_item(md5="xyz"), map_item(md5=5), map_item(name=""), map_item(name=7),
            map_item(name="x" * 1000), map_item(games=["hl"]), map_item(tags="a"), map_item(tags=[1]), map_item(map=None),
            map_item(map={"name": "../../etc"}), map_item(map={"name": "de_" + "x" * 40}), map_item(map={"name": "a b"}),
            map_item(health={"verdict": "great"}), map_item(health={"verdict": "good", "minutes": -1}),
            map_item(health={"verdict": "good", "stuckPct": float("nan")}), map_item(mode={"id": "x"}),
            mode_item(mode={"id": "bad id"}), mode_item(mode=None), mode_item(mode={"id": "a", "maps": ["a b"]}),
        ]
        for b in bad:
            with self.subTest(b=str(b)[:80]):
                with self.assertRaises(cm.Reject) as e:
                    cm.clean_item(b)
                self.assertEqual(e.exception.code, 400)

    def test_text_is_one_safe_line(self):
        it = cm.clean_item(map_item(name="de_‮map\x00 \n two", summary="a\r\nb"))
        self.assertEqual(it["name"], "de_ map two")
        self.assertEqual(it["summary"], "a b")

    def test_submission_envelope(self):
        self.assertEqual(cm.parse_submission(body(map_item())), ("Mod", 739197, 1885247, "0.1.3"))
        page = json.dumps({"schema": 1, "app": "1.0", "page": {"model": "Spray", "id": 5, "file": 6}}).encode()
        self.assertEqual(cm.parse_submission(page), ("Spray", 5, 6, "1.0"))
        for raw, code in [(b"x" * (cm.MAX_BODY + 1), 413), (b"not json", 400), (b"[]", 400), (b"\xff", 400),
                          (body(map_item(), schema=2), 400), (body(map_item(), schema=0), 400),
                          (json.dumps({"schema": 1}).encode(), 400), (body(map_item(gb=0)), 400), (body(map_item(file="1")), 400),
                          (body(map_item(model="Wip")), 400)]:
            with self.subTest(raw=raw[:30]):
                with self.assertRaises(cm.Reject) as e:
                    cm.parse_submission(raw)
                self.assertEqual(e.exception.code, code)

    def test_keys(self):
        self.assertEqual(cm.parse_key("mod:12"), ("Mod", 12))
        self.assertEqual(cm.parse_key("Spray:3"), ("Spray", 3))
        self.assertEqual(cm.parse_key("https://gamebanana.com/mods/739197"), ("Mod", 739197))
        self.assertEqual(cm.parse_key("gamebanana.com/sprays/5?x=1"), ("Spray", 5))
        for bad in ("mod:0", "mod:-1", "wip:3", "https://evil.com/mods/1", "mod:123456789", ""):
            self.assertIsNone(cm.parse_key(bad), bad)


# ---- GameBanana pages -------------------------------------------------------------------------

class Pages(unittest.TestCase):
    def test_tag_shapes(self):
        self.assertTrue(cm.has_tag([{"_sTitle": "Software Used", "_sValue": "Hammer, LonerStrike"}]))
        self.assertTrue(cm.has_tag(["Software Used: Loner-Strike"]))
        self.assertFalse(cm.has_tag(["Software Used: Hammer"]))
        self.assertFalse(cm.has_tag([{"_sTitle": "Genre", "_sValue": "LonerStrike"}]))
        self.assertFalse(cm.has_tag("Software Used: LonerStrike"))

    def test_kinds_from_categories(self):
        self.assertEqual(cm.page_kind(MAP, "Mod"), "map")
        self.assertEqual(cm.page_kind(MODE, "Mod"), "mode")
        self.assertEqual(cm.page_kind(PREFAB, "Mod"), "prefab")
        self.assertEqual(cm.page_kind(SPRAY, "Spray"), "spray")

    def test_check_page(self):
        facts = cm.check_page(MAP, "Mod", 739197, 1885247)
        self.assertEqual((facts["kind"], facts["title"], facts["author"], facts["submitterId"]), ("map", "de_marakesh", "TheHouthi", 6096365))
        self.assertEqual(facts["file"]["md5"], "bb48b1faf89c4daad7545c4f3a2ce681")
        self.assertTrue(facts["screenshots"] and all(u.startswith("https://images.gamebanana.com/") for u in facts["screenshots"]))
        nocat = copy.deepcopy(MAP)
        nocat["_aCategory"], nocat["_aSuperCategory"] = {"_idRow": 1, "_sName": "Skins"}, {"_idRow": 2, "_sName": "Skins"}
        cases = {
            "untagged": (fixture("mod_739197_untagged.json"), 1885247, "tag"),
            "game": (dict(MAP, _aGame={"_idRow": 1}), 1885247, "1.6"),
            "category": (nocat, 1885247, "category"),
            "flagged": (dict(MAP, _bIsFlagged=True), 1885247, "flagged"),
            "file": (MAP, 999, "isn't on the page"),
        }
        pending = copy.deepcopy(MAP)
        pending["_aFiles"][0].update(_sAvResult="", _sAvState="pending")
        cases["av pending"] = (pending, 1885247, "still checking")
        infected = copy.deepcopy(MAP)
        infected["_aFiles"][0].update(_sAvResult="infected", _sAvState="done")
        cases["av"] = (infected, 1885247, "virus")
        big = copy.deepcopy(MAP)
        big["_aFiles"][0]["_nFilesize"] = cm.MAX_FILE + 1
        cases["size"] = (big, 1885247, "too big")
        for name, (page, fid, words) in cases.items():
            with self.subTest(name):
                with self.assertRaises(cm.Reject) as e:
                    cm.check_page(page, "Mod", 739197, fid)
                self.assertEqual(e.exception.code, 422)
                self.assertIn(words, e.exception.message)

    def test_fetch_outcomes(self):
        gb = FakeGB({("Mod", 1): None, ("Mod", 2): OSError("down"), ("Mod", 739197): dict(MAP, _bIsTrashed=True)})
        self.assertEqual(cm.fetch_page("Mod", 1, gb)[0], "gone")
        self.assertEqual(cm.fetch_page("Mod", 2, lambda u, l: (_ for _ in ()).throw(cm.FetchError("x")))[0], "error")
        self.assertEqual(cm.fetch_page("Mod", 739197, gb)[0], "gone")
        self.assertEqual(cm.fetch_page("Mod", 5, lambda u, l: (200, b"<html>"))[0], "error")
        self.assertEqual(cm.fetch_page("Mod", 5, lambda u, l: (500, b"{}"))[0], "error")
        self.assertEqual(cm.fetch_page("Mod", 5, lambda u, l: (200, json.dumps(dict(MAP)).encode()))[0], "error", "another page answered")

    def test_fetching_only_gamebanana(self):
        for url in ("http://gamebanana.com/apiv11/Mod/1/ProfilePage", "https://evil.com/x",
                    "https://gamebanana.com.evil.com/x", "https://user@gamebanana.com/x", "file:///etc/passwd",
                    "https://127.0.0.1/x", "https://gamebanana.com:8443/x"):
            with self.assertRaises(cm.FetchError, msg=url):
                cm.http_get(url, 100)
        with self.assertRaises(cm.FetchError):
            cm.page_url("Mod", "1/../../x")
        with self.assertRaises(cm.FetchError):
            cm.page_url("Wip", 1)

    def test_no_redirects_and_size_cap(self):
        class H(BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def do_GET(self):
                if self.path == "/redirect":
                    self.send_response(302)
                    self.send_header("Location", "http://169.254.169.254/latest/meta-data")
                    self.end_headers()
                    return
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b"x" * 5000)

        srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        try:
            host = f"127.0.0.1:{srv.server_address[1]}"
            status, _ = cm.http_get(f"http://{host}/redirect", 100, hosts=(host,), schemes=("http",))
            self.assertEqual(status, 302, "a redirect is answered as is, never followed")
            with self.assertRaises(cm.FetchError):
                cm.http_get(f"http://{host}/big", 100, hosts=(host,), schemes=("http",))
            self.assertEqual(len(cm.http_get(f"http://{host}/big", 9000, hosts=(host,), schemes=("http",))[1]), 5000)
        finally:
            srv.shutdown()


# ---- registrations ------------------------------------------------------------------------------

class Packages(unittest.TestCase):
    def test_code_in_a_package(self):
        self.assertEqual(cm.code_files(["maps/a.bsp", "sound/x.wav", "addons/amxmodx/configs/a.ini", "dir/"]), [])
        bad = ["addons/amxmodx/plugins/x.amxx", "SCRIPTING/X.SMA", "a.dll", "b.so", "c.dylib", "d.exe", "e.bat", "f.sh",
               "inner.zip", "x.rar", "addons/metamod/plugins.ini", "cstrike/dlls/mp.dll", "tools\\run.ps1"]
        self.assertEqual(cm.code_files(bad), bad)
        with self.assertRaises(cm.Reject) as e:
            cm.check_package(["maps/a.bsp", "addons/amxmodx/plugins/x.amxx"])
        self.assertIn("Game Mode Studio", e.exception.message)
        with self.assertRaises(cm.Reject) as e:
            cm.check_package(["maps/a.bsp", "inner.7z"])
        self.assertIn("inner.7z", e.exception.message)

    def test_listing_reads_only_the_directory(self):
        names = [f"sound/s{i}.wav" for i in range(50)]
        b = io.BytesIO()
        with zipfile.ZipFile(b, "w") as z:
            for n in names:
                z.writestr(n, os.urandom(20000))  # 1 MB that never needs reading
        data = b.getvalue()
        read = []

        def get(url, start, end):
            read.append(end - start + 1)
            return data[start:end + 1], "https://filecache1.gamebanana.com/x.zip"
        self.assertEqual(cm.package_names({"id": 5, "name": "x.zip", "size": len(data)}, get), names)
        self.assertLess(sum(read), 200_000)
        self.assertLessEqual(len(read), 3 + cm.HEADER_SAMPLES, "the tail, maybe the directory, a few local headers")
        with self.assertRaises(cm.FetchError):
            cm.package_names({"id": 5, "name": "x.zip", "size": len(data)}, lambda u, s, e: (data[s:e + 1], u)) if False else \
                zipfile.ZipFile(cm.RangeFile("u", len(data), lambda u, s, e: (data[s:e + 1], u), budget=1000))

    def test_what_cant_be_looked_inside(self):
        with self.assertRaises(cm.Reject) as e:
            cm.package_names({"id": 5, "name": "x.rar", "size": 10})
        self.assertIn("zip", e.exception.message)
        with self.assertRaises(cm.Reject):
            cm.package_names({"id": 5, "name": "x.zip", "size": 100}, lambda u, s, e: (b"y" * (e - s + 1), u))
        self.assertEqual(cm.package_names({"id": 5, "name": "Pic.WAD", "size": 10}), ["Pic.WAD"])

    def test_downloads_only_from_gamebanana(self):
        ok = ["https://gamebanana.com/dl/1886785", "https://files.gamebanana.com/mods/cage.zip", "https://filecache43.gamebanana.com/mods/cage.zip"]
        bad = ["http://files.gamebanana.com/x", "https://gamebanana.com/apiv11/x", "https://evil.com/dl/1", "https://files.gamebanana.com.evil.com/x",
               "https://user@files.gamebanana.com/x", "https://files.gamebanana.com:444/x", "https://169.254.169.254/x", "https://filecacheX.gamebanana.com/x"]
        for u in ok:
            self.assertTrue(cm.file_url_ok(u), u)
        for u in bad:
            self.assertFalse(cm.file_url_ok(u), u)
            with self.assertRaises(cm.FetchError):
                cm.range_get(u, 0, 10)


class Submissions(unittest.TestCase):
    def test_content_is_listed_at_once(self):
        c, rec = make()
        code, ans = c.submit(body(map_item()), "1.2.3.4")
        self.assertEqual((code, ans["state"], ans["key"]), (200, "listed", "mod:739197"))
        it = json.loads(c.store.get("mod:739197")["item"])
        self.assertEqual(it["author"], "TheHouthi", "the author is GameBanana's submitter, not the app's word")
        self.assertNotIn("https://evil.example/x.png", it["screenshots"])
        self.assertEqual(json.loads(cm.build_index(c.store)[0])["items"][0]["gb"], 739197)
        msg = rec.sent[0][1]
        self.assertEqual(msg["allowed_mentions"], {"parse": []})
        self.assertIn("Listed map", msg["content"])
        self.assertIn("lsmod.py pull mod:739197", msg["content"])
        self.assertEqual(c.status("Mod", 739197)["state"], "listed")
        self.assertEqual(c.status("Mod", 1)["state"], "unknown")
        self.assertEqual(c.submit(body(mode_item()), "1.2.3.4")[1]["state"], "listed", "a Game Mode Studio (settings only) mode")

    def _uid_setup(self, creator_at, squatter_at, creator_old_at=None):
        """The creator's page (MODE, member 3608549) and a squatter's copy (member 999), both packages
        declaring the same uid, uploaded at the given GameBanana dates. creator_old_at: an older file
        on the creator's page that carries the uid too."""
        u = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"
        mode = {"kind": "mode", "name": "Cage", "uid": u, "mode": {"id": "cage", "uid": u, "maps": [], "plugins": []}}
        files = FakeFiles()
        files.files[1886785] = zip_of(["modes/cage/cage.graph.json"], {"loner-strike.json": manifest(mode)})
        files.files[1886000] = zip_of(["modes/cage/cage.graph.json"], {"loner-strike.json": manifest(mode)})
        files.files[2100000] = zip_of(["modes/cage/cage.graph.json"], {"loner-strike.json": manifest(dict(mode, name="Cage (copy)"))})
        creator = copy.deepcopy(MODE)
        creator["_aFiles"] = [dict(creator["_aFiles"][0], _tsDateAdded=creator_at)]
        if creator_old_at:
            creator["_aFiles"].append(dict(creator["_aFiles"][0], _idRow=1886000, _tsDateAdded=creator_old_at))
        thief = copy.deepcopy(MODE)
        thief["_idRow"] = 800001
        thief["_aSubmitter"] = dict(thief["_aSubmitter"], _idRow=999, _sName="Squatter")
        thief["_aFiles"] = [dict(thief["_aFiles"][0], _idRow=2100000, _tsDateAdded=squatter_at)]
        gb = FakeGB()
        gb.pages[("Mod", 739703)] = creator
        gb.pages[("Mod", 800001)] = thief
        c, rec = make(gb=gb, files=files)
        return c, rec, u

    def _uids_of(self, c, key, col="item"):
        return cm.item_uids(json.loads(c.store.get(key)[col] or "{}"))

    def _hide(self, c, key):
        row = c.store.get(key)
        row.update(state="hidden", hidden_by="recheck")
        c.store.put(row, "test")

    def test_an_older_upload_never_strips_a_listed_entry(self):
        """A squatter registers a copy first (uploaded later than the creator's file): both stay listed;
        the creator's older upload doesn't strip the listed squatter automatically, a moderator decides."""
        c, rec, u = self._uid_setup(creator_at=1_700_000_000, squatter_at=1_790_000_000)
        self.assertEqual(c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")[1]["state"], "listed")
        code, ans = c.submit(body(mode_item()), "1.2.3.4")
        self.assertEqual((code, ans["state"]), (200, "listed"), "the creator is never refused")
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 999, "nothing listed loses a uid automatically")
        self.assertEqual((self._uids_of(c, "mod:800001"), self._uids_of(c, "mod:739703")), ([u], []))
        conflict = c.store.db.execute("SELECT * FROM uid_conflicts").fetchone()
        self.assertIn("is older", conflict["why"])
        self.assertTrue(any("give-uid" in str(s[1]) for s in rec.sent), "the moderators are asked")
        c.store.give_uid(u, "mod:739703", "test")
        self.assertEqual((self._uids_of(c, "mod:739703"), self._uids_of(c, "mod:800001")), ([u], []))
        idx = {e["gb"]: e for e in json.loads(cm.build_index(c.store)[0])["items"]}
        self.assertEqual((idx[739703].get("uid"), idx[739703]["ownerId"]), (u, 3608549))
        self.assertNotIn("uid", idx[800001], "listed, linked to nothing")

    def test_an_older_upload_takes_from_an_unlisted_holder(self):
        c, rec, u = self._uid_setup(creator_at=1_700_000_000, squatter_at=1_790_000_000)
        c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")
        self._hide(c, "mod:800001")
        self.assertEqual(c.submit(body(mode_item()), "1.2.3.4")[1]["state"], "listed")
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549, "the older upload wins")
        self.assertEqual((self._uids_of(c, "mod:739703"), self._uids_of(c, "mod:800001")), ([u], []))

    def test_both_sides_dates_are_their_earliest_upload(self):
        """The holder's newest file is later than the newcomer's, but an older file on the holder's page
        carries the uid too: the holder's earliest upload counts, so the newcomer doesn't take it."""
        c, rec, u = self._uid_setup(creator_at=1_790_000_000, squatter_at=1_700_000_000, creator_old_at=1_600_000_000)
        c.submit(body(mode_item()), "1.2.3.4")
        self._hide(c, "mod:739703")
        self.assertEqual(c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")[1]["state"], "listed")
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549, "1600 beats 1700")
        self.assertEqual(self._uids_of(c, "mod:800001"), [])

    def test_the_holders_proven_date_never_weakens(self):
        """The holder proved an old upload (1600) when it registered; that file then leaves the page, so a
        fresh look finds only its newest (1790). A later claimant (1700) must still lose."""
        c, rec, u = self._uid_setup(creator_at=1_790_000_000, squatter_at=1_700_000_000, creator_old_at=1_600_000_000)
        c.submit(body(mode_item()), "1.2.3.4")
        self.assertEqual(c.store.uid_owner(u)["file_at"], 1_600_000_000, "the earliest proven date is stored")
        page = c.get.pages[("Mod", 739703)]
        page["_aFiles"] = [f for f in page["_aFiles"] if f["_idRow"] != 1886000]
        self._hide(c, "mod:739703")
        self.assertEqual(c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")[1]["state"], "listed")
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549, "the stored 1600 still beats 1700")
        self.assertEqual(c.store.uid_owner(u)["file_at"], 1_600_000_000, "and it never went up")
        self.assertEqual(self._uids_of(c, "mod:800001"), [])

    def test_a_held_submission_takes_nothing_until_approved(self):
        c, rec, u = self._uid_setup(creator_at=1_700_000_000, squatter_at=1_790_000_000)
        c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")
        self._hide(c, "mod:800001")
        block(c, MODE)
        self.assertEqual(c.submit(body(mode_item()), "1.2.3.4")[1]["state"], "pending")
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 999, "a blocked creator's submission takes nothing")
        self.assertTrue(c.store.db.execute("SELECT 1 FROM uid_conflicts WHERE why LIKE '%waits for a moderator%'").fetchone())
        c.moderate("mod:739703", "approve", {"reason": ""}, live=False)
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549, "settled when approved")
        self.assertEqual(self._uids_of(c, "mod:739703"), [u])

    def test_a_pin_strips_a_waiting_proposal_too(self):
        c, rec, u = self._uid_setup(creator_at=1_700_000_000, squatter_at=1_790_000_000)
        c.submit(body(mode_item()), "1.2.3.4")
        block(c, {"_aSubmitter": {"_idRow": 999, "_sName": "Squatter"}})
        c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")
        self.assertEqual(self._uids_of(c, "mod:800001", "proposed"), [u], "waiting, still declaring it")
        c.store.give_uid(u, "mod:739703", "test")
        self.assertEqual(self._uids_of(c, "mod:800001", "proposed"), [], "the pin reaches proposals")
        c.moderate("mod:800001", "approve", {"reason": ""}, live=False)
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549)
        self.assertEqual(self._uids_of(c, "mod:800001"), [])

    def test_unproven_dates_go_to_a_moderator(self):
        for creator_at, squatter_at in ((1_750_000_000, 1_750_000_000), (0, 1_750_000_000), (1_750_000_000, 0)):
            c, rec, u = self._uid_setup(creator_at, squatter_at)
            self.assertEqual(c.submit(body(mode_item(gb=800001, file=2100000)), "5.6.7.8")[0], 200)
            self._hide(c, "mod:800001")
            self.assertEqual(c.submit(body(mode_item()), "1.2.3.4")[0], 200, "never blocked")
            self.assertEqual(c.store.uid_owner(u)["owner_id"], 999, "unproven: the holder keeps it for now")
            self.assertEqual(self._uids_of(c, "mod:739703"), [])
            cid = c.store.db.execute("SELECT id FROM uid_conflicts").fetchone()[0]
            self.assertTrue(any("give-uid" in str(s[1]) for s in rec.sent))
            c.store.give_uid(u, "mod:739703", "test")
            self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549)
            self.assertEqual((self._uids_of(c, "mod:739703"), self._uids_of(c, "mod:800001")), ([u], []))
            self.assertEqual(c.store.uid_conflict(cid)["settled"], "mod:739703")
            c.submit(body(mode_item(gb=800001, file=2100000)), "9.9.9.9")
            self.assertEqual(c.store.uid_owner(u)["owner_id"], 3608549, "a pin beats any date")

    def test_settling_rules(self):
        h = {"owner_id": 1, "file_at": 100, "pinned": 0}
        self.assertEqual(cm.settle_uid(None, 2, 50), "same")
        self.assertEqual(cm.settle_uid(h, 1, 999), "same")
        self.assertEqual(cm.settle_uid(h, 2, 50), "take")
        self.assertEqual(cm.settle_uid(h, 2, 50, holder_listed=True), "flag", "nothing listed loses it automatically")
        self.assertEqual(cm.settle_uid(h, 2, 50, may_take=False), "flag", "a held submission never takes")
        self.assertEqual(cm.settle_uid(h, 2, 50, holder_at=40), "keep", "the holder's earliest upload counts")
        self.assertEqual(cm.settle_uid(h, 2, 90, holder_at=300), "take", "a later fresh look never raises the stored 100")
        self.assertEqual(cm.settle_uid(dict(h, file_at=0), 2, 90, holder_at=300), "take", "nothing stored: the fresh look is the evidence")
        self.assertEqual(cm.settle_uid(h, 2, 150), "keep")
        self.assertEqual(cm.settle_uid(h, 2, 100), "keep")
        self.assertEqual(cm.settle_uid(h, 2, 0), "keep")
        self.assertEqual(cm.settle_uid(dict(h, file_at=0), 2, 50), "keep")
        self.assertEqual(cm.settle_uid(dict(h, pinned=1), 2, 1), "keep")

    def test_listed_uids_are_owned_after_the_migration(self):
        """Entries listed before the uids table came in own their uids from the start."""
        u = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"
        tmp = tempfile.mkdtemp()
        path = os.path.join(tmp, "c.db")
        db = sqlite3.connect(path)
        for sql in cm.MIGRATIONS[:3]:
            for stmt in [x for x in sql.split(";") if x.strip()]:
                db.execute(stmt)
        db.execute("PRAGMA user_version=3")
        row = cm.new_row("Mod", 739703, 5)
        row.update(kind="mode", state="listed", submitter_id=3608549, submitter="RedSMURF",
                   item=json.dumps({"kind": "mode", "uid": u.upper(), "mode": {"id": "cage", "uid": u}}))
        db.execute(f"INSERT INTO entries ({','.join(row)}) VALUES ({','.join('?' * len(row))})", tuple(row.values()))
        db.commit()
        db.close()
        store = cm.Store(path)
        self.assertEqual((store.uid_owner(u)["owner_id"], store.uid_owner(u)["by"]), (3608549, "backfill"))

    def test_a_refused_submission_claims_nothing(self):
        u = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"
        files = FakeFiles()
        files.files[1886785] = zip_of(["modes/cage/cage.graph.json"], {"loner-strike.json": manifest(
            {"kind": "mode", "name": "Cage", "uid": u, "mode": {"id": "cage", "uid": u, "maps": [], "plugins": []}})})
        c, _ = make(files=files)
        block(c, MODE)
        cm.MAX_PENDING, keep = 0, cm.MAX_PENDING
        try:
            code, _ans = c.submit(body(mode_item()), "1.2.3.4")
        finally:
            cm.MAX_PENDING = keep
        self.assertEqual(code, 503, "refused after the uid check (too many waiting)")
        self.assertIsNone(c.store.uid_owner(u), "and it didn't keep the uid")

    def test_uid_case_doesnt_matter(self):
        u = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"
        self.assertEqual(cm.item_uids({"uid": " " + u.upper() + " ", "mode": {"uid": u}}), [u])
        self.assertEqual(cm.item_uids({"uid": "nope", "mode": {"uid": 5}}), [])
        c, _ = make()
        c.store.set_uid_owner(cm.norm_uid(u.upper()), 1, "x", "test")
        self.assertEqual(c.store.uid_owner(u)["owner_id"], 1)

    def test_sprays_are_listed_too_but_blocked_creators_wait(self):
        c, rec = make()
        self.assertEqual(c.submit(body(SPRAY_ITEM), "a")[1]["state"], "listed")
        c, rec = make()
        block(c, SPRAY)
        code, ans = c.submit(body(SPRAY_ITEM), "a")
        self.assertEqual((code, ans["state"]), (200, "pending"))
        self.assertIn("blocked creator", rec.sent[0][1]["content"])
        self.assertIn("lsmod.py approve spray:82585", rec.sent[0][1]["content"])
        self.assertEqual(json.loads(cm.build_index(c.store)[0])["items"], [])
        self.assertEqual(c.moderate("spray:82585", "approve", {})["state"], "listed")
        self.assertEqual(json.loads(cm.build_index(c.store)[0])["items"][0]["model"], "Spray")

    def test_the_app_retrying_is_harmless(self):
        gb, files = FakeGB(), FakeFiles()
        c, rec = make(gb, files=files)
        self.assertEqual(c.submit(body(map_item()), "1.2.3.4")[0], 200)
        asked = files.calls
        for _ in range(2):
            self.assertEqual(c.submit(body(map_item()), "1.2.3.4")[0], 200)
        self.assertEqual(files.calls, asked, "a retry reads the page (a newer file?) but not the same package again")
        self.assertEqual(len(rec.sent), 1, "nor tell the team again")

    def test_refusals(self):
        page = copy.deepcopy(MAP)
        page["_aFiles"][0].update(_sAvResult="", _sAvState="pending")
        rar = copy.deepcopy(MAP)
        rar["_aFiles"][0]["_sFile"] = "de_marakesh.rar"
        cases = [
            (FakeGB({("Mod", 739197): fixture("mod_739197_untagged.json")}), None, 422, "tag"),
            (FakeGB({}), None, 422, "isn't on GameBanana"),
            (FakeGB({("Mod", 739197): OSError("down")}), None, 502, "didn't answer"),
            (FakeGB({("Mod", 739197): page}), None, 422, "still checking"),
            (FakeGB({("Mod", 739197): rar}), None, 422, "zip"),
            (None, {1885247: zip_of(["maps/a.bsp", "addons/amxmodx/plugins/a.amxx"])}, 422, "Game Mode Studio"),
            (None, {1885247: zip_of(["maps/a.bsp", "addons/amxmodx/scripting/a.sma"])}, 422, "Game Mode Studio"),
            (None, {1885247: zip_of(["maps/a.bsp", "addons/metamod/dlls/x.dll"])}, 422, "run code"),
            (None, {1885247: zip_of(["maps/a.bsp", "more.zip"])}, 422, "can't be checked"),
            (None, {1885247: b"not a zip at all" * 10}, 422, "couldn't be read"),
            (None, {1885247: cm.FetchError("down")}, 502, "file server"),
        ]
        for gb, files, code, words in cases:
            with self.subTest(words):
                ff = FakeFiles()
                ff.files.update(files or {})
                c, rec = make(gb, files=ff)
                got, ans = c.submit(body(map_item()), "1.2.3.4")
                self.assertEqual(got, code, ans)
                self.assertIn(words, ans["error"])
                self.assertIsNone(c.store.get("mod:739197"), "nothing is kept from a refused registration")
                self.assertEqual(rec.sent, [])
        ff = FakeFiles()
        ff.files[1886785] = zip_of(["modes/cage/graph.json"], {"loner-strike.json": manifest(
            {"kind": "mode", "name": "Cage", "mode": {"id": "cage", "plugins": [{"file": "cage.amxx", "sha256": "ab" * 32, "source": "cage.sma"}]}})})
        c, _ = make(files=ff)
        code, ans = c.submit(body(mode_item()), "ip")
        self.assertEqual(code, 422)
        self.assertIn("Plugin modes can't be shared yet", ans["error"], "a plugin mode in the package itself")

    def test_rate_limits(self):
        c, _ = make()
        c.ip_limit = cm.Window(2, 3600)
        self.assertEqual(c.submit(b"junk", "9.9.9.9")[0], 400)
        self.assertEqual(c.submit(b"junk", "9.9.9.9")[0], 400)
        self.assertEqual(c.submit(body(map_item()), "9.9.9.9")[0], 429, "junk counts too")
        self.assertEqual(c.submit(body(map_item()), "8.8.8.8")[0], 200)
        c, _ = make()
        c.ip_day = cm.Window(1, 86400)
        self.assertEqual(c.submit(body(map_item()), "a")[0], 200)
        other = copy.deepcopy(MAP)
        other["_idRow"] = 5
        c.get.pages[("Mod", 5)] = other
        code, ans = c.submit(body(map_item(gb=5)), "a")
        self.assertEqual(code, 429, "the daily limit is per visitor")
        self.assertEqual(c.submit(body(map_item(gb=5)), "b")[0], 200, "the same GameBanana member's page from someone else")

    def test_queue_cap_for_blocked_creators(self):
        c, _ = make()
        block(c, SPRAY)
        old = cm.MAX_PENDING
        cm.MAX_PENDING = 0
        try:
            self.assertEqual(c.submit(body(SPRAY_ITEM), "a")[0], 503)
            self.assertEqual(c.submit(body(map_item()), "a")[0], 200, "maps don't wait, so they don't fill the queue")
        finally:
            cm.MAX_PENDING = old


# ---- moderation ---------------------------------------------------------------------------------

def new_file(page, fid=2000000, md5="1" * 32):
    page = copy.deepcopy(page)
    page["_aFiles"].append(dict(page["_aFiles"][0], _idRow=fid, _sMd5Checksum=md5))
    return page


class Moderation(unittest.TestCase):
    def test_approve_checks_the_page_again(self):
        c, _ = make()
        block(c, SPRAY)
        c.submit(body(SPRAY_ITEM), "a")
        untagged = copy.deepcopy(SPRAY)
        untagged["_aTags"] = []
        c.get.pages[("Spray", 82585)] = untagged
        with self.assertRaises(cm.Reject) as e:
            c.moderate("spray:82585", "approve", {})
        self.assertIn("tag", e.exception.message)
        c.get.pages[("Spray", 82585)] = OSError("down")
        with self.assertRaises(cm.Reject) as e:
            c.moderate("spray:82585", "approve", {})
        self.assertEqual(e.exception.code, 502)
        self.assertEqual(c.store.get("spray:82585")["state"], "pending")
        with self.assertRaises(cm.Reject):
            c.moderate("mod:5", "approve", {})

    def test_reject_then_resubmit(self):
        c, _ = make()
        block(c, SPRAY)
        c.submit(body(SPRAY_ITEM), "a")
        with self.assertRaises(cm.Reject):
            c.moderate("spray:82585", "reject", {})  # a reason is needed
        d = c.moderate("spray:82585", "reject", {"reason": "Not a spray."})
        self.assertEqual((d["state"], d["reason"]), ("rejected", "Not a spray."))
        code, ans = c.submit(body(SPRAY_ITEM), "b")
        self.assertEqual(code, 409)
        self.assertIn("Not a spray.", ans["error"])
        st = c.status("Spray", 82585)
        self.assertEqual((st["state"], st["reason"]), ("rejected", "Not a spray."))
        c.get.pages[("Spray", 82585)] = new_file(SPRAY)
        self.assertEqual(c.submit(body(dict(SPRAY_ITEM, file=2000000, md5="1" * 32)), "b"), (200, {"ok": True, "key": "spray:82585", "state": "pending"}))

    def test_updates(self):
        c, rec = make()
        c.submit(body(map_item()), "a")
        c.get.pages[("Mod", 739197)] = new_file(MAP)
        c.get_range.files[2000000] = c.get_range.files[1885247]
        code, ans = c.submit(body(map_item(file=2000000, md5="1" * 32)), "a")
        self.assertEqual((code, ans["state"]), (200, "listed"))
        self.assertEqual(json.loads(cm.build_index(c.store)[0])["items"][0]["md5"], "1" * 32, "a map's update replaces it at once")
        # a blocked creator's update waits; the listed one stays until then
        c.submit(body(SPRAY_ITEM), "a")
        block(c, SPRAY)
        c.get.pages[("Spray", 82585)] = new_file(SPRAY, 2000001)
        c.get_range.files[2000001] = sample("tempdecal.wad")
        self.assertEqual(c.submit(body(dict(SPRAY_ITEM, file=2000001, md5="1" * 32)), "a")[1]["state"], "pending",
                         "the app hears its update waits")
        self.assertEqual(c.store.get("spray:82585")["state"], "listed")
        self.assertIn("update", rec.sent[-1][1]["content"])
        spray = next(i for i in json.loads(cm.build_index(c.store)[0])["items"] if i["model"] == "Spray")
        self.assertEqual(spray["md5"], "26600d11dcffa32874195d9a32bcc010")
        self.assertEqual(c.queue("")["entries"][0]["key"], "spray:82585")

    def test_hidden_by_the_team_stays_hidden(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "hide", {"reason": "Asked by the author."})
        c.get.pages[("Mod", 739197)] = new_file(MAP)
        c.get_range.files[2000000] = c.get_range.files[1885247]
        self.assertEqual(c.submit(body(map_item(file=2000000, md5="1" * 32)), "a")[1]["state"], "pending", "the new file waits")
        self.assertEqual(c.store.get("mod:739197")["state"], "hidden")

    def test_pull_anything_and_restore(self):
        c, _ = make()
        d = c.moderate("https://gamebanana.com/mods/42", "pull", {"reason": "Malware reports."})
        self.assertEqual(d["state"], "pulled")
        self.assertEqual(c.submit(body(map_item(gb=42)), "a")[0], 409)
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "pull", {"reason": "Asked by the author."})
        idx = json.loads(cm.build_index(c.store)[0])
        self.assertEqual(idx["revoked"], ["mod:42", "mod:739197"])
        self.assertEqual(idx["items"][0]["status"], "revoked")
        self.assertEqual(c.moderate("mod:739197", "restore", {})["state"], "listed")
        with self.assertRaises(cm.Reject):
            c.moderate("mod:42", "restore", {})  # never listed: nothing to bring back
        with self.assertRaises(cm.Reject):
            c.moderate("mod:739197", "pull", {})  # a reason is needed

    def test_hide_and_feature(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "feature", {"featured": True})
        self.assertTrue(json.loads(cm.build_index(c.store)[0])["items"][0]["featured"])
        c.moderate("mod:739197", "hide", {"reason": "Waiting on a question."})
        self.assertEqual(json.loads(cm.build_index(c.store)[0])["items"][0]["status"], "hidden")
        with self.assertRaises(cm.Reject):
            c.moderate("mod:739197", "feature", {"featured": "yes"})
        for gone in ("explode", "review"):
            with self.assertRaises(cm.Reject):
                c.moderate("mod:739197", gone, {"review": "reviewed"})


# ---- reports ------------------------------------------------------------------------------------

def report(model="Mod", gb=739197, reason="broken", note="", **kw):
    d = {"schema": 1, "app": "1.0.0", "model": model, "id": gb, "reason": reason, "note": note}
    d.update(kw)
    return json.dumps(d).encode()


class Reports(unittest.TestCase):
    def test_a_report_reaches_the_mod_channel(self):
        c, rec = make()
        c.submit(body(map_item(name="de_marakesh")), "a")
        code, ans = c.report(report(note="@everyone it crashes <@123> **now** " + "x" * 600), "203.0.113.9")
        self.assertEqual((code, ans), (200, {"ok": True}))
        r = c.store.reports("mod:739197")[0]
        self.assertEqual(r["reason"], "broken")
        self.assertEqual(len(r["note"]), cm.MAX_NOTE)
        self.assertNotIn("203.0.113.9", json.dumps(r), "no address is kept")
        self.assertEqual(r["reporter"], c.reporter("203.0.113.9"))
        msg = rec.sent[-1][1]
        self.assertEqual(msg["allowed_mentions"], {"parse": []})
        text = msg["content"]
        for want in ("Report: broken", "de\\_marakesh", "https://gamebanana.com/mods/739197", f"#{r['reporter']}",
                     'lsmod.py pull mod:739197 --reason "..."'):
            self.assertIn(want, text)
        self.assertNotIn("@everyone", text)
        self.assertNotIn("<@123>", text)
        self.assertNotIn("203.0.113.9", text)

    def test_any_tagged_page_can_be_reported(self):
        c, rec = make()
        self.assertEqual(c.report(report(gb=55, reason="offensive"), "a")[0], 200)
        self.assertIn("not in our list", rec.sent[-1][1]["content"])
        self.assertEqual(c.report(report(model="Spray", gb=9, reason="stolen"), "a")[0], 200)

    def test_one_report_per_page_per_person_and_a_quiet_channel(self):
        c, rec = make()
        c.report(report(), "a")
        c.report(report(), "a")
        self.assertEqual(len(c.store.reports("mod:739197")), 1)
        c.report(report(), "b")
        self.assertEqual(len(c.store.reports("mod:739197")), 2)
        self.assertEqual(len(rec.sent), 1, "the channel hears about a page at most every half hour")

    def test_limits_and_junk(self):
        c, _ = make()
        c.report_ip = cm.Window(2, 3600)
        c.report(report(gb=1), "z")
        c.report(report(gb=2), "z")
        self.assertEqual(c.report(report(gb=3), "z")[0], 429)
        c, _ = make()
        for i, raw in enumerate((b"x", b"[]", report(reason="boring"), report(gb=0), report(gb="1"), report(model="Wip"),
                                 report(note=5), report(schema=2), b"{" + b" " * 5000 + b"}")):
            with self.subTest(raw=raw[:40]):
                self.assertIn(c.report(raw, f"y{i}")[0], (400, 413))
        self.assertEqual(c.store.reports(), [])


# ---- the index ----------------------------------------------------------------------------------

class Index(unittest.TestCase):
    def test_schema_exactly_as_the_app_reads_it(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        block(c, MODE)
        c.submit(body(mode_item()), "a")  # a blocked creator's: waiting, not in the index
        c.submit(body(SPRAY_ITEM), "a")
        data, sha = cm.build_index(c.store)
        self.assertEqual(sha, hashlib.sha256(data).hexdigest())
        idx = json.loads(data)
        self.assertEqual(set(idx), {"schema", "updated", "revoked", "verifiedAuthors", "items"})
        self.assertEqual(idx["schema"], 1)
        self.assertEqual([i["gb"] for i in sorted(idx["items"], key=lambda i: i["gb"])], [82585, 739197])
        m = next(i for i in idx["items"] if i["kind"] == "map")
        types = {"kind": str, "model": str, "gb": int, "file": int, "md5": str, "name": str, "author": str, "summary": str,
                 "games": list, "map": dict, "screenshots": list, "tags": list, "featured": bool,
                 "added": int, "updated": int, "status": str}
        for k, t in types.items():
            self.assertIsInstance(m[k], t, k)
        self.assertEqual(set(m["map"]), {"name", "bspSize", "bspSha256", "modes", "modeRefs", "nav", "places", "lsv", "res", "overview"})
        self.assertNotIn("health", m, "nothing proves the author's health numbers, so none are listed")
        self.assertEqual(m["model"], "Mod")
        self.assertEqual(m["author"], "TheHouthi")
        self.assertFalse(m["featured"], "featured is the team's pick, not the author's")
        s = next(i for i in idx["items"] if i["kind"] == "spray")
        self.assertEqual(s["model"], "Spray")
        self.assertNotIn("map", s)
        # only values the app's enums know (an unknown one would make it drop the whole list)
        for i in idx["items"]:
            self.assertIn(i["status"], ("listed", "hidden", "revoked"))
            self.assertIn(i["kind"], cm.KINDS)
            self.assertIn(i["model"], ("Mod", "Spray"))

    def test_same_database_same_bytes(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        a = cm.build_index(c.store)
        time.sleep(1.1)
        self.assertEqual(a, cm.build_index(c.store))

    def test_an_empty_index(self):
        c, _ = make()
        self.assertEqual(json.loads(cm.build_index(c.store)[0]), {"schema": 1, "updated": 0, "revoked": [], "verifiedAuthors": [], "items": []})

    def test_caps(self):
        c, _ = make()
        old = cm.MAX_INDEX
        c.submit(body(map_item()), "a")
        cm.MAX_INDEX = 100
        try:
            with self.assertRaises(cm.Reject):
                cm.build_index(c.store)
        finally:
            cm.MAX_INDEX = old


# ---- the re-check -------------------------------------------------------------------------------

class Recheck(unittest.TestCase):
    def listed(self):
        c, rec = make()
        c.submit(body(map_item()), "a")
        block(c, MODE)
        c.submit(body(mode_item()), "a")  # waiting for a moderator
        return c, rec

    def test_nothing_changed(self):
        c, rec = self.listed()
        n = len(rec.sent)
        r = c.recheck(pause=0)
        self.assertEqual((r["checked"], r["changed"]), (2, 0))
        self.assertEqual(len(rec.sent), n)
        self.assertGreater(c.store.get("mod:739197")["checked"], 0)

    def test_untagged_is_pulled(self):
        c, rec = self.listed()
        c.get.pages[("Mod", 739197)] = fixture("mod_739197_untagged.json")
        r = c.recheck(pause=0)
        self.assertEqual((r["changed"], r["heldBack"]), (1, False), "one change on its own goes through")
        self.assertEqual(c.store.get("mod:739197")["state"], "pulled")
        self.assertIn("tag", c.store.get("mod:739197")["reason"])
        idx = json.loads(cm.build_index(c.store)[0])
        self.assertEqual(idx["revoked"], ["mod:739197"])
        self.assertIn("Re-check", rec.sent[-1][1]["content"])

    def test_a_waiting_page_that_vanished_is_turned_down(self):
        c, _ = self.listed()
        del c.get.pages[("Mod", 739703)]
        c.recheck(pause=0)
        self.assertEqual(c.store.get("mod:739703")["state"], "rejected")

    def test_two_of_two_leaving_waits_for_a_person(self):
        c, rec = self.listed()
        c.get.pages[("Mod", 739197)] = fixture("mod_739197_untagged.json")
        del c.get.pages[("Mod", 739703)]
        r = c.recheck(pause=0)
        self.assertTrue(r["heldBack"])
        self.assertEqual(c.store.get("mod:739197")["state"], "listed")
        self.assertTrue(c.store.meta("last_recheck")["heldBack"])
        self.assertIn("held back", rec.sent[-1][1]["content"])

    def test_hides_count_toward_the_guard(self):
        pages = {}
        c, _ = make(FakeGB(pages))
        for i in range(1, 5):
            p = copy.deepcopy(MAP)
            p["_idRow"] = i
            pages[("Mod", i)] = p
            c.submit(body(map_item(gb=i)), f"ip{i}")
        for i in range(1, 5):  # every listed file replaced at once
            pages[("Mod", i)]["_aFiles"][0]["_sMd5Checksum"] = "9" * 32
        r = c.recheck(pause=0)
        self.assertTrue(r["heldBack"])
        self.assertEqual(c.store.count("listed"), 4)

    def test_replaced_file_hides(self):
        c, _ = self.listed()
        page = copy.deepcopy(MAP)
        page["_aFiles"][0]["_sMd5Checksum"] = "2" * 32
        c.get.pages[("Mod", 739197)] = page
        c.recheck(pause=0)
        self.assertEqual(c.store.get("mod:739197")["state"], "hidden")

    def test_flagged_file_pulls(self):
        c, _ = self.listed()
        page = copy.deepcopy(MAP)
        page["_aFiles"][0]["_sAvResult"] = "infected"
        c.get.pages[("Mod", 739197)] = page
        c.recheck(pause=0)
        self.assertEqual(c.store.get("mod:739197")["state"], "pulled")

    def test_network_trouble_changes_nothing(self):
        c, _ = self.listed()
        c.get.pages[("Mod", 739197)] = OSError("down")
        c.get.pages[("Mod", 739703)] = OSError("down")
        r = c.recheck(pause=0)
        self.assertEqual(r["changed"], 0)
        self.assertEqual(c.store.get("mod:739197")["state"], "listed")
        self.assertIn("couldn't check", c.store.get("mod:739197")["check_note"])

    def test_too_many_gone_at_once_is_held_back(self):
        pages = {}
        c, rec = make(FakeGB(pages))
        for i in range(1, 11):
            p = copy.deepcopy(MAP)
            p["_idRow"] = i
            pages[("Mod", i)] = p
            c.submit(body(map_item(gb=i)), f"ip{i}")
        pages.clear()  # GameBanana says "no such page" for everything
        r = c.recheck(pause=0)
        self.assertTrue(r["heldBack"])
        self.assertEqual(c.store.count("listed"), 10)
        self.assertIn("held back", rec.sent[-1][1]["content"])


# ---- Discord ------------------------------------------------------------------------------------

class Discord(unittest.TestCase):
    def test_only_discord_webhooks(self):
        self.assertTrue(cm.webhook_ok("https://discord.com/api/webhooks/1/abc"))
        for bad in ("http://discord.com/api/webhooks/1/a", "https://evil.com/api/webhooks/1/a", "https://discord.com/x",
                    "https://discord.com.evil.com/api/webhooks/1", "https://discord.com/api/webhooks/1/a?wait=1", ""):
            self.assertFalse(cm.webhook_ok(bad), bad)

    def test_page_text_cant_ping_or_format(self):
        t = cm.discord_text("@everyone **[click](https://x)** `x`")
        self.assertNotIn("@everyone", t)
        self.assertNotIn("**", t.replace("\\*", ""))

    def test_the_address_is_never_logged(self):
        url = "https://discord.com/api/webhooks/123/SECRETSECRET"
        err = io.StringIO()
        with redirect_stderr(err):
            n = cm.Notifier(url, post=lambda u, b: 500)
            n.say("hi", wait=True)
            cm.Notifier("https://evil.com/SECRETSECRET")
        self.assertNotIn("SECRET", err.getvalue())
        self.assertIn("status 500", err.getvalue())

    def test_off_without_a_webhook(self):
        sent = []
        cm.Notifier("", post=lambda u, b: sent.append(b)).say("x", wait=True)
        self.assertEqual(sent, [])


# ---- over HTTP, through the server list service ------------------------------------------------------

class Http(unittest.TestCase):
    def setUp(self):
        self.c, self.rec = make()
        reg = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        self.srv = ThreadingHTTPServer(("127.0.0.1", 0), sl.make_handler(reg, sl.RateLimit(3), lambda a: (None, None), community=self.c))
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.base = f"http://127.0.0.1:{self.srv.server_address[1]}"

    def tearDown(self):
        self.srv.shutdown()

    def call(self, method, path, data=None, token=None, headers=None):
        h = dict(headers or {})
        if token:
            h["Authorization"] = f"Bearer {token}"
        raw = data if isinstance(data, bytes) or data is None else json.dumps(data).encode()
        req = urllib.request.Request(self.base + path, data=raw, method=method, headers=h)
        try:
            with urllib.request.urlopen(req, timeout=5) as r:
                return r.status, r.read(), dict(r.headers)
        except urllib.error.HTTPError as e:
            return e.code, e.read(), dict(e.headers)

    def test_submit_status_and_moderate(self):
        code, out, hd = self.call("POST", "/community/submit", body(map_item()), headers={"Content-Type": "application/json"})
        self.assertEqual(code, 200, out)
        self.assertEqual(json.loads(out)["state"], "listed")
        code, out, hd = self.call("GET", "/community/submission/mod/739197")
        self.assertEqual((code, json.loads(out)["state"]), (200, "listed"))
        self.assertEqual(hd.get("Access-Control-Allow-Origin"), "*")
        self.assertEqual(self.call("GET", "/community/submission/wip/1")[0], 404)
        self.assertEqual(self.call("GET", "/community/health")[0], 200)
        # moderation
        self.assertEqual(self.call("GET", "/community/admin/queue")[0], 401)
        code, out, hd = self.call("GET", "/community/admin/queue", token=TOKEN)
        self.assertEqual(code, 200)
        self.assertNotIn("Access-Control-Allow-Origin", hd, "no browser may read moderation answers")
        self.assertEqual(json.loads(out)["entries"], [], "nothing waits: it was listed")
        code, out, _ = self.call("POST", "/community/admin/item/mod/739197", {"action": "hide", "reason": "x"}, token=TOKEN)
        self.assertEqual((code, json.loads(out)["state"]), (200, "hidden"), out)
        code, out, _ = self.call("POST", "/community/admin/item/mod/739197", {"action": "restore"}, token=TOKEN)
        self.assertEqual((code, json.loads(out)["state"]), (200, "listed"), out)
        code, data, hd = self.call("GET", "/community/admin/index", token=TOKEN)
        self.assertEqual(hd["X-Index-Sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(json.loads(data)["items"][0]["gb"], 739197)
        code, out, _ = self.call("GET", "/community/admin/status", token=TOKEN)
        st = json.loads(out)
        self.assertTrue(st["changedSincePublish"])
        self.call("POST", "/community/admin/published", {"sha256": hd["X-Index-Sha256"]}, token=TOKEN)
        self.assertFalse(json.loads(self.call("GET", "/community/admin/status", token=TOKEN)[1])["changedSincePublish"])
        # the server list still works beside it
        self.assertEqual(self.call("GET", "/servers")[0], 200)

    def test_bad_requests(self):
        self.assertEqual(self.call("POST", "/community/submit", b"x" * (cm.MAX_BODY + 10))[0], 413)
        self.assertEqual(self.call("POST", "/community/submit", b"{")[0], 400)
        self.assertEqual(self.call("GET", "/community/submit")[0], 405)
        self.assertEqual(self.call("GET", "/community/nothing")[0], 404)
        self.assertEqual(self.call("POST", "/community/admin/item/mod/1", b"[1]", token=TOKEN)[0], 400)

    def test_wrong_tokens_lock_out_that_visitor(self):
        for _ in range(cm.ADMIN_FAILS_PER_HOUR):
            self.assertEqual(self.call("GET", "/community/admin/queue", token="nope", headers={"CF-Connecting-IP": "5.5.5.5"})[0], 401)
        self.assertEqual(self.call("GET", "/community/admin/queue", token=TOKEN, headers={"CF-Connecting-IP": "5.5.5.5"})[0], 429)
        self.assertEqual(self.call("GET", "/community/admin/queue", token=TOKEN, headers={"CF-Connecting-IP": "6.6.6.6"})[0], 200)

    def test_rate_limit_by_cloudflare_visitor(self):
        self.c.ip_limit = cm.Window(1, 3600)
        self.assertEqual(self.call("POST", "/community/submit", b"{}", headers={"CF-Connecting-IP": "7.7.7.7"})[0], 400)
        self.assertEqual(self.call("POST", "/community/submit", b"{}", headers={"CF-Connecting-IP": "7.7.7.7"})[0], 429)
        self.assertEqual(self.call("POST", "/community/submit", b"{}", headers={"CF-Connecting-IP": "7.7.7.8"})[0], 400)

    def test_moderation_off_or_local_only(self):
        c = cm.Community(cm.Store(os.path.join(tempfile.mkdtemp(), "c.db")), token="short", get=FakeGB())
        self.srv.RequestHandlerClass = sl.make_handler(sl.Registry(os.path.join(tempfile.mkdtemp(), "l.json")), sl.RateLimit(3), community=c)
        self.assertEqual(self.call("GET", "/community/admin/queue", token="short")[0], 503)
        self.c.admin_local = True
        self.srv.RequestHandlerClass = sl.make_handler(sl.Registry(os.path.join(tempfile.mkdtemp(), "l.json")), sl.RateLimit(3), community=self.c)
        self.assertEqual(self.call("GET", "/community/admin/queue", token=TOKEN, headers={"CF-Connecting-IP": "1.1.1.1"})[0], 403)
        self.assertEqual(self.call("GET", "/community/admin/queue", token=TOKEN)[0], 200)

    def test_status_is_rate_limited(self):
        self.c.status_limit = cm.Window(2, 60)
        for _ in range(2):
            self.assertEqual(self.call("GET", "/community/submission/mod/1", headers={"CF-Connecting-IP": "9.9.9.9"})[0], 200)
        self.assertEqual(self.call("GET", "/community/submission/mod/1", headers={"CF-Connecting-IP": "9.9.9.9"})[0], 429)

    def test_a_chunked_body(self):
        import http.client
        conn = http.client.HTTPConnection("127.0.0.1", self.srv.server_address[1], timeout=5)
        raw = body(map_item())
        conn.request("POST", "/community/submit", body=iter([raw[:50], raw[50:]]), headers={"Content-Type": "application/json"},
                     encode_chunked=True)
        r = conn.getresponse()
        self.assertEqual((r.status, json.loads(r.read())["state"]), (200, "listed"))
        conn.close()

    def test_the_publish_export(self):
        pub = "p" * 40
        self.c.publish_digest = hashlib.sha256(pub.encode()).digest()
        self.c.admin_local = True  # the real admin API stays on the box; the export works through the tunnel
        cf = {"CF-Connecting-IP": "20.1.2.3"}
        self.call("POST", "/community/submit", body(map_item()))
        code, data, hd = self.call("GET", "/community/publish/index", token=pub, headers=cf)
        self.assertEqual(code, 200)
        self.assertEqual((data, hd["X-Index-Sha256"]), cm.build_index(self.c.store))
        self.assertEqual(hd["X-Recheck-Held-Back"], "0")
        self.assertNotIn("Access-Control-Allow-Origin", hd)
        code, txt, hd = self.call("GET", "/community/publish/index.sha256", token=pub, headers=cf)
        self.assertEqual(txt.decode(), f"{hashlib.sha256(data).hexdigest()}  index.json\n")
        self.c.store.set_meta("last_recheck", {"heldBack": True})
        self.assertEqual(self.call("GET", "/community/publish/index", token=pub, headers=cf)[2]["X-Recheck-Held-Back"], "1")
        # the publish token opens nothing else, and the admin token doesn't open the export
        self.assertEqual(self.call("GET", "/community/admin/index", token=pub)[0], 401)
        self.assertEqual(self.call("POST", "/community/admin/item/mod/739197", {"action": "pull", "reason": "x"}, token=pub)[0], 401)
        self.assertEqual(self.call("GET", "/community/publish/index", token=TOKEN, headers=cf)[0], 401)
        self.assertEqual(self.call("GET", "/community/publish/other", token=pub, headers=cf)[0], 404)

    def test_the_publish_export_locks_out_and_limits(self):
        pub = "p" * 40
        self.c.publish_digest = hashlib.sha256(pub.encode()).digest()
        for _ in range(cm.ADMIN_FAILS_PER_HOUR):
            self.assertEqual(self.call("GET", "/community/publish/index", token="nope", headers={"CF-Connecting-IP": "5.5.5.5"})[0], 401)
        self.assertEqual(self.call("GET", "/community/publish/index", token=pub, headers={"CF-Connecting-IP": "5.5.5.5"})[0], 429)
        self.c.publish_limit = cm.Window(1, 3600)
        self.assertEqual(self.call("GET", "/community/publish/index", token=pub, headers={"CF-Connecting-IP": "6.6.6.6"})[0], 200)
        self.assertEqual(self.call("GET", "/community/publish/index", token=pub, headers={"CF-Connecting-IP": "6.6.6.6"})[0], 429)
        self.c.publish_digest = None
        self.assertEqual(self.call("GET", "/community/publish/index", token=pub, headers={"CF-Connecting-IP": "7.7.7.7"})[0], 503)

    def test_releasing_a_hold(self):
        self.c.store.set_meta("last_recheck", {"heldBack": True, "at": 1})
        self.assertEqual(self.call("POST", "/community/admin/hold", {}, token=TOKEN)[0], 400)
        self.assertEqual(self.call("POST", "/community/admin/hold", {"held": False}, token=TOKEN)[0], 200)
        self.assertFalse(self.c.store.meta("last_recheck")["heldBack"])

    def test_without_community_nothing_changes(self):
        self.srv.RequestHandlerClass = sl.make_handler(sl.Registry(os.path.join(tempfile.mkdtemp(), "l.json")), sl.RateLimit(3))
        self.assertEqual(self.call("POST", "/community/submit", body(map_item()))[0], 404)

    def test_the_admin_tool(self):
        out = tempfile.mkdtemp()
        env = dict(os.environ, LS_COMMUNITY_TOKEN=TOKEN)
        tool = [sys.executable, "-I", os.path.join(HERE, "lsmod.py"), "--server", self.base]

        def run(*args):
            return subprocess.run(tool + list(args), capture_output=True, text=True, env=env, timeout=30)
        self.call("POST", "/community/submit", body(map_item()))
        r = run("queue", "--state", "listed")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("mod:739197", r.stdout)
        r = run("pull", "https://gamebanana.com/mods/739197", "--reason", "test")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("pulled", r.stdout)
        r = run("restore", "mod:739197")
        self.assertIn("listed", r.stdout)
        r = run("block", "mod:739197", "--reason", "spam")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("TheHouthi", run("blocked").stdout)
        r = run("unblock", "mod:739197")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.call("POST", "/community/report", report(note="broken bsp"))
        self.assertIn("broken bsp", run("reports", "mod:739197").stdout)
        r = run("reject", "mod:739197")
        self.assertNotEqual(r.returncode, 0, "a reason is required")
        r = run("publish", "--out", out)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("--upload", r.stdout, "without --upload nothing goes out")
        data = open(os.path.join(out, "index.json"), "rb").read()
        self.assertEqual(open(os.path.join(out, "index.json.sha256")).read().split()[0], hashlib.sha256(data).hexdigest())
        r = run("show", "mod:739197")
        self.assertIn("restore", r.stdout)
        self.assertNotIn(TOKEN, r.stdout + r.stderr)
        r = subprocess.run([sys.executable, "-I", os.path.join(HERE, "lsmod.py"), "--server", "http://list.lonerstrike.com", "status"],
                           capture_output=True, text=True, env=env, timeout=30)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("isn't https", r.stderr)
        env["LS_COMMUNITY_TOKEN"] = "wrong" * 10
        r = run("queue")
        self.assertNotEqual(r.returncode, 0)
        self.assertNotIn("wrong" * 10, r.stdout + r.stderr)


class Provenance(unittest.TestCase):
    """What's listed comes from GameBanana and the package, never from the request."""

    def test_a_strangers_made_up_metadata_is_ignored(self):
        c, _ = make()
        lies = map_item(name="FREE VBUCKS", summary="lies", author="admin", games=["czero"], tags=["scam"],
                        map={"name": "de_dust2", "bspSize": 5, "modes": ["zombie"], "nav": True, "lsv": True, "overview": True})
        self.assertEqual(c.submit(body(lies), "stranger")[1]["state"], "listed")
        it = json.loads(cm.build_index(c.store)[0])["items"][0]
        self.assertEqual((it["name"], it["author"], it["summary"], it["games"], it["tags"]),
                         ("de_marakesh", "TheHouthi", "A market town.", ["cstrike"], ["desert"]))
        self.assertEqual(it["md5"], "bb48b1faf89c4daad7545c4f3a2ce681", "the MD5 GameBanana lists")
        m = it["map"]
        self.assertEqual((m["name"], m["modes"], m["bspSize"]), ("de_marakesh", ["classic", "csdm"], len(tiny_bsp())))
        self.assertEqual((m["nav"], m["lsv"], m["places"], m["res"], m["overview"]), (True, True, True, False, False),
                         "only what ships in the zip counts")
        self.assertNotIn("health", it)

    def test_a_stranger_can_only_refresh_to_the_truth(self):
        c, _ = make()
        c.submit(body(map_item()), "creator")
        before = c.store.get("mod:739197")
        self.assertEqual(c.submit(body(map_item(name="EVIL")), "stranger")[0], 200)
        self.assertEqual(c.store.get("mod:739197")["item"], before["item"], "same file: nothing changes")
        c.get.pages[("Mod", 739197)] = new_file(MAP)
        c.get_range.files[2000000] = map_zip(["overviews/de_marakesh.bmp"])
        c.submit(body(map_item(file=2000000, name="EVIL", summary="EVIL")), "stranger")
        it = json.loads(c.store.get("mod:739197")["item"])
        self.assertEqual((it["name"], it["summary"], it["md5"], it["map"]["overview"]), ("de_marakesh", "A market town.", "1" * 32, True))

    def package_refusal(self, data, words):
        ff = FakeFiles()
        ff.files[1885247] = data
        c, _ = make(files=ff)
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn(words, ans["error"])

    def test_package_refusals(self):
        self.package_refusal(map_zip(man=dict(MAP_MANIFEST, kind="prefab")), "category")
        self.package_refusal(map_zip(man=dict(MAP_MANIFEST, map=dict(MAP_MANIFEST["map"], name="de_other"))), "isn't in it")
        self.package_refusal(map_zip(man=dict(MAP_MANIFEST, map=dict(MAP_MANIFEST["map"], name="../../x"))), "loner-strike.json")
        self.package_refusal(map_zip(man={k: v for k, v in MAP_MANIFEST.items() if k != "map"}), "loner-strike.json: map")
        self.package_refusal(zip_of(["maps/de_marakesh.bsp"]), "no loner-strike.json")
        self.package_refusal(zip_of(["maps/de_marakesh.bsp"], {"loner-strike.json": b"{nope"}), "couldn't be read")
        self.package_refusal(zip_of(["maps/de_marakesh.bsp"], {"loner-strike.json": json.dumps({"schema": 9, "item": {}}).encode()}), "newer")
        # a small zip entry that would unpack to far more than a manifest can be
        self.package_refusal(zip_of(["maps/de_marakesh.bsp"], {"loner-strike.json": manifest(MAP_MANIFEST) + b" " * 300_000}), "too big")
        b = io.BytesIO()
        with zipfile.ZipFile(b, "w") as z:
            z.writestr("maps/de_marakesh.bsp", b"x")
            z.writestr(zipfile.ZipInfo("loner-strike.json"), manifest(MAP_MANIFEST), compress_type=zipfile.ZIP_BZIP2)
        self.package_refusal(b.getvalue(), "odd way")

    def test_places_need_a_nav(self):
        ff = FakeFiles()
        ff.files[1885247] = zip_of(["maps/de_marakesh.bsp"], {"loner-strike.json": manifest(MAP_MANIFEST)})
        c, _ = make(files=ff)
        c.submit(body(map_item()), "a")
        m = json.loads(c.store.get("mod:739197")["item"])["map"]
        self.assertEqual((m["nav"], m["places"], m["lsv"]), (False, False, False))

    def test_sprays_and_textures_without_a_manifest(self):
        c, _ = make()
        c.submit(body(SPRAY_ITEM), "a")
        it = json.loads(c.store.get("spray:82585")["item"])
        self.assertEqual((it["name"], it["kind"]), (SPRAY["_sName"], "spray"))


class FileChoice(unittest.TestCase):
    """The server picks the file: the newest that passes, never older than what's listed."""

    def two_files(self, c, newer_zip=None, newer_av="clean"):
        page = new_file(MAP)  # 2000000 is newer (same date, bigger id)
        page["_aFiles"][1]["_sAvResult"] = newer_av
        c.get.pages[("Mod", 739197)] = page
        c.get_range.files[2000000] = newer_zip or map_zip(["overviews/de_marakesh.bmp"])

    def test_the_newest_file_is_listed_whatever_the_request_names(self):
        c, _ = make()
        self.two_files(c)
        c.submit(body(map_item(file=1885247)), "a")
        self.assertEqual(json.loads(c.store.get("mod:739197")["item"])["file"], 2000000)

    def test_a_stranger_cant_downgrade(self):
        c, _ = make()
        self.two_files(c)
        c.submit(body(map_item()), "creator")
        before = c.store.get("mod:739197")["item"]
        code, ans = c.submit(body(map_item(file=1885247)), "stranger")
        self.assertEqual((code, ans["state"]), (200, "listed"))
        self.assertEqual(c.store.get("mod:739197")["item"], before)

    def test_a_newer_file_updates_the_listing(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        self.assertEqual(json.loads(c.store.get("mod:739197")["item"])["file"], 1885247)
        self.two_files(c)
        c.submit(body(map_item()), "anyone")
        it = json.loads(c.store.get("mod:739197")["item"])
        self.assertEqual((it["file"], it["md5"]), (2000000, "1" * 32))

    def test_a_newer_file_that_fails_keeps_the_listed_one(self):
        for bad in ({"newer_av": "infected"}, {"newer_zip": map_zip(["addons/amxmodx/plugins/x.amxx"])}):
            c, _ = make()
            c.submit(body(map_item()), "a")
            self.two_files(c, **bad)
            code, ans = c.submit(body(map_item(file=2000000)), "a")
            self.assertEqual((code, ans["state"]), (200, "listed"), bad)
            self.assertIn("newer file", ans["note"])
            row = c.store.get("mod:739197")
            self.assertEqual(json.loads(row["item"])["file"], 1885247)
            self.assertIn("wasn't used", row["check_note"])

    def test_a_new_page_takes_the_newest_file_that_passes(self):
        c, _ = make()
        self.two_files(c, newer_av="infected")
        self.assertEqual(c.submit(body(map_item(file=2000000)), "a")[0], 200)
        self.assertEqual(json.loads(c.store.get("mod:739197")["item"])["file"], 1885247)

    def test_a_moderator_can_go_back(self):
        c, _ = make()
        self.two_files(c)
        c.submit(body(map_item()), "a")
        self.assertEqual(c.moderate("mod:739197", "usefile", {"file": 1885247})["listed"]["file"], 1885247,
                         "going back to an older file is the moderator's tool")
        with self.assertRaises(cm.Reject):
            c.moderate("mod:739197", "usefile", {"file": 999})


class Transitions(unittest.TestCase):
    """A creator's registration never undoes what a moderator did (pull, hide, block, reject)."""

    def fresh_file(self, c, page=MAP, key=("Mod", 739197), fid=2000000, md5="1" * 32):
        c.get.pages[key] = new_file(page, fid, md5)
        c.get_range.files[fid] = c.get_range.files.get(1885247)
        return map_item(file=fid, md5=md5)

    def test_a_team_hide_stays(self):
        c, rec = make()
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "hide", {"reason": "Asked by the author."})
        c.store.touch("mod:739197", 1, "")  # whatever the re-check wrote, it's the team's hide
        code, ans = c.submit(body(self.fresh_file(c)), "a")
        self.assertEqual((code, ans["state"]), (200, "pending"))
        row = c.store.get("mod:739197")
        self.assertEqual((row["state"], row["hidden_by"], row["reason"]), ("hidden", "team", "Asked by the author."))
        self.assertEqual(json.loads(row["proposed"])["md5"], "1" * 32, "the new file waits")
        self.assertIn("hidden by the team", rec.sent[-1][1]["content"])
        idx = json.loads(cm.build_index(c.store)[0])["items"][0]
        self.assertEqual((idx["status"], idx["md5"]), ("hidden", "bb48b1faf89c4daad7545c4f3a2ce681"))
        self.assertEqual(c.moderate("mod:739197", "approve", {})["state"], "listed", "a moderator lifts it")
        self.assertEqual(c.store.get("mod:739197")["hidden_by"], "")

    def test_a_recheck_hide_lifts_when_the_page_passes(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        c.get.pages[("Mod", 739197)] = new_file(MAP, 2000000, "1" * 32)
        c.get.pages[("Mod", 739197)]["_aFiles"].pop(0)  # the listed file is gone from the page
        c.recheck(pause=0)
        row = c.store.get("mod:739197")
        self.assertEqual((row["state"], row["hidden_by"]), ("withdrawn", "recheck"), "the listed file was removed: withdrawn")
        untagged = copy.deepcopy(c.get.pages[("Mod", 739197)])
        untagged["_aTags"] = []
        c.get.pages[("Mod", 739197)], ok = untagged, c.get.pages[("Mod", 739197)]
        c.get_range.files[2000000] = c.get_range.files[1885247]
        self.assertEqual(c.submit(body(map_item(file=2000000, md5="1" * 32)), "a")[0], 422, "only when it passes")
        self.assertEqual(c.store.get("mod:739197")["state"], "withdrawn")
        c.get.pages[("Mod", 739197)] = ok
        self.assertEqual(c.submit(body(map_item(file=2000000, md5="1" * 32)), "a")[1]["state"], "listed")
        row = c.store.get("mod:739197")
        self.assertEqual((row["hidden_by"], row["check_note"], row["reason"]), ("", "", ""))

    def test_pulls_stay_whoever_pulled(self):
        for who in ("team", "recheck"):
            c, _ = make()
            c.submit(body(map_item()), "a")
            if who == "team":
                c.moderate("mod:739197", "pull", {"reason": "Malware."})
            else:
                c.get.pages[("Mod", 739197)] = fixture("mod_739197_untagged.json")
                c.recheck(pause=0)
                c.get.pages[("Mod", 739197)] = MAP
            self.assertEqual(c.store.get("mod:739197")["hidden_by"], who)
            self.assertEqual(c.submit(body(self.fresh_file(c)), "a")[0], 409, who)
            self.assertEqual(c.store.get("mod:739197")["state"], "pulled")

    def test_a_team_rejection_sends_the_next_file_to_a_moderator(self):
        c, _ = make()
        block(c, MAP)
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "reject", {"reason": "Stolen."})
        c.unblock(MAP["_aSubmitter"]["_idRow"], "test")
        self.assertEqual(c.submit(body(map_item()), "a")[0], 409, "the same file")
        code, ans = c.submit(body(self.fresh_file(c)), "a")
        self.assertEqual((code, ans["state"]), (200, "pending"), "unblocked, but the team turned this page down")
        self.assertEqual(c.store.get("mod:739197")["reason"], "Stolen.")

    def test_a_recheck_rejection_doesnt_hold(self):
        c, _ = make()
        block(c, MAP)
        c.submit(body(map_item()), "a")
        del c.get.pages[("Mod", 739197)]
        c.recheck(pause=0)
        self.assertEqual(c.store.get("mod:739197")["hidden_by"], "recheck")
        c.unblock(MAP["_aSubmitter"]["_idRow"], "test")
        c.get.pages[("Mod", 739197)] = MAP
        self.assertEqual(c.submit(body(map_item()), "a")[1]["state"], "listed")

    def test_blocks_stay(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        block(c, MAP)
        self.assertEqual(c.submit(body(self.fresh_file(c)), "a")[1]["state"], "pending")
        self.assertTrue(c.store.is_blocked(MAP["_aSubmitter"]["_idRow"]))

    def test_a_team_hold_stays_the_teams_through_a_recheck(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "hide", {"reason": "x"})
        c.get.pages[("Mod", 739197)] = fixture("mod_739197_untagged.json")
        c.recheck(pause=0)
        row = c.store.get("mod:739197")
        self.assertEqual((row["state"], row["hidden_by"]), ("pulled", "team"))

    def test_featured_and_reason_survive_a_resubmission(self):
        c, _ = make()
        c.submit(body(map_item()), "a")
        c.moderate("mod:739197", "feature", {"featured": True})
        c.submit(body(self.fresh_file(c)), "a")
        self.assertTrue(c.store.get("mod:739197")["featured"])


def shift_entries(data, after_index, junk):
    """A zip with `junk` bytes slipped in before entry number `after_index` (directory offsets
    fixed up), as a hidden extra entry would be."""
    z = zipfile.ZipFile(io.BytesIO(data))
    infos = sorted(z.infolist(), key=lambda i: i.header_offset)
    cut_at = infos[after_index].header_offset
    eocd = data.rfind(b"PK\x05\x06")
    cd_off = struct.unpack("<I", data[eocd + 16:eocd + 20])[0]
    out = bytearray(data[:cut_at] + junk + data[cut_at:])
    n = len(junk)
    pos = cd_off + n
    while out[pos:pos + 4] == b"PK\x01\x02":
        off = struct.unpack("<I", out[pos + 42:pos + 46])[0]
        if off >= cut_at:
            out[pos + 42:pos + 46] = struct.pack("<I", off + n)
        nl, xl, cl = struct.unpack("<HHH", out[pos + 28:pos + 34])
        pos += 46 + nl + xl + cl
    e = eocd + n
    out[e + 16:e + 20] = struct.pack("<I", cd_off + n)
    return bytes(out)


class Hardening(unittest.TestCase):
    def test_visitors_are_counted_by_ipv4_and_ipv6_64(self):
        self.assertEqual(cm.visitor_key("203.0.113.9"), "203.0.113.9")
        self.assertEqual(cm.visitor_key("2001:db8:1:2:aaaa::1"), cm.visitor_key("2001:db8:1:2:ffff::9"))
        self.assertNotEqual(cm.visitor_key("2001:db8:1:2::1"), cm.visitor_key("2001:db8:1:3::1"))
        self.assertEqual(cm.visitor_key("::ffff:203.0.113.9"), "203.0.113.9")
        self.assertEqual(cm.visitor_key("junk"), "junk")
        c, _ = make()
        c.ip_limit = cm.Window(1, 3600)
        self.assertEqual(c.submit(b"x", "2001:db8:1:2::1")[0], 400)
        self.assertEqual(c.submit(b"x", "2001:db8:1:2::2")[0], 429, "another address in the same /64")

    def test_limit_tables_stay_small(self):
        w = cm.Window(1, 3600, max_keys=100)
        for i in range(1000):
            w.allow(f"k{i}", t=1000 + i)
        self.assertLessEqual(len(w.hits), 100)
        self.assertIn("k999", w.hits)
        self.assertNotIn("k0", w.hits, "the oldest go first")

    def test_fair_shares_of_the_global_caps(self):
        c, _ = make()
        c.gb_share = cm.Window(1, 60)
        c.submit(body(map_item()), "1.1.1.1")
        code, ans = c.submit(body(map_item(gb=5)), "1.1.1.1")
        self.assertEqual(code, 429, "one visitor's share of GameBanana checks")
        c.report_day = cm.Window(2, 86400)
        c.report(report(gb=1), "2.2.2.2")
        c.report(report(gb=2), "2.2.2.2")
        self.assertEqual(c.report(report(gb=3), "2.2.2.2")[0], 503)
        self.assertEqual(c.report(report(gb=3), "3.3.3.3")[0], 200, "others still can")

    def test_two_package_checks_at_a_time(self):
        c, _ = make()
        c.pkg_slots.acquire()
        c.pkg_slots.acquire()
        try:
            code, ans = c.submit(body(map_item()), "a")
            self.assertEqual(code, 429)
            self.assertIn("other packages", ans["error"])
        finally:
            c.pkg_slots.release()
            c.pkg_slots.release()
        self.assertEqual(c.submit(body(map_item()), "a")[0], 200)

    def package(self, data, name="x.zip"):
        return cm.read_package({"id": 5, "name": name, "size": len(data)}, lambda u, s, e: (data[s:e + 1], u))

    def test_entry_count_is_checked_before_anything_is_built(self):
        old = cm.MAX_ENTRIES
        cm.MAX_ENTRIES = 3
        try:
            with self.assertRaises(cm.Reject) as e:
                self.package(zip_of([f"sound/{i}.wav" for i in range(5)]))
            self.assertIn("too many files", e.exception.message)
        finally:
            cm.MAX_ENTRIES = old
        z = bytearray(zip_of(["maps/a.bsp"]))
        e = z.rfind(b"PK\x05\x06")
        z[e + 8:e + 12] = b"\xff\xff\xff\xff"  # zip64 markers
        with self.assertRaises(cm.Reject):
            self.package(bytes(z))

    def test_directory_and_entries_must_agree(self):
        good = map_zip()
        self.assertIn("maps/de_marakesh.bsp", self.package(good)[0])
        for bad in (b"junk" * 50 + good,                                   # data before the first entry
                    shift_entries(good, 2, b"\0" * 300),                  # room for a hidden entry
                    good.replace(b"maps/de_marakesh.nav", b"maps/de_marakesh.cfg", 1)):  # local header renamed
            with self.subTest(len(bad)):
                with self.assertRaises(cm.Reject):
                    self.package(bad)

    def test_damaged_manifest_data(self):
        z = bytearray(map_zip())
        i = zipfile.ZipFile(io.BytesIO(bytes(z))).getinfo("loner-strike.json")
        start = i.header_offset + 30 + len("loner-strike.json")
        for k in range(start + 2, start + 12):
            z[k] ^= 0xFF
        with self.assertRaises(cm.Reject) as e:
            self.package(bytes(z))
        self.assertEqual(e.exception.code, 422)

    def test_names_and_the_allow_list(self):
        for bad in ("maps\\x.bsp", "../x.bsp", "maps/./x.bsp", "maps/x.bsp.", "maps/x.bsp ", "maps/.hidden", "maps/x\x00.bsp",
                    "/maps/x.bsp", "maps/de_\u0430.bsp", "maps//x.bsp"):
            self.assertIsNone(cm.norm_path(bad), bad)
        self.assertEqual(cm.norm_path("Maps/De_Dust.BSP"), "maps/de_dust.bsp")
        allowed = {"map": ["maps/a.bsp", "maps/a.lsv", "sound/x/y.wav", "gfx/env/skyup.tga", "overviews/a.bmp", "mytex.wad", "README.txt"],
                   "prefab": ["prefabs/can.map", "prefabs/can.json", "prefabs/can.png", "props.wad"],
                   "spray": ["tempdecal.wad", "logos/x.bmp", "preview.png"],
                   "texture": ["mytex.wad"],
                   "mode": ["modes/zm/graph.json", "addons/amxmodx/configs/gamemodes/zm.ini", "sound/zm/a.wav"]}
        refused = {"map": ["server.cfg", "maps/a.cfg", "liblist.gam", "addons/amxmodx/configs/plugins.ini", "cstrike.wad", "Thumbs.db",
                           "maps/a.exe", "models/a.txt", "readme.md"],
                   "prefab": ["maps/a.bsp", "prefabs/x/y.map"],
                   "spray": ["sound/a.wav"],
                   "texture": ["maps/a.bsp", "sub/x.wad"],
                   "mode": ["modes/zm/plugins-zm.ini", "addons/amxmodx/configs/zm.cfg", "addons/amxmodx/plugins/zm.amxx", "modes/zm/x.sma"]}
        for kind, names in allowed.items():
            cm.check_package(names, kind)
        for kind, names in refused.items():
            for n in names:
                with self.subTest(kind=kind, name=n):
                    with self.assertRaises(cm.Reject):
                        cm.check_package(["README.txt", n], kind)
        with self.assertRaises(cm.Reject) as e:
            cm.check_package(["maps/A.bsp", "maps/a.bsp"], "map")
        self.assertIn("twice", e.exception.message)
        with self.assertRaises(cm.Reject) as e:
            cm.check_package(["maps/a.bsp", "liblist.gam"], "map")
        self.assertIn("settings", e.exception.message)


class MapSource(unittest.TestCase):
    """P5: a map package with its editable source (source/: the project, a .map, its own textures)."""

    def facts(self):
        return {"kind": "map", "file": {"id": 9, "md5": "ab" * 16}, "title": "Market", "author": "someone", "screenshots": []}

    def test_source_files_are_allowed_in_map_packages_only(self):
        cm.check_package(["maps/a.bsp", "source/a.lsmap", "source/a.map", "source/a_custom.wad", "source/README-source.txt", "README.txt"], "map")
        for bad, kind in (("source/a.exe", "map"), ("source/sub/a.lsmap", "map"), ("source/a.lsmap", "prefab"), ("source/a.dll", "map")):
            with self.subTest(bad=bad, kind=kind):
                with self.assertRaises(cm.Reject):
                    cm.check_package(["README.txt", bad], kind)

    def test_the_entry_says_source_only_when_the_project_ships(self):
        man = manifest(dict(MAP_MANIFEST, source=True))
        sizes = {"maps/de_marakesh.bsp": 10, "source/de_marakesh.lsmap": 5, "source/de_marakesh.map": 5}
        it = cm.build_item("Mod", 7, self.facts(), sizes, man)
        self.assertTrue(it.get("source"))
        it = cm.build_item("Mod", 7, self.facts(), {"maps/de_marakesh.bsp": 10}, man)
        self.assertNotIn("source", it, "no project in source/: not 'with source'")
        with self.assertRaises(cm.Reject):
            cm.package_item(manifest(dict(MAP_MANIFEST, source="yes")), "map")
        row = {"item": json.dumps(dict(cm.build_item("Mod", 7, self.facts(), sizes, man))), "state": "listed", "model": "Mod", "gb": 7,
               "submitter_id": 1, "featured": 0, "added": 1, "updated": 2}
        self.assertTrue(cm.index_item(row)["source"])


class FileChecks(unittest.TestCase):
    """Every file in a package is checked (formats.py) before it's listed."""

    def test_results_are_kept_with_the_entry(self):
        c, rec = make()
        c.submit(body(map_item()), "a")
        checks = json.loads(c.store.get("mod:739197")["item"])["checks"]
        self.assertEqual(checks["kinds"], {"map": 1, "nav": 1, "knowledge": 1, "text": 2})
        self.assertNotIn("checks", json.loads(cm.build_index(c.store)[0])["items"][0], "not part of the index")

    def test_a_bad_file_is_named(self):
        for name, data, words in (("maps/de_marakesh.bsp", struct.pack("<i", 31) + bytes(200), "version 31"),
                                  ("maps/de_marakesh.bsp", tiny_bsp()[:130], "past the end"),
                                  ("maps/de_marakesh.nav", b"\xce\xfa\xed\xfe" + struct.pack("<I", 9), "version 9")):
            ff = FakeFiles()
            ff.files[1885247] = zip_of([n for n in ("maps/de_marakesh.bsp", "maps/de_marakesh.nav") if n != name],
                                       {name: data, "loner-strike.json": manifest(MAP_MANIFEST)})
            c, rec = make(files=ff)
            code, ans = c.submit(body(map_item()), "a")
            self.assertEqual(code, 422, ans)
            self.assertIn(name, ans["error"])
            self.assertIn(words, ans["error"])
            self.assertIsNone(c.store.get("mod:739197"))

    def test_the_downloaded_file_must_be_the_listed_one(self):
        c, _ = make()
        c.verify_md5 = True
        code, ans = c.submit(body(map_item()), "a")  # the fixture page lists another MD5
        self.assertEqual(code, 422)
        self.assertIn("checksum", ans["error"])
        gb, files = FakeGB(), FakeFiles()
        gb.files = files
        page = copy.deepcopy(MAP)
        page["_aFiles"][0]["_sMd5Checksum"] = hashlib.md5(files.files[1885247]).hexdigest()
        gb.pages[("Mod", 739197)] = page
        c2 = cm.Community(cm.Store(os.path.join(tempfile.mkdtemp(), "c.db")), get=gb, get_range=files)
        self.assertEqual(c2.submit(body(map_item()), "a")[0], 200)

    def test_too_big_to_check(self):
        old = cm.CHECK_MAX
        cm.CHECK_MAX = 100
        try:
            c, _ = make()
            code, ans = c.submit(body(map_item()), "a")
            self.assertEqual(code, 422)
            self.assertIn("too big to check", ans["error"])
        finally:
            cm.CHECK_MAX = old

    def test_one_check_at_a_time(self):
        c, _ = make()
        c.check_slot.acquire()
        try:
            code, ans = c.submit(body(map_item()), "a")
            self.assertEqual(code, 429)
            self.assertIn("another package", ans["error"])
        finally:
            c.check_slot.release()

    def test_a_zip_entry_bigger_than_it_says(self):
        data = bytearray(map_zip())
        z = zipfile.ZipFile(io.BytesIO(bytes(data)))
        i = z.getinfo("README.txt")
        # make the directory claim a smaller unpacked size than the data really unpacks to
        cd = data.rfind(b"PK\x01\x02" + data[data.find(b"PK\x01\x02") + 4:data.find(b"PK\x01\x02") + 4])
        pos = data.find(b"PK\x01\x02")
        while pos >= 0:
            nlen = struct.unpack_from("<H", data, pos + 28)[0]
            if data[pos + 46:pos + 46 + nlen] == b"README.txt":
                struct.pack_into("<I", data, pos + 24, 10)
            pos = data.find(b"PK\x01\x02", pos + 4)
        ff = FakeFiles()
        ff.files[1885247] = bytes(data)
        c, _ = make(files=ff)
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)


class CheckerChild(unittest.TestCase):
    """Package checks run in a child process: killed and reaped at the limit, one at a time,
    without slowing the service."""

    def setUp(self):
        self.old = cm.CHECK_SECONDS
        cm.CHECK_SECONDS = 2

    def tearDown(self):
        cm.CHECK_SECONDS = self.old

    def hung(self, c, code="import time; time.sleep(60)"):
        pidfile = os.path.join(tempfile.mkdtemp(), "pid")
        c.checker_cmd = [sys.executable, "-c", f"import os; open({pidfile!r}, 'w').write(str(os.getpid())); " + code]
        return pidfile

    def test_a_hung_check_is_killed_and_reaped_before_the_slot_is_free(self):
        c, _ = make()
        pidfile = self.hung(c)
        t = time.monotonic()
        code, ans = c.submit(body(map_item()), "a")
        took = time.monotonic() - t
        self.assertEqual(code, 422)
        self.assertIn("too long", ans["error"])
        self.assertLess(took, 8)
        pid = int(open(pidfile).read())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)  # gone, and reaped (a zombie would still answer)
        self.assertTrue(c.check_slot.acquire(blocking=False), "the slot is free once the child has exited")
        c.check_slot.release()

    def test_the_slot_is_held_while_the_child_runs(self):
        c, _ = make()
        self.hung(c)
        first = threading.Thread(target=c.submit, args=(body(map_item()), "a"))
        first.start()
        time.sleep(0.8)
        self.assertIsNotNone(c.checking, "the child is running")
        code, ans = c.submit(body(map_item(gb=739703)), "b")
        self.assertEqual(code, 429)
        first.join(10)
        self.assertIsNone(c.checking)

    def test_the_service_keeps_answering_during_a_check(self):
        c, _ = make()
        self.hung(c, "while True: pass")  # a child burning CPU
        reg = sl.Registry(os.path.join(tempfile.mkdtemp(), "list.json"))
        srv = ThreadingHTTPServer(("127.0.0.1", 0), sl.make_handler(reg, sl.RateLimit(3), lambda a: (None, None), community=c))
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        try:
            base = f"http://127.0.0.1:{srv.server_address[1]}"
            check = threading.Thread(target=c.submit, args=(body(map_item()), "x"))
            check.start()
            time.sleep(0.5)
            worst = 0.0
            for _ in range(10):
                t = time.monotonic()
                with urllib.request.urlopen(base + "/servers", timeout=5) as r:
                    r.read()
                worst = max(worst, time.monotonic() - t)
                time.sleep(0.05)
            check.join(10)
            self.assertLess(worst, 0.3, f"/servers took {worst:.3f} s during a check")
        finally:
            srv.shutdown()

    def test_a_hostile_slow_file_hits_the_deadline(self):
        # a 4096x4096 RLE picture made of one-pixel packets: millions of steps to walk
        tga = struct.pack("<BBBHHBHHHHBB", 0, 0, 11, 0, 0, 0, 0, 0, 4096, 4096, 8, 0) + b"\x00\x00" * (4096 * 4096)
        cm.CHECK_SECONDS = 1
        ff = FakeFiles()
        ff.files[1885247] = zip_of(["maps/de_marakesh.bsp"], {"gfx/env/skyup.tga": tga, "loner-strike.json": manifest(MAP_MANIFEST)})
        c, _ = make(files=ff)
        t = time.monotonic()
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("too long", ans["error"])
        self.assertLess(time.monotonic() - t, 8)

    def test_the_deadline_inside_the_checks(self):
        import formats
        formats.set_deadline(time.monotonic() - 1)
        try:
            with self.assertRaises(formats.FormatError) as e:
                formats.check("maps/a.bsp", io.BytesIO(tiny_bsp()), len(tiny_bsp()))
            self.assertIn("too long", str(e.exception))
        finally:
            formats.set_deadline(None)


class Migrations(unittest.TestCase):
    def test_reopening_keeps_everything(self):
        p = os.path.join(tempfile.mkdtemp(), "c.db")
        gb, files = FakeGB(), FakeFiles()
        gb.files = files
        c = cm.Community(cm.Store(p), get=gb, get_range=files)
        c.verify_md5 = False
        c.submit(body(map_item()), "a")
        s = cm.Store(p)
        self.assertEqual(s.db.execute("PRAGMA user_version").fetchone()[0], len(cm.MIGRATIONS))
        self.assertEqual(s.get("mod:739197")["state"], "listed")

    def test_version_1_rows_count_as_the_teams(self):
        p = os.path.join(tempfile.mkdtemp(), "c.db")
        old = cm.MIGRATIONS
        cm.MIGRATIONS = old[:1]
        try:
            s1 = cm.Store(p)
            for key, state in (("mod:1", "hidden"), ("mod:2", "pulled"), ("mod:3", "rejected"), ("mod:4", "listed")):
                s1.db.execute("INSERT INTO entries (key, model, gb, state, added, updated) VALUES (?,?,?,?,1,1)", (key, "Mod", int(key[4:]), state))
            s1.db.close()
        finally:
            cm.MIGRATIONS = old
        s2 = cm.Store(p)
        self.assertEqual(s2.db.execute("PRAGMA user_version").fetchone()[0], len(cm.MIGRATIONS))
        self.assertEqual([s2.get(f"mod:{i}")["hidden_by"] for i in range(1, 5)], ["team", "team", "team", ""])


if __name__ == "__main__":
    unittest.main()


class Withdrawn(unittest.TestCase):
    """What kind of "gone" decides: GameBanana's moderation (withheld) pulls; a private page or a
    removed file is withdrawn and comes back by itself; a deleted page (author or moderator, the API
    can't tell) is withdrawn for review and only a moderator lists it again."""

    def setUp(self):
        self.c, self.rec = make()
        self.c.submit(body(map_item()), "a")
        self.page = self.c.get.pages[("Mod", 739197)]

    def set_page(self, **flags):
        p = copy.deepcopy(self.page)
        p.update(flags)
        self.c.get.pages[("Mod", 739197)] = p

    def state(self):
        r = self.c.store.get("mod:739197")
        return r["state"], r["hidden_by"]

    def test_withheld_by_gamebanana_is_pulled(self):
        self.set_page(_bIsWithheld=True)
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("pulled", "recheck"))
        self.assertEqual(json.loads(cm.build_index(self.c.store)[0])["revoked"], ["mod:739197"])
        # back: still pulled (only a moderator restores)
        self.c.get.pages[("Mod", 739197)] = self.page
        self.c.recheck(pause=0)
        self.assertEqual(self.state()[0], "pulled")

    def test_a_private_page_comes_back_by_itself(self):
        self.set_page(_bIsPrivate=True)
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("withdrawn", "recheck"))
        idx = json.loads(cm.build_index(self.c.store)[0])
        self.assertEqual((idx["items"][0]["status"], idx["revoked"]), ("withdrawn", []))
        self.c.recheck(pause=0)
        self.assertEqual(self.state()[0], "withdrawn", "still private: no change")
        self.c.get.pages[("Mod", 739197)] = self.page
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("listed", ""))

    def test_a_deleted_page_waits_for_a_moderator(self):
        self.set_page(_bIsTrashed=True)
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("withdrawn", "review"))
        self.c.get.pages[("Mod", 739197)] = self.page
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("withdrawn", "review"), "never back by itself")
        self.assertIn("Back on GameBanana", self.rec.sent[-1][1]["content"])
        # the creator registering it again doesn't list it either
        self.assertNotEqual(self.c.submit(body(map_item()), "a")[1]["state"], "listed")
        self.assertEqual(self.state()[0], "withdrawn")
        self.c.moderate("mod:739197", "restore", {"reason": "Looked: fine."})
        self.assertEqual(self.state(), ("listed", ""))

    def test_no_such_page_is_a_deletion_too(self):
        del self.c.get.pages[("Mod", 739197)]
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("withdrawn", "review"))

    def test_a_removed_file_comes_back_by_itself(self):
        p = copy.deepcopy(self.page)
        p["_aFiles"] = []
        self.c.get.pages[("Mod", 739197)] = p
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("withdrawn", "recheck"))
        self.c.get.pages[("Mod", 739197)] = self.page
        self.c.recheck(pause=0)
        self.assertEqual(self.state(), ("listed", ""))

    def test_a_team_hold_never_comes_back_by_itself(self):
        self.set_page(_bIsPrivate=True)
        self.c.recheck(pause=0)
        row = self.c.store.get("mod:739197")
        row["hidden_by"] = "team"
        self.c.store.put(row, "test", "")
        self.c.get.pages[("Mod", 739197)] = self.page
        self.c.recheck(pause=0)
        self.assertEqual(self.state()[0], "withdrawn")


class Unlist(unittest.TestCase):
    """The author unlists (and lists again) with the page's proof: a code on the GameBanana page."""

    def setUp(self):
        self.c, _ = make()
        self.c.submit(body(map_item()), "a")
        self.page = self.c.get.pages[("Mod", 739197)]

    def ask(self, action, ip="u"):
        return self.c.unlist(json.dumps({"key": "mod:739197", "action": action}).encode(), ip)

    def put_on_page(self, code, submitter=None):
        p = copy.deepcopy(self.page)
        p["_sText"] = (p.get("_sText") or "") + f" {code} "
        if submitter is not None:
            p["_aSubmitter"]["_idRow"] = submitter
        self.c.get.pages[("Mod", 739197)] = p

    def test_unlist_then_list_again_with_the_code(self):
        code, a = self.ask("unlist")
        self.assertEqual((code, a["state"]), (200, "needs-proof"))
        self.assertIn(a["code"], a["how"])
        # not on the page yet: nothing changes
        code, b = self.ask("unlist")
        self.assertEqual((b["state"], b["code"]), ("needs-proof", a["code"]))
        self.assertEqual(self.c.store.get("mod:739197")["state"], "listed")
        self.put_on_page(a["code"])
        code, d = self.ask("unlist")
        self.assertEqual((code, d["state"]), (200, "unlisted"))
        row = self.c.store.get("mod:739197")
        self.assertEqual((row["state"], row["hidden_by"]), ("unlisted", "creator"))
        self.assertEqual(json.loads(cm.build_index(self.c.store)[0])["items"][0]["status"], "unlisted")
        # an update keeps it unlisted
        self.assertEqual(self.c.submit(body(map_item()), "a")[1]["state"], "unlisted")
        # list again: a new code
        _, e = self.ask("list")
        self.assertNotEqual(e["code"], a["code"])
        self.put_on_page(e["code"])
        self.assertEqual(self.ask("list")[1]["state"], "listed")

    def test_another_member_cant(self):
        _, a = self.ask("unlist")
        self.put_on_page(a["code"], submitter=424242)
        code, b = self.ask("unlist")
        self.assertEqual(code, 403)
        self.assertEqual(self.c.store.get("mod:739197")["state"], "listed")

    def test_a_team_hold_stays(self):
        self.c.moderate("mod:739197", "hide", {"reason": "Checking."})
        code, a = self.ask("unlist")
        self.assertEqual(code, 409)
        self.assertIn("team", a["error"])

    def test_bad_requests(self):
        self.assertEqual(self.c.unlist(b"{nope", "x")[0], 400)
        self.assertEqual(self.c.unlist(json.dumps({"key": "mod:1", "action": "unlist"}).encode(), "x")[0], 404)
        self.assertEqual(self.c.unlist(json.dumps({"key": "mod:739197", "action": "delete"}).encode(), "x")[0], 400)
        self.assertEqual(self.ask("list")[0], 409, "it's listed already")

    def test_codes_expire(self):
        _, a = self.ask("unlist")
        later = cm.now() + cm.UNLIST_CODE_TTL + 5
        _, b = self.c.unlist(json.dumps({"key": "mod:739197", "action": "unlist"}).encode(), "u", now_=later)
        self.assertNotEqual(a["code"], b["code"])

    def unlisted(self):
        _, a = self.ask("unlist")
        self.put_on_page(a["code"])
        self.assertEqual(self.ask("unlist")[1]["state"], "unlisted")

    def test_unlisted_entries_are_still_rechecked(self):
        for change in ("untagged", "flagged", "withheld"):
            with self.subTest(change):
                self.setUp()
                self.unlisted()
                p = copy.deepcopy(self.page)
                if change == "untagged":
                    p["_aTags"] = []
                elif change == "flagged":
                    p["_aFiles"][0]["_sAvResult"] = "infected"
                else:
                    p["_bIsWithheld"] = True
                self.c.get.pages[("Mod", 739197)] = p
                self.c.recheck(pause=0)
                self.assertEqual(self.c.store.get("mod:739197")["state"], "pulled", change)
                self.assertEqual(json.loads(cm.build_index(self.c.store)[0])["revoked"], ["mod:739197"])

    def test_unlisted_and_deleted_stays_unlisted(self):
        self.unlisted()
        del self.c.get.pages[("Mod", 739197)]
        self.c.recheck(pause=0)
        self.assertEqual(self.c.store.get("mod:739197")["state"], "unlisted")

    def test_list_again_checks_the_page_first(self):
        self.unlisted()
        _, a = self.ask("list")
        p = copy.deepcopy(self.page)
        p["_aTags"] = []
        p["_sText"] = (p.get("_sText") or "") + f" {a['code']} "
        self.c.get.pages[("Mod", 739197)] = p
        code, b = self.ask("list")
        self.assertEqual(code, 409)
        self.assertIn("can't be listed again", b["error"])
        self.assertEqual(self.c.store.get("mod:739197")["state"], "pulled", "anything that pulls it pulls it")

    def test_list_again_needs_the_listed_file(self):
        self.unlisted()
        _, a = self.ask("list")
        p = copy.deepcopy(self.page)
        p["_aFiles"] = []
        p["_sText"] = (p.get("_sText") or "") + f" {a['code']} "
        self.c.get.pages[("Mod", 739197)] = p
        code, b = self.ask("list")
        self.assertEqual(code, 409)
        self.assertEqual(self.c.store.get("mod:739197")["state"], "unlisted")
