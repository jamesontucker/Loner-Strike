# Builds the Studio's five example modes (they ship as templates and as the community's seed
# modes): Infection, Gun Game Lite, King of the Hill, One in the Chamber, Juggernaut. Each is a
# small graph with comment boxes that explain each part, so it teaches the Studio as it's read.
# Run from this folder, then lay them out the way Tidy up does:
#   python3 make_examples.py [names] && (cd ../../../../ui && node scripts/tidy-studio-templates.mjs <the files>)
# Text in graphs: letters, digits, spaces and plain punctuation only (no slash, star or quotes).
import json, os

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../../../ui/src/views/tools/studio/templates")


class Mode:
    def __init__(s, mid, name, desc, uid, settings=()):
        s.g = {"schema": "loner-strike/graph", "version": 3,
               "mode": {"id": mid, "uid": uid, "name": name, "description": desc, "settings": [{"cvar": c, "value": v} for c, v in settings]},
               "variables": [], "nodes": [], "edges": [], "comments": []}
        s.row = -1
        s.col = 0
        s.graph = None

    def part(s, text, color=None):
        """A new comment box (a row of the graph) that the next nodes go into."""
        s.row += 1
        s.col = 0
        c = {"id": f"c{len(s.g['comments']) + 1}", "text": text, "pos": {"x": 0, "y": s.row * 700}, "size": {"w": 6000, "h": 660}}
        if color:
            c["color"] = color
        if s.graph:
            c["graph"] = s.graph
        s.g["comments"].append(c)

    def n(s, id, type, params=None, below=False):
        """A node, placed after the last one in this part (or under it)."""
        if below:
            s.col -= 1
        x = 60 + s.col * 300
        y = s.row * 700 + 80 + (220 if below else 0)
        s.col += 1
        node = {"id": id, "type": type, "pos": {"x": x, "y": y}, "params": params or {}}
        if s.graph:
            node["graph"] = s.graph
        s.g["nodes"].append(node)
        return id

    def w(s, a, b):
        """A wire: "node.pin" -> "node.pin"."""
        (an, ap), (bn, bp) = a.split("."), b.split(".")
        s.g["edges"].append({"id": f"w{len(s.g['edges']) + 1}", "from": {"node": an, "pin": ap}, "to": {"node": bn, "pin": bp}})

    def flow(s, *ids):
        """Flow wires along a chain. "a" means a's Then; "a.pin" picks a's output (written right after
        "a" it only picks the pin: "loop", "loop.each", "x" wires loop.each -> x)."""
        for a, b in zip(ids, ids[1:]):
            if "." in b and b.split(".")[0] == a.split(".")[0]:
                continue
            src = a if "." in a else f"{a}.then"
            node, _, pin = b.partition(".")
            s.w(src, f"{node}.{pin if pin in ('exec', 'reset', 'break', 'open', 'close', 'toggle') else 'exec'}")

    def var(s, id, name, type, scope, default=None, reset=None):
        v = {"id": id, "name": name, "type": type, "scope": scope}
        if default is not None:
            v["default"] = default
        if reset:
            v["reset"] = reset
        s.g["variables"].append(v)

    def func(s, id, name, inputs=(), outputs=()):
        s.g.setdefault("functions", []).append({"id": id, "name": name, "inputs": [{"id": i, "name": nm, "type": t} for i, nm, t in inputs], "outputs": [{"id": i, "name": nm, "type": t} for i, nm, t in outputs]})

    def save(s, file):
        with open(os.path.join(OUT, file), "w") as f:
            json.dump(s.g, f, indent=1)
            f.write("\n")


