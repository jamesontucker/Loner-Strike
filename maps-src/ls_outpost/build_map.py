#!/usr/bin/env python3
"""ls_outpost (v2): a walled desert outpost at night, built for Last Stand.

Writes ls_outpost.map (Valve 220, what our map editor reads and writes) next to this file.
    python3 build_map.py
Open the .map in Tools > Map editor (Import) to edit it by hand, or change the tables here and
run it again. GoldSrc units, x east, y north, z up. README.md has the layout and the reasons.

The idea in one look:
  the compound    walls 160 high (32 thick) round a 1024 x 896 courtyard. A firing step (z 112)
                  runs inside the west, south and east walls and over the three gates
                  (gatehouses), reached by two stairs. Keep (last stand) on the north wall,
                  armory with Barney's counter in the NE corner, generator and mast NW.
  the gates       west, east, south: 128 wide x 96 high, each closed by a boarded barricade
                  you can see and shoot through (planks + one CLIP brush in the same entity)
  the routes      west: the open field (2 storm drains, floodlight mast, supply drops by the wall)
                  east: the alley (2 bends, red warning lamps, a crashed truck)
                  south: the canyon road (2 bends, a burning truck, orange light)
  the creatures   come out of storm drains (culverts) at the far end of each route
Textures only from cs_dust.wad, cstrike.wad (CS 1.6) and halflife.wad (every HL install).
"""
import os

WADS = ["cs_dust.wad", "cstrike.wad", "halflife.wad"]
SKY_Z = 640
WORLD = (-2112, -2048, 2240, 1152)   # inner bounds x0 y0 x1 y1
ROCK_H = 320                          # the canyon walls round everything
CLIP_TOP = 400

# compound
CX0, CY0, CX1, CY1 = -544, -480, 544, 480     # outer face of the walls
WT = 32                                        # wall thickness
IX0, IY0, IX1, IY1 = CX0 + WT, CY0 + WT, CX1 - WT, CY1 - WT   # courtyard inner face
WALL_H = 160                                   # wall top (parapet top)
STEP_Z = 112                                   # firing step floor
STEP_D = 96                                    # firing step depth
GATE_W, GATE_H = 128, 96

# ---------------------------------------------------------------------------------------
# open ground: (name, x0, y0, x1, y1, floor texture). Everything else is canyon rock.
OPEN = [
    ("compound", CX0, CY0, CX1, CY1, "SandRoad"),
    # west: the field, its two storm drains and the supply-drop ground by the wall
    ("field", -1536, -576, CX0, 576, "-0Sand"),
    ("drain_nw", -1536, 576, -1280, 960, "TNNL_FLR1"),
    ("drain_sw", -1536, -960, -1280, -576, "TNNL_FLR1"),
    # east: the alley
    ("apron_e", CX1, -192, 832, 192, "OUT_PAVE1"),
    ("alley1", 832, -80, 1216, 80, "OUT_PAVE1"),
    ("alley2", 1056, 80, 1216, 640, "OUT_PAVE1"),
    ("alley3", 1216, 480, 1728, 640, "OUT_PAVE1"),
    ("drain_e", 1728, 400, 2048, 720, "TNNL_FLR1"),
    ("bev_a1", 928, 80, 1056, 208, "OUT_PAVE1"),
    ("bev_a2", 1216, 352, 1344, 480, "OUT_PAVE1"),
    # south: the canyon road
    ("apron_s", -192, -736, 192, CY0, "SandRoad"),
    ("road1", -112, -1088, 112, -736, "SandRoad"),
    ("road2", 112, -1088, 768, -928, "SandRoad"),
    ("road3", 608, -1536, 768, -1088, "SandRoad"),
    ("drain_s", 528, -1856, 848, -1536, "TNNL_FLR1"),
    ("bev_r1", 112, -928, 240, -800, "SandRoad"),
    ("bev_r2", 480, -1216, 608, -1088, "SandRoad"),
]
# 45-degree wedges in each turn's inside corner (triangle counter-clockwise from above)
BEVELS = [
    [(928, 80), (1056, 208), (928, 208)],
    [(1216, 352), (1344, 352), (1344, 480)],
    [(112, -800), (240, -928), (240, -800)],
    [(480, -1088), (480, -1216), (608, -1216)],
]
# the storm drains' roofs (the creatures come out of the dark): x0 y0 x1 y1
DRAINS = [(-1536, 640, -1280, 960), (-1536, -960, -1280, -640), (1792, 400, 2048, 720), (528, -1856, 848, -1600)]
DRAIN_Z = 176

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
        self.faces = faces   # (p0, p1, p2, tex, normal, scale)

    def text(self):
        out = ["{"]
        for p0, p1, p2, tex, n, scale in self.faces:
            u, v = tex_axes(n)
            pts = " ".join("( %s %s %s )" % tuple(fmt(c) for c in p) for p in (p0, p1, p2))
            out.append("%s %s [ %d %d %d 0 ] [ %d %d %d 0 ] 0 %s %s" % (pts, tex, *u, *v, fmt(scale), fmt(scale)))
        out.append("}")
        return "\n".join(out)


