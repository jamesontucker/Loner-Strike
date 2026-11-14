# Builds every_node_3.graph.json: every registry node that every_node and every_node_2 don't have
# (the game and server facts, world settings, player actions, entities, effects, AMX Mod X interop,
# the newer events), each wired so the graph checks clean: actions in one flow from Player Spawned
# (its player for Player pins), an entity from Find Component, positions from Make Vector.
# Run from this folder: python3 make_every_node_3.py
import json

reg = json.load(open("../../../../../ui/src/views/tools/studio/nodes.json"))
have = set()
for f in ("every_node.graph.json", "every_node_2.graph.json"):
    have |= {n["type"] for n in json.load(open(f))["nodes"]}
todo = [d for d in reg["nodes"] if d["type"] not in have]

nodes, edges = [], []
def N(id, type, params=None, x=0, y=0):
    nodes.append({"id": id, "type": type, "pos": {"x": x * 260, "y": y * 220}, "params": params or {}})
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

# the Custom Event nodes name one of these (Gate Opened: what the map fires, with inputs; Tick: a timer's)
CUSTOM = [{"id": "ce1", "name": "Gate Opened", "map": True, "inputs": [{"id": "who", "name": "Who", "type": "player"}, {"id": "n", "name": "Count", "type": "number"}, {"id": "t", "name": "Label", "type": "text"}]},
          {"id": "ce2", "name": "Tick", "inputs": []}]
TIMELINES = [{"id": "tl1", "name": "Platform", "length": 4, "loop": True, "autoplay": True, "tick": 0.05, "tracks": [
    {"id": "lift", "name": "Lift", "kind": "float", "keys": [{"t": 0, "v": 0, "ease": "ease"}, {"t": 2, "v": 128}, {"t": 4, "v": 0}]},
    {"id": "pos", "name": "Position", "kind": "vector", "keys": [{"t": 0, "v": [0, 0, 0]}, {"t": 4, "v": [256, 0, 64]}]},
    {"id": "glow", "name": "Glow", "kind": "color", "keys": [{"t": 0, "v": [255, 0, 0]}, {"t": 2, "v": [0, 0, 255], "ease": "ease"}, {"t": 4, "v": [255, 0, 0]}]},
    {"id": "beat", "name": "Beat", "kind": "event", "keys": [{"t": 1}, {"t": 3}]}]}]
PARAMS = {"timeline.play": {"timeline": "tl1"}, "event.custom": {"event": "ce1"}, "event.call": {"event": "ce1"}, "timer.by_event": {"event": "ce2", "secs": 5, "loop": True}}
N("ps", "event.player_spawned", {}, 0, 0)
N("tick", "event.custom", {"event": "ce2"}, 0, 4)
N("ent", "comp.find", {"name": "spot1"}, 0, 2)
N("vec", "vec.make", {"x": 0, "y": 0, "z": 64}, 0, 3)
last = ("ps", "then")
row, col = 0, 1
for i, d in enumerate(todo):
    nid = f"n{i}"
    col += 1
    if col > 12:
        col, row = 1, row + 1
    N(nid, d["type"], PARAMS.get(d["type"], {}), col, 5 + row)
    ins = {p["id"]: p for p in d["inputs"]}
    if d.get("event"):
        continue
    for p in d["inputs"]:
        if p["type"] == "exec":
            continue
        need = p.get("required") or (p["type"] in ("player",) and not p.get("optional") and not p.get("param"))
        if not need:
            continue
        src = {"player": ("ps", "player"), "entity": ("ent", "out"), "vector": ("vec", "out")}.get(p["type"])
        if src:
            W(src[0], src[1], nid, p["id"])
    if "exec" in ins:
        W(last[0], last[1], nid, "exec")
        outs = [o["id"] for o in d["outputs"] if o["type"] == "exec"]
        if outs:
            last = (nid, "then" if "then" in outs else outs[0])

g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": "every_node_3", "name": "Every Node 3", "description": "Game, world, players, effects, interop"},
     "variables": [], "nodes": nodes, "edges": edges, "comments": [], "customEvents": CUSTOM, "timelines": TIMELINES}
json.dump(g, open("every_node_3.graph.json", "w"), indent=1)
print(len(todo), "new kinds;", len(nodes), "nodes", len(edges), "wires")