# ---- Infection ------------------------------------------------------------------------------
def infection():
    m = Mode("infection", "Infection", "One zombie; its victims turn; humans must outlast the clock",
             "6d1f0f3e-4c1b-4a7e-9a51-2b7e3c9d0a11",
             [("mp_round_infinite", 1), ("mp_autoteambalance", 0), ("mp_limitteams", 0)])
    m.func("infect", "Infect", inputs=[("who", "Who", "player")])
    m.var("started", "Infection Started", "bool", "mode", False, "round")

    m.part("Each round everyone starts human (Counter-Terrorist). After a 15 second countdown one random player turns. The humans have 3 minutes on the mode's own round clock.")
    m.n("rs", "event.round_started")
    m.n("all", "flow.for_each_player", {"which": "all"})
    m.n("human", "player.set_team", {"team": "ct"})
    m.n("look", "player.set_glow", {"color": "off"}, below=True)
    m.n("cd", "timer.countdown", {"secs": 15, "text": "The infection starts in", "slot": "top"})
    m.n("clock", "clock.set", {"secs": 180})
    m.n("pick", "flow.pick_player", {"which": "alive"})
    m.n("first", "fn.call", {"func": "infect"})
    m.n("on", "var.set", {"var": "started", "value": True})
    m.n("tell", "msg.broadcast", {"text": "Run! The infection has started.", "seconds": 4})
    m.flow("rs", "all", "all.each", "human", "look")
    m.flow("all.then", "cd", "cd.done", "clock", "pick", "first", "on", "tell")
    m.w("all.player", "human.player")
    m.w("all.player", "look.player")
    m.w("pick.player", "first.who")

    m.part("A human killed by a zombie turns: they come back a second later on the zombie team. A zombie killed by a human comes back after 5 seconds. Once the infection has started, nobody stays dead.")
    m.n("pk", "event.player_killed")
    m.n("gate", "flow.branch")
    m.n("getstarted", "var.get", {"var": "started"}, below=True)
    m.n("was_human", "cond.is_team", {"team": "ct"})
    m.n("turn", "player.set_team", {"team": "t"})
    m.n("say", "msg.chat", {"text": "Infected:"})
    m.n("wait1", "timer.delay", {"secs": 1})
    m.n("back1", "player.respawn")
    m.n("inf", "fn.call", {"func": "infect"})
    m.n("wait5", "timer.delay", {"secs": 5})
    m.n("back5", "player.respawn")
    m.n("inf2", "fn.call", {"func": "infect"})
    m.w("getstarted.out", "gate.cond")
    m.flow("pk", "gate", "gate.true", "was_human", "was_human.true", "turn", "say", "wait1", "back1", "inf")
    m.flow("was_human.false", "wait5", "back5", "inf2")
    for t in ("was_human.player", "turn.player", "say.value", "back1.player", "inf.who", "back5.player", "inf2.who"):
        m.w("pk.victim", t)

    m.part("The round ends when no human is left (the zombies win) or when the clock runs out (the humans win). Checked after every death. Infection Started goes off as the round ends, so it can only end once.")
    m.n("pk2", "event.player_killed")
    m.n("left", "game.count_alive", {"team": "ct"}, below=True)
    m.n("none", "math.compare", {"op": "eq", "b": 0}, below=True)
    m.n("going", "var.get", {"var": "started"}, below=True)
    m.n("both", "logic.and_or", {"op": "and"}, below=True)
    m.n("check", "flow.branch")
    m.n("off1", "var.set", {"var": "started", "value": False})
    m.n("zwin", "msg.broadcast", {"text": "The zombies took everyone!", "seconds": 5})
    m.n("zend", "score.end_round", {"winner": "t"})
    m.n("up", "event.time_up")
    m.n("off2", "var.set", {"var": "started", "value": False})
    m.n("hwin", "msg.broadcast", {"text": "The humans survived!", "seconds": 5})
    m.n("hend", "score.end_round", {"winner": "ct"})
    m.w("left.out", "none.a")
    m.w("none.out", "both.a")
    m.w("going.out", "both.b")
    m.w("both.out", "check.cond")
    m.flow("pk2", "check", "check.true", "off1", "zwin", "zend")
    m.flow("up", "off2", "hwin", "hend")

    m.part("Zombies keep to their claws: one that picks up a gun and fires it loses it again. Weapon Fired runs often, so it does as little as it can.")
    m.n("fired", "event.weapon_fired")
    m.n("zombie", "cond.is_team", {"team": "t"})
    m.n("knife", "text.equals", {"b": "knife"}, below=True)
    m.n("isknife", "flow.branch")
    m.n("strip", "player.strip", {"knife": True})
    m.w("fired.player", "zombie.player")
    m.w("fired.weapon", "knife.a")
    m.w("knife.out", "isknife.cond")
    m.w("fired.player", "strip.player")
    m.flow("fired", "zombie", "zombie.true", "isknife", "isknife.false", "strip")

    m.graph = "infect"
    m.part("Infect: what makes a zombie. Called for the first zombie and for everyone who turns. Strong and quick, knife only, glowing green.")
    m.n("fe", "fn.entry")
    m.n("zt", "player.set_team", {"team": "t"})
    m.n("zs", "player.strip", {"knife": True})
    m.n("zh", "player.set_health", {"amount": 1500})
    m.n("zv", "player.set_speed", {"speed": 290})
    m.n("zg", "player.set_glow", {"color": "green"})
    m.n("zm", "msg.hud", {"text": "You're a zombie. Infect the humans!", "slot": "center", "seconds": 4})
    m.flow("fe", "zt", "zs", "zh", "zv", "zg", "zm")
    for t in ("zt", "zs", "zh", "zv", "zg", "zm"):
        m.w("fe.who", f"{t}.player")
    m.graph = None
    m.save("infection.graph.json")


