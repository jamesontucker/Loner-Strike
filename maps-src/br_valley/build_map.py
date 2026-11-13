#!/usr/bin/env python3
"""br_valley: a Battle Royale map in a river valley, by day.

Writes br_valley.map (Valve 220, what our map editor reads and writes) next to this file, and
preview.png (a quick top-down of the layout, no lighting).
    python3 build_map.py
Open the .map in Tools > Map editor (Import) to edit it by hand; or change the tables here and
run it again. GoldSrc units, x east, y north, z up. Same brush/entity writer as ls_outpost's
build_map.py (Valve 220 faces from three points, world-aligned texture axes), extended with
convex prisms for sloped terrain.

Size (everything networked in GoldSrc is +/-4096, see docs/TOOLS-NOTES.md "BR map"):
  playable   -3584..3584 in x and y (7168 square, 28 x 28 cells of 256)
  backdrop   one ring of tall cliffs outside that (unreachable, clip wall at the play edge)
  vertical   tunnels at z -512..-320, surface -160..~600, sky ceiling 3584 (room for a drop-in)

Layers:
  terrain    a height grid (vertices every 256) drawn as triangle prisms; flat pads under the
             points of interest; slopes clamped to at most ~35 degrees; a river channel with
             cliff banks, three bridges, two fords and stairs out of the bed
  surface    town (multi-storey, rooftops), factory, farm, radio hill (tower, ladder),
             docks, gas station, quarry pit, bunker, lumber camp (watchtower, ladder),
             two mesas (cliffs, one ramp each), cabins, rocks and trees for cover
  below      a tunnel network: town sewer, a long tunnel under the river, the bunker complex;
             stair trenches and two ladder hatches lead down
Textures only from cs_dust.wad, cstrike.wad (CS 1.6) and halflife.wad; the valve `cliff` sky.
"""
import math
import os
import struct
import zlib

NAME = "br_valley"
WADS = ["cs_dust.wad", "cstrike.wad", "halflife.wad"]
CELL = 256
N = 30                      # cells per side (0 and 29 are the backdrop ring)
X0 = -CELL * N // 2         # -3840
PLAY = 3584                 # playable half-size
SKY_Z = 3584
FLOOR_Z = -768              # bottom of the underground slab
SLAB_TOP = -320             # terrain prisms start here; tunnel ceilings
TUN_Z = -512                # tunnel floor
BED_Z = -160                # river bed
MAXD = 112                  # largest height step between neighbouring vertices (256 apart)

GRASS, GRASS2, DIRT, ROAD, ASPH = "CSTRIKE_FT3GRAS", "-0OUT_GRSS1", "CSTRIKE_FT2DIRT", "OUT_GRVL2", "CSTRIKE_FT4APHT"
ROCK, ROCK2, MUD, SAND = "OUT_RK3", "-0OUT_RK4", "CSTRIKE_FT2MUD", "-1Sand"
CONC, CONC_F, CEIL = "CSTRIKE_WR4CCPN", "-0CRETE4_FLR01", "TNNL_C1"
TUNW = "TNNL_W4"

# ---------------------------------------------------------------------------------------
# brushes (as ls_outpost: Valve 220, world-aligned texture axes by the face's main axis)


def tex_axes(n):
    ax = max(range(3), key=lambda i: abs(n[i]))
    if ax == 2:
        return (1, 0, 0), (0, -1, 0)
    if ax == 0:
        return (0, 1, 0), (0, 0, -1)
    return (1, 0, 0), (0, 0, -1)


def fmt(v):
    v = round(v, 3)
    return ("%d" % v) if float(v).is_integer() else ("%.3f" % v)


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


class Brush:
    def __init__(self):
        self.faces = []   # (p0, p1, p2, tex, n, scale)

    def face(self, pts, n, tex, scale=1):
        """a plane through three of pts with outward normal n (order fixed up here)"""
        p0, p1, p2 = pts[0], pts[1], pts[2]
        c = cross(sub(p0, p1), sub(p2, p1))
        if dot(c, n) < 0:
            p0, p2 = p2, p0
        self.faces.append((p0, p1, p2, tex, n, scale))

    def text(self):
        out = ["{"]
        for p0, p1, p2, tex, n, scale in self.faces:
            u, v = tex_axes(n)
            pts = " ".join("( %s %s %s )" % tuple(fmt(c) for c in p) for p in (p0, p1, p2))
            out.append("%s %s [ %d %d %d 0 ] [ %d %d %d 0 ] 0 %s %s" % (pts, tex, *u, *v, fmt(scale), fmt(scale)))
        out.append("}")
        return "\n".join(out)


def prism(top, zb, toptex, side, bottom=None, scale=1, topscale=None):
    """a convex prism: top = convex polygon [(x,y,z)...] (any order of winding), straight sides
    down to the flat bottom zb"""
    b = Brush()
    cx = sum(p[0] for p in top) / len(top)
    cy = sum(p[1] for p in top) / len(top)
    n = cross(sub(top[1], top[0]), sub(top[2], top[0]))
    if n[2] < 0:
        n = (-n[0], -n[1], -n[2])
    b.face(top[:3], n, toptex, topscale or scale)
    b.face([(top[0][0], top[0][1], zb), (top[1][0], top[1][1], zb), (top[2][0], top[2][1], zb)], (0, 0, -1), bottom or side, scale)
    k = len(top)
    for i in range(k):
        p, q = top[i], top[(i + 1) % k]
        dx, dy = q[0] - p[0], q[1] - p[1]
        nn = (dy, -dx, 0)
        mx, my = (p[0] + q[0]) / 2 - cx, (p[1] + q[1]) / 2 - cy
        if nn[0] * mx + nn[1] * my < 0:
            nn = (-dy, dx, 0)
        b.face([p, q, (p[0], p[1], zb)], nn, side, scale)
    b.bounds = (min(p[0] for p in top), min(p[1] for p in top), zb,
                max(p[0] for p in top), max(p[1] for p in top), max(p[2] for p in top))
    b.poly = [(p[0], p[1]) for p in top]
    b.top3 = top[:3]
    return b


def box(x0, y0, z0, x1, y1, z1, side, top=None, bottom=None, scale=1):
    assert x1 > x0 and y1 > y0 and z1 > z0, (x0, y0, z0, x1, y1, z1)
    top = top or side
    b = prism([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], z0, top, side, bottom or top, scale)
    return b


def blob(x, y, r, n, seed, squash=1.0, turn=0.0):
    """an irregular convex polygon round x, y (rocks, foliage): n corners, radius about r"""
    pts = []
    for k in range(n):
        seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF
        jit = 0.82 + 0.3 * (seed / 0x7FFFFFFF)
        a = turn + 2 * math.pi * k / n
        pts.append((x + r * jit * math.cos(a), y + r * jit * squash * math.sin(a)))
    return hull(pts)


