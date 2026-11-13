#!/usr/bin/env python3
"""ls_outpost: a desert relay station at night, built for Last Stand.

Writes ls_outpost.map (Valve 220, what our map editor reads and writes) next to this file.
    python3 build_map.py            # writes ls_outpost.map
Open the .map in Tools > Map editor (Import) to edit it by hand afterwards; or change the
layout here and run it again. Everything is in GoldSrc units, x east, y north, z up.

Layout (docs in README.md beside this file):
  base      the compound courtyard in the middle (-448..448 square), walls 128 high
            station building NW (fallback room, roof = overwatch), armory SW (the only
            buy zone), east gun platform, south rampart
  west      "the yard": wide open ground with cover, two spawn pockets around corners
  east      "the alley": a 128-wide twisting lane with a roofed tunnel stretch
  south     "the walkway": raised path over the rooftops, enters the base on the wall top
  north     the drop clearing: open sky, visible from the gate and the station roof
Textures: only cs_dust.wad, cstrike.wad (CS 1.6) and halflife.wad (every HL install).
"""
import os

WADS = ["cs_dust.wad", "cstrike.wad", "halflife.wad"]
SKY_Z = 640          # ceiling of the world (sky)
WORLD = (-2304, -2304, 2304, 1344)   # inner bounds x0 y0 x1 y1
WALL_H = 288         # default height of the solid "town" around the lanes
CLIP_TOP = 400       # invisible walls above low walls reach this high

# ---------------------------------------------------------------------------------------
# open ground: (name, x0, y0, x1, y1, floor texture). Everything else is filled solid.
OPEN = [
    # base
    ("courtyard", -448, -448, 448, 448, "SandRoad"),
    ("gate_w", -640, -96, -448, 96, "SandRoad"),
    ("gate_e", 448, -64, 640, 64, "SandRoad"),
    ("gate_n", -96, 448, 96, 640, "SandRoad"),
    ("gate_s", 256, -640, 448, -448, "OUT_PAVE2"),       # on the wall top (floor at 128)
    # north: the drop clearing
    ("clearing", -512, 640, 512, 1216, "-1Sand"),
    # west: the yard and its two spawn pockets
    ("yard", -1984, -640, -640, 640, "-0Sand"),
    ("nw_neck", -1984, 640, -1792, 896, "-2Sand"),
    ("nw_pocket", -2240, 896, -1792, 1152, "-2Sand"),
    ("sw_neck", -1984, -896, -1792, -640, "-2Sand"),
    ("sw_pocket", -2240, -1152, -1792, -896, "-2Sand"),
    # east: the alley
    ("alley1", 640, -64, 896, 64, "OUT_GRVL2"),
    ("alley2", 768, 64, 896, 640, "OUT_GRVL2"),
    ("alley3", 896, 512, 1408, 640, "OUT_GRVL2"),
    ("alley4", 1280, -512, 1408, 512, "OUT_GRVL2"),
    ("alley5", 1408, -512, 1984, -384, "OUT_GRVL2"),
    ("e_pocket", 1984, -768, 2240, -128, "-3Sand"),
    # south: the walkway (floor raised to 128) and its spawn yard
    ("walk3", 256, -1024, 448, -640, "OUT_PAVE2"),
    ("walk2", -128, -1216, 448, -1024, "OUT_PAVE2"),
    ("walk1", -128, -1600, 64, -1216, "OUT_PAVE2"),
    ("s_stairs", -128, -1856, 64, -1600, "OUT_PAVE2"),
    ("s_pocket", -320, -2240, 256, -1856, "-3Sand"),
    # corner bevels: the square at each turn's inside corner is opened, then half of it is
    # filled back with a 45-degree wedge (BEVELS below), so nobody grinds on a sharp corner
    ("bevel_w1", 64, -1344, 192, -1216, "OUT_PAVE2"),
    ("bevel_w2", 128, -1024, 256, -896, "OUT_PAVE2"),
    ("bevel_a1", 640, 64, 768, 192, "OUT_GRVL2"),
    ("bevel_a2", 896, 384, 1024, 512, "OUT_GRVL2"),
    ("bevel_a3", 1152, 384, 1280, 512, "OUT_GRVL2"),
    ("bevel_a4", 1408, -384, 1536, -256, "OUT_GRVL2"),
]