# ---- Gun Game Lite --------------------------------------------------------------------------
GUNS = ["mp5navy", "p90", "m3", "famas", "ak47", "m4a1", "awp", "deagle"]

def gungame():
    m = Mode("gun_game_lite", "Gun Game Lite", "Each kill gives the next gun; a knife kill at the end wins",
             "2a8c4f61-0d3e-4b9a-8f27-5c1e7d3b9a42",
             [("mp_freeforall", 1), ("mp_round_infinite", 1)])
    m.func("arm", "Give Level Gun", inputs=[("who", "Who", "player")])
    m.var("level", "Level", "number", "player", 0)

    m.part("Free for all, no buying, and you come back 2 seconds after you die, with the gun of your level.")
    m.n("rs", "event.round_started")
    m.n("nobuy", "buy.where", {"where": "nowhere"})
    m.n("bots", "bots.no_buy", {"on": True})
    m.n("hello", "msg.broadcast", {"text": "Gun Game: every kill gives you the next gun. A knife kill at the end wins.", "seconds": 6})
    m.flow("rs", "nobuy", "bots", "hello")
    m.n("ps", "event.player_spawned")
    m.n("give", "fn.call", {"func": "arm"})
    m.w("ps.player", "give.who")
    m.flow("ps", "give")
    m.n("pk0", "event.player_killed")
    m.n("wait", "timer.delay", {"secs": 2})
    m.n("back", "player.respawn")
    m.w("pk0.victim", "back.player")
    m.flow("pk0", "wait", "back")

    m.part(f"A kill by another player moves the killer up a level and hands over the next gun at once. Level {len(GUNS)} is the knife, and a kill with it wins. A suicide costs a level.")
    m.n("pk", "event.player_killed")
    m.n("self", "flow.branch")
    m.n("bad", "logic.and_or", {"op": "or"}, below=True)
    m.n("down", "var.add", {"var": "level", "amount": -1})
    m.n("floor", "math.clamp", {"min": 0, "max": len(GUNS)}, below=True)
    m.n("lvl0", "var.get", {"var": "level"}, below=True)
    m.n("fix", "var.set", {"var": "level"})
    m.n("up", "var.add", {"var": "level", "amount": 1})
    m.n("lvl", "var.get", {"var": "level"}, below=True)
    m.n("won", "math.compare", {"op": "gt", "b": len(GUNS)}, below=True)
    m.n("end", "flow.branch")
    m.n("next", "fn.call", {"func": "arm"})
    m.n("hud", "msg.hud", {"text": "Level", "slot": "left", "seconds": 3})
    m.w("pk.suicide", "bad.a")
    m.w("pk.world", "bad.b")
    m.w("bad.out", "self.cond")
    m.flow("pk", "self", "self.true", "down", "fix")
    m.w("pk.victim", "down.player")
    m.w("pk.victim", "lvl0.player")
    m.w("lvl0.out", "floor.a")
    m.w("floor.out", "fix.value")
    m.w("pk.victim", "fix.player")
    m.flow("self.false", "up", "end")
    m.w("pk.attacker", "up.player")
    m.w("pk.attacker", "lvl.player")
    m.w("lvl.out", "won.a")
    m.w("won.out", "end.cond")
    m.flow("end.false", "next", "hud")
    m.w("pk.attacker", "next.who")
    m.w("pk.attacker", "hud.player")
    m.w("lvl.out", "hud.value")

    m.part("Someone made it through every gun: everyone hears who, levels go back to 0 and a new round starts.")
    m.n("win_r", "flow.reroute")
    m.n("cheer", "msg.chat", {"text": "Gun Game won by"})
    m.n("big", "msg.broadcast", {"text": "We have a winner! New game in 5 seconds.", "seconds": 5})
    m.n("reset", "flow.for_each_player", {"which": "all"})
    m.n("zero", "var.set", {"var": "level", "value": 0})
    m.n("pause", "timer.delay", {"secs": 5})
    m.n("again", "game.restart_round")
    m.w("end.true", "win_r.in")
    m.w("win_r.out", "cheer.exec")
    m.w("pk.attacker", "cheer.value")
    m.flow("cheer", "big", "reset", "reset.each", "zero")
    m.w("reset.player", "zero.player")
    m.flow("reset.then", "pause", "again")

    m.graph = "arm"
    m.part(f"Give Level Gun: takes everything but the knife and gives the gun for the player's level ({', '.join(GUNS)}, then the knife).")
    m.n("fe", "fn.entry")
    m.n("strip", "player.strip", {"knife": True})
    m.n("lv", "var.get", {"var": "level"}, below=True)
    m.n("sw", "flow.switch_num", {"cases": ",".join(str(i) for i in range(len(GUNS)))})
    m.w("fe.who", "strip.player")
    m.w("fe.who", "lv.player")
    m.w("lv.out", "sw.in")
    m.flow("fe", "strip", "sw")
    for i, gun in enumerate(GUNS):
        gid = m.n(f"g{i}", "player.give_weapon", {"weapon": gun, "ammo": 200}, below=i % 2 == 1)
        m.w(f"sw.c{i}", f"{gid}.exec")
        m.w("fe.who", f"{gid}.player")
    m.n("kn", "msg.hud", {"text": "Knife round! One knife kill wins.", "slot": "center", "seconds": 4})
    m.w("sw.default", "kn.exec")
    m.w("fe.who", "kn.player")
    m.graph = None
    m.save("gun-game-lite.graph.json")


