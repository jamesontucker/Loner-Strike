#!/usr/bin/env python3
# Custom creatures in community packages (maps, modes, prefabs, and the creature kind), offline:
#   cd services/serverlist && python3 -m unittest test_creatures
# The packages go through the real checks (the checker child, formats.py) as in test_community.
import copy
import json
import os
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import community as cm  # noqa: E402
import formats  # noqa: E402
from test_community import MAP_MANIFEST, PREFAB, FakeFiles, FakeGB, body, make, manifest, map_item, zip_of  # noqa: E402
from test_formats import model  # noqa: E402

GOOD_INI = b"""; >>> from mod:1234 (made in Loner-Strike)
; types: gb1001.runner_x gb1001.boss_y
; files: models/gb1001/runner_x.mdl sound/gb1001/hit.wav ../not/checked:here
[gb1001.runner_x]
base = runner
name = Fast One / v2
hp = 80
model = models/gb1001/runner_x.mdl models/zombie.mdl
snd_attack = gb1001/hit.wav gb1001/hit2.wav | player/pl_pain2.wav
[gb1001.boss_y]
base = gb1001.runner_x
; <<< end
"""
CREATURES = [{"name": "gb1001.runner_x", "model": True, "sounds": 2}, {"name": "gb1001.boss_y"}]
FILES = {"creatures/creatures.ini": GOOD_INI, "models/gb1001/runner_x.mdl": model()}
# (the app keeps packed files under the author's own folders)
FILES_NS = {"models/gb1001/qa/runner_x.mdl": model()}
ASSETS = ["sound/gb1001/hit.wav", "sound/gb1001/hit2.wav"]


MEMBERS = {"Ann": 1001, "Bob": 1002, "ann!": 7771234}


def creature_page(gb=706630, file_id=1789869, author="Ann"):
    """A tagged page among the game's skins (the prefab fixture, moved, renumbered, by Ann)."""
    p = copy.deepcopy(PREFAB)
    p["_aSubmitter"]["_sName"] = author
    p["_aSubmitter"]["_idRow"] = MEMBERS.get(author, 5795251)
    p["_idRow"] = gb
    p["_aFiles"] = p["_aFiles"][:1]
    p["_aFiles"][0]["_idRow"] = file_id
    p["_aCategory"] = {"_idRow": 999001, "_sName": "Monsters", "_sModelName": "ModCategory"}
    p["_aSuperCategory"] = {"_idRow": 999000, "_sName": "Skins", "_sModelName": "ModCategory"}
    return p


def creature_zip(ini=GOOD_INI, man=None, extra=(), folder="gb1001"):
    """Ann's creature package (or with its files in another member's folder)."""
    files = {k.replace("gb1001/", folder + "/"): v for k, v in FILES.items()}
    files.update({"creatures/creatures.ini": ini, "creatures/preview.png": b"\x89PNG\r\n\x1a\n" + bytes(32)})
    return zip_of([a.replace("gb1001/", folder + "/") for a in ASSETS] + list(extra),
                  dict(files, **{"loner-strike.json": manifest(man or {"kind": "creature", "name": "Fast One", "creatures": CREATURES})}))


def creature_item(**kw):
    it = {"kind": "creature", "model": "Mod", "gb": 706630, "file": 1789869, "md5": "0" * 32, "name": "Fast One", "creatures": CREATURES}
    it.update(kw)
    return it