# the wedges: (triangle counter-clockwise seen from above, floor z, top z, side, top, clip above)
BEVELS = [
    ([(64, -1344), (192, -1344), (192, -1216)], 128, 176, "SandCCrete", "OUT_ROOF1B", True),
    ([(128, -1024), (256, -896), (128, -896)], 128, 176, "SandCCrete", "OUT_ROOF1B", True),
    ([(640, 64), (768, 192), (640, 192)], 0, 288, "-0csSandWall", "-0Sand", False),
    ([(896, 384), (1024, 384), (1024, 512)], 0, 288, "-0csSandWall", "-0Sand", False),
    ([(1152, 512), (1152, 384), (1280, 384)], 0, 288, "-0csSandWall", "-0Sand", False),
    ([(1408, -256), (1536, -384), (1536, -256)], 0, 288, "-0csSandWall", "-0Sand", False),
]

# raised floors inside open ground: (x0, y0, x1, y1, top z, top texture)
RAISED = [
    (256, -640, 448, -448, 128, "OUT_PAVE2"),    # south gate on the wall top
    (256, -448, 448, -288, 128, "OUT_PAVE2"),    # rampart inside the base
    (256, -1024, 448, -640, 128, "OUT_PAVE2"),
    (-128, -1216, 448, -1024, 128, "OUT_PAVE2"),
    (-128, -1600, 64, -1216, 128, "OUT_PAVE2"),
    (64, -1344, 192, -1216, 128, "OUT_PAVE2"),
    (128, -1024, 256, -896, 128, "OUT_PAVE2"),
]

# solid zones with their own height: (x0, y0, x1, y1, height, side tex, top tex, clip above)
ZONES = [
    (-640, -640, 640, -448, 176, "SandCCrete", "OUT_ROOF2B", True),   # base south wall: parapet for the wall-top gate
    (-640, -448, 640, 640, 128, "csSandWall2", "SandTrim", True),     # base walls
    (-640, -1856, 640, -640, 176, "SandCCrete", "OUT_ROOF1B", True),  # rooftops along the walkway
]
DEFAULT_SIDE, DEFAULT_TOP = "-0csSandWall", "-0Sand"

# ---------------------------------------------------------------------------------------
# brushes


def tex_axes(n):
    ax = max(range(3), key=lambda i: abs(n[i]))
    if ax == 2:
        return (1, 0, 0), (0, -1, 0)
    if ax == 0:
        return (0, 1, 0), (0, 0, -1)
    return (1, 0, 0), (0, 0, -1)


# for an outward normal n: two tangents a, b with a x b = n (Quake's plane from 3 points)
TANG = {
    (1, 0, 0): ((0, 1, 0), (0, 0, 1)), (-1, 0, 0): ((0, 0, 1), (0, 1, 0)),
    (0, 1, 0): ((0, 0, 1), (1, 0, 0)), (0, -1, 0): ((1, 0, 0), (0, 0, 1)),
    (0, 0, 1): ((1, 0, 0), (0, 1, 0)), (0, 0, -1): ((0, 1, 0), (1, 0, 0)),
}


def fmt(v):
    return ("%d" % v) if float(v).is_integer() else ("%.3f" % v)


class Brush:
    def __init__(self, faces):
        self.faces = faces   # list of (p0, p1, p2, tex, n)

    def text(self):
        out = ["{"]
        for p0, p1, p2, tex, n, scale in self.faces:
            u, v = tex_axes(n)
            pts = " ".join("( %s %s %s )" % tuple(fmt(c) for c in p) for p in (p0, p1, p2))
            out.append("%s %s [ %d %d %d 0 ] [ %d %d %d 0 ] 0 %s %s" % (pts, tex, *u, *v, fmt(scale), fmt(scale)))
        out.append("}")
        return "\n".join(out)


