# Builds the Game Mode Studio's Last Stand and Battle Royale templates (src/views/tools/studio/
# templates/). Then `npm run tidy:studio` lays every template out like Tidy up does.
#   python3 scripts/make-studio-templates.py && npm run tidy:studio
import json, os

OUT = os.path.join(os.path.dirname(__file__), "..", "src", "views", "tools", "studio", "templates")

class G:
    def __init__(self):
        self.nodes, self.edges, self.comments = [], [], []
    def n(self, id, type, **params):
        self.nodes.append({"id": id, "type": type, "pos": {"x": 0, "y": 0}, "params": params})
        return id
    def w(self, a, ap, b, bp="exec"):
        self.edges.append({"id": f"w{len(self.edges)+1}", "from": {"node": a, "pin": ap}, "to": {"node": b, "pin": bp}})
    def then(self, *ids):
        for a, b in zip(ids, ids[1:]):
            self.w(a, "then", b)
    def note(self, id, text, members):
        # placed round its nodes by the tidy step (members start inside it)
        self.comments.append({"id": id, "text": text, "pos": {"x": 0, "y": 0}, "size": {"w": 10, "h": 10}, "_members": members})
    def save(self, name, mode, variables):
        # members: each comment starts round its nodes' (0,0) spots; spread them so tidy can tell
        for k, c in enumerate(self.comments):
            for nid in c.pop("_members"):
                for nd in self.nodes:
                    if nd["id"] == nid:
                        nd["pos"] = {"x": k * 5000 + 10, "y": 10}
            c["pos"] = {"x": k * 5000, "y": 0}
            c["size"] = {"w": 4000, "h": 4000}
        # nodes in no comment: away from all of them
        boxed = {nd["id"] for nd in self.nodes if nd["pos"]["y"] == 10}
        for i, nd in enumerate(self.nodes):
            if nd["id"] not in boxed:
                nd["pos"] = {"x": -100000, "y": i * 10}
        g = {"schema": "loner-strike/graph", "version": 2, "mode": mode, "variables": variables, "nodes": self.nodes, "edges": self.edges, "comments": self.comments}
        json.dump(g, open(os.path.join(OUT, name), "w"), indent=1, ensure_ascii=False)
        print(name, len(self.nodes), "nodes")

# ---------------------------------------------------------------- Last Stand (lite)
g = G()
# set up once: the Last Stand map pack (wave spawners, barricades, drops), rounds end when we say
g.n("ms", "event.map_started")
g.n("ms_pack", "comp.load_pack", pack="laststand")
g.then("ms", "ms_pack")
g.note("c_setup", "Set up once: Last Stand's map pack (wave spawners, barricades, supply drops). The mode's settings (This mode): rounds only end when the mode says, no team balance.", ["ms", "ms_pack"])

# spawns: Terrorists are the horde (knife, more health, green), Counter-Terrorists defend
g.n("sp", "event.player_spawned")
g.n("sp_t", "cond.is_team", team="t"); g.w("sp", "player", "sp_t", "player"); g.w("sp", "then", "sp_t")
g.n("sp_strip", "player.strip", knife=True); g.w("sp", "player", "sp_strip", "player"); g.w("sp_t", "true", "sp_strip")
g.n("sp_hp", "player.set_health", amount=150); g.w("sp", "player", "sp_hp", "player")
g.n("sp_glow", "player.set_glow", color="green"); g.w("sp", "player", "sp_glow", "player")
g.then("sp_strip", "sp_hp", "sp_glow")
g.n("sp_ctstrip", "player.strip", knife=True); g.w("sp", "player", "sp_ctstrip", "player"); g.w("sp_t", "false", "sp_ctstrip")
g.n("sp_gun", "player.give_weapon", weapon="m4a1", ammo=120); g.w("sp", "player", "sp_gun", "player"); g.then("sp_ctstrip", "sp_gun")
g.n("sp_arm", "player.give_item", item="vesthelm"); g.w("sp", "player", "sp_arm", "player"); g.then("sp_gun", "sp_arm")
g.note("c_spawn", "Terrorists are the horde: knife only, 150 health, green. Counter-Terrorists defend with an M4 and armor.", ["sp", "sp_t", "sp_strip", "sp_hp", "sp_glow", "sp_ctstrip", "sp_gun", "sp_arm"])