class Rules(unittest.TestCase):
    def test_names(self):
        for ok in ("gb1001.runner_x", "gb1.b", "gb123456789." + "y" * 19):
            self.assertTrue(cm.creature_name_ok(ok), ok)
        for bad in ("runner", "zombie", "ann.runner", "Gb1001.runner", "gb.x", "gb0.x", "gb01.x", "gb1234567890.x", "gb1001.", ".x",
                    "gb1001.123", "gb1001.a-b", "../x.y", "gb1.b.c", "gb123456789." + "y" * 20, None, 5):
            self.assertFalse(cm.creature_name_ok(bad), bad)

    def test_ini(self):
        self.assertIsNone(cm.creatures_ini_problem(GOOD_INI))
        # a base defined later in the same file, and a built-in base in any case
        self.assertIsNone(cm.creatures_ini_problem(b"[gb1001.a]\nbase = gb1001.b\n[gb1001.b]\nbase = Zombie\n"))
        self.assertEqual(cm.creatures_ini_types(GOOD_INI), ["gb1001.runner_x", "gb1001.boss_y"])
        for bad, words in ((b"[zombie]\nhp = 1\n", "built-in"), (b"[Runner]\n", "built-in"), (b"[runner_x]\n", "gb<member id>.<name>"),
                           (b"[gb1001.x]\nmodel = ../../evil.mdl\n", "outside"), (b"[gb1001.x]\nmodel = C:/x.mdl\n", "outside"),
                           (b"[gb1001.x]\nmodel = models\\\\x.mdl\n", "outside"), (b"[gb1001.x]\nmodel = maps/x.bsp\n", "models/....mdl"),
                           (b"[gb1001.x]\nmodel = models/a.mdl | sound/x.wav\n", "models/....mdl"),
                           (b"[gb1001.x]\nsnd_die = x.exe\n", ".wav path"), (b"[gb1001.x]\nsnd_die = a/1.wav | /etc/x.wav\n", "outside"),
                           (b"[gb1001.x]\nsnd_pain = a/../../x.wav\n", "outside"), (b"[gb1001.x]\nbase = gb1001.nope\n", "isn't a built-in"),
                           (b"[gb1001.x]\nbase = gb1002.y\n[gb1001.y]\n", "isn't a built-in"), (b"; nothing\n", "no creature"),
                           (b"[gb1001.x]\n" + b"a" * 600 + b"\n", "longer"), (b"[gb1001.x]\0", "text"), (b"[gb1001.x]\n" * 1 + b" " * (65 << 10), "too big"),
                           (b"".join(b"[gb1001.t%d]\n" % i for i in range(33)), "33 types")):
            self.assertIn(words, cm.creatures_ini_problem(bad) or "", bad[:40])

    def test_every_key_that_names_a_file(self):
        hostile = {
            b"model = models/ok.mdl /etc/passwd": "outside", b"model = models/a.mdl models/../../x.mdl": "outside",
            b"model = \\\\server\\share\\x.mdl": "outside", b"model = C:/x.mdl": "outside", b"model = /models/x.mdl": "outside",
            b"snd_pain = a/1.wav maps/x.bsp": ".wav path", b"snd_pain = /abs/x.wav": "outside", b"snd_idle = ok.wav x.exe": ".wav path",
            b"snd_alert = ../x.wav": "outside", b"snd_hit = d:x.wav": "outside", b"snd_miss = a.wav|/x.wav": "outside",
            b"snd_die = a.wav | \\\\h\\x.wav": "outside",
            b"ls = model=models/a.mdl anims=walk run attack die": "shared creatures don't take", b"ls = model=../x.mdl": "outside",
            b"models = models/a.mdl": "shared creatures don't take", b"sounds = a/1.wav": "shared creatures don't take", b"say_idle = sound/x.wav": "sentence", b"say_pain = HG_PAIN": None,
            b"walk = models/x.mdl": "names a file where", b"glow = /x": "outside", b"whatever = a/b": "names a file where",
            b"name = Fast / One": None, b"snd_idle = ann/a.wav ann/b.wav": None, b"display = X": "shared creatures don't take",
            b"health = 5": "shared creatures don't take", b"detect = 5": "shared creatures don't take", b"fov = 5": "shared creatures don't take",
            b"behavior = chase": "shared creatures don't take", b"guard_radius = 5": "shared creatures don't take", b"behaviour = chase": None,
        }
        for line, words in hostile.items():
            with self.subTest(line=line):
                why = cm.creatures_ini_problem(b"[gb1001.x]\n" + line + b"\n")
                if words is None:
                    self.assertIsNone(why)
                else:
                    self.assertIn(words, why or "")

    def test_namespaces(self):
        self.assertTrue(cm.namespace_is("gb1001.x", 1001))
        for name, member in (("gb1001.x", 100), ("gb100.x", 1001), ("gb10010.x", 1001), ("x.gb1001", 1001)):
            self.assertFalse(cm.namespace_is(name, member), name)
        sec = cm.creatures_ini_sections(GOOD_INI)
        self.assertEqual(sorted(sec), ["gb1001.boss_y", "gb1001.runner_x"])
        # comments and spacing don't change a section; a value does
        same = GOOD_INI.replace(b"hp = 80", b"hp = 80   ; tough").replace(b"; <<< end", b"")
        self.assertEqual(cm.creatures_ini_sections(same), sec)
        self.assertNotEqual(cm.creatures_ini_sections(GOOD_INI.replace(b"hp = 80", b"hp = 81"))["gb1001.runner_x"], sec["gb1001.runner_x"])

    def test_the_format_check_reads_it(self):
        self.assertEqual(formats.check("creatures/creatures.ini", __import__("io").BytesIO(GOOD_INI), len(GOOD_INI)), "creatures")
        with self.assertRaises(formats.FormatError):
            formats.check("creatures/creatures.ini", __import__("io").BytesIO(b"[zombie]\n"), 9)
        # another .ini is plain text
        self.assertEqual(formats.check("addons/amxmodx/configs/gamemodes/zombie.ini", __import__("io").BytesIO(b"[zombie]\n"), 9), "text")

    def test_where_creature_files_may_be(self):
        cm.check_package(["maps/a.bsp", "creatures/creatures.ini", "models/gb1001/x.mdl", "sound/gb1001/x.wav"], "map")
        cm.check_package(["creatures/creatures.ini", "models/gb1001/qa/x.mdl", "sound/gb1001/qa/a/1.wav"], "creature")
        cm.check_package(["modes/cage/graph.json", "creatures/creatures.ini", "models/gb1001/x.mdl"], "mode")
        cm.check_package(["prefabs/can.map", "prefabs/can.json", "creatures/creatures.ini", "models/gb1001/x.mdl", "sound/gb1001/x.wav"], "prefab")
        cm.check_package(["creatures/creatures.ini", "creatures/preview.png", "models/gb1001/x.mdl", "sound/gb1001/x.wav", "README.txt", "loner-strike.json"], "creature")
        for kind, bad in (("spray", "creatures/creatures.ini"), ("texture", "creatures/creatures.ini"), ("creature", "maps/a.bsp"),
                          ("creature", "creatures/other.ini"), ("creature", "addons/amxmodx/plugins/x.amxx"), ("creature", "../models/x.mdl"),
                          ("creature", "creatures/sub/preview.png"), ("prefab", "sprites/x.spr"), ("map", "creatures/x.ini")):
            with self.subTest(kind=kind, path=bad), self.assertRaises(cm.Reject):
                cm.check_package(["README.txt", bad], kind)

    def test_strict_entry_names(self):
        cm.check_package(["maps/de_a-b 2.bsp", "README.txt", "creatures/creatures.ini", "models/gb1/a_b.mdl"], "map")
        bad = ["maps/a.bsp.", "maps/a.bsp ", "maps/dir./a.bsp", "maps/dir /a.bsp", "models/progra~1/a.mdl", "maps/de_dust~2.bsp",
               "maps/con.bsp", "maps/CON", "models/Nul.mdl", "sound/com1/x.wav", "sound/lpt9.wav", "models/aux .mdl", "maps/conout$.bsp",
               "maps/a(1).bsp", "maps/a;b.bsp", "maps/caf\u00e9.bsp", "maps/\u0130.bsp", "models/a\u200b.mdl"]
        for n in bad:
            with self.subTest(n=n), self.assertRaises(cm.Reject):
                cm.check_package(["README.txt", n], "map")
        # (com0, lpt10, console and con_x are ordinary names)
        cm.check_package(["maps/com0.bsp", "maps/lpt10.bsp", "maps/console.bsp", "maps/con_x.bsp"], "map")
        # twins: case, and the same letters composed or not (non-ASCII never passes, so neither twin does)
        for twins in (["maps/A.bsp", "maps/a.bsp"], ["MAPS/a.bsp", "maps/a.bsp"], ["maps/caf\u00e9.bsp", "maps/cafe\u0301.bsp"],
                      ["maps/\u00c9.bsp", "maps/\u00e9.bsp"]):
            with self.subTest(twins=twins), self.assertRaises(cm.Reject):
                cm.check_package(twins, "map")
        self.assertTrue(cm._outer_name_ok("de_dust2 (fixed).zip"), "the page's own file name keeps the wider set")

    def test_items(self):
        it = cm.clean_item(creature_item())
        self.assertEqual(it["creatures"], [{"name": "gb1001.runner_x", "model": True, "sounds": 2}, {"name": "gb1001.boss_y", "model": False, "sounds": 0}])
        m = cm.clean_item(map_item(creatures=CREATURES[:1]))
        self.assertEqual(m["creatures"][0]["name"], "gb1001.runner_x")
        self.assertNotIn("creatures", cm.clean_item(map_item()))
        u = "3f2b8c1e-9a4d-4e6f-8b2a-1c3d5e7f9a0b"
        r = cm.clean_item(creature_item(uid=u, remixOf={"model": "Mod", "gb": 5, "uid": u, "name": "Runner", "author": "bo"}))
        self.assertEqual((r["kind"], r["remixOf"]["gb"]), ("creature", 5))
        for bad in (creature_item(creatures=[]), creature_item(creatures=[{"name": "zombie"}]), creature_item(creatures=[{"name": "runner_x"}]),
                    creature_item(creatures="x"), creature_item(creatures=[{"name": "a.b"}] * 33),
                    dict(map_item(), creatures=[{"name": "../x"}])):
            with self.assertRaises(cm.Reject):
                cm.clean_item(bad)
        with self.assertRaises(cm.Reject):
            cm.clean_item({"kind": "spray", "model": "Spray", "gb": 1, "file": 1, "md5": "0" * 32, "name": "x", "creatures": CREATURES[:1]})

    def test_page_kind(self):
        self.assertEqual(cm.page_kind(creature_page(), "Mod"), "creature")
        self.assertEqual(cm.page_kind(PREFAB, "Mod"), "prefab")


