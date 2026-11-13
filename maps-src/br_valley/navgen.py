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
stored in a v5 nav; the game builds them from func_ladder at load.
"""
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
    bsp, out = sys.argv[1], sys.argv[2]
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
            seeds.append(tuple(float(v) for v in kv["origin"].split()))
    for s in sys.argv[3:]:
        seeds.append(tuple(float(v) for v in s.split(",")))
    stack = []
    for (x, y, z) in seeds:
        i, j = int(round(x / STEP)), int(round(y / STEP))
        for lift in (40, 16, 2):          # under a low roof the higher starts are inside it
            g = ground(i * STEP, j * STEP, z + lift, 200)
            if g:
                break
        if g and g[1] >= 0.7:
            n, new = add(i, j, round(g[0], 1), g[1])
            if new:
                stack.append(n)
    traces = [0]

    def walk(x, y, z, nx, ny):
        """the player's own moves (PM_StepMove-like): up to STEPUP, across, down to the floor"""
        cz, nz = z, 1.0
        px, py = x, y
        for k in range(1, SUB + 1):
            tx, ty = x + (nx - x) * k / SUB, y + (ny - y) * k / SUB
            up = h.trace((px, py, cz + HALF + 1), (px, py, cz + HALF + 1 + STEPUP))
            hz = cz + 1 + STEPUP * up["frac"]
            t = h.trace((px, py, hz + HALF), (tx, ty, hz + HALF))
            traces[0] += 3
            if t["frac"] < 1.0 or t["startsolid"]:
                return None
            g = ground(tx, ty, hz, hz - cz + DROP)
            if not g or g[1] < 0.7 or z - g[0] > DROP:
                return None
            cz, nz = g
            px, py = tx, ty
        return cz, nz

    while stack:
        n = stack.pop()
        x, y = n.i * STEP, n.j * STEP
        for d in range(4):
            di, dj = DX[d]
            ni, nj = n.i + di, n.j + dj
            g = walk(x, y, n.z, ni * STEP, nj * STEP)
            if not g:
                continue
            m, new = add(ni, nj, round(g[0], 1), g[1])
            n.to[d] = m
            if new:
                stack.append(m)
    nodes = [n for l in grid.values() for n in l]
    print("%d samples, %d traces, %.0f s" % (len(nodes), traces[0], time.time() - t0))

    # rectangles: linked both ways inside, flat within 8 units of the corners' plane
    def bi(a, d):
        b = a.to[d]
        return b if b is not None and b.to[OPP[d]] is a else None

    MAXW = 16
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


if __name__ == "__main__":
    main()
