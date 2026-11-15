#!/usr/bin/env python3
"""Runs the format checks (formats.py) over folders of real game content, read-only.

  python3 corpus_check.py <folder>... [--hostile]

Prints how many files of each kind were checked and every refusal with its reason (a refusal of
real content is a bug in a check unless the file really is broken). --hostile also damages a
sample of them in memory (truncated, offsets and counts made huge, versions changed) and
reports any damaged file that was NOT refused, or any crash.
"""
import io
import os
import random
import struct
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import formats  # noqa: E402


def files(roots):
    for root in roots:
        for d, _, names in os.walk(root):
            for n in names:
                p = os.path.join(d, n)
                ext = n.lower()[n.lower().rfind("."):] if "." in n else ""
                if ext in formats.CHECKS and os.path.isfile(p):
                    yield p, ext


def check_bytes(name, data):
    return formats.check(name, io.BytesIO(data), len(data))


TEXT = (".txt", ".res", ".ini", ".json", ".map", ".mp3", ".png", ".jpg", ".jpeg")


def damaged(ext, data, rnd):
    """Variants of a good file that must be refused (text and signature-only kinds are still
    themselves when cut short, so only the binary formats are damaged)."""
    if ext in TEXT:
        return []
    out = [("empty", b""), ("4 bytes", data[:4]), ("a third", data[:len(data) // 3])]
    if len(data) > 64:
        out.append(("half", data[:len(data) // 2]))
    if ext in (".bsp", ".wad", ".mdl", ".spr", ".nav", ".lsv", ".wav"):
        out.append(("bad magic/version", b"XXXX" + data[4:] if ext != ".bsp" else struct.pack("<i", 31) + data[4:]))
    if ext == ".bsp" and len(data) >= 124:
        for lump in (1, 5, 7, 12, 14):
            b = bytearray(data)
            struct.pack_into("<i", b, 4 + lump * 8, len(data) - 4)  # a lump starting at the end
            struct.pack_into("<i", b, 8 + lump * 8, 4096)
            out.append((f"lump {lump} past the end", bytes(b)))
    if ext == ".wad" and len(data) >= 12:
        b = bytearray(data)
        struct.pack_into("<i", b, 4, 0x7FFFFFFF)
        out.append(("huge count", bytes(b)))
        b = bytearray(data)
        struct.pack_into("<i", b, 8, len(data))
        out.append(("directory past the end", bytes(b)))
    if ext == ".mdl" and len(data) >= 244 and data[:4] == b"IDST":
        for field in (0, 6, 10, 16):
            b = bytearray(data)
            struct.pack_into("<i", b, 140 + field * 4, 0x7FFFFFF)
            out.append((f"huge count {field}", bytes(b)))
            b = bytearray(data)
            if struct.unpack_from("<i", data, 140 + field * 4)[0]:
                struct.pack_into("<i", b, 144 + field * 4, len(data) - 8)
                out.append((f"table {field} past the end", bytes(b)))
    if ext == ".spr" and len(data) >= 42:
        b = bytearray(data)
        struct.pack_into("<i", b, 28, 1 << 20)
        out.append(("huge frame count", bytes(b)))
    return out


def main(argv):
    hostile = "--hostile" in argv
    roots = [a for a in argv if a != "--hostile"]
    seen, refused, missed, crashed = Counter(), [], [], []
    tried = 0
    rnd = random.Random(1)
    for i, (p, ext) in enumerate(files(roots)):
        try:
            with open(p, "rb") as f:
                formats.check(p, f, os.path.getsize(p))
            seen[ext] += 1
        except formats.FormatError as e:
            refused.append((p, str(e)))
            seen[ext] += 1
            continue
        except Exception as e:  # a crash is a bug either way
            crashed.append((p, repr(e)))
            continue
        if hostile and os.path.getsize(p) <= 64 << 20:
            data = open(p, "rb").read()
            for what, bad in damaged(ext, data, rnd):
                tried += 1
                try:
                    check_bytes(p, bad)
                    if bad != data:
                        missed.append((p, what))
                except formats.FormatError:
                    pass
                except Exception as e:
                    crashed.append((p, f"{what}: {e!r}"))
    total = sum(seen.values())
    print(f"checked {total}: " + ", ".join(f"{k} {v}" for k, v in sorted(seen.items())))
    print(f"refused {len(refused)}")
    for p, why in refused:
        print(f"  REFUSED {p}: {why}")
    print(f"crashed {len(crashed)}")
    for p, why in crashed:
        print(f"  CRASH {p}: {why}")
    if hostile:
        print(f"damaged variants tried {tried}")
        print(f"damaged variants not refused {len(missed)}")
        groups = Counter((os.path.splitext(p)[1].lower(), what) for p, what in missed)
        for (ext, what), n in groups.most_common():
            print(f"  {n:6} {ext} {what}")
        for p, what in missed[:20]:
            print(f"  MISSED {p}: {what}")
    return 1 if refused or crashed or missed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