# round start: the horde waits for the first wave
g.n("rs", "event.round_started")
g.n("rs_w0", "var.set", var="wave", value=0)
g.n("rs_kill", "flow.for_each_player", which="alive_t")
g.n("rs_k", "player.kill"); g.w("rs_kill", "player", "rs_k", "player"); g.w("rs_kill", "each", "rs_k")
g.n("rs_msg", "msg.broadcast", text="Hold the base! The first wave is coming.", seconds=4)
g.n("rs_cd", "timer.countdown", secs=15, text="First wave in", slot="top")
g.n("rs_go", "comp.wave_start", wave=0); g.w("rs_cd", "done", "rs_go")
g.w("rs", "then", "rs_w0"); g.w("rs_w0", "then", "rs_kill"); g.w("rs_kill", "then", "rs_msg"); g.then("rs_msg", "rs_cd")
g.note("c_start", "Each round is one stand: the horde waits, then wave 1.", ["rs", "rs_w0", "rs_kill", "rs_k", "rs_msg", "rs_cd", "rs_go"])

# a wave starts
g.n("ws", "comp.on_wave_start")
g.n("ws_set", "var.set", var="wave"); g.w("ws", "wave", "ws_set", "value"); g.w("ws", "then", "ws_set")
g.n("ws_hud", "msg.hud", text="Wave", slot="top", seconds=6); g.w("ws", "wave", "ws_hud", "value"); g.then("ws_set", "ws_hud")
g.n("ws_snd", "game.play_sound", sound="radio/locknload"); g.then("ws_hud", "ws_snd")

# a wave is cleared: won, or a break with money and the dead back
g.n("wc", "comp.on_wave_cleared")
g.n("wc_cmp", "math.compare", op="ge", b=5); g.w("wc", "wave", "wc_cmp", "a")
g.n("wc_br", "flow.branch"); g.w("wc_cmp", "out", "wc_br", "cond"); g.w("wc", "then", "wc_br")
g.n("wc_won", "msg.broadcast", text="You held out! The base is safe.", seconds=5); g.w("wc_br", "true", "wc_won")
g.n("wc_end", "score.end_round", winner="ct"); g.then("wc_won", "wc_end")
g.n("wc_dead", "flow.for_each_player", which="dead"); g.w("wc_br", "false", "wc_dead")
g.n("wc_isct", "cond.is_team", team="ct"); g.w("wc_dead", "player", "wc_isct", "player"); g.w("wc_dead", "each", "wc_isct")
g.n("wc_resp", "player.respawn"); g.w("wc_dead", "player", "wc_resp", "player"); g.w("wc_isct", "true", "wc_resp")
g.n("wc_pay", "flow.for_each_player", which="alive_ct"); g.w("wc_dead", "then", "wc_pay")
g.n("wc_money", "player.add_money", amount=1500); g.w("wc_pay", "player", "wc_money", "player"); g.w("wc_pay", "each", "wc_money")
g.n("wc_cd", "timer.countdown", secs=15, text="Next wave in", slot="top"); g.w("wc_pay", "then", "wc_cd")
g.n("wc_next", "comp.wave_start", wave=0); g.w("wc_cd", "done", "wc_next")
g.n("wc_tip", "msg.chat", text="Wave cleared: +1500. Say !shop to buy."); g.w("wc_cd", "then", "wc_tip")
g.note("c_waves", "Waves: hold out for 5. Between waves the fallen come back and everyone gets money.", ["ws", "ws_set", "ws_hud", "ws_snd", "wc", "wc_cmp", "wc_br", "wc_won", "wc_end", "wc_dead", "wc_isct", "wc_resp", "wc_pay", "wc_money", "wc_cd", "wc_next", "wc_tip"])

