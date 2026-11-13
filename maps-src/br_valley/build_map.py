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
MAXD = 160                  # largest height step between neighbouring vertices (256 apart): 32 degrees (v5: 112, 24)

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
# shown and solid to bullets and sight (hull 0), not in the clip hulls: stair steps (a CLIP ramp over each flight
# is what you walk on) and tree foliage (a CLIP box is what you bump into). Saves clipnodes, and stairs walk smoothly
detail_nc = Ent("func_detail", zhlt_detaillevel=1, zhlt_noclip=1)
ents.append(detail_nc)


def frustum(poly, zb, zt, s, tex, bottom=None, scale=1):
    """a convex brush: the polygon at zb, the same scaled by s round its middle at zt (s 0: a point, a cone)"""
    b = Brush()
    cx_ = sum(p[0] for p in poly) / len(poly)
    cy_ = sum(p[1] for p in poly) / len(poly)
    bot = [(p[0], p[1], zb) for p in poly]
    top = [(cx_ + (p[0] - cx_) * s, cy_ + (p[1] - cy_) * s, zt) for p in poly]
    b.face(bot[:3], (0, 0, -1), bottom or tex, scale)
    if s > 0:
        b.face(top[:3], (0, 0, 1), tex, scale)
    k = len(poly)
    for i in range(k):
        p, q = bot[i], bot[(i + 1) % k]
        t = top[i] if s > 0 else (cx_, cy_, zt)
        n = cross(sub(q, p), sub(t, p))
        mx, my = (p[0] + q[0]) / 2 - cx_, (p[1] + q[1]) / 2 - cy_
        if n[0] * mx + n[1] * my < 0:
            n = (-n[0], -n[1], -n[2])
        b.face([p, q, t], n, tex, scale)
    b.bounds = (min(p[0] for p in poly), min(p[1] for p in poly), zb, max(p[0] for p in poly), max(p[1] for p in poly), zt)
    b.poly = [(p[0], p[1]) for p in poly]
    b.top3 = top[:3] if s > 0 else [(p[0], p[1], zt) for p in poly[:3]]
    return b


def regular(x, y, r, n, turn=0.0):
    """a regular n-gon round x, y (counter-clockwise)"""
    return [(x + r * math.cos(turn + 2 * math.pi * k / n), y + r * math.sin(turn + 2 * math.pi * k / n)) for k in range(n)]


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


def NC(b):
    """shown, solid to bullets, not to movement (see detail_nc)"""
    detail_nc.brushes.append(b)
    register(b)
    b.noclip = True


def P(b):
    """a prop: a detail brush that has to sit on something (checked for floating and buried)"""
    D(b)
    b.prop = True


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
        if b.group.startswith(EXEMPT) or b.group in ("rock", "tree", "bush", "boulder") or getattr(b, "kind", None) == "ramp":
            continue          # (a CLIP ramp's foot may meet a cross-slope: it's invisible; its steps are checked)
        (ax, ay, az), (bx_, by, bz), (cx_, cy, cz) = b.top3
        n = cross((bx_ - ax, by - ay, bz - az), (cx_ - ax, cy - ay, cz - az))
        if abs(n[2]) < 1e-9:
            continue
        x0, y0, _, x1, y1, _ = b.bounds
        if getattr(b, "tread", None):
            x0, y0, x1, y1 = b.tread
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
BED_STAIRS = [4, 14]        # more stairs out of the bed (besides those by the bridges)


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
    ("bunker", 2304, -3328, 3328, -2304, 64),
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

# the level below (z TUN_Z..SLAB_TOP; v4): named places joined by tunnels, each with its own look, light, sound
# and way up. Tunnels: (name, x0, y0, x1, y1), floor TUN_Z
TUNNELS = [
    ("sewer", -1664, 320, 256, 512),             # the town's sewer, under the main street (a channel down its middle)
    ("sewer_s", -992, -1952, -800, 320),         # the old sewer south: town - cisterns - the gas station's tank vault
    ("gas_w", -2752, -2016, -800, -1824),        # gas station west to the factory
    ("factory", -2752, -2560, -2560, -1408),     # the factory's service tunnel: boiler house - west tunnel - fort caves
    ("river", -800, -2016, 2944, -1824),         # under the river to the bunker
    ("gas_s", -1280, -2560, -1088, -1824),       # gas station trench to the west tunnel
    ("docks_s", 0, -1824, 192, -1536),           # the river tunnel up to the boathouse cellar
    ("bunker_b", 2688, -2496, 2944, -1824),      # the bunker's north tunnel (a 192 opening into the guard room)
    ("quarry", 2816, -1824, 3008, -1152),        # bunker north to the quarry workings
    ("drain_n", -576, 512, -384, 1920),          # storm drain: the town sewer north to the deep chamber
    ("mine", -2240, 2112, 2432, 2304),           # the mine gallery: farm - deep chamber - under the river - lumber caves
    ("mine_n", -1600, 2304, -1408, 2432),        # the gallery up to the north caves
    ("pumpline", 256, 352, 1536, 480),           # the pump line: sewer east, under the river, to the pumping works
    ("vault_way", -912, 512, -816, 640),         # a grate-floored passage from the sewer to the town vault (you hear it)
    ("armoury_way", 3008, -3008, 3104, -2944),   # the same into the bunker's armoury
    ("stope_way", -896, 2816, -800, 2880),       # and into the deep stope
    ("ridge_way", 1920, -1824, 2112, -1664),     # the river tunnel up into the ridge caves
    ("drain_e", 3072, 1408, 3264, 1920),         # the east drain: the quarry workings north to the lumber caves
]
# places: (name, x0, y0, x1, y1, style, floor). Styles: what it looks like, how it's lit, what you hear (PSTYLE)
PLACES = [
    ("fort caves", -3328, -1408, -2432, 1408, "cave", TUN_Z),        # natural caves under the fort and West Peak
    ("west mine", -3328, 1664, -2048, 2560, "mine", TUN_Z),          # the old mine's west workings (the farm adit)
    ("north caves", -2048, 2432, -1280, 3200, "cave", TUN_Z),        # caves under the north road
    ("deep chamber", -1152, 1920, -128, 2816, "cave", TUN_Z),        # the deep chamber under radio hill's flank
    ("deep stope", -896, 2880, -384, 3264, "vault", -704),           # ...and its stope, a level down behind a grate (tier 5)
    ("lumber caves", 2432, 1792, 3328, 2816, "cave", TUN_Z),         # caves under the lumber camp
    ("cisterns", -1664, -1280, -128, -128, "cistern", -576),         # the town's cistern hall: columns, a water channel
    ("town vault", -1088, 640, -640, 1024, "vault", TUN_Z),          # under the town hall (tier 5), one grate way in
    ("boiler house", -3328, -3328, -2304, -2560, "utility", TUN_Z),  # the factory's basement: boilers, pipes
    ("tank vault", -1536, -3328, -640, -2560, "utility", TUN_Z),     # the gas station's fuel tanks
    ("boathouse cellar", -128, -1536, 512, -768, "pump", TUN_Z),     # the docks' cellar (a hatch in the yard)
    ("pumping works", 1536, -1024, 2304, 1024, "pump", TUN_Z),       # the river pumping station's works
    ("ridge caves", 1664, -2304, 2304, -1152, "cave", TUN_Z),        # caves under the east ridge
    ("quarry workings", 2432, -1152, 3328, 1536, "works", TUN_Z),    # the quarry's old workings
    ("bunker hall", 2304, -2944, 3136, -2688, "bunker", TUN_Z),      # the bunker: a long hall,
    ("guard room", 2304, -2688, 2688, -2304, "bunker", TUN_Z),       # the guard room by the bunker's stair trench,
    ("generator room", 2816, -2688, 3200, -2368, "bunker", TUN_Z),   # the generator room,
    ("armoury", 2944, -3328, 3264, -3008, "vault", -704),            # and the armoury a level down (tier 5)
]
PSTYLE = {   # walls, floor, lamp colour, lamp texture, sound
    "cave": (ROCK, MUD, "255 190 120", "~SPOTYELLOW", "ambience/drips.wav"),
    "mine": (ROCK, MUD, "255 190 120", "~SPOTYELLOW", "ambience/squeeks1.wav"),
    "works": (ROCK2, "OUT_GRAVEL", "235 235 220", "~LIGHT3A", "ambience/industrial2.wav"),
    "bunker": (CONC, CONC_F, "170 200 255", "~SPOTBLUE", "ambience/techamb1.wav"),
    "vault": (CONC, CONC_F, "255 120 100", "~SPOTRED", "ambience/labdrone2.wav"),
    "utility": (TUNW, "-0TNNL_FLR1", "255 165 80", "~LIGHT5A", "ambience/dronemachine1.wav"),
    "cistern": ("-0TNNL_W12A", "-0TNNL_FLR1B", "170 235 190", "~SPOTGREEN", "ambience/waterfall1.wav"),
    "sewer": ("-0TNNL_W13A", "-0TNNL_FLR7", "170 235 190", "~SPOTGREEN", "ambience/drips.wav"),
    "pump": ("CSTRIKE_ME2RSTW", CONC_F, "150 220 255", "~SPOTBLUE", "ambience/pumper.wav"),
    "grate": (CONC, "CSTRIKE_FE2GTNG", "255 120 100", "~SPOTRED", None),
}
TUNNEL_STYLE = {"stope_way": "grate", "drain_e": "works", "sewer": "sewer", "sewer_s": "sewer", "drain_n": "sewer", "mine": "mine", "mine_n": "mine", "vault_way": "grate",
                "armoury_way": "grate", "pumpline": "pump", "docks_s": "pump", "factory": "utility", "gas_w": "utility",
                "gas_s": "utility", "river": "sewer", "bunker_b": "bunker", "quarry": "works", "ridge_way": "cave"}


def style_at(x, y):
    """the style of the place (or tunnel) under x, y; outside them by region (the solid rock between)"""
    for (n_, x0, y0, x1, y1, st, fl) in PLACES:
        if x0 <= x < x1 and y0 <= y < y1:
            return st
    for (n_, x0, y0, x1, y1) in TUNNELS:
        if x0 <= x < x1 and y0 <= y < y1:
            return TUNNEL_STYLE.get(n_, "utility")
    return "cave" if (x < -2240 or y > 1600 or x > 2368) else "utility"


def zone_style(x, y):
    """rock (caves, mine, workings) or built (the rest): for ceilings, portals"""
    st = style_at(x, y)
    return "cave" if st in ("cave", "mine") else "works" if st == "works" else st


# stair trenches down: (cells x0, y0, x1, y1 in cell units, direction the stairs go DOWN)
TRENCHES = [
    ("town", 15, 13, 16, 17, "n"),      # town south-east corner, down northwards to the sewer
    ("gas", 10, 5, 14, 6, "w"),         # behind the gas station, down westwards
    ("factory", 3, 6, 4, 10, "n"),      # factory yard, down northwards into the fort caves (four cells: 33.7 degrees)
    ("bunker", 24, 2, 25, 6, "n"),      # bunker yard, down northwards
    ("quarry", 26, 13, 27, 16, "s"),    # quarry floor, down southwards
    ("farm", 5, 22, 6, 26, "s"),        # the mine's adit in the farmyard, down southwards to the west mine
    ("drain", 12, 20, 16, 21, "w"),     # the town's north edge, down westwards into the storm drain (to the deep chamber)
    ("pumping", 22, 14, 23, 18, "s"),   # the pumping station, down southwards into the pumping works
]
# ladder hatches: (i, j, what stands over it). A 96 hole with a ladder on its north wall, the cell made flat;
# every one under a small structure that says what it is (and keeps a camper from covering the hole from
# every side): kiosk (a brick storm-drain kiosk), pump (a pump house), headframe (a mine's timber head-frame),
# shed (a tin shed)
HATCH_KIND = {(13, 16): "kiosk",        # the main street, into the sewer
              (6, 7): "shed",           # by the factory road, into the west tunnel
              (8, 16): "kiosk",         # the sewer's west end, by house A
              (11, 13): "kiosk",        # south of the town, into the cisterns
              (18, 23): "pump", (21, 23): "pump",   # pump houses on both river banks over the mine gallery
              (25, 23): "headframe",    # the lumber camp, into the lumber caves
              (9, 26): "headframe",     # the north road, into the north caves
              (5, 9): "shed",           # by the west track south-east of the fort, into the fort caves
              (22, 13): "pump",         # the pumping station east of the river, into its works
              (15, 9): "shed",          # the docks' yard, into the boathouse cellar
              (9, 2): "shed"}           # south-west of the gas station, into the tank vault
HATCHES = list(HATCH_KIND)

# ---------------------------------------------------------------------------------------
# the height grid

H = [[0.0] * (N + 1) for _ in range(N + 1)]
FIX = [[False] * (N + 1) for _ in range(N + 1)]


def noise(x, y):
    return (24 * math.sin(x / 610.0 + 1.3) * math.cos(y / 530.0 + 0.4) + 16 * math.sin((x + y) / 380.0)
            + 10 * math.cos((x - 2 * y) / 290.0))


# v4: ridges between the places (you crest one to see the next place; no long sightlines across the valley)
# and sunken lanes along three tracks (gullies with banks you can walk out of anywhere: slopes stay under 24
# degrees). (x0, y0, x1, y1, height or depth, half width)
RIDGES = [(-1350, -1450, -150, -1250, 299, 360),      # between the town and the gas station
          (-3050, 1300, -1850, 1500, 260, 340),       # between the fort / West Peak and the farm
          (-450, -3300, 700, -2600, 273, 380),         # south of the gas station to the river
          (1600, 1700, 1950, 2500, 221, 300),          # east bank, north: the viaduct's branch stairs come down it
          (-1500, 1900, -900, 1700, 208, 300),         # north of the town, between the storm drain and the north road
          (1250, -3200, 2000, -2600, 234, 340),        # the south-east, between the river and the bunker
          (2080, -1000, 2120, 850, 286, 260),          # (v5) the east field, between the pumping station and the quarry
          (-2150, 1650, -1650, 2050, 260, 280)]        # (v5) the gap between the farm and the town
GULLIES = [(-1000, -1930, -420, -1100, 130, 230),      # the gas station - docks track: a sunken lane (a crouched player hides in it)
           (-2480, -1650, -2250, -600, 110, 210),       # the west track past the fort
           (-1500, 2330, -400, 2420, 110, 210)]         # the north road under radio hill


def seg_d(x, y, x0, y0, x1, y1):
    dx, dy = x1 - x0, y1 - y0
    t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy or 1.0)))
    return math.hypot(x - x0 - t * dx, y - y0 - t * dy)


for i in range(N + 1):
    for j in range(N + 1):
        x, y = cx(i), cx(j)
        h = noise(x, y) + 24
        for hx, hy, hh, hr in HILLS:
            d2 = ((x - hx) ** 2 + (y - hy) ** 2) / (hr * hr)
            h += hh * math.exp(-d2 * 1.6)
        for (x0, y0, x1, y1, hh, hw) in RIDGES:
            h += hh * math.exp(-(seg_d(x, y, x0, y0, x1, y1) / hw) ** 2 * 1.6)
        for (x0, y0, x1, y1, hh, hw) in GULLIES:
            h -= hh * math.exp(-(seg_d(x, y, x0, y0, x1, y1) / hw) ** 2 * 1.6)
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
# hatch cells flat at their own height (the hatch is drawn as flat boxes)
for (i, j) in HATCHES:
    _fixed = [H[i + a][j + b] for a in (0, 1) for b in (0, 1) if FIX[i + a][j + b]]
    fix_rect(cx(i), cx(j), cx(i + 1), cx(j + 1), max(_fixed) if _fixed else round(sum(H[i + a][j + b] for a in (0, 1) for b in (0, 1)) / 32) * 8)
# the backdrop ring: tall cliffs, fixed
for i in range(N + 1):
    for j in range(N + 1):
        if min(i, j, N - i, N - j) == 0:
            x, y = cx(i), cx(j)
            side_ = 0 if j == N else 1 if i == N else 2 if j == 0 else 3
            H[i][j] = (1250 + 260 * math.sin(x / 700.0 + y / 900.0 + side_) + 180 * math.cos(y / 400.0 - x / 1300.0)
                       + 150 * math.sin(x / 233.0 + y / 377.0 + 2.1 * side_) + 110 * math.cos(x / 131.0 - y / 171.0 + side_)
                       + (260 if (i * 7 + j * 13) % 5 == 0 else 0))
            # notches: the river's gorge in the north and out in the south, the main road's cut in the west and east
            if (j == N and 17 <= i <= 20) or (j == 0 and 15 <= i <= 18):
                H[i][j] = 240
            if (i == 0 or i == N) and 14 <= j <= 15:
                H[i][j] = 420
            FIX[i][j] = True


def bound_by_fixed():
    """every free vertex within reach of the fixed ones: at most MAXD a cell (8 ways) up or down from any
    pad, bank or trench rim (a hill can't stand right beside the quarry pit and make a cliff of its rim);
    where two fixed places disagree, the middle"""
    lo = [[-1e9] * (N + 1) for _ in range(N + 1)]
    hi = [[1e9] * (N + 1) for _ in range(N + 1)]
    for i in range(1, N):
        for j in range(1, N):
            if not FIX[i][j]:
                continue
            for a in range(1, N):
                for b in range(1, N):
                    d = max(abs(a - i), abs(b - j))
                    lo[a][b] = max(lo[a][b], H[i][j] - MAXD * d)
                    hi[a][b] = min(hi[a][b], H[i][j] + MAXD * d)
    for i in range(1, N):
        for j in range(1, N):
            if FIX[i][j]:
                continue
            if lo[i][j] > hi[i][j]:
                H[i][j] = (lo[i][j] + hi[i][j]) / 2
            else:
                H[i][j] = min(max(H[i][j], lo[i][j]), hi[i][j])