def box(x0, y0, z0, x1, y1, z1, side, top=None, bottom=None, scale=1):
    """an axis-aligned box; side/top/bottom textures (a dict per normal also works)"""
    assert x1 > x0 and y1 > y0 and z1 > z0, (x0, y0, z0, x1, y1, z1)
    top = top or side
    bottom = bottom or top
    faces = []
    for n in TANG:
        a, b = TANG[n]
        # a point on the face
        p = [x1 if n[0] > 0 else x0, y1 if n[1] > 0 else y0, z1 if n[2] > 0 else z0]
        if n[0] < 0: p[0] = x0
        if n[1] < 0: p[1] = y0
        if n[2] < 0: p[2] = z0
        p1 = tuple(p)
        p0 = tuple(p1[i] + 64 * a[i] for i in range(3))
        p2 = tuple(p1[i] + 64 * b[i] for i in range(3))
        t = top if n == (0, 0, 1) else bottom if n == (0, 0, -1) else side
        if isinstance(t, dict):
            t = t.get(n, t.get("side"))
        faces.append((p0, p1, p2, t, n, scale))
    return Brush(faces)


def prism(poly, z0, z1, side, top=None, scale=1):
    """a vertical prism over a convex polygon given counter-clockwise from above"""
    top = top or side
    faces = []
    for (ax, ay), (bx, by) in zip(poly, poly[1:] + poly[:1]):
        ex, ey = bx - ax, by - ay
        ln = (ex * ex + ey * ey) ** 0.5
        n = (ey / ln, -ex / ln, 0)
        p1 = (ax, ay, z0)
        faces.append(((bx, by, z0), p1, (ax, ay, z0 + 64), side, n, scale))
    for n, z, t in (((0, 0, 1), z1, top), ((0, 0, -1), z0, top)):
        a, b = TANG[n]
        x, y = poly[0]
        p1 = (x, y, z)
        faces.append((tuple(p1[i] + 64 * a[i] for i in range(3)), p1, tuple(p1[i] + 64 * b[i] for i in range(3)), t, n, scale))
    return Brush(faces)


class Ent:
    def __init__(self, classname, **keys):
        self.keys = {"classname": classname}
        self.keys.update({k: str(v) for k, v in keys.items()})
        self.brushes = []

    def text(self, i):
        out = ["// entity %d" % i, "{"]
        for k, v in self.keys.items():
            out.append('"%s" "%s"' % (k, v))
        for bi, b in enumerate(self.brushes):
            out.append("// brush %d" % bi)
            out.append(b.text())
        out.append("}")
        return "\n".join(out)


world = Ent("worldspawn", mapversion=220, wad=";".join(WADS), skyname="night",
            message="Outpost (Last Stand)", sounds=1, MaxRange=4096)
ents = [world]
detail = Ent("func_detail", zhlt_detaillevel=1)   # props, stairs, building: no VIS cuts
ents.append(detail)


def W(b):
    world.brushes.append(b)


def D(b):
    detail.brushes.append(b)


def point(classname, x, y, z, **keys):
    e = Ent(classname, origin="%d %d %d" % (x, y, z), **keys)
    ents.append(e)
    return e

# ---------------------------------------------------------------------------------------
# the sky box around everything
x0, y0, x1, y1 = WORLD
T = 64
for b in [
    (x0 - T, y0 - T, SKY_Z, x1 + T, y1 + T, SKY_Z + T),     # ceiling
    (x0 - T, y0 - T, -128 - T, x1 + T, y1 + T, -128),       # floor (under the fill)
    (x0 - T, y0 - T, -128, x0, y1 + T, SKY_Z),
    (x1, y0 - T, -128, x1 + T, y1 + T, SKY_Z),
    (x0, y0 - T, -128, x1, y0, SKY_Z),
    (x0, y1, -128, x1, y1 + T, SKY_Z),
]:
    W(box(*b, "SKY"))

# ---------------------------------------------------------------------------------------
# solid fill: the complement of OPEN, cut on a grid, merged into rectangles


def inside(r, x, y):
    return r[0] <= x < r[2] and r[1] <= y < r[3]


xs = sorted({x0, x1} | {r[1] for r in OPEN} | {r[3] for r in OPEN} | {z[0] for z in ZONES} | {z[2] for z in ZONES})
ys = sorted({y0, y1} | {r[2] for r in OPEN} | {r[4] for r in OPEN} | {z[1] for z in ZONES} | {z[3] for z in ZONES})
xs = [v for v in xs if x0 <= v <= x1]
ys = [v for v in ys if y0 <= v <= y1]


def cell_kind(cx, cy):
    for r in OPEN:
        if inside(r[1:5], cx, cy):
            return ("open", r[5])
    for z in ZONES:
        if inside(z, cx, cy):
            return ("solid", z[4], z[5], z[6], z[7])
    return ("solid", WALL_H, DEFAULT_SIDE, DEFAULT_TOP, False)


