#!/usr/bin/env python3
"""mapcheck.py: the Last Stand map checks, on the compiled map and its nav (learned or navgen.py's).

    python3 mapcheck.py <map.bsp> <map.nav> [--floor Z ...] [--max-areas N]

Works on any Last Stand map (ls_outpost, ls_lab). Reads the entities from the .bsp, the player's
collision hull (hull 1) through navgen.Hull, and the nav's areas and links (v5); func_ladders join
the areas at their foot and their top (players and bots climb; creatures don't). It checks:

  spawns     every info_player_start / info_player_deathmatch has room to stand (the standing hull
             isn't in anything), a floor under it within 24 units, no other spawn's box overlapping
             it, and a nav area under it
  reach      every nav area can be walked to from the CT spawns, and back (no traps: a place you
             can drop into but not leave is an area that can't get back)
  pockets    raised places (areas 64+ over the floor levels given with --floor, default 0) grouped
             into connected pockets; each one of 8+ areas needs 2 or more ways out (each ladder is
             one; walking ways count apart when they go different ways or are 160+ apart):
             nothing up high goes nowhere
  items      weapon spawners, loot crates, supply drops (ground within 80), the armory (lst_shop)
             and every ls_marker are on the walk from the CT spawns (the armory within 96: its
             shopkeeper stands behind a counter you buy across)
  creatures  from every wave spawner a creature (walking, no ladders) reaches a barricade; with every
             barricade standing it can't reach a CT spawn or the base (lst_base), so no way in skips
             the gates; and each barricade is reached by at least one spawner
  ladders    joined at the foot and at the top
  areas      the nav has at most --max-areas areas (6000: the brain builds its map knowledge only
             for navs up to that)
Exit code 1 if anything fails.
"""
import math
import os
import re
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from navgen import Hull, HALF  # noqa: E402


def read_nav(path):
    nav = open(path, "rb").read()
    p = 0

    def u(f):
        nonlocal p
        v = struct.unpack_from("<" + f, nav, p)
        p += struct.calcsize("<" + f)
        return v
    magic, ver = u("II")
    assert magic == 0xFEEDFACE and ver == 5, "nav v5 only (%x %d)" % (magic, ver)
    u("I")
    (cnt,) = u("H")
    for _ in range(cnt):
        (ln,) = u("H")
        p += ln
    (na,) = u("I")
    areas = {}
    for _ in range(na):
        (aid,) = u("I")
        u("B")
        x1, y1, z1, x2, y2, z2 = u("6f")
        zne, zsw = u("2f")
        links = []
        for d in range(4):
            (c,) = u("I")
            links += list(u("%dI" % c)) if c else []
        (hc,) = u("B")
        for _ in range(hc):
            u("I")
            u("3f")
            u("B")
        (ap,) = u("B")
        p += ap * 14
        (se,) = u("I")
        for _ in range(se):
            u("I")
            u("B")
            u("I")
            u("B")
            (sc,) = u("B")
            p += sc * 5
        u("H")
        areas[aid] = dict(id=aid, x0=min(x1, x2), y0=min(y1, y2), x1=max(x1, x2), y1=max(y1, y2),
                          z=(z1 + z2 + zne + zsw) / 4.0, zlo=min(z1, z2, zne, zsw), zhi=max(z1, z2, zne, zsw), out=links)
    return areas


def area_at(areas, x, y, z, r=0.0, dz=40.0):
    """the area under (x, y) (feet z), or the nearest within r"""
    best, bd = None, 1e9
    for a in areas.values():
        dx = max(a["x0"] - x, 0, x - a["x1"])
        dy = max(a["y0"] - y, 0, y - a["y1"])
        d = math.hypot(dx, dy)
        if d > r or not (a["zlo"] - dz <= z <= a["zhi"] + dz):
            continue
        d += abs(a["z"] - z) * 0.01
        if d < bd:
            best, bd = a, d
    return best


def bfs(starts, adj):
    seen = set(starts)
    st = list(starts)
    while st:
        n = st.pop()
        for m in adj.get(n, ()):
            if m not in seen:
                seen.add(m)
                st.append(m)
    return seen