class Registration(unittest.TestCase):
    """Whole registrations: the zip read by range requests and every file checked by the child."""

    def setup(self, data, page=None):
        gb, ff = FakeGB(), FakeFiles()
        gb.pages[("Mod", 706630)] = page or creature_page()
        ff.files[1789869] = data
        return make(gb=gb, files=ff)[0]

    def test_a_creature_is_listed_with_what_it_carries(self):
        c = self.setup(creature_zip())
        code, ans = c.submit(body(creature_item()), "ann")
        self.assertEqual((code, ans.get("state")), (200, "listed"), ans)
        it = [x for x in json.loads(cm.build_index(c.store)[0])["items"] if x["gb"] == 706630][0]
        self.assertEqual((it["kind"], [x["name"] for x in it["creatures"]]), ("creature", ["gb1001.runner_x", "gb1001.boss_y"]))

    def refused(self, data, words):
        c = self.setup(data)
        code, ans = c.submit(body(creature_item()), "ann")
        self.assertEqual(code, 422, ans)
        self.assertIn(words, ans["error"])

    def test_refusals(self):
        self.refused(creature_zip(ini=b"[zombie]\nhp = 9999\n"), "built-in")
        self.refused(creature_zip(ini=b"[runner_x]\nhp = 9\n"), "gb<member id>.<name>")
        self.refused(creature_zip(ini=b"[gb1001.runner_x]\nmodel = ../../../evil.mdl\n"), "outside")
        self.refused(creature_zip(ini=b"[gb1001.runner_x]\nsnd_die = sound/x.wav sound/../../x.wav\n[gb1001.boss_y]\n"), "outside")
        self.refused(creature_zip(ini=b"[gb1001.runner_x]\nbase = gb1001.gone\n[gb1001.boss_y]\n"), "gb1001.gone")
        self.refused(creature_zip(ini=b"[gb1001.runner_x]\nhp = 1\n"), "gb1001.boss_y")  # listed but not defined
        self.refused(creature_zip(extra=["addons/amxmodx/plugins/x.amxx"]), "Plugin")
        self.refused(creature_zip(extra=["dlls/mp.dll"]), "run code")
        self.refused(zip_of(ASSETS, dict(FILES, **{"loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": []})})), "creatures")
        no_ini = {k: v for k, v in FILES.items() if k != "creatures/creatures.ini"}
        self.refused(zip_of(ASSETS, dict(no_ini, **{"loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": CREATURES})})), "doesn't include them")
        self.refused(zip_of(ASSETS, dict(FILES, **{"models/gb1001/runner_x.mdl": b"not a model",
                                                   "loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": CREATURES})})), "runner_x.mdl")

    def test_ids_are_the_uploaders(self):
        # Ann (1001) publishes gb1001.*; someone else (7771234) can't publish changed gb1001 types
        c = self.setup(creature_zip())
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)
        c.get.pages[("Mod", 706631)] = creature_page(gb=706631, file_id=1789870, author="ann!")
        c.get_range.files[1789870] = creature_zip(ini=GOOD_INI.replace(b"hp = 80", b"hp = 5000"))
        code, ans = c.submit(body(creature_item(gb=706631, file=1789870)), "imposter")
        self.assertEqual(code, 422, ans)
        self.assertIn("yours are named gb7771234.", ans["error"])
        mine = [dict(x, name=x["name"].replace("gb1001.", "gb7771234.")) for x in CREATURES]
        c.get_range.files[1789870] = creature_zip(ini=GOOD_INI.replace(b"gb1001.", b"gb7771234."), man={"kind": "creature", "name": "x", "creatures": mine})
        code, ans = c.submit(body(creature_item(gb=706631, file=1789870, creatures=mine)), "ann!")
        self.assertEqual(code, 200, ans)

    def test_one_bad_row_never_stops_a_recheck(self):
        c = self.setup(creature_zip())
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)
        c.submit(body(map_item()), "a")
        # both pages dropped the tag: the first write fails, the second still happens, the mods are told
        for k in (("Mod", 706630), ("Mod", 739197)):
            pg = copy.deepcopy(c.get.pages[k])
            pg["_aTags"] = []
            c.get.pages[k] = pg
        put, said = c.store.put, []
        c.notifier.say = lambda text, wait=False: said.append(text)

        def flaky(row, action, detail="", claim=None):
            if row["key"] == "mod:706630":
                raise cm.Reject(409, "boom")
            return put(row, action, detail, claim)
        c.store.put = flaky
        out = c.recheck(keys=["mod:706630", "mod:739197"], pause=0)
        self.assertEqual(c.store.get("mod:739197")["state"], "pulled")
        self.assertEqual(c.store.get("mod:706630")["state"], "listed")
        self.assertTrue(any("Couldn't re-check (pull) `mod:706630`" in t for t in said), said)
        self.assertEqual([x["key"] for x in out["changes"]], ["mod:739197"])

    def test_one_bad_row_never_stops_a_block_with_pull(self):
        c = self.setup(creature_zip())
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)
        c.get.pages[("Mod", 706631)] = creature_page(gb=706631, file_id=1789870)
        c.get_range.files[1789870] = creature_zip()
        self.assertEqual(c.submit(body(creature_item(gb=706631, file=1789870)), "ann")[0], 200)
        put, said = c.store.put, []
        c.notifier.say = lambda text, wait=False: said.append(text)
        c.store.put = lambda row, action, detail="", claim=None: (_ for _ in ()).throw(RuntimeError("disk")) if row["key"] == "mod:706630" else put(row, action, detail, claim)
        pulled = c.block(1001, "Ann", "mod", "spam", pull=True)
        self.assertEqual(pulled, ["mod:706631"])
        self.assertEqual(c.store.get("mod:706631")["state"], "pulled")
        self.assertTrue(any("Couldn't pull `mod:706630`" in t for t in said), said)

    def test_creature_files(self):
        man = {"kind": "creature", "name": "Fast One", "creatures": CREATURES}
        # a file the creatures don't use, in their folder or anywhere in a creature package
        self.refused(creature_zip(extra=["sound/gb1001/unused.wav"]), "isn't used by any")
        self.refused(creature_zip(extra=["sound/other/x.wav"]), "isn't used by any")
        # a file the creatures use, shipped outside their folder
        ini = GOOD_INI.replace(b"models/gb1001/runner_x.mdl", b"models/mine/runner_x.mdl")
        self.refused(zip_of(ASSETS, {"creatures/creatures.ini": ini, "models/mine/runner_x.mdl": model(), "loner-strike.json": manifest(man)}), "must be in models/gb1001/")
        # the game's own model, shipped
        ini = GOOD_INI.replace(b"models/gb1001/runner_x.mdl models/zombie.mdl", b"models/zombie.mdl")
        self.refused(zip_of(ASSETS, {"creatures/creatures.ini": ini, "models/zombie.mdl": model(), "loner-strike.json": manifest(man)}), "game's own models/zombie.mdl")

    def test_carried_files_must_match_the_owners(self):
        c = self.setup(creature_zip())
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)
        changed = dict(FILES, **{"models/gb1001/runner_x.mdl": model() + b"x"})
        c.get_range.files[1885247] = zip_of(["maps/de_marakesh.bsp", "README.txt", *ASSETS],
                                            dict(changed, **{"loner-strike.json": manifest(dict(MAP_MANIFEST, creatures=CREATURES))}))
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("isn't the same as that creator's listed file", ans["error"])

    def test_stock_lists_match_the_apps(self):
        app = os.path.join(HERE, "..", "..", "app", "src-tauri", "src", "tools")
        for n in ("stock_valve.txt", "stock_cstrike.txt", "stock_czero.txt"):
            mine, theirs = os.path.join(HERE, "stock", n), os.path.join(app, n)
            if os.path.exists(theirs):
                with open(mine, "rb") as a, open(theirs, "rb") as b:
                    self.assertEqual(a.read(), b.read(), f"stock/{n} differs from the app's: copy it again")

    def test_duplicate_entries(self):
        import io as _io
        import warnings
        import zipfile as _zf
        for twin in ("creatures/creatures.ini", "Creatures/creatures.ini", "creatures//creatures.ini", "./creatures/creatures.ini", "creatures\\creatures.ini"):
            b = _io.BytesIO()
            with warnings.catch_warnings(), _zf.ZipFile(b, "w") as z:
                warnings.simplefilter("ignore")
                for n, data in dict(FILES, **{"loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": CREATURES})}).items():
                    z.writestr(n, data)
                for n in ASSETS:
                    z.writestr(n, b"RIFF")
                z.writestr(twin, b"[gb1001.runner_x]\n[gb1001.boss_y]\n")
            with self.subTest(twin=twin):
                c = self.setup(b.getvalue())
                code, ans = c.submit(body(creature_item()), "ann")
                self.assertEqual(code, 422, ans)
                self.assertRegex(ans["error"], "twice|file names|same file")

    def map_with(self, c, ini):
        c.get_range.files[1885247] = zip_of(["maps/de_marakesh.bsp", "README.txt", *ASSETS],
                                            dict(FILES, **{"creatures/creatures.ini": ini, "loner-strike.json": manifest(dict(MAP_MANIFEST, creatures=CREATURES))}))
        return c.submit(body(map_item()), "a")

    def test_a_map_carrying_someone_elses_listed_creature(self):
        # TheHouthi's map carries Ann's types: refused until Ann's creature is listed, then only unchanged
        c = self.setup(creature_zip())
        code, ans = self.map_with(c, GOOD_INI)
        self.assertEqual(code, 422, ans)
        self.assertIn("yours are named gb6096365.", ans["error"])
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)
        code, ans = self.map_with(c, GOOD_INI.replace(b"hp = 80", b"hp = 9999"))
        self.assertEqual(code, 422, "changed: not Ann's creature any more")
        code, ans = self.map_with(c, GOOD_INI.replace(b"; <<< end", b"; carried"))
        self.assertEqual(code, 200, ans)
        it = [x for x in json.loads(cm.build_index(c.store)[0])["items"] if x["gb"] == 739197][0]
        self.assertEqual([x["name"] for x in it["creatures"]], ["gb1001.runner_x", "gb1001.boss_y"])
        self.assertNotIn("checks", it, "the section hashes stay on the server")

    def test_own_namespace_and_remixes(self):
        # a remix of Ann's creature by Bob: in Bob's name, the credit in remixOf
        bob_ini = GOOD_INI.replace(b"gb1001", b"gb1002")
        bob = [dict(x, name=x["name"].replace("gb1001.", "gb1002.")) for x in CREATURES]
        man = {"kind": "creature", "name": "Faster One", "creatures": bob, "remixOf": {"model": "Mod", "gb": 706630, "name": "Fast One", "author": "Ann"}}
        c = self.setup(creature_zip(ini=bob_ini, man=man, folder="gb1002"), page=creature_page(author="Bob"))
        code, ans = c.submit(body(creature_item(creatures=bob)), "bob")
        self.assertEqual(code, 200, ans)
        it = [x for x in json.loads(cm.build_index(c.store)[0])["items"] if x["gb"] == 706630][0]
        self.assertEqual((it["creatures"][0]["name"], it["remixOf"]["author"]), ("gb1002.runner_x", "Ann"))
        # Bob can't publish in Ann's name
        c = self.setup(creature_zip(), page=creature_page(author="Bob"))
        code, ans = c.submit(body(creature_item()), "bob")
        self.assertEqual(code, 422, ans)
        self.assertIn("yours are named gb1002.", ans["error"])
        # files under the author's own folders pass
        c = self.setup(zip_of(ASSETS, dict(FILES_NS, **{"creatures/creatures.ini": GOOD_INI.replace(b"models/gb1001/runner_x.mdl", b"models/gb1001/qa/runner_x.mdl"),
                                                        "loner-strike.json": manifest({"kind": "creature", "name": "Fast One", "creatures": CREATURES})})))
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)

    def test_a_map_whose_creatures_change_a_built_in(self):
        ff = FakeFiles()
        ff.files[1885247] = zip_of(["maps/de_marakesh.bsp", "README.txt"], {"creatures/creatures.ini": b"[runner]\nhp = 1\n", "loner-strike.json": manifest(MAP_MANIFEST)})
        c, _ = make(files=ff)
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("built-in", ans["error"])

