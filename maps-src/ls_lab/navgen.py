#!/usr/bin/env python3
"""navgen.py: a bot nav (CS nav v5) for a big map without waiting hours for the game's learner.

    python3 navgen.py <map.bsp> <out.nav> [seed x,y,z ...]

The game's learner (ReGameDLL's LearnStep) samples a 25-unit grid with ~10 long point traces per
sample and looks every new sample up in a linked list, so on a 7168-square valley it runs for
hours. This does the same walk with the player's own collision hull (hull 1 from the .bsp, plus
solid brush entities): from every spawn, respawn point, loot crate and floor weapon it floods the
25-unit grid; a step is walkable when the standing player can walk there (in four moves, each with step-up 18, drop up to
200 one way, slope normal z >= 0.7). Samples become rectangles (all linked both ways, on one
plane within 8 units), rectangles become nav areas, linked where their samples are.

No hiding spots, approach areas or encounter spots: load the map with this nav and run
`bot_nav_analyze` in the server console (the game adds them and saves the nav). Ladders aren't
stored in a v5 nav; the game builds them from func_ladder at load. The walk goes up and down
ladders (func_ladder) too, so platforms only a ladder reaches are in the nav.

    python3 navgen.py <map.bsp> <out.nav> --check <map>.layers.json

also runs the map's checks on the real collision hulls (the second half of build_map.py's validator):
every place a drop-in can land on is walked from too; a place you can't walk (or fall at most 200,
<= 14 hp) back from to the respawn points is a trap; every high or underground pocket (connected
samples more than 100 over / under the ground) needs at least two ways out to somewhere else;
every ladder joins at the bottom and the top; every connector's ends are on the nav; every
supply drop has ground beside it (bots open crates from within 80). Exit code 1 if any fails.
"""
import json
import math
import re
import struct
import sys
import time

STEP = 25.0
STEPUP = 18.0
SUB = 4              # a step between samples is walked in this many moves, each with its own step-up (stairs)
DROP = 200.0
HALF = 36.0          # hull 1: origin 36 above the feet
NORTH, EAST, SOUTH, WEST = range(4)
DX = {NORTH: (0, -1), EAST: (1, 0), SOUTH: (0, 1), WEST: (-1, 0)}
OPP = {NORTH: SOUTH, SOUTH: NORTH, EAST: WEST, WEST: EAST}