rows = []
for j in range(len(ys) - 1):
    cy = (ys[j] + ys[j + 1]) / 2
    runs = []
    i = 0
    while i < len(xs) - 1:
        k = cell_kind((xs[i] + xs[i + 1]) / 2, cy)
        s = i
        while i + 1 < len(xs) - 1 and cell_kind((xs[i + 1] + xs[i + 2]) / 2, cy) == k:
            i += 1
        runs.append((xs[s], xs[i + 1], k))
        i += 1
    rows.append(runs)

# merge identical runs over consecutive rows
active = {}
done = []
for j, runs in enumerate(rows):
    keys = set(runs)
    for key in list(active):
        if key not in keys:
            done.append((key, active.pop(key), j))
    for key in runs:
        if key not in active:
            active[key] = j
for key, s in active.items():
    done.append((key, s, len(rows)))

for (ax0, ax1, k), j0, j1 in done:
    by0, by1 = ys[j0], ys[j1]
    if k[0] == "open":
        W(box(ax0, by0, -128, ax1, by1, 0, "-0csSandWall", top=k[1]))
    else:
        h, side, top, clip = k[1:]
        W(box(ax0, by0, -128, ax1, by1, h, side, top=top))
        if clip:
            W(box(ax0, by0, h, ax1, by1, CLIP_TOP, "CLIP"))

# corner wedges
for poly, z0, z1, side, top, clip in BEVELS:
    W(prism(poly, z0, z1, side, top))
    if clip:
        W(prism(poly, z1, CLIP_TOP, "CLIP"))

# raised floors (walkway, wall-top gate, rampart)
for rx0, ry0, rx1, ry1, z, tex in RAISED:
    W(box(rx0, ry0, 0, rx1, ry1, z, "SandCCrete", top=tex))

# ---------------------------------------------------------------------------------------
# stairs: n steps of `rise` up along a direction, as detail


def stairs(xa, ya, xb, yb, z0, z1, toward, side="SandCCrete", top="OUT_PAVE1"):
    """steps filling the box xa..xb, ya..yb, rising from z0 to z1 toward 'n','s','e','w'"""
    n = int(round((z1 - z0) / 16))
    length = (yb - ya) if toward in "ns" else (xb - xa)
    d = length / n
    for i in range(n):
        h = z0 + 16 * (i + 1)
        if toward == "n":
            b = (xa, ya + d * i, z0, xb, yb, h)
        elif toward == "s":
            b = (xa, ya, z0, xb, yb - d * i, h)
        elif toward == "e":
            b = (xa + d * i, ya, z0, xb, yb, h)
        else:
            b = (xa, ya, z0, xb - d * i, yb, h)
        D(box(*b, side, top=top))


# south: spawn yard up to the walkway
stairs(-128, -1856, 64, -1600, 0, 128, "n")
# base: rampart down to the courtyard (west, along the south wall)
stairs(0, -448, 256, -288, 0, 128, "e")
# base: station roof stairs (south of the building, rising west)
stairs(-448, 96, -128, 192, 0, 160, "w")
# base: east gun platform and its stairs
D(box(320, 96, 0, 448, 352, 96, "CSTRIKE_ME4METL", top="CSTRIKE_FE2GTNG"))
stairs(128, 128, 320, 224, 0, 96, "e", side="CSTRIKE_ME4METL", top="CSTRIKE_FE2GTNG")

# ---------------------------------------------------------------------------------------
# the station building (NW of the courtyard): the fallback room, its roof is overwatch
SX0, SY0, SX1, SY1, SH = -448, 192, -128, 448, 160
WT = 16
ext = {"side": "SandCCrete"}
D(box(SX0, SY0, 0, SX1, SY0 + WT, SH - 16, "SandCCrete"))                    # south wall
D(box(SX1 - WT, SY0 + WT, 0, SX1, 256, SH - 16, "SandCCrete"))                # east wall, below the door
D(box(SX1 - WT, 384, 0, SX1, SY1, SH - 16, "SandCCrete"))                     # east wall, above the door
D(box(SX1 - WT, 256, 112, SX1, 384, SH - 16, "SandCCrete"))                   # over the door
D(box(SX0, SY0, SH - 16, SX1, SY1, SH, "SandCCrete", top="OUT_ROOF2B", bottom="CSTRIKE_ME1CLNG"))  # roof
# inside: a radio console and crates
D(box(-432, 400, 0, -304, 432, 40, "~LAB1_COMP3", top="CSTRIKE_ME4METL"))
D(box(-432, 208, 0, -368, 272, 64, "MltryCrteSd", top="MltryCrteTp"))
# roof parapet ledge (low, you shoot over it)
D(box(SX0, SY0, SH, SX1, SY0 + 8, SH + 24, "SandTrim"))
# relay mast on the roof (decor, out of the way in the corner)
D(box(-432, 416, SH, -400, 448, SH + 224, "CSTRIKE_ME4METL"))
D(box(-424, 420, SH + 224, -408, 444, SH + 232, "~SPOTRED"))

