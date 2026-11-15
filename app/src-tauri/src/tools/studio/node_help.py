# The node help (the Studio's "?" panel and F1 on a picked node, and MCP's node list): for
# every node in the registry, what it does, each pin and setting, a small example that drops
# into a graph, related nodes, and a cost or risk note where one matters.
#
# Most of it is worked out from the registry itself, so a new node gets help the moment it's
# added; the hand-written notes below add what can't be worked out (a longer explanation, a
# pin's meaning, the cost). make_nodes.py calls write() after it writes nodes.json; to remake
# only the help: python3 node_help.py ../../../../ui/src/views/tools/studio/nodes.json
#
# Every example is checked by the app's own quick check (help_tests.rs), so none can go stale.
import json, os, sys

TYPE_WORDS = {
    "exec": "flow", "player": "a player", "entity": "an entity (a thing in the map)", "team": "a team",
    "number": "a number", "bool": "yes or no", "text": "text", "vector": "a position or direction (X, Y, Z)",
    "timer": "a timer", "any": "any value", "var": "the variable's type",
    "$T": "the type set in Holds", "$K": "the key type set in Keys", "$E": "the enum's value", "$S": "the struct",
    "$F": "the field's type",
}

def type_words(t):
    if t in TYPE_WORDS:
        return TYPE_WORDS[t]
    if t.startswith("array<"):
        return "a list of " + type_words(t[6:-1]).replace("a ", "", 1).replace("an ", "", 1) + "s"
    if t.startswith("map<"):
        return "a map (keys to values)"
    return t

