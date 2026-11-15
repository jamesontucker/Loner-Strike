#!/usr/bin/env python3
"""The final Studio check's graphs (docs/notes/FinalCheck.md), written once and kept as version 3
graphs on purpose: after the type system (graph v4) lands they also exercise the migration.

    python3 scripts/finalcheck/make_graphs.py [--sim-base A]

Writes scripts/finalcheck/graphs/:
  cover_*.graph.json  every node added this round (since 3782f0ae), in game: the node agents'
                      every-node test graphs as HEAD has them, each its own mode (fc_cover_*)
  stress_*.graph.json the janky set, every comfort limit at its max (fc_st_*)
  perf_*.graph.json   the cost runs' realistic modes: shipped templates (fc_perf_*)
  big.graph.json      the Studio UI check: the cover graphs side by side, cut to the 400-node cap
  new_kinds.json      the node kinds added this round (what coverage counts)

The precache graphs list real files from the sim base (they must exist on the server), so run it
with the sim base the check will use. Re-running overwrites everything; don't, once the check has
started on these (their point is being the v3 copies).
"""
import json
import os
import re
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT = os.path.join(REPO, "scripts", "finalcheck", "graphs")
STUDIO = "app/src-tauri/src/tools/studio"
NODES = "app/ui/src/views/tools/studio/nodes.json"
ROUND_START = "3782f0ae"  # the combined re-check: the round's node batches came after it
SIMS = os.path.expanduser("~/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Loner-Strike/Sim")
BASE = sys.argv[sys.argv.index("--sim-base") + 1] if "--sim-base" in sys.argv else "A"


def git_show(rev, path):
    return subprocess.run(["git", "-C", REPO, "show", f"{rev}:{path}"], capture_output=True, text=True, check=True).stdout


def kinds(text):
    return {n["type"] for n in json.loads(text)["nodes"]}


def save(name, g):
    with open(os.path.join(OUT, name), "w") as f:
        json.dump(g, f, indent=1)
        f.write("\n")


def limits():
    """(id, label, max) for every comfort limit in limits.rs at HEAD."""
    src = git_show("HEAD", f"{STUDIO}/limits.rs")
    return [(m[0], m[1], int(m[2])) for m in re.findall(r'Limit \{ id: "(\w+)", label: "([^"]+)", default: \d+, lag_above: \d+, max: (\d+)', src)]


def limit_vars():
    return [{"id": f"lim_{i}", "name": label[:30], "type": "number", "scope": "mode", "default": mx,
             "setting": {"label": label[:30], "help": "A comfort limit (final check: at its max).", "min": 1, "max": mx, "step": 1, "group": "Limits"}}
            for i, label, mx in limits()]