# ---------------------------------------------------------------------------------------
# the armory (SW of the courtyard): the only buy zone, two doorways
AX0, AY0, AX1, AY1, AH = -448, -448, -192, -224, 128
D(box(AX0, AY1 - WT, 0, -384, AY1, AH, "CSTRIKE_WB6PLN2"))            # north wall, west of the door
D(box(-256, AY1 - WT, 0, AX1, AY1, AH, "CSTRIKE_WB6PLN2"))            # north wall, east of the door
D(box(-384, AY1 - WT, 112, -256, AY1, AH, "CSTRIKE_WB6PLN2"))         # over the north door
D(box(AX1 - WT, AY0, 0, AX1, -400, AH, "CSTRIKE_WB6PLN2"))            # east wall, south of the door
D(box(AX1 - WT, -304, 0, AX1, AY1 - WT, AH, "CSTRIKE_WB6PLN2"))       # east wall, north of the door
D(box(AX1 - WT, -400, 112, AX1, -304, AH, "CSTRIKE_WB6PLN2"))         # over the east door
D(box(AX0, AY0, AH, AX1, AY1, AH + 16, "CSTRIKE_WB6PLN2", top="OUT_ROOF3B", bottom="CSTRIKE_CE2XCNG"))
# floor plate (thin, flush 1 unit up so it shows)
D(box(AX0, AY0, 0, AX1 - WT, AY1 - WT, 1, "CSTRIKE_FE2SQRS"))
# weapon racks: lockers along the west and south walls, an ammo bench
for i, yy in enumerate(range(-432, -272, 64)):
    D(box(AX0, yy, 1, AX0 + 24, yy + 56, 112, "C1A1_LOCKER"))
for xx in range(-416, -256, 64):
    D(box(xx, AY0, 1, xx + 56, AY0 + 24, 112, "C1A1_LOCKER"))
D(box(-336, -352, 1, -272, -320, 36, "MltryCrteSd", top="MltryCrteTp"))
# signs: caution stripes either side of both doors, a light strip over each
for bx in (-400, -256):
    D(box(bx, AY1, 0, bx + 16, AY1 + 4, 112, "+0~LAB1_CAU"))
for by in (-416, -304):
    D(box(AX1, by, 0, AX1 + 4, by + 16, 112, "+0~LAB1_CAU"))
D(box(-384, AY1, 116, -256, AY1 + 4, 124, "~LIGHT5A"))
D(box(AX1, -400, 116, AX1 + 4, -304, 124, "~LIGHT5A"))
D(box(-352, -352, AH - 4, -288, -320, AH, "~LIGHT3A"))
buy = Ent("func_buyzone", team=2)
buy.brushes.append(box(AX0 + 24, AY0 + 24, 0, AX1 - WT, AY1 - WT, AH, "AAATRIGGER"))
ents.append(buy)
# the shop marker for gm_laststand (W1's find_shop: an entity named lst_shop)
point("info_target", -320, -336, 16, targetname="lst_shop")

# ---------------------------------------------------------------------------------------
# cover


def crate(x, y, s=64, h=None, big=False, z=0):
    h = h or s
    if big:
        D(box(x, y, z, x + s, y + s, z + h, "SandCrtLrgSd", top="SandCrtLrgTp"))
    else:
        D(box(x, y, z, x + s, y + s, z + h, "MltryCrteSd", top="MltryCrteTp"))


def sandbags(xa, ya, xb, yb, h=48):
    D(box(xa, ya, 0, xb, yb, h, "OUT_SNBAG", top="OUT_SNBAGB"))