def main():
    args = sys.argv[1:]
    floors = []
    max_areas = 6000
    while "--floor" in args:
        k = args.index("--floor")
        floors.append(float(args[k + 1]))
        del args[k:k + 2]
    if "--max-areas" in args:
        k = args.index("--max-areas")
        max_areas = int(args[k + 1])
        del args[k:k + 2]
    floors = floors or [0.0]
    bsp, navp = args[0], args[1]
    h = Hull(bsp)
    areas = read_nav(navp)
    ok = True
    print("== %s: %d nav areas (limit %d) ==" % (os.path.basename(bsp), len(areas), max_areas))
    if len(areas) > max_areas:
        print("  AREAS: over the limit")
        ok = False

    def origin(kv):
        return tuple(float(v) for v in kv["origin"].split())

    def bounds(kv):
        b, _ = h.models[int(kv["model"][1:])]
        return b

    ents = h.ents
    # ladders: areas at the foot and the top
    ladders = []
    for kv in ents:
        if kv.get("classname") == "func_ladder" and kv.get("model", "").startswith("*"):
            b = bounds(kv)
            bot, top = [], []
            for a in areas.values():
                dx = max(a["x0"] - b[3], 0, b[0] - a["x1"])
                dy = max(a["y0"] - b[4], 0, b[1] - a["y1"])
                if math.hypot(dx, dy) > 44:
                    continue
                if b[2] - 72 <= a["z"] <= b[2] + 24:
                    bot.append(a["id"])
                elif b[5] - 56 <= a["z"] <= b[5] + 8:
                    top.append(a["id"])
            ladders.append((b, bot, top))
    walk = {k: list(a["out"]) for k, a in areas.items()}          # creatures: walking only
    adj = {k: list(v) for k, v in walk.items()}                    # players: walking + ladders
    lad_edge = {}
    for li, (b, bot, top) in enumerate(ladders):
        for x in bot:
            for y in top:
                adj[x].append(y)
                adj[y].append(x)
                lad_edge[(x, y)] = lad_edge[(y, x)] = li
    rev = {k: [] for k in areas}
    for k, ms in adj.items():
        for m in ms:
            if m in rev:
                rev[m].append(k)

    # spawns
    spawns = [(kv["classname"], origin(kv)) for kv in ents if kv.get("classname") in ("info_player_start", "info_player_deathmatch")]
    ct = [o for c, o in spawns if c == "info_player_start"]
    print("spawns: %d CT, %d T" % (len(ct), len(spawns) - len(ct)))
    bad = 0
    home = set()
    for i, (c, o) in enumerate(spawns):
        why = []
        t = h.trace(o, o)
        if t["startsolid"]:
            why.append("stuck in something")
        else:
            d = h.trace(o, (o[0], o[1], o[2] - 64))
            if d["frac"] >= 1.0 or o[2] - d["end"][2] > 24:
                why.append("no floor within 24 under it")
        for c2, o2 in spawns[i + 1:]:
            if abs(o[0] - o2[0]) < 33 and abs(o[1] - o2[1]) < 33 and abs(o[2] - o2[2]) < 73:
                why.append("overlaps the %s at %d %d" % (c2, o2[0], o2[1]))
        feet = o[2] - HALF
        a = area_at(areas, o[0], o[1], feet, r=20, dz=24)
        if not a:
            why.append("no nav area under it")
        elif c == "info_player_start":
            home.add(a["id"])
        if why:
            bad += 1
            print("  SPAWN %s at %d %d %d: %s" % (c, o[0], o[1], o[2], "; ".join(why)))
    print("spawns: %d with a problem" % bad)
    ok &= bad == 0

    there = bfs(home, adj)
    back = bfs(home, rev)
    lost = [k for k in areas if k not in there]
    traps = [k for k in areas if k in there and k not in back]
    print("reach: %d of %d areas walkable from the CT spawns, %d can't get back (traps)" % (len(areas) - len(lost), len(areas), len(traps)))
    for k in (lost + traps)[:20]:
        a = areas[k]
        print("  %s area %d at %d %d z %d" % ("UNREACHED" if k in lost else "TRAP", k, (a["x0"] + a["x1"]) / 2, (a["y0"] + a["y1"]) / 2, a["z"]))
    ok &= not lost and not traps

    # pockets: raised areas, connected, and their ways out
    def raised(a):
        return min(abs(a["z"] - f) for f in floors) >= 64
    pid, pockets = {}, []
    for k, a in areas.items():
        if k in pid or not raised(a):
            continue
        n = len(pockets)
        pid[k] = n
        st, mem = [k], [k]
        while st:
            x = st.pop()
            for m in walk[x] + [r for r in rev[x] if (r, x) not in lad_edge]:
                if m not in pid and raised(areas[m]) and abs(areas[m]["z"] - areas[x]["z"]) < 64:
                    pid[m] = n
                    st.append(m)
                    mem.append(m)
        pockets.append(mem)
    badp = 0
    big = [m for m in pockets if len(m) >= 8 or sum((areas[k]["x1"] - areas[k]["x0"]) * (areas[k]["y1"] - areas[k]["y0"]) for k in m) >= 20000]
    print("pockets: %d raised places (%d of size)" % (len(pockets), len(big)))
    for n, mem in enumerate(pockets):
        if mem not in big:
            continue
        ways_l, ways_w = set(), []
        for k in mem:
            a = areas[k]
            for m in adj[k]:
                if pid.get(m) == n or m not in back:
                    continue
                if (k, m) in lad_edge:
                    ways_l.add(lad_edge[(k, m)])
                else:
                    b = areas[m]
                    ways_w.append(((b["x0"] + b["x1"]) / 2, (b["y0"] + b["y1"]) / 2))
        groups = []
        for w in ways_w:
            for g in groups:
                if any(math.hypot(w[0] - f[0], w[1] - f[1]) < 160 for f in g):
                    g.append(w)
                    break
            else:
                groups.append([w])
        merged = True
        while merged:
            merged = False
            for i in range(len(groups)):
                for j in range(i + 1, len(groups)):
                    if any(math.hypot(e[0] - f[0], e[1] - f[1]) < 160 for e in groups[i] for f in groups[j]):
                        groups[i] += groups.pop(j)
                        merged = True
                        break
                if merged:
                    break
        nways = len(ways_l) + len(groups)
        xs = [areas[k]["x0"] for k in mem] + [areas[k]["x1"] for k in mem]
        ys = [areas[k]["y0"] for k in mem] + [areas[k]["y1"] for k in mem]
        zs = [areas[k]["z"] for k in mem]
        tag = "OK" if nways >= 2 else "FEWER THAN 2 WAYS OUT"
        if nways < 2:
            badp += 1
        print("  pocket %3d areas x %5d..%5d y %5d..%5d z %4d..%4d: %d ways out (%d ladders)  %s"
              % (len(mem), min(xs), max(xs), min(ys), max(ys), min(zs), max(zs), nways, len(ways_l), tag))
    ok &= badp == 0

    # ladders
    lj = sum(1 for b, bot, top in ladders if bot and top)
    print("ladders: %d of %d joined at the foot and the top" % (lj, len(ladders)))
    for b, bot, top in ladders:
        if not (bot and top):
            print("  LADDER at %d %d z %d..%d: %s" % ((b[0] + b[3]) / 2, (b[1] + b[4]) / 2, b[2], b[5], "no top" if bot else "no foot"))
            ok = False

    # items and markers on the walk
    miss = 0
    n_items = 0
    for kv in ents:
        c = kv.get("classname", "")
        if c in ("ls_weapon_spawner", "ls_loot_container", "ls_supply_drop", "ls_marker") or kv.get("targetname") == "lst_shop":
            if "origin" not in kv:
                continue
            o = origin(kv)
            z = o[2]
            # the floor under it
            t = h.trace((o[0], o[1], z + HALF + 8), (o[0], o[1], z - 400))
            fz = t["end"][2] - HALF if t["frac"] < 1 and not t["startsolid"] else z
            r = 80 if c == "ls_supply_drop" else 96 if kv.get("targetname") == "lst_shop" else 40   # (the shopkeeper stands behind a counter: you buy across it)
            a = area_at(areas, o[0], o[1], fz, r=r, dz=32)
            n_items += 1
            if not a or a["id"] not in there or a["id"] not in back:
                miss += 1
                print("  ITEM %s%s at %d %d %d: not on the walk" % (c, " " + kv.get("kind", "") if c == "ls_marker" else "", o[0], o[1], o[2]))
    print("items and markers: %d, %d off the walk" % (n_items, miss))
    ok &= miss == 0

    # creatures: spawners -> barricades, and no way in past them
    gates = []
    for kv in ents:
        if kv.get("classname") == "ls_barricade" and kv.get("model", "").startswith("*"):
            gates.append((kv.get("label") or kv.get("name") or kv.get("targetname"), bounds(kv)))
    gate_areas = []
    for name, b in gates:
        s = set()
        for a in areas.values():
            if a["x0"] <= b[3] + 8 and a["x1"] >= b[0] - 8 and a["y0"] <= b[4] + 8 and a["y1"] >= b[1] - 8 and b[2] - 40 <= a["z"] <= b[5]:
                s.add(a["id"])
        gate_areas.append((name, b, s))
    blocked = set().union(*[s for _, _, s in gate_areas]) if gate_areas else set()
    base = None
    for kv in ents:
        if kv.get("targetname") == "lst_base" and kv.get("size"):
            o = origin(kv)
            sx, sy, sz = (float(v) for v in kv["size"].split())
            base = (o[0] - sx / 2, o[1] - sy / 2, o[2] - 64, o[0] + sx / 2, o[1] + sy / 2, o[2] + sz + 64)
    base_areas = set()
    if base:
        for a in areas.values():
            cx, cy = (a["x0"] + a["x1"]) / 2, (a["y0"] + a["y1"]) / 2
            if base[0] <= cx <= base[3] and base[1] <= cy <= base[4] and base[2] <= a["z"] <= base[5]:
                base_areas.add(a["id"])
    sp = [origin(kv) for kv in ents if kv.get("classname") == "ls_wave_spawner"]
    reached = set()
    leak = 0
    print("creatures: %d wave spawners, %d barricades%s" % (len(sp), len(gates), ", base %d areas" % len(base_areas) if base else ""))
    for o in sp:
        t = h.trace((o[0], o[1], o[2] + HALF + 8), (o[0], o[1], o[2] - 400))
        a = area_at(areas, o[0], o[1], t["end"][2] - HALF, r=96, dz=32)
        if not a:
            print("  SPAWNER at %d %d %d: no nav area near it" % o)
            ok = False
            continue
        free = bfs([a["id"]], walk)
        hits = [name for name, b, s in gate_areas if s & free]
        reached.update(hits)
        walled = {a["id"]} | bfs([a["id"]], {k: [m for m in v if m not in blocked] for k, v in walk.items()})
        inside = (walled & home) | (walled & base_areas)
        if inside:
            leak += 1
            k = sorted(inside)[0]
            print("  LEAK from the spawner at %d %d: reaches %s at %d %d z %d with every barricade up" %
                  (o[0], o[1], "a CT spawn" if k in home else "the base", (areas[k]["x0"] + areas[k]["x1"]) / 2, (areas[k]["y0"] + areas[k]["y1"]) / 2, areas[k]["z"]))
        print("  spawner %5d %5d: reaches %d gate(s) walking: %s" % (o[0], o[1], len(hits), ", ".join(hits) or "NONE"))
        if not hits:
            ok = False
    for name, b, s in gate_areas:
        if name not in reached:
            print("  GATE %s: no spawner reaches it" % name)
            ok = False
        if not s:
            print("  GATE %s: no nav area through it" % name)
            ok = False
    ok &= leak == 0
    print("ALL CHECKS PASS" if ok else "SOME CHECKS FAIL")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