def box(x0, y0, z0, x1, y1, z1, side, top=None, bottom=None, scale=1, hidden=()):
    """an axis-aligned box: side / top / bottom textures; faces whose normal is in `hidden` get NULL
    (faces that touch or sit inside other brushes: nothing drawn there, nothing to flicker)"""
    assert x1 > x0 and y1 > y0 and z1 > z0, (x0, y0, z0, x1, y1, z1)
    top = top or side
    bottom = bottom or top
    faces = []
    for n, (a, b) in TANG.items():
        p1 = (x1 if n[0] > 0 else x0, y1 if n[1] > 0 else y0, z1 if n[2] > 0 else z0)
        p0 = tuple(p1[i] + 64 * a[i] for i in range(3))
        p2 = tuple(p1[i] + 64 * b[i] for i in range(3))
        t = top if n == (0, 0, 1) else bottom if n == (0, 0, -1) else side
        if n in hidden:
            t = "NULL"
        faces.append((p0, p1, p2, t, n, scale))
    return Brush(faces)


def prism(poly, z0, z1, side, top=None, scale=1):
    """a vertical prism over a convex polygon given counter-clockwise from above"""
    top = top or side
    faces = []
    for (ax, ay), (bx, by) in zip(poly, poly[1:] + poly[:1]):
        ex, ey = bx - ax, by - ay
        ln = (ex * ex + ey * ey) ** 0.5
        faces.append(((bx, by, z0), (ax, ay, z0), (ax, ay, z0 + 64), side, (ey / ln, -ex / ln, 0), scale))
    for n, z in (((0, 0, 1), z1), ((0, 0, -1), z0)):
        a, b = TANG[n]
        p1 = (poly[0][0], poly[0][1], z)
        faces.append((tuple(p1[i] + 64 * a[i] for i in range(3)), p1, tuple(p1[i] + 64 * b[i] for i in range(3)), top, n, scale))
    return Brush(faces)


class Ent:
    def __init__(self, classname, **keys):
        self.keys = {"classname": classname}
        self.keys.update({k: str(v) for k, v in keys.items()})
        self.brushes = []

    def text(self, i):
        out = ["// entity %d" % i, "{"]
        out += ['"%s" "%s"' % kv for kv in self.keys.items()]
        for bi, b in enumerate(self.brushes):
            out.append("// brush %d" % bi)
            out.append(b.text())
        out.append("}")
        return "\n".join(out)


world = Ent("worldspawn", mapversion=220, wad=";".join(WADS), skyname="night",
            message="Outpost (Last Stand)", MaxRange=4096)
ents = [world]
detail = Ent("func_detail", zhlt_detaillevel=1)   # props and furniture: no VIS cuts
ents.append(detail)


def W(b):
    world.brushes.append(b)


def D(b):
    detail.brushes.append(b)


def point(classname, x, y, z, **keys):
    e = Ent(classname, origin="%d %d %d" % (x, y, z), **keys)
    ents.append(e)
    return e


def brush_ent(classname, brushes, **keys):
    e = Ent(classname, **keys)
    e.brushes += brushes
    ents.append(e)
    return e