# the yard: staggered cover, wide lanes between (128+ everywhere)
crate(-896, -256, 128, big=True)
crate(-896, -128, 64)
sandbags(-1024, 240, -832, 272)
sandbags(-1088, -320, -1056, -160)
crate(-1280, 288, 128, big=True)
crate(-1216, 416, 64)
crate(-1344, -416, 128, big=True)
crate(-1280, -480, 64)
sandbags(-1536, -64, -1504, 128)
crate(-1600, 160, 64)
crate(-1664, -320, 128, big=True)
D(box(-1408, -176, 0, -1152, -144, 160, "-1csSandWall", top="SandTrim"))      # a broken wall
D(box(-1792, 320, 0, -1600, 352, 112, "SandCCrete", top="SandTrim"))
crate(-1856, -544, 64)
# a wrecked water tank on legs in the yard's middle (shoot under it, walk round it)
D(box(-1240, -8, 0, -1224, 8, 96, "CSTRIKE_ME4METL"))
D(box(-1240, 120, 0, -1224, 136, 96, "CSTRIKE_ME4METL"))
D(box(-1112, -8, 0, -1096, 8, 96, "CSTRIKE_ME4METL"))
D(box(-1112, 120, 0, -1096, 136, 96, "CSTRIKE_ME4METL"))
D(box(-1256, -24, 96, -1080, 152, 176, "-0CSTRIKE_WE1MT", top="CSTRIKE_ME4METL"))

# base: sandbag walls just inside each gate, crates for cover
sandbags(320, -224, 352, -96)                        # east gate, south side
crate(256, -160, 64)
crate(0, 320, 64)                                    # north gate
crate(96, 352, 64, h=48)

# alley: barrels/crates along the walls (leave 96+ free)
# (kept clear: 128 wide, nothing in the way)
# clearing: a few crates and a sandbag ring around the drop spot
crate(-448, 1088, 128, big=True)
crate(320, 704, 64)
crate(384, 1120, 64)
sandbags(-128, 960, -96, 1088)
sandbags(96, 832, 128, 960)

# walkway: low railing blocks at the bends (cover from the rooftops' direction)
D(box(-128, -1232, 128, -64, -1216, 160, "CSTRIKE_ME4METL"))
D(box(384, -1040, 128, 448, -1024, 160, "CSTRIKE_ME4METL"))

# tunnel roof on the alley's long south leg
W(box(1280, -192, 128, 1408, 320, WALL_H, "TNNL_W4", bottom="TNNL_C1"))
# tunnel mouth lights
D(box(1336, 316, 112, 1352, 320, 120, "~LIGHT3A"))
D(box(1336, -192, 112, 1352, -188, 120, "~LIGHT3A"))

# lamp fixtures on the base walls (warm)
for (lx, ly, nx, ny) in [(-448, -48, 1, 0), (448, 32, -1, 0), (-16, 448, 0, -1), (100, -448, 0, 1)]:
    if nx:
        D(box(lx if nx > 0 else lx - 8, ly, 104, lx + 8 if nx > 0 else lx, ly + 16, 120, "~LIGHT3A"))
    else:
        D(box(lx, ly if ny > 0 else ly - 8, 104, lx + 16, ly + 8 if ny > 0 else ly, 120, "~LIGHT3A"))

# ---------------------------------------------------------------------------------------
# barricades (gm_components' ls_barricade brushes, wall to wall at each choke)


def barricade(name, b):
    e = Ent("ls_barricade", targetname=name, name=name, team=2, hp=1000, repair_secs=3, repair_cost=100, blocks_bots=1)
    e.brushes.append(box(*b, "CRATE27", top="TNNL_BRD1B"))
    ents.append(e)


barricade("West gate", (-560, -96, 0, -544, 96, 96))
barricade("East gate", (544, -64, 0, 560, 64, 96))
barricade("Wall gate", (256, -560, 128, 448, -544, 224))

# ---------------------------------------------------------------------------------------
# entities: spawns, waves, drops, loot, weapons, light, sound

# 16 CT spawns in the courtyard (between the station, armory and the gates)
n = 0
for sy in (-224, -128, -32, 64):
    for sx in (-128, -32, 64, 160):
        point("info_player_start", sx, sy, 36, angles="0 %d 0" % (90 if sy < 0 else 270))
        n += 1