def hull(pts, snap=1.0):
    """convex hull, counter-clockwise (monotone chain), corners snapped to `snap` and no
    edge shorter than 4 units (slivers make the compiler's hulls ambiguous)"""
    pts = sorted(set((round(p[0] / snap) * snap, round(p[1] / snap) * snap) for p in pts))
    if len(pts) < 3:
        return pts

    def cr(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    lo, up = [], []
    for p in pts:
        while len(lo) >= 2 and cr(lo[-2], lo[-1], p) <= 0:
            lo.pop()
        lo.append(p)
    for p in reversed(pts):
        while len(up) >= 2 and cr(up[-2], up[-1], p) <= 0:
            up.pop()
        up.append(p)
    h = lo[:-1] + up[:-1]
    out = []
    for p in h:
        if not out or math.hypot(p[0] - out[-1][0], p[1] - out[-1][1]) >= 4:
            out.append(p)
    if len(out) > 2 and math.hypot(out[0][0] - out[-1][0], out[0][1] - out[-1][1]) < 4:
        out.pop()
    return out


def clip(poly, a, b):
    """keeps the part of a convex polygon left of the line a->b (Sutherland-Hodgman, one edge)"""
    out = []
    side = lambda p: (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    for k in range(len(poly)):
        p, q = poly[k], poly[(k + 1) % len(poly)]
        sp, sq = side(p), side(q)
        if sp >= 0:
            out.append(p)
        if (sp >= 0) != (sq >= 0):
            t = sp / (sp - sq)
            out.append((p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t))
    return out


def area(poly):
    return 0.5 * sum(poly[k][0] * poly[(k + 1) % len(poly)][1] - poly[(k + 1) % len(poly)][0] * poly[k][1] for k in range(len(poly)))


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


world = Ent("worldspawn", mapversion=220, wad=";".join(WADS), skyname="cliff",
            message="Valley (Battle Royale)", sounds=1, MaxRange=12000)
ents = [world]
detail = Ent("func_detail", zhlt_detaillevel=1)
ents.append(detail)


# placement registry: every solid brush with the group it belongs to (a building, a bridge, a
# rock...). Brushes of different groups must not overlap, except with the ground itself; cover
# (trees, rocks, bushes) is only placed where it's free. check_placement() fails the build.
GROUP = "terrain"
EXEMPT = ("terrain", "bed", "slab", "shell", "road", "bank")
REG = []
GRID = {}


def group(name):
    global GROUP
    GROUP = name


def _cells(bd):
    for gx_ in range(int(bd[0] // 256), int(bd[3] // 256) + 1):
        for gy_ in range(int(bd[1] // 256), int(bd[4] // 256) + 1):
            yield (gx_, gy_)


def register(b, grp=None):
    grp = grp or GROUP
    b.group = grp
    REG.append(b)
    if not grp.startswith(EXEMPT):
        for c in _cells(b.bounds):
            GRID.setdefault(c, []).append(b)


def _overlap(a, b, tol=0.5):
    A, B = a.bounds, b.bounds
    if not all(min(A[k + 3], B[k + 3]) - max(A[k], B[k]) > tol for k in range(3)):
        return False
    # the boxes overlap: test the footprints exactly (separating axes of the two convex polygons)
    for poly in (a.poly, b.poly):
        n = len(poly)
        for k in range(n):
            ex, ey = poly[(k + 1) % n][0] - poly[k][0], poly[(k + 1) % n][1] - poly[k][1]
            ax, ay = -ey, ex
            pa = [ax * p[0] + ay * p[1] for p in a.poly]
            pb = [ax * p[0] + ay * p[1] for p in b.poly]
            L = math.hypot(ax, ay) or 1.0
            if min(max(pa), max(pb)) - max(min(pa), min(pb)) <= tol * L:
                return False
    return True


def clashes(b, grp=None, allow=()):
    """the registered brushes (of other groups, not the ground) that b overlaps"""
    grp = grp or GROUP
    seen, out = set(), []
    for c in _cells(b.bounds):
        for o in GRID.get(c, ()):
            if id(o) in seen or o.group == grp or o.group.startswith(allow):
                continue
            seen.add(id(o))
            if _overlap(b, o):
                out.append(o)
    return out


def W(b):
    world.brushes.append(b)
    register(b)


def D(b):
    detail.brushes.append(b)
    register(b)


def check_placement():
    bad = []
    seen = set()
    for b in REG:
        if b.group.startswith(EXEMPT):
            continue
        for o in clashes(b, b.group):
            key = tuple(sorted((id(b), id(o))))
            if key in seen:
                continue
            seen.add(key)
            # cover may touch cover (a rock against a rock in a formation)
            if b.group in ("rock", "tree", "bush", "boulder") and o.group in ("rock", "tree", "bush", "boulder"):
                continue
            bad.append((b.group, o.group, [round(v) for v in b.bounds], [round(v) for v in o.bounds]))
    return bad


def _inside(poly, x, y):
    n, sgn = len(poly), 0
    for k in range(n):
        (ax, ay), (bx_, by) = poly[k], poly[(k + 1) % n]
        c = (bx_ - ax) * (y - ay) - (by - ay) * (x - ax)
        if abs(c) < 1e-6:
            continue
        if sgn == 0:
            sgn = 1 if c > 0 else -1
        elif (c > 0) != (sgn > 0):
            return False
    return True


def surface(x, y):
    """the ground a structure stands on (None in trench and hatch cells: no terrain there)"""
    if KIND.get((ci(x), ci(y))) == "cut":
        return None
    z = ground(x, y)
    for tri in CHAMFERS:
        if _inside(tri, x, y):
            return max(z, 0)
    return z


def check_ground():
    """structures the ground rises into: somewhere over the footprint the ground is above the
    brush's top (it's buried there) while elsewhere the top shows (stairs through a bank, a
    floor with the hillside poking through). Fully buried brushes (foundations) are fine."""
    bad = []
    for b in REG:
        if b.group.startswith(EXEMPT) or b.group in ("rock", "tree", "bush", "boulder"):
            continue
        (ax, ay, az), (bx_, by, bz), (cx_, cy, cz) = b.top3
        n = cross((bx_ - ax, by - ay, bz - az), (cx_ - ax, cy - ay, cz - az))
        if abs(n[2]) < 1e-9:
            continue
        x0, y0, _, x1, y1, _ = b.bounds
        nx, ny = max(2, int((x1 - x0) // 32) + 1), max(2, int((y1 - y0) // 32) + 1)
        shown = buried = None
        for ix in range(nx):
            for iy in range(ny):
                x = x0 + 2 + (x1 - x0 - 4) * ix / (nx - 1)
                y = y0 + 2 + (y1 - y0 - 4) * iy / (ny - 1)
                if not _inside(b.poly, x, y):
                    continue
                g = surface(x, y)
                if g is None:
                    continue
                t = az - (n[0] * (x - ax) + n[1] * (y - ay)) / n[2]
                if g < t - 1:
                    shown = True
                elif g > t + 8 and (buried is None or g - t > buried[2]):
                    buried = (round(x), round(y), round(g - t))
        if shown and buried:
            bad.append((b.group, [round(v) for v in b.bounds], buried))
    return bad


def point(classname, x, y, z, **keys):
    e = Ent(classname, origin="%d %d %d" % (x, y, z), **keys)
    ents.append(e)
    return e


# ---------------------------------------------------------------------------------------
# layout tables

def cx(i):
    return X0 + CELL * i


def ci(x):
    return int(math.floor((x - X0) / CELL))


BRIDGE_ROWS = [8, 17, 25]   # south, middle, north road
FORDS = [13, 21]            # rows where the river has ramps down and up both banks
BED_STAIRS = [4, 14, 27]    # more stairs out of the bed (besides those by the bridges)


def river_span(j):
    """the river's cells in row j: [west, east) in cell columns. A meander of 2-3 cells,
    kept clear of the places on both banks"""
    # the bends (row: centre column), eased between, plus a small wobble
    bends = [(0, 19.0), (4, 16.4), (7.5, 18.6), (11, 20.3), (14, 19.6), (17, 19.0), (20, 19.6), (23, 20.2),
             (26, 17.6), (29, 16.6)]
    for (j0, c0), (j1, c1) in zip(bends, bends[1:]):
        if j0 <= j <= j1:
            f = (1 - math.cos(math.pi * (j - j0) / (j1 - j0))) / 2
            c = c0 + (c1 - c0) * f + 0.3 * math.sin(j * 1.7)
            break
    v = math.sin(j * 0.47 + 1.3) + 0.4 * math.sin(j * 1.3)
    w = 4 if v > 1.15 else 3 if v > 0.3 else 2
    w0 = int(round(c - w / 2.0))
    w1 = w0 + w
    lo = 18 if 8 <= j <= 21 else 15 if j <= 7 else 17 if j >= 23 else 16   # docks, town, radio hill on the west bank
    hi = 20 if j >= 25 else 21 if 9 <= j <= 22 else 22     # quarry, lookout, cabins on the east
    if w0 < lo:
        w0, w1 = lo, lo + w
    if w1 > hi:
        w0, w1 = hi - w, hi
    return w0, w1


SPAN = {j: river_span(j) for j in range(1, N - 1)}
# straight and even where it's crossed: the rows around each bridge, ford and bed stairs
for j in BRIDGE_ROWS + FORDS + BED_STAIRS:
    SPAN[j] = (SPAN[j][0], min(SPAN[j][0] + 3, SPAN[j][1]))
    if j + 1 < N - 1:
        SPAN[j + 1] = SPAN[j]
# keep the channel joined (each row overlaps the next by a cell at least)
for j in range(2, N - 1):
    a0, a1 = SPAN[j - 1]
    b0, b1 = SPAN[j]
    if b1 <= a0 + 0:
        SPAN[j] = (b0, a0 + 1)
    elif b0 >= a1:
        SPAN[j] = (a1 - 1, b1)
RIVER = set()
for j, (w0, w1) in SPAN.items():
    for i in range(w0, w1):
        RIVER.add((i, j))


def river_x(j):
    """west and east bank x of the river in row j"""
    w0, w1 = SPAN[j]
    return cx(w0), cx(w1)


def row_of(y):
    return ci(y)


# flat pads under places: (name, x0, y0, x1, y1, z)
PADS = [
    ("town", -1792, -512, 256, 1280, 64),
    ("factory", -3328, -3328, -1792, -1536, 0),
    ("farm", -3328, 1792, -1792, 3328, 32),
    ("radio", -512, 2560, 0, 3072, 448),
    ("gas", -1280, -3072, -512, -1792, 0),
    ("bunker", 2304, -3328, 3328, -2304, 160),
    ("lumber", 2304, 2048, 3328, 3072, 128),
    ("quarry", 2304, -512, 3328, 512, -128),
    ("docks", -256, -1536, 512, -768, 0),
]
# mesas: cells raised flat with cliff sides; one ramp each (x0, y0, x1, y1, z, ramp side)
MESAS = [
    ("fort", -3328, -1280, -2816, -512, 352, "e"),
    ("lookout", 2048, 1024, 2560, 1536, 320, "e"),
]
# small flat spots for cabins (centre); their height comes from the terrain
CABINS = [(-2816, 1024), (-1792, -1280), (2176, -1024), (1792, 3200), (-1280, 1792)]
# hills: (x, y, height, radius)
HILLS = [(-2420, -40, 560, 1000),                                    # West Peak (landmark)
         (2050, -1250, 200, 450), (2350, -1500, 240, 420), (2050, -1850, 220, 420),   # the east ridge
         (-600, -2600, 150, 380), (1200, -3100, 170, 520), (-3100, -2700, 140, 400), (300, 3250, 220, 600),
         (-1900, 3150, 120, 420), (-3350, 1900, 180, 500), (-1024, 2048, 180, 700), (2816, 1152, 260, 700), (-1408, -1152, 200, 600),
         (1920, -1920, 220, 600), (-2304, 1280, 160, 600), (0, -2944, 180, 700), (1920, 1792, 160, 500),
         (-3200, 640, 220, 600), (512, 1792, 140, 500)]

# roads: curves through waypoints (world units), each piece ends at a bridge or a place; drawn as
# thin overlays that follow the ground. (width, texture, waypoints)
def bx(j, side):
    """a point just off the bridge in row j on the west (-1) or east (+1) bank"""
    xw, xe = river_x(j)
    return (xw - 96 if side < 0 else xe + 96, cx(j) + 128)


ROADS = [
    # main road: the west edge, round the fort's hill, through the town's main street, over the middle bridge
    (224, ROAD, [(-3560, -60), (-3100, 120), (-2600, 330), (-2150, 430), (-1700, 400), (-1100, 390), (-560, 400),
                 (-60, 430), (380, 560), bx(17, -1)]),
    (224, ROAD, [bx(17, 1), (2000, 700), (2400, 560), (2700, 320), (2900, 120), (3250, 60), (3560, 120)]),
    # north road: farm, the radio hill's foot, the north bridge, the lumber camp
    (192, ROAD, [(-2560, 2240), (-2050, 2420), (-1500, 2330), (-950, 2440), (-400, 2420), (150, 2560), bx(25, -1)]),
    (192, ROAD, [bx(25, 1), (1950, 2620), (2350, 2470), (2680, 2380), (2950, 2330)]),
    # south road: factory yard, gas station, the south bridge, the bunker
    (192, ROAD, [(-2600, -2200), (-2150, -1980), (-1600, -1900), (-1000, -1930), (-450, -1830), (150, -1720),
                 bx(8, -1)]),
    (192, ROAD, [bx(8, 1), (1950, -1720), (2250, -1950), (2550, -2180), (2750, -2300)]),
    # west track: factory, past the fort, the main road, the farm
    (160, DIRT, [(-2600, -2200), (-2480, -1650), (-2300, -1150), (-2250, -600), (-2120, -100), (-2150, 430)]),
    (160, DIRT, [(-2150, 430), (-2350, 900), (-2250, 1450), (-2400, 1900), (-2560, 2240)]),
    # middle tracks: gas station - docks - town; town - radio hill
    (160, DIRT, [(-1000, -1930), (-760, -1450), (-420, -1100), (-300, -760), (-520, -380), (-520, 400)]),
    (160, DIRT, [(-544, 400), (-544, 1180), (-480, 1700), (-400, 2420)]),
    # east track: bunker - quarry - lumber camp (east of the lookout)
    (160, DIRT, [(2750, -2300), (2950, -1750), (2780, -1100), (2650, -480), (2700, 150), (2700, 320)]),
    (160, DIRT, [(2700, 560), (2760, 1100), (2900, 1650), (2850, 2050), (2950, 2330)]),
]


def smooth(pts, step=96.0):
    """a Catmull-Rom curve through the points, sampled about every `step` units"""
    out = []
    P = [pts[0]] + list(pts) + [pts[-1]]
    for k in range(1, len(P) - 2):
        p0, p1, p2, p3 = P[k - 1], P[k], P[k + 1], P[k + 2]
        n = max(1, int(math.hypot(p2[0] - p1[0], p2[1] - p1[1]) / step))
        for t in range(n):
            u = t / n
            out.append(tuple(0.5 * ((2 * p1[a]) + (-p0[a] + p2[a]) * u + (2 * p0[a] - 5 * p1[a] + 4 * p2[a] - p3[a]) * u * u
                                    + (-p0[a] + 3 * p1[a] - 3 * p2[a] + p3[a]) * u * u * u) for a in (0, 1)))
    out.append(tuple(pts[-1]))
    return out


ROADLINES = [(w, tex, smooth(pts)) for w, tex, pts in ROADS]


def near_road(x, y, m=0.0):
    for w, tex, line in ROADLINES:
        r = w / 2 + m
        for (ax, ay), (bx_, by) in zip(line, line[1:]):
            dx, dy = bx_ - ax, by - ay
            L2 = dx * dx + dy * dy or 1.0
            t = max(0.0, min(1.0, ((x - ax) * dx + (y - ay) * dy) / L2))
            if (x - ax - t * dx) ** 2 + (y - ay - t * dy) ** 2 < r * r:
                return True
    return False

# tunnels (z TUN_Z..SLAB_TOP): rectangles; trench entrances and hatches below
TUNNELS = [
    ("sewer", -1600, 320, 256, 512),            # under the town's main street
    ("sewer_s", -992, -1952, -800, 544),         # town south to the gas station
    ("gas_w", -2752, -2016, -800, -1824),        # gas station west to the factory
    ("factory", -2944, -2944, -2752, -1824),     # under the factory yard
    ("river", -800, -2016, 2688, -1824),         # under the river to the bunker
    ("gas_s", -1280, -2560, -1088, -1824),       # gas station trench to the west tunnel
    ("bunker_a", 2304, -2944, 3136, -2496),      # the bunker complex: two halls
    ("bunker_b", 2688, -2496, 2944, -1824),
    ("quarry", 2816, -1824, 3008, -320),          # bunker north to the quarry
]
# stair trenches down: (cells x0, y0, x1, y1 in cell units, direction the stairs go DOWN)
TRENCHES = [
    ("town", 15, 13, 16, 17, "n"),      # town south-east corner, down northwards to the sewer
    ("gas", 10, 5, 14, 6, "w"),         # behind the gas station, down westwards
    ("factory", 3, 6, 4, 9, "s"),       # factory yard, down southwards
    ("bunker", 24, 2, 25, 6, "n"),      # bunker yard, down northwards
    ("quarry", 26, 13, 27, 17, "s"),    # quarry floor, down southwards
]
# ladder hatches: cell (i, j); a 96 hole with a ladder on its north wall
HATCHES = [(13, 16), (6, 7)]

# ---------------------------------------------------------------------------------------
# the height grid

H = [[0.0] * (N + 1) for _ in range(N + 1)]
FIX = [[False] * (N + 1) for _ in range(N + 1)]


def noise(x, y):
    return (24 * math.sin(x / 610.0 + 1.3) * math.cos(y / 530.0 + 0.4) + 16 * math.sin((x + y) / 380.0)
            + 10 * math.cos((x - 2 * y) / 290.0))


for i in range(N + 1):
    for j in range(N + 1):
        x, y = cx(i), cx(j)
        h = noise(x, y) + 24
        for hx, hy, hh, hr in HILLS:
            d2 = ((x - hx) ** 2 + (y - hy) ** 2) / (hr * hr)
            h += hh * math.exp(-d2 * 1.6)
        # the play edge rises a bit (hills at the rim)
        e = min(i, j, N - i, N - j)
        if e <= 4:
            h += (4 - e) * 56
        H[i][j] = h

# river banks flat at 0 (one vertex either side of the channel)
for (c, j) in RIVER:
    for di in (-1, 0, 1, 2):
        for dj in (0, 1):
            ii, jj = c + di, j + dj
            if 1 <= ii < N and 1 <= jj < N:
                H[ii][jj] = 0
                FIX[ii][jj] = True


def fix_rect(x0, y0, x1, y1, z):
    for i in range(ci(x0), ci(x1) + 1):
        for j in range(ci(y0), ci(y1) + 1):
            H[i][j] = z
            FIX[i][j] = True


for name, x0, y0, x1, y1, z in PADS:
    fix_rect(x0, y0, x1, y1, z)
# the stair trenches' rims at the pad's height (one runs out past the gas station's pad)
for name, i0, j0, i1, j1, d in TRENCHES:
    fix_rect(cx(i0), cx(j0), cx(i1), cx(j1), H[i0][j0])
# the backdrop ring: tall cliffs, fixed
for i in range(N + 1):
    for j in range(N + 1):
        if min(i, j, N - i, N - j) == 0:
            x, y = cx(i), cx(j)
            H[i][j] = 1300 + 260 * math.sin(x / 700.0 + y / 900.0) + 180 * math.cos(y / 400.0 - x / 1300.0)
            FIX[i][j] = True


def relax():
    for _ in range(200):
        changed = False
        for i in range(1, N):
            for j in range(1, N):
                if FIX[i][j]:
                    continue
                nb = []
                for di in (-1, 0, 1):
                    for dj in (-1, 0, 1):
                        if (di or dj):
                            ii, jj = i + di, j + dj
                            if 1 <= ii < N and 1 <= jj < N:
                                nb.append(H[ii][jj])
                lo, hi = max(nb) - MAXD, min(nb) + MAXD
                v = H[i][j]
                if v > hi:
                    v = hi
                if v < lo:
                    v = lo
                if abs(v - H[i][j]) > 0.01:
                    H[i][j] = v
                    changed = True
        if not changed:
            break


relax()
# cabins: a flat 2x2-cell spot at the terrain's own height there
for (x, y) in CABINS:
    i, j = ci(x), ci(y)
    z = round(H[i][j] / 8) * 8
    fix_rect(x - 256, y - 256, x + 256, y + 256, z)
relax()
for i in range(N + 1):
    for j in range(N + 1):
        H[i][j] = round(H[i][j] / 8) * 8

# ---------------------------------------------------------------------------------------
# cells: what each one is

KIND = {}           # (i, j) -> "river" | "cut" | "mesa:<z>" ; default terrain
BLOCKED = []        # footprints kept clear of spawns, crates, trees and rocks
for c in RIVER:
    KIND[c] = "river"
for name, x0, y0, x1, y1, z, side in MESAS:
    for i in range(ci(x0), ci(x1)):
        for j in range(ci(y0), ci(y1)):
            KIND[(i, j)] = "mesa:%d" % z
TRENCH_CELLS = {}
for name, i0, j0, i1, j1, d in TRENCHES:
    for i in range(i0, i1):
        for j in range(j0, j1):
            KIND[(i, j)] = "cut"
            TRENCH_CELLS[(i, j)] = name
for (i, j) in HATCHES:
    KIND[(i, j)] = "cut"

def ground(x, y):
    """surface height at x, y (terrain triangles, pads, river bed, mesa tops)"""
    i, j = ci(x), ci(y)
    k = KIND.get((i, j))
    if k == "river":
        return BED_Z
    if k and k.startswith("mesa:"):
        return int(k[5:])
    fx, fy = (x - cx(i)) / CELL, (y - cx(j)) / CELL
    h00, h10, h01, h11 = H[i][j], H[i + 1][j], H[i][j + 1], H[i + 1][j + 1]
    # split along (i,j)-(i+1,j+1)
    if fx >= fy:
        return h00 + fx * (h10 - h00) + fy * (h11 - h10)
    return h00 + fy * (h01 - h00) + fx * (h11 - h01)


def cell_tex(i, j, slope):
    if min(i, j, N - 1 - i, N - 1 - j) == 0:
        return ROCK
    if slope > 0.55:
        return ROCK2
    for name, x0, y0, x1, y1, z in PADS:
        if x0 <= cx(i) < x1 and y0 <= cx(j) < y1:
            return {"town": ASPH, "factory": "-0CRETE4_FLR01", "gas": ASPH, "bunker": SAND,
                    "quarry": "OUT_GRAVEL", "docks": DIRT, "radio": DIRT}.get(name, GRASS)
    # the grass follows the ground: worn on the steeper slopes, rock where it's steepest
    return GRASS2 if slope > 0.46 else GRASS


# ---------------------------------------------------------------------------------------
# the sky box (well inside +/-4096) and the floor under the slab

group("shell")
E = -X0
T = 64
for b in [
    (-E - T, -E - T, SKY_Z, E + T, E + T, SKY_Z + T),
    (-E - T, -E - T, FLOOR_Z - T, E + T, E + T, FLOOR_Z),
    (-E - T, -E - T, FLOOR_Z, -E, E + T, SKY_Z),
    (E, -E - T, FLOOR_Z, E + T, E + T, SKY_Z),
    (-E, -E - T, FLOOR_Z, E, -E, SKY_Z),
    (-E, E, FLOOR_Z, E, E + T, SKY_Z),
]:
    W(box(*b, "SKY"))
# the play edge: invisible walls up to the sky (the backdrop cliffs are only to look at)
for b in [(-PLAY - 16, -PLAY, SLAB_TOP, -PLAY, PLAY, SKY_Z), (PLAY, -PLAY, SLAB_TOP, PLAY + 16, PLAY, SKY_Z),
          (-PLAY - 16, -PLAY - 16, SLAB_TOP, PLAY + 16, -PLAY, SKY_Z), (-PLAY - 16, PLAY, SLAB_TOP, PLAY + 16, PLAY + 16, SKY_Z)]:
    W(box(*b, "CLIP"))

# ---------------------------------------------------------------------------------------
group("terrain")
# ---------------------------------------------------------------------------------------
# terrain: flat cells merged into rectangles, sloped cells as two triangle prisms

flat = {}
for i in range(N):
    for j in range(N):
        k = KIND.get((i, j))
        if k in ("river", "cut") or (k and k.startswith("mesa:")):
            continue
        hs = (H[i][j], H[i + 1][j], H[i][j + 1], H[i + 1][j + 1])
        if max(hs) == min(hs):
            flat[(i, j)] = (hs[0], cell_tex(i, j, 0))
            continue
        p00 = (cx(i), cx(j), H[i][j])
        p10 = (cx(i + 1), cx(j), H[i + 1][j])
        p01 = (cx(i), cx(j + 1), H[i][j + 1])
        p11 = (cx(i + 1), cx(j + 1), H[i + 1][j + 1])
        for tri in ([p00, p10, p11], [p00, p11, p01]):
            n = cross(sub(tri[1], tri[0]), sub(tri[2], tri[0]))
            slope = math.hypot(n[0], n[1]) / abs(n[2])
            W(prism(tri, SLAB_TOP, cell_tex(i, j, slope), ROCK, bottom=CEIL, scale=2))

# mesa tops
for (i, j), k in list(KIND.items()):
    if k.startswith("mesa:"):
        flat[(i, j)] = (int(k[5:]), DIRT)


def merge_cells(cells):
    """{(i,j): key} -> [(i0, j0, i1, j1, key)] greedy rectangles"""
    left = dict(cells)
    out = []
    for j in range(N):
        for i in range(N):
            if (i, j) not in left:
                continue
            key = left[(i, j)]
            i1 = i
            while (i1 + 1, j) in left and left[(i1 + 1, j)] == key:
                i1 += 1
            j1 = j
            while all((ii, j1 + 1) in left and left[(ii, j1 + 1)] == key for ii in range(i, i1 + 1)):
                j1 += 1
            for ii in range(i, i1 + 1):
                for jj in range(j, j1 + 1):
                    del left[(ii, jj)]
            out.append((i, j, i1 + 1, j1 + 1, key))
    return out


for i0, j0, i1, j1, (z, tex) in merge_cells(flat):
    W(box(cx(i0), cx(j0), SLAB_TOP, cx(i1), cx(j1), z, ROCK, top=tex, bottom=CEIL, scale=2))

# roads: thin overlays (non-solid, so bots and players walk on the ground under them) cut to the
# terrain's triangles, in one func_illusionary per 1024 tile so VIS still culls them
def tri_of(i, j):
    p00 = (cx(i), cx(j), H[i][j])
    p10 = (cx(i + 1), cx(j), H[i + 1][j])
    p01 = (cx(i), cx(j + 1), H[i][j + 1])
    p11 = (cx(i + 1), cx(j + 1), H[i + 1][j + 1])
    return [(p00, p10, p11), (p00, p11, p01)]


def plane_z(t, x, y):
    (ax, ay, az), (bx_, by, bz), (cx_, cy, cz) = t
    n = cross((bx_ - ax, by - ay, bz - az), (cx_ - ax, cy - ay, cz - az))
    return az - (n[0] * (x - ax) + n[1] * (y - ay)) / n[2]


ROAD_TILES = {}
road_pieces = 0
for rk, (w, tex, line) in enumerate(ROADLINES):
    lift = 1.0 + 0.75 * (rk % 4)
    cuts = []
    for k in range(len(line)):
        a = line[max(k - 1, 0)]
        b = line[min(k + 1, len(line) - 1)]
        tx, ty = b[0] - a[0], b[1] - a[1]
        L = math.hypot(tx, ty) or 1.0
        nx, ny = -ty / L * w / 2, tx / L * w / 2
        cuts.append(((line[k][0] + nx, line[k][1] + ny), (line[k][0] - nx, line[k][1] - ny)))
    for k in range(len(line) - 1):
        quad = [cuts[k][1], cuts[k + 1][1], cuts[k + 1][0], cuts[k][0]]
        if area(quad) < 0:
            quad = quad[::-1]
        xs_, ys_ = [p[0] for p in quad], [p[1] for p in quad]
        for i in range(ci(min(xs_)), ci(max(xs_)) + 1):
            for j in range(ci(min(ys_)), ci(max(ys_)) + 1):
                if not (1 <= i < N - 1 and 1 <= j < N - 1):
                    continue
                kd = KIND.get((i, j))
                if kd in ("river", "cut") or (kd and kd.startswith("mesa:")):
                    continue
                for t in tri_of(i, j):
                    tri2 = [(t[0][0], t[0][1]), (t[1][0], t[1][1]), (t[2][0], t[2][1])]
                    if area(tri2) < 0:
                        tri2 = tri2[::-1]
                    poly = quad
                    for e in range(3):
                        poly = clip(poly, tri2[e], tri2[(e + 1) % 3])
                        if len(poly) < 3:
                            break
                    if len(poly) < 3 or abs(area(poly)) < 400:
                        continue
                    poly = hull(poly)
                    if len(poly) < 3:
                        continue
                    top = [(px, py, plane_z(t, px, py) + lift) for (px, py) in poly]
                    zb = min(q[2] for q in top) - 3
                    tile = (int((poly[0][0] + 4096) // 512), int((poly[0][1] + 4096) // 512))
                    if tile not in ROAD_TILES:
                        e_ = Ent("func_illusionary", zhlt_noclip=1)
                        ROAD_TILES[tile] = e_
                        ents.append(e_)
                    ROAD_TILES[tile].brushes.append(prism(top, zb, tex, "NULL", bottom="NULL", scale=2))
                    road_pieces += 1

group("bed")
# the river bed
bed = {c: 1 for c in RIVER}
for i0, j0, i1, j1, _ in merge_cells(bed):
    W(box(cx(i0), cx(j0), SLAB_TOP, cx(i1), cx(j1), BED_Z, ROCK, top=MUD, bottom=CEIL, scale=2))

group("bank")
# the channel's corners cut at 45 degrees (where it widens or bends), so it doesn't read as a grid
KEEP_STRAIGHT = set()
CHAMFERS = []        # the land triangles in river cells (ground at 0)
for j in BRIDGE_ROWS + FORDS + BED_STAIRS + [9, 10, 11]:      # (9-11: the docks' piers and stairs)
    KEEP_STRAIGHT |= {j, j + 1}
chamfers = 0
for (i, j) in sorted(RIVER):
    if j in KEEP_STRAIGHT:
        continue
    land = lambda a, b: (a, b) not in RIVER
    x0, y0, x1, y1 = cx(i), cx(j), cx(i + 1), cx(j + 1)
    for (dw, dn, tri) in [((-1, 0), (0, 1), [(x0, y0), (x1, y1), (x0, y1)]),     # land west and north: NW half
                          ((1, 0), (0, 1), [(x1, y0), (x1, y1), (x0, y1)]),      # NE
                          ((-1, 0), (0, -1), [(x0, y0), (x1, y0), (x0, y1)]),    # SW
                          ((1, 0), (0, -1), [(x0, y0), (x1, y0), (x1, y1)])]:    # SE
        if land(i + dw[0], j + dw[1]) and land(i + dn[0], j + dn[1]) and not land(i - dw[0], j - dw[1]) \
                and not land(i - dn[0], j - dn[1]):
            W(prism([(px, py, 0) for (px, py) in tri], SLAB_TOP, GRASS, ROCK, bottom=CEIL, scale=2))
            CHAMFERS.append(tri)
            chamfers += 1
            break
group("ford")
# fords: a ramp down from each bank to the middle of the channel
for j in FORDS:
    xw, xe = river_x(j)
    y0, y1 = cx(j), cx(j + 1)
    mid = (xw + xe) / 2
    D(prism([(xw, y0, 0), (mid, y0, BED_Z), (mid, y1, BED_Z), (xw, y1, 0)], BED_Z - 8, DIRT, ROCK))
    D(prism([(mid, y0, BED_Z), (xe, y0, 0), (xe, y1, 0), (mid, y1, BED_Z)], BED_Z - 8, DIRT, ROCK))


def stairs(xa, ya, xb, yb, z0, z1, toward, side=CONC, top="OUT_PAVE1", ent=None):
    """steps filling xa..xb, ya..yb, rising from z0 to z1 toward 'n','s','e','w' (as ls_outpost)"""
    n = int(round((z1 - z0) / 16))
    length = (yb - ya) if toward in "ns" else (xb - xa)
    d = length / n
    put = ent.brushes.append if ent else D
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
        put(box(*b, side, top=top))


# bridges: deck at bank height over the channel, railings, two piers; stairs out of the bed beside
WOOD, PLANK, RAIL = "OUT_WD", "CSTRIKE_FJ2DOCK", "CSTRIKE_ME4METL"
for j in sorted(BRIDGE_ROWS):
    group("bridge:%d" % j)
    xw, xe = river_x(j)
    y0, y1 = cx(j) + 16, cx(j + 1) - 16
    D(box(xw - 32, y0, -24, xe + 32, y1, 0, WOOD, top=PLANK))
    D(box(xw - 32, y0, 0, xe + 32, y0 + 8, 40, RAIL))
    D(box(xw - 32, y1 - 8, 0, xe + 32, y1, 40, RAIL))
    for px in ((xw * 2 + xe) / 3, (xw + xe * 2) / 3):
        D(box(px - 24, y0 + 32, BED_Z, px + 24, y1 - 32, -24, CONC))
# stairs out of the river bed along the banks (every few rows, both sides)
for j in list(sorted(BRIDGE_ROWS)) + BED_STAIRS:
    if j + 1 > N - 2:
        continue
    xw, xe = river_x(j + 1) if river_x(j + 1) == river_x(j) else river_x(j)
    if river_x(j + 1) != river_x(j):
        continue
    ya, yb = cx(j + 1) + 16, cx(j + 1) + 16 + 160
    group("bedstairs:%d" % j)
    stairs(xw, ya, xw + 96, yb, BED_Z, 0, "s", side=ROCK, top=DIRT)
    stairs(xe - 96, ya, xe, yb, BED_Z, 0, "s", side=ROCK, top=DIRT)

# mesas: rock cliff blocks are the cells themselves (drawn as flat above); ramps
for name, x0, y0, x1, y1, z, side in MESAS:
    group("ramp:" + name)
    # the foot follows the hillside across the ramp (each foot point 4 below its own ground):
    # three planar pieces folded at the foot's middle
    if side == "e":
        ya, yb = (y0 + y1) / 2 - 96, (y0 + y1) / 2 + 96
        L = 512
        for _ in range(8):
            ga, gc, gb = (ground(x1 + L - 2, yy) for yy in (ya + 2, (ya + yb) / 2, yb - 2))
            L = max(512, (z - (ga + gb) / 2) / 0.6)
        m, a_, c_, b_, n_ = (x1, ya, z), (x1 + L, ya, ga - 4), (x1 + L, (ya + yb) / 2, gc - 4), (x1 + L, yb, gb - 4), (x1, yb, z)
        BLOCKED.append((x1, ya - 32, x1 + L, yb + 32))
    else:
        xa, xb = (x0 + x1) / 2 - 96, (x0 + x1) / 2 + 96
        L = 512
        for _ in range(8):
            ga, gc, gb = (ground(xx, y0 - L + 2) for xx in (xa + 2, (xa + xb) / 2, xb - 2))
            L = max(512, (z - (ga + gb) / 2) / 0.6)
        m, a_, c_, b_, n_ = (xa, y0, z), (xa, y0 - L, ga - 4), ((xa + xb) / 2, y0 - L, gc - 4), (xb, y0 - L, gb - 4), (xb, y0, z)
        BLOCKED.append((xa - 32, y0 - L, xb + 32, y0))
    for tri in ([m, a_, c_], [m, c_, n_], [n_, c_, b_]):
        D(prism(tri, SLAB_TOP, DIRT, ROCK))

# ---------------------------------------------------------------------------------------
# underground: the slab under the terrain with tunnels cut out; trenches and hatches
group("slab")

OPEN_RECTS = [(x0, y0, x1, y1) for _, x0, y0, x1, y1 in TUNNELS]
for c, name in TRENCH_CELLS.items():
    OPEN_RECTS.append((cx(c[0]), cx(c[1]), cx(c[0] + 1), cx(c[1] + 1)))
for (i, j) in HATCHES:
    OPEN_RECTS.append((cx(i) + 80, cx(j) + 80, cx(i) + 176, cx(j) + 176))
    OPEN_RECTS.append((cx(i), cx(j), cx(i + 1), cx(j + 1)))   # the room at the hatch's foot

xs = sorted({-E, E} | {r[0] for r in OPEN_RECTS} | {r[2] for r in OPEN_RECTS})
ys = sorted({-E, E} | {r[1] for r in OPEN_RECTS} | {r[3] for r in OPEN_RECTS})


def slab_open(x, y):
    return any(r[0] <= x < r[2] and r[1] <= y < r[3] for r in OPEN_RECTS)


cells = {}
for a in range(len(xs) - 1):
    for b in range(len(ys) - 1):
        cells[(a, b)] = slab_open((xs[a] + xs[a + 1]) / 2, (ys[b] + ys[b + 1]) / 2)
# merge on the irregular grid (same greedy idea)
left = dict(cells)
for b in range(len(ys) - 1):
    for a in range(len(xs) - 1):
        if (a, b) not in left:
            continue
        key = left[(a, b)]
        a1 = a
        while (a1 + 1, b) in left and left[(a1 + 1, b)] == key:
            a1 += 1
        b1 = b
        while all((aa, b1 + 1) in left and left[(aa, b1 + 1)] == key for aa in range(a, a1 + 1)):
            b1 += 1
        for aa in range(a, a1 + 1):
            for bb in range(b, b1 + 1):
                del left[(aa, bb)]
        X0b, Y0b, X1b, Y1b = xs[a], ys[b], xs[a1 + 1], ys[b1 + 1]
        if key:
            W(box(X0b, Y0b, FLOOR_Z, X1b, Y1b, TUN_Z, TUNW, top="-0TNNL_FLR1"))
        else:
            W(box(X0b, Y0b, FLOOR_Z, X1b, Y1b, SLAB_TOP, TUNW, top=CEIL))

# trenches: stairs from the tunnel floor up to the pad; railings round the deep part
for name, i0, j0, i1, j1, d in TRENCHES:
    group("trench:" + name)
    x0, y0, x1, y1 = cx(i0), cx(j0), cx(i1), cx(j1)
    up = {"s": "n", "n": "s", "e": "w", "w": "e"}[d]
    top_z = H[i0][j0]
    # the stairs leave a 256 landing at the bottom end
    if d == "s":
        stairs(x0, y0 + 256, x1, y1, TUN_Z, top_z, up)
    elif d == "n":
        stairs(x0, y0, x1, y1 - 256, TUN_Z, top_z, up)
    elif d == "e":
        stairs(x0, y0, x1 - 256, y1, TUN_Z, top_z, up)
    else:
        stairs(x0 + 256, y0, x1, y1, TUN_Z, top_z, up)
    # railings (64 high: you can't jump over) on the long sides and the bottom end, in pieces;
    # where a building's wall stands beside the trench it is the railing
    RH = 64
    if d in "ns":
        yb = y0 if d == "s" else y1
        rails = [(x0 - 8, y0, x0 + 8, y1), (x1 - 8, y0, x1 + 8, y1), (x0, yb - 8, x1, yb + 8)]
    else:
        xb = x1 if d == "e" else x0
        rails = [(x0, y0 - 8, x1, y0 + 8), (x0, y1 - 8, x1, y1 + 8), (xb - 8, y0, xb + 8, y1)]
    for (a, b_, c, e) in rails:
        n_ = max(1, int(max(c - a, e - b_) // 128))
        for k in range(n_):
            if c - a > e - b_:
                piece = box(a + (c - a) * k / n_, b_, top_z, a + (c - a) * (k + 1) / n_, e, top_z + RH, RAIL)
            else:
                piece = box(a, b_ + (e - b_) * k / n_, top_z, c, b_ + (e - b_) * (k + 1) / n_, top_z + RH, RAIL)
            bx0, by0, _, bx1, by1, _ = piece.bounds
            # (the bunker's west wall, built later, stands beside its trench)
            if any(bx0 < r[2] and bx1 > r[0] and by0 < r[3] and by1 > r[1] for r in [(2560, -3072, 2592, -2624)]):
                continue
            if not clashes(piece):
                W(piece)

# hatches: the cell is solid but for a 96 hole; a ladder on the hole's north wall
for (i, j) in HATCHES:
    group("hatch:%d,%d" % (i, j))
    x0, y0 = cx(i), cx(j)
    z = H[i][j]
    hx0, hy0, hx1, hy1 = x0 + 80, y0 + 80, x0 + 176, y0 + 176
    W(box(x0, y0, SLAB_TOP, x0 + 256, hy0, z, ROCK, top=CONC_F))
    W(box(x0, hy1, SLAB_TOP, x0 + 256, y0 + 256, z, ROCK, top=CONC_F))
    W(box(x0, hy0, SLAB_TOP, hx0, hy1, z, ROCK, top=CONC_F))
    W(box(hx1, hy0, SLAB_TOP, x0 + 256, hy1, z, ROCK, top=CONC_F))
    # the shaft between slab top and the tunnel room is part of the open slab already;
    # the room's ceiling around the shaft: the cell's slab is open, so close it
    W(box(x0, y0, SLAB_TOP - 16, x0 + 256, hy0, SLAB_TOP, CONC, bottom=CEIL))
    W(box(x0, hy1, SLAB_TOP - 16, x0 + 256, y0 + 256, SLAB_TOP, CONC, bottom=CEIL))
    W(box(x0, hy0, SLAB_TOP - 16, hx0, hy1, SLAB_TOP, CONC, bottom=CEIL))
    W(box(hx1, hy0, SLAB_TOP - 16, x0 + 256, hy1, SLAB_TOP, CONC, bottom=CEIL))
    # a low rim on three sides (south, east, west); the ladder side (north) is the way out
    W(box(hx0 - 16, hy0 - 16, z, hx1 + 16, hy0, z + 40, RAIL))
    W(box(hx0 - 16, hy0, z, hx0, hy1, z + 40, RAIL))
    W(box(hx1, hy0, z, hx1 + 16, hy1, z + 40, RAIL))
    lad = Ent("func_wall", rendermode=4, renderamt=255)
    lad.brushes.append(box(hx0 + 16, hy1 - 4, TUN_Z, hx1 - 16, hy1, z + 8, "{CSTRIKE_LE6LAD"))
    ents.append(lad)
    fl = Ent("func_ladder")
    fl.brushes.append(box(hx0 + 16, hy1 - 12, TUN_Z, hx1 - 16, hy1, z + 8, "AAATRIGGER"))
    ents.append(fl)
    point("light", x0 + 128, y0 + 128, TUN_Z + 96, _light="255 220 170 120")

group("slab")
# tunnel lights every ~384 along each tunnel
for name, x0, y0, x1, y1 in TUNNELS:
    lx, ly = x1 - x0, y1 - y0
    n = max(1, int(max(lx, ly) // 384))
    for k in range(n):
        f = (k + 0.5) / n
        x = x0 + lx * (f if lx >= ly else 0.5)
        y = y0 + ly * (f if ly > lx else 0.5)
        point("light", x, y, SLAB_TOP - 24, _light="255 214 150 160")
        D(box(x - 16, y - 16, SLAB_TOP - 4, x + 16, y + 16, SLAB_TOP, "~LIGHT3A"))

# ---------------------------------------------------------------------------------------
# buildings




def wall(x0, y0, x1, y1, z0, z1, tex, openings, along):
    """a wall box x0..x1, y0..y1 from z0 to z1 with openings [(centre, width, zlo, zhi)]
    along 'x' or 'y'"""
    a0, a1 = (x0, x1) if along == "x" else (y0, y1)
    cuts = sorted((c - w / 2, c + w / 2, lo, hi) for c, w, lo, hi in openings)
    pos = a0
    for s, e, lo, hi in cuts:
        if s > pos:
            seg(x0, y0, x1, y1, along, pos, s, z0, z1, tex)
        if lo > z0:
            seg(x0, y0, x1, y1, along, s, e, z0, lo, tex)
        if hi < z1:
            seg(x0, y0, x1, y1, along, s, e, hi, z1, tex)
        pos = e
    if pos < a1:
        seg(x0, y0, x1, y1, along, pos, a1, z0, z1, tex)


def seg(x0, y0, x1, y1, along, a, b, z0, z1, tex):
    if b - a < 1 or z1 - z0 < 1:
        return
    if along == "x":
        W(box(a, y0, z0, b, y1, z1, tex))
    else:
        W(box(x0, a, z0, x1, b, z1, tex))


def building(x0, y0, x1, y1, z, floors=1, fh=160, wt=16, tex="OUT_WALL1A", inner=None, roof="OUT_ROOF2B",
             doors=(), windows=True, roof_access=False, floor_tex="CSTRIKE_FE2SQRS", light=True, updoors=(),
             roof_gaps=()):
    """updoors: [(floor, side, centre, width)] doorways on upper floors (to bridges); roof_gaps:
    [(side, centre, width)] openings in the roof's parapet (to bridges)"""
    keep = GROUP
    group("bld:%d,%d" % (x0, y0))
    top = _building(x0, y0, x1, y1, z, floors, fh, wt, tex, inner, roof, doors, windows, roof_access, floor_tex, light,
                    updoors, roof_gaps)
    group(keep)
    return top


def _building(x0, y0, x1, y1, z, floors, fh, wt, tex, inner, roof, doors, windows, roof_access, floor_tex, light,
              updoors, roof_gaps):
    """a box building: doors [(side, offset from the side's middle)] 128 wide; floors joined by
    stairs along the west wall rising north; roof_access puts stairs from the top floor onto
    the roof (parapet 40)"""
    BLOCKED.append((x0 - 32, y0 - 32, x1 + 32, y1 + 32))
    inner = inner or tex
    top = z + floors * fh
    for f in range(floors):
        fz = z + f * fh
        fz1 = fz + fh
        op = {"n": [], "s": [], "e": [], "w": []}
        if f == 0:
            for side, off in doors:
                mid = (x0 + x1) / 2 if side in "ns" else (y0 + y1) / 2
                op[side].append((mid + off, 128, fz, fz + 128))
        for (uf, side, c, w) in updoors:
            if uf == f:
                op[side].append((c, w, fz, fz + 112))
        if windows:
            for side in "nsew":
                a0, a1 = (x0, x1) if side in "ns" else (y0, y1)
                for c in range(int(a0) + 96, int(a1) - 64, 192):
                    if all(abs(c - o[0]) > 128 for o in op[side]):
                        op[side].append((c, 64, fz + 56, fz + 112))
        wall(x0, y1 - wt, x1, y1, fz, fz1, tex, op["n"], "x")
        wall(x0, y0, x1, y0 + wt, fz, fz1, tex, op["s"], "x")
        wall(x0, y0 + wt, x0 + wt, y1 - wt, fz, fz1, tex, op["w"], "y")
        wall(x1 - wt, y0 + wt, x1, y1 - wt, fz, fz1, tex, op["e"], "y")
    # floors / roof with a stair hole over each flight; flights switch back: from the ground
    # floor along the west wall rising north, the next along the east wall rising south, ...
    run = int(fh / 16) * 22
    sy0 = y0 + wt + 64
    sy1 = sy0 + run
    levels = floors if roof_access else floors - 1
    assert levels == 0 or (sy1 < y1 - wt - 32 and x1 - x0 >= 288), ("building too small", x0, y0, x1, y1)

    def flight_x(f):
        return (x0 + wt, x0 + wt + 112) if f % 2 == 0 else (x1 - wt - 112, x1 - wt)

    for f in range(1, floors + 1):
        fz = z + f * fh
        t, btm = (roof if f == floors else floor_tex), "CSTRIKE_ME1CLNG"
        if f <= levels:
            hx0, hx1 = flight_x(f - 1)
            W(box(x0, y0, fz - 16, x1, sy0, fz, tex, top=t, bottom=btm))
            W(box(x0, sy1, fz - 16, x1, y1, fz, tex, top=t, bottom=btm))
            W(box(hx1, sy0, fz - 16, x1, sy1, fz, tex, top=t, bottom=btm))
            W(box(x0, sy0, fz - 16, hx0, sy1, fz, tex, top=t, bottom=btm))
        else:
            W(box(x0, y0, fz - 16, x1, y1, fz, tex, top=t, bottom=btm))
    for f in range(levels):
        fz = z + f * fh
        fx0, fx1 = flight_x(f)
        if f % 2 == 0:
            stairs(fx0, sy0, fx1, sy1, fz, fz + fh, "n", side="OUT_WD", top="CSTRIKE_FJ2DOCK")
            # a rail beside the stairwell upstairs (stops walking into the hole), open at the top end
            W(box(fx1, sy0, fz + fh, fx1 + 8, sy1 - 64, fz + fh + 40, RAIL))
        else:
            stairs(fx0, sy0, fx1, sy1, fz, fz + fh, "s", side="OUT_WD", top="CSTRIKE_FJ2DOCK")
            W(box(fx0 - 8, sy0 + 64, fz + fh, fx0, sy1, fz + fh + 40, RAIL))
    if roof_access:
        gaps = {"n": [], "s": [], "e": [], "w": []}
        for (side, c, w) in roof_gaps:
            gaps[side].append((c, w, top, top + 40))
        wall(x0, y0, x1, y0 + 8, top, top + 40, tex, gaps["s"], "x")
        wall(x0, y1 - 8, x1, y1, top, top + 40, tex, gaps["n"], "x")
        wall(x0, y0 + 8, x0 + 8, y1 - 8, top, top + 40, tex, gaps["w"], "y")
        wall(x1 - 8, y0 + 8, x1, y1 - 8, top, top + 40, tex, gaps["e"], "y")
    W(box(x0 + wt, y0 + wt, z, x1 - wt, y1 - wt, z + 1, floor_tex))
    if light:
        for f in range(floors):
            point("light", (x0 + x1) / 2, (y0 + y1) / 2, z + f * fh + fh - 40, _light="255 230 190 110")
    return top


def crate(x, y, z, s=64, h=None, big=False):
    h = h or s
    D(box(x, y, z, x + s, y + s, z + h, "SandCrtLrgSd" if big else "MltryCrteSd", top="SandCrtLrgTp" if big else "MltryCrteTp"))


def sandbags(xa, ya, xb, yb, z, h=48):
    D(box(xa, ya, z, xb, yb, z + h, "OUT_SNBAG", top="OUT_SNBAGB"))


def ladder(x0, y0, x1, y1, z0, z1, face):
    """a visible ladder (func_wall) and the climbable volume (func_ladder); face = the side
    a climber stands on: n/s/e/w"""
    lad = Ent("func_wall", rendermode=4, renderamt=255)
    lad.brushes.append(box(x0, y0, z0, x1, y1, z1, "{CSTRIKE_LE6LAD"))
    register(lad.brushes[-1])
    ents.append(lad)
    fl = Ent("func_ladder")
    pad = {"n": (0, 0, 0, 8), "s": (0, -8, 0, 0), "e": (0, 0, 8, 0), "w": (-8, 0, 0, 0)}[face]
    fl.brushes.append(box(x0 + pad[0], y0 + pad[1], z0, x1 + pad[2], y1 + pad[3], z1, "AAATRIGGER"))
    ents.append(fl)


LOOT = []      # (x, y, z, tier, table)
WEAP = []      # (x, y, z, table, tier)

# --- the town (z 64): blocks around the main street (row 17 & 16) and a cross street -------
group("poi:town")
Z = 64
th = building(-1664, 640, -1216, 1024, Z, floors=2, tex="CSTRIKE_WB6PLN2", doors=[("s", 0), ("e", 64)], roof_access=True,
              roof_gaps=[("e", 980, 72)])
LOOT += [(-1440, 832, Z + 8, 3, "default"), (-1400, 880, Z + 160 + 8, 3, "default")]
# town hall: three floors and a roof, the tallest building in the valley
building(-1088, 576, -640, 1152, Z, floors=3, tex="OUT_WALL6", doors=[("s", 0), ("w", 128)], roof_access=True,
         updoors=[(2, "w", 980, 96), (1, "e", 992, 96)])
LOOT += [(-864, 700, Z + 8, 3, "default"), (-800, 900, Z + 168, 4, "default"), (-800, 900, Z + 328, 4, "default")]
WEAP += [(-760, 1000, Z + 488, "snipers", 4)]
building(-448, 640, 0, 1024, Z, floors=1, tex="OUT_WALL3A", doors=[("s", 0), ("n", 0)], roof_access=True,
         roof_gaps=[("w", 980, 72), ("s", -184, 80)])
LOOT += [(-224, 832, Z + 8, 2, "default")]
building(-1664, -192, -1152, 192, Z, floors=1, tex="OUT_WALL1A", doors=[("n", 0), ("e", 0)], roof_access=True)
LOOT += [(-1408, 0, Z + 8, 2, "default")]
building(-1024, -192, -576, 160, Z, floors=2, tex="CSTRIKE_WB6PLN3", doors=[("n", 0), ("e", 0)])
LOOT += [(-800, 0, Z + 168, 3, "default")]
WEAP += [(-760, -80, Z + 8, "smgs", 2)]
building(-448, -192, -64, 160, Z, floors=1, tex="OUT_WALL5A", doors=[("n", 0), ("s", 0), ("e", 0)], roof_access=True,
         roof_gaps=[("n", -184, 80)])
LOOT += [(-200, 0, Z + 8, 2, "gear")]
# the upper deck: walkways between the buildings, off the street
def walkway(name, x0, y0, x1, y1, z, along):
    """a plank deck with its top at z and railings on its long sides (along 'x' or 'y')"""
    keep = GROUP
    group("link:" + name)
    D(box(x0, y0, z - 16, x1, y1, z, WOOD, top=PLANK))
    if along == "x":
        D(box(x0, y0 - 8, z, x1, y0, z + 40, RAIL))
        D(box(x0, y1, z, x1, y1 + 8, z + 40, RAIL))
    else:
        D(box(x0 - 8, y0, z, x0, y1, z + 40, RAIL))
        D(box(x1, y0, z, x1 + 8, y1, z + 40, RAIL))
    group(keep)


walkway("houseA-hall", -1216, 944, -1088, 1016, Z + 320, "x")     # house A's roof to the hall's 3rd floor
walkway("hall-store", -640, 944, -448, 1040, Z + 160, "x")        # the hall's 2nd floor to the store's roof
walkway("main-street", -224, 160, -144, 640, Z + 160, "y")        # over the main street, roof to roof
# cars and crates in the streets
for (x, y) in [(-1408, 336), (-704, 448), (-1500, 1180), (-320, 1184)]:
    D(box(x, y, Z, x + 160, y + 80, Z + 56, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))
crate(-1760, 512, Z, 64)
crate(160, 576, Z, 64)
sandbags(-560, 192, -528, 320, Z)
WEAP += [(-512, 256, Z + 8, "rifles", 3), (-1600, 448, Z + 8, "pistols", 1)]

# --- the factory (z 0) --------------------------------------------------------------------
group("poi:factory")
Z = 0
FX0, FY0, FX1, FY1, FH = -3264, -3264, -2432, -2560, 288
BLOCKED.append((FX0 - 32, FY0 - 32, FX1 + 32, FY1 + 32))
WT = 24
wall(FX0, FY1 - WT, FX1, FY1, Z, FH, "CSTRIKE_ME2RSTW", [(-2848, 192, Z, Z + 176)], "x")
wall(FX0, FY0, FX1, FY0 + WT, Z, FH, "CSTRIKE_ME2RSTW", [(-3008, 128, Z, Z + 128)], "x")
wall(FX0, FY0 + WT, FX0 + WT, FY1 - WT, Z, FH, "CSTRIKE_ME2RSTW", [(-2912, 128, Z, Z + 128)], "y")
wall(FX1 - WT, FY0 + WT, FX1, FY1 - WT, Z, FH, "CSTRIKE_ME2RSTW", [(-2912, 192, Z, Z + 176)], "y")
W(box(FX0, FY0, FH, FX1, FY1, FH + 16, "CSTRIKE_ME2RSTW", top="OUT_ROOF3B", bottom="CSTRIKE_CE2XCNG"))
W(box(FX0 + WT, FY0 + WT, Z, FX1 - WT, FY1 - WT, Z + 1, "CSTRIKE_FE4CRGD"))
ladder(-3136, FY1, -3072, FY1 + 4, 0, FH + 24, "n")
for (a, b, c, d) in [(FX0, FY0, FX1, FY0 + 8), (FX0, FY0 + 8, FX0 + 8, FY1), (FX1 - 8, FY0 + 8, FX1, -2920),
                     (FX1 - 8, -2840, FX1, FY1),   # (a gap to the office walkway)
                     (FX0 + 8, FY1 - 8, -3168, FY1), (-3040, FY1 - 8, FX1 - 8, FY1)]:
    W(box(a, b, FH + 16, c, d, FH + 48, "CSTRIKE_ME2RSTW", top=RAIL))
# catwalk along the south wall at 160, stairs up from the west
D(box(FX0 + WT, FY0 + WT, 144, FX1 - WT, FY0 + WT + 128, 160, RAIL, top="CSTRIKE_FE2GTNG"))
D(box(FX0 + WT, FY0 + WT + 128, 160, FX1 - WT - 160, FY0 + WT + 136, 200, RAIL))
stairs(FX1 - WT - 160, FY0 + WT + 128, FX1 - WT, FY0 + WT + 128 + 220, 0, 160, "s", side=RAIL, top="CSTRIKE_FE2GTNG")
for (x, y) in [(-3072, -2880), (-2880, -2784), (-2688, -2944)]:
    crate(x, y, Z, 128, big=True)
crate(-3168, -2688, Z, 64)
for k in range(4):
    point("light", FX0 + 120 + k * 200, (FY0 + FY1) / 2, FH - 40, _light="255 240 210 160")
LOOT += [(-2960, -2700, 8, 3, "default"), (-3100, -3180, 168, 4, "default"), (-2540, -2700, 8, 2, "gear")]
WEAP += [(-2700, -3180, 168, "rifles", 3)]
# offices: two floors with a roof
building(-2304, -3200, -1920, -2752, Z, floors=2, tex=CONC, doors=[("e", 0), ("n", 0)], roof_access=True,
         roof_gaps=[("w", -2880, 80)])
walkway("factory-office", -2432, -2920, -2304, -2840, FH + 32, "x")   # the hall's roof to the office's roof
LOOT += [(-2112, -2976, 168, 3, "default")]
# containers in the yard
for (x, y, s) in [(-3264, -2496, 0), (-2688, -2496, 1), (-2304, -2496, 0)]:
    D(box(x, y, Z, x + 256, y + 96, Z + 112, "CSTRIKE_OE6TNKS" if s else "CSTRIKE_WE2RIBD", top="CSTRIKE_ME4METL"))
# chimney with a ladder to its top (look-out over the south)
D(box(-2400, -3264, 0, -2304, -3168, 480, "CSTRIKE_WB6PLN1", top="CSTRIKE_ME4METL"))
D(box(-2432, -3296, 480, -2272, -3168, 496, RAIL, top="CSTRIKE_FE2GTNG"))
ladder(-2384, -3168, -2320, -3164, 0, 504, "n")
WEAP += [(-2352, -3216, 504, "snipers", 4)]

# --- the farm (z 32) ----------------------------------------------------------------------
group("poi:farm")
Z = 32
BX0, BY0, BX1, BY1, BH = -3200, 2560, -2624, 3072, 256
BLOCKED.append((BX0 - 32, BY0 - 32, BX1 + 32, BY1 + 32))
wall(BX0, BY1 - 16, BX1, BY1, Z, Z + BH, "OUT_WD", [], "x")
wall(BX0, BY0, BX1, BY0 + 16, Z, Z + BH, "OUT_WD", [(-2912, 192, Z, Z + 176)], "x")
wall(BX0, BY0 + 16, BX0 + 16, BY1 - 16, Z, Z + BH, "OUT_WD", [(2816, 128, Z, Z + 128)], "y")
wall(BX1 - 16, BY0 + 16, BX1, BY1 - 16, Z, Z + BH, "OUT_WD", [(2816, 192, Z, Z + 176)], "y")
W(box(BX0, BY0, Z + BH, BX1, BY1, Z + BH + 16, "OUT_WD", top="OUT_ROOF1B", bottom="OUT_WD"))
W(box(BX0 + 16, BY0 + 16, Z, BX1 - 16, BY1 - 16, Z + 1, "CSTRIKE_FT2DIRT"))
# hay loft over the north half at +144, stairs on the east side
D(box(BX0 + 16, 2848, Z + 128, BX1 - 16, BY1 - 16, Z + 144, "OUT_WD", top="CSTRIKE_FJ2DOCK"))
stairs(BX1 - 16 - 112, 2848 - 200, BX1 - 16, 2848, Z, Z + 144, "n", side="OUT_WD", top="CSTRIKE_FJ2DOCK")
for (x, y) in [(-3136, 2624), (-3000, 2700), (-3136, 2950)]:
    D(box(x, y, Z, x + 96, y + 64, Z + 64, "OUT_SND2", top="OUT_SND2"))   # hay bales
point("light", -2912, 2800, Z + 200, _light="255 230 190 140")
LOOT += [(-2800, 2700, Z + 8, 2, "default"), (-2800, 2960, Z + 152, 3, "default")]
WEAP += [(-3100, 2960, Z + 152, "snipers", 3)]
building(-2304, 2560, -1920, 2944, Z, floors=2, tex="OUT_WALL7", doors=[("s", 0), ("e", 0)], roof_access=False)
LOOT += [(-2112, 2752, Z + 8, 2, "default"), (-2112, 2752, Z + 168, 2, "gear")]
# silo (decor) and fences round the fields
D(box(-2560, 2048, Z, -2368, 2240, Z + 448, "CSTRIKE_OE6TNKF", top="CSTRIKE_ME4METL"))
BLOCKED.append((-2592, 2016, -2336, 2272))
for (a, b, c, d) in [(-3200, 1856, -2688, 1872), (-3200, 1856, -3184, 2368), (-2176, 1856, -1856, 1872), (-1872, 1856, -1856, 2304)]:
    D(box(a, b, Z, c, d, Z + 44, "OUT_WD"))
WEAP += [(-2944, 2112, Z + 8, "smgs", 2)]

# --- radio hill (z 448): a lattice tower with a ladder, a hut -----------------------------
group("poi:radio")
Z = 448
TX, TY = -256, 2880
for (dx, dy) in [(-64, -64), (48, -64), (-64, 48), (48, 48)]:
    D(box(TX + dx, TY + dy, Z, TX + dx + 16, TY + dy + 16, Z + 384, RAIL))
D(box(TX - 96, TY - 96, Z + 384, TX + 96, TY + 96, Z + 400, RAIL, top="CSTRIKE_FE2GTNG"))
for (a, b, c, d) in [(TX - 96, TY - 96, TX + 96, TY - 88), (TX - 96, TY + 88, TX + 96, TY + 96), (TX - 96, TY - 88, TX - 88, TY + 88)]:
    D(box(a, b, Z + 400, c, d, Z + 440, RAIL))
D(box(TX - 8, TY - 8, Z + 400, TX + 8, TY + 8, Z + 640, RAIL))
D(box(TX - 6, TY - 6, Z + 640, TX + 6, TY + 6, Z + 652, "~SPOTRED"))
D(box(TX + 84, TY - 40, Z, TX + 96, TY + 40, Z + 384, RAIL))   # backing behind the ladder
ladder(TX + 96, TY - 32, TX + 100, TY + 32, Z, Z + 408, "e")
BLOCKED.append((TX - 128, TY - 128, TX + 128, TY + 128))
LOOT += [(TX - 48, TY + 48, Z + 408, 5, "default")]
building(-480, 2592, -160, 2784, Z, floors=1, fh=144, tex=CONC, doors=[("e", 0)], windows=True)
LOOT += [(-320, 2688, Z + 8, 3, "gear")]

# --- the gas station (z 0) ----------------------------------------------------------------
group("poi:gas")
Z = 0
for (x, y) in [(-928, -2016), (-928, -1856), (-592, -2016), (-592, -1856)]:
    D(box(x, y, Z, x + 16, y + 16, Z + 160, RAIL))
D(box(-960, -2048, Z + 160, -560, -1824, Z + 176, CONC, top="OUT_ROOF3B", bottom="CSTRIKE_ME1CLNG"))
BLOCKED.append((-960, -2048, -560, -1824))
for x in (-832, -704):
    D(box(x, -1952, Z, x + 32, -1920, Z + 64, "CSTRIKE_ME4RADR"))
building(-1216, -3008, -768, -2624, Z, floors=1, tex="OUT_WALL5A", doors=[("n", 0), ("e", 0)], roof_access=True)
LOOT += [(-992, -2816, 8, 2, "default"), (-760, -1936, 8, 1, "gear")]

# --- the docks (west bank, z 0): boathouse, piers over the river --------------------------
group("poi:docks")
Z = 0
building(-128, -1280, 320, -896, Z, floors=1, fh=176, tex="OUT_WD", doors=[("e", 0), ("s", 0), ("n", 0)], roof_access=True,
         floor_tex="CSTRIKE_FJ2DOCK")
LOOT += [(96, -1088, 8, 2, "default")]
for j in (9, 10, 11):
    xw, xe = river_x(j)
    y = cx(j) + 64
    D(box(xw - 64, y, -16, xw + 224, y + 128, 0, WOOD, top=PLANK))
    for px in (xw + 64, xw + 192):
        D(box(px - 12, y + 52, BED_Z, px + 12, y + 76, -16, WOOD))
    crate(xw + 96, y + 16, 0, 64)
xw, _ = river_x(10)
stairs(xw, cx(10) + 192, xw + 96, cx(10) + 192 + 176, BED_Z, 0, "s", side=WOOD, top=PLANK)
LOOT += [(river_x(11)[0] + 160, cx(11) + 128, 8, 2, "default")]
WEAP += [(river_x(9)[0] + 32, cx(9) + 96, 8, "smgs", 2)]

# --- the quarry (z -128): pit with rock piles, a shed, a crane-like gantry ------------------
group("poi:quarry")
Z = -128
for (x, y, s, h) in [(2432, -448, 160, 128), (3136, -416, 160, 96), (3136, 192, 160, 160), (2688, 0, 96, 176)]:
    D(box(x, y, Z, x + s, y + s, Z + h, ROCK2))
    BLOCKED.append((x - 32, y - 32, x + s + 32, y + s + 32))
building(2368, 96, 2752, 448, Z, floors=1, tex="CSTRIKE_ME2RSTW", doors=[("s", 0), ("e", 0)], roof_access=True)
D(box(2432, 0, Z, 2448, 16, Z + 320, RAIL))
D(box(2432, 0, Z + 320, 2688, 16, Z + 336, RAIL))
LOOT += [(2560, 272, Z + 8, 3, "default"), (2500, -100, Z + 8, 2, "gear")]
WEAP += [(3136, 400, Z + 8, "rifles", 3)]

# --- the bunker (z 160): thick concrete, two rooms, roof with parapet; the complex below ----
group("poi:bunker")
Z = 160
KX0, KY0, KX1, KY1, KH = 2560, -3072, 3072, -2624, 160
BLOCKED.append((KX0 - 32, KY0 - 32, KX1 + 32, KY1 + 32))
wall(KX0, KY1 - 32, KX1, KY1, Z, Z + KH, CONC, [(2688, 128, Z, Z + 128)], "x")
wall(KX0, KY0, KX1, KY0 + 32, Z, Z + KH, CONC, [(2700, 160, Z + 64, Z + 96), (2940, 160, Z + 64, Z + 96)], "x")
wall(KX0, KY0 + 32, KX0 + 32, KY1 - 32, Z, Z + KH, CONC, [(-2848, 128, Z, Z + 128)], "y")
wall(KX1 - 32, KY0 + 32, KX1, KY1 - 32, Z, Z + KH, CONC, [(-2848, 160, Z + 64, Z + 96)], "y")
wall(2800, KY0 + 32, 2832, KY1 - 32, Z, Z + KH, CONC, [(-2848, 128, Z, Z + 128)], "y")
W(box(KX0, KY0, Z + KH, KX1, KY1, Z + KH + 32, CONC, top=CONC_F, bottom=CEIL))
W(box(KX0 + 32, KY0 + 32, Z, KX1 - 32, KY1 - 32, Z + 1, CONC_F))
for (a, b, c, d) in [(KX0, KY0, KX1, KY0 + 16), (KX0, KY1 - 16, KX1, KY1), (KX0, KY0 + 16, KX0 + 16, KY1 - 16), (KX1 - 16, KY0 + 16, KX1, KY1 - 112)]:
    W(box(a, b, Z + KH + 32, c, d, Z + KH + 72, CONC))
stairs(KX1 + 16, KY1 - 448, KX1 + 144, KY1 - 112, Z, Z + KH + 32, "n", side=CONC, top=CONC_F)
D(box(KX1, KY1 - 112, Z, KX1 + 144, KY1 - 16, Z + KH + 32, CONC, top=CONC_F))
point("light", 2680, -2848, Z + 120, _light="255 220 170 140")
point("light", 2950, -2848, Z + 120, _light="255 220 170 140")
LOOT += [(2680, -2848, Z + 8, 4, "default"), (2950, -2900, Z + 8, 5, "default"), (2816, -2700, Z + KH + 40, 3, "gear")]
WEAP += [(2940, -2760, Z + 8, "rifles", 4)]
# trenches of sandbags round it
for (a, b, c, d) in [(2624, -2432, 2816, -2400), (3136, -2432, 3264, -2400), (3264, -3264, 3296, -3008)]:
    sandbags(a, b, c, d, Z)
# the complex below (tunnels bunker_a / bunker_b): crates, a weapon
LOOT += [(2816, -2720, TUN_Z + 8, 5, "default"), (2600, -2850, TUN_Z + 8, 3, "default")]
WEAP += [(3060, -2600, TUN_Z + 8, "snipers", 5)]
crate(2560, -2560, TUN_Z, 64)
crate(3040, -2900, TUN_Z, 64)

# --- the lumber camp (z 128): an open sawmill shed, log piles, a watchtower ----------------
group("poi:lumber")
Z = 128
for (x, y) in [(2432, 2304), (2880, 2304), (2432, 2624), (2880, 2624)]:
    D(box(x, y, Z, x + 24, y + 24, Z + 192, WOOD))
D(box(2400, 2272, Z + 192, 2936, 2680, Z + 208, WOOD, top="OUT_ROOF1B", bottom=WOOD))
BLOCKED.append((2400, 2272, 2936, 2680))
for (x, y) in [(2496, 2400), (2496, 2512), (2720, 2400)]:
    D(box(x, y, Z, x + 320, y + 64, Z + 48, "OUT_WD", top="TNNL_BRD1B"))
for (x, y) in [(3008, 2176), (3008, 2240), (3008, 2304)]:
    D(box(x, y, Z, x + 256, y + 48, Z + 48, "OUT_WD"))
building(2496, 2752, 2880, 3008, Z, floors=1, tex="OUT_WD", doors=[("s", 0), ("e", 0)], roof_access=False)
# watchtower: four posts, platform at +256, ladder on the south side
WX, WY = 3136, 2880
for (dx, dy) in [(-64, -64), (48, -64), (-64, 48), (48, 48)]:
    D(box(WX + dx, WY + dy, Z, WX + dx + 16, WY + dy + 16, Z + 256, WOOD))
D(box(WX - 80, WY - 80, Z + 256, WX + 80, WY + 80, Z + 272, WOOD, top=PLANK))
for (a, b, c, d) in [(WX - 80, WY + 72, WX + 80, WY + 80), (WX - 80, WY - 72, WX - 72, WY + 72), (WX + 72, WY - 72, WX + 80, WY + 72)]:
    D(box(a, b, Z + 272, c, d, Z + 312, WOOD))
D(box(WX - 80, WY - 80, Z + 384, WX + 80, WY + 80, Z + 400, WOOD, top="OUT_ROOF1B"))
for (dx, dy) in [(-80, -80), (64, -80), (-80, 64), (64, 64)]:
    D(box(WX + dx, WY + dy, Z + 272, WX + dx + 16, WY + dy + 16, Z + 384, WOOD))
D(box(WX - 40, WY - 80, Z, WX + 40, WY - 72, Z + 256, WOOD))      # backing behind the ladder
ladder(WX - 32, WY - 84, WX + 32, WY - 80, Z, Z + 280, "s")
BLOCKED.append((WX - 112, WY - 112, WX + 112, WY + 112))
point("light", 2672, 2480, Z + 160, _light="255 230 190 120")
LOOT += [(2672, 2600, Z + 8, 3, "default"), (2688, 2880, Z + 8, 2, "default"), (WX, WY, Z + 280, 4, "default")]
WEAP += [(2560, 2300, Z + 8, "rifles", 3)]

# --- the mesas: the old fort (west) and the lookout (east) ---------------------------------
group("poi:mesas")
z = 352
D(box(-3008, -1216, z, -2624, -1184, z + 96, "-1csSandWall", top="SandTrim"))
D(box(-3008, -608, z, -2816, -576, z + 96, "-1csSandWall", top="SandTrim"))
D(box(-3008, -1184, z, -2976, -800, z + 96, "-1csSandWall", top="SandTrim"))
sandbags(-2752, -832, -2720, -704, z)
crate(-2880, -960, z, 128, big=True)
LOOT += [(-2700, -1000, z + 8, 4, "default")]
WEAP += [(-2900, -680, z + 8, "rifles", 4)]
z = 320
sandbags(2112, 1408, 2304, 1440, z)
crate(2400, 1152, z, 64)
LOOT += [(2300, 1250, z + 8, 3, "default")]
WEAP += [(2180, 1100, z + 8, "snipers", 3)]

# --- cabins ---------------------------------------------------------------------------------
group("poi:cabins")
for k, (x, y) in enumerate(CABINS):
    z = H[ci(x)][ci(y)]
    building(x - 160, y - 128, x + 160, y + 128, z, floors=1, fh=144, tex="OUT_WD" if k % 2 else "OUT_WALL2",
             doors=[("s", 0), ("n", 0)], roof_access=False, floor_tex="CSTRIKE_FJ2DOCK")
    LOOT.append((x + 64, y, z + 8, 1 + (k % 2), "default"))

# ---------------------------------------------------------------------------------------
# cover in the open: rocks and trees (detail), placed on the ground


def blocked(x, y, m=0):
    if abs(x) > PLAY - 192 or abs(y) > PLAY - 192:
        return True
    i, j = ci(x), ci(y)
    for di in (-1, 0, 1):
        for dj in (-1, 0, 1):
            if KIND.get((i + di, j + dj)) in ("river", "cut") or (KIND.get((i + di, j + dj)) or "").startswith("mesa"):
                return True
    if near_road(x, y, m):
        return True
    for name, x0, y0, x1, y1, z in PADS:
        if x0 - m <= x <= x1 + m and y0 - m <= y <= y1 + m:
            return True
    for (x0, y0, x1, y1) in BLOCKED:
        if x0 - m <= x <= x1 + m and y0 - m <= y <= y1 + m:
            return True
    return False


def rnd(seed):
    # a small deterministic random (the map comes out the same every run)
    seed = (seed * 1103515245 + 12345) & 0x7FFFFFFF
    return seed, seed / 0x7FFFFFFF


seed = 7
trees = rocks = bushes = 0


# supply drop spots (open ground; cover keeps 160 clear of them, so a crate never lands in a rock or a tree)
DROPS = [(-1536, 1792), (1024, -2816), (-2048, -1280), (2048, 384), (0, -768), (-512, 1920), (2560, -1600), (-2688, -1664)]


def near_drop(x, y, r):
    return any(math.hypot(x - dx, y - dy) < r + 160 for dx, dy in DROPS)


def tree(x, y, z, u, v):
    """a trunk and irregular foliage from head height up (blocks sight; walk round it)"""
    global trees
    r = 52 + 34 * u
    top = z + 168 + 90 * v
    trunk = box(x - 12, y - 12, z - 32, x + 12, y + 12, z + 64, "OUT_WD", top="OUT_WD")
    leaves = prism([(px, py, top) for (px, py) in blob(x, y, r, 7, int(u * 1e6) + 3, turn=v * 3)], z + 48, "OUT_SHRB1", "OUT_SHRB1")
    group("tree")
    if near_drop(x, y, r) or clashes(trunk, allow=("tree", "rock", "bush", "boulder")) or clashes(leaves, allow=("tree", "rock", "bush", "boulder")):
        return
    # 40 of open ground between canopies (no slots to drop into between them, on a slope least of all)
    room = prism([(px, py, top) for (px, py) in blob(x, y, r + 40, 7, int(u * 1e6) + 3, turn=v * 3)], z - 32, "NULL", "NULL")
    group("tree-room")
    near = clashes(room, allow=("rock", "bush", "boulder"))
    group("tree")
    if any(o.group == "tree" for o in near):
        return
    D(trunk)
    D(leaves)
    BLOCKED.append((x - r - 8, y - r - 8, x + r + 8, y + r + 8))
    trees += 1


def rock(x, y, z, r, h, k):
    """a boulder: an irregular block with a tilted top"""
    global rocks
    poly = blob(x, y, r, 5 + k % 3, k, squash=0.65 + 0.35 * ((k * 7) % 10) / 10.0, turn=k * 0.7)
    ax, ay = 0.25 * math.cos(k), 0.25 * math.sin(k * 1.7)
    b = prism([(px, py, z + h + ax * (px - x) + ay * (py - y)) for (px, py) in poly], z - 48, ROCK2, ROCK2)
    group("rock")
    if near_drop(x, y, r) or clashes(b, allow=("rock",)):
        return
    D(b)
    BLOCKED.append((x - r - 24, y - r - 24, x + r + 24, y + r + 24))
    rocks += 1


def ground4(x, y, r):
    return min(ground(x - r, y - r), ground(x + r, y + r), ground(x - r, y + r), ground(x + r, y - r))


# boulders against the banks (not where the river is crossed, never into a pier or stairs)
group("boulder")
seed_r = 99
for j in range(1, N - 1):
    if j in KEEP_STRAIGHT:
        continue
    xw, xe = river_x(j)
    for side in (-1, 1):
        seed_r = (seed_r * 1103515245 + 12345) & 0x7FFFFFFF
        u = seed_r / 0x7FFFFFFF
        if u > 0.55:
            continue
        r = 56 + 60 * u
        x = (xw + r * 0.6) if side < 0 else (xe - r * 0.6)
        y = cx(j) + 40 + u * 170
        b = prism([(px, py, BED_Z + 40 + 90 * u) for (px, py) in blob(x, y, r, 6, seed_r)], BED_Z - 16, ROCK2, ROCK2)
        # not near a 45-degree bank corner either: the slot between them would trap a player
        near_corner = any(min(p[0] for p in t) - r - 64 < x < max(p[0] for p in t) + r + 64
                          and min(p[1] for p in t) - r - 64 < y < max(p[1] for p in t) + r + 64 for t in CHAMFERS)
        if not near_corner and not clashes(b):
            D(b)

# rock formations: a few big stones with smaller ones round them; some standing stones
for f in range(60):
    seed, u = rnd(seed)
    seed, v = rnd(seed)
    seed, w_ = rnd(seed)
    fx, fy = -PLAY + 300 + u * (2 * PLAY - 600), -PLAY + 300 + v * (2 * PLAY - 600)
    if blocked(fx, fy, 48):
        continue
    big = 90 + 70 * w_
    rock(fx, fy, ground4(fx, fy, big), big, 70 + 110 * w_, f * 13 + 5)
    for k in range(2 + int(3 * w_)):
        seed, a = rnd(seed)
        seed, d = rnd(seed)
        x, y = fx + math.cos(a * 6.283) * (big + 70 + 90 * d), fy + math.sin(a * 6.283) * (big + 70 + 90 * d)
        if blocked(x, y, 24):
            continue
        r = 44 + 50 * d
        rock(x, y, ground4(x, y, r), r, 40 + 50 * a, f * 31 + k)
# groves: clusters of trees of different sizes at irregular places, a few loose trees between
for g in range(90):
    seed, u = rnd(seed)
    seed, v = rnd(seed)
    seed, w_ = rnd(seed)
    gx_, gy_ = -PLAY + 300 + u * (2 * PLAY - 600), -PLAY + 300 + v * (2 * PLAY - 600)
    rad = 220 + 520 * w_
    for t_ in range(int(4 + 12 * w_)):
        seed, a = rnd(seed)
        seed, d = rnd(seed)
        x, y = gx_ + math.cos(a * 6.283) * rad * math.sqrt(d), gy_ + math.sin(a * 6.283) * rad * math.sqrt(d)
        if blocked(x, y, 56):
            continue
        tree(x, y, ground4(x, y, 64), a, d)
for t_ in range(520):
    seed, u = rnd(seed)
    seed, v = rnd(seed)
    x, y = -PLAY + 250 + u * (2 * PLAY - 500), -PLAY + 250 + v * (2 * PLAY - 500)
    if blocked(x, y, 64):
        continue
    tree(x, y, ground4(x, y, 64), u, v)
# bushes: low, to crouch behind, along the groves' edges and the roads' sides
for b_ in range(360):
    seed, u = rnd(seed)
    seed, v = rnd(seed)
    x, y = -PLAY + 250 + u * (2 * PLAY - 500), -PLAY + 250 + v * (2 * PLAY - 500)
    if blocked(x, y, 40):
        continue
    z = ground4(x, y, 40)
    b = prism([(px, py, z + 52) for (px, py) in blob(x, y, 46, 6, b_ * 17 + 1)], z - 16, "OUT_SHRB1", "OUT_SHRB1")
    group("bush")
    # a bush stands clear of trees and rocks (40 round it): no slots between them to drop into
    room = prism([(px, py, z + 52) for (px, py) in blob(x, y, 86, 6, b_ * 17 + 1)], z - 16, "NULL", "NULL")
    if near_drop(x, y, 46) or clashes(b, allow=("bush",)) or clashes(room, allow=("bush",)):
        continue
    D(b)
    BLOCKED.append((x - 70, y - 70, x + 70, y + 70))
    bushes += 1

# ---------------------------------------------------------------------------------------
# mode data: spawns, loot, floor weapons, supply drops, the zone


def spread(points, k, start):
    """farthest-point picks of k from points"""
    chosen = [start]
    while len(chosen) < k:
        best = max(points, key=lambda p: min((p[0] - c[0]) ** 2 + (p[1] - c[1]) ** 2 for c in chosen))
        chosen.append(best)
    return chosen


cands = []
for xg in range(-PLAY + 384, PLAY - 383, 128):
    for yg in range(-PLAY + 384, PLAY - 383, 128):
        if blocked(xg, yg, 64):
            continue
        zs = [ground(xg + dx, yg + dy) for dx in (-32, 32) for dy in (-32, 32)]
        if max(zs) - min(zs) > 24:
            continue
        cands.append((xg, yg, max(zs)))
picks = spread(cands, 28 + 32, cands[len(cands) // 2])
for k, (x, y, z) in enumerate(picks[:28]):
    ang = int(math.degrees(math.atan2(-y, -x))) % 360        # face the middle
    point("ls_respawn_point", x, y, z + 36, protect_secs=3, angle=ang)
# the game's own spawn points (it needs some per team; gm_components moves players to the above)
for k, (x, y, z) in enumerate(picks[28:]):
    point("info_player_start" if k % 2 else "info_player_deathmatch", x, y, z + 36, angle=0)

# loot crates in the places (above), plus field crates in the open
field = spread(cands, 23, (0, 0, 0))[1:]
for k, (x, y, z) in enumerate(field):
    if k < 10:
        LOOT.append((x + 40, y + 40, z + 8, 1, "default"))
    else:
        mid = math.hypot(x, y) < 2200
        WEAP.append((x - 40, y - 40, z + 8, ("rifles" if mid else "smgs") if k % 3 else "pistols", 3 if mid and k % 3 else 2 if k % 3 else 1))
for (x, y, z, tier, table) in LOOT:
    point("ls_loot_container", x, y, z, table=table, tier=tier, items=3 if tier >= 4 else 2)
for (x, y, z, table, tier) in WEAP:
    point("ls_weapon_spawner", x, y, z, item="table:" + table, tier=tier, respawn_secs=0)
# supply drop spots: open ground, away from the edge
for (x, y) in DROPS:
    point("ls_supply_drop", x, y, ground(x, y) + 8, at=-1, table="drop", fall_secs=15)
# the zone: centred over the valley, high up (the shrinker's random centres look for open air
# 64 units above the zone's height, so it sits above every hill), covering the whole play area
point("ls_safe_zone", 0, 0, 1536, radius=5100, damage=1, targetname="brzone")
point("ls_zone_shrinker", 0, 0, 1536, zone="brzone", start_delay=-1, random_center=1,
      stages="3400:60:50:1 2200:50:45:2 1300:45:40:4 700:40:30:6 300:30:25:10 0:25:25:15")

# one buy zone, out of reach high over the backdrop: a map without any gets one round every
# spawn point, and bots that spawn in one stand in their buy state (nothing to buy in BR)
bz = Ent("func_buyzone", team=0)
bz.brushes.append(box(3680, 3680, 3360, 3744, 3744, 3424, "AAATRIGGER"))
ents.append(bz)

# ---------------------------------------------------------------------------------------
# light and sound
point("light_environment", 0, 0, 2048, pitch=-52, angles="0 140 0", _light="255 238 208 280",
      _diffuse_light="150 178 220 110")
point("ambient_generic", 0, 0, 900, message="ambience/wind1.wav", health=3, spawnflags=1, pitch=100)
point("ambient_generic", -2816, 2560, 100, message="ambience/crickets.wav", health=3, spawnflags=4, pitch=100)
point("ambient_generic", -900, -1000, TUN_Z + 64, message="ambience/drips.wav", health=3, spawnflags=4, pitch=100)

# ---------------------------------------------------------------------------------------
# placement check: nothing may cut into anything else (props, stairs, piers, bridges, buildings)
BAD = check_placement()
for g1, g2, b1, b2 in BAD[:60]:
    print("OVERLAP %-22s %-22s %s %s" % (g1, g2, b1, b2))
if BAD and not os.environ.get("ALLOW_OVERLAP"):
    print("%d overlaps: fix the placements above (ALLOW_OVERLAP=1 writes the map anyway)" % len(BAD))
    raise SystemExit(1)
SUNK = check_ground()
for g1, b1, (x, y, d) in SUNK[:60]:
    print("GROUND %-22s %s: the ground is %d above its top at %d %d" % (g1, b1, d, x, y))
if SUNK and not os.environ.get("ALLOW_OVERLAP"):
    print("%d structures with the ground rising into them (ALLOW_OVERLAP=1 writes the map anyway)" % len(SUNK))
    raise SystemExit(1)

here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, NAME + ".map"), "w") as f:
    f.write("// %s: generated by build_map.py (edit that, or this in the map editor)\n" % NAME)
    for i, e in enumerate(ents):
        f.write(e.text(i) + "\n")
nb = sum(len(e.brushes) for e in ents)
print("%s.map: %d world brushes, %d detail brushes, %d brushes in all, %d entities, %d trees, %d rocks, %d bushes, %d loot, %d weapons"
      % (NAME, len(world.brushes), len(detail.brushes), nb, len(ents), trees, rocks, bushes, len(LOOT), len(WEAP)))

# ---------------------------------------------------------------------------------------
# preview.png: height colours, river, roads, pads, buildings, spawns (no lighting)
S = 4    # pixels per 32 units -> 7680/32*... keep it 960 px
PX = 960
img = [[(0, 0, 0)] * PX for _ in range(PX)]


def to_px(x, y):
    return int((x + E) / (2 * E) * PX), int((E - y) / (2 * E) * PX)


for py in range(PX):
    for px_ in range(PX):
        x = -E + (px_ + 0.5) / PX * 2 * E
        y = E - (py + 0.5) / PX * 2 * E
        i, j = ci(x), ci(y)
        if not (0 <= i < N and 0 <= j < N):
            continue
        k = KIND.get((i, j))
        if k == "river":
            c = (70, 110, 150)
        elif k == "cut":
            c = (40, 40, 40)
        else:
            z = ground(x, y)
            t = max(0.0, min(1.0, (z + 160) / 900.0))
            c = (int(60 + 150 * t), int(120 + 100 * t), int(50 + 120 * t))
            if min(i, j, N - 1 - i, N - 1 - j) == 0:
                c = (110, 100, 95)
        img[py][px_] = c


def rect(x0, y0, x1, y1, c, fill=False):
    a, b = to_px(x0, y1)
    cc, d = to_px(x1, y0)
    for py in range(max(0, b), min(PX, d + 1)):
        for px_ in range(max(0, a), min(PX, cc + 1)):
            if fill or py in (b, d) or px_ in (a, cc):
                img[py][px_] = c


for name, x0, y0, x1, y1 in TUNNELS:
    rect(x0, y0, x1, y1, (200, 60, 200))
for w, tex, line in ROADLINES:
    for (x, y) in line:
        rect(x - w / 2, y - w / 2, x + w / 2, y + w / 2, (176, 156, 112) if tex == ROAD else (150, 125, 90), fill=True)
for b in BLOCKED:
    rect(*b, (40, 30, 30), fill=True)
for e in ents:
    o = e.keys.get("origin")
    if not o:
        continue
    x, y, _ = (float(v) for v in o.split())
    col = {"ls_respawn_point": (255, 255, 255), "ls_loot_container": (255, 200, 0), "ls_weapon_spawner": (255, 80, 0),
           "ls_supply_drop": (0, 200, 255)}.get(e.keys["classname"])
    if col:
        rect(x - 40, y - 40, x + 40, y + 40, col, fill=True)
raw = b"".join(b"\0" + bytes(v for p in row for v in p) for row in img)
png = b"\x89PNG\r\n\x1a\n"


def chunk(t, d):
    return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)


png += chunk(b"IHDR", struct.pack(">IIBBBBB", PX, PX, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
with open(os.path.join(here, "preview.png"), "wb") as f:
    f.write(png)