# ---------------------------------------------------------------------------------------
# sky box
x0, y0, x1, y1 = WORLD
T = 64
for b in [
    (x0 - T, y0 - T, SKY_Z, x1 + T, y1 + T, SKY_Z + T),
    (x0 - T, y0 - T, -128 - T, x1 + T, y1 + T, -128),
    (x0 - T, y0 - T, -128, x0, y1 + T, SKY_Z),
    (x1, y0 - T, -128, x1 + T, y1 + T, SKY_Z),
    (x0, y0 - T, -128, x1, y0, SKY_Z),
    (x0, y1, -128, x1, y1 + T, SKY_Z),
]:
    W(box(*b, "SKY"))

# ---------------------------------------------------------------------------------------
# canyon rock: the complement of OPEN, cut on a grid, merged into rectangles


def inside(r, x, y):
    return r[0] <= x < r[2] and r[1] <= y < r[3]


xs = sorted({x0, x1} | {r[1] for r in OPEN} | {r[3] for r in OPEN})
ys = sorted({y0, y1} | {r[2] for r in OPEN} | {r[4] for r in OPEN})


def cell_kind(cx, cy):
    for r in OPEN:
        if inside(r[1:5], cx, cy):
            return ("open", r[5])
    return ("rock",)


rows = []
for j in range(len(ys) - 1):
    cy = (ys[j] + ys[j + 1]) / 2
    runs, i = [], 0
    while i < len(xs) - 1:
        k = cell_kind((xs[i] + xs[i + 1]) / 2, cy)
        s = i
        while i + 1 < len(xs) - 1 and cell_kind((xs[i + 1] + xs[i + 2]) / 2, cy) == k:
            i += 1
        runs.append((xs[s], xs[i + 1], k))
        i += 1
    rows.append(runs)
active, done = {}, []
for j, runs in enumerate(rows):
    for key in [k for k in active if k not in set(runs)]:
        done.append((key, active.pop(key), j))
    for key in runs:
        active.setdefault(key, j)
done += [(key, s, len(rows)) for key, s in active.items()]
for (ax0, ax1, k), j0, j1 in done:
    if k[0] == "open":
        W(box(ax0, ys[j0], -128, ax1, ys[j1], 0, "-0csSandWall", top=k[1]))
    else:
        W(box(ax0, ys[j0], -128, ax1, ys[j1], ROCK_H, "-0SandRock", top="-1Sand"))

for poly in BEVELS:
    W(prism(poly, 0, ROCK_H, "-0SandRock", "-1Sand"))

# storm drains: a concrete roof over the last stretch, a lintel at the mouth
for dx0, dy0, dx1, dy1 in DRAINS:
    W(box(dx0, dy0, DRAIN_Z, dx1, dy1, ROCK_H, "SandCCrete", bottom="TNNL_C1"))

# ---------------------------------------------------------------------------------------
# the compound walls, gates and the firing step
WALL = "csSandWall2"
STEP_TOP = "OUT_PAVE2"


SILL = STEP_Z + 24        # crenel sill: half the parapet (shoot down through the gaps, duck behind the merlons)
CRENEL = 48               # gap width; one gap every ~112 units


def wall(xa, ya, xb, yb, z0=0, z1=WALL_H, clip=True, crenel=True):
    """a wall segment. Along the firing step its parapet is crenellated: the wall runs up to SILL, then
    merlons up to WALL_H with CRENEL-wide gaps between them. CLIP from the sill up keeps everyone inside
    (bullets pass CLIP). Merlons are world brushes on the wall, so the compiler merges the touching faces."""
    if not (crenel and z1 == WALL_H):
        W(box(xa, ya, z0, xb, yb, z1, WALL, top="SandTrim"))
        if clip and z1 == WALL_H:
            W(box(xa, ya, WALL_H, xb, yb, CLIP_TOP, "CLIP"))
        return
    W(box(xa, ya, z0, xb, yb, SILL, WALL, top="SandTrim"))
    along_x = (xb - xa) >= (yb - ya)
    a, b = (xa, xb) if along_x else (ya, yb)
    n = int(round((b - a) / 112.0))
    cuts = []
    for i in range(n):
        c = a + (b - a) * (i + 0.5) / n
        cuts.append((c - CRENEL / 2, c + CRENEL / 2))
    edges = [a] + [v for cut in cuts for v in cut] + [b]
    for m0, m1 in zip(edges[0::2], edges[1::2]):
        if m1 - m0 < 1:
            continue
        W(box(m0, ya, SILL, m1, yb, WALL_H, WALL, top="SandTrim") if along_x else box(xa, m0, SILL, xb, m1, WALL_H, WALL, top="SandTrim"))
    if clip:
        W(box(xa, ya, SILL, xb, yb, CLIP_TOP, "CLIP"))