def relax():
    bound_by_fixed()
    for _ in range(3000):
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
    if min(i, j, N - 1 - i, N - 1 - j) == 0:      # the cliff ring: a look per side
        return ROCK2 if (i == 0 or i == N - 1) else ROCK
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
            flat[(i, j)] = (hs[0], cell_tex(i, j, 0), zone_style(cx(i) + 128, cx(j) + 128) in ("cave", "works"))
            continue
        p00 = (cx(i), cx(j), H[i][j])
        p10 = (cx(i + 1), cx(j), H[i + 1][j])
        p01 = (cx(i), cx(j + 1), H[i][j + 1])
        p11 = (cx(i + 1), cx(j + 1), H[i + 1][j + 1])
        for tri in ([p00, p10, p11], [p00, p11, p01]):
            n = cross(sub(tri[1], tri[0]), sub(tri[2], tri[0]))
            slope = math.hypot(n[0], n[1]) / abs(n[2])
            W(prism(tri, SLAB_TOP, cell_tex(i, j, slope), ROCK, bottom=ROCK if zone_style(cx(i) + 128, cx(j) + 128) in ("cave", "works") else CEIL, scale=2))

# mesa tops
for (i, j), k in list(KIND.items()):
    if k.startswith("mesa:"):
        flat[(i, j)] = (int(k[5:]), DIRT, zone_style(cx(i) + 128, cx(j) + 128) in ("cave", "works"))


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


for i0, j0, i1, j1, (z, tex, rocky) in merge_cells(flat):      # (the ceiling below: rock over caves and workings)
    W(box(cx(i0), cx(j0), SLAB_TOP, cx(i1), cx(j1), z, ROCK, top=tex, bottom=ROCK if rocky else CEIL, scale=2))

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
bed = {c: zone_style(cx(c[0]) + 128, cx(c[1]) + 128) in ("cave", "works") for c in RIVER}
for i0, j0, i1, j1, rocky in merge_cells(bed):
    W(box(cx(i0), cx(j0), SLAB_TOP, cx(i1), cx(j1), BED_Z, ROCK, top=MUD, bottom=ROCK if rocky else CEIL, scale=2))

# water in the river: 28 over the bed (a standing player stays dry: no swimming, no slowdown, wading sounds);
# liquids aren't in the clip hulls; not registered (nobody stands on water)
for i0, j0, i1, j1, _r in merge_cells({c: 1 for c in RIVER}):
    world.brushes.append(box(cx(i0), cx(j0), BED_Z, cx(i1), cx(j1), BED_Z + 28, "!WATERBLUE"))
for j in (4, 11, 18, 25):
    xw_, xe_ = cx(SPAN[j][0]), cx(SPAN[j][1])
    point("ambient_generic", (xw_ + xe_) / 2, cx(j) + 128, BED_Z + 64, message="ambience/waterfall2.wav", health=3, spawnflags=4, pitch=100)
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
            W(prism([(px, py, 0) for (px, py) in tri], SLAB_TOP, GRASS, ROCK, bottom=ROCK if zone_style(x0 + 128, y0 + 128) in ("cave", "works") else CEIL, scale=2))
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


# connectors between levels (stairs, ladders, ramps): the validator checks that both ends meet a
# floor a player can stand on, with head room, and that the floor goes on past the end
CONN = []
OUT = {"n": (0, 1), "s": (0, -1), "e": (1, 0), "w": (-1, 0)}


def connector(kind, name, bottom, bdir, top, tdir):
    """bottom/top: (x, y, feet z) just off each end (18 out: half a player and a bit); bdir/tdir: the way a
    player walks on from there"""
    CONN.append({"kind": kind, "name": name, "group": GROUP, "bottom": [round(v, 1) for v in bottom], "bdir": bdir,
                 "top": [round(v, 1) for v in top], "tdir": tdir})


def stairs(xa, ya, xb, yb, z0, z1, toward, side=CONC, top="OUT_PAVE1", ent=None, name=None, out_top=None, out_bottom=None):
    """steps filling xa..xb, ya..yb, rising from z0 to z1 toward 'n','s','e','w' (as ls_outpost). The last
    step's top is exactly z1 (the first one may be lower than 16). out_top / out_bottom: the way a player
    leaves at that end if it isn't straight on (stairs against a wall that you leave sideways)"""
    n = int(math.ceil((z1 - z0) / 16.0 - 0.01))
    length = (yb - ya) if toward in "ns" else (xb - xa)
    d = length / n
    put = ent.brushes.append if ent else NC
    lo_z = {"n": [(xa, ya, z0), (xb, ya, z0), (xb, yb, z1), (xa, yb, z1)], "s": [(xa, ya, z1), (xb, ya, z1), (xb, yb, z0), (xa, yb, z0)],
            "e": [(xa, ya, z0), (xb, ya, z1), (xb, yb, z1), (xa, yb, z0)], "w": [(xa, ya, z1), (xb, ya, z0), (xb, yb, z0), (xa, yb, z1)]}[toward]
    ramp = prism(lo_z, z0 - 8, "CLIP", "CLIP")
    ramp.kind = "ramp"
    D(ramp)
    mx, my = (xa + xb) / 2, (ya + yb) / 2
    lo = {"n": (mx, ya), "s": (mx, yb), "e": (xa, my), "w": (xb, my)}[toward]
    hi = {"n": (mx, yb), "s": (mx, ya), "e": (xb, my), "w": (xa, my)}[toward]
    up = OUT[toward]
    bd = OUT[out_bottom] if out_bottom else (-up[0], -up[1])
    td = OUT[out_top] if out_top else up

    def off(end, step_c, dd, along):
        # straight on: 24 past the end; sideways: from the end step's middle to 24 past the side
        if along:
            return (end[0] + dd[0] * 18, end[1] + dd[1] * 18)
        half = (xb - xa) / 2 if dd[0] else (yb - ya) / 2
        return (step_c[0] + dd[0] * (half + 18), step_c[1] + dd[1] * (half + 18))
    b_pt = off(lo, (lo[0] + up[0] * d / 2, lo[1] + up[1] * d / 2), bd, bd == (-up[0], -up[1]))
    t_pt = off(hi, (hi[0] - up[0] * d / 2, hi[1] - up[1] * d / 2), td, td == up)
    connector("stairs", name or GROUP, (b_pt[0], b_pt[1], z0), bd, (t_pt[0], t_pt[1], z1), td)
    for i in range(n):
        h = z1 - 16 * (n - 1 - i)
        if toward == "n":
            b = (xa, ya + d * i, z0, xb, yb, h)
            tr = (xa, ya + d * i, xb, ya + d * (i + 1))
        elif toward == "s":
            b = (xa, ya, z0, xb, yb - d * i, h)
            tr = (xa, yb - d * (i + 1), xb, yb - d * i)
        elif toward == "e":
            b = (xa + d * i, ya, z0, xb, yb, h)
            tr = (xa + d * i, ya, xa + d * (i + 1), yb)
        else:
            b = (xa, ya, z0, xb - d * i, yb, h)
            tr = (xb - d * (i + 1), ya, xb - d * i, yb)
        bb = box(*b, side, top=top)
        bb.tread = tr        # the part of this step that shows (the rest is under the next steps)
        put(bb)


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
    # each flight climbs straight up to the bank it stands against (10 steps of 24)
    ya, yb = cx(j + 1) + 24, cx(j + 1) + 24 + 96
    group("bedstairs:%d" % j)
    stairs(xw, ya, xw + 240, yb, BED_Z, 0, "w", side=CONC, top="OUT_PAVE1")      # stone steps (they read as steps
    stairs(xe - 240, ya, xe, yb, BED_Z, 0, "e", side=CONC, top="OUT_PAVE1")      # against the rock banks)

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

OPEN_RECTS = [(x0, y0, x1, y1, TUN_Z) for _, x0, y0, x1, y1 in TUNNELS] + [(x0, y0, x1, y1, fl) for (_n, x0, y0, x1, y1, _s, fl) in PLACES]
# the sewer's channel and the cisterns' water channel: 16 down (you step out anywhere)
OPEN_RECTS.append((-1472, 384, 256, 448, TUN_Z - 16))
OPEN_RECTS.append((-1664, -544, -128, -480, -592))
for c, name in TRENCH_CELLS.items():
    OPEN_RECTS.append((cx(c[0]), cx(c[1]), cx(c[0] + 1), cx(c[1] + 1), TUN_Z))
for (i, j) in HATCHES:
    OPEN_RECTS.append((cx(i) + 80, cx(j) + 80, cx(i) + 176, cx(j) + 176, TUN_Z))
    OPEN_RECTS.append((cx(i), cx(j), cx(i + 1), cx(j + 1), TUN_Z))   # the room at the hatch's foot

xs = sorted({-E, E} | {r[0] for r in OPEN_RECTS} | {r[2] for r in OPEN_RECTS})
ys = sorted({-E, E} | {r[1] for r in OPEN_RECTS} | {r[3] for r in OPEN_RECTS})


def slab_floor(x, y):
    """the floor below at x, y (the lowest of the open things there), or None in solid rock"""
    fl = [r[4] for r in OPEN_RECTS if r[0] <= x < r[2] and r[1] <= y < r[3]]
    return min(fl) if fl else None


def slab_open(x, y):
    return slab_floor(x, y) is not None


def near_style(x0, y0, x1, y1):
    """the style of the open place or tunnel nearest a block of rock"""
    best, bd = "cave", 1e9
    for (n_, a0, b0, a1, b1, st, fl) in PLACES:
        d = max(a0 - x1, x0 - a1, 0) + max(b0 - y1, y0 - b1, 0)
        if d < bd:
            best, bd = st, d
    for (n_, a0, b0, a1, b1) in TUNNELS:
        d = max(a0 - x1, x0 - a1, 0) + max(b0 - y1, y0 - b1, 0)
        if d < bd:
            best, bd = TUNNEL_STYLE.get(n_, "utility"), d
    return best


cells = {}
for a in range(len(xs) - 1):
    for b in range(len(ys) - 1):
        mx, my = (xs[a] + xs[a + 1]) / 2, (ys[b] + ys[b + 1]) / 2
        fl_ = slab_floor(mx, my)
        cells[(a, b)] = (fl_, style_at(mx, my) if fl_ is not None else near_style(xs[a], ys[b], xs[a + 1], ys[b + 1]))
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
        fl, st = key           # (solid rock: faced as the nearest place, cell by cell, in the merge key)
        wall_t, floor_t = PSTYLE[st][0], PSTYLE[st][1]
        if fl is not None:
            W(box(X0b, Y0b, FLOOR_Z, X1b, Y1b, fl, wall_t, top=floor_t, scale=2))
        else:
            W(box(X0b, Y0b, FLOOR_Z, X1b, Y1b, SLAB_TOP, wall_t, top=CEIL))

