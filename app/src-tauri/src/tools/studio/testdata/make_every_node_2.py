# Builds every_node_2.graph.json: the nodes every_node.graph.json has no room for (a graph holds
# at most 400 nodes): the mode-building pieces (bot hints, barricades, loot, zone, drops, creatures
# with health/tier/pay), wired so it checks clean and so a sim run prints what each one did.
# The sim map needs: an ls_barricade named gate1, an ls_safe_zone named sz with an ls_zone_shrinker
# named shr, an entity named spot1 (creatures and the drop come there). Run from this folder.
import json

nodes, edges = [], []
row = [0]
def N(id, type, params=None, x=0):
    nodes.append({"id": id, "type": type, "pos": {"x": x * 260, "y": row[0] * 220}, "params": params or {}})
    return id
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
def chain(*ids):
    for a, b in zip(ids, ids[1:]):
        W(a, "then", b, "exec")
def nl():
    row[0] += 1
def pr(id, text, x):
    return N(id, "debug.print", {"text": text, "chat": False}, x)

# a player spawns: tagged, friends with their tag, prefers the high layer
N("ps", "event.player_spawned")
N("tag", "bots.tag", {"tag": "sq1"}, 1); W("ps", "player", "tag", "player")
N("fr", "bots.friends", {"a": "tag", "a_tag": "sq1", "b": "tag", "b_tag": "sq1", "on": True}, 2)
N("ly", "bots.layer", {"who": "player", "tag": "", "layer": "high"}, 3); W("ps", "player", "ly", "player")
chain("ps", "tag", "fr", "ly")
nl()

# the round: phases, no buying, creatures as enemies, a goal and an offer at the barricade,
# its health set and mended, loot rolled, the zone pushed on, a creature with its own health/tier/pay,
# a supply drop called, then the fight
N("rs", "event.round_started")
N("ph", "bots.phase", {"phase": "loot"}, 1)
N("nb", "bots.no_buy", {"on": True}, 2)
N("ce", "bots.creature_enemies", {"on": True}, 3)
N("gate", "comp.find", {"name": "gate1"}, 3)
N("goal", "bots.goal", {"who": "ct", "tag": "", "kind": "defend", "prio": 40, "secs": 30, "radius": 128}, 4); W("gate", "out", "goal", "at")
N("off", "bots.offer", {"who": "all", "tag": "", "kind": "trap", "cost": 500, "order": 0}, 5); W("gate", "out", "off", "at")
N("gc", "bots.goal_clear", {"who": "ct", "tag": "", "kind": "defend"}, 6)
N("oc", "bots.offer_clear", {"who": "all", "tag": ""}, 7); W("gate", "out", "oc", "at")
chain("rs", "ph", "nb", "ce", "goal", "off", "gc", "oc")
nl()
N("bset", "comp.barricade_set_hp", {"hp": 50}, 0); W("gate", "out", "bset", "barricade")
N("bhp", "comp.barricade_hp", {}, 1); W("gate", "out", "bhp", "barricade")
N("p_bhp", "debug.print", {"text": "X2 barricade at:", "chat": False}, 1); W("bhp", "out", "p_bhp", "value")
N("brep", "comp.barricade_repair", {}, 2); W("gate", "out", "brep", "barricade")
N("bhp2", "comp.barricade_hp", {}, 3); W("gate", "out", "bhp2", "barricade")
N("p_bhp2", "debug.print", {"text": "X2 barricade mended:", "chat": False}, 3); W("bhp2", "out", "p_bhp2", "value")
N("loot", "comp.loot_roll", {"table": "default", "tier": 1}, 4)
N("p_loot", "debug.print", {"text": "X2 loot:", "chat": False}, 5); W("loot", "item", "p_loot", "value")
N("p_rolled", "debug.print", {"text": "X2 rolled:", "chat": False}, 6); W("loot", "rolled", "p_rolled", "value")
W("oc", "then", "bset", "exec")
chain("bset", "p_bhp", "brep", "p_bhp2", "loot", "p_loot", "p_rolled")
nl()
N("shr", "comp.find", {"name": "shr"}, 0)
N("skip", "comp.zone_skip", {}, 1); W("shr", "out", "skip", "zone")
N("zst", "comp.zone_stage", {}, 1); W("shr", "out", "zst", "zone")
N("p_zst", "debug.print", {"text": "X2 zone stage:", "chat": False}, 2); W("zst", "out", "p_zst", "value")
N("zn", "comp.zone_next", {}, 2); W("shr", "out", "zn", "zone")
N("fmt", "text.format", {"text": "X2 zone next: [0] s, radius [1], closing [2]"}, 3)
W("zn", "secs", "fmt", "a0"); W("zn", "radius", "fmt", "a1"); W("zn", "closing", "fmt", "a2")
N("p_zn", "debug.print", {"text": "", "chat": False}, 4); W("fmt", "out", "p_zn", "value")
W("p_rolled", "then", "skip", "exec")
chain("skip", "p_zst", "p_zn")
nl()
N("cs", "creature.spawn", {"kind": "zombie", "at": "spot1", "health": 300, "tier": "2", "pay": 150}, 0)
N("p_cs", "debug.print", {"text": "X2 creature:", "chat": False}, 1); W("cs", "out", "p_cs", "value")
N("chp", "creature.health", {}, 1); W("cs", "out", "chp", "in")
N("p_chp", "debug.print", {"text": "X2 creature health:", "chat": False}, 2); W("chp", "out", "p_chp", "value")
N("dz", "timer.delay", {"secs": 2}, 3)
N("zst2", "comp.zone_stage", {}, 3); W("shr", "out", "zst2", "zone")
N("p_zst2", "debug.print", {"text": "X2 zone stage 2 s later:", "chat": False}, 4); W("zst2", "out", "p_zst2", "value")
N("drop", "comp.drop_call", {"at": "spot1"}, 2)
N("ph2", "bots.phase", {"phase": "fight"}, 3)
W("p_zn", "then", "cs", "exec")
chain("cs", "p_cs", "p_chp", "drop", "ph2", "dz", "p_zst2")
nl()

# what the components say back
N("bd", "comp.on_barricade_damaged")
N("p_bd", "debug.print", {"text": "X2 barricade damaged, left:", "chat": False}, 1); W("bd", "hp", "p_bd", "value"); W("bd", "then", "p_bd", "exec")
N("br", "comp.on_barricade_repaired", {}, 2)
N("p_br", "debug.print", {"text": "X2 barricade repaired to:", "chat": False}, 3); W("br", "hp", "p_br", "value"); W("br", "then", "p_br", "exec")
N("dl", "comp.on_drop_landed", {}, 4)
N("p_dl", "debug.print", {"text": "X2 drop landed:", "chat": False}, 5); W("dl", "drop", "p_dl", "value"); W("dl", "then", "p_dl", "exec")
nl()
N("ck", "creature.on_killed")
N("p_ck", "debug.print", {"text": "X2 creature killed by:", "chat": False}, 1); W("ck", "killer", "p_ck", "value"); W("ck", "then", "p_ck", "exec")

g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": "every_node_2", "name": "Every Node 2", "description": "Mode-building nodes"},
     "variables": [], "nodes": nodes, "edges": edges, "comments": []}
json.dump(g, open("every_node_2.graph.json", "w"), indent=1)
print(len(nodes), "nodes", len(edges), "wires")