# ---- King of the Hill -----------------------------------------------------------------------
def koth():
    m = Mode("king_of_the_hill", "King of the Hill", "Hold the hill to score; the first team to 120 wins",
             "9b3e5d27-7f1a-4c08-b6e2-4a9d8c1f5e73",
             [("mp_round_infinite", 1)])
    m.g["markers"] = [{"id": "hill", "label": "Hill", "description": "Where the hill is: a capture point is made here.", "type": "point", "min": 0, "max": 1, "colour": "#ffb347", "icon": "flag"}]
    m.func("hill", "Make Hill", inputs=[("at", "At", "entity")])
    m.var("point", "Hill", "entity", "mode")
    m.var("holder", "Held By", "team", "mode")
    m.var("held", "Is Held", "bool", "mode", False, "round")
    m.var("score", "Hill Score", "number", "team", 0, "round")

    m.part("Each round a capture point is made on the hill: the map's Hill marker, else bomb site A, else where the hostages are. Bots go for it by themselves.")
    m.n("rs", "event.round_started")
    m.n("no", "var.set", {"var": "held", "value": False})
    m.n("mk", "marker.find", {"marker": "hill", "n": 1}, below=True)
    m.n("has", "flow.branch")
    m.n("onmk", "fn.call", {"func": "hill"})
    m.n("site", "flow.pick_entity", {"by": "class", "name": "func_bomb_target"})
    m.n("onsite", "fn.call", {"func": "hill"})
    m.n("hostage", "flow.pick_entity", {"by": "class", "name": "hostage_entity"})
    m.n("onhost", "fn.call", {"func": "hill"})
    m.n("nohill", "msg.chat", {"text": "This map has no hill. Add a Hill marker in the map editor."})
    m.flow("rs", "no", "has", "has.true", "onmk")
    m.w("mk.found", "has.cond")
    m.w("mk.out", "onmk.at")
    m.flow("has.false", "site", "onsite")
    m.w("site.entity", "onsite.at")
    m.flow("site.none", "hostage", "onhost")
    m.w("hostage.entity", "onhost.at")
    m.flow("hostage.none", "nohill")
    m.n("rs3", "event.round_started")
    m.n("tick", "timer.repeat", {"every": 1, "times": 0})
    m.flow("rs3", "tick")

    m.part("Who holds the hill: Point Captured says which team took it, Point Lost that nobody has it now.")
    m.n("cap", "comp.on_capture")
    m.n("set", "var.set", {"var": "holder"})
    m.n("yes", "var.set", {"var": "held", "value": True})
    m.n("news", "msg.broadcast", {"text": "The hill has a new king!", "seconds": 3})
    m.n("lost", "comp.on_capture_lost")
    m.n("clear", "var.set", {"var": "held", "value": False})
    m.w("cap.team", "set.value")
    m.flow("cap", "set", "yes", "news")
    m.flow("lost", "clear")

    m.part("Every second the team on the hill scores a point (Hill Score is kept per team) and everyone sees it. 120 wins the round.")
    m.n("t_r", "flow.reroute")
    m.n("isheld", "flow.branch")
    m.n("h", "var.get", {"var": "held"}, below=True)
    m.n("add", "var.add", {"var": "score", "amount": 1})
    m.n("who", "var.get", {"var": "holder"}, below=True)
    m.n("show", "msg.hud", {"text": "", "slot": "top", "seconds": 1.2})
    m.n("now", "var.get", {"var": "score"}, below=True)
    m.n("line", "text.format", {"text": "The hill is held by [0]: [1] of 120"}, below=True)
    m.n("win", "flow.branch")
    m.n("done", "math.compare", {"op": "ge", "b": 120}, below=True)
    m.n("stop", "timer.stop_all")
    m.n("crown", "msg.broadcast", {"text": "King of the Hill! The round is won.", "seconds": 5})
    m.n("end", "score.end_round")
    m.w("tick.tick", "t_r.in")
    m.w("t_r.out", "isheld.exec")
    m.w("h.out", "isheld.cond")
    m.w("who.out", "add.team")
    m.w("who.out", "now.team")
    m.flow("isheld.true", "add", "show", "win")
    m.w("who.out", "line.a0")
    m.w("now.out", "line.a1")
    m.w("line.out", "show.value")
    m.w("now.out", "done.a")
    m.w("done.out", "win.cond")
    m.flow("win.true", "stop", "crown", "end")
    m.w("who.out", "end.winner")

    m.graph = "hill"
    m.part("Make Hill: a capture point at the place given, kept in the Hill variable. Taking it alone takes 6 seconds, faster with friends.")
    m.n("fe", "fn.entry")
    m.n("make", "comp.create", {"kind": "ls_capture_point", "keys": "radius=220 capture_secs=6"})
    m.n("keep", "var.set", {"var": "point"})
    m.w("fe.at", "make.at_ent")
    m.w("make.out", "keep.value")
    m.flow("fe", "make", "keep")
    m.graph = None
    m.save("king-of-the-hill.graph.json")