assert n >= 12
# T spawns in the pockets (the game needs some; zombie bots spawn here without creatures)
for (tx, ty) in [(-2112, 960), (-2112, 1088), (-2112, -960), (-2112, -1088), (2112, -640), (2112, -256),
                 (-192, -2112), (128, -2112)]:
    point("info_player_deathmatch", tx, ty, 36, angles="0 0 0")

wave = dict(team=1, radius=96, rate=1.2)
point("ls_wave_spawner", -2048, 1024, 8, name="Yard north", **wave)
point("ls_wave_spawner", -2048, -1024, 8, name="Yard south", **wave)
point("ls_wave_spawner", 2112, -448, 8, name="Alley", **wave)
point("ls_wave_spawner", -32, -2048, 8, name="Walkway", **wave)

point("ls_supply_drop", -224, 928, 8, table="drop", fall_secs=12)
point("ls_supply_drop", 224, 1024, 8, table="drop", fall_secs=12)
point("ls_supply_drop", 224, 288, 8, table="drop", fall_secs=12)

point("ls_loot_container", -1728, 480, 8, table="default", tier=2)
point("ls_loot_container", 2176, -192, 8, table="default", tier=3)
point("ls_loot_container", 352, -864, 136, table="default", tier=2)
point("ls_loot_container", -384, 704, 8, table="default", tier=2)
point("ls_loot_container", -1216, 64, 8, table="gear", tier=2)

point("ls_weapon_spawner", -96, 384, 8, item="table:gear", tier=3, respawn_secs=45)
point("ls_weapon_spawner", -368, 0, 8, item="table:pistols", tier=2, respawn_secs=30)
point("ls_weapon_spawner", 384, 224, 104, item="table:snipers", tier=3, respawn_secs=60)
point("ls_weapon_spawner", -1408, 384, 8, item="table:rifles", tier=3, respawn_secs=60)

# moonlight: cold, dim; the base gets warm lamps, the lanes only a few cold ones
point("light_environment", 0, 0, 512, pitch=-50, angles="0 210 0", _light="96 116 170 70", _diffuse_light="40 50 90 40")
warm = "255 186 112 260"
for (lx, ly, lz) in [(-432, -48, 112), (432, 32, 112), (-16, 432, 112), (108, -432, 112),
                     (0, 0, 200), (-288, 320, 128), (384, 224, 200), (352, -368, 200)]:
    point("light", lx, ly, lz, _light=warm)
point("light", -288, 320, 120, _light="255 210 160 120")              # inside the station
point("light", -320, -336, 116, _light="255 230 190 320")             # armory: bright
point("light", -320, -200, 112, _light="255 200 80 160")              # over the armory door
point("light", -176, -352, 112, _light="255 200 80 160")
cold = "150 170 220 120"
for (lx, ly, lz) in [(-1216, 64, 260), (-1700, -400, 200), (-1700, 500, 200), (832, 352, 200),
                     (1344, 400, 200), (1344, 64, 112), (1344, -320, 112), (1700, -448, 200),
                     (2112, -448, 200), (-2048, 1024, 160), (-2048, -1024, 160), (0, 960, 260),
                     (-32, -2048, 200), (-32, -1408, 260), (352, -832, 260), (160, -1120, 260)]:
    point("light", lx, ly, lz, _light=cold)

point("ambient_generic", 0, 0, 300, message="ambience/des_wind2.wav", health=3, spawnflags=1, pitch=100)
point("ambient_generic", 0, 960, 64, message="ambience/crickets.wav", health=4, spawnflags=4, pitch=100)
point("ambient_generic", -1300, 0, 64, message="ambience/crickets.wav", health=3, spawnflags=8, pitch=90)
point("ambient_generic", 1344, 64, 96, message="ambience/drips.wav", health=4, spawnflags=4, pitch=100)
point("ambient_generic", -320, 400, 40, message="ambience/computalk1.wav", health=2, spawnflags=2, pitch=100)

# ---------------------------------------------------------------------------------------
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "ls_outpost.map"), "w") as f:
    f.write("// ls_outpost: generated by build_map.py (edit that, or this in the map editor)\n")
    for i, e in enumerate(ents):
        f.write(e.text(i) + "\n")
print("ls_outpost.map: %d world brushes, %d detail brushes, %d entities" % (len(world.brushes), len(detail.brushes), len(ents)))