if __name__ == "__main__":
    unittest.main()


def npc_bsp(model):
    """A tiny map with an NPC whose model is `model` (NPC-editor)."""
    import struct
    ents = b'{\n"classname" "worldspawn"\n}\n{\n"classname" "ls_npc"\n"name" "Joe"\n"model" "' + model.encode() + b'"\n}\n\0'
    lumps = [(0, 0)] * 15
    lumps[0] = (124, len(ents))
    lumps[14] = (124 + len(ents), 64)
    head = struct.pack("<i", 30) + b"".join(struct.pack("<ii", o, n) for o, n in lumps)
    return head + ents + bytes(64)


class NpcFiles(unittest.TestCase):
    """NPC-editor: an NPC's or interactable's own model travels in the uploader's folder, listed on
    the pack's `; npc:` line and named by the package's own map."""

    def test_the_npc_line_is_checked(self):
        self.assertIsNone(formats.creatures_ini_problem(b"; types:\n; npc: models/gb1/qa/joe.mdl sound/gb1/hi.wav\n"))
        self.assertEqual(formats.creatures_ini_npc(b"; npc: models/gb1/qa/joe.mdl\n"), ["models/gb1/qa/joe.mdl"])
        for bad in (b"; npc: models/../x.mdl\n", b"; npc: /etc/x.mdl\n", b"; npc: c:/x.mdl\n", b"; npc: maps/x.bsp\n", b"; npc: models\\x.mdl\n"):
            with self.subTest(bad=bad):
                self.assertIsNotNone(formats.creatures_ini_problem(bad))
        self.assertIsNotNone(formats.creatures_ini_problem(b"; types:\n"), "nothing at all: refused")

    def setup(self, data):
        gb, ff = FakeGB(), FakeFiles()
        ff.files[1885247] = data
        return make(gb=gb, files=ff)[0]

    def npc_map(self, named, listed, ship):
        files = {"maps/de_marakesh.bsp": npc_bsp(named), "creatures/creatures.ini": b"; types:\n; npc: " + listed.encode() + b"\n",
                 ship: model(), "loner-strike.json": manifest(MAP_MANIFEST)}
        return zip_of(["README.txt"], files)

    def test_a_map_brings_its_npcs_model(self):
        mine = "models/gb6096365/qa/joe.mdl"
        c = self.setup(self.npc_map(mine, mine, mine))
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual((code, ans.get("state")), (200, "listed"), ans)

    def test_an_npc_file_the_map_doesnt_use(self):
        mine = "models/gb6096365/qa/joe.mdl"
        c = self.setup(self.npc_map("models/barney.mdl", mine, mine))
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("isn't used by any of the package's NPCs", ans["error"])

    def test_an_npc_file_in_someone_elses_folder(self):
        theirs = "models/gb1001/qa/joe.mdl"
        c = self.setup(self.npc_map(theirs, theirs, theirs))
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("isn't the same as that creator's listed file", ans["error"])

    def test_an_unlisted_file_in_the_owners_folder(self):
        mine = "models/gb6096365/qa/joe.mdl"
        files = {"maps/de_marakesh.bsp": npc_bsp(mine), "creatures/creatures.ini": b"; types:\n; npc: " + mine.encode() + b"\n",
                 mine: model(), "models/gb6096365/qa/stray.mdl": model(), "loner-strike.json": manifest(MAP_MANIFEST)}
        c = self.setup(zip_of(["README.txt"], files))
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("stray.mdl isn't used", ans["error"])
