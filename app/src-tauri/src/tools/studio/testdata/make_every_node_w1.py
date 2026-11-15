# Builds every_node_w1.graph.json: W1's utility nodes (spatial queries, random picks, math, text,
# number arrays, vectors, relative space and aim, Multi Gate, Do Once Per Player, Toggle Variable,
# On Tick), each wired so the graph checks clean: every pure output goes to a Print String in one
# flow from Player Spawned; the flow nodes follow it; two On Ticks (one fast, with a Line of Sight:
# the Problems warning) and their resets from Round Started / Player Killed.
# Run from this folder: python3 make_every_node_w1.py
import json

nodes, edges = [], []
def N(id, type, params=None, x=0, y=0):
    nodes.append({"id": id, "type": type, "pos": {"x": x * 260, "y": y * 220}, "params": params or {}})
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

VARS = [{"id": "flag", "name": "Flag", "type": "bool", "scope": "mode", "default": False},
        {"id": "pflag", "name": "Player flag", "type": "bool", "scope": "player", "default": False},
        {"id": "tflag", "name": "Team flag", "type": "bool", "scope": "team", "default": True}]

N("ps", "event.player_spawned", {}, 0, 0)
N("vec", "vec.make", {"x": 0, "y": 0, "z": 64}, 0, 2)
N("vec2", "vec.make", {"x": 100, "y": 50, "z": 0}, 0, 3)
N("txt", "text.literal", {"value": "  Hello, World,  again  "}, 0, 4)
N("n1", "math.number", {"value": 3}, 0, 5)
N("n2", "math.number", {"value": 1}, 0, 6)
N("nums", "array.make", {"of": "number", "count": 2}, 0, 7)
W("n1", "out", "nums", "i0")
W("n2", "out", "nums", "i1")
N("team", "player.get_team", {}, 0, 8)
W("ps", "player", "team", "player")
N("ent", "spatial.nearest_entity", {"class": "info_player_start", "max": 4096}, 0, 9)
W("ps", "player", "ent", "to")

