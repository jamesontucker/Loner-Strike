# Builds every_node.graph.json: one graph with every node in the registry, wired so it checks
# clean. Run from this folder: python3 make_every_node.py
import json

nodes, edges = [], []
row = [0]
def N(id, type, params=None, x=0):
    nodes.append({"id": id, "type": type, "pos": {"x": x * 260, "y": row[0] * 220}, "params": params or {}})
    return id
def W(a, ap, b, bp):
    edges.append({"id": f"w{len(edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
def chain(*steps):
    # steps: (node, outpin) pairs; each runs the next through its "then" (or the pin given)
    for (a, ap), (b, _) in zip(steps, steps[1:]):
        W(a, ap, b, "exec")
def nl():
    row[0] += 1

variables = [
    {"id": "kills", "name": "Kills", "type": "number", "scope": "player", "default": 0},
    {"id": "round", "name": "Round", "type": "number", "scope": "mode", "default": 1},
    {"id": "flag", "name": "Flag", "type": "bool", "scope": "mode", "default": False},
    {"id": "last", "name": "Last thing", "type": "text", "scope": "mode", "default": "none"},
]

# round started: round = round + 1, countdown, repeat, delay, waves, reset
N("rs", "event.round_started")
N("rs_get", "var.get", {"var": "round"}, 1)
N("rs_add1", "math.op", {"op": "add", "b": 1}, 1)
W("rs_get", "out", "rs_add1", "a")
N("rs_set", "var.set", {"var": "round"}, 2); W("rs_add1", "out", "rs_set", "value")
N("rs_cd", "timer.countdown", {"secs": 5, "text": "Go in", "slot": "top"}, 3)
N("rs_msg", "msg.broadcast", {"text": "Go!", "seconds": 2}, 4)
N("rs_rep", "timer.repeat", {"every": 10, "times": 3}, 4)
N("rs_alive", "game.count_alive", {"team": "none"}, 5)
N("rs_hud", "msg.hud", {"text": "Alive:", "slot": "right", "seconds": 5}, 5); W("rs_alive", "out", "rs_hud", "value")
N("rs_delay", "timer.delay", {"secs": 5}, 5)
N("rs_wave", "comp.wave_start", {"wave": 0}, 6)
N("rs_wstop", "comp.wave_stop", {}, 7)
N("rs_reset", "comp.reset", {}, 8)
chain(("rs", "then"), ("rs_set", "then"), ("rs_cd", "then"), ("rs_rep", "then"), ("rs_delay", None))
W("rs_cd", "done", "rs_msg", "exec")
W("rs_rep", "tick", "rs_hud", "exec")
chain(("rs_delay", "then"), ("rs_wave", "then"), ("rs_wstop", "then"), ("rs_reset", None))
nl()

# player killed: headshot branch, kills, team check, weapon, health
N("pk", "event.player_killed")
N("pk_re", "flow.reroute", {}, 1)
N("pk_br", "flow.branch", {}, 2); W("pk", "headshot", "pk_br", "cond")
N("pk_add", "var.add", {"var": "kills", "amount": 2}, 3); W("pk", "attacker", "pk_add", "player")
N("pk_rand", "math.random", {"min": 1, "max": 3, "frac": False}, 3)
N("pk_add2", "var.add", {"var": "kills"}, 3); W("pk", "attacker", "pk_add2", "player"); W("pk_rand", "out", "pk_add2", "amount")
N("pk_team", "cond.is_team", {"team": "ct"}, 4); W("pk", "attacker", "pk_team", "player")
N("pk_give", "player.give_weapon", {"weapon": "m4a1", "ammo": 60}, 5); W("pk", "attacker", "pk_give", "player")
N("pk_hp", "player.get_health", {}, 5); W("pk", "attacker", "pk_hp", "player")
N("pk_hp10", "math.op", {"op": "add", "b": 10}, 6); W("pk_hp", "out", "pk_hp10", "a")
N("pk_seth", "player.set_health", {}, 6); W("pk", "attacker", "pk_seth", "player"); W("pk_hp10", "out", "pk_seth", "amount")
N("pk_name", "player.get_name", {}, 5); W("pk", "victim", "pk_name", "player")
N("pk_hud", "msg.hud", {"text": "You got", "slot": "center"}, 6); W("pk", "attacker", "pk_hud", "player"); W("pk_name", "out", "pk_hud", "value")
W("pk", "then", "pk_re", "in"); W("pk_re", "out", "pk_br", "exec")
W("pk_br", "true", "pk_add", "exec"); W("pk_br", "false", "pk_add2", "exec")
W("pk_add", "then", "pk_team", "exec")
W("pk_team", "true", "pk_give", "exec"); W("pk_give", "then", "pk_seth", "exec")
W("pk_team", "false", "pk_hud", "exec")
nl()

# player spawned: strip, armor, speed, team, teleport, kills = 0
N("ps", "event.player_spawned")
N("ps_strip", "player.strip", {"knife": True}, 1)
N("ps_arm", "player.set_armor", {"amount": 50}, 2)
N("ps_spd", "player.set_speed", {"speed": 300}, 3)
N("ps_team", "player.set_team", {"team": "t"}, 4)
N("ps_tp", "player.teleport", {"name": "spawn_a"}, 5)
N("ps_set", "var.set", {"var": "kills", "value": 0}, 6)
for nid in ["ps_strip", "ps_arm", "ps_spd", "ps_team", "ps_tp", "ps_set"]:
    W("ps", "player", nid, "player")
chain(("ps", "then"), ("ps_strip", "then"), ("ps_arm", "then"), ("ps_spd", "then"), ("ps_team", "then"), ("ps_tp", "then"), ("ps_set", None))
nl()

# joined / left
N("pj", "event.player_joined")
N("pj_bot", "player.is_bot", {}, 1); W("pj", "player", "pj_bot", "player")
N("pj_hud", "msg.hud", {"text": "Bot:", "slot": "left"}, 2); W("pj", "player", "pj_hud", "player"); W("pj_bot", "out", "pj_hud", "value")
N("pj_alive", "player.is_alive", {}, 2); W("pj", "player", "pj_alive", "player")
N("pj_br", "flow.branch", {}, 3); W("pj_alive", "out", "pj_br", "cond")
W("pj", "then", "pj_hud", "exec"); W("pj_hud", "then", "pj_br", "exec")
N("pl", "event.player_left", {}, 4)
N("pl_set", "var.set", {"var": "last", "value": "someone left"}, 5)
W("pl", "then", "pl_set", "exec")
nl()

# bombs
N("bp", "event.bomb_planted")
N("bp_team", "player.get_team", {}, 1); W("bp", "player", "bp_team", "player")
N("bp_hud", "msg.hud", {"text": "Planted by", "slot": "center"}, 2); W("bp_team", "out", "bp_hud", "value")
W("bp", "then", "bp_hud", "exec")
N("bd", "event.bomb_defused", {}, 3)
N("bd_end", "score.end_round", {"winner": "ct"}, 4); W("bd", "then", "bd_end", "exec")
N("bx", "event.bomb_exploded", {}, 5)
N("bx_end", "score.end_round", {"winner": "t"}, 6); W("bx", "then", "bx_end", "exec")
nl()

# round ended: flag logic
N("re", "event.round_ended")
N("re_cnt", "game.count_alive", {}, 1); W("re", "winner", "re_cnt", "team")
N("re_cmp", "math.compare", {"op": "gt", "b": 0}, 2); W("re_cnt", "out", "re_cmp", "a")
N("re_flag", "var.get", {"var": "flag"}, 2)
N("re_not", "logic.not", {}, 3); W("re_flag", "out", "re_not", "a")
N("re_and", "logic.and_or", {"op": "and"}, 4); W("re_cmp", "out", "re_and", "a"); W("re_not", "out", "re_and", "b")
N("re_br", "flow.branch", {}, 5); W("re_and", "out", "re_br", "cond")
N("re_set", "var.set", {"var": "flag", "value": True}, 6)
N("re_stop", "timer.stop_all", {}, 6)
W("re", "then", "re_br", "exec"); W("re_br", "true", "re_set", "exec"); W("re_br", "false", "re_stop", "exec")
nl()

# used / touched
N("pu", "event.player_used", {"name": "door_button"})
N("pu_door", "comp.door", {"do": "open"}, 1); W("pu", "entity", "pu_door", "door")
N("pu_find", "comp.find", {"name": "trap1"}, 1)
N("pu_trap", "comp.trap_fire", {}, 2); W("pu_find", "out", "pu_trap", "trap")
chain(("pu", "then"), ("pu_door", "then"), ("pu_trap", None))
N("pt", "event.player_touched", {"name": ""}, 4)
N("pt_crate", "comp.crate_open", {}, 5); W("pt", "entity", "pt_crate", "container"); W("pt", "player", "pt_crate", "player")
W("pt", "then", "pt_crate", "exec")
nl()

# components
N("cc", "comp.on_capture")
N("cc_lock", "comp.capture_lock", {"secs": 20}, 1); W("cc", "point", "cc_lock", "point")
N("cc_hud", "msg.hud", {"text": "Taken by", "slot": "top"}, 2); W("cc", "team", "cc_hud", "value")
chain(("cc", "then"), ("cc_lock", "then"), ("cc_hud", None))
N("cl", "comp.on_capture_lost", {}, 4)
N("cl_find", "comp.find", {"name": "ex1"}, 5)
N("cl_ex", "comp.extract_open", {"open": True}, 5); W("cl_find", "out", "cl_ex", "point")
W("cl", "then", "cl_ex", "exec")
nl()
N("zs", "comp.on_zone_stage")
N("zs_r", "math.op", {"op": "mul", "b": 100}, 1); W("zs", "stage", "zs_r", "a")
N("zs_shrink", "comp.zone_shrink", {"at": "mid", "secs": 20}, 2); W("zs", "zone", "zs_shrink", "zone"); W("zs_r", "out", "zs_shrink", "radius")
W("zs", "then", "zs_shrink", "exec")
N("zt", "comp.on_zone_tick", {}, 4)
N("zt_set", "comp.zone_set", {"at": "mid", "radius": 900}, 5); W("zt", "zone", "zt_set", "zone")
N("zt_hud", "msg.hud", {"text": "Ouch", "slot": "center"}, 6); W("zt", "player", "zt_hud", "player"); W("zt", "damage", "zt_hud", "value")
chain(("zt", "then"), ("zt_set", "then"), ("zt_hud", None))
nl()
N("ex", "comp.on_extract")
N("ex_add", "var.add", {"var": "kills", "amount": 5}, 1); W("ex", "player", "ex_add", "player")
W("ex", "then", "ex_add", "exec")
N("ws", "comp.on_wave_start", {}, 3)
N("ws_hud", "msg.hud", {"text": "Wave", "slot": "top"}, 4); W("ws", "wave", "ws_hud", "value"); W("ws", "then", "ws_hud", "exec")
N("wp", "comp.on_wave_spawn", {}, 5)
N("wp_hud", "msg.hud", {"text": "In:", "slot": "right"}, 6); W("wp", "spawned", "wp_hud", "value"); W("wp", "then", "wp_hud", "exec")
nl()
N("wc", "comp.on_wave_cleared")
N("wc_next", "math.op", {"op": "add", "b": 1}, 1); W("wc", "wave", "wc_next", "a")
N("wc_start", "comp.wave_start", {}, 2); W("wc_next", "out", "wc_start", "wave"); W("wc", "then", "wc_start", "exec")
N("lo", "comp.on_loot", {}, 4)
N("lo_hud", "msg.hud", {"text": "Looted", "slot": "left"}, 5); W("lo", "player", "lo_hud", "player"); W("lo", "then", "lo_hud", "exec")
nl()
N("ck", "comp.on_checkpoint")
N("ck_set", "var.set", {"var": "round"}, 1); W("ck", "order", "ck_set", "value"); W("ck", "then", "ck_set", "exec")
N("bb", "comp.on_barricade_broken", {}, 3)
N("bb_msg", "msg.broadcast", {"text": "The barricade is down!"}, 4); W("bb", "then", "bb_msg", "exec")
N("me", "comp.on_meeting_end", {}, 5)
N("me_hud", "msg.hud", {"text": "Out:", "slot": "center"}, 6); W("me", "ejected", "me_hud", "value"); W("me", "then", "me_hud", "exec")
nl()

# creatures
N("ck2", "creature.on_killed")
N("ck2_set", "var.set", {"var": "last"}, 1); W("ck2", "kind", "ck2_set", "value")
N("ck2_make", "comp.create", {"kind": "ls_loot_container", "at": "crate_spot", "keys": "table=rifles radius=40"}, 2)
N("ck2_spawn", "creature.spawn", {"kind": "zombie", "at": "spot1"}, 3)
N("ck2_hud", "msg.hud", {"text": "New one:", "slot": "right"}, 4); W("ck2_spawn", "out", "ck2_hud", "value")
N("ck2_last", "var.get", {"var": "last"}, 4)
N("ck2_hud2", "msg.hud", {"text": "Killed:", "slot": "left"}, 5); W("ck2_last", "out", "ck2_hud2", "value"); W("ck2", "killer", "ck2_hud2", "player")
chain(("ck2", "then"), ("ck2_set", "then"), ("ck2_make", "then"), ("ck2_spawn", "then"), ("ck2_hud", "then"), ("ck2_hud2", None))

g = {"schema": "loner-strike/graph", "version": 2, "mode": {"id": "every_node", "name": "Every Node", "description": "Every node in the registry"},
     "variables": variables, "nodes": nodes, "edges": edges, "comments": []}
json.dump(g, open("every_node.graph.json", "w"), indent=1)
print(len(nodes), "nodes", len(edges), "wires")