# trenches: stairs from the tunnel floor up to the pad; railings round the deep part
for name, i0, j0, i1, j1, d in TRENCHES:
    group("trench:" + name)
    x0, y0, x1, y1 = cx(i0), cx(j0), cx(i1), cx(j1)
    # where a place below runs round the trench, a rock wall along its sides (the way in is at its foot)
    if d in "ns":
        sides = [(x0 - 16, (y0 + 256) if d == "s" else y0, x0, y1 if d == "s" else (y1 - 256)), (x1, (y0 + 256) if d == "s" else y0, x1 + 16, y1 if d == "s" else (y1 - 256))]
    else:
        sides = [((x0 + 256) if d == "w" else x0, y0 - 16, x1 if d == "w" else (x1 - 256), y0), ((x0 + 256) if d == "w" else x0, y1, x1 if d == "w" else (x1 - 256), y1 + 16)]
    for (a, b_, c, e) in sides:
        if any(slab_floor(a + (c - a) * f, b_ + (e - b_) * g) is not None for f in (0.1, 0.5, 0.9) for g in (0.5,)):
            W(box(a, b_, TUN_Z - 16, c, e, SLAB_TOP, PSTYLE[style_at((a + c) / 2, (b_ + e) / 2)][0]))
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
        n_ = max(1, int(max(c - a, e - b_) // 512))          # (long pieces: fewer brushes; a piece that would hit a wall is left out)
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
    # the way in reads as one from across the field: a portal over the stairs' top end with a lamp
    # under its lintel, and a light at the foot
    # the way in reads as one from across the field: over the stairs' top 256 a roofed housing (concrete
    # trenches) or the mine's timber head-frame (the farm adit, 384 tall); daylight down the stairs, a sign at the foot
    mine = zone_style((x0 + x1) / 2, (y0 + y1) / 2) == "cave"
    if d in "ns":
        ye = y1 if d == "s" else y0
        hb = (x0, ye - 256, x1, ye) if d == "s" else (x0, ye, x1, ye + 256)
        foot = ((x0 + x1) / 2, (y0 + 128) if d == "s" else (y1 - 128))
        aim = -90 if d == "s" else 90
    else:
        xe = x1 if d == "w" else x0
        hb = (xe - 256, y0, xe, y1) if d == "w" else (xe, y0, xe + 256, y1)
        foot = ((x0 + 128) if d == "w" else (x1 - 128), (y0 + y1) / 2)
        aim = 180 if d == "w" else 0
    ht = 384 if mine else 160
    fr = "OUT_WD" if mine else CONC
    if name == "bunker":         # (its east side is the bunker's wall)
        hb = (hb[0], hb[1], hb[2] - 32, hb[3])
    for (px, py) in ((hb[0] - 16, hb[1] - 16), (hb[2], hb[1] - 16), (hb[0] - 16, hb[3]), (hb[2], hb[3])):
        if name == "bunker" and px > hb[0]:      # (the roof's east edge rests on the bunker's wall)
            continue
        P(box(px, py, top_z - 32, px + 16, py + 16, top_z + ht, fr))
        if mine:     # (pointed caps: nothing to land on up there)
            D(frustum([(px, py), (px + 16, py), (px + 16, py + 16), (px, py + 16)], top_z + ht, top_z + ht + 32, 0.0, fr))
    if not mine:
        D(box(hb[0] - 16, hb[1] - 16, top_z + ht, hb[2] + 16, hb[3] + 16, top_z + ht + 16, fr, top="OUT_ROOF3B"))
    else:           # the adit's housing: a plank roof at 176 under the head-frame
        D(box(hb[0] - 16, hb[1] - 16, top_z + 160, hb[2] + 16, hb[3] + 16, top_z + 176, "OUT_WD", top="OUT_ROOF1B"))
    if not mine:    # concrete trenches lined with concrete (not raw rock), from below ground to the top
        for (a, b_, c, e) in ((x0, y0, x0 + 8, y1), (x1 - 8, y0, x1, y1)) if d in "ns" else ((x0, y0, x1, y0 + 8), (x0, y1 - 8, x1, y1)):
            NC(box(a, b_, SLAB_TOP, c, e, top_z, CONC))
    if mine:        # the head-frame's beams and its winding wheel (high up, not in anyone's way)
        for zz in (top_z + 192, top_z + ht - 16):
            NC(box(hb[0] - 16, hb[1] - 16, zz, hb[2] + 16, hb[1], zz + 16, "OUT_WD"))
            NC(box(hb[0] - 16, hb[3], zz, hb[2] + 16, hb[3] + 16, zz + 16, "OUT_WD"))
        wx, wy = (hb[0] + hb[2]) / 2, (hb[1] + hb[3]) / 2
        NC(prism([(wx - 4, wy - 64, top_z + ht - 16), (wx + 4, wy - 64, top_z + ht - 16), (wx + 4, wy + 64, top_z + ht - 16),
                  (wx - 4, wy + 64, top_z + ht - 16)], top_z + ht - 144, "CSTRIKE_ME4METL", "CSTRIKE_ME4METL"))
    point("light", (hb[0] + hb[2]) / 2, (hb[1] + hb[3]) / 2, top_z + ht - 24, _light="255 215 160 110")
    point("light_spot", (hb[0] + hb[2]) / 2, (hb[1] + hb[3]) / 2, top_z + 40, pitch=-45, angles="-45 %d 0" % aim, _cone=40, _cone2=70,
          _light="215 232 255 700")
    point("light", foot[0], foot[1], TUN_Z + 140, _light="215 232 255 160")
    sign = Ent("func_illusionary")
    sign.brushes.append(box(foot[0] - 32, foot[1] - 2, SLAB_TOP - 48, foot[0] + 32, foot[1] + 2, SLAB_TOP - 32, "+0EXIT"))
    ents.append(sign)

# hatches: the cell is solid but for a 96 hole; a ladder on the hole's north wall; over it a small structure
# (HATCH_KIND) with its door on the ladder's side; under it daylight down the shaft and an EXIT sign
def hatch_house(kind, x0, y0, z, hx0, hy0, hx1, hy1):
    mid = (hx0 + hx1) / 2
    if kind == "headframe":     # a mine's timber head-frame over a boarded housing (door on the ladder's side)
        ht = 128
        W(box(hx0 - 16, hy0 - 16, z, hx1 + 16, hy0, z + ht, "OUT_WD"))
        W(box(hx0 - 16, hy0, z, hx0, hy1 + 72, z + ht, "OUT_WD"))
        W(box(hx1, hy0, z, hx1 + 16, hy1 + 72, z + ht, "OUT_WD"))
        W(box(hx0 - 16, hy1 + 56, z, mid - 40, hy1 + 72, z + ht, "OUT_WD"))
        W(box(mid + 40, hy1 + 56, z, hx1 + 16, hy1 + 72, z + ht, "OUT_WD"))
        W(box(mid - 40, hy1 + 56, z + 112, mid + 40, hy1 + 72, z + ht, "OUT_WD"))
        W(box(hx0 - 16, hy0 - 16, z + ht, hx1 + 16, hy1 + 72, z + ht + 16, "OUT_WD", top="OUT_ROOF1B"))
        point("light", mid, hy1 + 28, z + ht - 24, _light="255 220 170 70")
        for (px, py) in ((hx0 - 32, hy0 - 32), (hx1 + 16, hy0 - 32), (hx0 - 32, hy1 + 16), (hx1 + 16, hy1 + 16)):
            P(box(px, py, z - 32, px + 16, py + 16, z + 256, "OUT_WD"))
            D(frustum([(px, py), (px + 16, py), (px + 16, py + 16), (px, py + 16)], z + 256, z + 288, 0.0, "OUT_WD"))
        for yy in (hy0 - 32, hy1 + 16):           # cross beams (above head height, not in anyone's way)
            NC(box(hx0 - 16, yy, z + 240, hx1 + 16, yy + 16, z + 256, "OUT_WD"))
        NC(box((hx0 + hx1) / 2 - 4, hy0 - 16, z + 200, (hx0 + hx1) / 2 + 4, hy1 + 16, z + 248, "CSTRIKE_ME4METL"))   # the wheel
        return
    tex, roof = {"kiosk": ("OUT_WALL3A", "OUT_ROOF2B"), "pump": (CONC, "OUT_ROOF3B"), "shed": ("CSTRIKE_ME2RSTW", "CSTRIKE_ME4METL")}[kind]
    # walls tight round the hole on three sides, the north wall 56 past the hole with a door: you climb out,
    # step north and leave by the door
    ht = 128 if kind != "pump" else 160
    W(box(hx0 - 16, hy0 - 16, z, hx1 + 16, hy0, z + ht, tex))
    W(box(hx0 - 16, hy0, z, hx0, hy1 + 72, z + ht, tex))
    W(box(hx1, hy0, z, hx1 + 16, hy1 + 72, z + ht, tex))
    W(box(hx0 - 16, hy1 + 56, z, mid - 40, hy1 + 72, z + ht, tex))                 # the north wall, a door in its middle
    W(box(mid + 40, hy1 + 56, z, hx1 + 16, hy1 + 72, z + ht, tex))
    W(box(mid - 40, hy1 + 56, z + 112, mid + 40, hy1 + 72, z + ht, tex))
    W(box(hx0 - 16, hy0 - 16, z + ht, hx1 + 16, hy1 + 72, z + ht + 16, tex, top=roof))
    point("light", mid, hy1 + 28, z + ht - 24, _light="255 220 170 70")
    if kind == "pump":          # a pipe out of the pump house into the ground: it's for something
        D(box(hx1 + 16, (hy0 + hy1) / 2 - 12, z + 24, hx1 + 112, (hy0 + hy1) / 2 + 12, z + 48, "CSTRIKE_ME4METL"))
        D(box(hx1 + 96, (hy0 + hy1) / 2 - 12, z - 32, hx1 + 112, (hy0 + hy1) / 2 + 12, z + 24, "CSTRIKE_ME4METL"))


for (i, j) in HATCHES:
    group("hatch:%d,%d" % (i, j))
    x0, y0 = cx(i), cx(j)
    z = H[i][j]
    fl = slab_floor(x0 + 128, y0 + 128)
    hx0, hy0, hx1, hy1 = x0 + 80, y0 + 80, x0 + 176, y0 + 176
    sw = PSTYLE[style_at(x0 + 128, y0 + 128)][0]          # the shaft's walls as the room's
    rocky = zone_style(x0 + 128, y0 + 128) in ("cave", "works")
    W(box(x0, y0, SLAB_TOP, x0 + 256, hy0, z, sw, top=CONC_F))
    W(box(x0, hy1, SLAB_TOP, x0 + 256, y0 + 256, z, sw, top=CONC_F))
    W(box(x0, hy0, SLAB_TOP, hx0, hy1, z, sw, top=CONC_F))
    W(box(hx1, hy0, SLAB_TOP, x0 + 256, hy1, z, sw, top=CONC_F))
    # the room's ceiling round the shaft (the cell's slab is open, so close it), as the room's ceiling
    for (a, b, c, d) in ((x0, y0, x0 + 256, hy0), (x0, hy1, x0 + 256, y0 + 256), (x0, hy0, hx0, hy1), (hx1, hy0, x0 + 256, hy1)):
        W(box(a, b, SLAB_TOP - 16, c, d, SLAB_TOP, sw, bottom=ROCK if rocky else CEIL))
    lad = Ent("func_wall", rendermode=4, renderamt=255)
    lad.brushes.append(box(hx0 + 16, hy1 - 4, fl, hx1 - 16, hy1, z + 8, "{CSTRIKE_LE6LAD"))
    ents.append(lad)
    fl_ = Ent("func_ladder")
    fl_.brushes.append(box(hx0 + 16, hy1 - 12, fl, hx1 - 16, hy1, z + 8, "AAATRIGGER"))
    ents.append(fl_)
    connector("ladder", "hatch:%d,%d" % (i, j), ((hx0 + hx1) / 2, hy1 - 26, fl), (0, -1), ((hx0 + hx1) / 2, hy1 + 20, z), (0, 1))
    hatch_house(HATCH_KIND[(i, j)], x0, y0, z, hx0, hy0, hx1, hy1)
    # daylight down the shaft (you see the way out from two rooms away), a pool of light at its foot, a sign
    point("light_spot", (hx0 + hx1) / 2, (hy0 + hy1) / 2, SLAB_TOP - 24, pitch=-90, angles="-90 0 0", _cone=40, _cone2=70,
          _light="215 232 255 900")
    sign = Ent("func_illusionary")
    sign.brushes.append(box((hx0 + hx1) / 2 - 32, hy0 - 4, SLAB_TOP - 48, (hx0 + hx1) / 2 + 32, hy0, SLAB_TOP - 32, "+0EXIT"))
    ents.append(sign)


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


def building(x0, y0, x1, y1, z, floors=1, fh=160, wt=16, tex="OUT_WALL1A", inner=None, roof=None,
             doors=(), windows=True, roof_access=False, floor_tex="CSTRIKE_FJ2DOCK", light=True, updoors=(),
             roof_gaps=(), stairs_east=False):
    """updoors: [(floor, side, centre, width)] doorways on upper floors (to bridges); roof_gaps:
    [(side, centre, width)] openings in the roof's parapet (to bridges, ladders, or a drop of at most
    176 to the ground); stairs_east: the first flight along the east wall instead of the west"""
    keep = GROUP
    group("bld:%d,%d" % (x0, y0))
    roof = roof or ("OUT_ROOF2B", "OUT_ROOF3B", "OUT_ROOF1B")[(int(x0) // 64 + int(y0) // 64) % 3]
    top = _building(x0, y0, x1, y1, z, floors, fh, wt, tex, inner, roof, doors, windows, roof_access, floor_tex, light,
                    updoors, roof_gaps, stairs_east)
    group(keep)
    return top


def _building(x0, y0, x1, y1, z, floors, fh, wt, tex, inner, roof, doors, windows, roof_access, floor_tex, light,
              updoors, roof_gaps, stairs_east=False):
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

    def west(f):
        return (f % 2 == 0) != stairs_east

    def flight_x(f):
        return (x0 + wt, x0 + wt + 112) if west(f) else (x1 - wt - 112, x1 - wt)

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
        # a rail beside the stairwell upstairs (stops walking into the hole), open at the top end
        if f % 2 == 0:
            stairs(fx0, sy0, fx1, sy1, fz, fz + fh, "n", side="OUT_WD", top="CSTRIKE_FJ2DOCK")
            rx = (fx1, fx1 + 8) if west(f) else (fx0 - 8, fx0)
            W(box(rx[0], sy0, fz + fh, rx[1], sy1 - 64, fz + fh + 40, RAIL))
            W(box(fx0, sy0 - 8, fz + fh, fx1, sy0, fz + fh + 40, RAIL))      # and across the hole's far end
        else:
            stairs(fx0, sy0, fx1, sy1, fz, fz + fh, "s", side="OUT_WD", top="CSTRIKE_FJ2DOCK")
            rx = (fx1, fx1 + 8) if west(f) else (fx0 - 8, fx0)
            W(box(rx[0], sy0 + 64, fz + fh, rx[1], sy1, fz + fh + 40, RAIL))
            W(box(fx0, sy1, fz + fh, fx1, sy1 + 8, fz + fh + 40, RAIL))
    if roof_access:
        gaps = {"n": [], "s": [], "e": [], "w": []}
        for (side, c, w) in roof_gaps:
            gaps[side].append((c, w, top, top + 40))
        wall(x0, y0, x1, y0 + 8, top, top + 40, tex, gaps["s"], "x")
        wall(x0, y1 - 8, x1, y1, top, top + 40, tex, gaps["n"], "x")
        wall(x0, y0 + 8, x0 + 8, y1 - 8, top, top + 40, tex, gaps["w"], "y")
        wall(x1 - 8, y0 + 8, x1, y1 - 8, top, top + 40, tex, gaps["e"], "y")
    W(box(x0 + wt, y0 + wt, z, x1 - wt, y1 - wt, z + 1, floor_tex))
    # furniture: a table and a crate in the ground floor's free corners (cover inside, and it's lived in)
    flx = flight_x(0) if levels else None
    placed = 0
    for (cx_, cy_, sx_, sy_) in ((x0 + wt, y0 + wt, 1, 1), (x1 - wt, y0 + wt, -1, 1), (x0 + wt, y1 - wt, 1, -1), (x1 - wt, y1 - wt, -1, -1)):
        if placed >= 2:
            break
        w_, d_ = (96, 48) if placed == 0 else (48, 48)
        bx0, bx1 = sorted((cx_ + sx_ * 24, cx_ + sx_ * (24 + w_)))
        by0, by1 = sorted((cy_ + sy_ * 24, cy_ + sy_ * (24 + d_)))
        near_door = False
        for (side, off) in doors:
            mid = (x0 + x1) / 2 + off if side in "ns" else (y0 + y1) / 2 + off
            if side == "n" and sy_ < 0 and abs((bx0 + bx1) / 2 - mid) < 200: near_door = True
            if side == "s" and sy_ > 0 and abs((bx0 + bx1) / 2 - mid) < 200: near_door = True
            if side == "w" and sx_ > 0 and abs((by0 + by1) / 2 - mid) < 200: near_door = True
            if side == "e" and sx_ < 0 and abs((by0 + by1) / 2 - mid) < 200: near_door = True
        in_stairs = flx is not None and bx0 < flx[1] + 48 and bx1 > flx[0] - 48 and by0 < sy1 + 64 and by1 > sy0 - 96
        if near_door or in_stairs or x1 - x0 < 320 or y1 - y0 < 256:
            continue
        if placed == 0:
            P(box(bx0, by0, z + 1, bx1, by1, z + 40, "OUT_WD", top="CSTRIKE_FJ2DOCK"))
        else:
            P(box(bx0, by0, z + 1, bx1, by1, z + 49, "MltryCrteSd", top="MltryCrteTp"))
        placed += 1
    if light:
        for f in range(floors):
            point("light", (x0 + x1) / 2, (y0 + y1) / 2, z + f * fh + fh - 40, _light="255 230 190 110")
    return top


def crate(x, y, z, s=64, h=None, big=False):
    h = h or s
    P(box(x, y, z, x + s, y + s, z + h, "SandCrtLrgSd" if big else "MltryCrteSd", top="SandCrtLrgTp" if big else "MltryCrteTp"))


def sandbags(xa, ya, xb, yb, z, h=48):
    P(box(xa, ya, z, xb, yb, z + h, "OUT_SNBAG", top="OUT_SNBAGB"))


def ladder(x0, y0, x1, y1, z0, z1, face, top_z=None, name=None, bottom_z=None):
    """a visible ladder (func_wall) and the climbable volume (func_ladder); face = the side
    a climber stands on: n/s/e/w. top_z: the floor you step off onto at the top, on the side
    away from the climber (default 8 under the ladder's top); bottom_z: the floor at its foot"""
    f = OUT[face]
    lx, ly = (x0 + x1) / 2, (y0 + y1) / 2
    ht = ((x1 - x0) if f[0] else (y1 - y0)) / 2
    connector("ladder", name or GROUP, (lx + f[0] * (ht + 20), ly + f[1] * (ht + 20), z0 if bottom_z is None else bottom_z), f,
              (lx - f[0] * (ht + 20), ly - f[1] * (ht + 20), z1 - 8 if top_z is None else top_z), (-f[0], -f[1]))
    lad = Ent("func_wall", rendermode=4, renderamt=255)
    lad.brushes.append(box(x0, y0, z0, x1, y1, z1, "{CSTRIKE_LE6LAD"))
    register(lad.brushes[-1])
    lad.brushes[-1].kind = "ladder"
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
              roof_gaps=[("e", 980, 72)], updoors=[(1, "s", -1480, 96)])
LOOT += [(-1440, 832, Z + 8, 2, "default"), (-1400, 880, Z + 160 + 8, 3, "default"), (-1560, 980, Z + 320 + 8, 3, "default")]
# town hall: three floors and a roof, the tallest building in the valley
building(-1088, 576, -640, 1152, Z, floors=3, tex="OUT_WALL6", doors=[("s", 0), ("w", 128)], roof_access=True, floor_tex="-0CRETE4_FLR01",
         updoors=[(2, "w", 980, 96), (1, "e", 992, 96)], roof_gaps=[("n", -888, 72)])
# the hall's fire ladder: from the street behind it straight up to the roof (the roof's second way down)
ladder(-912, 1152, -864, 1156, Z, Z + 480 + 8, "n", name="hall fire ladder")
LOOT += [(-864, 700, Z + 8, 4, "default"), (-800, 900, Z + 168, 3, "default"), (-800, 900, Z + 328, 3, "default")]
WEAP += [(-760, 1000, Z + 488, "snipers", 4)]
building(-448, 640, 0, 1024, Z, floors=1, tex="OUT_WALL3A", doors=[("s", 0), ("n", 0)], roof_access=True,
         roof_gaps=[("w", 992, 96), ("s", -184, 80), ("e", 800, 96)])
LOOT += [(-224, 832, Z + 8, 2, "default")]
building(-1664, -192, -1152, 192, Z, floors=1, tex="OUT_WALL1A", doors=[("n", 0), ("e", 0)], roof_access=True,
         roof_gaps=[("n", -1480, 96), ("e", 0, 96), ("s", -1300, 96)])
LOOT += [(-1408, 0, Z + 8, 2, "default"), (-1250, 120, Z + 168, 3, "default")]
building(-1024, -192, -576, 160, Z, floors=2, tex="CSTRIKE_WB6PLN3", doors=[("n", 0), ("e", 0)], updoors=[(1, "e", -16, 96)])
LOOT += [(-800, 0, Z + 168, 3, "default")]
WEAP += [(-760, -80, Z + 8, "smgs", 2)]
building(-448, -192, -64, 160, Z, floors=1, tex="OUT_WALL5A", doors=[("n", 0), ("s", 0), ("w", 0)], roof_access=True,
         roof_gaps=[("n", -184, 80), ("w", -16, 96), ("s", -320, 96)], stairs_east=True)
LOOT += [(-200, 0, Z + 8, 2, "gear")]
# the upper deck: walkways between the buildings, off the street
def walkway(name, x0, y0, x1, y1, z, along):
    """a plank deck with its top at z and railings on its long sides (along 'x' or 'y')"""
    keep = GROUP
    group("link:" + name)
    D(box(x0, y0, z - 16, x1, y1, z, WOOD, top=PLANK))
    BLOCKED.append((x0 - 16, y0 - 16, x1 + 16, y1 + 16))
    if along == "x":
        connector("walkway", "link:" + name, (x0 - 18, (y0 + y1) / 2, z), (-1, 0), (x1 + 18, (y0 + y1) / 2, z), (1, 0))
    else:
        connector("walkway", "link:" + name, ((x0 + x1) / 2, y0 - 18, z), (0, -1), ((x0 + x1) / 2, y1 + 18, z), (0, 1))
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
walkway("west-street", -1528, 192, -1432, 640, Z + 160, "y")      # house D's roof over the street to house A's 2nd floor
walkway("E-F", -576, -56, -448, 24, Z + 160, "x")                 # house E's 2nd floor to house F's roof
# cars and crates in the streets
for (x, y) in [(-1408, 336), (-704, 448), (-1500, 1180), (-320, 1184)]:
    D(box(x, y, Z + 12, x + 160, y + 80, Z + 52, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))      # (on its wheels)
    D(box(x + 40, y + 8, Z + 52, x + 112, y + 72, Z + 88, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))      # the cab
    for (wx, wy) in ((x + 16, y - 4), (x + 112, y - 4), (x + 16, y + 76), (x + 112, y + 76)):
        P(box(wx, wy, Z, wx + 32, wy + 8, Z + 24, "CSTRIKE_ME4METL"))                                   # wheels
crate(-1792, 600, Z, 64)
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
# the roof, with a hatch over the catwalk's west end (a ladder from the catwalk comes up through it)
for (a, b, c, d) in [(FX0, FY0, FX1, FY0 + WT), (FX0, -3160, FX1, FY1), (FX0, FY0 + WT, -3216, -3160), (-3120, FY0 + WT, FX1, -3160)]:
    W(box(a, b, FH, c, d, FH + 16, "CSTRIKE_ME2RSTW", top="OUT_ROOF3B", bottom="CSTRIKE_CE2XCNG"))
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
# from the catwalk's west end a ladder up through the roof hatch (stands at the hatch's north edge)
ladder(-3200, -3164, -3136, -3160, 160, FH + 24, "s", top_z=FH + 16, name="factory roof hatch")
for (x, y) in [(-3072, -2880), (-2880, -2784), (-2688, -2944)]:
    crate(x, y, Z, 128, big=True)
crate(-3168, -2688, Z, 64)
for k in range(4):
    point("light", FX0 + 120 + k * 200, (FY0 + FY1) / 2, FH - 40, _light="255 240 210 160")
LOOT += [(-2960, -2700, 8, 4, "default"), (-3100, -3180, 168, 3, "default"), (-2540, -2700, 8, 2, "gear")]
WEAP += [(-2700, -3180, 168, "rifles", 3)]
# offices: two floors with a roof
building(-2304, -3200, -1920, -2752, Z, floors=2, tex=CONC, doors=[("e", 0), ("n", 0)], roof_access=True, floor_tex="CSTRIKE_FE2SQRS",
         roof_gaps=[("w", -2880, 80), ("e", -2808, 80)])
walkway("factory-office", -2432, -2920, -2304, -2840, FH + 32, "x")   # the hall's roof to the office's roof
LOOT += [(-2112, -2976, 168, 3, "default")]
# containers in the yard
for (x, y, s) in [(-3264, -2496, 0), (-2688, -2496, 1), (-2304, -2496, 0)]:
    P(box(x, y, Z, x + 256, y + 96, Z + 112, "CSTRIKE_OE6TNKS" if s else "CSTRIKE_WE2RIBD", top="CSTRIKE_ME4METL"))
# chimney with a ladder up two of its sides to a railed platform (look-out over the south)
D(box(-2400, -3264, 0, -2304, -3168, 480, "CSTRIKE_WB6PLN1", top="CSTRIKE_ME4METL"))
D(box(-2400, -3264, 480, -2272, -3168, 496, RAIL, top="CSTRIKE_FE2GTNG"))
for (a, b, c, d) in [(-2400, -3264, -2392, -3176), (-2280, -3256, -2272, -3176), (-2320, -3176, -2272, -3168),
                     (-2400, -3176, -2384, -3168), (-2320, -3264, -2272, -3256), (-2400, -3264, -2384, -3256)]:
    D(box(a, b, 496, c, d, 536, RAIL))
ladder(-2384, -3168, -2320, -3164, 0, 504, "n", top_z=496, name="chimney north")
ladder(-2384, -3268, -2320, -3264, 0, 504, "s", top_z=496, name="chimney south")
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
# the roof: a deck with a railing, a hatch over the hay loft (ladder up from the loft) and the hay hoist's
# ladder up the west wall
for (a, b, c, d) in [(BX0, BY0, BX1, 2972), (BX0, 2972, -3040, BY1), (-2944, 2972, BX1, BY1), (-3040, BY1 - 16, -2944, BY1)]:
    W(box(a, b, Z + BH, c, d, Z + BH + 16, "OUT_WD", top="OUT_ROOF1B", bottom="OUT_WD"))
for (a, b, c, d) in [(BX0, BY0, BX1, BY0 + 8), (BX0, BY1 - 8, BX1, BY1), (BX1 - 8, BY0 + 8, BX1, BY1 - 8),
                     (BX0, BY0 + 8, BX0 + 8, 2936), (BX0, 3016, BX0 + 8, BY1 - 8)]:
    D(box(a, b, Z + BH + 16, c, d, Z + BH + 56, "OUT_WD"))
ladder(-3024, 2972, -2960, 2976, Z + 144, Z + BH + 24, "n", top_z=Z + BH + 16, name="barn loft to roof")
ladder(-3204, 2944, -3200, 3008, Z, Z + BH + 24, "w", top_z=Z + BH + 16, name="barn hoist ladder")
LOOT += [(-2800, 2700, Z + BH + 24, 3, "default")]
W(box(BX0 + 16, BY0 + 16, Z, BX1 - 16, BY1 - 16, Z + 1, "CSTRIKE_FT2DIRT"))
# hay loft over the north half at +144, stairs on the east side
D(box(BX0 + 16, 2848, Z + 128, BX1 - 16, BY1 - 16, Z + 144, "OUT_WD", top="CSTRIKE_FJ2DOCK"))
stairs(BX1 - 16 - 112, 2848 - 200, BX1 - 16, 2848, Z, Z + 144, "n", side="OUT_WD", top="CSTRIKE_FJ2DOCK")
for (x, y) in [(-3136, 2624), (-3000, 2700), (-3136, 2950)]:
    P(box(x, y, Z, x + 96, y + 64, Z + 64, "OUT_SND2", top="OUT_SND2"))   # hay bales
point("light", -2912, 2800, Z + 200, _light="255 230 190 140")
LOOT += [(-2800, 2700, Z + 8, 2, "default"), (-2800, 2960, Z + 152, 3, "default")]
WEAP += [(-3100, 2960, Z + 152, "snipers", 3)]
building(-2272, 2560, -1888, 2944, Z, floors=2, tex="OUT_WALL7", doors=[("s", 0), ("e", 0)], roof_access=True,
         roof_gaps=[("n", -2048, 72)])
ladder(-2080, 2944, -2016, 2948, Z, Z + 320 + 8, "n", name="farmhouse back ladder")
LOOT += [(-2080, 2752, Z + 8, 2, "default"), (-2080, 2752, Z + 168, 2, "gear"), (-1968, 2900, Z + 328, 3, "default")]
# the silo: eight-sided, corrugated, a cap ring; a caged ladder up its east face and a switchback stair scaffold
# against its west face (a perch you can fight your way onto), a railed top with a sniper rifle
SCX, SCY, SR = -3008, 2272, 112
group("poi:silo")
D(prism([(px, py, Z + 448) for (px, py) in regular(SCX, SCY, SR, 8, math.pi / 8)], Z, "CSTRIKE_ME2RSTW", "CSTRIKE_ME2RSTW"))
NC(frustum(regular(SCX, SCY, SR + 8, 8, math.pi / 8), Z + 432, Z + 448, 1.0, "CSTRIKE_ME4METL"))
top8 = regular(SCX, SCY, SR, 8, math.pi / 8)
for k in range(8):
    (ax, ay), (bx_, by) = top8[k], top8[(k + 1) % 8]
    mx_, my_ = (ax + bx_) / 2 - SCX, (ay + by) / 2 - SCY
    if abs(my_) < 10 and abs(mx_) > 50:      # the east face (the ladder) and the west face (the bridge from the stairs)
        continue
    nx_, ny_ = -mx_ / math.hypot(mx_, my_) * 8, -my_ / math.hypot(mx_, my_) * 8
    D(prism([(ax, ay, Z + 488), (bx_, by, Z + 488), (bx_ + nx_, by + ny_, Z + 488), (ax + nx_, ay + ny_, Z + 488)], Z + 448, RAIL, RAIL))
ax_e = SCX + SR * math.cos(math.pi / 8)
ladder(ax_e, SCY - 24, ax_e + 4, SCY + 24, Z, Z + 456, "e", top_z=Z + 448, name="silo east")
for zz in (Z + 120, Z + 240, Z + 360):      # the ladder's cage hoops (not in the climber's way)
    NC(box(ax_e + 4, SCY - 40, zz, ax_e + 48, SCY - 36, zz + 6, RAIL))
    NC(box(ax_e + 4, SCY + 36, zz, ax_e + 48, SCY + 40, zz + 6, RAIL))
    NC(box(ax_e + 44, SCY - 40, zz, ax_e + 48, SCY + 40, zz + 6, RAIL))


def stair_scaffold(x0, y0, z0, z1, flights, step=20, name="scaffold"):
    """switchback stairs in an open steel scaffold: two 96-wide columns side by side (x0..x0+192), flights along
    y, a landing at each end; flight 0 rises north in the west column from y0+96, the next south in the east one...
    returns the top landing's box"""
    rise = (z1 - z0) / flights
    n = int(round(rise / 16))
    L = n * step
    land = None
    for k in range(flights):
        za, zb = z0 + rise * k, z0 + rise * (k + 1)
        col = (x0, x0 + 96) if k % 2 == 0 else (x0 + 96, x0 + 192)
        if k % 2 == 0:
            stairs(col[0], y0 + 96, col[1], y0 + 96 + L, za, zb, "n", side=RAIL, top="CSTRIKE_FE2GTNG", name="%s %d" % (name, k))
            land = (x0, y0 + 96 + L, x0 + 192, y0 + 192 + L, zb)
            D(prism([(col[0] - 8, y0 + 96, za + 40), (col[0], y0 + 96, za + 40), (col[0], y0 + 96 + L, zb + 40), (col[0] - 8, y0 + 96 + L, zb + 40)],
                    za, RAIL, RAIL))
        else:
            stairs(col[0], y0 + 96, col[1], y0 + 96 + L, za, zb, "s", side=RAIL, top="CSTRIKE_FE2GTNG", name="%s %d" % (name, k))
            land = (x0, y0, x0 + 192, y0 + 96, zb)
            D(prism([(col[1], y0 + 96, zb + 40), (col[1] + 8, y0 + 96, zb + 40), (col[1] + 8, y0 + 96 + L, za + 40), (col[1], y0 + 96 + L, za + 40)],
                    za, RAIL, RAIL))
        D(box(land[0], land[1], zb - 16, land[2], land[3], zb, RAIL, top="CSTRIKE_FE2GTNG"))
    for (px, py) in ((x0 - 8, y0), (x0 + 184, y0), (x0 - 8, y0 + 184 + L), (x0 + 184, y0 + 184 + L)):
        D(box(px, py, min(ground(px, py), ground(px + 8, py + 8)) - 16, px + 8, py + 8, z1, RAIL))
    BLOCKED.append((x0 - 32, y0 - 32, x0 + 224, y0 + 224 + L))
    return land


lz = stair_scaffold(-3328, SCY - 48, Z, Z + 448, 4, step=20, name="silo stairs")
D(box(-3136, SCY - 32, Z + 432, SCX - SR * math.cos(math.pi / 8) + 4, SCY + 32, Z + 448, RAIL, top="CSTRIKE_FE2GTNG"))   # the bridge across
connector("walkway", "silo bridge", (-3150, SCY, Z + 448), (-1, 0), (SCX - 80, SCY, Z + 448), (1, 0))
for (a, b, c, d) in [(-3136, SCY - 40, SCX - 100, SCY - 32), (-3136, SCY + 32, SCX - 100, SCY + 40)]:
    D(box(a, b, Z + 448, c, d, Z + 488, RAIL))
WEAP += [(-2980, 2272, Z + 456, "snipers", 4)]
BLOCKED.append((SCX - SR - 32, SCY - SR - 32, SCX + SR + 64, SCY + SR + 32))
group("poi:farm")
for (a, b, c, d) in [(-3200, 1856, -2688, 1872), (-3200, 1856, -3184, 2128), (-2176, 1856, -1856, 1872), (-1872, 1856, -1856, 2304)]:
    P(box(a, b, Z, c, d, Z + 44, "OUT_WD"))
WEAP += [(-2944, 2112, Z + 8, "smgs", 2)]

# --- radio hill (z 448): a lattice tower with a ladder, a hut -----------------------------
group("poi:radio")
Z = 448
TX, TY = -256, 2880
for (dx, dy) in [(-64, -64), (48, -64), (-64, 48), (48, 48)]:
    D(box(TX + dx, TY + dy, Z, TX + dx + 16, TY + dy + 16, Z + 384, RAIL))
D(box(TX - 96, TY - 96, Z + 384, TX + 96, TY + 96, Z + 400, RAIL, top="CSTRIKE_FE2GTNG"))
for (a, b, c, d) in [(TX - 96, TY - 96, TX + 96, TY - 88), (TX - 96, TY + 88, TX + 96, TY + 96), (TX - 96, TY - 88, TX - 88, TY - 40),
                     (TX - 96, TY + 40, TX - 88, TY + 88)]:
    D(box(a, b, Z + 400, c, d, Z + 440, RAIL))
D(box(TX - 8, TY - 8, Z + 400, TX + 8, TY + 8, Z + 640, RAIL))
D(box(TX - 6, TY - 6, Z + 640, TX + 6, TY + 6, Z + 652, "~SPOTRED"))
D(box(TX + 84, TY - 40, Z, TX + 96, TY + 40, Z + 384, RAIL))   # backing behind the ladders
D(box(TX - 96, TY - 40, Z + 200, TX - 84, TY + 40, Z + 384, RAIL))
ladder(TX + 96, TY - 32, TX + 100, TY + 32, Z, Z + 408, "e", top_z=Z + 400, name="radio tower east")
# west: a ladder up to a caged landing at 200 (on four posts), a second ladder from it to the platform
D(box(TX - 208, TY - 64, Z + 184, TX - 96, TY + 64, Z + 200, RAIL, top="CSTRIKE_FE2GTNG"))
for (px, py) in ((TX - 208, TY - 64), (TX - 112, TY - 64), (TX - 208, TY + 48), (TX - 112, TY + 48)):
    D(box(px, py, Z, px + 16, py + 16, Z + 184, RAIL))
for (a, b, c, d) in [(TX - 208, TY - 64, TX - 96, TY - 56), (TX - 208, TY - 56, TX - 200, TY + 56), (TX - 160, TY + 56, TX - 96, TY + 64)]:
    D(box(a, b, Z + 200, c, d, Z + 248, RAIL))          # the cage (waist high: cover to shoot over)
ladder(TX - 200, TY + 64, TX - 160, TY + 68, Z, Z + 208, "n", top_z=Z + 200, name="radio tower west, lower")
ladder(TX - 100, TY - 32, TX - 96, TY + 32, Z + 200, Z + 408, "w", top_z=Z + 400, bottom_z=Z + 200, name="radio tower west, upper")
BLOCKED.append((TX - 128, TY - 128, TX + 128, TY + 128))
LOOT += [(TX - 48, TY + 48, Z + 408, 4, "default")]
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
building(-1216, -3008, -768, -2624, Z, floors=1, tex="OUT_WALL5A", doors=[("n", 0), ("w", 128)], roof_access=True,
         roof_gaps=[("w", -2808, 80), ("s", -1100, 96)], stairs_east=True)
LOOT += [(-992, -2816, 8, 4, "default"), (-760, -1936, 8, 1, "gear")]

# --- the docks (west bank, z 0): boathouse, piers over the river --------------------------
group("poi:docks")
Z = 0
building(-128, -1280, 320, -896, Z, floors=1, fh=176, tex="OUT_WD", doors=[("e", 0), ("s", 0), ("n", 0)], roof_access=True,
         floor_tex="CSTRIKE_FJ2DOCK", roof_gaps=[("n", 192, 96), ("e", -1040, 96)])
LOOT += [(96, -1088, 8, 4, "default")]
for j in (9, 10, 11):
    xw, xe = river_x(j)
    y = cx(j) + 64
    D(box(xw - 64, y, -16, xw + 224, y + 128, 0, WOOD, top=PLANK))
    for px in (xw + 64, xw + 192):
        D(box(px - 12, y + 52, BED_Z, px + 12, y + 76, -16, WOOD))
    crate(xw + 96, y + 16, 0, 64)
xw = cx(min(i for (i, j) in RIVER if j == 11))      # the bank as it is in that row (the channel's cells)
stairs(xw, cx(11) + 208, xw + 240, cx(11) + 304, BED_Z, 0, "w", side=WOOD, top=PLANK)   # up onto the bank past the last pier
LOOT += [(river_x(11)[0] + 160, cx(11) + 128, 8, 2, "default")]
WEAP += [(river_x(9)[0] + 32, cx(9) + 96, 8, "smgs", 2)]

# --- the quarry (z -128): pit with rock piles, a shed, a crane-like gantry ------------------
group("poi:quarry")
Z = -128
QUARRY_PILES = [(2512, -368, 96, 128), (3216, -336, 96, 96), (3216, 272, 96, 150), (2736, 48, 64, 160)]
for (x, y, r_, h) in QUARRY_PILES:
    BLOCKED.append((x - r_ - 32, y - r_ - 32, x + r_ + 32, y + r_ + 32))
building(2368, 96, 2752, 448, Z, floors=1, tex="CSTRIKE_ME2RSTW", doors=[("s", 0), ("e", 0)], roof_access=True)
LOOT += [(2560, 272, Z + 8, 4, "default"), (2500, -100, Z + 8, 2, "gear")]
WEAP += [(3136, 400, Z + 8, "rifles", 3)]

# --- the bunker (z 160): thick concrete, two rooms, roof with parapet; the complex below ----
group("poi:bunker")
Z = 64          # (v5: the pad came down 96, so its stair trench is 36.9 degrees)
KX0, KY0, KX1, KY1, KH = 2560, -3072, 3072, -2624, 160
BLOCKED.append((KX0 - 32, KY0 - 32, KX1 + 32, KY1 + 32))
wall(KX0, KY1 - 32, KX1, KY1, Z, Z + KH, CONC, [(2688, 128, Z, Z + 128)], "x")
wall(KX0, KY0, KX1, KY0 + 32, Z, Z + KH, CONC, [(2700, 160, Z + 64, Z + 96), (2940, 160, Z + 64, Z + 96)], "x")
wall(KX0, KY0 + 32, KX0 + 32, KY1 - 32, Z, Z + KH, CONC, [(-2848, 128, Z, Z + 128)], "y")
wall(KX1 - 32, KY0 + 32, KX1, KY1 - 32, Z, Z + KH, CONC, [(-2848, 160, Z + 64, Z + 96)], "y")
wall(2800, KY0 + 32, 2832, KY1 - 32, Z, Z + KH, CONC, [(-2848, 128, Z, Z + 128)], "y")
W(box(KX0, KY0, Z + KH, KX1, KY1, Z + KH + 32, CONC, top=CONC_F, bottom=CEIL))
W(box(KX0 + 32, KY0 + 32, Z, KX1 - 32, KY1 - 32, Z + 1, CONC_F))
for (a, b, c, d) in [(KX0, KY0, KX1, KY0 + 16), (KX0, KY1 - 16, 2892, KY1), (2972, KY1 - 16, KX1, KY1), (KX0, KY0 + 16, KX0 + 16, KY1 - 16),
                     (KX1 - 16, KY0 + 16, KX1, KY1 - 112)]:
    W(box(a, b, Z + KH + 32, c, d, Z + KH + 72, CONC))
ladder(2900, KY1, 2964, KY1 + 4, Z, Z + KH + 40, "n", top_z=Z + KH + 32, name="bunker north ladder")
stairs(KX1 + 16, KY1 - 448, KX1 + 144, KY1 - 112, Z, Z + KH + 32, "n", side=CONC, top=CONC_F)
D(box(KX1, KY1 - 112, Z, KX1 + 144, KY1 - 16, Z + KH + 32, CONC, top=CONC_F))
point("light", 2680, -2848, Z + 120, _light="255 220 170 140")
point("light", 2950, -2848, Z + 120, _light="255 220 170 140")
LOOT += [(2680, -2848, Z + 8, 3, "default"), (2950, -2900, Z + 8, 3, "default"), (2816, -2700, Z + KH + 40, 3, "gear")]
WEAP += [(2940, -2760, Z + 8, "rifles", 3)]
# trenches of sandbags round it
for (a, b, c, d) in [(2624, -2432, 2816, -2400), (3136, -2432, 3264, -2400), (3264, -3264, 3296, -3008)]:
    sandbags(a, b, c, d, Z)

# --- the lumber camp (z 128): an open sawmill shed, log piles, a watchtower ----------------
group("poi:lumber")
Z = 128
for (x, y) in [(2432, 2304), (2880, 2304), (2432, 2624), (2880, 2624)]:
    D(box(x, y, Z, x + 24, y + 24, Z + 160, WOOD))
D(box(2400, 2272, Z + 160, 2936, 2680, Z + 176, WOOD, top="OUT_ROOF1B", bottom=WOOD))
BLOCKED.append((2400, 2272, 2936, 2680))
for (x, y) in [(2496, 2400), (2496, 2512), (2720, 2400)]:
    P(box(x, y, Z, x + 320, y + 64, Z + 48, "OUT_WD", top="TNNL_BRD1B"))
for (x, y) in [(3008, 2176), (3008, 2240), (3008, 2304)]:
    P(box(x, y, Z, x + 256, y + 48, Z + 48, "OUT_WD"))
building(2496, 2752, 2880, 3008, Z, floors=1, tex="OUT_WD", doors=[("s", 0), ("e", 0)], roof_access=False)
# watchtower beside the sawmill: four posts, an open railed platform at +256, a ladder on the south side and
# a short flight of stairs down to the sawmill's roof (+176, a safe drop to the yard all round)
WX, WY = 3152, 2560
for (dx, dy) in [(-64, -64), (48, -64), (-64, 48), (48, 48)]:
    D(box(WX + dx, WY + dy, Z, WX + dx + 16, WY + dy + 16, Z + 256, WOOD))
D(box(WX - 80, WY - 80, Z + 256, WX + 80, WY + 80, Z + 272, WOOD, top=PLANK))
for (a, b, c, d) in [(WX - 80, WY + 72, WX + 80, WY + 80), (WX + 72, WY - 72, WX + 80, WY + 72),
                     (WX - 80, WY - 80, WX - 40, WY - 72), (WX + 40, WY - 80, WX + 80, WY - 72),
                     (WX - 80, WY - 72, WX - 72, WY - 16), (WX - 80, WY + 64, WX - 72, WY + 72)]:
    D(box(a, b, Z + 272, c, d, Z + 312, WOOD))
D(box(WX - 40, WY - 80, Z, WX + 40, WY - 72, Z + 256, WOOD))      # backing behind the ladder
ladder(WX - 32, WY - 84, WX + 32, WY - 80, Z, Z + 280, "s", top_z=Z + 272, name="watchtower ladder")
stairs(2936, WY - 8, WX - 80, WY + 64, Z + 176, Z + 272, "e", side=WOOD, top=PLANK, name="watchtower to sawmill roof")
LOOT += [(2560, 2400, Z + 184, 3, "default")]
BLOCKED.append((WX - 112, WY - 112, WX + 112, WY + 112))
point("light", 2672, 2480, Z + 160, _light="255 230 190 120")
LOOT += [(2672, 2600, Z + 8, 4, "default"), (2688, 2880, Z + 8, 2, "default"), (WX + 24, WY + 24, Z + 280, 3, "default")]
WEAP += [(2560, 2300, Z + 8, "rifles", 3)]

# --- the mesas: the old fort (west) and the lookout (east) ---------------------------------
group("poi:mesas")
z = 352
P(box(-3264, -1216, z, -2848, -1184, z + 96, "-1csSandWall", top="SandTrim"))
P(box(-3008, -608, z, -2848, -576, z + 96, "-1csSandWall", top="SandTrim"))
P(box(-3264, -1184, z, -3232, -800, z + 96, "-1csSandWall", top="SandTrim"))
sandbags(-2912, -832, -2880, -704, z)
crate(-3040, -1040, z, 128, big=True)
LOOT += [(-2950, -1100, z + 8, 4, "default")]
WEAP += [(-2900, -680, z + 8, "rifles", 4)]
z = 320
sandbags(2112, 1408, 2304, 1440, z)
crate(2400, 1152, z, 64)
LOOT += [(2300, 1250, z + 8, 3, "default")]
WEAP += [(2180, 1100, z + 8, "snipers", 3)]
# each mesa's second way down (the first is its ramp): the fort's north gate, a few steps onto West Peak's
# flank; the lookout's cliff stairs along its north face to a landing at the top
zf = 352
for _ in range(5):
    n_ = max(1, int(math.ceil((352 - zf) / 16.0 - 0.01)))
    zf = max(ground(x_, -512 + 24 * n_ + 24) for x_ in (-3192, -3152, -3112))
n_ = max(1, int(math.ceil((352 - zf) / 16.0 - 0.01)))
group("mesastairs:fort")
stairs(-3200, -512, -3104, -512 + 24 * n_, 352 - 16 * n_, 352, "s", side="-1csSandWall", top="SandTrim", name="fort north gate")
P(box(-3232, -560, 352, -3200, -512, 352 + 128, "-1csSandWall", top="SandTrim"))      # the gate's posts on the mesa's edge
P(box(-3104, -560, 352, -3072, -512, 352 + 128, "-1csSandWall", top="SandTrim"))
BLOCKED.append((-3232, -512, -3072, -512 + 24 * n_ + 32))

# --- the river pumping station (east of the river): two pump houses over the works below (their hatches), a
# water tower between them (no way up: it's to look at and to hide behind), pipe runs as cover
group("poi:pumping")
PWX, PWY = 1920, -760
gz_ = min(ground(PWX + dx, PWY + dy) for dx in (-80, 80) for dy in (-80, 80))
for (dx, dy) in ((-72, -72), (56, -72), (-72, 56), (56, 56)):
    P(box(PWX + dx, PWY + dy, gz_ - 16, PWX + dx + 16, PWY + dy + 16, gz_ + 256, RAIL))
D(prism([(px, py, gz_ + 352) for (px, py) in regular(PWX, PWY, 104, 8, math.pi / 8)], gz_ + 256, "CSTRIKE_OE6TNKS", "CSTRIKE_ME4METL"))
D(frustum(regular(PWX, PWY, 108, 8, math.pi / 8), gz_ + 352, gz_ + 528, 0.0, "CSTRIKE_ME4METL"))      # (steep: nothing to stand on)
for (ya, yb) in ((96, 216),):            # (a pipe north to the pump house)
    D(box(PWX - 12, PWY + ya, min(ground(PWX, PWY + ya), ground(PWX, PWY + yb)) - 8, PWX + 12, PWY + yb, max(ground(PWX, PWY + ya), ground(PWX, PWY + yb)) + 32, "CSTRIKE_ME4METL"))
BLOCKED.append((PWX - 128, PWY - 260, PWX + 128, PWY + 260))
LOOT += [(PWX + 120, PWY + 40, gz_ + 8, 2, "default")]

# --- cabins ---------------------------------------------------------------------------------
group("poi:cabins")
for k, (x, y) in enumerate(CABINS):
    z = H[ci(x)][ci(y)]
    building(x - 160, y - 128, x + 160, y + 128, z, floors=1, fh=144, tex="OUT_WD" if k % 2 else "OUT_WALL2",
             doors=[("s", 0), ("n", 0)], roof_access=False, floor_tex="CSTRIKE_FJ2DOCK")
    LOOT.append((x + 64, y, z + 8, 1 + (k % 2), "default"))

# ---------------------------------------------------------------------------------------
# v3 layers: the high routes across the valley and the underground's furnishings and loot
# (see docs/notes/BRmap.md "br_valley v3: three layers")


def slope_deck(xa, xb, ya, yb, za, zb, tex=WOOD, toptex=PLANK, rails=True):
    """a deck along x from xa (top za) to xb (top zb), ya..yb wide, in pieces of <=128 (each 16 thick
    under its low end), railings 40 high on both long sides"""
    n = max(1, int(math.ceil(abs(xb - xa) / 128.0)))
    for k in range(n):
        x0_, x1_ = xa + (xb - xa) * k / n, xa + (xb - xa) * (k + 1) / n
        z0_, z1_ = za + (zb - za) * k / n, za + (zb - za) * (k + 1) / n
        lo_, hi_ = (x0_, x1_) if x0_ < x1_ else (x1_, x0_)
        zl, zh = (z0_, z1_) if x0_ < x1_ else (z1_, z0_)
        D(prism([(lo_, ya, zl), (hi_, ya, zh), (hi_, yb, zh), (lo_, yb, zl)], min(zl, zh) - 16, tex, tex, bottom=tex, topscale=1))
        if rails:
            for (r0, r1) in ((ya - 8, ya), (yb, yb + 8)):
                D(prism([(lo_, r0, zl + 40), (hi_, r0, zh + 40), (hi_, r1, zh + 40), (lo_, r1, zl + 40)], min(zl, zh) - 16, RAIL, RAIL))


# --- the old rail viaduct (v4): from the radio hill's cliff edge (z 448) east over the river and the north bridge,
# then south along the lookout's flank, down to the lookout mesa (z 320): the north's high line. Piers every
# ~320 with stepped corbels (the arches), a 48-thick deck with the old rails on it, parapets. Ways off: the
# hill, the lookout, and a long flight of stone stairs down to the east bank half way
group("high:viaduct")
VD_Y0, VD_Y1, VD_Z = 2816, 2944, 448
VB_X0, VB_X1, VB_Y = 2208, 2336, 1536          # the south leg, down to the lookout's north edge at z 320
BR_X0, BR_X1 = 1888, 1984                     # the branch stairs down to the east bank
for xa in range(0, VB_X1, 512):
    D(box(xa, VD_Y0, VD_Z - 48, min(xa + 512, VB_X1), VD_Y1, VD_Z, CONC, top="OUT_PAVE1"))
for yy in (VD_Y0 + 32, VD_Y1 - 40):            # the old rails (you walk over them)
    NC(box(0, yy, VD_Z, VB_X1 - 64, yy + 8, VD_Z + 6, RAIL))
for (a, b) in ((0, BR_X0), (BR_X1, VB_X0)):    # parapets, with gaps for the branch stairs and the south leg
    D(box(a, VD_Y0, VD_Z, b, VD_Y0 + 8, VD_Z + 40, CONC))
D(box(0, VD_Y1 - 8, VD_Z, VB_X1, VD_Y1, VD_Z + 40, CONC))
D(box(VB_X1 - 8, VD_Y0, VD_Z, VB_X1, VD_Y1 - 8, VD_Z + 40, CONC))
connector("walkway", "viaduct deck", (-18, (VD_Y0 + VD_Y1) / 2, VD_Z), (-1, 0), (VB_X0 - 40, (VD_Y0 + VD_Y1) / 2, VD_Z), (1, 0))
# the south leg: a gentle slope down to the lookout (pieces of 256, parapets both sides)
nleg = 5
for k in range(nleg):
    ya, yb = VD_Y0 - (VD_Y0 - VB_Y) * k / nleg, VD_Y0 - (VD_Y0 - VB_Y) * (k + 1) / nleg
    za, zb = VD_Z - (VD_Z - 320) * k / nleg, VD_Z - (VD_Z - 320) * (k + 1) / nleg
    D(prism([(VB_X0, yb, zb), (VB_X1, yb, zb), (VB_X1, ya, za), (VB_X0, ya, za)], zb - 48, CONC, CONC, topscale=1))
    for (r0, r1) in ((VB_X0, VB_X0 + 8), (VB_X1 - 8, VB_X1)):
        D(prism([(r0, yb, zb + 40), (r1, yb, zb + 40), (r1, ya, za + 40), (r0, ya, za + 40)], zb - 48, CONC, CONC))
connector("ramp", "viaduct south leg", ((VB_X0 + VB_X1) / 2, VB_Y - 18, 320), (0, -1), ((VB_X0 + VB_X1) / 2, VD_Y0 + 40, VD_Z), (0, 1))
# the branch stairs, with one sloped rail on each side
# (down the slope: the flight ends at the first step that reaches the ground)
for n_ in range(1, 40):
    if VD_Z - 16 * n_ <= ground((BR_X0 + BR_X1) / 2, VD_Y0 - 24 * n_ - 12) + 8:
        break
BR_FOOT = VD_Y0 - 24 * n_
stairs(BR_X0, BR_FOOT, BR_X1, VD_Y0, VD_Z - 16 * n_, VD_Z, "n", side=CONC, top="OUT_PAVE1", name="viaduct stairs")
for (r0, r1) in ((BR_X0 - 8, BR_X0), (BR_X1, BR_X1 + 8)):
    D(prism([(r0, BR_FOOT, VD_Z - 16 * n_ + 40), (r1, BR_FOOT, VD_Z - 16 * n_ + 40), (r1, VD_Y0, VD_Z + 40), (r0, VD_Y0, VD_Z + 40)],
            VD_Z - 16 * n_, CONC, CONC))


def pier(px, py, w, d, ztop):
    """a stone pier from the ground (or the river bed) to under the deck, with two corbels each way along the deck"""
    gz = BED_Z if KIND.get((ci(px), ci(py))) == "river" else min(ground(px + dx, py + dy) for dx in (-w, w) for dy in (-d, d)) - 16
    D(box(px - w, py - d, gz, px + w, py + d, ztop, CONC))
    for (k, o) in ((1, 48), (2, 96)):
        D(box(px - w - o, py - d, ztop - 48 * (3 - k), px - w, py + d, ztop - 48 * (2 - k), CONC))
        D(box(px + w, py - d, ztop - 48 * (3 - k), px + w + o, py + d, ztop - 48 * (2 - k), CONC))


for px in (160, 400, 960, 1440, 1760, 2080):
    if not near_road(px, (VD_Y0 + VD_Y1) / 2, 48):
        pier(px, (VD_Y0 + VD_Y1) / 2, 32, 40, VD_Z - 48)
D(box(VB_X0 + 24, VD_Y0 + 24, min(ground(VB_X0, VD_Y0), ground(VB_X1, VD_Y1)) - 16, VB_X1 - 24, VD_Y1 - 24, VD_Z - 48, CONC))   # the corner pier
for k, py in enumerate((2560, 2240, 1920)):
    zt = VD_Z - (VD_Z - 320) * (VD_Y0 - py) / (VD_Y0 - VB_Y) - 48
    if not near_road((VB_X0 + VB_X1) / 2, py, 48):
        D(box(VB_X0 + 24, py - 32, min(ground(VB_X0 + 24, py), ground(VB_X1 - 24, py)) - 16, VB_X1 - 24, py + 32, zt, CONC))
BLOCKED.append((-16, VD_Y0 - 32, VB_X1 + 32, VD_Y1 + 32))
BLOCKED.append((VB_X0 - 32, VB_Y, VB_X1 + 32, VD_Y0))
BLOCKED.append((BR_X0 - 32, BR_FOOT - 32, BR_X1 + 32, VD_Y0))
LOOT += [(704, 2880, VD_Z + 8, 3, "default"), (2272, 2200, VD_Z - 64 + 8, 3, "default")]

# --- the town's roofs reach the river: from the store's roof east over the yard to the middle bridge, down
# a flight of stairs at the bank (the rooftop quarter joins the river crossing)
group("high:store-river")
ZT = 64 + 160
D(box(0, 752, ZT - 16, 512, 848, ZT, WOOD, top=PLANK))
D(box(0, 848, ZT, 520, 856, ZT + 40, RAIL))
D(box(0, 744, ZT, 416, 752, ZT + 40, RAIL))
D(box(512, 752, ZT, 520, 848, ZT + 40, RAIL))
for px in (192, 400):
    D(box(px, 784, ground(px, 800) - 16, px + 16, 816, ZT - 16, "OUT_WD"))
connector("walkway", "store-river walkway", (-18, 800, ZT), (-1, 0), (380, 800, ZT), (1, 0))
zf = 0
for _ in range(6):
    n_ = int(math.ceil((ZT - zf) / 16.0 - 0.01))
    zf = max(ground(x_, y_) for x_ in (418, 464, 510) for y_ in (752 - 24 * n_ + 2, 752 - 24 * n_ + 40))
n_ = int(math.ceil((ZT - zf) / 16.0 - 0.01))
sb_ = 752 - 24 * n_
stairs(416, sb_, 512, 752, ZT - 16 * n_, ZT, "n", side=WOOD, top=PLANK, name="store-river stairs")
for (r0, r1) in ((408, 416), (512, 520)):
    D(prism([(r0, sb_, ZT - 16 * n_ + 40), (r1, sb_, ZT - 16 * n_ + 40), (r1, 752, ZT + 40), (r0, 752, ZT + 40)], ZT - 16 * n_, RAIL, RAIL))
BLOCKED.append((-16, sb_ - 32, 552, 880))
# --- the east ridge trail (v5): a plank catwalk along the east ridge's crest from above the quarry's south rim to
# the bunker yard, a lookout hut with a mast half way (the south-east's landmark), stairs down at both ends
group("high:ridgetrail")
RT_X0, RT_X1, RT_YS, RT_YN = 2592, 2688, -1850, -800
HY = -1300
RT_Z = round((max(ground(x_, y_) for x_ in (RT_X0, RT_X1) for y_ in range(RT_YS, RT_YN + 1, 64)) + 72) / 8) * 8
for ya in range(RT_YS, RT_YN, 448):
    yb = min(ya + 448, RT_YN)
    D(box(RT_X0, ya, RT_Z - 16, RT_X1, yb, RT_Z, WOOD, top=PLANK))
    for (ra, rb) in ((ya, min(yb, HY - 112)), (max(ya, HY + 112), yb)):    # (open onto the hut's platform)
        if rb - ra > 1:
            D(box(RT_X0 - 8, ra, RT_Z, RT_X0, rb, RT_Z + 40, WOOD))
    D(box(RT_X1, ya, RT_Z, RT_X1 + 8, yb, RT_Z + 40, WOOD))
    for py in (ya + 32, yb - 48):
        if ground(RT_X0 + 40, py) < RT_Z - 40:
            D(box(RT_X0 + 32, py, ground(RT_X0 + 40, py) - 16, RT_X0 + 48, py + 16, RT_Z - 16, WOOD))
            D(box(RT_X1 - 48, py, ground(RT_X1 - 40, py) - 16, RT_X1 - 32, py + 16, RT_Z - 16, WOOD))
connector("walkway", "east ridge trail", ((RT_X0 + RT_X1) / 2, RT_YS + 40, RT_Z), (0, 1), ((RT_X0 + RT_X1) / 2, RT_YN - 40, RT_Z), (0, -1))
# the lookout hut: a platform west of the trail half way, waist walls, posts, a roof, a mast with a red lamp
HY = -1300
HX0, HX1 = RT_X0 - 232, RT_X0 - 8
D(box(HX0, HY - 112, RT_Z - 16, HX1, HY + 112, RT_Z, WOOD, top=PLANK))
for (a, b, c, d) in ((HX0, HY - 112, HX0 + 8, HY + 112), (HX0, HY - 112, HX1, HY - 104), (HX0, HY + 104, HX1, HY + 112)):
    D(box(a, b, RT_Z, c, d, RT_Z + 48, WOOD))
for (px, py) in ((HX0, HY - 112), (HX1 - 16, HY - 112), (HX0, HY + 96), (HX1 - 16, HY + 96)):
    D(box(px, py, min(ground(px, py), ground(px + 16, py + 16)) - 16, px + 16, py + 16, RT_Z + 144, WOOD))
D(frustum([(HX0 - 16, HY - 128), (HX1 + 16, HY - 128), (HX1 + 16, HY + 128), (HX0 - 16, HY + 128)], RT_Z + 144, RT_Z + 208, 0.0, "OUT_ROOF1B"))
D(box(HX0 + 8, HY - 8, RT_Z + 200, HX0 + 24, HY + 8, RT_Z + 640, RAIL))
NC(box(HX0 + 10, HY - 6, RT_Z + 640, HX0 + 22, HY + 6, RT_Z + 652, "~SPOTRED"))
point("light", HX0 + 16, HY, RT_Z + 600, _light="255 60 40 120")
for (end, dirn) in ((RT_YS, "s"), (RT_YN, "n")):
    zf = 0
    for _ in range(6):
        n_ = int(math.ceil((RT_Z - zf) / 16.0 - 0.01))
        far = end - 24 * n_ if dirn == "s" else end + 24 * n_
        zf = max(ground(x_, far + (12 if dirn == "s" else -12)) for x_ in (RT_X0 + 8, RT_X1 - 8))
    n_ = int(math.ceil((RT_Z - zf) / 16.0 - 0.01))
    if dirn == "s":
        stairs(RT_X0, end - 24 * n_, RT_X1, end, RT_Z - 16 * n_, RT_Z, "n", side=WOOD, top=PLANK, name="ridge trail south stairs")
    else:
        stairs(RT_X0, end, RT_X1, end + 24 * n_, RT_Z - 16 * n_, RT_Z, "s", side=WOOD, top=PLANK, name="ridge trail north stairs")
    yy0, yy1 = (end - 24 * n_, end) if dirn == "s" else (end, end + 24 * n_)
    BLOCKED.append((RT_X0 - 48, yy0 - 48, RT_X1 + 48, yy1 + 48))
BLOCKED.append((HX0 - 48, RT_YS, RT_X1 + 48, RT_YN))
LOOT += [(HX0 + 120, HY + 40, RT_Z + 8, 3, "default")]
WEAP += [(HX0 + 120, HY - 40, RT_Z + 8, "snipers", 3)]

# --- the cliff ring: a few tall rock spires in front of it, a waterfall where the river comes in, a boarded road
# tunnel where the main road leaves to the west (all out of reach, behind the play edge)
group("ring")
for k, (x, y, r, h) in enumerate([(-3720, 2600, 130, 1700), (-3700, -2300, 110, 1500), (3700, 1900, 140, 1800), (3720, -900, 120, 1600),
                                  (-1900, 3720, 120, 1650), (2400, 3700, 130, 1750), (-600, -3710, 120, 1500), (2900, -3700, 110, 1600)]):
    D(frustum(blob(x, y, r, 6, 77 + 13 * k), 0, h, 0.35, ROCK2, scale=4))      # (scale 4: little lightmap)
NC(box(512, PLAY + 40, BED_Z, 1280, PLAY + 48, 240, "+0WATERF"))
for (a, b) in ((-3712, -3696),):
    D(box(a, -180, 0, b, -140, 300, CONC))
    D(box(a, 20, 0, b, 60, 300, CONC))
    D(box(a, -180, 300, b, 60, 340, CONC))
    D(box(a + 4, -140, 0, a + 12, 20, 300, "OUT_WD"))

# cover on the town's roofs (something to duck behind on the way across): a water tank on house A, chimney
# stacks on house D, house F and the store
group("high:roofcover")
D(prism([(px, py, 384 + 96) for (px, py) in regular(-1440, 760, 48, 8)], 384, "CSTRIKE_ME2RSTW", "CSTRIKE_ME4METL"))
for (x, y, z) in ((-1300, -60, 224), (-380, 40, 224), (-150, 900, 224)):
    D(box(x - 24, y - 24, z, x + 24, y + 24, z + 96, "OUT_WALL1A", top="CSTRIKE_ME4METL"))

# --- the fuel line: a pipe bridge from the factory office's roof (z 320) down to the gas station shop's roof
# (z 160) over the south yard: the factory's roofs and the gas station join up high
group("high:pipebridge")
slope_deck(-1920, -1216, -2848, -2768, 320, 160)
connector("ramp", "fuel line bridge", (-1192, -2808, 160), (1, 0), (-1944, -2808, 320), (-1, 0))
for px in (-1760, -1568, -1376):
    zt = 320 - 160 * (px + 1920) / 704.0
    D(box(px - 12, -2820, min(ground(px - 12, -2820), ground(px + 12, -2796)) - 16, px + 12, -2796, zt - 16, "CSTRIKE_ME4METL"))
BLOCKED.append((-1936, -2880, -1200, -2736))
LOOT += [(-1568, -2808, 320 - 160 * 352 / 704.0 + 8, 3, "default")]

# --- the level below, place by place (v4): each its own look, light, sound, cover; loot tier 3 and SMGs, the
# best (tier 5, rifles) only in three vaults, each behind one grate-floored way in that you hear
KEEP_CLEAR = [(cx(c[0]) - 64, cx(c[1]) - 64, cx(c[0] + 1) + 64, cx(c[1] + 1) + 64) for c in TRENCH_CELLS] + \
             [(cx(i) - 48, cx(j) - 48, cx(i + 1) + 48, cx(j + 1) + 48) for (i, j) in HATCHES] + \
             [(x0 - 32, y0 - 32, x1 + 32, y1 + 32) for n_, x0, y0, x1, y1 in TUNNELS if n_ not in ("mine", "river")]
PL = {n_: (x0, y0, x1, y1, st, fl) for (n_, x0, y0, x1, y1, st, fl) in PLACES}


def clear_below(x, y, r):
    if not any(a - r < x < c + r and b - r < y < d + r for (a, b, c, d) in KEEP_CLEAR):
        return slab_open(x - r, y - r) and slab_open(x + r, y + r) and slab_open(x - r, y + r) and slab_open(x + r, y - r)
    return False


def pillar_rock(x, y, r, k, fl):
    if clear_below(x, y, r + 48):
        b = prism([(px, py, SLAB_TOP + 8) for (px, py) in blob(x, y, r, 7, 31 * k + 7)], fl - 8, ROCK2, ROCK2)
        if not clashes(b):
            D(b)


def lights_in(name, step=384, bright=120):
    x0, y0, x1, y1, st, fl = PL[name]
    bright = max(bright, 210)
    if min(x1 - x0, y1 - y0) < 600:
        step = min(step, 256)
    lamps = Ent("func_illusionary")
    nx, ny = max(1, int((x1 - x0) // step)), max(1, int((y1 - y0) // step))
    for a in range(nx):
        for b in range(ny):
            lx, ly = x0 + (x1 - x0) * (a + 0.5) / nx, y0 + (y1 - y0) * (b + 0.5) / ny
            point("light", lx, ly, SLAB_TOP - 40, _light="%s %d" % (PSTYLE[st][2], bright))
            lamps.brushes.append(box(lx - 12, ly - 12, SLAB_TOP - 6, lx + 12, ly + 12, SLAB_TOP, PSTYLE[st][3]))
    ents.append(lamps)
    if PSTYLE[st][4]:
        point("ambient_generic", (x0 + x1) / 2, (y0 + y1) / 2, fl + 96, message=PSTYLE[st][4], health=4, spawnflags=4, pitch=100)


def cave(name, k):
    """a cave: corners cut off, rock pillars, rock hanging from the ceiling, a raised shelf you walk up onto"""
    x0, y0, x1, y1, st, fl = PL[name]
    group("under:" + name)
    for (ax, ay, sx, sy) in ((x0, y0, 1, 1), (x1, y0, -1, 1), (x0, y1, 1, -1), (x1, y1, -1, -1)):
        tri = [(ax, ay, SLAB_TOP), (ax + sx * 224, ay, SLAB_TOP), (ax, ay + sy * 224, SLAB_TOP)]
        tb_ = prism(tri, fl - 8, ROCK2, ROCK2)
        if clear_below(ax + sx * 75, ay + sy * 75, 80) and not clashes(tb_):
            D(tb_)
    w, h = x1 - x0, y1 - y0
    spots = [(0.3, 0.3), (0.7, 0.7), (0.3, 0.75), (0.7, 0.25)] if max(w, h) < 1400 else \
            [(0.3, 0.15), (0.7, 0.35), (0.3, 0.55), (0.7, 0.75), (0.35, 0.9)] if h > w else [(0.15, 0.3), (0.35, 0.7), (0.55, 0.3), (0.75, 0.7), (0.9, 0.35)]
    for m, (u, v) in enumerate(spots):
        pillar_rock(x0 + u * w, y0 + v * h, 72 + 18 * (m % 3), k * 7 + m, fl)
    for m, (u, v) in enumerate([(0.5, 0.5), (0.2, 0.9) if h > w else (0.9, 0.2)]):     # the ceiling coming down
        x, y = x0 + u * w, y0 + v * h
        if clear_below(x, y, 140):
            D(frustum(blob(x, y, 130, 6, k * 13 + m), -420, SLAB_TOP + 8, 1.0, ROCK2))
    for m, (u, v) in enumerate([(0.55, 0.12) if h > w else (0.12, 0.55)]):            # a shelf 48 up, a slope onto it
        x, y = x0 + u * w, y0 + v * h
        if clear_below(x, y, 200):
            poly = blob(x, y, 180, 7, k * 5 + 1)
            D(prism([(px, py, fl + 24 + 24 * (px - x) / 180.0) for (px, py) in poly], fl - 8, ROCK2, ROCK2, topscale=2))
    lights_in(name, step=320, bright=230)


group("under:deep stope")
stairs(-896, 2880, -800, 3144, -704, TUN_Z, "s", side=CONC, top=CONC_F, name="deep stope steps", out_bottom="e")
lights_in("deep stope", step=256, bright=240)
for k, n_ in enumerate(("fort caves", "north caves", "deep chamber", "lumber caves", "ridge caves")):
    cave(n_, k)

# the old mine: timber frames along the gallery, carts, the west workings (timber props, a cart, ore)
group("under:mine")
for fx in range(-2048, 2432, 448):
    if any(x0 - 48 <= fx <= x1 + 48 for (_n, x0, y0, x1, y1, _s, _f) in PLACES if y0 < 2304 and y1 > 2112) or \
            any(cx(i) - 48 <= fx <= cx(i + 1) + 48 for (i, j) in HATCHES if cx(j) < 2304 and cx(j + 1) > 2112):
        continue
    D(box(fx, 2112, TUN_Z, fx + 16, 2128, SLAB_TOP - 16, "OUT_WD"))
    D(box(fx, 2288, TUN_Z, fx + 16, 2304, SLAB_TOP - 16, "OUT_WD"))
    NC(box(fx, 2112, SLAB_TOP - 16, fx + 16, 2304, SLAB_TOP, "OUT_WD"))
for (x, y) in [(-1800, 2136), (300, 2180), (2100, 2136)]:
    P(box(x, y, TUN_Z, x + 96, y + 56, TUN_Z + 48, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))
group("under:west mine")
x0, y0, x1, y1, st, fl = PL["west mine"]
for (x, y) in [(-3072, 1856), (-2688, 1856), (-3072, 2368), (-2560, 2432)]:
    if clear_below(x, y, 40):
        D(box(x - 12, y - 12, fl, x + 12, y + 12, SLAB_TOP, "OUT_WD"))             # pit props
for k, (x, y) in enumerate([(-2944, 2112), (-3200, 1760)]):
    pillar_rock(x, y, 84, 40 + k, fl)
P(box(-3232, 2400, fl, -3136, 2456, fl + 48, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))
lights_in("west mine", bright=110)
# lighting along the tunnels
for name, tx0, ty0, tx1, ty1 in TUNNELS:
    lx, ly = tx1 - tx0, ty1 - ty0
    st = TUNNEL_STYLE.get(name, "utility")
    nl = max(1, int(max(lx, ly) // 448))
    lamps = Ent("func_illusionary")
    for k in range(nl):
        f = (k + 0.5) / nl
        x = tx0 + lx * (f if lx >= ly else 0.5)
        y = ty0 + ly * (f if ly > lx else 0.5)
        if any(a <= x < c and b <= y < d for (_n, a, b, c, d, _s, _f) in PLACES):
            continue
        point("light", x, y, SLAB_TOP - 24, _light="%s 130" % PSTYLE[st][2])
        lamps.brushes.append(box(x - 12, y - 12, SLAB_TOP - 4, x + 12, y + 12, SLAB_TOP, PSTYLE[st][3]))
    if lamps.brushes:
        ents.append(lamps)
    if st != "grate":
        stripe = Ent("func_illusionary")
        if lx >= ly:
            stripe.brushes.append(box(tx0, ty0, TUN_Z + 40, tx1, ty0 + 2, TUN_Z + 56, PSTYLE[st][3]))
            stripe.brushes.append(box(tx0, ty1 - 2, TUN_Z + 40, tx1, ty1, TUN_Z + 56, PSTYLE[st][3]))
            ends_ = ((tx0 + 48, (ty0 + ty1) / 2), (tx1 - 48, (ty0 + ty1) / 2))
        else:
            stripe.brushes.append(box(tx0, ty0, TUN_Z + 40, tx0 + 2, ty1, TUN_Z + 56, PSTYLE[st][3]))
            stripe.brushes.append(box(tx1 - 2, ty0, TUN_Z + 40, tx1, ty1, TUN_Z + 56, PSTYLE[st][3]))
            ends_ = (((tx0 + tx1) / 2, ty0 + 48), ((tx0 + tx1) / 2, ty1 - 48))
        for (sx_, sy_) in ends_:
            stripe.brushes.append(box(sx_ - 2, sy_ - 32, SLAB_TOP - 48, sx_ + 2, sy_ + 32, SLAB_TOP - 32, "+0EXIT") if lx >= ly else
                                  box(sx_ - 32, sy_ - 2, SLAB_TOP - 48, sx_ + 32, sy_ + 2, SLAB_TOP - 32, "+0EXIT"))
        ents.append(stripe)

# the cisterns: a columned hall 256 high, a water channel down its middle, steps down from the old sewer
group("under:cisterns")
x0, y0, x1, y1, st, fl = PL["cisterns"]
for x in (-1472, -1088, -704, -320):
    for y in (-1088, -704, -320):
        if clear_below(x, y, 40):
            D(box(x - 24, y - 24, fl, x + 24, y + 24, SLAB_TOP, "-0TNNL_W12A"))
W(box(x0, -544, -592, x1, -480, -582, "!WATERBLUE"))
stairs(-992, -224, -800, -128, fl, TUN_Z, "n", side="-0TNNL_W12A", top="-0TNNL_FLR1B", name="cisterns north steps")
stairs(-992, -1280, -800, -1184, fl, TUN_Z, "s", side="-0TNNL_W12A", top="-0TNNL_FLR1B", name="cisterns south steps")
lights_in("cisterns", step=512, bright=140)
# the sewer: a channel down its middle (16 down, water), ledges either side
group("under:sewer")
W(box(-1472, 384, TUN_Z - 16, 256, 448, TUN_Z - 6, "!WATERBLUE"))
point("ambient_generic", -700, 416, TUN_Z + 64, message="ambience/drips.wav", health=4, spawnflags=4, pitch=90)
# the town vault (tier 5): strongboxes, one grate way in from the sewer
group("under:town vault")
lights_in("town vault", step=384, bright=150)
for (x, y) in [(-1040, 680), (-720, 680), (-1040, 940)]:
    crate(x, y, TUN_Z, 64)
# the boiler house under the factory: boilers, pipes along the north wall, a work bench
group("under:boiler house")
x0, y0, x1, y1, st, fl = PL["boiler house"]
for (x, y) in [(-3104, -3104), (-2848, -3104), (-2592, -3104)]:
    if clear_below(x, y, 112):
        D(prism([(px, py, fl + 176) for (px, py) in regular(x, y, 96, 8, math.pi / 8)], fl, "CSTRIKE_ME2RSTW", "CSTRIKE_ME2RSTW"))
NC(box(x0 + 32, y1 - 48, -400, x1 - 32, y1 - 24, -376, "-0GENPIPE1"))
NC(box(x0 + 32, y1 - 48, -440, x1 - 32, y1 - 24, -416, "-0GENPIPE1"))
P(box(-2560, -2752, fl, -2368, -2688, fl + 40, "CSTRIKE_ME4METL"))
lights_in("boiler house", bright=150)
# the tank vault under the gas station: three fuel tanks, pipes
group("under:tank vault")
x0, y0, x1, y1, st, fl = PL["tank vault"]
for (x, y) in [(-1408, -3136), (-1088, -3136), (-768, -3136)]:
    if clear_below(x, y, 120):
        D(prism([(px, py, fl + 160) for (px, py) in regular(x, y, 104, 8, math.pi / 8)], fl, "CSTRIKE_OE6TNKS", "CSTRIKE_ME4METL"))
NC(box(x0 + 32, -2992, -420, x1 - 32, -2968, -396, "-0GENPIPE1"))
lights_in("tank vault", bright=120)
# the boathouse cellar and the pumping works: pump blocks, pipes, a sump
for nm in ("boathouse cellar", "pumping works"):
    group("under:" + nm)
    x0, y0, x1, y1, st, fl = PL[nm]
    w, h = x1 - x0, y1 - y0
    for (u, v) in ((0.3, 0.3), (0.7, 0.7)) if nm == "boathouse cellar" else ((0.3, 0.2), (0.7, 0.4), (0.3, 0.6), (0.7, 0.85)):
        x, y = x0 + u * w, y0 + v * h
        if clear_below(x, y, 100):
            D(box(x - 64, y - 48, fl, x + 64, y + 48, fl + 96, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))
    NC(box(x0 + 32, y0 + 24, -400, x1 - 32, y0 + 48, -376, "-0GENPIPE1"))     # (along the south wall)
    lights_in(nm, bright=120)
# the quarry workings: rock pillars, timber props
cave_k = 9
group("under:quarry workings")
x0, y0, x1, y1, st, fl = PL["quarry workings"]
for m, (u, v) in enumerate([(0.25, 0.2), (0.7, 0.45), (0.3, 0.65), (0.75, 0.85)]):
    pillar_rock(x0 + u * (x1 - x0), y0 + v * (y1 - y0), 90, 70 + m, fl)
lights_in("quarry workings", bright=130)
# the bunker: a long hall, the guard room (a barrier), the generator room (generators), the armoury a level down
group("under:bunker")
P(box(2368, -2496, TUN_Z, 2624, -2464, TUN_Z + 48, "OUT_SNBAG", top="OUT_SNBAGB"))
for (x, y) in [(2912, -2560), (3104, -2560)]:
    D(box(x - 48, y - 64, TUN_Z, x + 48, y + 64, TUN_Z + 112, "CSTRIKE_ME2RSTW", top="CSTRIKE_ME4METL"))
stairs(3008, -3272, 3104, -3008, -704, TUN_Z, "n", side=CONC, top=CONC_F, name="armoury steps")
for nm in ("bunker hall", "guard room", "generator room", "armoury"):
    lights_in(nm, step=384, bright=150 if nm == "armoury" else 120)

# --- loot below
LOOT += [(-1520, 448, TUN_Z + 8, 3, "default"),        # the sewer's west end
         (-2900, -1100, TUN_Z + 8, 3, "default"), (-2700, 900, TUN_Z + 8, 3, "default"),     # the fort caves
         (-2900, 2200, TUN_Z + 8, 3, "default"),        # the west mine
         (-1700, 2900, TUN_Z + 8, 3, "default"),        # the north caves
         (-500, 2300, TUN_Z + 8, 3, "default"),         # the deep chamber
         (-600, 3120, -704 + 8, 5, "default"),          # ...its stope (a vault)
         (2900, 2500, TUN_Z + 8, 3, "default"),         # the lumber caves
         (-1300, -900, -576 + 8, 3, "default"), (-500, -400, -576 + 8, 3, "default"),        # the cisterns
         (-864, 880, TUN_Z + 8, 5, "default"),          # the town vault
         (-2700, -2800, TUN_Z + 8, 3, "default"),       # the boiler house
         (-1100, -2800, TUN_Z + 8, 3, "default"),       # the tank vault
         (128, -1200, TUN_Z + 8, 3, "default"),         # the boathouse cellar
         (1900, 0, TUN_Z + 8, 3, "default"),            # the pumping works
         (1980, -1700, TUN_Z + 8, 3, "default"),        # the ridge caves
         (2700, 900, TUN_Z + 8, 3, "default"), (3000, -800, TUN_Z + 8, 3, "default"),      # the quarry workings
         (2500, -2600, TUN_Z + 8, 3, "default"),        # the guard room
         (3100, -3200, -704 + 8, 5, "default"),         # the armoury (a vault)
         (1100, -1920, TUN_Z + 8, 3, "default")]        # the south tunnel under the river
WEAP += [(-2800, -300, TUN_Z + 8, "smgs", 3), (-2600, 2000, TUN_Z + 8, "smgs", 3), (-300, 2600, TUN_Z + 8, "smgs", 3),
         (-1300, -400, -576 + 8, "smgs", 3), (1650, 600, TUN_Z + 8, "smgs", 3), (2800, 300, TUN_Z + 8, "smgs", 3),
         (2900, 2100, TUN_Z + 8, "smgs", 3),
         (-760, 760, TUN_Z + 8, "rifles", 5), (-480, 3180, -704 + 8, "rifles", 5), (3200, -3100, -704 + 8, "rifles", 5)]

# ---------------------------------------------------------------------------------------
# cover in the open: rocks and trees (detail), placed on the ground


def steep(x, y, r=56):
    """too steep to stand on round x, y (cover there leaves ledges you can't step off onto the slope)"""
    zs = [ground(x + dx, y + dy) for dx in (-r, 0, r) for dy in (-r, 0, r)]
    return max(zs) - min(zs) > 0.66 * 2 * r


def blocked(x, y, m=0):
    if abs(x) > PLAY - 192 or abs(y) > PLAY - 192:
        return True
    if steep(x, y):
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
DROPS = [(-1536, 1792), (1024, -2816), (-2048, -1280), (2800, 1150), (0, -768), (-512, 1920), (2560, -1600), (-2688, -1664)]


def drop_ok(x, y):
    """a crate lands here and bots can stand beside it (they open it from within 80 units): no river,
    trench or mesa under it or 96 round it, outside every structure's footprint, the ground even"""
    for dx in (-96, 0, 96):
        for dy in (-96, 0, 96):
            k = KIND.get((ci(x + dx), ci(y + dy))) or ""
            if k in ("river", "cut") or k.startswith("mesa"):
                return False
    if abs(x) > PLAY - 384 or abs(y) > PLAY - 384:
        return False
    if any(x0 - 32 <= x <= x1 + 32 and y0 - 32 <= y <= y1 + 32 for (x0, y0, x1, y1) in BLOCKED):
        return False
    # well back from the river (its banks, boulders and the docks), and not in a dip
    if any(math.hypot(cx(i) + CELL / 2 - x, cx(j) + CELL / 2 - y) < 512 for (i, j) in RIVER):
        return False
    if ground(x, y) < 0:
        return False
    # not on the slope down into (or up onto) a pad or mesa at another height (the quarry pit's rim)
    z = ground(x, y)
    for _n, x0, y0, x1, y1, pz in PADS:
        if abs(pz - z) > 32 and x0 - 384 <= x <= x1 + 384 and y0 - 384 <= y <= y1 + 384:
            return False
    for _n, x0, y0, x1, y1, pz, _r in MESAS:
        if x0 - 256 <= x <= x1 + 256 and y0 - 256 <= y <= y1 + 256:
            return False
    zs = [ground(x + dx, y + dy) for dx in (-64, 0, 64) for dy in (-64, 0, 64)]
    return max(zs) - min(zs) <= 40


def dry(x, y):
    """the nearest spot to x, y where drop_ok holds (rings of 64)"""
    for r in range(0, 1281, 64):
        n = max(1, int(2 * math.pi * r / 64))
        for k in range(n):
            a = 2 * math.pi * k / n
            px, py = int(round((x + r * math.cos(a)) / 32) * 32), int(round((y + r * math.sin(a)) / 32) * 32)
            if drop_ok(px, py):
                return (px, py)
    raise SystemExit("no dry ground near the supply drop at %d %d" % (x, y))


_moved = [(d, dry(*d)) for d in DROPS]
for a, b in _moved:
    if a != b:
        print("supply drop %s moved to %s (river, trench or a structure)" % (a, b))
DROPS = [b for _, b in _moved]


def near_drop(x, y, r):
    return any(math.hypot(x - dx, y - dy) < r + 160 for dx, dy in DROPS)


def tree(x, y, z, u, v):
    """a trunk and irregular foliage from head height up (blocks sight; walk round it)"""
    global trees
    r = 52 + 34 * u
    top = z + 168 + 90 * v
    trunk = box(x - 12, y - 12, ground4(x, y, 12) - 32, x + 12, y + 12, z + 64, "OUT_WD", top="OUT_WD")
    base = blob(x, y, r, 7, int(u * 1e6) + 3, turn=v * 3)
    leaves = prism([(px, py, top) for (px, py) in base], z + 48, "OUT_SHRB1", "OUT_SHRB1")      # (its room, for placing)
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
    # a pine: three tiers, each narrower, the top one to a point; the lowest starts at head height
    hgt = top - (z + 48)
    tiers = [(z + 48, z + 48 + 0.42 * hgt, 1.0, 0.62), (z + 40 + 0.36 * hgt, z + 48 + 0.72 * hgt, 0.78, 0.4), (z + 40 + 0.66 * hgt, top, 0.52, 0.0)]
    for (zb_, zt_, sc, st_) in tiers:
        tp = [(x + (px - x) * sc, y + (py - y) * sc) for (px, py) in base]
        fol = frustum(tp, zb_, zt_, st_, "OUT_SHRB1")
        NC(fol)
        fol.kind = "foliage"
    NC(trunk)
    trunk.prop = True
    # what you bump into: one box round the lowest tier, from the ground to the top
    xs_, ys_ = [p[0] for p in base], [p[1] for p in base]
    cb = prism([(px, py, top) for (px, py) in base], ground4(x, y, r) - 16, "CLIP", "CLIP")      # (the canopy's own outline)
    cb.kind = "treeclip"
    D(cb)
    BLOCKED.append((x - r - 8, y - r - 8, x + r + 8, y + r + 8))
    trees += 1


def poly_gap(pa, pb):
    """the smallest distance between two convex polygons' outlines (0 if they cross)"""
    def seg(p, a, b_):
        dx, dy = b_[0] - a[0], b_[1] - a[1]
        t = max(0.0, min(1.0, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy or 1.0)))
        return math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy)
    if any(_inside(pb, *p) for p in pa) or any(_inside(pa, *p) for p in pb):
        return 0.0
    return min(min(seg(p, pb[k], pb[(k + 1) % len(pb)]) for k in range(len(pb)) for p in pa),
               min(seg(p, pa[k], pa[(k + 1) % len(pa)]) for k in range(len(pa)) for p in pb))


def rock(x, y, z, r, h, k, spire=False):
    """a boulder: an irregular block with a tilted top (spire: a top too steep to stand on: a tall outcrop
    nobody can be dropped onto and stuck on)"""
    global rocks
    poly = blob(x, y, r, 5 + k % 3, k, squash=0.65 + 0.35 * ((k * 7) % 10) / 10.0, turn=k * 0.7)
    ax, ay = (1.15 * math.cos(k), 1.15 * math.sin(k)) if spire else (0.25 * math.cos(k), 0.25 * math.sin(k * 1.7))
    b = prism([(px, py, z + h + ax * (px - x) + ay * (py - y)) for (px, py) in poly], z - 48, ROCK2, ROCK2)
    group("rock")
    if near_drop(x, y, r) or clashes(b, allow=("rock",)):
        return
    # rocks stand 40 apart: a near miss (or a V where two meet) leaves a slot a player drops into
    for c in _cells((b.bounds[0] - 48, b.bounds[1] - 48, 0, b.bounds[3] + 48, b.bounds[4] + 48, 0)):
        for o in GRID.get(c, ()):
            if o.group == "rock" and poly_gap(b.poly, o.poly) < 40:
                return
    D(b)
    b.prop = True
    BLOCKED.append((x - r - 24, y - r - 24, x + r + 24, y + r + 24))
    rocks += 1


def ground4(x, y, r):
    return min(ground(x - r, y - r), ground(x + r, y + r), ground(x - r, y + r), ground(x + r, y - r))


# the quarry's rock piles (irregular, tilted tops) and a spoil heap of gravel against the pit's west rim
for k, (x, y, r_, h) in enumerate(QUARRY_PILES):
    rock(x, y, -128, r_, h, 300 + 17 * k)
# --- small cover between the places (cheap): foxholes (a U of sandbags, open to one side) and berm lines
# (earth banks you crouch behind), on fairly level open ground
group("cover:dug")
dug = 0


def level(x, y, r):
    zs = [ground(x + dx, y + dy) for dx in (-r, 0, r) for dy in (-r, 0, r)]
    return max(zs) - min(zs) < 0.22 * 2 * r      # (gentle ground only)


def far_spread(cands, k, keep_from=()):
    """k of cands spread out (farthest-point), also kept away from keep_from"""
    out = []
    pool = list(cands)
    while pool and len(out) < k:
        best = max(pool, key=lambda p_: min([(p_[0] - q[0]) ** 2 + (p_[1] - q[1]) ** 2 for q in out + list(keep_from)] or [1e12]))
        out.append(best)
        pool = [p_ for p_ in pool if (p_[0] - best[0]) ** 2 + (p_[1] - best[1]) ** 2 > 700 ** 2]
    return out


_c = [(x, y) for x in range(-PLAY + 512, PLAY - 511, 128) for y in range(-PLAY + 512, PLAY - 511, 128)
      if not steep(x, y) and not blocked(x, y, 120) and level(x, y, 96)]
FOXHOLES = [(x, y, (x // 128 + y // 128) % 4) for (x, y) in far_spread(_c, 14)]
_c2 = [(x, y) for (x, y) in _c if not blocked(x, y, 240) and level(x, y, 200) and all(math.hypot(x - a, y - b) > 600 for (a, b, _r) in FOXHOLES)]
BERMS = [(x, y, "x" if (x // 128) % 2 else "y") for (x, y) in far_spread(_c2, 10)]


for (x, y, rot) in FOXHOLES:
    if blocked(x, y, 96) or not level(x, y, 96):
        continue
    z = min(ground(x + dx, y + dy) for dx in (-80, 80) for dy in (-80, 80)) - 4
    parts = [(-64, 48, 64, 80), (-80, -48, -48, 80), (48, -48, 80, 80)]          # open to the south, turned by rot
    for (a0, b0, a1, b1) in parts:
        pts = [(a0, b0), (a1, b1)]
        for _ in range(rot):
            pts = [(-py, px) for (px, py) in pts]
        xs_ = [p_[0] for p_ in pts]
        ys_ = [p_[1] for p_ in pts]
        bx0, by0, bx1, by1 = x + min(xs_), y + min(ys_), x + max(xs_), y + max(ys_)
        gl = [ground(px, py) for px in (bx0, bx1) for py in (by0, by1)]
        D(box(bx0, by0, min(gl) - 4, bx1, by1, max(gl) + 48, "OUT_SNBAG", top="OUT_SNBAGB"))   # (dug into its own ground)
    BLOCKED.append((x - 112, y - 112, x + 112, y + 112))
    dug += 1
for (x, y, along) in BERMS:
    if blocked(x, y, 220) or not level(x, y, 200):
        continue
    hl, hw = 192, 40
    rect = [(x - hl, y - hw), (x + hl, y - hw), (x + hl, y + hw), (x - hl, y + hw)] if along == "x" else \
           [(x - hw, y - hl), (x + hw, y - hl), (x + hw, y + hl), (x - hw, y + hl)]
    z = min(ground(px, py) for (px, py) in rect) - 8
    b_ = frustum(rect, z, max(ground(px, py) for (px, py) in rect) + 44, 0.45, DIRT)
    if not clashes(b_):
        D(b_)          # (earth: its foot runs into the slope)
        BLOCKED.append((x - hl - 48, y - hl - 48, x + hl + 48, y + hl + 48) if along == "y" else (x - hl - 48, y - hw - 64, x + hl + 48, y + hw + 64))
        dug += 1
group("terrain")
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
            b.prop = True

# the ridges' crests: rock outcrops and a line of pines along each (a crest 200 high with 200 of rock and trees
# on it blocks the view like a 400 ridge)
crest_k = 0
for (x0, y0, x1, y1, hh, hw) in RIDGES:
    L_ = math.hypot(x1 - x0, y1 - y0) or 1.0
    nx_, ny_ = -(y1 - y0) / L_, (x1 - x0) / L_
    for m, f in enumerate((0.15, 0.4, 0.62, 0.86)):
        x, y = x0 + (x1 - x0) * f + nx_ * (50 if m % 2 else -50), y0 + (y1 - y0) * f + ny_ * (50 if m % 2 else -50)
        if not blocked(x, y, 180):      # (clear of foxholes, berms and the rest: no pit closed off by a rock)
            rock(x, y, ground4(x, y, 120), 120 + 30 * (m % 2), 130 + 20 * ((m + crest_k) % 3), 500 + 7 * crest_k + m)   # (at most ~170: a safe drop)
    for m, f in enumerate((0.05, 0.27, 0.5, 0.73, 0.95)):
        x, y = x0 + (x1 - x0) * f + nx_ * (190 if m % 2 else -190), y0 + (y1 - y0) * f + ny_ * (190 if m % 2 else -190)
        if not blocked(x, y, 56):
            tree(x, y, ground(x, y), 0.4 + 0.12 * (m % 4), 0.5 + 0.1 * (m % 3))
    crest_k += 1
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
        tree(x, y, ground(x, y), a, d)
for t_ in range(520):
    seed, u = rnd(seed)
    seed, v = rnd(seed)
    x, y = -PLAY + 250 + u * (2 * PLAY - 500), -PLAY + 250 + v * (2 * PLAY - 500)
    if blocked(x, y, 64):
        continue
    tree(x, y, ground(x, y), u, v)
# bushes: low, to crouch behind, along the groves' edges and the roads' sides
for b_ in range(360):
    seed, u = rnd(seed)
    seed, v = rnd(seed)
    x, y = -PLAY + 250 + u * (2 * PLAY - 500), -PLAY + 250 + v * (2 * PLAY - 500)
    if blocked(x, y, 40):
        continue
    z = ground(x, y)
    b = prism([(px, py, z + 52) for (px, py) in blob(x, y, 46, 6, b_ * 17 + 1)], ground4(x, y, 40) - 16, "OUT_SHRB1", "OUT_SHRB1")
    group("bush")
    # a bush stands clear of trees and rocks (40 round it): no slots between them to drop into
    room = prism([(px, py, z + 140) for (px, py) in blob(x, y, 86, 6, b_ * 17 + 1)], z - 16, "NULL", "NULL")   # (no canopy low over it)
    # and well clear of tree canopies (a player-wide wedge between a bush and a canopy on a slope is a trap)
    wide = prism([(px, py, z + 140) for (px, py) in blob(x, y, 130, 6, b_ * 17 + 1)], z - 16, "NULL", "NULL")
    if near_drop(x, y, 46) or clashes(b, allow=("bush",)) or clashes(room, allow=("bush",)) or \
            any(o.group == "tree" for o in clashes(wide, allow=("bush",))):
        continue
    D(b)
    b.prop = True
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
# drop-in landing spots for BR (ls_marker kind=br_drop: people, campers and hunters come down over these;
# looters over a tier 2+ crate): the same open, ground-level picks
DROP_EXTRA = [(-888, 1236), (-3040, -2368), ((BR_X0 + BR_X1) / 2, BR_FOOT - 64), (-3232, 2112)]   # the hall's fire ladder,
# the factory's north ladder, the viaduct's stairs, the silo's stairs
# and one just past each stair trench's top (W1 can land bots by a way down: the drop-in spot decides their layer)
for (_n, i0, j0, i1, j1, d_) in TRENCHES:
    tx_, ty_ = {"s": ((cx(i0) + cx(i1)) / 2, cx(j1) + 160), "n": ((cx(i0) + cx(i1)) / 2, cx(j0) - 160),
                "w": (cx(i1) + 160, (cx(j0) + cx(j1)) / 2), "e": (cx(i0) - 160, (cx(j0) + cx(j1)) / 2)}[d_]
    DROP_EXTRA.append((tx_, ty_))
picks_drop = list(picks[:28]) + [(x, y, ground(x, y)) for (x, y) in DROP_EXTRA]
for (x, y, z) in picks_drop:
    point("ls_marker", x, y, z + 8, kind="br_drop", mode="br")
# the game's own spawn points (it needs some per team; gm_components moves players to the above)
for k, (x, y, z) in enumerate(picks[28:]):
    point("info_player_start" if k % 2 else "info_player_deathmatch", x, y, z + 36, angle=0)

# loot crates in the places (above), plus field crates in the open
field = spread(cands, 23, (0, 0, 0))[1:]
for k, (x, y, z) in enumerate(field):
    if k < 10:
        LOOT.append((x + 40, y + 40, z + 8, 1, "default"))
    else:
        # (the open ground keeps to SMGs and pistols: rifles are below ground and up high)
        WEAP.append((x - 40, y - 40, z + 8, "smgs" if k % 3 else "pistols", 2 if k % 3 else 1))
# a gun near every drop-in spot: a player who lands anywhere finds one in the loot-up (17 s). The walk
# from a spot to the nearest gun on the ground is at most GUN_RUN (about 10 s at 250 a second; the
# straight line here, the real walk is checked on the nav by navgen.py --check); where there is none,
# a pistol a short way off the spot
GUN_RUN = 2500


def gun_near(x, y, z, r, dz=100):
    return any(math.hypot(wx - x, wy - y) <= r and abs(wz - 8 - z) < dz for (wx, wy, wz, _t, _r) in WEAP)


added_guns = 0
for (x, y, z) in picks_drop:
    if gun_near(x, y, z, GUN_RUN * 0.55, dz=80):
        continue
    for r_, a_ in [(r, a) for r in (160, 224, 288, 384, 480, 576, 672) for a in range(0, 360, 15)]:
        gx_, gy_ = x + r_ * math.cos(math.radians(a_)), y + r_ * math.sin(math.radians(a_))
        if not blocked(gx_, gy_, 0) and abs(ground(gx_, gy_) - z) < 48:
            WEAP.append((round(gx_), round(gy_), round(ground(gx_, gy_)) + 8, "pistols", 1))
            added_guns += 1
            break
    else:          # (a flat spot on steep ground: the pistol right beside it)
        WEAP.append((x + 48, y, round(ground(x + 48, y)) + 8, "pistols", 1))
        added_guns += 1
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
# ---------------------------------------------------------------------------------------
# the validator (before anything is written): connectors, props, landing spots; the layer pockets are
# checked on the compiled map by navgen.py --check (it walks the real collision hulls)
HEAD = 72


def _brushes_at(x, y):
    for b in GRID.get((int(x // 256), int(y // 256)), ()):
        if getattr(b, "kind", None) == "ladder":
            continue
        if b.bounds[0] - 0.01 <= x <= b.bounds[3] + 0.01 and b.bounds[1] - 0.01 <= y <= b.bounds[4] + 0.01 and _inside(b.poly, x, y):
            yield b


def top_at(b, x, y):
    (ax, ay, az), (bx_, by, bz), (cx_, cy, cz) = b.top3
    n = cross((bx_ - ax, by - ay, bz - az), (cx_ - ax, cy - ay, cz - az))
    return az - (n[0] * (x - ax) + n[1] * (y - ay)) / n[2] if abs(n[2]) > 1e-9 else b.bounds[5]


def floors_at(x, y):
    """every height a player could stand on at x, y: the ground (or the tunnel floor), and the tops of brushes"""
    out = []
    g = surface(x, y)
    if g is not None:
        out.append(g)
    if slab_open(x, y):
        out.append(slab_floor(x, y))
    for b in _brushes_at(x, y):
        out.append(top_at(b, x, y))
    return out


def solid_between(x, y, z0, z1):
    """is anything solid at x, y between heights z0 and z1 (a brush, the ground, the underground slab)?"""
    g = surface(x, y)
    if g is not None and z0 < g - 18 and z1 > SLAB_TOP:      # (a hull corner on a slope may be up to a step in it)
        return "ground"
    if z0 < SLAB_TOP - 1 and z1 > FLOOR_Z and not slab_open(x, y):
        return "rock"
    if slab_open(x, y) and z0 < slab_floor(x, y) - 1:
        return "rock"
    if z0 < SLAB_TOP - 1 and z1 > SLAB_TOP + 1:
        return "ceiling"
    for b in _brushes_at(x, y):
        if getattr(b, "kind", None) == "foliage" or getattr(b, "noclip", False):
            continue
        if b.bounds[2] < z1 - 1 and top_at(b, x, y) > z0 + 1:
            return b.group
    return None


def stand(x, y, z, tol=18):
    """(floor, problem): the floor a player at about feet height z stands on at x, y, and whether there's room"""
    fl = [f for f in floors_at(x, y) if z - tol <= f <= z + tol]
    if not fl:
        return None, "nothing to stand on"
    f = max(fl)
    for dx, dy in ((0, 0), (-15, -15), (15, -15), (-15, 15), (15, 15)):
        w = solid_between(x + dx, y + dy, f + 1, f + HEAD)
        if w:
            return f, "no room to stand (%s)" % w
    return f, None


def check_connectors():
    """every stair, ladder, ramp and walkway: both ends meet a floor with room to stand, and the floor goes on
    past the end (no stairs into a wall, into the air, or to nowhere)"""
    bad = []
    for c in CONN:
        for end, dkey in (("bottom", "bdir"), ("top", "tdir")):
            x, y, z = c[end]
            f, why = stand(x, y, z)
            if why:
                bad.append((c["kind"], c["name"], end, (round(x), round(y), round(z)), why))
                continue
            dx, dy = c[dkey]
            # the floor goes on: straight on, or to the left or right (a landing you turn on)
            tries = [stand(x + ax * 32, y + ay * 32, f) for (ax, ay) in ((dx, dy), (-dy, dx), (dy, -dx))]
            if all(t[1] for t in tries):
                bad.append((c["kind"], c["name"], end, (round(x), round(y), round(z)), "goes nowhere: past the end, " + tries[0][1]))
    return bad


def check_props():
    """props (crates, cars, containers, hay, logs, sandbags, fences, carts, rocks, trees, bushes) sit on
    something: not floating (most of the footprint over air), not buried (the ground up over most of a box,
    or a rock or bush that barely shows)"""
    bad = []
    for b in REG:
        if not getattr(b, "prop", False):
            continue
        x0, y0, z0, x1, y1, z1 = b.bounds
        nx, ny = max(2, int((x1 - x0) // 16) + 1), max(2, int((y1 - y0) // 16) + 1)
        gaps, sunk, shows = [], [], []
        for ix in range(nx):
            for iy in range(ny):
                x = x0 + 2 + (x1 - x0 - 4) * ix / (nx - 1)
                y = y0 + 2 + (y1 - y0 - 4) * iy / (ny - 1)
                if not _inside(b.poly, x, y):
                    continue
                tb = top_at(b, x, y)
                g = surface(x, y)
                under = [v for v in (g, slab_floor(x, y)) if v is not None and v < tb] + [-99999]
                for o in _brushes_at(x, y):
                    if o is not b and getattr(o, "kind", None) != "foliage" and top_at(o, x, y) < tb:
                        under.append(top_at(o, x, y))
                sup = max(under)
                gaps.append(z0 - sup)
                g = (slab_floor(x, y) or TUN_Z) if (g is None or (z0 < SLAB_TOP and slab_open(x, y))) else g
                sunk.append(g - z0)
                shows.append(tb - g)
        if not gaps:
            continue
        gaps.sort()
        natural = b.group in ("rock", "tree", "bush", "boulder")
        if gaps[len(gaps) // 2] > 50000:
            bad.append((b.group, [round(v) for v in b.bounds], "buried: the ground is over its top"))
        elif gaps[len(gaps) // 2] > 6:
            bad.append((b.group, [round(v) for v in b.bounds], "floating: %d over what's under it" % gaps[len(gaps) // 2]))
        elif natural and max(shows) < 24:
            bad.append((b.group, [round(v) for v in b.bounds], "buried: shows only %d" % max(shows)))
        elif not natural and sum(1 for v in sunk if v > max(16, 0.3 * (z1 - z0))) > 0.3 * len(sunk):     # (a post's footing is fine)
            bad.append((b.group, [round(v) for v in b.bounds], "buried: the ground is %d up its side" % max(sunk)))
    return bad


def landing_top(x, y):
    """where something falling straight down from the sky at x, y lands"""
    return max(floors_at(x, y) + [b.bounds[5] for b in _brushes_at(x, y) if getattr(b, "kind", None) == "foliage"])


def check_landing():
    """drop-in spots (respawn points, br_drop markers) and supply drops land where they're meant to: on the
    ground at their own height, not on a roof, a tower or a tree over them"""
    bad = []
    for e in ents:
        k = e.keys["classname"]
        if k not in ("ls_respawn_point", "ls_marker", "ls_supply_drop"):
            continue
        x, y, z = (float(v) for v in e.keys["origin"].split())
        t = max(landing_top(x + dx, y + dy) for dx in (-16, 16) for dy in (-16, 16))
        if t > z + 8:
            bad.append((k, round(x), round(y), round(z), round(t)))
    return bad


def layer_of(x, y, z):
    """under (the tunnels), high (more than 100 over the ground or, on a mesa, its foot) or the surface"""
    g = surface(x, y)
    if z < SLAB_TOP or (g is not None and z < g - 100):
        return "under"
    k = KIND.get((ci(x), ci(y))) or ""
    if k.startswith("mesa:"):
        i, j = ci(x), ci(y)
        g = sum(H[i + a][j + b] for a in (0, 1) for b in (0, 1)) / 4.0
    if g is not None and z > g + 100:
        return "high"
    return "surface"


def check_guns():
    """every drop-in spot (br_drop markers, respawn points) has a gun on the ground within about 10 s"""
    bad = []
    for e in ents:
        if e.keys["classname"] in ("ls_marker", "ls_respawn_point"):
            x, y, z = (float(v) for v in e.keys["origin"].split())
            if not gun_near(x, y, z - 30 if e.keys["classname"] == "ls_respawn_point" else z - 2, GUN_RUN * 0.6):
                bad.append((e.keys["classname"], round(x), round(y)))
    return bad


GB = check_guns()
for row in GB:
    print("GUN %-18s at %d %d: no gun within a 10 s run" % row)
print("guns: %d pistols added by drop-in spots; %d spots without a gun near" % (added_guns, len(GB)))
CB = check_connectors()
for kind, name, end, pt, why in CB:
    print("CONNECTOR %-8s %-28s %-6s at %s: %s" % (kind, name, end, pt, why))
PB = check_props()
for grp, bd, why in PB[:80]:
    print("PROP %-22s %s: %s" % (grp, bd, why))
LAND = check_landing()
for row in LAND:
    print("LANDING %-18s at %d %d %d lands on %d" % row)
LAYER_LOOT = {}
for e in ents:
    if e.keys["classname"] in ("ls_loot_container", "ls_weapon_spawner"):
        x, y, z = (float(v) for v in e.keys["origin"].split())
        L_ = LAYER_LOOT.setdefault(layer_of(x, y, z - 8), {"crates": [], "weapons": []})
        L_["crates" if e.keys["classname"] == "ls_loot_container" else "weapons"].append(float(e.keys.get("tier", 1)))
for name in ("under", "surface", "high"):
    L_ = LAYER_LOOT.get(name, {"crates": [], "weapons": []})
    c, w = L_["crates"], L_["weapons"]
    print("LOOT %-8s %2d crates (tier avg %.1f, %d of tier 4-5), %2d weapons (tier avg %.1f)"
          % (name, len(c), sum(c) / max(1, len(c)), sum(1 for t in c if t >= 4), len(w), sum(w) / max(1, len(w))))
if (CB or PB or LAND or GB) and not os.environ.get("ALLOW_OVERLAP"):
    print("%d connectors, %d props, %d landing spots, %d drop-in spots without a gun to fix (ALLOW_OVERLAP=1 writes the map anyway)"
          % (len(CB), len(PB), len(LAND), len(GB)))
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
# the layer data for navgen.py --check (the checks on the compiled map): the connectors, the ground the
# layers are measured from (every 64; on a mesa its foot, in a trench its rim), the tree canopies
import json
GSTEP = 64
gnd = []
for gy in range(-PLAY, PLAY + 1, GSTEP):
    row = []
    for gx in range(-PLAY, PLAY + 1, GSTEP):
        x_, y_ = min(gx, PLAY - 1), min(gy, PLAY - 1)
        i_, j_ = ci(x_), ci(y_)
        k_ = KIND.get((i_, j_)) or ""
        if k_.startswith("mesa:") or k_ == "cut":
            row.append(round(sum(H[i_ + a_][j_ + b_] for a_ in (0, 1) for b_ in (0, 1)) / 4.0))
        else:
            row.append(round(surface(x_, y_)))
    gnd.append(row)
canopies = [[round(v) for v in b_.bounds] for b_ in REG if getattr(b_, "kind", None) in ("foliage", "treeclip")]
# solid things that block sight at eye height (for line-of-sight counts; not used by navgen)
sight = [[round(v) for v in b_.bounds] for b_ in REG if b_.group.startswith(("bld:", "rock", "boulder", "poi:", "high:", "hatch:", "trench:"))
         and not getattr(b_, "noclip", False) and b_.bounds[5] - b_.bounds[2] > 48 and b_.bounds[5] > -200]
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), NAME + ".layers.json"), "w") as f:
    json.dump({"grid": {"x0": -PLAY, "y0": -PLAY, "step": GSTEP, "ground": gnd}, "slab_top": SLAB_TOP,
               "connectors": CONN, "canopies": canopies, "sight": sight,
               "supply_drops": [[x_, y_, round(ground(x_, y_))] for (x_, y_) in DROPS]}, f, separators=(",", ":"))
nb = sum(len(e.brushes) for e in ents)
print("%s.map: %d world brushes, %d detail brushes, %d brushes in all, %d entities, %d trees, %d rocks, %d bushes, %d foxholes/berms, %d loot, %d weapons"
      % (NAME, len(world.brushes), len(detail.brushes), nb, len(ents), trees, rocks, bushes, dug, len(LOOT), len(WEAP)))

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
