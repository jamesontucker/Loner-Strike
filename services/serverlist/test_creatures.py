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
# (the app keeps packed files under the author's own folders)
FILES_NS = {"models/ann/qa/runner_x.mdl": model()}
ASSETS = ["sound/ann/hit.wav", "sound/ann/hit2.wav"]


def creature_page(gb=706630, file_id=1789869, author="Ann"):
    """A tagged page among the game's skins (the prefab fixture, moved, renumbered, by Ann)."""
    p = copy.deepcopy(PREFAB)
    p["_aSubmitter"]["_sName"] = author
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
                           (b"[ann.x]\nsnd_die = x.exe\n", ".wav path"), (b"[ann.x]\nsnd_die = a/1.wav | /etc/x.wav\n", "outside"),
                           (b"[ann.x]\nsnd_pain = a/../../x.wav\n", "outside"), (b"[ann.x]\nbase = ann.nope\n", "isn't a built-in"),
                           (b"[ann.x]\nbase = bob.y\n[ann.y]\n", "isn't a built-in"), (b"; nothing\n", "no creature"),
                           (b"[ann.x]\n" + b"a" * 600 + b"\n", "longer"), (b"[ann.x]\0", "text"), (b"[ann.x]\n" * 1 + b" " * (65 << 10), "too big"),
                           (b"".join(b"[ann.t%d]\n" % i for i in range(33)), "33 types")):
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
                why = cm.creatures_ini_problem(b"[ann.x]\n" + line + b"\n")
                if words is None:
                    self.assertIsNone(why)
                else:
                    self.assertIn(words, why or "")

    def test_namespaces(self):
        self.assertEqual((cm.author_slug("Jam Tucker!"), cm.author_slug("  "), cm.author_slug("1234"), cm.author_slug("Texture_Woman66")),
                         ("jam_tucker", "a", "a1234", "texture_woman66"))
        self.assertTrue(cm.namespace_is("jam_tucker.x", "jam_tucker"))
        self.assertFalse(cm.namespace_is("jam.x", "jam_tucker"), "a cut only when the name needs it")
        self.assertFalse(cm.namespace_is("bob.x", "bobby"), "someone else's shorter name")
        long_slug = "abcdefghijklmnopqrst"
        self.assertTrue(cm.namespace_is("abcdefghij." + "y" * 20, long_slug))
        self.assertFalse(cm.namespace_is("abcdefghi." + "y" * 20, long_slug))
        sec = cm.creatures_ini_sections(GOOD_INI)
        self.assertEqual(sorted(sec), ["ann.boss_y", "ann.runner_x"])
        # comments and spacing don't change a section; a value does
        same = GOOD_INI.replace(b"hp = 80", b"hp = 80   ; tough").replace(b"; <<< end", b"")
        self.assertEqual(cm.creatures_ini_sections(same), sec)
        self.assertNotEqual(cm.creatures_ini_sections(GOOD_INI.replace(b"hp = 80", b"hp = 81"))["ann.runner_x"], sec["ann.runner_x"])

    def test_the_format_check_reads_it(self):
        self.assertEqual(formats.check("creatures/creatures.ini", __import__("io").BytesIO(GOOD_INI), len(GOOD_INI)), "creatures")
        with self.assertRaises(formats.FormatError):
            formats.check("creatures/creatures.ini", __import__("io").BytesIO(b"[zombie]\n"), 9)
        # another .ini is plain text
        self.assertEqual(formats.check("addons/amxmodx/configs/gamemodes/zombie.ini", __import__("io").BytesIO(b"[zombie]\n"), 9), "text")

    def test_where_creature_files_may_be(self):
        cm.check_package(["maps/a.bsp", "creatures/creatures.ini", "models/ann/x.mdl", "sound/ann/x.wav"], "map")
        cm.check_package(["creatures/creatures.ini", "models/ann/qa/x.mdl", "sound/ann/qa/a/1.wav"], "creature")
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

    def map_with(self, c, ini):
        c.get_range.files[1885247] = zip_of(["maps/de_marakesh.bsp", "README.txt", *ASSETS],
                                            dict(FILES, **{"creatures/creatures.ini": ini, "loner-strike.json": manifest(dict(MAP_MANIFEST, creatures=CREATURES))}))
        return c.submit(body(map_item()), "a")

    def test_a_map_carrying_someone_elses_listed_creature(self):
        # TheHouthi's map carries Ann's types: refused until Ann's creature is listed, then only unchanged
        c = self.setup(creature_zip())
        code, ans = self.map_with(c, GOOD_INI)
        self.assertEqual(code, 422, ans)
        self.assertIn("isn't in your name (thehouthi.", ans["error"])
        self.assertEqual(c.submit(body(creature_item()), "ann")[0], 200)
        code, ans = self.map_with(c, GOOD_INI.replace(b"hp = 80", b"hp = 9999"))
        self.assertEqual(code, 422, "changed: not Ann's creature any more")
        code, ans = self.map_with(c, GOOD_INI.replace(b"; <<< end", b"; carried"))
        self.assertEqual(code, 200, ans)
        it = [x for x in json.loads(cm.build_index(c.store)[0])["items"] if x["gb"] == 739197][0]
        self.assertEqual([x["name"] for x in it["creatures"]], ["ann.runner_x", "ann.boss_y"])
        self.assertNotIn("checks", it, "the section hashes stay on the server")

    def test_own_namespace_and_remixes(self):
        # a remix of Ann's creature by Bob: in Bob's name, the credit in remixOf
        bob_ini = GOOD_INI.replace(b"ann.", b"bob.")
        bob = [dict(x, name=x["name"].replace("ann.", "bob.")) for x in CREATURES]
        man = {"kind": "creature", "name": "Faster One", "creatures": bob, "remixOf": {"model": "Mod", "gb": 706630, "name": "Fast One", "author": "Ann"}}
        c = self.setup(creature_zip(ini=bob_ini, man=man), page=creature_page(author="Bob"))
        code, ans = c.submit(body(creature_item(creatures=bob)), "bob")
        self.assertEqual(code, 200, ans)
        it = [x for x in json.loads(cm.build_index(c.store)[0])["items"] if x["gb"] == 706630][0]
        self.assertEqual((it["creatures"][0]["name"], it["remixOf"]["author"]), ("bob.runner_x", "Ann"))
        # Bob can't publish in Ann's name
        c = self.setup(creature_zip(), page=creature_page(author="Bob"))
        code, ans = c.submit(body(creature_item()), "bob")
        self.assertEqual(code, 422, ans)
        self.assertIn("isn't in your name (bob.", ans["error"])
        # files under the author's own folders pass
        c = self.setup(zip_of(ASSETS, dict(FILES_NS, **{"creatures/creatures.ini": GOOD_INI.replace(b"models/ann/runner_x.mdl", b"models/ann/qa/runner_x.mdl"),
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
