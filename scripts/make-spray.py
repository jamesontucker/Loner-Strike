#!/usr/bin/env python3
"""Makes the bots' spray (plugins/game/sprites/lonerstrike/spray.spr) from the app icon.

A GoldSrc sprite, oriented (it lies flat on whatever wall it's put on) and alpha-tested
(palette index 255 is see-through), 64x64 like a player's spray. gm_humanbots puts it on
walls. Run it again when the icon changes:

    python3 scripts/make-spray.py [icon.png] [out.spr]

No image library needed: it reads the PNG itself (8-bit RGB or RGBA, not interlaced).
"""

import math
import os
import struct
import sys
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "app", "src-tauri", "icons", "128x128.png")
OUT = os.path.join(ROOT, "plugins", "game", "sprites", "lonerstrike", "spray.spr")
SIZE = 64


def read_png(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        sys.exit(f"{path} isn't a PNG")
    pos, idat, w = 8, b"", 0
    while pos < len(data):
        n, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        pos += 12 + n
        if kind == b"IHDR":
            w, h, depth, color, _, _, interlace = struct.unpack(">IIBBBBB", body)
            if depth != 8 or color not in (2, 6) or interlace:
                sys.exit("Only 8-bit RGB/RGBA PNGs that aren't interlaced")
            bpp = 4 if color == 6 else 3
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    raw = zlib.decompress(idat)
    stride = w * bpp
    rows, prev, i = [], bytearray(stride), 0
    for _ in range(h):
        f = raw[i]
        line = bytearray(raw[i + 1:i + 1 + stride])
        i += 1 + stride
        for x in range(stride):
            a = line[x - bpp] if x >= bpp else 0
            b = prev[x]
            c = prev[x - bpp] if x >= bpp else 0
            if f == 1:
                line[x] = (line[x] + a) & 255
            elif f == 2:
                line[x] = (line[x] + b) & 255
            elif f == 3:
                line[x] = (line[x] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[x] = (line[x] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    px = [[tuple(r[x * bpp:x * bpp + bpp]) + ((255,) if bpp == 3 else ()) for x in range(w)] for r in rows]
    return w, h, px


def shrink(w, h, px, size):
    """Box filter down to size x size (colour weighted by alpha, so edges don't go dark)."""
    out = []
    for y in range(size):
        row = []
        y0, y1 = y * h // size, max(y * h // size + 1, (y + 1) * h // size)
        for x in range(size):
            x0, x1 = x * w // size, max(x * w // size + 1, (x + 1) * w // size)
            r = g = b = a = 0
            for yy in range(y0, y1):
                for xx in range(x0, x1):
                    pr, pg, pb, pa = px[yy][xx]
                    r += pr * pa
                    g += pg * pa
                    b += pb * pa
                    a += pa
            n = (y1 - y0) * (x1 - x0)
            row.append((r // a, g // a, b // a, a // n) if a else (0, 0, 0, 0))
        out.append(row)
    return out


def palette(colours, k=255):
    """Median cut to at most k colours."""
    boxes = [list(set(colours))]
    while len(boxes) < k:
        boxes.sort(key=lambda b: max(max(c[i] for c in b) - min(c[i] for c in b) for i in range(3)) if len(b) > 1 else -1)
        box = boxes.pop()
        if len(box) < 2:
            boxes.append(box)
            break
        ch = max(range(3), key=lambda i: max(c[i] for c in box) - min(c[i] for c in box))
        box.sort(key=lambda c: c[ch])
        boxes += [box[:len(box) // 2], box[len(box) // 2:]]
    return [tuple(sum(c[i] for c in b) // len(b) for i in range(3)) for b in boxes if b]


def main():
    src = sys.argv[1] if len(sys.argv) > 1 else SRC
    out = sys.argv[2] if len(sys.argv) > 2 else OUT
    w, h, px = read_png(src)
    img = shrink(w, h, px, SIZE)
    solid = [p[:3] for row in img for p in row if p[3] >= 128]
    pal = palette(solid)
    near = {}

    def index(c):
        if c not in near:
            near[c] = min(range(len(pal)), key=lambda i: sum((pal[i][j] - c[j]) ** 2 for j in range(3)))
        return near[c]

    pixels = bytes(index(p[:3]) if p[3] >= 128 else 255 for row in img for p in row)
    pal = pal + [(0, 0, 0)] * (255 - len(pal)) + [(0, 0, 255)]  # 255: see-through

    # sprite v2: type 3 oriented, format 3 alpha test, one frame
    head = struct.pack("<4siiifiiifi", b"IDSP", 2, 3, 3, math.hypot(SIZE / 2, SIZE / 2), SIZE, SIZE, 1, 0.0, 0)
    head += struct.pack("<h", 256) + b"".join(bytes(c) for c in pal)
    frame = struct.pack("<iiiii", 0, -SIZE // 2, SIZE // 2, SIZE, SIZE) + pixels
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, "wb").write(head + frame)
    print(f"{out}: {SIZE}x{SIZE}, {len(set(pixels)) - (255 in pixels)} colours, {os.path.getsize(out)} bytes")


if __name__ == "__main__":
    main()