# ---- hand-written notes ----------------------------------------------------------------------
# more: a longer explanation; pins: {pin id: meaning}; cost / risk: a short note;
# related: extra node types; caption: what the example shows
NOTES = {
    "event.round_started": {"more": "The usual place to set a round up: give weapons, reset scores, show a message. It runs once per round for the whole server, not per player (use For Each Player for that).", "related": ["event.new_round", "event.freeze_ended", "flow.for_each_player"]},
    "event.new_round": {"more": "Runs as the new round is being set up, before freeze time. Players are respawning, so per-player work is better done in Player Spawned."},
    "event.round_ended": {"pins": {"winner": "The team that won; no team for a draw.", "round": "How many rounds have been played on this map, counting this one."}},
    "event.player_killed": {
        "more": "Runs once for every death. Attacker is nobody when the world did it (a fall, the bomb, a trigger_hurt); check with Is Valid before giving the attacker anything.",
        "pins": {"victim": "Who died.", "attacker": "Who killed them; nobody for the world, and the victim themselves for a suicide.", "headshot": "Yes when the killing shot hit the head.", "weapon": "The weapon's short name, as the kill feed shows it: ak47, knife, grenade, world.", "distance": "How far apart they were, in units (a player is 72 units tall).", "team_kill": "Yes when the attacker shot a teammate.", "suicide": "Yes when the victim killed themselves (a grenade, the Kill node, the kill command).", "world": "Yes when no player did it."},
        "related": ["event.player_hurt", "player.add_frags", "player.respawn"],
    },
    "event.player_hurt": {
        "more": "Runs after the damage is done, with what was really taken: nothing when it was blocked, less when armor took some. To change the damage before it lands, use Player About To Be Hurt.",
        "pins": {"damage": "Health taken by this hit.", "left": "Health left after it (0 or less: the hit killed them).", "headshot": "Yes when a bullet or the knife hit the head.", "hitgroup": "Where a bullet hit: head, chest, stomach, arm, leg; body for anything else (grenades, falls).", "weapon": "The weapon that did it: the attacker's gun or knife, hegrenade for a grenade, empty when no player did it."},
        "cost": "Runs on every hit, often many times a second in a firefight. Keep what follows it small.",
        "related": ["event.player_about_hurt", "event.player_killed"],
    },
    "event.player_about_hurt": {"more": "Runs before damage lands, so Damage nodes after it can scale or block it.", "cost": "Runs on every hit. Keep what follows it small."},
    "event.player_spawned": {"more": "Runs each time a player comes alive: every round, and after Respawn. The place for loadouts and per-player setup.", "related": ["player.give_weapon", "player.strip", "player.set_health"]},
    "event.player_joined": {"more": "Runs once when someone (or a bot) has joined, before they pick a team. Per-player variables start fresh here."},
    "event.player_left": {"more": "Runs as a player leaves. Their slot can be taken by someone else right after, so tidy up their variables here."},
    "event.weapon_fired": {"cost": "Runs on every shot (at most ten times a second per player). Keep what follows it small, or a busy server slows down."},
    "event.player_touched": {"cost": "Runs at most once a second per player, so a trigger can't flood the mode."},
    "event.chat_command": {"pins": {"args": "Whatever was typed after the command: for /give 5, the text 5."}},
    "event.player_said": {"cost": "Runs on every chat line. Fine for a few checks."},
    "flow.for_each_player": {"more": "Runs Each once for every player that matches, then Then. The Player pin is the current one inside Each.", "cost": "At most 32 players, so it's cheap. Inside a Repeat that ticks often, it adds up."},
    "flow.for_each_entity": {"cost": "Searches the whole map each time it runs. Fine at a round's start; in a fast Repeat it can slow the server."},
    "flow.for_loop": {"cost": "Each step runs everything wired to Body. Thousands of steps in one go can stall the server for a moment."},
    "flow.while": {"more": "Repeats Body while the condition is true. Limit stops it after that many times, so a condition that never turns false can't freeze the server.", "risk": "A loop that never ends is stopped by Limit; the Problems panel says when that happened."},
    "timer.delay": {"more": "Waits, then goes on. Other things keep running while it waits. Running it again before it's done starts another wait."},
    "timer.repeat": {"more": "Runs Tick every so many seconds, Times times (0: until the round ends or you stop it). Then runs right away.", "cost": "A short interval with a lot after Tick is the most common way a mode gets heavy. Half a second or more is usually plenty."},
    "timer.retrigger": {"more": "Like Delay, but running it again starts the wait over, so Then runs once things go quiet."},
    "timer.countdown": {"more": "Shows a countdown in the HUD each second, then runs Done."},
    "ent.find_all": {"cost": "Searches the whole map. Find once (at a round's start) and keep the list in a variable instead of finding again and again."},
    "ent.spawn_prop": {"more": "Puts a model in the world at an entity's place. The model is loaded when the map loads, so it must be a file the server and players have.", "risk": "Each prop is an entity; the game has a limit (about 900). Remove props you don't need any more."},
    "ent.remove": {"risk": "Removing map entities the game needs (spawns, bomb sites, buy zones) can break a round."},
    "fx.explosion": {"pins": {"dmg": "How much it hurts players near it (0: only the look and sound)."}, "risk": "With damage, it can kill players, and those count as deaths by the world."},
    "fx.lights": {"more": "Changes the light of the whole map for everyone, until set back to Normal or the map changes."},
    "game.run_command": {"risk": "Only gameplay settings and a short list of safe commands are allowed; anything else is refused and the node's OK is No. A community mode that has this shows \"runs server commands\".", "related": ["game.set_cvar", "game.amx_command"]},
    "game.amx_command": {"risk": "Only slap, slay, kick, say commands, map changes to real maps and allowed settings are let through. A community mode with this shows \"runs AMX commands\"."},
    "amx.call": {"risk": "Calls into another plugin. If that plugin or function isn't there, OK is No and nothing happens. A community mode with this shows \"calls other plugins\"."},
    "player.kick": {"risk": "Takes the player off the server. A community mode with this shows \"kicks players\", so use it only when the mode needs it."},
    "game.change_map": {"risk": "Ends the game on this map for everyone. Only maps the server has are allowed."},
    "game.restart_round": {"risk": "Restarts the round for everyone, like sv_restartround."},
    "player.respawn": {"more": "Brings a dead player back at a spawn point. Nothing happens if they're alive or a spectator."},
    "player.kill": {"more": "Kills the player as a suicide (Player Killed sees Suicide)."},
    "player.set_speed": {"more": "The game sets speed again when a player switches weapons, so set it after Switch To Weapon, or on a Repeat, to keep it."},
    "player.set_team": {"more": "Moves the player to the other team at once, without killing them."},
    "player.strip": {"more": "Takes every weapon away; keeps the knife unless you turn Knife off."},
    "player.give_weapon": {"more": "Gives the weapon with reserve ammo. A player who already has one in that slot gets it as well; Strip first for a clean loadout."},
    "game.set_cvar": {"more": "A setting changed for the rest of the map. For settings the mode always wants, add them under This mode's Game settings instead (set every map).", "risk": "A few settings are set again by the game's own game.cfg after the mode's, so they don't stick: the bomb for the Terrorists (mp_give_player_c4) is one. Take the bomb with Remove Weapon (c4) at Round Started instead."},
    "player.take_weapon": {"more": "Takes one weapon away; the C4 choice takes the bomb (a mode without bomb sites should, at Round Started: the bomb can't be turned off with a setting)."},
    "table.get_row": {"more": "Data tables (My mode > Data tables) are grids of values you type or paste from a spreadsheet: a gun list, prices, waves. Get Row finds the row whose key column has this value and gives every cell of it as a pin.", "pins": {"key": "The value to look for in the key column.", "found": "Yes when a row has that key.", "row": "The row's number (from 0), or -1 when none has the key."}, "related": ["table.get_cell", "table.for_each", "table.find"]},
    "table.get_cell": {"more": "Reads one cell. Rows count from 0, so the first row is 0; a row past the end gives 0, No or empty text.", "related": ["table.get_row", "table.count"]},
    "table.for_each": {"more": "Runs Each for every row in order, with that row's cells as pins (and Row, its number). Then runs once after the last.", "cost": "A table has at most 256 rows, and reading one costs almost nothing: the plugin holds tables as fixed lists."},
    "table.find": {"more": "Every row (as row numbers, in order) where the column you pick has this value. Read their cells with Get Cell, or loop over them with For Each.", "related": ["table.get_cell", "flow.for_each_array"]},
    "timeline.play": {"more": "Each key says how the track goes to the next: Linear, Constant (a step), Ease In, Ease Out, Ease In-Out, or Custom, where you drag the key's tangent handles to shape the curve (Alt drags one side; Auto tangents puts them back). The plugin works the curves out with a few multiplications a tick, no tables.", "cost": "A Timeline in a Function with a Player input runs once per player: up to 32 copies, each ticking 20 times a second at the default Tick."},
    "player.to_spectators": {"more": "Moves the player to the spectators. They choose a team again to come back."},
    "var.set": {"more": "Sets a variable. A per-player variable needs a Player wired in, a per-team one a Team."},
    "msg.hud": {"more": "Shows a line of text on the player's screen (everyone's when no player is wired in) for a few seconds. Each of the four slots holds one line, so a new message in the same slot replaces the old one."},
    "msg.broadcast": {"more": "Shows a big message in the middle of the screen."},
    "debug.print": {"more": "Writes a line to the server console (and the chat, unless Chat is off). Handy while you build: Test run's log shows these lines."},
    "comp.create": {"risk": "Each component is an entity; remove the ones you're done with."},
}