g = GATE_W // 2
# north wall (the keep and armory back onto it)
wall(CX0, IY1, CX1, CY1, crenel=False)
# south wall, gate at x -64..64
wall(CX0, CY0, -g, IY0)
wall(g, CY0, CX1, IY0)
wall(-g, CY0, g, IY0, z0=GATE_H)
# west wall, gate at y -64..64, a gap in the parapet at the ladder (y 256..320)
LAD0, LAD1 = 256, 320
wall(CX0, IY0, IX0, -g)
wall(CX0, g, IX0, LAD0)
wall(CX0, LAD0, IX0, LAD1, z1=STEP_Z, clip=False)
wall(CX0, LAD1, IX0, IY1)
wall(CX0, -g, IX0, g, z0=GATE_H)
# east wall, gate at y -64..64
wall(IX1, IY0, CX1, -g)
wall(IX1, g, CX1, IY1)
wall(IX1, -g, CX1, g, z0=GATE_H)


def step(xa, ya, xb, yb, z0=0):
    W(box(xa, ya, z0, xb, yb, STEP_Z, "SandCCrete", top=STEP_TOP))


# firing step: west and east full length, south between them; over each gate a slab (gatehouse)
SX0, SX1, SY0 = IX0 + STEP_D, IX1 - STEP_D, IY0 + STEP_D
step(IX0, IY0, SX0, -g)
step(IX0, g, SX0, IY1)
step(IX0, -g, SX0, g, z0=GATE_H)
step(SX1, IY0, IX1, -g)
step(SX1, g, IX1, IY1)
step(SX1, -g, IX1, g, z0=GATE_H)
step(SX0, IY0, -g, SY0)
step(g, IY0, SX1, SY0)
step(-g, IY0, g, SY0, z0=GATE_H)
# gate tunnels: a lamp over each inner mouth
for (lx0, ly0, lx1, ly1) in [(SX0, -24, SX0 + 4, 24), (SX1 - 4, -24, SX1, 24), (-24, SY0, 24, SY0 + 4)]:
    D(box(lx0, ly0, 100, lx1, ly1, 108, "~LIGHT3A"))


def stairs(xa, ya, xb, yb, z0, z1, toward, side="SandCCrete", top="OUT_PAVE1"):
    """steps filling the box, rising from z0 to z1 toward 'n', 's', 'e' or 'w'"""
    n = int(round((z1 - z0) / 16))
    length = (yb - ya) if toward in "ns" else (xb - xa)
    d = length / n
    for i in range(n):
        h = z0 + 16 * (i + 1)
        b = {"n": (xa, ya + d * i, z0, xb, yb, h), "s": (xa, ya, z0, xb, yb - d * i, h),
             "e": (xa + d * i, ya, z0, xb, yb, h), "w": (xa, ya, z0, xb - d * i, yb, h)}[toward]
        D(box(*b, side, top=top))


# two stairs up to the firing step, from the courtyard's south half
stairs(SX0, -256, SX0 + 224, -160, 0, STEP_Z, "w")
stairs(SX1 - 224, -256, SX1, -160, 0, STEP_Z, "e")