# deaths: a defender down (the base falls when none are left), a zombie down (money)
g.n("pk", "event.player_killed")
g.n("pk_ct", "cond.is_team", team="ct"); g.w("pk", "victim", "pk_ct", "player"); g.w("pk", "then", "pk_ct")
g.n("pk_left", "game.count_alive", team="ct")
g.n("pk_none", "math.compare", op="eq", b=0); g.w("pk_left", "out", "pk_none", "a")
g.n("pk_br", "flow.branch"); g.w("pk_none", "out", "pk_br", "cond"); g.w("pk_ct", "true", "pk_br")
g.n("pk_lost", "msg.broadcast", text="The base is overrun!", seconds=5); g.w("pk_br", "true", "pk_lost")
g.n("pk_end", "score.end_round", winner="t"); g.then("pk_lost", "pk_end")
g.n("pk_by", "cond.is_team", team="ct"); g.w("pk", "attacker", "pk_by", "player"); g.w("pk_ct", "false", "pk_by")
g.n("pk_pay", "player.add_money", amount=300); g.w("pk", "attacker", "pk_pay", "player"); g.w("pk_by", "true", "pk_pay")
g.n("pk_score", "player.add_frags", amount=1); g.w("pk", "attacker", "pk_score", "player"); g.then("pk_pay", "pk_score")
g.note("c_deaths", "Deaths: the base falls when no defender is left; each zombie killed pays 300.", ["pk", "pk_ct", "pk_left", "pk_none", "pk_br", "pk_lost", "pk_end", "pk_by", "pk_pay", "pk_score"])

# the shop: say /shop
g.n("sh", "event.chat_command", command="shop")
g.n("sh_menu", "msg.menu", title="Shop", o1="AK-47 (2500)", o2="Kevlar and helmet (1000)", o3="HE grenade (300)", o4=""); g.w("sh", "player", "sh_menu", "player"); g.w("sh", "then", "sh_menu")
for k, (price, node, params) in enumerate([(2500, "player.give_weapon", {"weapon": "ak47", "ammo": 90}), (1000, "player.give_item", {"item": "vesthelm"}), (300, "player.give_weapon", {"weapon": "hegrenade", "ammo": 0})], start=1):
    m = g.n(f"sh_m{k}", "player.get_money"); g.w("sh", "player", m, "player")
    c = g.n(f"sh_c{k}", "math.compare", op="ge", b=price); g.w(m, "out", c, "a")
    b = g.n(f"sh_b{k}", "flow.branch"); g.w(c, "out", b, "cond"); g.w("sh_menu", f"c{k}", b)
    give = g.n(f"sh_g{k}", node, **params); g.w("sh", "player", give, "player"); g.w(b, "true", give)
    pay = g.n(f"sh_p{k}", "player.add_money", amount=-price); g.w("sh", "player", pay, "player"); g.then(give, pay)
    no = g.n(f"sh_n{k}", "msg.chat", text="Not enough money for that."); g.w("sh", "player", no, "player"); g.w(b, "false", no)
g.note("c_shop", "The shop: say !shop (or slash shop). Each choice checks the money first.", ["sh", "sh_menu"] + [f"sh_{x}{k}" for k in (1, 2, 3) for x in "mcbgpn"])

g.save("last-stand.graph.json", {"id": "last_stand_lite", "name": "Last Stand Lite", "description": "Hold the base against 5 waves", "settings": [{"cvar": "mp_round_infinite", "value": 1}, {"cvar": "mp_autoteambalance", "value": 0}]},
       [{"id": "wave", "name": "Wave", "type": "number", "scope": "mode", "default": 0}])

# ---------------------------------------------------------------- Battle Royale (lite)
g = G()
g.n("ms", "event.map_started")
g.n("ms_pack", "comp.load_pack", pack="br")
g.then("ms", "ms_pack")
g.note("c_setup", "Set up once: Battle Royale's map pack (safe zone, shrinker, respawn points, crates). The mode's settings (This mode): free for all, rounds only end when the mode says.", ["ms", "ms_pack"])