# ---- One in the Chamber ---------------------------------------------------------------------
def oitc():
    m = Mode("one_in_the_chamber", "One in the Chamber", "One bullet, a kill gives one more, three lives",
             "4e7a1c93-2b5d-4f60-a8c4-7d2e9b0f6a35",
             [("mp_freeforall", 1), ("mp_round_infinite", 1)])
    m.var("lives", "Lives", "number", "player", 3, "round")
    m.var("playing", "Still In", "number", "mode", 0)

    m.part("Free for all, no buying. Everyone spawns with a knife and a deagle holding a single bullet.")
    m.n("rs", "event.round_started")
    m.n("nobuy", "buy.where", {"where": "nowhere"})
    m.n("bots", "bots.no_buy", {"on": True})
    m.n("hello", "msg.broadcast", {"text": "One in the Chamber: one bullet, one kill, one more bullet. Three lives.", "seconds": 6})
    m.flow("rs", "nobuy", "bots", "hello")
    m.n("ps", "event.player_spawned")
    m.n("strip", "player.strip", {"knife": True})
    m.n("give", "player.give_weapon", {"weapon": "deagle", "ammo": 0})
    m.n("hold", "player.switch_weapon", {"weapon": "deagle"})
    m.n("one", "player.set_ammo", {"clip": 1, "reserve": 0})
    m.flow("ps", "strip", "give", "hold", "one")
    for t in ("strip", "give", "hold", "one"):
        m.w("ps.player", f"{t}.player")

    m.part("A deagle bullet always kills (the knife hurts as usual). Player About To Be Hurt can change the damage before it lands.")
    m.n("ah", "event.player_about_hurt")
    m.n("gun", "player.weapon_held", {}, below=True)
    m.n("isdeagle", "text.equals", {"b": "deagle"}, below=True)
    m.n("check", "flow.branch")
    m.n("kill", "dmg.set", {"amount": 400})
    m.w("ah.attacker", "gun.player")
    m.w("gun.out", "isdeagle.a")
    m.w("isdeagle.out", "check.cond")
    m.flow("ah", "check", "check.true", "kill")

    m.part("A kill gives the killer one more bullet. The one who died loses a life, and comes back after 2 seconds if they have any left.")
    m.n("pk", "event.player_killed")
    m.n("byplayer", "flow.branch")
    m.n("other", "logic.and_or", {"op": "or"}, below=True)
    m.n("notother", "logic.not", {}, below=True)
    m.n("swap", "player.switch_weapon", {"weapon": "deagle"})
    m.n("bullet", "player.give_ammo", {"n": 1})
    m.n("lose", "var.add", {"var": "lives", "amount": -1})
    m.n("left", "var.get", {"var": "lives"}, below=True)
    m.n("any", "math.compare", {"op": "gt", "b": 0}, below=True)
    m.n("more", "flow.branch")
    m.n("wait", "timer.delay", {"secs": 2})
    m.n("back", "player.respawn")
    m.n("out", "msg.chat", {"text": "Out of lives:"})
    m.w("pk.suicide", "other.a")
    m.w("pk.world", "other.b")
    m.w("other.out", "notother.a")
    m.w("notother.out", "byplayer.cond")
    m.flow("pk", "byplayer", "byplayer.true", "swap", "bullet", "lose")
    m.flow("byplayer.false", "lose")
    m.w("pk.attacker", "swap.player")
    m.w("pk.attacker", "bullet.player")
    m.w("pk.victim", "lose.player")
    m.w("pk.victim", "left.player")
    m.w("left.out", "any.a")
    m.w("any.out", "more.cond")
    m.flow("lose", "more", "more.true", "wait", "back")
    m.w("pk.victim", "back.player")
    m.flow("more.false", "out")
    m.w("pk.victim", "out.value")

    m.part("After each death, count who still has lives. When only one is left, they win and a new round starts with three lives each.")
    m.n("pk2", "event.player_killed")
    m.n("zero", "var.set", {"var": "playing", "value": 0})
    m.n("each", "flow.for_each_player", {"which": "all"})
    m.n("l2", "var.get", {"var": "lives"}, below=True)
    m.n("has", "math.compare", {"op": "gt", "b": 0}, below=True)
    m.n("in", "flow.branch")
    m.n("count", "var.add", {"var": "playing", "amount": 1})
    m.n("n", "var.get", {"var": "playing"}, below=True)
    m.n("last", "math.compare", {"op": "le", "b": 1}, below=True)
    m.n("over", "flow.branch")
    m.n("winner", "flow.pick_player", {"which": "alive"})
    m.n("cheer", "msg.chat", {"text": "Last one standing:"})
    m.n("end", "score.end_round", {"winner": "draw"})
    m.flow("pk2", "zero", "each", "each.each", "in", "in.true", "count")
    m.w("each.player", "l2.player")
    m.w("l2.out", "has.a")
    m.w("has.out", "in.cond")
    m.flow("each.then", "over", "over.true", "winner", "cheer", "end")
    m.w("n.out", "last.a")
    m.w("last.out", "over.cond")
    m.w("winner.player", "cheer.value")
    m.save("one-in-the-chamber.graph.json")