# the ladder down to the field (supply drops) and back up. Creature-proof by geometry: the ladder
# starts 76 up (over the head of anything standing on the ground) above a 40-high step that players
# hop onto and creatures can't climb (they step 18). Ladder art 1 unit off its metal backing.
LSTEP = 40
D(box(CX0 - 40, LAD0 + 8, 0, CX0, LAD1 - 8, LSTEP, "MltryCrteSd", top="MltryCrteTp", hidden=((1, 0, 0), (0, 0, -1))))
D(box(CX0 - 4, LAD0 + 8, LSTEP, CX0, LAD1 - 8, STEP_Z, "CSTRIKE_ME4METL", hidden=((1, 0, 0), (0, 0, -1))))
brush_ent("func_wall", [box(CX0 - 7, LAD0 + 12, LSTEP + 8, CX0 - 5, LAD1 - 12, STEP_Z - 2, "{CSTRIKE_LE6LAD")], rendermode=4, renderamt=255)
brush_ent("func_ladder", [box(CX0 - 24, LAD0 + 8, 76, CX0 - 8, LAD1 - 8, STEP_Z + 24, "AAATRIGGER")])
D(box(CX0 - 6, LAD0 + 4, STEP_Z + 4, CX0, LAD0 + 8, STEP_Z + 40, "+0~LAB1_CAU"))
D(box(CX0 - 6, LAD1 - 8, STEP_Z + 4, CX0, LAD1 - 4, STEP_Z + 40, "+0~LAB1_CAU"))

# ---------------------------------------------------------------------------------------
# the keep: the last stand. One wide door and two windows onto the courtyard
KX0, KX1, KY0 = -224, 224, 192
KH = 144
KW = 16
K = "SandCCrete"
# south face: door in the middle (128 x 112), a window each side (64 wide, z 48..96)
for xa, xb in [(KX0, -176), (-112, -64), (64, 112), (176, KX1)]:
    D(box(xa, KY0, 0, xb, KY0 + KW, KH, K))
for xa, xb in [(-176, -112), (112, 176)]:
    D(box(xa, KY0, 0, xb, KY0 + KW, 48, K))
    D(box(xa, KY0, 96, xb, KY0 + KW, KH, K))
D(box(-64, KY0, 112, 64, KY0 + KW, KH, K))                   # over the door
D(box(KX0, KY0 + KW, 0, KX0 + KW, IY1, KH, K))               # west face
D(box(KX1 - KW, KY0 + KW, 0, KX1, IY1, KH, K))               # east face
W(box(KX0, KY0, KH, KX1, IY1, WALL_H, "OUT_ROOF2B", bottom="CSTRIKE_ME1CLNG"))   # roof
W(box(KX0, KY0, WALL_H, KX1, IY1, CLIP_TOP, "CLIP"))
# inside: radio desks along the back wall, crates to fight from
D(box(-176, IY1 - 40, 0, -48, IY1, 40, "~LAB1_COMP3", top="CSTRIKE_ME4METL"))
D(box(48, IY1 - 40, 0, 176, IY1, 40, "~LAB1_COMP3", top="CSTRIKE_ME4METL"))
D(box(-176, 272, 0, -128, 320, 48, "MltryCrteSd", top="MltryCrteTp"))
D(box(128, 272, 0, 176, 320, 48, "MltryCrteSd", top="MltryCrteTp"))
# a sign light over the door
D(box(-48, KY0 - 4, 116, 48, KY0, 124, "~LIGHT5A"))

# ---------------------------------------------------------------------------------------
# the armory (NE corner, by the east gate): Barney's counter, the only buy zone
AX0, AX1, AY0 = KX1, SX1, 256
AH = 128
A = "CSTRIKE_WB6PLN2"
D(box(AX0, AY0, 0, 264, AY0 + KW, AH, A))                    # south face, west of the door
D(box(392, AY0, 0, AX1, AY0 + KW, AH, A))
D(box(264, AY0, 112, 392, AY0 + KW, AH, A))
D(box(AX0, AY0 + KW, 0, AX0 + KW, IY1, AH, A))               # west face
D(box(AX1 - 16, AY0 + KW, STEP_Z, AX1, IY1, AH, A))          # east face above the firing step
W(box(AX0, AY0, AH, AX1, IY1, AH + 16, "OUT_ROOF3B", bottom="CSTRIKE_CE2XCNG"))
W(box(AX0, AY0, AH + 16, AX1, IY1, CLIP_TOP, "CLIP"))
# the counter, lockers behind it, ammo crates
# the counter runs wall to wall; clip over it keeps the back for Barney (you buy across it)
D(box(AX0 + KW, 352, 0, AX1, 368, 40, "CSTRIKE_ME4METL", top="CSTRIKE_FE2SQRS"))
D(box(AX0 + KW, 352, 40, AX1, 368, AH, "CLIP"))
for xx in range(264, 392, 64):
    D(box(xx, IY1 - 24, 0, xx + 56, IY1, 112, "C1A1_LOCKER"))
