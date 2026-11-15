# Writes every_node_assets.graph.json: files as values (gen/assets.rs): Model / Sprite / Sound
# variables (mode, player, an array), a Function's local that only it holds, the Model / Sprite /
# Sound nodes, Set Variable with a file, File Name Of. Run from this folder: python3 make_every_node_assets.py
import json

nodes, edges = [], []
def N(id, type, params=None, graph=None):
    n = {"id": id, "type": type, "pos": {"x": 260 * (len(nodes) % 8), "y": 220 * (len(nodes) // 8)}, "params": params or {}}
    if graph:
        n["graph"] = graph
    nodes.append(n)
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges) + 1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

variables = [
    {"id": "taunt", "name": "Taunt", "type": "sound", "scope": "mode", "default": "items/gunpickup2.wav"},
    {"id": "skin", "name": "Skin", "type": "model", "scope": "player", "default": "models/w_c4.mdl"},
    {"id": "mark", "name": "Mark", "type": "sprite", "scope": "mode", "default": ""},
    {"id": "pool", "name": "Pool", "type": "array<sound>", "scope": "mode"},
]
functions = [{"id": "fk", "name": "Kit", "inputs": [{"id": "p", "name": "Who", "type": "player"}], "outputs": [],
              "locals": [{"id": "lbox", "name": "Box", "type": "model", "scope": "mode", "default": "models/w_weaponbox.mdl"}]}]

N("ps", "event.player_spawned")
N("setm", "var.set", {"var": "mark", "value": "sprites/arrow1.spr"})
W("ps", "then", "setm", "exec")
N("snd", "asset.sound", {"file": "ambience/thunder_clap.wav"})
N("add", "array.add", {"of": "sound"})
N("pool_get", "var.get", {"var": "pool"})
W("setm", "then", "add", "exec")
W("pool_get", "out", "add", "array")
W("snd", "out", "add", "item")
N("tg", "var.get", {"var": "taunt"})
N("nm", "asset.file_name")
W("tg", "out", "nm", "file")
N("pr", "debug.print", {"chat": False})
W("add", "then", "pr", "exec")
W("nm", "out", "pr", "value")
N("mdl", "asset.model", {"file": "models/w_c4.mdl"})
N("spr", "asset.sprite", {"file": "sprites/arrow1.spr"})
N("nm2", "asset.file_name")
W("mdl", "out", "nm2", "file")
N("pr2", "debug.print", {"chat": False})
W("pr", "then", "pr2", "exec")
W("nm2", "out", "pr2", "value")
N("nm3", "asset.file_name")
W("spr", "out", "nm3", "file")
N("pr3", "debug.print", {"chat": False})
W("pr2", "then", "pr3", "exec")
W("nm3", "out", "pr3", "value")
# files wired into typed settings: a Sound into Play Sound 2D, a Model into Spawn NPC and Spawn Prop
N("p2d", "snd.play_2d", {})
W("pr3", "then", "p2d", "exec")
W("snd", "out", "p2d", "sound")
N("npc", "npc.spawn", {})
W("p2d", "then", "npc", "exec")
W("ps", "player", "npc", "at_ent")
W("mdl", "out", "npc", "model")
N("prop", "ent.spawn_prop", {})
W("npc", "then", "prop", "exec")
W("ps", "player", "prop", "at")
W("mdl", "out", "prop", "model")
N("call", "fn.call", {"func": "fk"})
W("prop", "then", "call", "exec")
W("ps", "player", "call", "p")
# the Function: shows its local's name
N("fk_e", "fn.entry", graph="fk")
N("fk_get", "var.get", {"var": "lbox"}, graph="fk")
N("fk_nm", "asset.file_name", graph="fk")
N("fk_pr", "debug.print", {"chat": False}, graph="fk")
W("fk_e", "then", "fk_pr", "exec")
W("fk_get", "out", "fk_nm", "file")
W("fk_nm", "out", "fk_pr", "value")
g = {"schema": "loner-strike/graph", "version": 4, "mode": {"id": "every_node_assets", "name": "Every node (files as values)", "description": "Model, Sprite and Sound values"},
     "variables": variables, "nodes": nodes, "edges": edges, "comments": [], "functions": functions}
json.dump(g, open("every_node_assets.graph.json", "w"), indent=1)
print(len(nodes), "nodes")
