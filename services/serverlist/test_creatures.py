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
; types: ann.runner_x ann.boss_y
; files: models/ann/runner_x.mdl sound/ann/hit.wav ../not/checked:here
[ann.runner_x]
base = runner
name = Fast One / v2
hp = 80
model = models/ann/runner_x.mdl models/zombie.mdl
snd_attack = ann/hit.wav ann/hit2.wav | player/pl_pain2.wav
[ann.boss_y]
base = ann.runner_x
; <<< end
"""
CREATURES = [{"name": "ann.runner_x", "model": True, "sounds": 2}, {"name": "ann.boss_y"}]
FILES = {"creatures/creatures.ini": GOOD_INI, "models/ann/runner_x.mdl": model()}
ASSETS = ["sound/ann/hit.wav", "sound/ann/hit2.wav"]


def creature_page(gb=706630, file_id=1789869):
    """A tagged page among the game's skins (the prefab fixture, moved and renumbered)."""
    p = copy.deepcopy(PREFAB)
    p["_idRow"] = gb
    p["_aFiles"] = p["_aFiles"][:1]
    p["_aFiles"][0]["_idRow"] = file_id
    p["_aCategory"] = {"_idRow": 999001, "_sName": "Monsters", "_sModelName": "ModCategory"}
    p["_aSuperCategory"] = {"_idRow": 999000, "_sName": "Skins", "_sModelName": "ModCategory"}
    return p


def creature_zip(ini=GOOD_INI, man=None, extra=()):
    files = dict(FILES, **{"creatures/creatures.ini": ini, "creatures/preview.png": b"\x89PNG\r\n\x1a\n" + bytes(32)})
    return zip_of([*ASSETS, *extra], dict(files, **{"loner-strike.json": manifest(man or {"kind": "creature", "name": "Fast One", "creatures": CREATURES})}))


def creature_item(**kw):
    it = {"kind": "creature", "model": "Mod", "gb": 706630, "file": 1789869, "md5": "0" * 32, "name": "Fast One", "creatures": CREATURES}
    it.update(kw)
    return it