# getters and setters that belong together, worked out from the names
def pairs(types):
    rel = {}
    for t in types:
        cat, name = t.split(".", 1)
        for cand in (f"{cat}.set_{name}", f"{cat}.{name[4:]}" if name.startswith("set_") else None, f"{cat}.get_{name[4:]}" if name.startswith("set_") else None, f"{cat}.set_{name[4:]}" if name.startswith("get_") else None):
            if cand and cand in types and cand != t:
                rel.setdefault(t, []).append(cand)
    return rel

# ---- examples -------------------------------------------------------------------------------
def make_example(d, defs):
    """A small piece of graph that uses the node: an event to start it, its needs wired in, and
    a Print String for what it gives. None where the node needs the mode's own pieces (its
    variables, Functions, structs, enums, markers)."""
    special = {"var", "varvalue", "func", "timer", "fn", "macro", "struct", "enum", "field", "enumvalue", "marker", "cevent", "tline", "table", "column"}
    if any(p["kind"] in special for p in d["params"]) or d.get("reroute"):
        return None
    if d["type"].startswith(("fn.", "macro.", "struct.", "enum.", "func.")):
        return None
    if any(p["type"].startswith("$") or "<$" in p["type"] for p in d["inputs"] + d["outputs"]):
        return None
    nodes, edges = [], []
    def N(id, type, x, y, params=None):
        nodes.append({"id": id, "type": type, "pos": {"x": x, "y": y}, "params": params or {}})
    def W(a, ap, b, bp):
        edges.append({"id": f"w{len(edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
    ins = [p for p in d["inputs"] if p["type"] != "exec"]
    need = lambda p: p.get("required") or (p["type"] in ("player", "entity", "vector") and not p.get("optional") and not p.get("param"))
    has_exec = any(p["type"] == "exec" for p in d["inputs"])
    wants_player = any(p["type"] == "player" and need(p) for p in ins)
    x_node = 340
    if d.get("event"):
        N("it", d["type"], 0, 0)
        start = ("it", "then") if any(o["id"] == "then" for o in d["outputs"]) else None
    else:
        ev = "event.player_spawned" if wants_player else "event.round_started"
        if d["type"].startswith("dmg."):
            ev = "event.player_about_hurt"
        N("ev", ev, 0, 0)
        N("it", d["type"], x_node, 0, {p["id"]: p["default"] for p in d["params"]})
        start = ("ev", "then")
    srcs = {}
    if not d.get("event"):
        y = 160
        for p in ins:
            if not need(p):
                continue
            t = p["type"]
            if t == "player":
                if wants_player:
                    W("ev", "victim" if ev == "event.player_about_hurt" else "player", "it", p["id"])
            elif t == "entity":
                if "ent" not in srcs:
                    N("ent", "flow.pick_entity" if has_exec else "comp.find", 0, y, {"by": "class", "name": "info_player_deathmatch"} if has_exec else {"name": "spot1"})
                    srcs["ent"] = True
                    y += 120
                W("ent", "entity" if has_exec else "out", "it", p["id"])
            elif t == "vector":
                if "vec" not in srcs:
                    if wants_player:
                        N("vec", "vec.player", 0, y)
                        W("ev", "player", "vec", "player")
                    else:
                        N("vec", "vec.make", 0, y, {"x": 0, "y": 0, "z": 64})
                    srcs["vec"] = True
                    y += 120
                W("vec", "out", "it", p["id"])
            elif t == "text":
                N("txt", "text.literal", 0, y, {"value": "hello"})
                W("txt", "out", "it", p["id"])
                y += 120
            elif t == "number":
                N("num", "math.number", 0, y, {"value": 1})
                W("num", "out", "it", p["id"])
                y += 120
            elif t == "bool":
                N("yes", "logic.and_or", 0, y)
                W("yes", "out", "it", p["id"])
                y += 120
            else:
                return None
    # the flow
    chain = start
    if has_exec and chain:
        if "ent" in srcs and has_exec:
            # Pick Random Entity comes first in the flow
            W(chain[0], chain[1], "ent", "exec")
            chain = ("ent", "then")
        W(chain[0], chain[1], "it", "exec")
        outs = [o["id"] for o in d["outputs"] if o["type"] == "exec"]
        chain = ("it", "then") if "then" in outs else (("it", outs[0]) if outs else None)
    # show the first value it gives
    vals = [o for o in d["outputs"] if o["type"] in ("number", "bool", "text", "player", "team", "vector", "entity")]
    if vals and chain:
        v = vals[0]
        N("show", "debug.print", x_node * 2 if not d.get("event") else x_node, 0, {"text": f"{v.get('label') or d['title']}:", "chat": False})
        W("it", v["id"], "show", "value")
        W(chain[0], chain[1], "show", "exec")
    if len(nodes) < 2:
        return None
    if not has_exec and not vals:
        return None
    if not has_exec and not d.get("event") and not chain:
        return None
    return {"nodes": nodes, "edges": edges}

def caption(d, ex):
    types = [n["type"] for n in ex["nodes"]]
    starts = {"event.player_spawned": "When a player spawns", "event.round_started": "When a round starts", "event.player_about_hurt": "When a player is about to be hurt"}
    first = next((starts[t] for t in types if t in starts), None)
    shows = "show" in [n["id"] for n in ex["nodes"]]
    if d.get("event"):
        return f"{d['title']} prints what it gives to the server console." if shows else f"Runs when this happens."
    acts = any(p["type"] == "exec" for p in d["inputs"])
    if acts:
        s = f"{first}, {d['title']} runs" if first else f"{d['title']} runs"
        return s + (" and its result is printed." if shows else ".")
    return f"{first}, {d['title']} is read and printed to the server console." if first else f"{d['title']} is read and printed."

# ---- settings and pins ----------------------------------------------------------------------
def setting_help(p, pinned):
    k = p["kind"]
    if k == "choice":
        s = "Pick one: " + ", ".join(c[1] for c in p.get("choices", [])[:12]) + ("…" if len(p.get("choices", [])) > 12 else "") + "."
    elif k == "number":
        lo, hi = p.get("min"), p.get("max")
        s = f"A number{f' from {lo:g} to {hi:g}' if lo is not None and hi is not None else ''}."
    elif k == "bool":
        s = "On or off."
    elif k == "text":
        s = "Text: letters, digits, spaces and plain punctuation."
    elif k in ("name", "entname"):
        s = "A name, as the map or the game knows it (letters, digits, _ . -)."
    elif k == "path":
        s = "A file in the game folder, like models/w_c4.mdl or ambience/thunder_clap.wav."
    elif k == "keys":
        s = "Settings for the component, as key=value pairs."
    elif k == "type":
        s = "What kind of values it holds."
    elif k == "cases":
        s = "The cases, separated by commas; each gets its own flow pin."
    else:
        s = "Picked from this mode's own pieces."
    if pinned:
        s += " A wire into the pin of the same name is used instead."
    return s

# HUD & View (engine nodes, Y)
NOTES.update({
    "eng.hide_hud": {"more": "Uses the game's own HUD switches (HideWeapon), so it costs nothing while it's on. The radar, health and armor are one switch in Counter-Strike. A spectator's own HUD isn't touched.", "related": ["eng.fov", "msg.hud"]},
    "eng.fov": {"more": "Scopes still work: a sniper zooms in and goes back to this field of view. Very wide or very narrow views can make people feel sick; keep it between 60 and 120 for long play.", "related": ["eng.hide_hud"]},
    "eng.view_model": {"more": "The model must be a file under models/ (the game's own, like models/v_knife.mdl, or your own). A file that isn't there is left out and the weapon keeps its own model. Only the look changes: damage, speed and sounds are the weapon's.", "risk": "Each model the mode uses is loaded with the map (the game has a limit of 512 models)."},
    "eng.flashlight": {"more": "Don't let them use it also turns it off; Let them use it undoes that. The battery is what the HUD shows; it drains and recharges as usual."},
    "eng.nightvision": {"related": ["eng.flashlight"]},
    "eng.footsteps": {"more": "Only the footstep sounds: jumping and landing still make noise, and bots still hear players who shoot."},
    "eng.score_tag": {"more": "The game sends the tags again when someone dies, spawns or picks up the bomb; this one is put back on each time until it's set to None."},
    "eng.score_color": {"more": "Only how the player is shown changes. The game still treats them as their real team for damage, spawns, buying and winning. Everyone's radar and name tags follow the shown team, and the player's own client does too (their buy menu shows that team's guns, which they can't buy).", "risk": "Players can be confused about who's on which team; say so in the mode's description."},
    "eng.spectate": {"more": "The watcher sees through Target's eyes and can't switch away while it lasts. When Target dies they keep watching the body until Release or a new Target."},
    "fx.beam_ents": {"risk": "Each beam is a temporary effect in every player's game, which has a shared limit; past 24 effects in one server frame the rest are skipped."},
    "fx.icon_over": {"more": "The icon follows the player's head on everyone's screen who sees it. Seen by a team means the team at the moment it's shown. A sprite file that isn't on the server is left out (nothing shows).", "related": ["eng.score_tag", "fx.marker"]},
    "hud.radar_blip": {"more": "The radar only has room for a few dots: four hostage dots (slots 1 to 4) and one dropped-bomb dot. On a hostage map the real hostages use the same kind of dots. The game redraws the bomb dot when the real bomb is dropped or picked up."},
    "hud.team_score": {"more": "Only the number shown: the game keeps its own count of rounds won and shows it again at the next round's end."},
    "world.weather": {"more": "Players with weather effects turned off in their options see nothing; it's only looks (no sounds, no slowdown)."},
    "world.fog": {"more": "OpenGL draws fog; the software renderer doesn't. Thickness 0.001 to 0.005 is a light haze; 0.02 and up hides things a short way off.", "risk": "Thick fog hides players from people but not from bots."},
    "ent.bone_pos": {"more": "The bone is found by name in the entity's own model each time (a player is models/player.mdl on the server, whatever skin they wear), so one node works for every player. The server poses the model from its own animation, which is close to what players see but not exact.", "pins": {"found": "No when the entity's model has no bone with that name, or it isn't a studio model.", "angles": "Pitch, yaw and roll of the bone."}, "related": ["ent.attachment_pos", "vec.distance"]},
    "ent.attachment_pos": {"more": "Attachments are a model's fixed points (a weapon's muzzle and shell ejection, a hand): most have no names, so the list shows each one's number and the bone it sits on.", "pins": {"found": "No when the entity's model has fewer attachments, or it isn't a studio model."}, "related": ["ent.bone_pos"]},
    "eng.soundtrack": {"more": "The tracks are the Half-Life soundtrack's, numbered as the game numbers them (2 is the first). It's the player's own music volume (MP3 volume in the options).", "related": ["fx.sound_to"]},
})

# P5: player input, cooldowns, pickups, weapon tuning, saving (gen/p5.rs)
NOTES.update({
    "in.pressed": {"more": "Works the same on your own game, a listen server and a dedicated Linux server: the server sees a button when the player's game sends it, so it arrives with their ping (a 100 ms player's press shows up about 100 ms after they pressed). Only buttons the game itself has: fire, right click, use, jump, duck, reload, walk and the move keys. For any other key, use Player Command (players bind the key to it). Include bots: bots press buttons too (on by default); untick it for something only people should set off.",
        "pins": {"only": "A player to listen to (from a variable, set when they're picked): nobody else's presses run it. Not wired: everyone's.", "player": "Who pressed it.", "button": "Which button, by name: attack, attack2, use, jump, duck, reload, walk, forward, back, left, right."},
        "cost": "A check of each player's buttons every server frame, only when a mode has this node; the flow runs only on a press.", "related": ["in.holding", "in.command", "cd.start"]},
    "in.holding": {"more": "What reached the server, so it trails the player's screen by their ping. Use it inside a flow that's already running (a timer, On Tick) to ask whether fire or duck is held right now.", "related": ["in.pressed"]},
    "in.command": {"more": "A key the game has no button for (F, G, a mouse button) can still start a flow: name a command (ls_ and letters, like ls_dash), and players bind a key to it in their console (bind f \"ls_dash\"); Tell Player To Bind shows them the line. Arguments is whatever came after the command, as text only: the mode can read it, but it's never run as a command. Player Command uses a second (each player) keeps a held or scripted key from flooding the server: a setting in the mode's Limits group, 5 to start (the checks warn above 20).",
        "pins": {"player": "Who used the command.", "args": "The rest of the line, as text (empty when there was none)."},
        "risk": "Safe by design: the command name can only be ls_ and letters (never one of the game's own), and the arguments are never run.", "related": ["in.tell_bind", "in.pressed", "event.chat_command"]},
    "in.tell_bind": {"more": "The game never lets a server change a player's keys, so this only tells them what to type: it shows the line in chat. Nothing is bound for them.", "related": ["in.command"]},
    "cd.start": {"more": "One cooldown per ability name: for each player, each team, or one for everyone. Start it when the ability is used, check Is Ready before letting it run again. A player's cooldowns start clean when they leave; a new map starts them all clean.",
        "pins": {"player": "Whose cooldown (For: each player).", "team": "Which team's cooldown (For: each team).", "secs": "How long until it's ready again."}, "related": ["cd.ready", "cd.left", "cd.reset", "in.pressed"]},
    "cd.ready": {"related": ["cd.start", "cd.left"]},
    "cd.left": {"more": "Good for a HUD line (Dash ready in 3 s).", "related": ["cd.start", "cd.ready"]},
    "cd.reset": {"related": ["cd.start"]},
    "item.picked_up": {"more": "Weapons and grenades from the ground (dropped ones, or a map's armoury), and armor from a map's armoury. Not what's bought, given by the mode, or handed out at spawn.",
        "pins": {"player": "Who picked it up.", "item": "Its short name: ak47, deagle, hegrenade, kevlar."}, "related": ["item.dropped", "player.give_weapon"]},
    "item.dropped": {"pins": {"player": "Who dropped it.", "weapon": "Its short name: ak47, deagle, c4."}, "related": ["item.picked_up", "player.drop_weapon"]},
    "wpn.infinite_ammo": {"more": "The clip: every shot puts the bullet back, so they never reload. Spare ammo: topped up to the most that gun can carry whenever it changes. Works with every gun; grenades aren't ammo.", "related": ["wpn.fire_rate", "wpn.recoil"]},
    "wpn.fire_rate": {"more": "Changes the time between shots after each shot, so it follows every gun's own speed (2 makes an AK-47 fire twice as fast). Very high values can look odd: animations are the gun's own.", "cost": "A hook on each gun's fire, only when a mode has a weapon tuning node.", "related": ["wpn.recoil", "wpn.infinite_ammo"]},
    "wpn.recoil": {"more": "Scales the view kick each shot gives. Spread (where bullets go) is the game's own and doesn't change.", "related": ["wpn.fire_rate"]},
    "save.text_get": {"related": ["save.text_set", "save.get"]},
    "save.text_set": {"more": "Kept in the mode's own file beside Saved Value's (written within 5 seconds, and at the map's end; a crash leaves one whole file). Players are known by their Steam ID; bots and LAN players by their name.", "related": ["save.text_get", "save.set"]},
    "save.server_get": {"more": "For things that belong to the server, not a player: a record time and who set it, a counter of rounds played across maps.", "pins": {"out": "The saved number (0 until set).", "text": "The saved text (empty until set)."}, "related": ["save.server_set", "save.top"]},
    "save.server_set": {"related": ["save.server_get"]},
    "save.top": {"more": "Reads every player's value of one Saved Value (its name) from the save file, so players who aren't on the server count too, named as they were last seen. Each runs once per place, best first. How many places is Top Players places, a setting in the mode's Limits group, 10 to start (the checks warn above 50).",
        "pins": {"rank": "The place: 1 for the best.", "name": "Their name as last seen (bots and LAN players: the name they were saved by).", "value": "Their saved value."},
        "cost": "Goes through the whole save file each time it runs: run it now and then (a round's start, a chat command), not every tick.", "related": ["save.get", "save.add", "save.set"]},
})

EXEC_OUT = {"then": "Goes on when it's done.", "true": "Runs when the answer is yes.", "false": "Runs when the answer is no.", "each": "Runs once for every item.", "body": "Runs once for every step.", "none": "Runs when nothing was found.", "tick": "Runs every time the timer fires.", "done": "Runs when it's over.", "default": "Runs when no case matches.", "valid": "Runs when it's there.", "invalid": "Runs when it isn't."}

def pin_help(d, p, out, notes):
    if p["id"] in notes.get("pins", {}):
        return notes["pins"][p["id"]]
    if p["type"] == "exec":
        if not out:
            return {"exec": "Runs this node.", "reset": "Lets it run again.", "break": "Stops the loop.", "open": "Opens the gate.", "close": "Closes the gate.", "toggle": "Opens or closes the gate."}.get(p["id"], f"Runs {p.get('label') or p['id']}.")
        return EXEC_OUT.get(p["id"], f"Runs on {p.get('label') or p['id']}.")
    label = p.get("label") or p["id"]
    words = type_words(p["type"])
    s = f"The {label.lower()}." if words.split(" ", 1)[-1].startswith(label.lower()) else f"{label}: {words}."
    if not out:
        if p.get("param"):
            s += " Leave it empty to use the setting."
        elif p.get("required"):
            s += " Needs a wire."
        elif p.get("optional"):
            s += " Optional."
    return s

def build(registry):
    defs = {d["type"]: d for d in registry["nodes"]}
    rel = pairs(list(defs))
    out = {}
    for d in registry["nodes"]:
        t = d["type"]
        notes = NOTES.get(t, {})
        params = {p["id"] for p in d["params"]}
        pinned = {p.get("param") for p in d["inputs"] if p.get("param")}
        h = {
            "title": d["title"],
            "category": d["category"],
            "what": d["help"],
            "pins": [{"id": p["id"], "dir": "in", "help": pin_help(d, p, False, notes)} for p in d["inputs"]]
                  + [{"id": p["id"], "dir": "out", "help": pin_help(d, p, True, notes)} for p in d["outputs"]],
            "settings": [{"id": p["id"], "help": setting_help(p, p["id"] in pinned)} for p in d["params"]],
        }
        if notes.get("more"):
            h["more"] = notes["more"]
        if d.get("needs") == "components":
            h["needs"] = "Needs Loner-Strike's components plugin, which comes with the app."
        elif d.get("needs") == "creatures":
            h["needs"] = "Needs the Loner-Strike bot brain with creatures."
        r = list(dict.fromkeys(notes.get("related", []) + rel.get(t, [])))
        if len(r) < 3:
            # a few neighbors from the same category
            same = [x["type"] for x in registry["nodes"] if x["category"] == d["category"] and x["type"] != t and x["type"] not in r]
            i = [x["type"] for x in registry["nodes"]].index(t)
            same.sort(key=lambda x: abs([y["type"] for y in registry["nodes"]].index(x) - i))
            r += same[: 3 - len(r)]
        h["related"] = [x for x in r if x in defs and x != t][:6]
        for k in ("cost", "risk"):
            if notes.get(k):
                h[k] = notes[k]
        if d.get("advanced") and "risk" not in h:
            h["risk"] = "A power tool: a community mode that uses it is flagged for players to see."
        ex = make_example(d, defs)
        if ex:
            h["example"] = {"caption": notes.get("caption") or caption(d, ex), **ex}
        out[t] = h
    return {"version": 1, "help": out}

def write(registry, path):
    data = build(registry)
    with open(path, "w") as f:
        json.dump(data, f, indent=0, ensure_ascii=False, separators=(",", ":"))
    return len(data["help"])

if __name__ == "__main__":
    src = sys.argv[1]
    reg = json.load(open(src))
    n = write(reg, os.path.join(os.path.dirname(src), "nodehelp.json"))
    print(n, "nodes with help")
