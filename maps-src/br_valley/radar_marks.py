#!/usr/bin/env python3
"""br_valley: the ways down on the radar, and nothing outside the map's square.

Run after the radar maker has written the overview (8-bit BMP and its .txt):
    python3 radar_marks.py <overviews/br_valley.bmp> [<br_valley.layers.json>]
- a yellow down-arrow with a dark rim over every stair trench's top (the big ways down);
- a smaller one over every ladder hatch;
- everything outside the play square (+/-3584, a few units of slack) painted the background colour (the ring's
  notches left dark wedges sticking out of the square).
The picture's palette: an entry no pixel uses becomes the marks' yellow (else the nearest to it); near-black is
the nearest entry.
"""
import json
import os
import struct
import sys

E = 3592            # the play edge (the radar shows the play square; the ring's notches left wedges past it)
WIDTH, HEIGHT = 1024, 768


def frame(txt):
    zoom, origin, rot = 1.0, (0.0, 0.0, 0.0), False
    for line in open(txt):
        t = line.split()
        if len(t) >= 2 and t[0].upper() == "ZOOM":
            zoom = float(t[1])
        elif len(t) >= 4 and t[0].upper() == "ORIGIN":
            origin = tuple(float(v) for v in t[1:4])
        elif len(t) >= 2 and t[0].upper() == "ROTATED":
            rot = int(float(t[1])) != 0
    return zoom, origin, rot


def main():
    bmp = sys.argv[1]
    here = os.path.dirname(os.path.abspath(__file__))
    layers = sys.argv[2] if len(sys.argv) > 2 else os.path.join(here, "br_valley.layers.json")
    zoom, (ox, oy, _oz), rot = frame(os.path.splitext(bmp)[0] + ".txt")
    u = 8192.0 / (zoom * WIDTH)

    def to_px(x, y):        # (as the radar maker's Frame::to_pixel)
        if rot:
            return (x - ox + 4096.0 / zoom) / u, (oy + 3072.0 / zoom - y) / u
        return (oy + 4096.0 / zoom - y) / u, (ox + 3072.0 / zoom - x) / u

    def to_world(c, r):
        if rot:
            return ox - 4096.0 / zoom + c * u, oy + 3072.0 / zoom - r * u
        return ox + 3072.0 / zoom - r * u, oy + 4096.0 / zoom - c * u

    d = bytearray(open(bmp, "rb").read())
    off, = struct.unpack("<I", d[10:14])
    w, h, planes, bpp = struct.unpack("<iiHH", d[18:30])
    assert bpp == 8 and w == WIDTH and abs(h) == HEIGHT, (w, h, bpp)
    pal = [(d[54 + 4 * k + 2], d[54 + 4 * k + 1], d[54 + 4 * k]) for k in range(256)]
    stride = (w + 3) & ~3
    bottom_up = h > 0

    def at(c, r):
        return off + ((HEIGHT - 1 - r) if bottom_up else r) * stride + c

    def nearest(rgb):
        return min(range(256), key=lambda k: sum((pal[k][i] - rgb[i]) ** 2 for i in range(3)))

    bg = d[at(0, 0)]                     # (the corner is background)
    used = set(d[off:off + stride * HEIGHT])
    free = [k for k in range(256) if k not in used]
    if free:
        yellow = free[0]
        d[54 + 4 * yellow:54 + 4 * yellow + 3] = bytes((40, 214, 255))      # (BGR)
        pal[yellow] = (255, 214, 40)
    else:
        yellow = nearest((255, 214, 40))
    dark = nearest((20, 20, 20))
    # outside the ring: background
    masked = 0
    for r in range(HEIGHT):
        for c in range(WIDTH):
            x, y = to_world(c + 0.5, r + 0.5)
            if (abs(x) > E or abs(y) > E) and d[at(c, r)] != bg:
                d[at(c, r)] = bg
                masked += 1

    def arrow(x, y, size):
        """a down-pointing triangle (in the picture) with a dark rim, centred on the world point"""
        cx, cy = to_px(x, y)
        n = 0
        for r in range(int(cy - size - 2), int(cy + size + 3)):
            for c in range(int(cx - size - 2), int(cx + size + 3)):
                if not (0 <= c < WIDTH and 0 <= r < HEIGHT):
                    continue
                t = (r + 0.5 - (cy - size)) / (2.0 * size)        # 0 at the top edge, 1 at the point
                half = size * (1.0 - t)
                dx = abs(c + 0.5 - cx)
                if -0.12 <= t <= 1.12 and dx <= half + 1.6:
                    inner = 0.0 <= t <= 1.0 and dx <= half - 0.4 and t <= 1.0 - 0.9 / size
                    d[at(c, r)] = yellow if inner and t >= 0.12 else dark
                    n += 1
        return n

    L = json.load(open(layers))
    trenches = [c for c in L["connectors"] if c["name"].startswith("trench:")]
    hatches = [c for c in L["connectors"] if c["name"].startswith("hatch:")]
    seen = set()
    for c in trenches:
        if c["name"] in seen:
            continue
        seen.add(c["name"])
        arrow(c["top"][0], c["top"][1], 9)
    for c in hatches:
        arrow(c["top"][0], c["top"][1], 6)
    open(bmp, "wb").write(bytes(d))
    print("radar: %d trench and %d hatch marks, %d pixels outside the ring cleared" % (len(seen), len(hatches), masked))


if __name__ == "__main__":
    main()
