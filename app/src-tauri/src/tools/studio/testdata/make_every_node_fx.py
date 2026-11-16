# Writes every_node_fx.graph.json: every effects/sounds/spawning node (gen/fx.rs) and every spawn
# node with the shared transform wired (At entity + Local offset, Rotation, Rotation offset,
# Velocity, Scale; At position). Run from this folder: python3 make_every_node_fx.py
import json

nodes, edges = [], []
def N(id, type, params=None):
    nodes.append({"id": id, "type": type, "pos": {"x": 260 * (len(nodes) % 8), "y": 220 * (len(nodes) // 8)}, "params": params or {}})
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges) + 1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

N("ps", "event.player_spawned")
N("v1", "vec.make", {"x": 0, "y": 0, "z": 40})
N("vofs", "vec.make", {"x": 32, "y": 0, "z": 8})
N("vrot", "vec.make", {"x": 0, "y": 90, "z": 0})
N("vvel", "vec.make", {"x": 0, "y": 0, "z": 300})
chain = []
def A(id, type, params=None, wires=()):
    N(id, type, params)
    chain.append(id)
    for src, sp, pin in wires:
        W(src, sp, id, pin)
P = ("ps", "player")
A("dl", "fx.dlight", {"r": 255, "g": 80, "b": 40, "radius": 300, "life": 2, "decay": 50}, [(*P, "follow")])
A("dl2", "fx.dlight", {}, [("v1", "out", "pos")])
A("dc", "fx.decal", {"decal": "blood", "reach": 128}, [("v1", "out", "pos")])
A("gb", "fx.gibs", {"model": "glass", "material": "glass", "count": 12}, [(*P, "at")])
A("gb2", "fx.gibs", {"custom": "models/hgibs.mdl", "material": "flesh"}, [("v1", "out", "pos")])
A("rg", "fx.ring", {"r": 80, "g": 160, "b": 255, "radius": 400}, [("v1", "out", "pos")])
A("s2", "snd.play_2d", {"who": "player", "sound": "buttons/bell1.wav", "volume": 0.6, "pitch": 120}, [(*P, "player")])
A("s2b", "snd.play_2d", {"who": "ct", "sound": "custom/fx_sting.wav"})
A("sa", "snd.play_at", {"sound": "ambience/thunder_clap.wav", "falloff": "large"}, [("v1", "out", "pos")])
A("sa2", "snd.play_at", {"falloff": "everywhere"}, [(*P, "at")])
A("so", "snd.play_on", {"channel": "voice", "falloff": "small", "pitch": 80}, [(*P, "ent")])
A("lp", "snd.loop", {"every": 2}, [(*P, "on")])
A("lp2", "snd.loop", {"sound": "ambience/drips.wav"}, [("v1", "out", "pos")])
A("st", "snd.stop", {"what": "loop"}, [("lp", "out", "loop")])
A("st2", "snd.stop", {"what": "entity", "channel": "voice"}, [(*P, "ent")])
A("st3", "snd.stop", {"what": "player"}, [(*P, "player")])
A("se", "ent.spawn_entity", {"class": "env_sprite", "keys": "model=sprites/glow01.spr|rendermode=5|renderamt=200", "scale": 2}, [(*P, "at_ent"), ("vofs", "out", "offset"), ("vrot", "out", "angles_offset"), ("vvel", "out", "vel")])
A("se2", "ent.spawn_entity", {"class": "env_beam", "keys": "texture=sprites/laserbeam.spr|LightningStart=x|renderamt=200"}, [("v1", "out", "pos")])
A("se3", "ent.spawn_entity", {"class": "env_spark", "keys": "MaxDelay=1"}, [(*P, "at_ent")])
A("sw", "ent.spawn_weapon", {"item": "awp", "ammo": 30, "respawn": 10}, [(*P, "at_ent"), ("vofs", "out", "offset"), ("vvel", "out", "vel")])
A("sw2", "ent.spawn_weapon", {"item": "kevlar"}, [("v1", "out", "pos"), ("vrot", "out", "angles")])
A("pr", "ent.spawn_prop", {"model": "models/w_c4.mdl", "solid": False}, [(*P, "at"), ("vofs", "out", "offset"), ("vrot", "out", "angles"), ("vvel", "out", "vel")])
A("pr2", "ent.spawn_prop", {"model": "models/w_c4.mdl", "scale": 1.5}, [("v1", "out", "pos")])
A("pr3", "ent.spawn_prop", {"model": "models/w_c4.mdl"}, [(*P, "at")])
# Collision: a box the model's own size (and the helper compiled)
A("pr4", "ent.spawn_prop", {"model": "models/w_c4.mdl", "collision": "box_bullets", "blocked": "force"}, [(*P, "at")])
A("cr", "creature.spawn", {"kind": "zombie"}, [(*P, "at_ent"), ("vofs", "out", "offset"), ("vvel", "out", "vel")])
A("np", "npc.spawn", {"name": "Guide", "yaw": 45, "copy_rot": True}, [(*P, "at_ent"), ("vofs", "out", "offset")])
A("iu", "npc.spawn_use", {}, [("v1", "out", "pos"), ("vrot", "out", "angles")])
A("cc", "comp.create", {"kind": "ls_capture_point"}, [(*P, "at_ent"), ("vofs", "out", "offset"), ("vrot", "out", "angles")])
# If the spot is blocked: at the bot itself (blocked by the bot), each way
A("pforce", "ent.spawn_prop", {"model": "models/w_c4.mdl", "blocked": "force"}, [(*P, "at")])
A("palways", "ent.spawn_prop", {"model": "models/w_c4.mdl", "blocked": "nearby_always", "max_dist": 64}, [(*P, "at")])
A("pnear", "ent.spawn_weapon", {"item": "deagle", "blocked": "nearby", "max_dist": 96}, [(*P, "at_ent")])
N("br", "flow.branch", {})
W("pnear", "spawned", "br", "exec")
W("pnear", "moved", "br", "cond")
# a fallback chain: exactly at the bot, else (Failed) at a point
A("pfail", "ent.spawn_prop", {"model": "models/w_c4.mdl", "blocked": "exact"}, [(*P, "at")])
N("pB", "ent.spawn_prop", {"model": "models/w_c4.mdl"})
W("pfail", "failed", "pB", "exec")
W("v1", "out", "pB", "pos")
N("why", "flow.switch_enum", {"enum": "ls_spawn_fail"})
W("pB", "failed", "why", "exec")
W("pfail", "reason", "why", "in")
A("cretry", "creature.spawn", {"kind": "zombie", "blocked": "retry", "retry_every": 1, "retry_tries": 0}, [(*P, "at_ent")])
W("ps", "then", chain[0], "exec")
for a, b in zip(chain, chain[1:]):
    W(a, "then", b, "exec")
g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": "every_node_fx", "name": "Every node (effects, sounds, spawning)", "description": "the effects agent's nodes and the spawn transform"}, "variables": [], "nodes": nodes, "edges": edges, "comments": [], "functions": []}
json.dump(g, open("every_node_fx.graph.json", "w"), indent=1)
print(len(nodes), "nodes")