D(box(AX0 + KW, 288, 0, AX0 + KW + 48, 336, 48, "MltryCrteSd", top="MltryCrteTp"))
# caution stripes either side of the door, a light strip over it, a lamp inside
for bx in (248, 392):
    D(box(bx, AY0 - 4, 0, bx + 16, AY0, 112, "+0~LAB1_CAU"))
D(box(264, AY0 - 4, 116, 392, AY0, 124, "~LIGHT5A"))
D(box(304, 384, AH - 4, 368, 416, AH, "~LIGHT3A"))
brush_ent("func_buyzone", [box(AX0 + KW, AY0 + KW, 0, AX1, IY1, AH, "AAATRIGGER")], team=2)
point("info_target", 336, 396, 16, targetname="lst_shop")   # behind the counter

# ---------------------------------------------------------------------------------------
# NW corner: the generator and the radio mast (a landmark seen from every lane)
D(box(SX0, 320, 0, -288, IY1, 72, "OUT_GAD1", top="CSTRIKE_ME4METL"))
D(box(-336, 216, 0, -320, 232, 448, "CSTRIKE_ME4METL"))
D(box(-344, 208, 448, -312, 240, 456, "~SPOTRED"))

# courtyard cover: sandbags facing each gate (64+ round them), a few crates
SB, SBT = "OUT_SNBAG", "OUT_SNBAGB"
D(box(SX0 + 64, -96, 0, SX0 + 96, 96, 48, SB, top=SBT))
D(box(SX1 - 96, -96, 0, SX1 - 64, 96, 48, SB, top=SBT))
D(box(-96, SY0 + 96, 0, 96, SY0 + 128, 48, SB, top=SBT))
D(box(-288, 64, 0, -224, 128, 64, "MltryCrteSd", top="MltryCrteTp"))

# ---------------------------------------------------------------------------------------
# barricades: boards with gaps you see and shoot through + one CLIP brush across the opening


def barricade(name, ident, axis, out, inward, lo, hi):
    """W1's format (notes: "barricades solid to everyone"): the ls_barricade entity is the CLIP brush
    filling the opening (+ a thin top rail) - solid to everything that moves, bullets and sight pass;
    the visible boards are separate func_illusionary brushes named <ident>_boards (boards=), never solid
    (the nav learns through, bullets pass), hidden one by one as it weakens.
    axis 'x': the opening spans y lo..hi; `out` is the wall's outer face (x), `inward` +1/-1 toward the
    courtyard. Nothing here shares a visible plane with anything else (no z-fighting): the frame is
    recessed 4 units into the opening, the boards sit 3 units inside the frame's faces, the rail stops
    1 unit under the lintel, and every face that touches a jamb, the floor, the lintel or a post is NULL."""
    f0 = out + 4 * inward                       # the frame: 12 deep, starting 4 inside the outer face
    f1 = f0 + 12 * inward
    fa, fb = min(f0, f1), max(f0, f1)

    def slab(d0, d1, z0, z1, a0, a1, tex, top, hidden=()):
        if axis == "x":
            return box(d0, a0, z0, d1, a1, z1, tex, top=top, hidden=hidden)
        return box(a0, d0, z0, a1, d1, z1, tex, top=top, hidden=hidden)
    lo_n = (0, -1, 0) if axis == "x" else (-1, 0, 0)     # the face toward the lo jamb
    hi_n = (0, 1, 0) if axis == "x" else (1, 0, 0)
    up, down = (0, 0, 1), (0, 0, -1)
    # the frame: two posts against the jambs, floor to lintel, always there
    D(slab(fa, fb, 0, GATE_H, lo, lo + 12, "TNNL_BRD1", "TNNL_BRD1B", hidden=(lo_n, up, down)))
    D(slab(fa, fb, 0, GATE_H, hi - 12, hi, "TNNL_BRD1", "TNNL_BRD1B", hidden=(hi_n, up, down)))
    # four boards 10 high with 16-unit gaps, 6 thick, centred in the frame; their ends run 4 into the posts
    for z0, z1 in [(6, 16), (32, 42), (58, 68), (84, 94)]:
        brush_ent("func_illusionary", [slab(fa + 3, fb - 3, z0, z1, lo + 8, hi - 8, "CRATE27", "TNNL_BRD1B",
                                            hidden=(lo_n, hi_n))], targetname=ident + "_boards")
    # the blocker (CLIP, never drawn) and a rail just under the lintel, both between the posts
    brush_ent("ls_barricade", [slab(fa, fb, 0, GATE_H, lo + 12, hi - 12, "CLIP", "CLIP"),
                               slab(fa + 2, fb - 2, GATE_H - 6, GATE_H - 1, lo + 10, hi - 10, "TNNL_BRD1B", "TNNL_BRD1B",
                                    hidden=(lo_n, hi_n))],
              targetname=name, name=name, team=2, hp=800, repair_secs=3, repair_cost=100, blocks_bots=1,
              boards=ident + "_boards", see_through=1)