# ---- Juggernaut -----------------------------------------------------------------------------
def juggernaut():
    m = Mode("juggernaut", "Juggernaut", "One big, slow, tough player; whoever kills them becomes it",
             "c5d2a8e4-1f6b-4d3a-9e70-8b4f2c6a1d59",
             [("mp_freeforall", 1), ("mp_round_infinite", 1)])
    m.func("crown", "Make Juggernaut", inputs=[("who", "Who", "player")])
    m.var("jugg", "Juggernaut", "player", "mode")

    m.part("Free for all with respawns. 5 seconds into each round a random player becomes the Juggernaut.")
    m.n("rs", "event.round_started")
    m.n("hello", "msg.broadcast", {"text": "Juggernaut: kill the big one to become it. 20 points win.", "seconds": 6})
    m.n("wait", "timer.delay", {"secs": 5})
    m.n("pick", "flow.pick_player", {"which": "alive"})
    m.n("first", "fn.call", {"func": "crown"})
    m.flow("rs", "hello", "wait", "pick", "first")
    m.w("pick.player", "first.who")
    m.n("ps", "event.player_spawned")
    m.n("plain", "player.set_glow", {"color": "off"})
    m.w("ps.player", "plain.player")
    m.flow("ps", "plain")

    m.part("Whoever kills the Juggernaut becomes it and scores 2 more. If the Juggernaut died some other way, a random player gets it. Everyone comes back after 3 seconds.")
    m.n("pk", "event.player_killed")
    m.n("cur", "var.get", {"var": "jugg"}, below=True)
    m.n("same", "logic.equal", {"of": "player"}, below=True)
    m.n("wasit", "flow.branch")
    m.n("ok", "flow.branch")
    m.n("fair", "logic.and_or", {"op": "or"}, below=True)
    m.n("notfair", "logic.not", {}, below=True)
    m.n("bonus", "player.add_frags", {"amount": 2})
    m.n("next", "fn.call", {"func": "crown"})
    m.n("rand", "flow.pick_player", {"which": "alive"})
    m.n("next2", "fn.call", {"func": "crown"})
    m.n("wait3", "timer.delay", {"secs": 3})
    m.n("back", "player.respawn")
    m.w("pk.victim", "same.a")
    m.w("cur.out", "same.b")
    m.w("same.out", "wasit.cond")
    m.w("pk.suicide", "fair.a")
    m.w("pk.world", "fair.b")
    m.w("fair.out", "notfair.a")
    m.w("notfair.out", "ok.cond")
    m.flow("pk", "wasit", "wasit.true", "ok", "ok.true", "bonus", "next")
    m.w("pk.attacker", "bonus.player")
    m.w("pk.attacker", "next.who")
    m.flow("ok.false", "rand", "next2")
    m.w("rand.player", "next2.who")
    m.w("wasit.false", "wait3.exec")
    m.flow("next", "wait3")
    m.flow("next2", "wait3.exec")
    m.flow("wait3", "back")
    m.w("pk.victim", "back.player")

    m.part("20 points (kills, and 2 more for killing the Juggernaut) win the round. Scores start again next round.")
    m.n("pk2", "event.player_killed")
    m.n("sc", "player.get_frags", {}, below=True)
    m.n("enough", "math.compare", {"op": "ge", "b": 20}, below=True)
    m.n("won", "flow.branch")
    m.n("cheer", "msg.chat", {"text": "Juggernaut won by"})
    m.n("end", "score.end_round", {"winner": "draw"})
    m.n("nr", "event.new_round")
    m.n("all", "flow.for_each_player", {"which": "all"})
    m.n("zero", "player.set_frags", {"n": 0})
    m.w("pk2.attacker", "sc.player")
    m.w("sc.out", "enough.a")
    m.w("enough.out", "won.cond")
    m.flow("pk2", "won", "won.true", "cheer", "end")
    m.w("pk2.attacker", "cheer.value")
    m.flow("nr", "all", "all.each", "zero")
    m.w("all.player", "zero.player")

    m.part("Bots hunt the Juggernaut: every 10 seconds each bot is sent after them (it still fights whoever it meets).")
    m.n("rs2", "event.round_started")
    m.n("hunt", "timer.repeat", {"every": 10, "times": 0})
    m.n("bots", "flow.for_each_player", {"which": "bots"})
    m.n("j", "var.get", {"var": "jugg"}, below=True)
    m.n("follow", "bot.follow", {"secs": 9})
    m.flow("rs2", "hunt")
    m.w("hunt.tick", "bots.exec")
    m.flow("bots.each", "follow")
    m.w("bots.player", "follow.bot")
    m.w("j.out", "follow.who")

    m.graph = "crown"
    m.part("Make Juggernaut: lots of health, slower, a machine gun and a red glow, and everyone is told.")
    m.n("fe", "fn.entry")
    m.n("set", "var.set", {"var": "jugg"})
    m.n("hp", "player.set_health", {"amount": 1000})
    m.n("armor", "player.give_item", {"item": "vesthelm"})
    m.n("gun", "player.give_weapon", {"weapon": "m249", "ammo": 200})
    m.n("slow", "player.set_speed", {"speed": 200})
    m.n("glow", "player.set_glow", {"color": "red"})
    m.n("tell", "msg.chat", {"text": "The Juggernaut is"})
    m.w("fe.who", "set.value")
    m.flow("fe", "set", "hp", "armor", "gun", "slow", "glow", "tell")
    for t in ("hp", "armor", "gun", "slow", "glow"):
        m.w("fe.who", f"{t}.player")
    m.w("fe.who", "tell.value")
    m.graph = None
    m.save("juggernaut.graph.json")


if __name__ == "__main__":
    # python3 make_examples.py [infection gungame koth oitc juggernaut]: only those (default all)
    import sys
    every = {"infection": infection, "gungame": gungame, "koth": koth, "oitc": oitc, "juggernaut": juggernaut}
    want = sys.argv[1:] or list(every)
    for k in want:
        every[k]()
    print("wrote", ", ".join(want))
