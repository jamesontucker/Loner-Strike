# The Starter library: premade Functions and Macros that ship with the app (My library lists
# them as built in; the node search lists them too). Run from this folder:
#   python3 make_starter.py
# writes ../../../../ui/src/views/tools/studio/library/st_*.lib.json (short names: an item's id,
# builtin_<name>, is at most 24 letters), the test mode that uses
# every one (testdata/starter_library_a/_b.graph.json: codegen, amxxpc and the sim) and
# builtin_library.rs (the app's own copy, for /api/tools/studio/library and MCP).
#
# A library item holds no events, so each premade is the part a mode calls from its own event
# (the help says which). Inputs left at 0 or empty use the default the help names. Stock files
# only (sprites and sounds every CS 1.6 and Condition Zero install has).
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
UI = os.path.join(HERE, "../../../../ui/src/views/tools/studio")
OUT = os.path.join(UI, "library")
NODES = {n["type"]: n for n in json.load(open(os.path.join(UI, "nodes.json")))["nodes"]}
# --committed: only premades whose nodes are all in the committed nodes.json (other agents' node
# batches still in progress leave theirs for the next run); the others are listed as waiting
if "--committed" in sys.argv:
    import subprocess
    head = subprocess.run(["git", "show", "HEAD:app/ui/src/views/tools/studio/nodes.json"], cwd=HERE, capture_output=True, text=True, check=True).stdout
    NODES_OK = {n["type"] for n in json.loads(head)["nodes"]}
else:
    NODES_OK = set(NODES)


