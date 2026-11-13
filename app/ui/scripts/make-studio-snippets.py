# Builds the Game Mode Studio's snippet library (src/views/tools/studio/snippets/): common pieces
# of a mode to drop into a graph. Then `npm run tidy:studio` lays them out.
#   python3 scripts/make-studio-snippets.py && npm run tidy:studio
import json, os

OUT = os.path.join(os.path.dirname(__file__), "..", "src", "views", "tools", "studio", "snippets")

class S:
    def __init__(self):
        self.nodes, self.edges, self.variables = [], [], []
    def n(self, id, type, **params):
        self.nodes.append({"id": id, "type": type, "pos": {"x": 0, "y": 0}, "params": params})
        return id
    def w(self, a, ap, b, bp="exec"):
        self.edges.append({"id": f"w{len(self.edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
    def then(self, *ids):
        for a, b in zip(ids, ids[1:]):
            self.w(a, "then", b)
    def var(self, id, name, type, scope="mode", default=0, reset=None):
        v = {"id": id, "name": name, "type": type, "scope": scope, "default": default}
        if reset:
            v["reset"] = reset
        self.variables.append(v)
    def save(self, id, name, description):
        for i, nd in enumerate(self.nodes):
            nd["pos"] = {"x": i * 260, "y": 0}
        d = {"schema": "loner-strike/graph-snippet", "version": 1, "name": name, "description": description,
             "nodes": self.nodes, "edges": self.edges, "comments": [], "variables": self.variables}
        json.dump(d, open(os.path.join(OUT, f"{id}.snippet.json"), "w"), indent=1, ensure_ascii=False)
        print(id, len(self.nodes))

s = S()
s.n("ws", "comp.on_wave_start"); s.n("hud", "msg.hud", text="Wave", slot="top", seconds=6); s.w("ws", "wave", "hud", "value"); s.w("ws", "then", "hud")
s.n("wc", "comp.on_wave_cleared"); s.n("cd", "timer.countdown", secs=15, text="Next wave in", slot="top"); s.w("wc", "then", "cd")
s.n("go", "comp.wave_start", wave=0); s.w("cd", "done", "go")
s.save("wave_system", "Wave system", "Waves from the map's wave spawners: shown on screen, the next one 15 s after a clear (start the first yourself).")

s = S()
s.n("rs", "event.round_started"); s.n("all", "flow.for_each_player", which="alive"); s.w("rs", "then", "all")
s.n("fr", "player.freeze", on=True); s.w("all", "player", "fr", "player"); s.w("all", "each", "fr")
s.n("cd", "timer.countdown", secs=5, text="Go in", slot="center"); s.w("all", "then", "cd")
s.n("all2", "flow.for_each_player", which="alive"); s.w("cd", "done", "all2")
s.n("un", "player.freeze", on=False); s.w("all2", "player", "un", "player"); s.w("all2", "each", "un")
s.n("msg", "msg.broadcast", text="Go!", seconds=2); s.w("all2", "then", "msg")
s.save("countdown_start", "Countdown and start", "Everyone frozen for a 5 s countdown at the start of the round, then Go.")

s = S()
s.n("sh", "event.chat_command", command="shop")
s.n("menu", "msg.menu", title="Shop", o1="AK-47 (2500)", o2="Kevlar and helmet (1000)", o3="HE grenade (300)", o4=""); s.w("sh", "player", "menu", "player"); s.w("sh", "then", "menu")
for k, (price, node, params) in enumerate([(2500, "player.give_weapon", {"weapon": "ak47", "ammo": 90}), (1000, "player.give_item", {"item": "vesthelm"}), (300, "player.give_weapon", {"weapon": "hegrenade", "ammo": 0})], start=1):
    m = s.n(f"m{k}", "player.get_money"); s.w("sh", "player", m, "player")
    c = s.n(f"c{k}", "math.compare", op="ge", b=price); s.w(m, "out", c, "a")
    b = s.n(f"b{k}", "flow.branch"); s.w(c, "out", b, "cond"); s.w("menu", f"c{k}", b)
    g = s.n(f"g{k}", node, **params); s.w("sh", "player", g, "player"); s.w(b, "true", g)
    p = s.n(f"p{k}", "player.add_money", amount=-price); s.w("sh", "player", p, "player"); s.then(g, p)
    no = s.n(f"n{k}", "msg.chat", text="Not enough money for that."); s.w("sh", "player", no, "player"); s.w(b, "false", no)
s.save("shop_menu", "Shop menu", "Say !shop (or slash shop): a menu that checks the money, gives the item and takes the price.")

s = S()
s.n("pk", "event.player_killed"); s.n("down", "player.down", secs=30, hold=3, health=50); s.w("pk", "victim", "down", "player"); s.w("pk", "then", "down")
s.n("rv", "event.player_revived"); s.n("pay", "player.add_money", amount=200); s.w("rv", "by", "pay", "player"); s.w("rv", "then", "pay")
s.save("revive", "Revive", "Fallen players are downed for 30 s; a teammate holding USE on them for 3 s brings them back (+200 for the helper).")

s = S()
s.n("rs", "event.round_started"); s.n("each", "flow.for_each_entity", by="class", name="ls_zone_shrinker"); s.w("rs", "then", "each")
s.n("go", "comp.start"); s.w("each", "entity", "go", "comp"); s.w("each", "each", "go")
s.n("zt", "comp.on_zone_tick"); s.n("hud", "msg.hud", text="Get back in the zone!", slot="center", seconds=1); s.w("zt", "player", "hud", "player"); s.w("zt", "then", "hud")
s.save("zone_shrinks", "Zone that shrinks", "Starts the map pack's zone shrinkers each round and tells players outside the zone.")

s = S()
s.n("rs", "event.round_started"); s.n("rep", "timer.repeat", every=60, times=0); s.w("rs", "then", "rep")
s.n("pick", "flow.pick_entity", by="class", name="ls_respawn_point"); s.w("rep", "tick", "pick")
s.n("drop", "comp.drop_call"); s.w("pick", "entity", "drop", "at_ent"); s.w("pick", "then", "drop")
s.save("supply_drop", "Supply drop every minute", "A supply drop onto a random respawn point every 60 s until the round ends.")

s = S()
s.n("sp", "event.player_spawned"); s.n("strip", "player.strip", knife=True); s.w("sp", "player", "strip", "player"); s.w("sp", "then", "strip")
s.n("gun", "player.give_weapon", weapon="m4a1", ammo=90); s.w("sp", "player", "gun", "player")
s.n("pistol", "player.give_weapon", weapon="deagle", ammo=35); s.w("sp", "player", "pistol", "player")
s.n("arm", "player.give_item", item="vesthelm"); s.w("sp", "player", "arm", "player")
s.n("hp", "player.set_health", amount=100); s.w("sp", "player", "hp", "player")
s.then("strip", "gun", "pistol", "arm", "hp")
s.save("loadout_on_spawn", "Give loadout on spawn", "Every spawn: weapons taken away, then an M4, a Desert Eagle, armor and full health.")

s = S()
s.n("pk", "event.player_killed"); s.n("left", "squad.alive"); s.n("one", "math.compare", op="le", b=1); s.w("left", "out", "one", "a")
s.n("br", "flow.branch"); s.w("one", "out", "br", "cond"); s.w("pk", "then", "br")
s.n("win", "flow.pick_player", which="alive"); s.w("br", "true", "win")
s.n("name", "msg.hud", text="Winner:", slot="center", seconds=6); s.w("win", "player", "name", "value"); s.w("win", "then", "name")
s.n("team", "player.get_team"); s.w("win", "player", "team", "player")
s.n("end", "score.end_round"); s.w("team", "out", "end", "winner"); s.then("name", "end")
s.n("draw", "score.end_round", winner="draw"); s.w("win", "none", "draw")
s.save("last_standing", "Last squad standing wins", "When one squad (or one player, without squads) is left, they win and the round ends.")

s = S()
s.n("pk", "event.player_killed"); s.n("alive", "player.is_alive"); s.w("pk", "attacker", "alive", "player")
s.n("br", "flow.branch"); s.w("alive", "out", "br", "cond"); s.w("pk", "then", "br")
s.n("pay", "player.add_money", amount=300); s.w("pk", "attacker", "pay", "player"); s.w("br", "true", "pay")
s.n("pts", "player.add_frags", amount=1); s.w("pk", "attacker", "pts", "player"); s.then("pay", "pts")
s.n("snd", "game.play_sound", sound="buttons/blip1"); s.w("pk", "attacker", "snd", "player"); s.then("pts", "snd")
s.save("kill_reward", "Kill reward", "A living killer gets 300, a point and a blip.")

s = S()
s.n("rs", "event.round_started"); s.n("each", "flow.for_each_entity", by="name", name="creature_spot"); s.w("rs", "then", "each")
s.n("make", "creature.spawn", kind="zombie", at="creature_spot"); s.w("each", "entity", "make", "at_ent"); s.w("each", "each", "make")
s.n("ck", "creature.on_killed"); s.n("pay", "player.add_money", amount=100); s.w("ck", "killer", "pay", "player"); s.w("ck", "then", "pay")
s.save("creatures_at_markers", "Creatures at markers", "A zombie at every entity named creature_spot each round; 100 for each one killed.")

s = S()
s.n("rs", "event.round_started"); s.n("clk", "clock.set", secs=180); s.w("rs", "then", "clk")
s.n("up", "event.time_up"); s.n("msg", "msg.broadcast", text="Time's up: the Counter-Terrorists held on!", seconds=4); s.w("up", "then", "msg")
s.n("end", "score.end_round", winner="ct"); s.then("msg", "end")
s.save("round_timer_win", "Round timer win", "The mode's own 3 minute clock; when it runs out, the Counter-Terrorists win.")

s = S()
s.var("tscore", "Team score", "number", scope="team", default=0)
s.n("pk", "event.player_killed"); s.n("team", "player.get_team"); s.w("pk", "attacker", "team", "player")
s.n("add", "var.add", var="tscore", amount=1); s.w("team", "out", "add", "team"); s.w("pk", "then", "add")
s.n("get", "var.get", var="tscore"); s.w("team", "out", "get", "team")
s.n("cmp", "math.compare", op="ge", b=10); s.w("get", "out", "cmp", "a")
s.n("br", "flow.branch"); s.w("cmp", "out", "br", "cond"); s.then("add", "br")
s.n("end", "score.end_round"); s.w("team", "out", "end", "winner"); s.w("br", "true", "end")
s.save("team_score_win", "Team score to 10 wins", "Each kill scores for the killer's team; the first team to 10 wins the round.")

s = S()
s.n("t", "event.player_touched", name="tele_in"); s.n("tp", "player.teleport", name="tele_out"); s.w("t", "player", "tp", "player"); s.w("t", "then", "tp")
s.save("teleport_on_touch", "Teleport on touch", "Walking into the trigger named tele_in takes a player to the entity named tele_out.")

s = S()
s.n("c", "event.chat_command", command="heal"); s.n("hp", "player.set_health", amount=100); s.w("c", "player", "hp", "player"); s.w("c", "then", "hp")
s.n("say", "msg.chat", text="Healed."); s.w("c", "player", "say", "player"); s.then("hp", "say")
s.save("chat_command_action", "Chat command does something", "Say !heal (or slash heal): back to full health.")