barricade("West gate", "west", "x", CX0, 1, -g, g)
barricade("East gate", "east", "x", CX1, -1, -g, g)
barricade("South gate", "south", "y", CY0, 1, -g, g)

# ---------------------------------------------------------------------------------------
# the routes: cover and landmarks

# field: the floodlight mast, a burnt-out jeep, sandbag lines, crates (all 96+ apart)
D(box(-1104, -272, 0, -1088, -256, 352, "CSTRIKE_ME4METL"))
D(box(-1128, -296, 352, -1064, -232, 368, "~LIGHT3A", top="CSTRIKE_ME4METL"))
D(box(-928, -224, 0, -736, -128, 72, "OUT_DMP1A", top="OUT_DMPLID"))
D(box(-1280, 96, 0, -1088, 128, 48, SB, top=SBT))
D(box(-1344, -352, 0, -1216, -224, 96, "SandCrtLrgSd", top="SandCrtLrgTp"))
D(box(-1248, 320, 0, -1184, 384, 64, "MltryCrteSd", top="MltryCrteTp"))
D(box(-864, 416, 0, -832, 512, 48, SB, top=SBT))
D(box(-1440, 0, 0, -1376, 64, 64, "MltryCrteSd", top="MltryCrteTp"))
# alley: the crashed truck in the first bend's mouth (leaves 96 to pass), warning lamps
D(box(1120, -80, 0, 1208, -8, 88, "OUT_DMP2A", top="OUT_DMPLID"))
for lx, ly in [(1068, 300), (1300, 628), (1600, 492)]:
    D(box(lx - 8, ly - 4, 112, lx + 8, ly + 4, 128, "~SPOTRED"))
# road: the burning truck at the first bend, barrels
D(box(240, -1088, 0, 432, -1024, 80, "OUT_DMP3A", top="OUT_DMPLID"))
D(box(608, -1360, 0, 648, -1320, 48, "OUT_TNK1", top="CSTRIKE_ME4METL"))

# ---------------------------------------------------------------------------------------
# entities

# 16 CT spawns in the courtyard
for sy in (-128, -32, 64, 160):
    for sx in (-144, -48, 48, 144):
        point("info_player_start", sx, sy, 36, angles="0 %d 0" % (270 if sy > 0 else 90))
# T spawns in the drains (the game needs some)
for tx, ty in [(-1408, 864), (-1408, -864), (1952, 560), (688, -1760), (-1472, 800), (-1344, -800), (1888, 480), (608, -1696)]:
    point("info_player_deathmatch", tx, ty, 36, angles="0 0 0")

wave = dict(team=1, radius=96, rate=1.2)
point("ls_wave_spawner", -1408, 832, 8, name="Field north drain", **wave)
point("ls_wave_spawner", -1408, -832, 8, name="Field south drain", **wave)
point("ls_wave_spawner", 1920, 560, 8, name="Alley drain", **wave)
point("ls_wave_spawner", 688, -1728, 8, name="Road drain", **wave)

# supply drops: the open field just outside the west wall, by the ladder
point("ls_supply_drop", -736, 384, 8, table="drop", fall_secs=12)
point("ls_supply_drop", -960, 256, 8, table="drop", fall_secs=12)
point("ls_supply_drop", -768, 160, 8, table="drop", fall_secs=12)