class Rules(unittest.TestCase):
    def test_names(self):
        for ok in ("ann.runner_x", "a.b", "team-7.zombie", "x" * 10 + "." + "y" * 20):
            self.assertTrue(cm.creature_name_ok(ok), ok)
        for bad in ("runner", "zombie", "Ann.runner", "ann.", ".x", "ann.123", "ann.a-b", "../x.y", "a.b.c", "x" * 12 + "." + "y" * 20, None, 5):
            self.assertFalse(cm.creature_name_ok(bad), bad)

    def test_ini(self):
        self.assertIsNone(cm.creatures_ini_problem(GOOD_INI))
        # a base defined later in the same file, and a built-in base in any case
        self.assertIsNone(cm.creatures_ini_problem(b"[ann.a]\nbase = ann.b\n[ann.b]\nbase = Zombie\n"))
        self.assertEqual(cm.creatures_ini_types(GOOD_INI), ["ann.runner_x", "ann.boss_y"])
        for bad, words in ((b"[zombie]\nhp = 1\n", "built-in"), (b"[Runner]\n", "built-in"), (b"[runner_x]\n", "<author>.<name>"),
                           (b"[ann.x]\nmodel = ../../evil.mdl\n", "outside"), (b"[ann.x]\nmodel = C:/x.mdl\n", "outside"),
                           (b"[ann.x]\nmodel = models\\\\x.mdl\n", "outside"), (b"[ann.x]\nmodel = maps/x.bsp\n", "models/....mdl"),
                           (b"[ann.x]\nmodel = models/a.mdl | sound/x.wav\n", "models/....mdl"),
                           (b"[ann.x]\nsnd_die = x.exe\n", ".wav path"), (b"[ann.x]\nsnd_die = a/1.wav | /etc/x.wav\n", ".wav path"),
                           (b"[ann.x]\nsnd_pain = a/../../x.wav\n", "outside"), (b"[ann.x]\nbase = ann.nope\n", "isn't a built-in"),
                           (b"[ann.x]\nbase = bob.y\n[ann.y]\n", "isn't a built-in"), (b"; nothing\n", "no creature"),
                           (b"[ann.x]\n" + b"a" * 600 + b"\n", "longer"), (b"[ann.x]\0", "text"), (b"[ann.x]\n" * 1 + b" " * (65 << 10), "too big"),
                           (b"".join(b"[ann.t%d]\n" % i for i in range(33)), "33 types")):
            self.assertIn(words, cm.creatures_ini_problem(bad) or "", bad[:40])

    def test_the_format_check_reads_it(self):
        self.assertEqual(formats.check("creatures/creatures.ini", __import__("io").BytesIO(GOOD_INI), len(GOOD_INI)), "creatures")
        with self.assertRaises(formats.FormatError):
            formats.check("creatures/creatures.ini", __import__("io").BytesIO(b"[zombie]\n"), 9)
        # another .ini is plain text
        self.assertEqual(formats.check("addons/amxmodx/configs/gamemodes/zombie.ini", __import__("io").BytesIO(b"[zombie]\n"), 9), "text")

    def test_where_creature_files_may_be(self):
        cm.check_package(["maps/a.bsp", "creatures/creatures.ini", "models/ann/x.mdl", "sound/ann/x.wav"], "map")
        cm.check_package(["modes/cage/graph.json", "creatures/creatures.ini", "models/ann/x.mdl"], "mode")
        cm.check_package(["prefabs/can.map", "prefabs/can.json", "creatures/creatures.ini", "models/ann/x.mdl", "sound/ann/x.wav"], "prefab")
        cm.check_package(["creatures/creatures.ini", "creatures/preview.png", "models/ann/x.mdl", "sound/ann/x.wav", "README.txt", "loner-strike.json"], "creature")
        for kind, bad in (("spray", "creatures/creatures.ini"), ("texture", "creatures/creatures.ini"), ("creature", "maps/a.bsp"),
                          ("creature", "creatures/other.ini"), ("creature", "addons/amxmodx/plugins/x.amxx"), ("creature", "../models/x.mdl"),
                          ("creature", "creatures/sub/preview.png"), ("prefab", "sprites/x.spr"), ("map", "creatures/x.ini")):
            with self.subTest(kind=kind, path=bad), self.assertRaises(cm.Reject):
                cm.check_package(["README.txt", bad], kind)

    def test_items(self):
        it = cm.clean_item(creature_item())
        self.assertEqual(it["creatures"], [{"name": "ann.runner_x", "model": True, "sounds": 2}, {"name": "ann.boss_y", "model": False, "sounds": 0}])
        m = cm.clean_item(map_item(creatures=CREATURES[:1]))
        self.assertEqual(m["creatures"][0]["name"], "ann.runner_x")
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
        self.assertEqual((it["kind"], [x["name"] for x in it["creatures"]]), ("creature", ["ann.runner_x", "ann.boss_y"]))

    def refused(self, data, words):
        c = self.setup(data)
        code, ans = c.submit(body(creature_item()), "ann")
        self.assertEqual(code, 422, ans)
        self.assertIn(words, ans["error"])

    def test_refusals(self):
        self.refused(creature_zip(ini=b"[zombie]\nhp = 9999\n"), "built-in")
        self.refused(creature_zip(ini=b"[runner_x]\nhp = 9\n"), "<author>.<name>")
        self.refused(creature_zip(ini=b"[ann.runner_x]\nmodel = ../../../evil.mdl\n"), "outside")
        self.refused(creature_zip(ini=b"[ann.runner_x]\nsnd_die = sound/x.wav sound/../../x.wav\n[ann.boss_y]\n"), "outside")
        self.refused(creature_zip(ini=b"[ann.runner_x]\nbase = ann.gone\n[ann.boss_y]\n"), "ann.gone")
        self.refused(creature_zip(ini=b"[ann.runner_x]\nhp = 1\n"), "ann.boss_y")  # listed but not defined
        self.refused(creature_zip(extra=["addons/amxmodx/plugins/x.amxx"]), "Plugin")
        self.refused(creature_zip(extra=["dlls/mp.dll"]), "run code")
        self.refused(zip_of(ASSETS, dict(FILES, **{"loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": []})})), "creatures")
        no_ini = {k: v for k, v in FILES.items() if k != "creatures/creatures.ini"}
        self.refused(zip_of(ASSETS, dict(no_ini, **{"loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": CREATURES})})), "doesn't include them")
        self.refused(zip_of(ASSETS, dict(FILES, **{"models/ann/runner_x.mdl": b"not a model",
                                                   "loner-strike.json": manifest({"kind": "creature", "name": "x", "creatures": CREATURES})})), "runner_x.mdl")

    def test_a_map_carrying_creatures(self):
        ff = FakeFiles()
        ff.files[1885247] = zip_of(["maps/de_marakesh.bsp", "README.txt", *ASSETS],
                                   dict(FILES, **{"loner-strike.json": manifest(dict(MAP_MANIFEST, creatures=CREATURES))}))
        c, _ = make(files=ff)
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 200, ans)
        it = json.loads(cm.build_index(c.store)[0])["items"][0]
        self.assertEqual([x["name"] for x in it["creatures"]], ["ann.runner_x", "ann.boss_y"])
        # a map whose creatures.ini changes a built-in type is refused
        ff.files[1885247] = zip_of(["maps/de_marakesh.bsp", "README.txt"], {"creatures/creatures.ini": b"[runner]\nhp = 1\n", "loner-strike.json": manifest(MAP_MANIFEST)})
        c, _ = make(files=ff)
        code, ans = c.submit(body(map_item()), "a")
        self.assertEqual(code, 422, ans)
        self.assertIn("built-in", ans["error"])


if __name__ == "__main__":
    unittest.main()