class G:
    """A small graph builder: nodes in a row, wires by (node, pin)."""

    def __init__(self, mode_id, name, desc):
        self.g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": mode_id, "name": name, "description": desc},
                  "variables": limit_vars(), "nodes": [], "edges": [], "comments": []}
        self.n = 0

    def node(self, kind, **params):
        self.n += 1
        nid = f"n{self.n}"
        self.g["nodes"].append({"id": nid, "type": kind, "pos": {"x": (self.n % 12) * 320, "y": (self.n // 12) * 260}, "params": params})
        return nid

    def wire(self, a, ap, b, bp):
        self.g["edges"].append({"id": f"w{len(self.g['edges']) + 1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

    def chain(self, start, pin, kinds_params):
        """exec chain: start.pin -> each node's exec, then -> next"""
        prev, ppin = start, pin
        ids = []
        for kind, params in kinds_params:
            nid = self.node(kind, **params)
            self.wire(prev, ppin, nid, "exec")
            prev, ppin = nid, "then"
            ids.append(nid)
        return ids


def playable(g):
    """An every-node graph made to compile, made a mode that passes the Studio's check: each node's
    unset settings as the Studio fills them when it's added, and a position for the effects that
    need one (the node agents' graphs leave those out)."""
    reg = {n["type"]: n for n in json.load(open(os.path.join(REPO, NODES)))["nodes"]}
    wired = {(e["to"]["node"], e["to"]["pin"]) for e in g["edges"]}
    spot = None
    for n in list(g["nodes"]):
        d = reg.get(n["type"])
        if not d:
            continue
        for p in d.get("params", []):
            if p["id"] not in n.setdefault("params", {}) and p.get("default") not in (None, ""):
                n["params"][p["id"]] = p["default"]
        ins = {i["id"] for i in d.get("inputs", [])}
        need = [x for x in ("pos", "follow", "at", "at_ent") if x in ins]
        if n["type"] in ("fx.marker", "fun.burst") and not any((n["id"], x) in wired for x in need):
            if spot is None:
                spot = "fc_spot"
                g["nodes"].append({"id": spot, "type": "vec.make", "pos": {"x": n["pos"]["x"] - 200, "y": n["pos"]["y"]}, "params": {"x": 0, "y": 0, "z": 64}})
            g["edges"].append({"id": f"fc_w_{n['id']}", "from": {"node": spot, "pin": "out"}, "to": {"node": n["id"], "pin": "pos"}})


def cover():
    head = kinds(open(os.path.join(REPO, NODES)).read())
    new = sorted(head - kinds(git_show(ROUND_START, NODES)))
    save("new_kinds.json", {"since": ROUND_START, "kinds": new})
    # the node agents' every-node graphs: the fewest that hold every new kind
    names = sorted(f for f in os.listdir(os.path.join(REPO, STUDIO, "testdata")) if f.startswith("every_node") and f.endswith(".graph.json"))
    graphs = {f: json.loads(git_show("HEAD", f"{STUDIO}/testdata/{f}")) for f in names}
    left, picked = set(new), []
    while left:
        f = max(graphs, key=lambda f: len(left & {n["type"] for n in graphs[f]["nodes"]}))
        got = left & {n["type"] for n in graphs[f]["nodes"]}
        if not got:
            break
        picked.append(f)
        left -= got
    out = []
    for f in picked:
        g = graphs[f]
        short = f[len("every_node"):-len(".graph.json")].strip("_") or "main"
        g["mode"]["id"] = f"fc_cover_{short}"
        g["mode"]["name"] = f"FC cover {short}"
        g["mode"]["description"] = f"Final check: {f} in a game"[:60]
        playable(g)
        save(f"cover_{short}.graph.json", g)
        out.append((f"cover_{short}.graph.json", len(g["nodes"]), len(set(new) & {n["type"] for n in g["nodes"]})))
    return new, out, sorted(left)


def stress():
    made = []
    # 1. infinite spawning on the fastest On Tick: every spawning node, every player, nothing removed
    s = G("fc_st_spawn", "FC spawn flood", "Final check: spawns every tick until the caps")
    t = s.node("event.on_tick", interval=0)
    each = s.node("flow.for_each_player", which="alive")
    s.wire(t, "then", each, "exec")
    pos = s.node("ent.position")
    s.wire(each, "player", pos, "in")
    ids = s.chain(each, "each", [
        ("ent.spawn_entity", {"class": "env_sprite", "keys": "model=sprites/glow01.spr|rendermode=5|renderamt=255", "blocked": "force"}),
        ("ent.spawn_prop", {"model": "models/w_c4.mdl", "solid": False, "blocked": "force"}),
        ("proj.spawn", {"model": "models/w_hegrenade.mdl", "life": 60, "speed": 50, "trail": True}),
        ("vol.spawn", {"shape": "box"}),
        ("fx.marker", {"who": "all"}),
        ("snd.loop", {"sound": "ambience/wind1.wav"}),
    ])
    for nid, pin in zip(ids, ["pos", "pos", "pos", "pos", "pos", "pos"]):
        s.wire(pos, "out", nid, pin)
    trail = s.node("fun.trail")
    s.wire(ids[-1], "then", trail, "exec")
    s.wire(ids[1], "out", trail, "ent")
    save("stress_spawn.graph.json", s.g)
    made.append(("stress_spawn.graph.json", "caps: entity slots, projectiles, trigger volumes, markers, loop sounds, trails"))

    # 2. nested loops on a fast On Tick: the loop cap (10,000 a loop) nested, every second
    s = G("fc_st_loops", "FC nested loops", "Final check: two loops at the cap, nested, every second")
    s.g["variables"].append({"id": "count", "name": "Count", "type": "number", "scope": "mode", "default": 0})
    t = s.node("event.on_tick", interval=1)
    a = s.node("flow.for_loop", first=0, last=9999)
    b = s.node("flow.for_loop", first=0, last=9999)
    add = s.node("var.add", var="count", amount=1)
    s.wire(t, "then", a, "exec")
    s.wire(a, "body", b, "exec")
    s.wire(b, "body", add, "exec")
    save("stress_loops.graph.json", s.g)
    made.append(("stress_loops.graph.json", "10,000 x 10,000 iterations a second: does the server hang?"))
    # 2b. the milder one people really make: one capped loop every tick
    s = G("fc_st_loop1", "FC loop per tick", "Final check: one loop at the cap on every tick")
    s.g["variables"].append({"id": "count", "name": "Count", "type": "number", "scope": "mode", "default": 0})
    t = s.node("event.on_tick", interval=0)
    a = s.node("flow.for_loop", first=0, last=9999)
    add = s.node("var.add", var="count", amount=1)
    s.wire(t, "then", a, "exec")
    s.wire(a, "body", add, "exec")
    save("stress_loop1.graph.json", s.g)
    made.append(("stress_loop1.graph.json", "10,000 iterations every tick"))

    # 3. per-frame effects and messages to everyone, every tick
    s = G("fc_st_fx", "FC effects flood", "Final check: every effect and message every tick")
    t = s.node("event.on_tick", interval=0)
    each = s.node("flow.for_each_player", which="all")
    s.wire(t, "then", each, "exec")
    pos = s.node("ent.position")
    s.wire(each, "player", pos, "in")
    up = s.node("vec.make", x=0, y=0, z=200)
    fx = s.chain(each, "each", [
        ("fx.beam", {"secs": 1}), ("debug.draw_line", {"secs": 1}), ("fx.dlight", {}), ("fx.ring", {}),
        ("fun.burst", {"count": 8}), ("fx.sprite_at", {}), ("fx.tracer", {}),
        ("msg.color_chat", {"text": "flood"}), ("msg.big_hud", {"text": "flood", "seconds": 1}),
        ("snd.play_2d", {"who": "all", "sound": "buttons/bell1.wav"}), ("msg.chat", {"text": "flood"}),
    ])
    s.wire(pos, "out", fx[0], "a"); s.wire(up, "out", fx[0], "b")
    s.wire(pos, "out", fx[1], "a"); s.wire(up, "out", fx[1], "b")
    s.wire(pos, "out", fx[2], "pos"); s.wire(pos, "out", fx[3], "pos")
    s.wire(pos, "out", fx[4], "pos"); s.wire(pos, "out", fx[5], "pos")
    s.wire(pos, "out", fx[6], "from"); s.wire(up, "out", fx[6], "to")
    for i in (7, 8, 10):
        s.wire(each, "player", fx[i], "player")
    s.wire(each, "player", fx[9], "player")
    save("stress_fx.graph.json", s.g)
    made.append(("stress_fx.graph.json", "cap: Effect messages a frame (160 at max)"))

    # 4. a message burst: 10,000 messages in one frame at round start
    s = G("fc_st_burst", "FC message burst", "Final check: 10,000 messages in one server frame")
    r = s.node("event.round_started")
    a = s.node("flow.for_loop", first=0, last=9999)
    s.wire(r, "then", a, "exec")
    s.chain(a, "body", [("msg.color_chat", {"text": "burst"}), ("msg.big_hud", {"text": "burst", "seconds": 1}), ("snd.play_2d", {"who": "all", "sound": "buttons/bell1.wav"})])
    save("stress_burst.graph.json", s.g)
    made.append(("stress_burst.graph.json", "cap: Effect messages a frame; a person joined must not overflow"))

    # 5. the precache flood: the mode's own files at the most (160) and past it (refused)
    root = os.path.join(SIMS, BASE)
    sounds = sorted(os.path.relpath(os.path.join(d, f), os.path.join(root, "valve/sound")).replace(os.sep, "/")
                    for d, _, fs in os.walk(os.path.join(root, "valve/sound")) for f in fs if f.endswith(".wav") and re.fullmatch(r"[A-Za-z0-9_\-.]{1,50}", f))
    sounds = [x for x in sounds if re.fullmatch(r"[A-Za-z0-9_\-./]{1,58}", x)]
    models = sorted(f"models/{f}" for f in os.listdir(os.path.join(root, "valve/models")) if f.endswith(".mdl") and not re.search(r"\d\d\.mdl$|t\.mdl$", f) and re.fullmatch(r"[A-Za-z0-9_\-.]{1,50}", f))
    if len(sounds) < 120 or len(models) < 50:
        raise SystemExit(f"the sim base {BASE} has too few files for the precache graphs ({len(sounds)} sounds, {len(models)} models)")
    for mode_id, name, n_snd, n_mdl in [("fc_st_pre160", "stress_precache160.graph.json", 110, 50), ("fc_st_pre170", "stress_precache170.graph.json", 115, 55)]:
        s = G(mode_id, f"FC precache {n_snd + n_mdl}", f"Final check: {n_snd + n_mdl} files of its own")
        r = s.node("event.round_started")
        prev, pin = r, "then"
        for i in range(n_snd):
            nid = s.node("snd.play_2d", who="all", sound=sounds[i * len(sounds) // n_snd])
            s.wire(prev, pin, nid, "exec")
            prev, pin = nid, "then"
        for i in range(n_mdl):
            nid = s.node("ent.spawn_prop", model=models[i * len(models) // n_mdl], solid=False, blocked="force")
            s.wire(prev, pin, nid, "exec")
            prev, pin = nid, "then"
        save(name, s.g)
    made.append(("stress_precache160.graph.json", "160 files: loads; on a heavy map the game's 512 is the risk"))
    made.append(("stress_precache170.graph.json", "170 files: the Studio must refuse it with a clear message"))
    return made


def perf():
    """Realistic Studio modes for the cost runs: two shipped templates, as HEAD has them."""
    made = []
    for tpl, short in [("team-dm", "tdm"), ("last-stand", "ls")]:
        g = json.loads(git_show("HEAD", f"app/ui/src/views/tools/studio/templates/{tpl}.graph.json"))
        g["mode"]["id"] = f"fc_perf_{short}"
        g["mode"]["name"] = f"FC perf {tpl}"
        g["mode"]["description"] = f"Final check: the {tpl} template for the cost runs"[:60]
        g["mode"].pop("uid", None)
        save(f"perf_{short}.graph.json", g)
        made.append((f"perf_{short}.graph.json", len(g["nodes"]), g.get("version")))
    return made


def big(cover_files):
    """The cover graphs side by side in one graph (the UI check only: never compiled)."""
    g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": "fc_big", "name": "FC big graph", "description": "Final check: Studio UI at the 400-node cap"},
         "variables": [], "nodes": [], "edges": [], "comments": []}
    seen_vars = set()
    for k, (f, _, _) in enumerate(cover_files):
        src = json.load(open(os.path.join(OUT, f)))
        dx = k * 6000
        pre = f"g{k}_"
        for v in src.get("variables", []):
            if v["id"] not in seen_vars:
                seen_vars.add(v["id"])
                g["variables"].append(v)
        for key in ("enums", "structs", "functions", "macros", "markers"):
            for item in src.get(key, []):
                if all(item.get("id") != x.get("id") for x in g.setdefault(key, [])):
                    g[key].append(item)
        for n in src["nodes"]:
            n = dict(n)
            n["id"] = pre + n["id"]
            n["pos"] = {"x": n["pos"]["x"] + dx, "y": n["pos"]["y"]}
            g["nodes"].append(n)
        for e in src["edges"]:
            e = json.loads(json.dumps(e))
            e["id"] = pre + e["id"]
            e["from"]["node"] = pre + e["from"]["node"]
            e["to"]["node"] = pre + e["to"]["node"]
            g["edges"].append(e)
    # a graph holds 400 nodes at most (graph.rs MAX_NODES, schema.ts): the biggest a person can open
    keep = {n["id"] for n in g["nodes"][:400]}
    g["nodes"] = [n for n in g["nodes"] if n["id"] in keep]
    g["edges"] = [e for e in g["edges"] if e["from"]["node"] in keep and e["to"]["node"] in keep]
    save("big.graph.json", g)
    return len(g["nodes"]), len(g["edges"])


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    new, cov, left = cover()
    print(f"{len(new)} node kinds added since {ROUND_START}; cover graphs:")
    for f, n, k in cov:
        print(f"  {f}: {n} nodes, {k} new kinds")
    if left:
        print(f"  NOT in any every-node graph: {left}")
    for f, why in stress():
        print(f"  {f}: {why}")
    for f, n, v in perf():
        print(f"  {f}: {n} nodes (version {v})")
    print("  big.graph.json: %d nodes, %d wires" % big(cov))