point("ls_loot_container", -1312, -128, 8, table="default", tier=2)
point("ls_loot_container", 1136, 520, 8, table="default", tier=3)
point("ls_loot_container", 480, -1008, 8, table="default", tier=2)
point("ls_loot_container", 0, 352, 8, table="gear", tier=2)

point("ls_weapon_spawner", -256, -32, 8, item="table:gear", tier=3, respawn_secs=45)
point("ls_weapon_spawner", 288, 32, 8, item="table:pistols", tier=2, respawn_secs=30)
point("ls_weapon_spawner", IX0 + 48, -320, STEP_Z + 8, item="table:snipers", tier=3, respawn_secs=60)
point("ls_weapon_spawner", -1152, 448, 8, item="table:rifles", tier=3, respawn_secs=60)

# the base for the overrun rule: the courtyard (an area that does nothing while off)
point("ls_damage_zone", 0, 0, 0, targetname="lst_base", name="Base", enabled=0, show=0, dmg=0,
      size="%d %d 256" % (CX1 - CX0, CY1 - CY0))

# light: a dim cold moon; warm lamps in the base; one colour per route
point("light_environment", 0, 0, 512, pitch=-50, angles="0 210 0", _light="96 116 170 60", _diffuse_light="40 50 90 30")
warm = "255 186 112 240"
for lx, ly, lz in [(SX0 + 16, 0, 96), (SX1 - 16, 0, 96), (0, SY0 + 16, 96), (0, 0, 220), (-288, -288, 200),
                   (288, -288, 200), (0, 176, 120), (-336, 280, 140)]:
    point("light", lx, ly, lz, _light=warm)
point("light", 0, 320, 120, _light="200 255 200 140")            # keep: the radio screens
point("light", 336, 320, 116, _light="255 240 210 320")          # armory: bright
point("light", 328, 236, 112, _light="255 210 90 180")           # over the armory door
point("light", -328, 224, 460, _light="255 40 30 80")            # mast beacon
# field: white-blue floodlight
for lx, ly, lz in [(-1096, -264, 340), (-760, 300, 200), (-1300, 300, 220), (-1300, -400, 220)]:
    point("light", lx, ly, lz, _light="190 210 255 300" if lz > 300 else "150 170 230 110")
# alley: red warning lamps
for lx, ly, lz in [(1068, 300, 120), (1300, 620, 120), (1600, 500, 120), (900, 0, 160), (1140, 120, 140)]:
    point("light", lx, ly, lz, _light="255 50 30 140")
# road: the burning truck, orange
for lx, ly, lz in [(336, -1040, 110), (0, -900, 180), (688, -1300, 160)]:
    point("light", lx, ly, lz, _light="255 140 40 %d" % (320 if lz == 110 else 120))
# drains: a faint green
for lx, ly in [(-1408, 800), (-1408, -800), (1920, 560), (688, -1700)]:
    point("light", lx, ly, 150, _light="90 140 70 70")

point("ambient_generic", 0, 0, 300, message="ambience/des_wind2.wav", health=3, spawnflags=1, pitch=100)
point("ambient_generic", -1000, 0, 64, message="ambience/crickets.wav", health=3, spawnflags=8, pitch=95)
point("ambient_generic", 336, -1040, 64, message="ambience/burning1.wav", health=5, spawnflags=4, pitch=100)
point("ambient_generic", 0, 360, 40, message="ambience/computalk1.wav", health=2, spawnflags=2, pitch=100)
for lx, ly in [(-1408, 864), (-1408, -864), (1952, 560), (688, -1760)]:
    point("ambient_generic", lx, ly, 96, message="ambience/drips.wav", health=4, spawnflags=4, pitch=100)

# ---------------------------------------------------------------------------------------
here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "ls_outpost.map"), "w") as f:
    f.write("// ls_outpost: generated by build_map.py (edit that, or this in the map editor)\n")
    for i, e in enumerate(ents):
        f.write(e.text(i) + "\n")
print("ls_outpost.map: %d world brushes, %d detail brushes, %d entities" % (len(world.brushes), len(detail.brushes), len(ents)))