class Item:
    def __init__(self, file, name, desc, category, help):
        self.file, self.name, self.desc, self.category, self.help = file, name, desc, category, help
        self.functions, self.macros, self.variables, self.nodes, self.edges, self.comments = [], [], [], [], [], []
        self.graph = None  # the Function or Macro the next nodes go on
        self.x = 0
        self.kind = self.root = None
        self.pins = {}  # node id -> (inputs, outputs) for checking wires
        self.missing = set()  # node types not committed yet (--committed)

    # ---- parts -------------------------------------------------------------------------------
    def function(self, id, name, inputs=(), outputs=(), pure=False, root=False):
        f = {"id": id, "name": name, "inputs": [{"id": i, "name": n, "type": t} for i, n, t in inputs], "outputs": [{"id": i, "name": n, "type": t} for i, n, t in outputs]}
        if pure:
            f["pure"] = True
        self.functions.append(f)
        self.graph, self.x = id, 0
        if root or self.root is None:
            self.kind, self.root = "function", id
        e = self.N("entry", "fn.entry")
        self.pins[e] = (set(), {"then"} | {i for i, _, _ in inputs})
        return e

    def ret(self, row=0):
        f = next(f for f in self.functions if f["id"] == self.graph)
        r = self.N("ret", "fn.return", row=row)
        self.pins[r] = ({"exec"} | {o["id"] for o in f["outputs"]}, set())
        return r

    def macro(self, id, name, inputs=(), outputs=(), root=False):
        m = {"id": id, "name": name, "inputs": [{"id": i, "name": n, "type": t} for i, n, t in inputs], "outputs": [{"id": i, "name": n, "type": t} for i, n, t in outputs]}
        self.macros.append(m)
        self.graph, self.x = id, 0
        if root or self.root is None:
            self.kind, self.root = "macro", id
        e = self.N("in", "macro.entry")
        self.pins[e] = (set(), {i for i, _, _ in inputs})
        x = self.N("out", "macro.exit", x=4000)
        self.pins[x] = ({o for o, _, _ in outputs}, set())
        return e, x

    def var(self, id, name, type, scope="mode", default=None):
        v = {"id": id, "name": name, "type": type, "scope": scope}
        if default is not None:
            v["default"] = default
        self.variables.append(v)
        return id

    def note(self, text, w=1200):
        self.comments.append({"id": f"c{len(self.comments) + 1}", "graph": self.graph, "pos": {"x": -40, "y": -260}, "size": {"w": w, "h": 120}, "text": text})

    # ---- nodes and wires ---------------------------------------------------------------------
    def N(self, id, type, row=0, x=None, **params):
        if type not in NODES_OK:
            self.missing.add(type)
        assert type in NODES, f"{self.file}: no node {type}"
        nid = f"{self.graph}_{id}"
        assert all(n["id"] != nid for n in self.nodes), nid
        if x is None:
            self.x += 260
            x = self.x
        self.nodes.append({"id": nid, "type": type, "pos": {"x": x, "y": row * 180}, "params": params, "graph": self.graph})
        d = NODES[type]
        for p in params:
            assert any(q["id"] == p for q in d.get("params", [])), f"{self.file}: {type} has no setting {p}"
        if type not in ("fn.entry", "fn.return", "macro.entry", "macro.exit"):
            self.pins[nid] = ({i["id"] for i in d.get("inputs", [])}, {o["id"] for o in d.get("outputs", [])})
        return nid

    def W(self, a, ap, b, bp):
        a = a if a.startswith(self.graph + "_") else f"{self.graph}_{a}"
        b = b if b.startswith(self.graph + "_") else f"{self.graph}_{b}"
        ins, outs = self.pins.get(a, (set(), set()))[1], self.pins.get(b, (set(), set()))[0]
        dyn = lambda t, p: t in ("flow.sequence", "flow.switch_num", "flow.switch_text", "fn.call", "macro.use", "text.format") or p.startswith("then") or p == "default"
        ta = next(n["type"] for n in self.nodes if n["id"] == a)
        tb = next(n["type"] for n in self.nodes if n["id"] == b)
        assert ap in ins or dyn(ta, ap), f"{self.file}: {a} ({ta}) has no output {ap}"
        assert bp in outs or dyn(tb, bp), f"{self.file}: {b} ({tb}) has no input {bp}"
        self.edges.append({"id": f"w{len(self.edges) + 1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

    def chain(self, *pairs):
        """exec wires: (node, out pin) -> next node's exec"""
        for (a, ap), b in zip(pairs, pairs[1:]):
            self.W(a, ap, b[0] if isinstance(b, tuple) else b, "exec")

    def num(self, id, value, row=3):
        return self.N(id, "math.number", row=row, value=value)

    def default(self, id, src, pin, value, row=3):
        """src.pin when it's above 0, else value: (node, "out")"""
        c = self.N(f"{id}_set", "math.compare", row=row, op="gt", b=0)
        self.W(src, pin, c, "a")
        d = self.num(f"{id}_d", value, row=row + 1)
        s = self.N(id, "logic.select", row=row, of="number")
        self.W(c, "out", s, "cond")
        self.W(src, pin, s, "a")
        self.W(d, "out", s, "b")
        return s

    def get(self, id, var, player_from=None, row=2):
        g = self.N(id, "var.get", row=row, var=var)
        if player_from:
            self.W(player_from[0], player_from[1], g, "player")
        return g

    # ---- out ---------------------------------------------------------------------------------
    def json(self):
        return {
            "schema": "loner-strike/graph-library", "version": 1, "name": self.name, "description": self.desc, "category": self.category,
            "kind": self.kind, "root": self.root, "enums": [], "structs": [], "functions": self.functions, "macros": self.macros,
            "variables": self.variables, "nodes": self.nodes, "edges": self.edges, "comments": self.help_comment() + self.comments,
        }

    def help_comment(self):
        # the help, as a comment box over the root graph (what it's for, what to wire in, what bots do)
        return [{"id": "help", "graph": self.root, "pos": {"x": -40, "y": -420}, "size": {"w": 1400, "h": 120}, "text": f"{self.name}: {self.help}"}]


ITEMS = []


def item(*a):
    it = Item(*a)
    ITEMS.append(it)
    return it


# =========================================================================================
# Movement
# =========================================================================================

# ---- Jump Pad ---------------------------------------------------------------------------
it = item("st_jump_pad", "Jump Pad", "Launches a player up (Speed, 0: 800) with a sound: call it from a pad's Begin Overlap.", "Starter: movement",
          "Make Jump Pad puts a trigger volume (named jumppad) at an entity; wire Begin Overlap (name jumppad) into Launch From Jump Pad. Bots that walk on it fly too.")
e = it.function("jp_make", "Make Jump Pad", [("at", "At", "entity")])
v = it.N("vol", "vol.spawn", shape="sphere", radius=96, name="jumppad")
it.W(e, "then", v, "exec")
pos = it.N("pos", "vec.entity", row=2); it.W(e, "at", pos, "in"); it.W(pos, "out", v, "pos")
mk = it.N("mark", "fx.marker", sprite="sprites/arrow1.spr", height=8, scale=0.4, r=80, g=200, b=255, pulse=True)
it.W(v, "then", mk, "exec"); it.W(v, "volume", mk, "follow")
r = it.ret(); it.W(mk, "then", r, "exec")
e = it.function("jp_launch", "Launch From Jump Pad", [("who", "Who", "entity"), ("speed", "Speed", "number")], root=True)
as_p = it.N("asp", "cast.player"); it.W(e, "then", as_p, "exec"); it.W(e, "who", as_p, "in")
sp = it.default("speed", e, "speed", 800)
up = it.N("up", "player.launch"); it.W(as_p, "ok", up, "exec"); it.W(as_p, "out", up, "player"); it.W(sp, "out", up, "speed")
snd = it.N("snd", "fx.sound_to", sound="items/gunpickup2.wav"); it.W(up, "then", snd, "exec"); it.W(as_p, "out", snd, "player")
r = it.ret(); it.W(snd, "then", r, "exec"); it.W(as_p, "fail", r, "exec")
it.note("Launch From Jump Pad: Who is Begin Overlap's who (only players fly); Speed 0 is 800 units a second straight up.")

# ---- Double Jump ------------------------------------------------------------------------
it = item("st_double_jump", "Double Jump", "A second jump in the air (Boost 0: 320 up), once until the player lands again.", "Starter: movement",
          "Wire Player Pressed Button (Jump) in with the player. On the ground it only notes the jump; in the air it boosts once. Bots: Bot Wants Ability can fire it now and then while they chase or flee (call it from a Repeat for bots in the air).")
USED = it.var("dj_used", "Double jumped", "bool", "player", False)
ein, eout = it.macro("dj_jump", "Double Jump", [("go", "In", "exec"), ("who", "Player", "player"), ("boost", "Boost", "number")], [("next", "Jumped", "exec"), ("no", "Not now", "exec")], root=True)
tr = it.N("tr", "player.transform", row=2); it.W(ein, "who", tr, "player")
br = it.N("ground", "flow.branch"); it.W(ein, "go", br, "exec"); it.W(tr, "ground", br, "cond")
rs = it.N("reset", "var.set", var=USED, value=False); it.W(br, "true", rs, "exec"); it.W(ein, "who", rs, "player")
it.W(rs, "then", eout, "no")
used = it.get("used", USED, (ein, "who"), row=3)
br2 = it.N("once", "flow.branch"); it.W(br, "false", br2, "exec"); it.W(used, "out", br2, "cond")
it.W(br2, "true", eout, "no")
bo = it.default("boost", ein, "boost", 320, row=4)
vb = it.N("vb", "vec.break", row=3); it.W(tr, "vel", vb, "in")
nv = it.N("nv", "vec.make", row=3); it.W(vb, "x", nv, "x"); it.W(vb, "y", nv, "y"); it.W(bo, "out", nv, "z")
sv = it.N("boostv", "player.set_velocity"); it.W(br2, "false", sv, "exec"); it.W(ein, "who", sv, "player"); it.W(nv, "out", sv, "v")
su = it.N("mark", "var.set", var=USED, value=True); it.W(sv, "then", su, "exec"); it.W(ein, "who", su, "player")
snd = it.N("snd", "fx.sound_to", sound="player/pl_jump2.wav"); it.W(su, "then", snd, "exec"); it.W(ein, "who", snd, "player")
it.W(snd, "then", eout, "next")

# ---- Dash -------------------------------------------------------------------------------
it = item("st_dash", "Dash", "A quick dash the way the player looks (Speed 0: 700), at most once every Cooldown seconds (0: 3).", "Starter: movement",
          "Wire Player Pressed Button (Reload, or any key you like) in with the player. Not ready says the seconds left. Bots: from a Repeat, Bot Wants Ability (Ability with Cooldown) lets them dash now and then while chasing or fleeing; the cooldown holds for them too.")
NEXT = it.var("dash_at", "Dash ready at", "number", "player", 0)
ein, eout = it.macro("dash_go", "Dash", [("go", "In", "exec"), ("who", "Player", "player"), ("speed", "Speed", "number"), ("cd", "Cooldown", "number")], [("next", "Dashed", "exec"), ("wait", "Not ready", "exec"), ("left", "Seconds left", "number")], root=True)
cd = it.default("cd", ein, "cd", 3)
sp = it.default("speed", ein, "speed", 700, row=5)
now = it.N("now", "game.time", row=2)
at = it.get("at", NEXT, (ein, "who"))
left = it.N("left", "math.op", row=2, op="sub"); it.W(at, "out", left, "a"); it.W(now, "out", left, "b")
ok = it.N("ok", "math.compare", row=2, op="le", b=0); it.W(left, "out", ok, "a")
br = it.N("br", "flow.branch"); it.W(ein, "go", br, "exec"); it.W(ok, "out", br, "cond")
up = it.N("ceil", "math.fn", row=3, op="ceil"); it.W(left, "out", up, "a")
it.W(br, "false", eout, "wait"); it.W(up, "out", eout, "left")
aim = it.N("aim", "vec.player", row=7, what="aim"); it.W(ein, "who", aim, "player")
ab = it.N("ab", "vec.break", row=7); it.W(aim, "out", ab, "in")
flat = it.N("flat", "vec.make", row=7, z=0); it.W(ab, "x", flat, "x"); it.W(ab, "y", flat, "y")
dirn = it.N("dir", "vec.normalize", row=7); it.W(flat, "out", dirn, "a")
fast = it.N("fast", "vec.scale", row=8); it.W(dirn, "out", fast, "a"); it.W(sp, "out", fast, "n")
lift = it.N("lift", "vec.make", row=8, x=0, y=0, z=120)
v = it.N("v", "vec.op", row=8, op="add"); it.W(fast, "out", v, "a"); it.W(lift, "out", v, "b")
sv = it.N("push", "player.set_velocity"); it.W(br, "true", sv, "exec"); it.W(ein, "who", sv, "player"); it.W(v, "out", sv, "v")
nxt = it.N("nxt", "math.op", row=3, op="add"); it.W(now, "out", nxt, "a"); it.W(cd, "out", nxt, "b")
st = it.N("st", "var.set", var=NEXT); it.W(sv, "then", st, "exec"); it.W(ein, "who", st, "player"); it.W(nxt, "out", st, "value")
snd = it.N("snd", "fx.sound_to", sound="weapons/knife_slash1.wav"); it.W(st, "then", snd, "exec"); it.W(ein, "who", snd, "player")
it.W(snd, "then", eout, "next")

# ---- Teleporter Pair --------------------------------------------------------------------
it = item("st_teleporter", "Teleporter Pair", "Sends a player to a destination entity, at most once every Cooldown seconds (0: 2).", "Starter: movement",
          "Wire a pad's Begin Overlap (or Player Touched) in with the other pad's entity as To. The cooldown stops a player bouncing straight back. Bots use teleporters they walk into.")
LAST = it.var("tp_last", "Last teleport", "number", "player", 0)
e = it.function("tp_go", "Teleport Through", [("who", "Who", "entity"), ("to", "To", "entity"), ("cd", "Cooldown", "number")], [("went", "Went", "bool")])
as_p = it.N("asp", "cast.player"); it.W(e, "then", as_p, "exec"); it.W(e, "who", as_p, "in")
cd = it.default("cd", e, "cd", 2)
now = it.N("now", "game.time", row=2)
last = it.get("last", LAST, (as_p, "out"))
since = it.N("since", "math.op", row=2, op="sub"); it.W(now, "out", since, "a"); it.W(last, "out", since, "b")
ok = it.N("ok", "math.compare", row=2, op="ge"); it.W(since, "out", ok, "a"); it.W(cd, "out", ok, "b")
br = it.N("br", "flow.branch"); it.W(as_p, "ok", br, "exec"); it.W(ok, "out", br, "cond")
tp = it.N("tp", "player.teleport"); it.W(br, "true", tp, "exec"); it.W(as_p, "out", tp, "player"); it.W(e, "to", tp, "to")
st = it.N("st", "var.set", var=LAST); it.W(tp, "then", st, "exec"); it.W(as_p, "out", st, "player"); it.W(now, "out", st, "value")
snd = it.N("snd", "fx.sound_to", sound="debris/beamstart2.wav"); it.W(st, "then", snd, "exec"); it.W(as_p, "out", snd, "player")
yes = it.ret(); it.W(snd, "then", yes, "exec")
went_t = it.N("went_t", "math.compare", row=4, op="eq", a=1, b=1); it.W(went_t, "out", yes, "went")
no = it.N("ret2", "fn.return", row=1); it.pins[no] = ({"exec", "went"}, set())
it.W(br, "false", no, "exec"); it.W(as_p, "fail", no, "exec")
went_f = it.N("went_f", "math.compare", row=5, op="eq", a=0, b=1); it.W(went_f, "out", no, "went")

# =========================================================================================
# Abilities
# =========================================================================================

# ---- Ability with Cooldown --------------------------------------------------------------
it = item("st_ability", "Ability with Cooldown", "A template for any ability: Use runs when it's ready, Not ready says the seconds left; Bot Wants Ability for bots.", "Starter: abilities",
          "Wire your key or event into In with the Player; build the ability on Use. Cooldown 0 is 10 seconds. Bots: Bot Wants Ability (Chance a second, 0: 25%) says yes when a living enemy is within Range (0: 1500) so a Repeat every second can fire it in fights.")
READY = it.var("ab_ready", "Ability ready at", "number", "player", 0)
ein, eout = it.macro("ab_use", "Ability with Cooldown", [("go", "In", "exec"), ("who", "Player", "player"), ("cd", "Cooldown", "number")],
                     [("use", "Use", "exec"), ("wait", "Not ready", "exec"), ("left", "Seconds left", "number")], root=True)
cd = it.default("cd", ein, "cd", 10)
now = it.N("now", "game.time", row=2)
at = it.get("at", READY, (ein, "who"))
left = it.N("left", "math.op", row=2, op="sub"); it.W(at, "out", left, "a"); it.W(now, "out", left, "b")
ready = it.N("ready", "math.compare", row=2, op="le", b=0); it.W(left, "out", ready, "a")
br = it.N("br", "flow.branch"); it.W(ein, "go", br, "exec"); it.W(ready, "out", br, "cond")
nxt = it.N("next", "math.op", row=3, op="add"); it.W(now, "out", nxt, "a"); it.W(cd, "out", nxt, "b")
st = it.N("st", "var.set", var=READY); it.W(br, "true", st, "exec"); it.W(ein, "who", st, "player"); it.W(nxt, "out", st, "value")
it.W(st, "then", eout, "use")
up = it.N("ceil", "math.fn", row=4, op="ceil"); it.W(left, "out", up, "a")
it.W(br, "false", eout, "wait"); it.W(up, "out", eout, "left")
it.note("Ability with Cooldown: each player has their own cooldown. Seconds left is whole seconds (for a HUD line).")
e = it.function("ab_bot", "Bot Wants Ability", [("bot", "Bot", "player"), ("chance", "Chance a second", "number"), ("range", "Range", "number")], [("yes", "Use it", "bool")])
ch = it.default("chance", e, "chance", 25)
rg = it.default("range", e, "range", 1500, row=6)
FOUND = it.var("ab_seen", "Enemy near", "bool", "mode", False)
s0 = it.N("s0", "var.set", var=FOUND, value=False); it.W(e, "then", s0, "exec")
each = it.N("each", "flow.for_each_player", which="alive"); it.W(s0, "then", each, "exec")
myteam = it.N("myt", "player.get_team", row=2); it.W(e, "bot", myteam, "player")
histeam = it.N("hist", "player.get_team", row=2); it.W(each, "player", histeam, "player")
other = it.N("other", "logic.equal", row=2, of="team", op="ne"); it.W(myteam, "out", other, "a"); it.W(histeam, "out", other, "b")
pa = it.N("pa", "vec.player", row=3); it.W(e, "bot", pa, "player")
pb = it.N("pb", "vec.player", row=3); it.W(each, "player", pb, "player")
dist = it.N("dist", "vec.distance", row=3); it.W(pa, "out", dist, "a"); it.W(pb, "out", dist, "b")
near = it.N("near", "math.compare", row=3, op="lt"); it.W(dist, "out", near, "a"); it.W(rg, "out", near, "b")
both = it.N("both", "logic.and_or", row=2, op="and"); it.W(other, "out", both, "a"); it.W(near, "out", both, "b")
br2 = it.N("br2", "flow.branch"); it.W(each, "body" if False else "each", br2, "exec"); it.W(both, "out", br2, "cond")
s1 = it.N("s1", "var.set", var=FOUND, value=True); it.W(br2, "true", s1, "exec")
isbot = it.N("isbot", "player.is_bot", row=4); it.W(e, "bot", isbot, "player")
roll = it.N("roll", "math.random", row=5, min=0, max=100)
lucky = it.N("lucky", "math.compare", row=5, op="lt"); it.W(roll, "out", lucky, "a"); it.W(ch, "out", lucky, "b")
seen = it.get("seen", FOUND, row=4)
a1 = it.N("a1", "logic.and_or", row=4, op="and"); it.W(isbot, "out", a1, "a"); it.W(seen, "out", a1, "b")
a2 = it.N("a2", "logic.and_or", row=4, op="and"); it.W(a1, "out", a2, "a"); it.W(lucky, "out", a2, "b")
r = it.ret(); it.W(each, "then", r, "exec"); it.W(a2, "out", r, "yes")

# ---- Fireball ---------------------------------------------------------------------------
it = item("st_fireball", "Fireball", "Throws a fireball where the player aims; it explodes where it hits (Damage 0: 60, Speed 0: 1100).", "Starter: abilities",
          "Wire it after Ability with Cooldown's Use. The explosion hurts in a radius and the kills go to the thrower. Bots can throw it through Bot Wants Ability.")
ein, eout = it.macro("fb_throw", "Fireball", [("go", "In", "exec"), ("who", "Player", "player"), ("dmg", "Damage", "number"), ("speed", "Speed", "number")], [("next", "Thrown", "exec")])
dmg = it.default("dmg", ein, "dmg", 60)
spd = it.default("speed", ein, "speed", 1100, row=5)
eyes = it.N("eyes", "vec.player", row=2, what="eyes"); it.W(ein, "who", eyes, "player")
aim = it.N("aim", "vec.player", row=2, what="aim"); it.W(ein, "who", aim, "player")
p = it.N("proj", "proj.spawn", model="sprites/fexplo.spr", scale=0.6, life=4, radius=150, explode=True, trail=True, dangerous=True)
it.W(ein, "go", p, "exec"); it.W(ein, "who", p, "owner"); it.W(eyes, "out", p, "pos"); it.W(aim, "out", p, "dir"); it.W(dmg, "out", p, "damage"); it.W(spd, "out", p, "speed")
it.W(p, "then", eout, "next")
it.note("Fireball: the projectile carries the damage (Radius 150, an explosion where it hits); the kills go to the thrower.")

# ---- Proximity Mine ---------------------------------------------------------------------
# (each mine's marker, owner and damage in lists beside the list of mine volumes, at the same place)
it = item("st_mine", "Proximity Mine", "Places a mine at the player's feet that blows up when someone steps near (Radius 0: 96, Damage 0: 120).", "Starter: abilities",
          "Place Mine makes a trigger volume named mine and a marker. Wire Begin Overlap (name mine) into Mine Stepped On. Its owner can't set it off. How many a player may lay is up to your mode. Bots: the mine is marked for bots to avoid once Bots Avoid Area exists (until then they may walk into it).")
VOLS = it.var("mine_vols", "Mines", "array<entity>", "mode")
MARKS = it.var("mine_marks", "Mine markers", "array<entity>", "mode")
OWNER = it.var("mine_owner", "Mine owners", "array<player>", "mode")
DMG = it.var("mine_dmg", "Mine damage", "array<number>", "mode")
e = it.function("mine_place", "Place Mine", [("who", "Player", "player"), ("radius", "Radius", "number"), ("dmg", "Damage", "number")], [("mine", "Mine", "entity")], root=True)
rad = it.default("rad", e, "radius", 96)
dm = it.default("dmg", e, "dmg", 120, row=5)
pos = it.N("pos", "vec.player", row=2, what="origin"); it.W(e, "who", pos, "player")
v = it.N("vol", "vol.spawn", shape="sphere", radius=96, name="mine"); it.W(e, "then", v, "exec"); it.W(pos, "out", v, "pos")
mk = it.N("mark", "fx.marker", sprite="sprites/ledglow.spr", height=4, scale=0.3, r=255, g=40, b=0, pulse=True); it.W(v, "then", mk, "exec"); it.W(pos, "out", mk, "pos")
prev = (mk, "then")
for k, (var, of, src) in enumerate([(VOLS, "entity", (v, "volume")), (MARKS, "entity", (mk, "out")), (OWNER, "player", (e, "who")), (DMG, "number", (dm, "out"))]):
    ad = it.N(f"add{k}", "array.add", of=of); it.W(prev[0], prev[1], ad, "exec"); it.W(src[0], src[1], ad, "item")
    g = it.get(f"g{k}", var, row=3 + k); it.W(g, "out", ad, "array")
    prev = (ad, "then")
r = it.ret(); it.W(prev[0], prev[1], r, "exec"); it.W(v, "volume", r, "mine")
e = it.function("mine_step", "Mine Stepped On", [("mine", "Volume", "entity"), ("who", "Who", "entity")])
gv = it.get("gv", VOLS, row=2)
idx = it.N("idx", "array.find", row=2, of="entity"); it.W(gv, "out", idx, "array"); it.W(e, "mine", idx, "item")
known = it.N("known", "math.compare", row=2, op="ge", b=0); it.W(idx, "out", known, "a")
br0 = it.N("br0", "flow.branch"); it.W(e, "then", br0, "exec"); it.W(known, "out", br0, "cond")
def at(id, var, of, row):
    g = it.get(f"g_{id}", var, row=row)
    a = it.N(id, "array.get", row=row, of=of); it.W(g, "out", a, "array"); it.W(idx, "out", a, "at")
    return a
own = at("own", OWNER, "player", 3)
asp = it.N("asp", "cast.player"); it.W(br0, "true", asp, "exec"); it.W(e, "who", asp, "in")
mine_own = it.N("isown", "logic.equal", row=4, of="player", op="eq"); it.W(asp, "out", mine_own, "a"); it.W(own, "out", mine_own, "b")
br = it.N("br", "flow.branch"); it.W(asp, "ok", br, "exec"); it.W(mine_own, "out", br, "cond")
boom_pos = it.N("bpos", "vec.entity", row=2); it.W(e, "mine", boom_pos, "in")
dmv = at("dmv", DMG, "number", 5)
boom = it.N("boom", "fx.explosion", radius=200); it.W(br, "false", boom, "exec"); it.W(asp, "fail", boom, "exec"); it.W(boom_pos, "out", boom, "pos"); it.W(dmv, "out", boom, "dmg")
mkf = at("mkf", MARKS, "entity", 6)
rm = it.N("rmmark", "fx.marker_remove"); it.W(boom, "then", rm, "exec"); it.W(mkf, "out", rm, "marker")
rv = it.N("rmvol", "vol.remove"); it.W(rm, "then", rv, "exec"); it.W(e, "mine", rv, "volume")
prev = (rv, "then")
for k, (var, of) in enumerate([(VOLS, "entity"), (MARKS, "entity"), (OWNER, "player"), (DMG, "number")]):
    ri = it.N(f"rm{k}", "array.remove_index", of=of); it.W(prev[0], prev[1], ri, "exec"); it.W(idx, "out", ri, "at")
    g = it.get(f"gr{k}", var, row=7 + k); it.W(g, "out", ri, "array")
    prev = (ri, "then")
r = it.ret(); it.W(prev[0], prev[1], r, "exec"); it.W(br, "true", r, "exec"); it.W(br0, "false", r, "exec")

# =========================================================================================
# Pickups
# =========================================================================================

def pickup(file, pre, name, desc, help, give):
    it = item(file, name, desc, "Starter: pickups", help)
    VOLS = it.var(f"{pre}_vols", f"{name} volumes", "array<entity>", "mode")
    POS = it.var(f"{pre}_pos", f"{name} spots", "array<vector>", "mode")
    MK = it.var(f"{pre}_mk", f"{name} markers", "array<entity>", "mode")
    ein, eout = it.macro(f"{pre}_at", f"Spawn {name} Here", [("go", "In", "exec"), ("pos", "Position", "vector")], [("next", "Then", "exec"), ("pick", "Pickup", "entity")])
    vol = it.N("vol", "vol.spawn", shape="sphere", radius=48, name=pre); it.W(ein, "go", vol, "exec")
    pick = it.N("pickpos", "flow.reroute", row=2); it.W(ein, "pos", pick, "in")
    it.W(pick, "out", vol, "pos")
    mk = it.N("mark", "fx.marker", **give["marker"]); it.W(vol, "then", mk, "exec"); it.W(pick, "out", mk, "pos")
    prev = (mk, "then")
    for k, (var, of, src) in enumerate([(VOLS, "entity", (vol, "volume")), (POS, "vector", (pick, "out")), (MK, "entity", (mk, "out"))]):
        ad = it.N(f"add{k}", "array.add", of=of); it.W(prev[0], prev[1], ad, "exec"); it.W(src[0], src[1], ad, "item")
        g = it.get(f"g{k}", var, row=4 + k); it.W(g, "out", ad, "array")
        prev = (ad, "then")
    # bots go and get it (a loot goal there)
    bg = it.N("bots", "bots.goal", kind="loot", prio=40, secs=3600, radius=48); it.W(prev[0], prev[1], bg, "exec"); it.W(vol, "volume", bg, "at")
    it.W(bg, "then", eout, "next"); it.W(vol, "volume", eout, "pick")
    ein, eout = it.macro(f"{pre}_spawn", f"Spawn {name}", [("go", "In", "exec"), ("at", "At", "entity")], [("next", "Then", "exec"), ("pick", "Pickup", "entity")])
    ep = it.N("epos", "vec.entity", row=2); it.W(ein, "at", ep, "in")
    sp = it.N("spawn", "macro.use", macro=f"{pre}_at"); it.W(ein, "go", sp, "go"); it.W(ep, "out", sp, "pos")
    it.W(sp, "next", eout, "next"); it.W(sp, "pick", eout, "pick")
    ein, eout = it.macro(f"{pre}_take", f"Take {name}", [("go", "In", "exec"), ("vol", "Volume", "entity"), ("who", "Who", "entity")] + give["inputs"] + [("respawn", "Respawn after", "number")], [("taken", "Taken", "exec"), ("no", "Not taken", "exec")], root=True)
    gv = it.get("gv", VOLS, row=2)
    idx = it.N("idx", "array.find", row=2, of="entity"); it.W(gv, "out", idx, "array"); it.W(ein, "vol", idx, "item")
    known = it.N("known", "math.compare", row=2, op="ge", b=0); it.W(idx, "out", known, "a")
    br0 = it.N("br0", "flow.branch"); it.W(ein, "go", br0, "exec"); it.W(known, "out", br0, "cond")
    it.W(br0, "false", eout, "no")
    asp = it.N("asp", "cast.player"); it.W(br0, "true", asp, "exec"); it.W(ein, "who", asp, "in")
    it.W(asp, "fail", eout, "no")
    done, refused = give["build"](it, ein, asp)
    if refused:
        it.W(refused[0], refused[1], eout, "no")
    def at(id, var, of, row):
        g = it.get(f"g_{id}", var, row=row)
        a = it.N(id, "array.get", row=row, of=of); it.W(g, "out", a, "array"); it.W(idx, "out", a, "at")
        return a
    pf = at("posf", POS, "vector", 5)
    mf = at("mkf", MK, "entity", 6)
    clr = it.N("botsoff", "bots.goal_clear", kind="loot"); it.W(done[0], done[1], clr, "exec")
    rm = it.N("rmmk", "fx.marker_remove"); it.W(clr, "then", rm, "exec"); it.W(mf, "out", rm, "marker")
    rv = it.N("rmvol", "vol.remove"); it.W(rm, "then", rv, "exec"); it.W(ein, "vol", rv, "volume")
    # the spot kept for the respawn (local copy, before the lists drop this pickup)
    SPOT = it.var(f"{pre}_spot", f"{name} respawn spot", "vector", "mode")
    keep = it.N("keep", "var.set", var=SPOT); it.W(rv, "then", keep, "exec"); it.W(pf, "out", keep, "value")
    prev = (keep, "then")
    for k, (var, of) in enumerate([(VOLS, "entity"), (POS, "vector"), (MK, "entity")]):
        ri = it.N(f"rm{k}", "array.remove_index", of=of); it.W(prev[0], prev[1], ri, "exec"); it.W(idx, "out", ri, "at")
        g = it.get(f"gr{k}", var, row=8 + k); it.W(g, "out", ri, "array")
        prev = (ri, "then")
    seq = it.N("seq", "flow.sequence", count=2); it.W(prev[0], prev[1], seq, "exec")
    it.W(seq, "then0", eout, "taken")
    rs = it.default("rs", ein, "respawn", 20, row=11)
    spot = it.get("spot", SPOT, row=12)
    wait = it.N("wait", "timer.delay"); it.W(seq, "then1", wait, "exec"); it.W(rs, "out", wait, "secs")
    again = it.N("again", "macro.use", macro=f"{pre}_at"); it.W(wait, "then", again, "go"); it.W(spot, "out", again, "pos")
    it.note(f"Take {name}: Volume and Who are Begin Overlap's (name {pre}). Respawn after 0 is 20 seconds. How many pickups a map has is up to you.")
    return it


def heal_build(it, ein, asp):
    hp = it.N("hp", "player.get_health", row=2); it.W(asp, "out", hp, "player")
    full = it.N("full", "math.compare", row=2, op="lt", b=100); it.W(hp, "out", full, "a")
    br = it.N("br", "flow.branch"); it.W(asp, "ok", br, "exec"); it.W(full, "out", br, "cond")
    amt = it.default("heal", ein, "heal", 50, row=3)
    add = it.N("add", "math.op", row=3, op="add"); it.W(hp, "out", add, "a"); it.W(amt, "out", add, "b")
    cap = it.N("cap", "math.op", row=3, op="min", b=100); it.W(add, "out", cap, "a")
    sh = it.N("sethp", "player.set_health"); it.W(br, "true", sh, "exec"); it.W(asp, "out", sh, "player"); it.W(cap, "out", sh, "amount")
    snd = it.N("snd", "fx.sound_to", sound="items/smallmedkit1.wav"); it.W(sh, "then", snd, "exec"); it.W(asp, "out", snd, "player")
    return (snd, "then"), (br, "false")


pickup("st_health_pack", "heal", "Health Pack", "A health pack on a trigger volume: heals (0: 50, up to 100) and comes back after Respawn seconds (0: 20).",
       "Spawn Health Pack at an entity (Spawn Health Pack Here takes a position); wire Begin Overlap (name heal) into Take Health Pack. A full-health player leaves it. Bots: each pack is a loot goal, so hurt or not, bots go and grab it.",
       {"marker": {"sprite": "sprites/glow01.spr", "height": 16, "scale": 0.5, "r": 40, "g": 255, "b": 60, "pulse": True}, "inputs": [("heal", "Heal", "number")], "build": heal_build})


def weapon_build(it, ein, asp):
    gw = it.N("give", "player.give_weapon", ammo=90); it.W(asp, "ok", gw, "exec"); it.W(asp, "out", gw, "player"); it.W(ein, "weapon", gw, "weapon")
    snd = it.N("snd", "fx.sound_to", sound="items/gunpickup2.wav"); it.W(gw, "then", snd, "exec"); it.W(asp, "out", snd, "player")
    return (snd, "then"), None  # (a weapon pickup is always taken)


pickup("st_weapon_pick", "weap", "Weapon Pickup", "A weapon lying on a trigger volume: gives it (Weapon, e.g. awp) and comes back after Respawn seconds (0: 20).",
       "Spawn Weapon Pickup at an entity (Spawn Weapon Pickup Here takes a position); wire Begin Overlap (name weap) into Take Weapon Pickup with the Weapon name (ak47, awp, m4a1...). Bots: each spawner is a loot goal, so bots go and grab it.",
       {"marker": {"sprite": "sprites/arrow1.spr", "height": 24, "scale": 0.4, "r": 255, "g": 200, "b": 0, "pulse": True}, "inputs": [("weapon", "Weapon", "text")], "build": weapon_build})

# =========================================================================================
# Player state
# =========================================================================================

# ---- Spawn Protection -------------------------------------------------------------------
it = item("st_spawn_prot", "Spawn Protection", "God mode and a glow for Seconds (0: 3) after a player spawns; it ends early if they shoot.", "Starter: player state",
          "Wire Player Spawned in. Call End Spawn Protection from Weapon Fired to end it as soon as they shoot. Bots get it too (and don't know they have it).")
PROT = it.var("sp_on", "Protected", "bool", "player", False)
ein, eout = it.macro("sp_give", "Spawn Protection", [("go", "In", "exec"), ("who", "Player", "player"), ("secs", "Seconds", "number")], [("next", "Then", "exec"), ("over", "Over", "exec")], root=True)
sc = it.default("secs", ein, "secs", 3)
gm = it.N("god", "player.godmode", on=True); it.W(ein, "go", gm, "exec"); it.W(ein, "who", gm, "player")
gl = it.N("glow", "player.set_glow", color="white"); it.W(gm, "then", gl, "exec"); it.W(ein, "who", gl, "player")
s1 = it.N("on", "var.set", var=PROT, value=True); it.W(gl, "then", s1, "exec"); it.W(ein, "who", s1, "player")
seq = it.N("seq", "flow.sequence", count=2); it.W(s1, "then", seq, "exec"); it.W(seq, "then0", eout, "next")
wait = it.N("wait", "timer.delay"); it.W(seq, "then1", wait, "exec"); it.W(sc, "out", wait, "secs")
endc = it.N("end", "fn.call", func="sp_end"); it.W(wait, "then", endc, "exec"); it.W(ein, "who", endc, "who")
it.W(endc, "then", eout, "over")
e = it.function("sp_end", "End Spawn Protection", [("who", "Player", "player")])
on = it.get("on", PROT, (e, "who"))
br = it.N("br", "flow.branch"); it.W(e, "then", br, "exec"); it.W(on, "out", br, "cond")
gm = it.N("god", "player.godmode", on=False); it.W(br, "true", gm, "exec"); it.W(e, "who", gm, "player")
gl = it.N("glow", "player.set_glow", color="off"); it.W(gm, "then", gl, "exec"); it.W(e, "who", gl, "player")
s0 = it.N("off", "var.set", var=PROT, value=False); it.W(gl, "then", s0, "exec"); it.W(e, "who", s0, "player")
r = it.ret(); it.W(s0, "then", r, "exec"); it.W(br, "false", r, "exec")

# ---- Lives Counter ----------------------------------------------------------------------
it = item("st_lives", "Lives Counter", "Each player's lives: Lose a Life on a death (with a HUD line), Out when none are left; Reset Lives at the start (0: 3).", "Starter: player state",
          "Wire Round Started into Reset Lives (For Each Player), and Player Killed's victim into Lose a Life; on Out, keep them dead or move them to spectators. Bots count lives like anyone.")
LIVES = it.var("lv_n", "Lives", "number", "player", 3)
e = it.function("lv_reset", "Reset Lives", [("who", "Player", "player"), ("n", "Lives", "number")])
n = it.default("n", e, "n", 3)
s = it.N("set", "var.set", var=LIVES); it.W(e, "then", s, "exec"); it.W(e, "who", s, "player"); it.W(n, "out", s, "value")
r = it.ret(); it.W(s, "then", r, "exec")
e = it.function("lv_lose", "Lose a Life", [("who", "Player", "player")], [("left", "Lives left", "number"), ("gone", "Out", "bool")], root=True)
a = it.N("sub", "var.add", var=LIVES, amount=-1); it.W(e, "then", a, "exec"); it.W(e, "who", a, "player")
g = it.get("g", LIVES, (e, "who"))
txt = it.N("txt", "text.format", row=3, text="Lives left: [0]"); it.W(g, "out", txt, "a0")
hud = it.N("hud", "msg.hud", slot="center", seconds=3, text=""); it.W(a, "then", hud, "exec"); it.W(e, "who", hud, "player"); it.W(txt, "out", hud, "value")
out = it.N("out", "math.compare", row=3, op="le", b=0); it.W(g, "out", out, "a")
r = it.ret(); it.W(hud, "then", r, "exec"); it.W(g, "out", r, "left"); it.W(out, "out", r, "gone")

# ---- Status effects ---------------------------------------------------------------------
it = item("st_status", "Burn, Slow and Freeze", "Status effects with timers: Burn (Seconds 0: 4, Damage a second 0: 5), Slow (0: 4 s at 0: 150 speed) and Freeze (0: 2 s).", "Starter: player state",
          "Call one from a hit, a trap or an ability. Each ends by itself; Slow puts the speed back to 250. Bots: a burning player's spot is marked for bots to avoid once Bots Avoid Area exists.")
ein, eout = it.macro("st_burn", "Burn", [("go", "In", "exec"), ("who", "Player", "player"), ("by", "By", "player"), ("secs", "Seconds", "number"), ("dps", "Damage a second", "number")], [("next", "Then", "exec")], root=True)
b = it.N("burn", "fun.burn", secs=4, dps=5); it.W(ein, "go", b, "exec"); it.W(ein, "who", b, "player"); it.W(ein, "by", b, "by")
it.W(b, "then", eout, "next")
it.note("Burn uses Set On Fire's own timer; Seconds and Damage a second are its settings (wire them in to change them per call).")
ein, eout = it.macro("st_slow", "Slow", [("go", "In", "exec"), ("who", "Player", "player"), ("secs", "Seconds", "number"), ("speed", "Speed", "number")], [("next", "Then", "exec"), ("over", "Over", "exec")])
sc = it.default("secs", ein, "secs", 4)
sp = it.default("speed", ein, "speed", 150, row=6)
s1 = it.N("slow", "player.set_speed"); it.W(ein, "go", s1, "exec"); it.W(ein, "who", s1, "player"); it.W(sp, "out", s1, "speed")
seq = it.N("seq", "flow.sequence", count=2); it.W(s1, "then", seq, "exec"); it.W(seq, "then0", eout, "next")
w = it.N("wait", "timer.delay"); it.W(seq, "then1", w, "exec"); it.W(sc, "out", w, "secs")
s2 = it.N("back", "player.set_speed", speed=250); it.W(w, "then", s2, "exec"); it.W(ein, "who", s2, "player")
it.W(s2, "then", eout, "over")
ein, eout = it.macro("st_freeze", "Freeze", [("go", "In", "exec"), ("who", "Player", "player"), ("secs", "Seconds", "number")], [("next", "Then", "exec"), ("over", "Over", "exec")])
sc = it.default("secs", ein, "secs", 2)
f1 = it.N("frz", "player.freeze", on=True); it.W(ein, "go", f1, "exec"); it.W(ein, "who", f1, "player")
gl = it.N("glow", "player.set_glow", color="blue"); it.W(f1, "then", gl, "exec"); it.W(ein, "who", gl, "player")
seq = it.N("seq", "flow.sequence", count=2); it.W(gl, "then", seq, "exec"); it.W(seq, "then0", eout, "next")
w = it.N("wait", "timer.delay"); it.W(seq, "then1", w, "exec"); it.W(sc, "out", w, "secs")
f2 = it.N("thaw", "player.freeze", on=False); it.W(w, "then", f2, "exec"); it.W(ein, "who", f2, "player")
g2 = it.N("glow2", "player.set_glow", color="off"); it.W(f2, "then", g2, "exec"); it.W(ein, "who", g2, "player")
it.W(g2, "then", eout, "over")

# =========================================================================================
# Rounds and scoring
# =========================================================================================

# ---- Kill Streak Announcer --------------------------------------------------------------
it = item("st_kill_streak", "Kill Streak Announcer", "Counts kills in a row per player and announces 3, 5, 7 and 10 to everyone; a death resets it.", "Starter: rounds and scoring",
          "Wire Player Killed in (Killer = attacker, Victim = victim). Streak is the killer's count after this kill. Bots' streaks are announced too.")
STREAK = it.var("ks_n", "Kill streak", "number", "player", 0)
e = it.function("ks_kill", "Count Kill Streak", [("killer", "Killer", "player"), ("victim", "Victim", "player")], [("n", "Streak", "number")], root=True)
z = it.N("reset", "var.set", var=STREAK, value=0); it.W(e, "then", z, "exec"); it.W(e, "victim", z, "player")
same = it.N("self", "logic.equal", row=2, of="player", op="ne"); it.W(e, "killer", same, "a"); it.W(e, "victim", same, "b")
br = it.N("br", "flow.branch"); it.W(z, "then", br, "exec"); it.W(same, "out", br, "cond")
add = it.N("add", "var.add", var=STREAK, amount=1); it.W(br, "true", add, "exec"); it.W(e, "killer", add, "player")
g = it.get("g", STREAK, (e, "killer"))
sw = it.N("sw", "flow.switch_num", cases="3,5,7,10"); it.W(add, "then", sw, "exec"); it.W(g, "out", sw, "in")
r = it.ret(); it.W(sw, "default", r, "exec"); it.W(br, "false", r, "exec"); it.W(g, "out", r, "n")
for k, (case, words) in enumerate([("3", "is on a killing spree!"), ("5", "is unstoppable!"), ("7", "is godlike!"), ("10", "is LEGENDARY!")]):
    txt = it.N(f"t{case}", "text.format", row=5 + k, text="[0] " + words); it.W(e, "killer", txt, "a0")
    h = it.N(f"h{case}", "msg.hud", row=5 + k, slot="top", seconds=4, text="")
    it.W(sw, f"c{k}", h, "exec"); it.W(txt, "out", h, "value")
    s = it.N(f"s{case}", "game.play_sound", row=5 + k, sound="buttons/bell1")
    it.W(h, "then", s, "exec")
    it.W(s, "then", r, "exec")

# ---- Countdown HUD ----------------------------------------------------------------------
it = item("st_countdown", "Countdown HUD", "Counts down Seconds (0: 60) on everyone's HUD as minutes:seconds, then Done.", "Starter: rounds and scoring",
          "Wire Round Started (or Freeze Time Ended) in. Label goes before the time (empty: Time left); it stops at 0 and runs Done. Bots don't read it.")
LEFT = it.var("cd_left", "Countdown left", "number", "mode", 0)
ein, eout = it.macro("cd_run", "Countdown HUD", [("go", "In", "exec"), ("secs", "Seconds", "number"), ("label", "Label", "text")], [("started", "Started", "exec"), ("done", "Done", "exec")], root=True)
sc = it.default("secs", ein, "secs", 60)
LABEL = it.var("cd_label", "Countdown label", "text", "mode", "")
sl = it.N("setl", "var.set", var=LABEL); it.W(ein, "go", sl, "exec"); it.W(ein, "label", sl, "value")
s = it.N("set", "var.set", var=LEFT); it.W(sl, "then", s, "exec"); it.W(sc, "out", s, "value")
rep = it.N("rep", "timer.repeat", every=1); it.W(s, "then", rep, "exec"); it.W(rep, "then", eout, "started")
g = it.get("g", LEFT, row=3)
ft = it.N("ft", "text.format_time", row=3, style="ms"); it.W(g, "out", ft, "secs")
gl = it.get("gl", LABEL, row=4)
empty = it.N("empty", "text.equals", row=4, b=""); it.W(gl, "out", empty, "a")
dl = it.N("dlabel", "text.literal", row=5, value="Time left")
lab = it.N("lab", "logic.select", row=4, of="text"); it.W(empty, "out", lab, "cond"); it.W(dl, "out", lab, "a"); it.W(gl, "out", lab, "b")
txt = it.N("txt", "text.format", row=3, text="[0]: [1]"); it.W(lab, "out", txt, "a0"); it.W(ft, "out", txt, "a1")
if "msg.big_hud" in NODES_OK:
    h = it.N("hud", "msg.big_hud", text="", x=-1, y=0.12, seconds=1.1, effect="fade"); it.W(rep, "tick", h, "exec"); it.W(txt, "out", h, "value")
else:
    h = it.N("hud", "msg.hud", slot="top", seconds=1.1, text=""); it.W(rep, "tick", h, "exec"); it.W(txt, "out", h, "value")
dec = it.N("dec", "var.add", var=LEFT, amount=-1); it.W(h, "then", dec, "exec")
over = it.N("over", "math.compare", row=4, op="lt", b=0); it.W(g, "out", over, "a")
br = it.N("br", "flow.branch"); it.W(dec, "then", br, "exec"); it.W(over, "out", br, "cond")
stop = it.N("stop", "timer.clear"); it.W(br, "true", stop, "exec"); it.W(rep, "handle", stop, "handle")
it.W(stop, "then", eout, "done")

# ---- Leaderboard Display (Top Players + Show Info Window) --------------------------------
if all(t in NODES_OK for t in ("save.top", "msg.info_window")):
    it = item("st_leaderboard", "Leaderboard Display", "Shows a player the top 10 of a saved score (Key, e.g. kills) in an info window.", "Starter: rounds and scoring",
              "Save the score with Add To Saved (the same Key) as players earn it, then call this from a say command, a key or the round's end. Bots are on it like anyone; they don't open it.")
    TXT = it.var("lb_text", "Leaderboard text", "text", "mode", "")
    e = it.function("lb_show", "Show Leaderboard", [("who", "Player", "player")], root=True)
    s0 = it.N("clear", "var.set", var=TXT, value=""); it.W(e, "then", s0, "exec")
    top = it.N("top", "save.top", key="kills", order="high", count=10); it.W(s0, "then", top, "exec")
    g = it.get("g", TXT, row=3)
    line = it.N("line", "text.format", row=3, text="[0][1]. [2]: [3][br]"); it.W(g, "out", line, "a0"); it.W(top, "rank", line, "a1"); it.W(top, "name", line, "a2"); it.W(top, "value", line, "a3")
    s1 = it.N("add", "var.set", var=TXT); it.W(top, "each", s1, "exec"); it.W(line, "out", s1, "value")
    g2 = it.get("g2", TXT, row=4)
    win = it.N("win", "msg.info_window", title="Top players", text=""); it.W(top, "then", win, "exec"); it.W(e, "who", win, "player"); it.W(g2, "out", win, "more")
    r = it.ret(); it.W(win, "then", r, "exec")
    it.note("Show Leaderboard: the Key is Top Players' setting (kills); change it to the key your mode saves.")
else:
    print("waiting: st_leaderboard needs save.top, msg.info_window")

# ---- Capture Zone -----------------------------------------------------------------------
it = item("st_capture", "Capture Zone", "A capture point at an entity that a team takes by standing in it for 10 seconds; bots are sent to take it.", "Starter: rounds and scoring",
          "Make Capture Zone at an entity; Point Captured runs when a team takes it (end the round there, or score). The time and size are the Create Component keys inside (time=10, radius=160): change them there. Bots: the zone is a capture goal for every bot.")
e = it.function("cz_make", "Make Capture Zone", [("at", "At", "entity")], [("zone", "Zone", "entity")], root=True)
c = it.N("make", "comp.create", kind="ls_capture_point", at="", keys="time=10|radius=160"); it.W(e, "then", c, "exec"); it.W(e, "at", c, "at_ent")
bg = it.N("bots", "bots.goal", kind="capture", prio=60, secs=3600, radius=160); it.W(c, "then", bg, "exec"); it.W(c, "out", bg, "at")
r = it.ret(); it.W(bg, "then", r, "exec"); it.W(c, "out", r, "zone")

# =========================================================================================
# Spectacle
# =========================================================================================

it = item("st_kill_cam", "Kill Cam", "Shows the victim the killer for Seconds (0: 3), from behind the killer, then back.", "Starter: spectacle",
          "Wire Player Killed in (Victim, Killer). Nothing for a suicide or a fall. Bots get it too (it doesn't change what they do).")
ein, eout = it.macro("kc_show", "Kill Cam", [("go", "In", "exec"), ("victim", "Victim", "player"), ("killer", "Killer", "player"), ("secs", "Seconds", "number")], [("next", "Then", "exec")], root=True)
same = it.N("same", "logic.equal", row=2, of="player", op="ne"); it.W(ein, "victim", same, "a"); it.W(ein, "killer", same, "b")
ok = it.N("valid", "player.is_alive", row=2); it.W(ein, "killer", ok, "player")
both = it.N("both", "logic.and_or", row=2, op="and"); it.W(same, "out", both, "a"); it.W(ok, "out", both, "b")
br = it.N("br", "flow.branch"); it.W(ein, "go", br, "exec"); it.W(both, "out", br, "cond")
v = it.N("view", "fun.view_from", who="player", secs=3, offset=96, move=2, freeze=False); it.W(br, "true", v, "exec"); it.W(ein, "victim", v, "player")
it.W(v, "then", eout, "next"); it.W(br, "false", eout, "next")
kpos = it.N("kpos", "vec.player", row=3, what="eyes"); it.W(ein, "killer", kpos, "player")
it.W(kpos, "out", v, "to")
vp = it.N("vpos", "vec.player", row=4, what="eyes"); it.W(ein, "victim", vp, "player"); it.W(vp, "out", v, "pos")

it = item("st_victory_cam", "Victory Camera", "Everyone looks at the winner for Seconds (0: 5) at the end of a round.", "Starter: spectacle",
          "Wire it where your mode picks a winner (before End Round). Winner is the player to show. Bots' views move too.")
ein, eout = it.macro("vc_show", "Victory Camera", [("go", "In", "exec"), ("winner", "Winner", "player"), ("secs", "Seconds", "number")], [("next", "Then", "exec")], root=True)
wp = it.N("wpos", "vec.player", row=2, what="eyes"); it.W(ein, "winner", wp, "player")
up = it.N("above", "vec.make", row=3, x=0, y=0, z=64)
cam = it.N("cam", "vec.op", row=3, op="add"); it.W(wp, "out", cam, "a"); it.W(up, "out", cam, "b")
v = it.N("view", "fun.view_from", who="all", secs=5, offset=128, move=3, freeze=True); it.W(ein, "go", v, "exec"); it.W(cam, "out", v, "pos"); it.W(wp, "out", v, "to")
txt = it.N("txt", "text.format", row=4, text="[0] wins!"); it.W(ein, "winner", txt, "a0")
h = it.N("hud", "msg.hud", slot="center", seconds=5, text=""); it.W(v, "then", h, "exec"); it.W(txt, "out", h, "value")
it.W(h, "then", eout, "next")

# ---- Hot Potato -------------------------------------------------------------------------
it = item("st_hot_potato", "Hot Potato", "A bomb one player holds for Seconds (0: 20): a hit passes it on, and when time runs out it blows up its holder.", "Starter: spectacle",
          "Give Potato to start (a random player); wire Player Hurt into Pass Potato (Attacker, Victim). Bots: everyone else hunts the holder, and the holder runs (a bot holder follows the farthest player away for a few seconds).")
HOLD = it.var("hp_holder", "Potato holder", "player", "mode")
LEFTV = it.var("hp_left", "Potato left", "number", "mode", 0)
e = it.function("hp_give", "Give Potato", [("who", "Player", "player"), ("secs", "Seconds", "number")], root=True)
sc = it.default("secs", e, "secs", 20)
s1 = it.N("hold", "var.set", var=HOLD); it.W(e, "then", s1, "exec"); it.W(e, "who", s1, "value")
s2 = it.N("left", "var.set", var=LEFTV); it.W(s1, "then", s2, "exec"); it.W(sc, "out", s2, "value")
mark = it.N("mark", "fn.call", func="hp_mark"); it.W(s2, "then", mark, "exec"); it.W(e, "who", mark, "who")
r = it.ret(); it.W(mark, "then", r, "exec")
e = it.function("hp_mark", "Mark Potato Holder", [("who", "Player", "player")])
gl = it.N("glow", "player.set_glow", color="red"); it.W(e, "then", gl, "exec"); it.W(e, "who", gl, "player")
txt = it.N("txt", "text.format", row=3, text="[0] has the hot potato!"); it.W(e, "who", txt, "a0")
h = it.N("hud", "msg.hud", slot="top", seconds=3, text=""); it.W(gl, "then", h, "exec"); it.W(txt, "out", h, "value")
clr = it.N("clear", "bots.goal_clear", kind="0", who="all"); it.W(h, "then", clr, "exec")
hunt = it.N("hunt", "bots.goal", who="all", kind="hunt", prio=70, secs=3600, radius=64); it.W(clr, "then", hunt, "exec"); it.W(e, "who", hunt, "at")
# the holder runs: a bot holder follows whoever is farthest from it for a while
isb = it.N("isb", "player.is_bot", row=4); it.W(e, "who", isb, "player")
brb = it.N("brb", "flow.branch"); it.W(hunt, "then", brb, "exec"); it.W(isb, "out", brb, "cond")
far = it.N("far", "flow.pick_player", which="alive"); it.W(brb, "true", far, "exec")
fol = it.N("follow", "bot.follow", secs=6, influence=20); it.W(far, "then", fol, "exec"); it.W(e, "who", fol, "bot"); it.W(far, "player", fol, "who")
r = it.ret(); it.W(fol, "then", r, "exec"); it.W(brb, "false", r, "exec"); it.W(far, "none", r, "exec")
e = it.function("hp_pass", "Pass Potato", [("from", "Attacker", "player"), ("to", "Victim", "player")], [("passed", "Passed", "bool")])
h0 = it.get("h0", HOLD)
is_h = it.N("ish", "logic.equal", row=2, of="player", op="eq"); it.W(e, "from", is_h, "a"); it.W(h0, "out", is_h, "b")
br = it.N("br", "flow.branch"); it.W(e, "then", br, "exec"); it.W(is_h, "out", br, "cond")
off = it.N("off", "player.set_glow", color="off"); it.W(br, "true", off, "exec"); it.W(e, "from", off, "player")
s1 = it.N("hold", "var.set", var=HOLD); it.W(off, "then", s1, "exec"); it.W(e, "to", s1, "value")
mk = it.N("mark", "fn.call", func="hp_mark"); it.W(s1, "then", mk, "exec"); it.W(e, "to", mk, "who")
r = it.ret(); it.W(mk, "then", r, "exec"); it.W(is_h, "out", r, "passed")
r2 = it.N("ret2", "fn.return", row=1); it.pins[r2] = ({"exec", "passed"}, set()); it.W(br, "false", r2, "exec"); it.W(is_h, "out", r2, "passed")
e = it.function("hp_tick", "Potato Tick", [], [("boom", "Blew up", "bool")])
dec = it.N("dec", "var.add", var=LEFTV, amount=-1); it.W(e, "then", dec, "exec")
lv = it.get("lv", LEFTV, row=2)
z = it.N("zero", "math.compare", row=2, op="le", b=0); it.W(lv, "out", z, "a")
br = it.N("br", "flow.branch"); it.W(dec, "then", br, "exec"); it.W(z, "out", br, "cond")
h1 = it.get("h1", HOLD, row=3)
alive = it.N("alive", "player.is_alive", row=3); it.W(h1, "out", alive, "player")
br2 = it.N("br2", "flow.branch"); it.W(br, "true", br2, "exec"); it.W(alive, "out", br2, "cond")
pos = it.N("pos", "vec.player", row=4); it.W(h1, "out", pos, "player")
boom = it.N("boom", "fx.explosion", dmg=500, radius=180); it.W(br2, "true", boom, "exec"); it.W(pos, "out", boom, "pos")
off = it.N("off", "player.set_glow", color="off"); it.W(boom, "then", off, "exec"); it.W(h1, "out", off, "player")
clr = it.N("clear", "bots.goal_clear", kind="0", who="all"); it.W(off, "then", clr, "exec")
r = it.ret(); it.W(clr, "then", r, "exec"); it.W(z, "out", r, "boom")
r2 = it.N("ret2", "fn.return", row=1); it.pins[r2] = ({"exec", "boom"}, set()); it.W(br, "false", r2, "exec"); it.W(br2, "false", r2, "exec"); it.W(z, "out", r2, "boom")
it.note("Potato Tick: call it every second (a Repeat from Round Started). Blew up is Yes on the tick it went off; give a new potato then.")

# =========================================================================================
# Story
# =========================================================================================

it = item("st_quest", "Quest Step Tracker", "Each player's quest step (a word you choose, e.g. find_fuse) with a HUD line; pairs with Run Dialog's Last row.", "Starter: story",
          "Set Quest Step when a talk ends (Run Dialog's Last row) or a task is done; Quest Step Of tells where a player is (empty: not started). Quest Done marks the end. Bots never start quests.")
STEP = it.var("qs_step", "Quest step", "text", "player", "")
e = it.function("qs_set", "Set Quest Step", [("who", "Player", "player"), ("step", "Step", "text"), ("say", "Objective", "text")], root=True)
s = it.N("set", "var.set", var=STEP); it.W(e, "then", s, "exec"); it.W(e, "who", s, "player"); it.W(e, "step", s, "value")
empty = it.N("empty", "text.equals", row=3, b=""); it.W(e, "say", empty, "a")
br = it.N("br", "flow.branch"); it.W(s, "then", br, "exec"); it.W(empty, "out", br, "cond")
txt = it.N("txt", "text.format", row=4, text="Objective: [0]"); it.W(e, "say", txt, "a0")
h = it.N("hud", "msg.hud", slot="left", seconds=6, text=""); it.W(br, "false", h, "exec"); it.W(e, "who", h, "player"); it.W(txt, "out", h, "value")
snd = it.N("snd", "fx.sound_to", sound="buttons/blip1.wav"); it.W(h, "then", snd, "exec"); it.W(e, "who", snd, "player")
r = it.ret(); it.W(snd, "then", r, "exec"); it.W(br, "true", r, "exec")
e = it.function("qs_of", "Quest Step Of", [("who", "Player", "player")], [("step", "Step", "text"), ("started", "Started", "bool")], pure=True)
g = it.get("g", STEP, (e, "who"))
empty = it.N("empty", "text.equals", row=3, b=""); it.W(g, "out", empty, "a")
nt = it.N("not", "logic.not", row=3); it.W(empty, "out", nt, "a")
r = it.ret(); it.W(e, "then", r, "exec"); it.W(g, "out", r, "step"); it.W(nt, "out", r, "started")
e = it.function("qs_done", "Quest Done", [("who", "Player", "player"), ("reward", "Reward", "number")])
s = it.N("set", "var.set", var=STEP, value="done"); it.W(e, "then", s, "exec"); it.W(e, "who", s, "player")
m = it.N("money", "player.add_money"); it.W(s, "then", m, "exec"); it.W(e, "who", m, "player"); it.W(e, "reward", m, "amount")
h = it.N("hud", "msg.hud", slot="center", seconds=4, text="Quest complete!"); it.W(m, "then", h, "exec"); it.W(e, "who", h, "player")
r = it.ret(); it.W(h, "then", r, "exec")


# =========================================================================================
# the test mode: every premade wired to the events a mode would use (codegen, amxxpc, the sim)
# =========================================================================================
class Skip(tuple):
    """A premade left out: its pins are the flow it would have followed."""
    def __new__(cls, after):
        return tuple.__new__(cls, after if after else (None, None))


def test_mode(part):
    """part a: movement, abilities, pickups, Hot Potato, Countdown; b: the rest (a graph holds 400 nodes)"""
    A = ("st_jump_pad", "st_teleporter", "st_dash", "st_double_jump", "st_ability", "st_fireball", "st_mine", "st_health_pack", "st_weapon_pick", "st_hot_potato", "st_countdown")
    mine = [it for it in ITEMS if (it.file in A) == (part == "a") and not it.missing]
    g = {"schema": "loner-strike/graph", "version": 3, "mode": {"id": f"starter_test_{part}", "name": f"Starter library test {part.upper()}", "description": "Starter library premades at once"},
         "variables": [], "functions": [], "macros": [], "nodes": [], "edges": [], "comments": []}
    for it in mine:
        v = it.json()
        for k in ("functions", "macros", "variables", "nodes"):
            g[k] += v[k]
        pre = it.file[3:]
        g["edges"] += [dict(w, id=f"{pre}_{w['id']}") for w in v["edges"]]
        g["comments"] += [dict(c, id=f"{pre}_{c['id']}") for c in v["comments"]]
    for k in ("functions", "macros", "variables", "nodes", "edges", "comments"):
        ids = [x["id"] for x in g[k]]
        assert len(ids) == len(set(ids)), (k, [i for i in ids if ids.count(i) > 1][:5])
    fns = {f["id"]: f for f in g["functions"]}
    mac = {m["id"]: m for m in g["macros"]}
    ev = {"n": 0, "y": 0}

    def N(type, **params):
        if type not in NODES_OK:
            return None
        ev["n"] += 1
        nid = f"ev{ev['n']}"
        g["nodes"].append({"id": nid, "type": type, "pos": {"x": 300 * (ev["n"] % 8), "y": ev["y"]}, "params": params})
        return nid

    def W(a, ap, b, bp):
        if isinstance(a, Skip):
            a, ap = a[0], a[1]
        if a is None or b is None:
            return
        g["edges"].append({"id": f"ev_w{len(g['edges'])}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})

    def call(f, after, **ins):
        # (a premade that isn't here, or nothing before it: skipped, the flow goes on past it)
        if after is not None and isinstance(after[0], Skip):
            after = tuple(after[0])
        if after is None or after[0] is None or any(v is None or v[0] is None for v in ins.values()) or (f not in fns and f not in mac):
            return Skip(after)
        if f in fns:
            c = N("fn.call", func=f)
            W(after[0], after[1], c, "exec")
            for pin, src in ins.items():
                W(src[0], src[1], c, pin)
            return (c, "then")
        c = N("macro.use", macro=f)
        W(after[0], after[1], c, "go")
        for pin, src in ins.items():
            W(src[0], src[1], c, pin)
        return c

    def lit(v):
        if isinstance(v, str):
            return (N("text.literal", value=v), "out")
        return (N("math.number", value=v), "out")

    def row():
        ev["y"] += 360

    rs = N("event.round_started")
    if part == "b":
        # a round: lives, the quest, a capture zone; spawns, kills, hurts, shots, the end of it
        each = N("flow.for_each_player", which="all"); W(rs, "then", each, "exec")
        c = call("lv_reset", (each, "each"), who=(each, "player"), n=lit(3))
        c = call("qs_set", c, who=(each, "player"), step=lit("survive"), say=lit("Survive the round"))
        pk = N("flow.pick_entity", by="class", name="info_player_deathmatch"); W(each, "then", pk, "exec")
        call("cz_make", (pk, "then"), at=(pk, "entity"))
        return part_b(g, N, W, call, lit, row)
    # a round: pickups at spawn points, a jump pad, a teleporter pad, the countdown, the potato, a mine
    seq = N("flow.sequence", count=2); W(rs, "then", seq, "exec")
    prev = (seq, "then0")
    for f in ("heal_spawn", "weap_spawn", "jp_make"):
        pk = N("flow.pick_entity", by="class", name="info_player_deathmatch"); W(prev[0], prev[1], pk, "exec")
        if f in fns:
            prev = call(f, (pk, "then"), at=(pk, "entity"))
        else:
            m = call(f, (pk, "then"), at=(pk, "entity")); prev = (m, "next")
    if "vol.spawn" in NODES_OK and "st_teleporter" in [it.file for it in mine]:
        pk = N("flow.pick_entity", by="class", name="info_player_start"); W(prev[0], prev[1], pk, "exec")
        tpv = N("vol.spawn", shape="sphere", radius=64, name="tp"); W(pk, "then", tpv, "exec")
        tpos = N("vec.entity"); W(pk, "entity", tpos, "in"); W(tpos, "out", tpv, "pos")
        prev = (tpv, "then")
    m = call("cd_run", prev, secs=lit(120), label=lit("Round"))
    pp = N("flow.pick_player", which="alive"); W(m, "started", pp, "exec")
    prev = call("hp_give", (pp, "then"), who=(pp, "player"), secs=lit(25))
    pb = N("flow.pick_player", which="bots"); W(prev[0], prev[1], pb, "exec")
    call("mine_place", (pb, "then"), who=(pb, "player"), radius=lit(0), dmg=lit(0))
    row()
    # every second: the potato, and bots throwing fireballs when an enemy is near
    rep = N("timer.repeat", every=1); W(seq, "then1", rep, "exec")
    tick = N("flow.sequence", count=2); W(rep, "tick", tick, "exec")
    call("hp_tick", (tick, "then0"))
    bots = N("flow.for_each_player", which="bots"); W(tick, "then1", bots, "exec")
    c = call("ab_bot", (bots, "each"), bot=(bots, "player"), chance=lit(0), range=lit(0))
    br = N("flow.branch"); W(c[0], "then", br, "exec"); W(c[0], "yes", br, "cond")
    ab = call("ab_use", (br, "true"), who=(bots, "player"), cd=lit(6))
    call("fb_throw", (ab, "use"), who=(bots, "player"), dmg=lit(0), speed=lit(0))
    # (an enemy near but the fireball not ready: a dash now and then; in the air: a double jump)
    call("dash_go", (ab, "wait"), who=(bots, "player"), speed=lit(0), cd=lit(4))
    call("dj_jump", (br, "false"), who=(bots, "player"), boost=lit(0))
    row()
    # the volumes
    for name, f, extra in [] if "vol.on_begin" not in NODES_OK else [("heal", "heal_take", {"heal": lit(50), "respawn": lit(10)}), ("weap", "weap_take", {"weapon": lit("awp"), "respawn": lit(15)}),
                           ("jumppad", "jp_launch", {"speed": lit(0)}), ("mine", "mine_step", {})]:
        b = N("vol.on_begin", name=name)
        ins = dict(extra)  # (part a)
        if f in fns:
            ins.update({"who": (b, "who")} if f == "jp_launch" else {"mine": (b, "volume"), "who": (b, "who")})
            call(f, (b, "then"), **ins)
        else:
            ins.update({"vol": (b, "volume"), "who": (b, "who")})
            call(f, (b, "then"), **ins)
        row()
    h = N("event.player_hurt")
    call("hp_pass", (h, "then"), **{"from": (h, "attacker"), "to": (h, "victim")})
    if "vol.on_begin" in NODES_OK and "tp_go" in fns:
        b = N("vol.on_begin", name="tp")
        pk = N("flow.pick_entity", by="class", name="info_player_deathmatch"); W(b, "then", pk, "exec")
        call("tp_go", (pk, "then"), who=(b, "who"), to=(pk, "entity"), cd=lit(0))
    return g


def part_b(g, N, W, call, lit, row):
    row()
    # spawns, kills, hurts, shots, the end of the round
    sp = N("event.player_spawned")
    call("sp_give", (sp, "then"), who=(sp, "player"), secs=lit(0))
    fired = N("event.weapon_fired")
    call("sp_end", (fired, "then"), who=(fired, "player"))
    row()
    k = N("event.player_killed")
    c = call("ks_kill", (k, "then"), killer=(k, "attacker"), victim=(k, "victim"))
    c = call("lv_lose", c, who=(k, "victim"))
    m = call("kc_show", c, victim=(k, "victim"), killer=(k, "attacker"), secs=lit(2))
    call("qs_done", (m, "next"), who=(k, "attacker"), reward=lit(100))
    row()
    h = N("event.player_hurt")
    hs = N("flow.branch"); W(h, "then", hs, "exec"); W(h, "headshot", hs, "cond")
    call("st_freeze", (hs, "true"), who=(h, "victim"), secs=lit(1))
    m = call("st_slow", (hs, "false"), who=(h, "victim"), secs=lit(1), speed=lit(200))
    knife = N("text.equals", b="knife"); W(h, "weapon", knife, "a")
    kb = N("flow.branch"); W(m, "next", kb, "exec"); W(knife, "out", kb, "cond")
    call("st_burn", (kb, "true"), who=(h, "victim"), by=(h, "attacker"), secs=lit(3), dps=lit(4))
    row()
    up = N("event.time_up")
    pw = N("flow.pick_player", which="alive"); W(up, "then", pw, "exec")
    call("vc_show", (pw, "then"), winner=(pw, "player"), secs=lit(3))
    return g


# =========================================================================================
# write
# =========================================================================================
def main():
    os.makedirs(OUT, exist_ok=True)
    for it in [x for x in ITEMS if x.missing]:
        print(f"waiting: {it.file} needs {', '.join(sorted(it.missing))}")
    # (a premade not made this time leaves no file behind)
    made = {x.file for x in ITEMS if not x.missing}
    for f in os.listdir(OUT):
        if f.startswith("st_") and f.endswith(".lib.json") and f[:-9] not in made:
            os.remove(os.path.join(OUT, f))
    for it in [x for x in ITEMS if not x.missing]:
        v = it.json()
        assert len(it.name) <= 40 and len(it.desc) <= 160, (it.name, len(it.desc))
        assert len("builtin_" + it.file) <= 24, it.file
        with open(os.path.join(OUT, it.file + ".lib.json"), "w") as f:
            json.dump(v, f, indent=1)
            f.write("\n")
        print(f"{it.file}: {len(it.nodes)} nodes, {len(it.edges)} wires")
    for part in "ab":
        with open(os.path.join(HERE, "testdata", f"starter_library_{part}.graph.json"), "w") as f:
            json.dump(test_mode(part), f, indent=1)
            f.write("\n")
    # the app's own list (mod.rs api_library adds these; MCP's studio_library reads it)
    names = sorted([f[:-9] for f in os.listdir(OUT) if f.endswith(".lib.json")])
    with open(os.path.join(HERE, "builtin_library.rs"), "w") as f:
        f.write("// Written by make_starter.py: the library items that ship with the app (ui/.../studio/library).\n")
        f.write("pub const BUILTIN: &[(&str, &str)] = &[\n")
        for n in names:
            f.write(f'    ("{n}", include_str!("../../../../ui/src/views/tools/studio/library/{n}.lib.json")),\n')
        f.write("];\n")


if __name__ == "__main__":
    main()