last = ["ps", "then"]
k = [0]
def show(src, pin, text="[0]"):
    k[0] += 1
    pid = f"p{k[0]}"
    N(pid, "debug.print", {"text": text, "chat": False}, 2 + k[0] % 12, k[0] // 12)
    W(last[0], last[1], pid, "exec")
    W(src, pin, pid, "value")
    last[:] = [pid, "then"]

def pure(id, type, params, wires, outs):
    N(id, type, params, 1, 10 + len(nodes) // 6)
    for src, sp, pin in wires:
        W(src, sp, id, pin)
    for o in outs:
        show(id, o)

PL = ("ps", "player")
pure("pir", "spatial.players_in_radius", {"radius": 500, "alive": True, "team": "any"}, [("vec", "out", "pos")], ["out"])
pure("np", "spatial.nearest_player", {"max": 2048, "team": "ct", "bots": "bots"}, [(*PL, "to")], ["out", "dist"])
pure("eir", "spatial.entities_in_radius", {"radius": 800, "class": "info_player_start"}, [(*PL, "to")], ["out"])
pure("eir2", "spatial.entities_in_radius", {"radius": 300, "class": ""}, [("vec", "out", "pos")], ["out"])
show("ent", "out"); show("ent", "dist")
pure("los", "spatial.line_of_sight", {"monsters": True}, [(*PL, "a"), ("vec", "out", "b_pos")], ["clear", "hit"])
pure("rc", "random.chance", {"pct": 30}, [], ["out"])
pure("rb", "random.bool", {}, [], ["out"])
pure("wp", "random.weighted", {"of": "text"}, [("nums", "out", "weights"), ("split", "out", "items")], ["index", "item"])
pure("mr", "math.map_range", {"a": 5, "in_min": 0, "in_max": 10, "out_min": 100, "out_max": 200, "clamp": True}, [], ["out"])
pure("sin", "math.trig", {"a": 30, "op": "sin"}, [], ["out"])
pure("tan", "math.trig", {"a": 0.5, "op": "tan", "unit": "rad"}, [], ["out"])
pure("at2", "math.atan2", {"y": 1, "x": -1}, [], ["out"])
pure("ac", "math.angle_convert", {"a": 3.14159, "op": "to_deg"}, [], ["out"])
pure("tr", "math.fn", {"a": -2.7, "op": "trunc"}, [], ["out"])
pure("ne", "flow.nearly_equal", {"a": 0.1, "b": 0.10001, "tol": 0.001}, [], ["out"])
pure("split", "text.split", {"sep": ","}, [("txt", "out", "a")], ["out"])
pure("rep", "text.replace", {"find": "World", "with": "There"}, [("txt", "out", "a")], ["out"])
pure("sub", "text.substring", {"start": 2, "len": 5}, [("txt", "out", "a")], ["out"])
pure("up", "text.upper", {}, [("txt", "out", "a")], ["out"])
pure("lo", "text.lower", {}, [("txt", "out", "a")], ["out"])
pure("trim", "text.trim", {}, [("txt", "out", "a")], ["out"])
pure("sw", "text.starts_with", {"b": "  hel"}, [("txt", "out", "a")], ["out"])
pure("ew", "text.ends_with", {"b": "AIN  "}, [("txt", "out", "a")], ["out"])
pure("join", "text.join", {"sep": " - "}, [("split", "out", "array")], ["out"])
pure("ft", "text.format_time", {"style": "auto"}, [("n1", "out", "secs")], ["out"])
pure("fn", "text.format_number", {"decimals": 2, "thousands": True}, [("mr", "out", "a")], ["out"])
pure("sum", "array.sum", {}, [("nums", "out", "array")], ["out"])
pure("amin", "array.min", {}, [("nums", "out", "array")], ["out", "index"])
pure("amax", "array.max", {}, [("nums", "out", "array")], ["out", "index"])
pure("avg", "array.average", {}, [("nums", "out", "array")], ["out"])
pure("sl", "array.slice", {"of": "text", "start": 1, "count": 2}, [("split", "out", "array")], ["out"])
pure("dot", "vec.dot", {}, [("vec", "out", "a"), ("vec2", "out", "b")], ["out"])
pure("cross", "vec.cross", {}, [("vec", "out", "a"), ("vec2", "out", "b")], ["out"])
pure("rnd", "vec.random_in_radius", {"radius": 200, "mode": "3d"}, [("vec", "out", "c")], ["out"])
pure("rot", "vec.rotate", {"yaw": 90, "pitch": 10}, [("vec2", "out", "a")], ["out"])
pure("lerp", "vec.lerp", {"alpha": 0.25}, [("vec", "out", "a"), ("vec2", "out", "b")], ["out"])
pure("toang", "vec.to_angles", {}, [("vec2", "out", "a")], ["out"])
pure("fwd", "vec.forward", {}, [("toang", "out", "a")], ["out"])
pure("loc", "rel.to_local", {}, [(*PL, "thing"), ("vec2", "out", "pos")], ["out"])
pure("wor", "rel.to_world", {}, [(*PL, "thing"), ("vec2", "out", "off")], ["out"])
pure("rp", "rel.position_of", {}, [(*PL, "a"), ("ent", "out", "b")], ["forward", "right", "up"])
pure("ang", "rel.angle_to", {}, [(*PL, "a"), ("ent", "out", "b")], ["out"])
pure("front", "rel.is_in_front", {"cone": 90}, [(*PL, "a"), ("ent", "out", "b")], ["out"])
pure("behind", "rel.is_behind", {"cone": 120}, [(*PL, "a"), ("ent", "out", "b")], ["out"])
pure("eye", "aim.eye_pos", {}, [(*PL, "player")], ["out"])
pure("dir", "aim.dir", {}, [(*PL, "player")], ["out"])
pure("aim", "aim.point", {"max": 2048}, [(*PL, "player")], ["pos", "ent", "normal", "hit"])

# the flow nodes
def flow(id, type, params, wires, x):
    N(id, type, params, x, 40)
    W(last[0], last[1], id, "exec")
    for src, sp, pin in wires:
        W(src, sp, id, pin)
    last[:] = [id, "then"]
flow("tg", "var.toggle", {"var": "flag"}, [], 1)
show("tg", "value")
flow("tgp", "var.toggle", {"var": "pflag"}, [(*PL, "player")], 2)
flow("tgt", "var.toggle", {"var": "tflag"}, [("team", "out", "team")], 3)
show("tgt", "value")
flow("rev", "array.reverse", {"of": "number"}, [("nums", "out", "array")], 4)
# (Do Once Per Player's resets from the same flow: its Player is Player Spawned's)
N("seq", "flow.sequence", {"count": 3}, 5, 41)
W(last[0], last[1], "seq", "exec")
N("dopp", "flow.do_once_per_player", {}, 5, 40)
W("seq", "then0", "dopp", "reset")
W("seq", "then1", "dopp", "reset_player")
W("seq", "then2", "dopp", "exec")
W(*PL, "dopp", "player")
last[:] = ["dopp", "then"]
N("mg", "flow.multi_gate", {"count": 3, "random": False, "loop": True, "start": 1}, 6, 40)
W(last[0], last[1], "mg", "exec")
for i in range(3):
    N(f"mgp{i}", "debug.print", {"text": f"gate {i}", "chat": False}, 7, 40 + i)
    W("mg", f"then{i}", f"mgp{i}", "exec")
N("mgr", "flow.multi_gate", {"count": 2, "random": True, "loop": False}, 6, 44)
W("mgp0", "then", "mgr", "exec")
N("mgrp", "debug.print", {"text": "random gate", "chat": False}, 7, 44)
W("mgr", "then1", "mgrp", "exec")
# resets
N("rs", "event.round_started", {}, 0, 46)
W("rs", "then", "mg", "reset")
# On Tick: 0.1 s with the spatial and aim nodes; 0.02 s with a Line of Sight (warned)
N("tick", "event.on_tick", {"interval": 0.1}, 0, 50)
N("tnp", "spatial.nearest_player", {"max": 8192}, 1, 51)
W("vec", "out", "tnp", "pos")
N("taim", "aim.point", {"max": 4096}, 2, 51)
W("tnp", "out", "taim", "player")
N("tft", "text.format_time", {"style": "hms"}, 2, 52)
W("tick", "delta", "tft", "secs")
N("tp1", "debug.print", {"text": "tick [0]", "chat": False}, 3, 50)
W("tick", "then", "tp1", "exec")
W("taim", "pos", "tp1", "value")
N("tp2", "debug.print", {"text": "delta [0]", "chat": False}, 4, 50)
W("tp1", "then", "tp2", "exec")
W("tft", "out", "tp2", "value")
N("fast", "event.on_tick", {"interval": 0.02}, 0, 54)
N("flos", "spatial.line_of_sight", {}, 1, 55)
W("vec", "out", "flos", "a_pos")
W("vec2", "out", "flos", "b_pos")
N("fp", "debug.print", {"text": "fast [0]", "chat": False}, 2, 54)
W("fast", "then", "fp", "exec")
W("flos", "clear", "fp", "value")

g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": "every_node_w1", "name": "Every Node W1", "description": "The utility nodes (W1)"},
     "variables": VARS, "nodes": nodes, "edges": edges}
json.dump(g, open("every_node_w1.graph.json", "w"), indent=1)
print(len(nodes), "nodes", len(edges), "edges")