g.n("rs", "event.round_started")
g.n("rs_each", "flow.for_each_player", which="alive"); g.w("rs", "then", "rs_each")
g.n("rs_strip", "player.strip", knife=True); g.w("rs_each", "player", "rs_strip", "player"); g.w("rs_each", "each", "rs_strip")
g.n("rs_spot", "comp.respawn_at"); g.w("rs_each", "player", "rs_spot", "player"); g.then("rs_strip", "rs_spot")
g.n("rs_zone", "flow.for_each_entity", by="class", name="ls_zone_shrinker"); g.w("rs_each", "then", "rs_zone")
g.n("rs_start", "comp.start"); g.w("rs_zone", "entity", "rs_start", "comp"); g.w("rs_zone", "each", "rs_start")
g.n("rs_msg", "msg.broadcast", text="Loot up and stay in the zone. Last one standing wins!", seconds=5); g.w("rs_zone", "then", "rs_msg")
g.n("rs_alive", "var.set", var="alive", value=0); g.then("rs_msg", "rs_alive")
g.n("rs_drops", "timer.repeat", every=60, times=4); g.then("rs_alive", "rs_drops")
g.n("rs_pick", "flow.pick_entity", by="class", name="ls_respawn_point"); g.w("rs_drops", "tick", "rs_pick")
g.n("rs_drop", "comp.drop_call"); g.w("rs_pick", "entity", "rs_drop", "at_ent"); g.w("rs_pick", "then", "rs_drop")
g.note("c_start", "The round: everyone to a spread-out spot with a knife, the zone starts closing, a supply drop every minute.", ["rs", "rs_each", "rs_strip", "rs_spot", "rs_zone", "rs_start", "rs_msg", "rs_alive", "rs_drops", "rs_pick", "rs_drop"])

g.n("pk", "event.player_killed")
g.n("pk_left", "game.count_alive", team="none")
g.n("pk_set", "var.set", var="alive"); g.w("pk_left", "out", "pk_set", "value"); g.w("pk", "then", "pk_set")
g.n("pk_hud", "msg.hud", text="Players left:", slot="right", seconds=5); g.w("pk_left", "out", "pk_hud", "value"); g.then("pk_set", "pk_hud")
g.n("pk_one", "math.compare", op="le", b=1); g.w("pk_left", "out", "pk_one", "a")
g.n("pk_br", "flow.branch"); g.w("pk_one", "out", "pk_br", "cond"); g.then("pk_hud", "pk_br")
g.n("pk_win", "flow.pick_player", which="alive"); g.w("pk_br", "true", "pk_win")
g.n("pk_name", "msg.hud", text="Winner:", slot="center", seconds=6); g.w("pk_win", "player", "pk_name", "value"); g.w("pk_win", "then", "pk_name")
g.n("pk_pts", "player.add_frags", amount=5); g.w("pk_win", "player", "pk_pts", "player"); g.then("pk_name", "pk_pts")
g.n("pk_team", "player.get_team"); g.w("pk_win", "player", "pk_team", "player")
g.n("pk_end", "score.end_round"); g.w("pk_team", "out", "pk_end", "winner"); g.then("pk_pts", "pk_end")
g.n("pk_draw", "score.end_round", winner="draw"); g.w("pk_win", "none", "pk_draw")
g.note("c_end", "When one player is left, they win (5 points) and the round ends.", ["pk", "pk_left", "pk_set", "pk_hud", "pk_one", "pk_br", "pk_win", "pk_name", "pk_pts", "pk_team", "pk_end", "pk_draw"])

g.n("zt", "comp.on_zone_tick")
g.n("zt_hud", "msg.hud", text="Get back in the zone!", slot="center", seconds=1); g.w("zt", "player", "zt_hud", "player"); g.w("zt", "then", "zt_hud")
g.n("lo", "comp.on_loot")
g.n("lo_chat", "msg.chat", text="Crate opened."); g.w("lo", "player", "lo_chat", "player"); g.w("lo", "then", "lo_chat")
g.note("c_tips", "Tips for players.", ["zt", "zt_hud", "lo", "lo_chat"])

g.save("battle-royale.graph.json", {"id": "royale_lite", "name": "Royale Lite", "description": "Last one standing in a shrinking zone", "settings": [{"cvar": "mp_freeforall", "value": 1}, {"cvar": "mp_round_infinite", "value": 1}]},
       [{"id": "alive", "name": "Players left", "type": "number", "scope": "mode", "default": 0}])
