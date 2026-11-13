#!/usr/bin/env python3
# Tests for the format checks (formats.py): valid samples pass; damaged ones (cut short, offsets
# and counts made huge, wrong versions, random damage) are refused with FormatError and never
# crash.  cd services/serverlist && python3 -m unittest test_formats
# The real-content proof (no false refusals) is corpus_check.py over the installed game, the
# nav pack and GameBanana packages; LS_CORPUS="<folder>:<folder>" runs it here too.
import io
import os
import random
import struct
import sys
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import formats  # noqa: E402
from test_community import sample, tiny_bsp  # noqa: E402


def check(name, data):
    return formats.check(name, io.BytesIO(data), len(data))


def sprite():
    head = b"IDSP" + struct.pack("<iiifiiifi", 2, 2, 0, 1.0, 2, 2, 1, 0.0, 0)
    return head + struct.pack("<H", 256) + bytes(768) + struct.pack("<i", 0) + struct.pack("<4i", 0, 0, 2, 2) + bytes(4)


def model():
    h = bytearray(244)
    h[0:4] = b"IDST"
    struct.pack_into("<i", h, 4, 10)
    struct.pack_into("<i", h, 72, 244 + 80 + 16 + 768)
    struct.pack_into("<ii", h, 140 + 10 * 4, 1, 244)  # one texture, its record at 244
    tex = b"skin.bmp".ljust(64, b"\0") + struct.pack("<4i", 0, 4, 4, 244 + 80)
    return bytes(h) + tex + bytes(16) + bytes(768)


def wad():
    mip = b"brick".ljust(16, b"\0") + struct.pack("<6I", 16, 16, 40, 40 + 256, 40 + 320, 40 + 336) + bytes(256 + 64 + 16 + 4) + struct.pack("<H", 256) + bytes(768)
    entry = struct.pack("<iiiBBH16s", 12, len(mip), len(mip), 0x43, 0, 0, b"brick")
    return b"WAD3" + struct.pack("<ii", 1, 12 + len(mip)) + mip + entry


SAMPLES = {"maps/a.bsp": tiny_bsp(), "maps/a.nav": sample("a.nav"), "maps/a.lsv": sample("a.lsv"), "sound/a.wav": sample("a.wav"),
           "a.wad": wad(), "sprites/a.spr": sprite(), "models/a.mdl": model(), "overviews/a.bmp": sample("a.bmp"),
           "gfx/a.tga": sample("a.tga")}


class Formats(unittest.TestCase):
    def test_samples_pass(self):
        for name, data in SAMPLES.items():
            with self.subTest(name):
                self.assertIsNotNone(check(name, data))
        self.assertEqual(check("README.txt", b"hello"), "text")
        self.assertEqual(check("x.json", b'{"a": 1}'), "text")
        self.assertIsNone(check("x.unknown", b"whatever"))

    def test_every_cut_is_refused(self):
        for name, data in SAMPLES.items():
            if name.endswith((".nav", ".lsv")) and len(data) < 30:
                continue  # (an empty nav/knowledge file cut at its end is still a header)
            # (a model's last texture palette may be cut short, as in real models: up to 768 bytes)
            for n in range(len(data) - (768 if name.endswith(".mdl") else 0)):
                with self.subTest(name=name, cut=n):
                    with self.assertRaises(formats.FormatError):
                        check(name, data[:n])

    def test_wrong_versions_and_kinds(self):
        bad = {"maps/a.bsp": struct.pack("<i", 31) + tiny_bsp()[4:], "a.wad": b"WAD2" + wad()[4:],
               "models/a.mdl": model()[:4] + struct.pack("<i", 9) + model()[8:], "sprites/a.spr": b"IDSP" + struct.pack("<i", 1) + sprite()[8:],
               "maps/a.nav": struct.pack("<II", 0xFEEDFACE, 9) + sample("a.nav")[8:], "sound/a.wav": sample("a.wav").replace(b"\x01\x00\x01\x00", b"\x02\x00\x01\x00", 1),
               "a.mp3": b"<html>", "a.json": b"{nope", "a.txt": b"a\0b"}
        for name, data in bad.items():
            with self.subTest(name):
                with self.assertRaises(formats.FormatError):
                    check(name, data)

    def test_huge_counts_and_offsets(self):
        b = bytearray(tiny_bsp())
        struct.pack_into("<ii", b, 4 + 14 * 8, 124, 64 * 1000)  # a models lump past the end
        variants = [("maps/a.bsp", bytes(b))]
        b = bytearray(tiny_bsp())
        struct.pack_into("<ii", b, 4 + 14 * 8, 124, 63)  # not a whole record
        variants.append(("maps/a.bsp", bytes(b)))
        w = bytearray(wad())
        struct.pack_into("<i", w, 4, 1 << 30)
        variants.append(("a.wad", bytes(w)))
        m = bytearray(model())
        struct.pack_into("<i", m, 140, 1 << 20)  # a million bones
        variants.append(("models/a.mdl", bytes(m)))
        m = bytearray(model())
        struct.pack_into("<i", m, 244 + 76, 1 << 24)  # its texture's pixels far away
        variants.append(("models/a.mdl", bytes(m)))
        s = bytearray(sprite())
        struct.pack_into("<i", s, 28, 1 << 20)
        variants.append(("sprites/a.spr", bytes(s)))
        for name, data in variants:
            with self.subTest(name):
                with self.assertRaises(formats.FormatError):
                    check(name, data)

    def test_random_damage_never_crashes(self):
        rnd = random.Random(7)
        for name, data in SAMPLES.items():
            for _ in range(400):
                b = bytearray(data)
                for _k in range(rnd.randint(1, 8)):
                    b[rnd.randrange(len(b))] = rnd.randrange(256)
                try:
                    check(name, bytes(b))
                except formats.FormatError:
                    pass

    @unittest.skipUnless(os.environ.get("LS_CORPUS"), "set LS_CORPUS to folders of real game content")
    def test_real_content(self):
        import corpus_check
        self.assertEqual(corpus_check.main(os.environ["LS_CORPUS"].split(os.pathsep)), 0)


if __name__ == "__main__":
    unittest.main()