class Hull:
    """hull 1 traces (as PM_PlayerTrace on the world and the solid brush entities)"""

    def __init__(self, path):
        d = open(path, "rb").read()
        self.size = len(d)
        lump = lambda i: struct.unpack_from("<ii", d, 4 + i * 8)
        o, l = lump(1)
        self.planes = [struct.unpack_from("<4fi", d, o + i * 20) for i in range(l // 20)]
        o, l = lump(9)
        self.clip = [struct.unpack_from("<ihh", d, o + i * 8) for i in range(l // 8)]
        mo, ml = lump(14)
        self.models = [(struct.unpack_from("<6f", d, mo + i * 64), struct.unpack_from("<4i", d, mo + i * 64 + 36)) for i in range(ml // 64)]
        self.head = self.models[0][1][1]
        o, l = lump(0)
        self.ents = [dict(re.findall(r'"([^"]+)" "([^"]*)"', e)) for e in re.findall(r"\{[^{}]*\}", d[o:o + l].decode("latin-1"))]
        self.solids = []
        for kv in self.ents:
            if kv.get("classname") in ("func_wall", "func_breakable", "func_door", "func_door_rotating") and kv.get("model", "").startswith("*"):
                b, heads = self.models[int(kv["model"][1:])]
                self.solids.append((heads[1], (b[0] - 16, b[1] - 16, b[2] - 36, b[3] + 16, b[4] + 16, b[5] + 36)))

    def contents(self, num, p):
        while num >= 0:
            n = self.clip[num]
            pl = self.planes[n[0]]
            num = n[1] if pl[0] * p[0] + pl[1] * p[1] + pl[2] * p[2] - pl[3] >= 0 else n[2]
        return num

    def _one(self, head, s, e):
        t = {"frac": 1.0, "end": e, "normal": (0.0, 0.0, 0.0), "allsolid": True, "startsolid": False}
        self._head = head
        self._rec(head, 0.0, 1.0, s, e, t)
        if t["allsolid"]:
            t["startsolid"] = True
        return t

    def trace(self, s, e):
        best = self._one(self.head, s, e)
        lo = [min(s[i], e[i]) for i in range(3)]
        hi = [max(s[i], e[i]) for i in range(3)]
        for head, bx in self.solids:
            if hi[0] < bx[0] or lo[0] > bx[3] or hi[1] < bx[1] or lo[1] > bx[4] or hi[2] < bx[2] or lo[2] > bx[5]:
                continue
            t = self._one(head, s, e)
            if t["startsolid"]:
                return t
            if t["frac"] < best["frac"]:
                best = t
        return best

    def _rec(self, num, p1f, p2f, p1, p2, t):
        if num < 0:
            if num != -2:
                t["allsolid"] = False
            else:
                t["startsolid"] = True
            return True
        n = self.clip[num]
        pl = self.planes[n[0]]
        if pl[4] < 3:
            t1 = p1[pl[4]] - pl[3]
            t2 = p2[pl[4]] - pl[3]
        else:
            t1 = pl[0] * p1[0] + pl[1] * p1[1] + pl[2] * p1[2] - pl[3]
            t2 = pl[0] * p2[0] + pl[1] * p2[1] + pl[2] * p2[2] - pl[3]
        if t1 >= 0 and t2 >= 0:
            return self._rec(n[1], p1f, p2f, p1, p2, t)
        if t1 < 0 and t2 < 0:
            return self._rec(n[2], p1f, p2f, p1, p2, t)
        frac = (t1 + 0.03125) / (t1 - t2) if t1 < 0 else (t1 - 0.03125) / (t1 - t2)
        frac = min(max(frac, 0.0), 1.0)
        midf = p1f + (p2f - p1f) * frac
        mid = (p1[0] + frac * (p2[0] - p1[0]), p1[1] + frac * (p2[1] - p1[1]), p1[2] + frac * (p2[2] - p1[2]))
        side = 0 if t1 >= 0 else 1
        if not self._rec(n[1 + side], p1f, midf, p1, mid, t):
            return False
        if self.contents(n[1 + (side ^ 1)], mid) != -2:
            return self._rec(n[1 + (side ^ 1)], midf, p2f, mid, p2, t)
        if t["allsolid"]:
            return False
        t["normal"] = (pl[0], pl[1], pl[2]) if side == 0 else (-pl[0], -pl[1], -pl[2])
        while self.contents(self._head, mid) == -2:
            frac -= 0.1
            if frac < 0:
                t["frac"] = midf
                t["end"] = mid
                return False
            midf = p1f + (p2f - p1f) * frac
            mid = (p1[0] + frac * (p2[0] - p1[0]), p1[1] + frac * (p2[1] - p1[1]), p1[2] + frac * (p2[2] - p1[2]))
        t["frac"] = midf
        t["end"] = mid
        return False


class Node:
    __slots__ = ("i", "j", "z", "nz", "to", "area")

    def __init__(self, i, j, z, nz):
        self.i, self.j, self.z, self.nz = i, j, z, nz
        self.to = [None] * 4
        self.area = None


def main():
    args = sys.argv[1:]
    layers = None
    if "--check" in args:
        k = args.index("--check")
        layers = json.load(open(args[k + 1]))
        del args[k:k + 2]
    bsp, out = args[0], args[1]
    h = Hull(bsp)
    t0 = time.time()
    grid = {}          # (i, j) -> [Node]

    def find(i, j, z):
        for n in grid.get((i, j), ()):
            if abs(n.z - z) < 12:
                return n
        return None

    def ground(x, y, ztop, depth):
        """feet height under (x, y) within depth below ztop (feet), and the floor's normal z"""
        t = h.trace((x, y, ztop + HALF), (x, y, ztop + HALF - depth))
        if t["startsolid"] or t["frac"] >= 1.0:
            return None
        return t["end"][2] - HALF, t["normal"][2]

    def add(i, j, z, nz):
        n = find(i, j, z)
        if n:
            return n, False
        n = Node(i, j, z, nz)
        grid.setdefault((i, j), []).append(n)
        return n, True

    seeds = []
    for kv in h.ents:
        if kv.get("classname") in ("info_player_start", "info_player_deathmatch", "ls_respawn_point", "ls_loot_container",
                                   "ls_weapon_spawner", "ls_supply_drop") and "origin" in kv:
            seeds.append(tuple(float(v) for v in kv["origin"].split()) + (kv["classname"],))
    for a in args[2:]:
        seeds.append(tuple(float(v) for v in a.split(",")) + ("arg",))
    stack = []
    seeded = {}        # node -> what seeded it (respawn points are "home")

    def seed(x, y, z, why):
        i, j = int(round(x / STEP)), int(round(y / STEP))
        g = None
        for lift in (40, 16, 2):          # under a low roof the higher starts are inside it
            g = ground(i * STEP, j * STEP, z + lift, 200)
            if g:
                break
        if g and g[1] >= 0.7:
            n, new = add(i, j, round(g[0], 1), g[1])
            seeded.setdefault(n, why)
            if new:
                stack.append(n)
            return n
        return None
    for (x, y, z, why) in seeds:
        seed(x, y, z, why)
    traces = [0]

    # where a drop-in lands: straight down from the sky every 64 (the tops of tree canopies aren't
    # seeded into the nav: they're only reported)
    landings, canopy_landings = [], []
    if layers:
        canopies = layers["canopies"]
        for gy in range(-3584 + 32, 3584, 64):
            for gx in range(-3584 + 32, 3584, 64):
                t = h.trace((gx, gy, 3500.0), (gx, gy, -700.0))
                traces[0] += 1
                if t["startsolid"] or t["frac"] >= 1.0 or t["normal"][2] < 0.7:
                    continue
                z = t["end"][2] - HALF
                if any(c[0] - 16 <= gx <= c[3] + 16 and c[1] - 16 <= gy <= c[4] + 16 and abs(z - c[5]) < 4 for c in canopies):
                    canopy_landings.append((gx, gy, z))
                    continue
                n = seed(gx, gy, z + 2, "landing")
                if n:
                    landings.append(n)

    ledge = [False]

    def walk(x, y, z, nx, ny):
        """the player's own moves (PM_StepMove-like): up to STEPUP, across, down to the floor
        (ledge[0]: the way was open but the floor further down than DROP: an unguarded ledge)"""
        cz, nz = z, 1.0
        px, py = x, y
        ledge[0] = False
        for k in range(1, SUB + 1):
            tx, ty = x + (nx - x) * k / SUB, y + (ny - y) * k / SUB
            up = h.trace((px, py, cz + HALF + 1), (px, py, cz + HALF + 1 + STEPUP))
            hz = cz + 1 + STEPUP * up["frac"]
            t = h.trace((px, py, hz + HALF), (tx, ty, hz + HALF))
            traces[0] += 3
            if t["frac"] < 1.0 or t["startsolid"]:
                return None
            g = ground(tx, ty, hz, hz - cz + DROP)
            if not g or z - g[0] > DROP:
                ledge[0] = True
                return None
            if g[1] < 0.7:
                return None
            cz, nz = g
            px, py = tx, ty
        return cz, nz

    ledges = set()

    def flood():
        while stack:
            n = stack.pop()
            x, y = n.i * STEP, n.j * STEP
            for d in range(4):
                di, dj = DX[d]
                ni, nj = n.i + di, n.j + dj
                g = walk(x, y, n.z, ni * STEP, nj * STEP)
                if not g:
                    if ledge[0]:
                        ledges.add((n, d))
                    continue
                m, new = add(ni, nj, round(g[0], 1), g[1])
                n.to[d] = m
                if new:
                    stack.append(m)

    # ladders: the samples at the foot and at the top of every func_ladder are joined both ways; a ladder
    # whose top (or foot) nobody has walked to yet seeds it, and the walk goes on from there
    ladders = []
    for kv in h.ents:
        if kv.get("classname") == "func_ladder" and kv.get("model", "").startswith("*"):
            b = h.models[int(kv["model"][1:])][0]
            ladders.append({"box": b, "bottom": [], "top": []})

    def near(n, b, r):
        x, y = n.i * STEP, n.j * STEP
        dx = max(b[0] - x, 0, x - b[3])
        dy = max(b[1] - y, 0, y - b[4])
        return dx * dx + dy * dy <= r * r

    def ends(lad):
        b = lad["box"]
        bot, top = [], []
        for i in range(int((b[0] - 48) // STEP), int((b[3] + 48) // STEP) + 2):
            for j in range(int((b[1] - 48) // STEP), int((b[4] + 48) // STEP) + 2):
                for n in grid.get((i, j), ()):
                    if not near(n, b, 44):
                        continue
                    if abs(n.z - b[2]) <= 24:
                        bot.append(n)
                    elif b[5] - 52 <= n.z <= b[5] + 8:
                        top.append(n)
        return bot, top

    flood()
    for _ in range(6):
        grew = False
        for lad in ladders:
            b = lad["box"]
            bot, top = ends(lad)
            for (want, have, zz) in ((top, bot, b[5]), (bot, top, b[2] + 18)):
                if want or not have:
                    continue
                for (x, y) in [((b[0] + b[3]) / 2, b[1] - r) for r in (20, 36)] + [((b[0] + b[3]) / 2, b[4] + r) for r in (20, 36)] + \
                              [(b[0] - r, (b[1] + b[4]) / 2) for r in (20, 36)] + [(b[3] + r, (b[1] + b[4]) / 2) for r in (20, 36)]:
                    g = ground(x, y, zz + 4, 64)
                    if g and g[1] >= 0.7 and (b[5] - 52 <= g[0] <= b[5] + 8 if want is top else abs(g[0] - b[2]) <= 24):
                        n, new = add(int(round(x / STEP)), int(round(y / STEP)), round(g[0], 1), g[1])
                        if new:
                            stack.append(n)
                            grew = True
        flood()
        if not grew:
            break
    for lad in ladders:
        lad["bottom"], lad["top"] = ends(lad)
    nodes = [n for l in grid.values() for n in l]
    print("%d samples, %d traces, %.0f s" % (len(nodes), traces[0], time.time() - t0))

    # rectangles: linked both ways inside, flat within 8 units of the corners' plane
    def bi(a, d):
        b = a.to[d]
        return b if b is not None and b.to[OPP[d]] is a else None

    MAXW = int(__import__("os").environ.get("NAV_MAXW", "32"))     # (the brain builds its map knowledge for navs of up to 6000 areas)
    nodes.sort(key=lambda n: (n.j, n.i, n.z))
    areas = []
    for n in nodes:
        if n.area is not None:
            continue
        # width along +x
        row = [n]
        while len(row) < MAXW:
            b = bi(row[-1], EAST)
            if b is None or b.area is not None or abs(b.nz - n.nz) > 0.02:
                break
            row.append(b)
        w = len(row)
        rows = [row]
        while len(rows) < MAXW:
            nxt = []
            for a in rows[-1]:
                b = bi(a, SOUTH)
                if b is None or b.area is not None or abs(b.nz - n.nz) > 0.02:
                    break
                nxt.append(b)
            if len(nxt) < w:
                break
            if any(bi(nxt[k], EAST) is not nxt[k + 1] for k in range(w - 1)):
                break
            # flat enough: every sample within 8 of the plane through three corners
            c00, c10, c01 = rows[0][0], rows[0][-1], nxt[0]
            hh = len(rows)
            ok = True
            for r, rr in enumerate(rows + [nxt]):
                for k, a in enumerate(rr):
                    zz = c00.z + (c10.z - c00.z) * (k / (w - 1) if w > 1 else 0) + (c01.z - c00.z) * (r / hh)
                    if abs(a.z - zz) > 8:
                        ok = False
                        break
                if not ok:
                    break
            if not ok:
                break
            rows.append(nxt)
        aid = len(areas) + 1
        for rr in rows:
            for a in rr:
                a.area = aid
        areas.append(rows)
    print("%d areas" % len(areas))

    # write nav v5
    def corner_z(rows, k, r):
        hgt, w = len(rows), len(rows[0])
        kk, rr = min(k, w - 1), min(r, hgt - 1)
        a = rows[rr][kk]
        # extrapolate half a step over the far edge (an area spans to the next sample)
        dzx = (rows[rr][w - 1].z - rows[rr][0].z) / (w - 1) if w > 1 else 0.0
        dzy = (rows[hgt - 1][kk].z - rows[0][kk].z) / (hgt - 1) if hgt > 1 else 0.0
        return a.z + (dzx if k >= w else 0) + (dzy if r >= hgt else 0)

    buf = [struct.pack("<III", 0xFEEDFACE, 5, h.size), struct.pack("<H", 0), struct.pack("<I", len(areas))]
    for aid, rows in enumerate(areas, 1):
        w, hgt = len(rows[0]), len(rows)
        n0 = rows[0][0]
        x0, y0 = n0.i * STEP - STEP / 2, n0.j * STEP - STEP / 2
        x1, y1 = x0 + w * STEP, y0 + hgt * STEP
        z_nw = corner_z(rows, 0, 0)
        z_ne = corner_z(rows, w, 0)
        z_se = corner_z(rows, w, hgt)
        z_sw = corner_z(rows, 0, hgt)
        conn = [set() for _ in range(4)]
        for r, rr in enumerate(rows):
            for k, a in enumerate(rr):
                for d in range(4):
                    b = a.to[d]
                    if b is not None and b.area != aid:
                        conn[d].add(b.area)
        buf.append(struct.pack("<IB", aid, 0))
        buf.append(struct.pack("<6f", x0, y0, z_nw, x1, y1, z_se))
        buf.append(struct.pack("<2f", z_ne, z_sw))
        for d in range(4):
            ids = sorted(conn[d])
            buf.append(struct.pack("<I", len(ids)) + b"".join(struct.pack("<I", i) for i in ids))
        buf.append(struct.pack("<B", 0))      # hiding spots
        buf.append(struct.pack("<B", 0))      # approach areas
        buf.append(struct.pack("<I", 0))      # encounter spots
        buf.append(struct.pack("<H", 0))      # place
    open(out, "wb").write(b"".join(buf))
    print("wrote %s (%.0f s)" % (out, time.time() - t0))
    if layers:
        ok = check(h, nodes, grid, seeded, landings, canopy_landings, ladders, ledges, layers)
        sys.exit(0 if ok else 1)


def check(h, nodes, grid, seeded, landings, canopy_landings, ladders, ledges, layers):
    """the map's checks on the walk (see the top of the file); prints a report, returns True if all pass"""
    G = layers["grid"]
    gnd, gx0, gy0, gs = G["ground"], G["x0"], G["y0"], G["step"]

    def around(x, y):
        """the ground within 128 (a bank beside the river bed is neither high nor under)"""
        c0 = int(round((x - gx0) / gs))
        r0 = int(round((y - gy0) / gs))
        return [gnd[r][c] for r in range(max(0, r0 - 2), min(len(gnd), r0 + 3)) for c in range(max(0, c0 - 2), min(len(gnd[0]), c0 + 3))]

    def layer(n):
        g = around(n.i * STEP, n.j * STEP)
        if n.z < layers["slab_top"] - 40 or n.z < min(g) - 100:
            return "under"
        return "high" if n.z > max(g) + 100 else "surface"

    out = {n: [m for m in n.to if m is not None] for n in nodes}
    via = {}           # (a, b) -> the ladder joining them
    for k, lad in enumerate(ladders):
        for a in lad["bottom"]:
            for b in lad["top"]:
                out[a].append(b)
                out[b].append(a)
                via[(a, b)] = via[(b, a)] = k
    rev = {n: [] for n in nodes}
    for n, ms in out.items():
        for m in ms:
            rev[m].append(n)

    def bfs(start, adj):
        seen = set(start)
        st = list(start)
        while st:
            n = st.pop()
            for m in adj[n]:
                if m not in seen:
                    seen.add(m)
                    st.append(m)
        return seen
    home = [n for n, why in seeded.items() if why == "ls_respawn_point"]
    back = bfs(home, rev)            # can walk (or drop <= 200, or climb) back to a respawn point
    there = bfs(home, out)           # can be walked to from a respawn point
    ok = True
    print("\n== checks on the compiled map ==")
    lay = {}
    for n in nodes:
        lay[n] = layer(n)
    for name in ("under", "surface", "high"):
        k = [n for n in nodes if lay[n] == name]
        print("layer %-8s %6d samples (%5.2f M sq units), %d walkable from the respawn points"
              % (name, len(k), len(k) * STEP * STEP / 1e6, sum(1 for n in k if n in there)))

    # traps: anything you can't get back from (drop-in landings included)
    trapped = [n for n in nodes if n not in back]
    comp = {}
    for n in trapped:
        if n in comp:
            continue
        cid = len(set(comp.values()))
        st = [n]
        comp[n] = cid
        while st:
            a = st.pop()
            for m in out[a] + rev[a]:
                if m not in back and m not in comp:
                    comp[m] = cid
                    st.append(m)
    groups = {}
    for n, c in comp.items():
        groups.setdefault(c, []).append(n)
    land_set = set(landings)
    print("traps: %d places (%d samples) you can't get back from without a fall over 200" % (len(groups), len(trapped)))
    for c, ns in sorted(groups.items(), key=lambda kv: -len(kv[1]))[:40]:
        xs = [n.i * STEP for n in ns]
        ys = [n.j * STEP for n in ns]
        print("  TRAP %4d samples at %5d %5d z %4d (%s)%s" % (len(ns), sum(xs) / len(xs), sum(ys) / len(ys), max(n.z for n in ns),
              lay[ns[0]], ", a drop-in lands here" if any(n in land_set for n in ns) else ""))
    if trapped:
        ok = False
    print("drop-in landings: %d every 64 units, %d on tree canopies (not checked as places)" % (len(landings), len(canopy_landings)))

    # pockets: the connected high / under samples, and their ways out
    pid = {}
    pockets = []
    for n in nodes:
        if lay[n] == "surface" or n in pid:
            continue
        k = len(pockets)
        pid[n] = k
        st, mem = [n], [n]
        while st:
            a = st.pop()
            for m in out[a] + rev[a]:
                if m not in pid and lay[m] == lay[n]:
                    pid[m] = k
                    st.append(m)
                    mem.append(m)
        pockets.append(mem)
    print("pockets: %d high or underground places" % len(pockets))
    bad_pockets = 0
    for k, mem in enumerate(pockets):
        if len(mem) < 8:
            continue
        # ways out: steps to a sample outside the pocket that leads back. Each ladder is one way; walking
        # (or dropping) ways are told apart by the way they go (n/e/s/w) and by being 128+ apart
        lad_ways, walk_ways = set(), []
        for a in mem:
            for d, m in enumerate(a.to):
                if m is not None and pid.get(m) != k and m in back:
                    walk_ways.append((a.i * STEP, a.j * STEP, d))
            for m in out[a]:
                if (a, m) in via and pid.get(m) != k and m in back:
                    lad_ways.add(via[(a, m)])
        clusters = [[w] for w in sorted(lad_ways)]
        groups_ = []
        for w in walk_ways:
            for g_ in groups_:
                if g_[0][2] == w[2] and any(math.hypot(w[0] - f[0], w[1] - f[1]) < 128 for f in g_):
                    g_.append(w)
                    break
            else:
                groups_.append([w])
        merged = True
        while merged:
            merged = False
            for a in range(len(groups_)):
                for b in range(a + 1, len(groups_)):
                    if groups_[a][0][2] == groups_[b][0][2] and any(math.hypot(e[0] - f[0], e[1] - f[1]) < 128 for e in groups_[a] for f in groups_[b]):
                        groups_[a] += groups_.pop(b)
                        merged = True
                        break
                if merged:
                    break
        clusters += groups_
        xs = [n.i * STEP for n in mem]
        ys = [n.j * STEP for n in mem]
        led = sum(1 for (n, d) in ledges if pid.get(n) == k and n in there)
        reach = sum(1 for n in mem if n in there)
        tag = "OK" if len(clusters) >= 2 else "FEWER THAN 2 WAYS OUT"
        if len(clusters) < 2:
            bad_pockets += 1
        print("  %-5s pocket %4d samples, x %5d..%5d y %5d..%5d z %4d..%4d: %d ways out%s%s  %s"
              % (lay[mem[0]], len(mem), min(xs), max(xs), min(ys), max(ys), min(n.z for n in mem), max(n.z for n in mem),
                 len(clusters), ", %d unguarded ledge samples on foot" % led if led else "",
                 "" if reach == len(mem) else ", %d only by drop-in" % (len(mem) - reach), tag))
    if bad_pockets:
        ok = False

    # ladders joined top and bottom
    joined = sum(1 for lad in ladders if lad["bottom"] and lad["top"])
    print("ladders: %d of %d joined at the bottom and the top" % (joined, len(ladders)))
    for lad in ladders:
        if not (lad["bottom"] and lad["top"]):
            b = lad["box"]
            print("  LADDER at %d %d z %d..%d: %s" % ((b[0] + b[3]) / 2, (b[1] + b[4]) / 2, b[2], b[5],
                                                      "no top" if lad["bottom"] else "no bottom"))
            ok = False

    def found(x, y, z, r=28, dz=24):
        for i in range(int((x - r) // STEP), int((x + r) // STEP) + 2):
            for j in range(int((y - r) // STEP), int((y + r) // STEP) + 2):
                for n in grid.get((i, j), ()):
                    if math.hypot(n.i * STEP - x, n.j * STEP - y) <= r and abs(n.z - z) <= dz and n in back:
                        return n
        return None
    miss = 0
    for c in layers["connectors"]:
        for end in ("bottom", "top"):
            x, y, z = c[end]
            if not found(x, y, z):
                print("  CONNECTOR %s %s: its %s (%d %d %d) isn't on the walk" % (c["kind"], c["name"], end, x, y, z))
                miss += 1
    print("connectors: %d, %d ends not on the walk" % (len(layers["connectors"]), miss))
    if miss:
        ok = False
    # a gun within a 10-second run (2500 units of walking) of every drop-in spot
    import heapq
    dist = {}
    pq = []
    for kv in h.ents:
        if kv.get("classname") == "ls_weapon_spawner":
            x, y, z = (float(v) for v in kv["origin"].split())
            n = found(x, y, z - 8, r=40, dz=40)
            if n and dist.get(n, 1e9) > 0:
                dist[n] = 0.0
                heapq.heappush(pq, (0.0, id(n), n))
    while pq:
        d, _, n = heapq.heappop(pq)
        if d > dist.get(n, 1e9):
            continue
        for m in rev[n]:
            c = d + (abs(m.z - n.z) * 1.5 + STEP if (m, n) in via else math.hypot(STEP, m.z - n.z))
            if c < dist.get(m, 1e9):
                dist[m] = c
                heapq.heappush(pq, (c, id(m), m))
    runs, far = [], 0
    for kv in h.ents:
        if kv.get("classname") == "ls_marker" and kv.get("kind") == "br_drop":
            x, y, z = (float(v) for v in kv["origin"].split())
            n = found(x, y, z - 8, r=40, dz=40)
            d = dist.get(n, 1e9) if n else 1e9
            runs.append(d)
            if d > 2500:
                far += 1
                print("  GUN: the drop-in spot at %d %d has no gun within a 10 s run (%s)" % (x, y, "%d units" % d if d < 1e8 else "none"))
    if runs:
        runs.sort()
        print("guns: drop-in spot to the nearest gun, walking: median %d, worst %d units (%.1f s at 250/s); %d over 10 s"
              % (runs[len(runs) // 2], runs[-1], runs[-1] / 250.0, far))
    if far:
        ok = False
    sd = 0
    for (x, y, z) in layers["supply_drops"]:
        if not found(x, y, z, r=80, dz=48):
            print("  SUPPLY DROP at %d %d %d: no ground beside it to open it from" % (x, y, z))
            sd += 1
    print("supply drops: %d, %d without ground beside them" % (len(layers["supply_drops"]), sd))
    if sd:
        ok = False
    print("ALL CHECKS PASS" if ok else "SOME CHECKS FAIL")
    return ok


if __name__ == "__main__":
    main()
