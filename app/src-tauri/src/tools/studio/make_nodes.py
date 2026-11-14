# Writes the Studio's node registry. Run from this folder:
#   python3 make_nodes.py ../../../../ui/src/views/tools/studio/nodes.json
import json, sys

E = lambda id="then", label="": {"id": id, "type": "exec", "label": label}
X = {"id": "exec", "type": "exec", "label": ""}
def P(id, type, label, **k):
    d = {"id": id, "type": type, "label": label}; d.update(k); return d
def prm(id, label, kind, default, **k):
    d = {"id": id, "label": label, "kind": kind, "default": default}; d.update(k); return d

TEAM2 = [["t", "Terrorists"], ["ct", "Counter-Terrorists"]]
TEAM3 = TEAM2 + [["none", "No team"]]
WEAPONS = [["ak47","AK-47"],["m4a1","M4A1"],["awp","AWP"],["scout","Scout"],["famas","FAMAS"],["galil","Galil"],["aug","AUG"],["sg552","SG 552"],["mp5navy","MP5"],["p90","P90"],["ump45","UMP 45"],["mac10","MAC-10"],["tmp","TMP"],["m3","M3 shotgun"],["xm1014","XM1014 shotgun"],["m249","M249"],["deagle","Desert Eagle"],["usp","USP"],["glock18","Glock"],["p228","P228"],["fiveseven","Five-SeveN"],["elite","Dual Elites"],["hegrenade","HE grenade"],["flashbang","Flashbang"],["smokegrenade","Smoke grenade"]]
SLOTS = [["top","Top middle"],["left","Left"],["right","Right"],["center","Middle"]]
COMPONENTS = [["ls_capture_point","Capture point"],["ls_safe_zone","Safe zone"],["ls_zone_shrinker","Shrinking zone"],["ls_damage_zone","Damage zone"],["ls_loot_container","Loot container"],["ls_weapon_spawner","Weapon spawner"],["ls_supply_drop","Supply drop"],["ls_wave_spawner","Wave spawner"],["ls_extraction","Extraction point"],["ls_checkpoint","Checkpoint"],["ls_trap","Trap"],["ls_respawn_point","Respawn point"],["ls_shop","Shop (a counter, items=Name|price|give|what;...)"],["ls_perk_machine","Perk machine"],["ls_upgrade_bench","Upgrade bench"],["ls_mystery_box","Mystery box"]]

nodes = []
def node(type, title, cat, help, inputs=(), outputs=(), params=(), **k):
    d = {"type": type, "title": title, "category": cat, "help": help, "inputs": list(inputs), "outputs": list(outputs), "params": list(params)}
    d.update(k); nodes.append(d)

# ---- events
ev = dict(event=True)
node("event.round_started", "Round Started", "Events", "Runs when a round begins: freeze time is over and everyone can move.", outputs=[E()], **ev)
node("event.round_ended", "Round Ended", "Events", "Runs when a round ends. Winner is no team for a draw.", outputs=[E(), P("winner","team","Winner")], **ev)
node("event.player_killed", "Player Killed", "Events", "Runs when a player dies. Attacker is nobody when the world killed them (a fall, the bomb).", outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("headshot","bool","Headshot")], **ev)
node("event.player_spawned", "Player Spawned", "Events", "Runs when a player comes alive (each round, or a respawn).", outputs=[E(), P("player","player","Player")], **ev)
node("event.player_joined", "Player Joined", "Events", "Runs when a player (or bot) has joined the server.", outputs=[E(), P("player","player","Player")], **ev)
node("event.player_left", "Player Left", "Events", "Runs when a player leaves the server.", outputs=[E(), P("player","player","Player")], **ev)
node("event.bomb_planted", "Bomb Planted", "Events", "Runs when the bomb is planted.", outputs=[E(), P("player","player","Planter")], **ev)
node("event.bomb_defused", "Bomb Defused", "Events", "Runs when the bomb is defused.", outputs=[E(), P("player","player","Defuser")], **ev)
node("event.bomb_exploded", "Bomb Exploded", "Events", "Runs when the bomb goes off.", outputs=[E()], **ev)
node("event.player_used", "Player Used", "Events", "Runs when a player presses a button (func_button) with this name. Leave the name empty for any button.", outputs=[E(), P("player","player","Player"), P("entity","entity","Button")], params=[prm("name","Button name","name","")], **ev)
node("event.player_touched", "Player Touched", "Events", "Runs when a player walks into a trigger (trigger_multiple) with this name, at most once a second each. Empty name: any.", outputs=[E(), P("player","player","Player"), P("entity","entity","Trigger")], params=[prm("name","Trigger name","name","")], **ev)

# ---- component events (gm_components)
cev = dict(event=True, needs="components")
node("comp.on_capture", "Point Captured", "Components", "A capture point was taken.", outputs=[E(), P("point","entity","Point"), P("team","team","Team"), P("player","player","Capturer")], **cev)
node("comp.on_capture_lost", "Point Lost", "Components", "A team lost a capture point.", outputs=[E(), P("point","entity","Point"), P("team","team","Team")], **cev)
node("comp.on_zone_stage", "Zone Stage", "Components", "A shrinking zone started a new stage.", outputs=[E(), P("zone","entity","Zone"), P("stage","number","Stage")], **cev)
node("comp.on_zone_tick", "Zone Hurt Player", "Components", "A player outside the zone took damage.", outputs=[E(), P("zone","entity","Zone"), P("player","player","Player"), P("damage","number","Damage")], **cev)
node("comp.on_extract", "Player Extracted", "Components", "A player got out at an extraction point.", outputs=[E(), P("point","entity","Point"), P("player","player","Player")], **cev)
node("comp.on_wave_start", "Wave Started", "Components", "A wave of enemies started.", outputs=[E(), P("wave","number","Wave")], **cev)
node("comp.on_wave_spawn", "Wave Spawned One", "Components", "One enemy of a wave came in.", outputs=[E(), P("spawned","entity","Spawned"), P("wave","number","Wave")], **cev)
node("comp.on_wave_cleared", "Wave Cleared", "Components", "Every enemy of a wave is dead.", outputs=[E(), P("wave","number","Wave")], **cev)
node("comp.on_loot", "Crate Opened", "Components", "A player opened a loot container.", outputs=[E(), P("container","entity","Container"), P("player","player","Player")], **cev)
node("comp.on_checkpoint", "Checkpoint Reached", "Components", "A player reached a checkpoint.", outputs=[E(), P("point","entity","Checkpoint"), P("player","player","Player"), P("order","number","Order")], **cev)
node("comp.on_barricade_damaged", "Barricade Damaged", "Components", "A barricade was hit (Health is what it has left).", outputs=[E(), P("barricade","entity","Barricade"), P("attacker","entity","Attacker"), P("hp","number","Health")], **cev)
node("comp.on_barricade_repaired", "Barricade Repaired", "Components", "A barricade was mended a step (by Player; nobody when the mode did it).", outputs=[E(), P("barricade","entity","Barricade"), P("player","player","Player"), P("hp","number","Health")], **cev)
node("comp.on_drop_landed", "Supply Drop Landed", "Components", "A supply drop came down (it can be opened now).", outputs=[E(), P("drop","entity","Drop")], **cev)
node("comp.on_barricade_broken", "Barricade Broken", "Components", "A barricade was broken.", outputs=[E(), P("barricade","entity","Barricade"), P("attacker","player","Attacker")], **cev)
node("comp.on_meeting_end", "Meeting Ended", "Components", "A meeting ended; Ejected is nobody when no one was voted out.", outputs=[E(), P("meeting","entity","Meeting"), P("ejected","player","Ejected")], **cev)
node("creature.on_killed", "Creature Killed", "Creatures", "A creature died. Needs the Loner-Strike bot brain with creatures.", outputs=[E(), P("creature","entity","Creature"), P("killer","player","Killer"), P("kind","text","Type")], event=True, needs="creatures")

# ---- conditions and flow
node("cond.is_team", "Is Team", "Conditions", "Checks which team a player is on. Nobody (an empty attacker) is on no team, so that goes to False.",
     inputs=[X, P("player","player","Player",required=True), P("team","team","Team",param="team")], outputs=[E("true","True"), E("false","False")],
     params=[prm("team","Team","choice","t",choices=TEAM2)])
node("flow.branch", "Branch", "Conditions", "Goes to True or False.", inputs=[X, P("cond","bool","Condition",param="cond")], outputs=[E("true","True"), E("false","False")], params=[prm("cond","Condition","bool",True)])
node("math.compare", "Compare", "Conditions", "Compares two numbers.", inputs=[P("a","number","A",param="a"), P("b","number","B",param="b")], outputs=[P("out","bool","Result")],
     params=[prm("a","A","number",0,min=-100000,max=100000), prm("op","Is","choice","gt",choices=[["gt","greater than"],["ge","at least"],["lt","less than"],["le","at most"],["eq","equal to"],["ne","not equal to"]]), prm("b","B","number",0,min=-100000,max=100000)], pure=True)
node("logic.and_or", "And / Or", "Conditions", "Both (And), or either one (Or).", inputs=[P("a","bool","A",param="a"), P("b","bool","B",param="b")], outputs=[P("out","bool","Result")],
     params=[prm("a","A","bool",True), prm("op","Needs","choice","and",choices=[["and","both (And)"],["or","either (Or)"]]), prm("b","B","bool",True)], pure=True)
node("logic.not", "Not", "Conditions", "True becomes False and False becomes True.", inputs=[P("a","bool","In",required=True)], outputs=[P("out","bool","Result")], pure=True)
node("math.op", "Math", "Numbers", "Adds, takes away, multiplies, divides, or picks the smaller or bigger number.", inputs=[P("a","number","A",param="a"), P("b","number","B",param="b")], outputs=[P("out","number","Result")],
     params=[prm("a","A","number",0,min=-100000,max=100000), prm("op","Do","choice","add",choices=[["add","A + B"],["sub","A - B"],["mul","A × B"],["div","A ÷ B"],["min","smaller"],["max","bigger"],["mod","remainder of A ÷ B"],["pow","A to the power B"]]), prm("b","B","number",1,min=-100000,max=100000)], pure=True)
node("math.random", "Random Number", "Numbers", "A random number from Min to Max (whole numbers unless Fractions is on).", inputs=[P("min","number","Min",param="min"), P("max","number","Max",param="max")], outputs=[P("out","number","Number")],
     params=[prm("min","Min","number",1,min=-100000,max=100000), prm("max","Max","number",10,min=-100000,max=100000), prm("frac","Fractions","bool",False)], pure=True)
node("player.get_health", "Health Of", "Players", "A player's health (0 if dead or nobody).", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Health")], pure=True)
node("player.get_team", "Team Of", "Players", "Which team a player is on.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","team","Team")], pure=True)
node("player.is_alive", "Is Alive", "Players", "Is the player alive?", inputs=[P("player","player","Player",required=True)], outputs=[P("out","bool","Alive")], pure=True)
node("player.is_bot", "Is Bot", "Players", "Is the player a bot?", inputs=[P("player","player","Player",required=True)], outputs=[P("out","bool","Bot")], pure=True)
node("player.get_name", "Name Of", "Players", "A player's name, as text.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","text","Name")], pure=True)
node("game.count_alive", "Count Alive", "Players", "How many players of a team are alive (or everyone).", inputs=[P("team","team","Team",param="team")], outputs=[P("out","number","Alive")],
     params=[prm("team","Team","choice","none",choices=[["t","Terrorists"],["ct","Counter-Terrorists"],["none","Everyone"]])], pure=True)

# ---- variables
node("var.get", "Get Variable", "Variables", "Reads one of this mode's variables (a per-player one needs a Player, a per-team one a Team).", inputs=[P("player","player","Player",optional=True), P("team","team","Team",optional=True)], outputs=[P("out","var","Value")], params=[prm("var","Variable","var","")], pure=True)
node("var.set", "Set Variable", "Variables", "Sets one of this mode's variables.", inputs=[X, P("player","player","Player",optional=True), P("team","team","Team",optional=True), P("value","var","Value",param="value")], outputs=[E()], params=[prm("var","Variable","var",""), prm("value","Value","varvalue","")])
node("var.add", "Add To Variable", "Variables", "Adds to a number variable (a negative amount takes away).", inputs=[X, P("player","player","Player",optional=True), P("team","team","Team",optional=True), P("amount","number","Amount",param="amount")], outputs=[E()], params=[prm("var","Variable","var","",varType="number"), prm("amount","Amount","number",1,min=-100000,max=100000)])

# ---- timers
node("timer.delay", "Delay", "Timers", "Waits, then carries on. Paused with the world while debugging.", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E(), P("handle","timer","Timer")], params=[prm("secs","Seconds","number",3,min=0.1,max=3600)])
node("timer.repeat", "Repeat", "Timers", "Runs Each every few seconds, a number of times (0: until the round ends). Then runs at once.", inputs=[X, P("every","number","Every (s)",param="every")], outputs=[E("then",""), E("tick","Each"), P("handle","timer","Timer")],
     params=[prm("every","Every (seconds)","number",5,min=0.1,max=3600), prm("times","Times (0 = until the round ends)","number",0,min=0,max=10000)])
node("timer.countdown", "Countdown", "Timers", "Counts down on everyone's screen, then runs Done.", inputs=[X], outputs=[E("then",""), E("done","Done"), P("handle","timer","Timer")],
     params=[prm("secs","Seconds","number",10,min=1,max=600), prm("text","Text","text","Starting in",max=60), prm("slot","Where","choice","top",choices=SLOTS)])
node("timer.stop_all", "Stop Timers", "Timers", "Stops every Delay, Repeat and Countdown of this mode.", inputs=[X], outputs=[E()])

# ---- players
node("player.give_weapon", "Give Weapon", "Players", "Gives a living player a weapon and spare bullets. Does nothing for a dead player, or one who already has that weapon (the debugger marks that).",
     inputs=[X, P("player","player","Player",required=True)], outputs=[E()], params=[prm("weapon","Weapon","choice","ak47",choices=WEAPONS), prm("ammo","Spare bullets","number",90,min=0,max=250)])
node("player.strip", "Strip Weapons", "Players", "Takes a player's weapons away (keeping the knife if you like).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()], params=[prm("knife","Keep the knife","bool",True)])
node("player.set_health", "Set Health", "Players", "Sets a living player's health.", inputs=[X, P("player","player","Player",required=True), P("amount","number","Health",param="amount")], outputs=[E()], params=[prm("amount","Health","number",100,min=1,max=10000)])
node("player.set_armor", "Set Armor", "Players", "Sets a living player's armor (with a helmet).", inputs=[X, P("player","player","Player",required=True), P("amount","number","Armor",param="amount")], outputs=[E()], params=[prm("amount","Armor","number",100,min=0,max=1000)])
node("player.set_team", "Set Team", "Players", "Moves a player to a team (it shows from their next spawn).", inputs=[X, P("player","player","Player",required=True), P("team","team","Team",param="team")], outputs=[E()], params=[prm("team","Team","choice","ct",choices=TEAM2)])
node("player.set_speed", "Set Speed", "Players", "Sets how fast a player runs (250 is normal) until they switch weapons.", inputs=[X, P("player","player","Player",required=True), P("speed","number","Speed",param="speed")], outputs=[E()], params=[prm("speed","Speed","number",320,min=50,max=2000)])
node("player.teleport", "Teleport", "Players", "Moves a living player to an entity: the one wired into To, or the one with this name.", inputs=[X, P("player","player","Player",required=True), P("to","entity","To",optional=True)], outputs=[E()], params=[prm("name","To the entity named","name","spot1")])

# ---- messages
node("msg.broadcast", "Show Message", "Messages", "Shows a big message in the middle of the screen, to one player or everyone (no Player).", inputs=[X, P("player","player","Player",optional=True)], outputs=[E()],
     params=[prm("text","Text","text","Hello!",max=100), prm("seconds","Seconds","number",3,min=1,max=20)])
node("msg.hud", "Show HUD Text", "Messages", "Shows text in a HUD slot, to one player or everyone (no Player). A Value wired in goes after the text.",
     inputs=[X, P("player","player","Player",optional=True), P("value","any","Value",optional=True)], outputs=[E()],
     params=[prm("text","Text","text","Score:",max=100), prm("slot","Where","choice","left",choices=SLOTS), prm("seconds","Seconds","number",4,min=0.5,max=60)])

# ---- scoring
node("score.end_round", "End Round", "Scoring", "Ends the round now with a winner (no team: a draw). The next round starts after the usual short wait.", inputs=[X, P("winner","team","Winner",param="winner")], outputs=[],
     params=[prm("winner","Winner","choice","draw",choices=[["t","Terrorists"],["ct","Counter-Terrorists"],["draw","Draw"]])])

# ---- component actions
ca = dict(needs="components")
node("comp.find", "Find Component", "Components", "The component (or any entity) with this name.", outputs=[P("out","entity","Entity")], params=[prm("name","Name","entname","A")], pure=True, **ca)
node("comp.create", "Create Component", "Components", "Makes a component at an entity (wired into At, or the one with this name), with keys like radius=160 capture_secs=8.", inputs=[X, P("at_ent","entity","At",optional=True)], outputs=[E(), P("out","entity","Component")],
     params=[prm("kind","Component","choice","ls_capture_point",choices=COMPONENTS), prm("at","At the entity named","name","spot1"), prm("keys","Keys","keys","",max=400)], **ca)
node("comp.zone_set", "Set Zone", "Components", "Sets a safe zone's radius (wire the Safe zone, or the Shrinking zone that drives it), and moves its center to an entity wired into Center or the one with this name (no name: it stays where it is).", inputs=[X, P("zone","entity","Zone",required=True), P("at_ent","entity","Center",optional=True), P("radius","number","Radius",param="radius")], outputs=[E()],
     params=[prm("at","Center at the entity named","name",""), prm("radius","Radius","number",1500,min=50,max=20000)], **ca)
node("comp.zone_shrink", "Shrink Zone", "Components", "Shrinks a safe zone (or a Shrinking zone's) over some seconds to a new radius, around an entity wired into Center or the one with this name (no name: its center stays).", inputs=[X, P("zone","entity","Zone",required=True), P("at_ent","entity","Center",optional=True), P("radius","number","Radius",param="radius")], outputs=[E()],
     params=[prm("at","New center at the entity named","name",""), prm("radius","Radius","number",600,min=50,max=20000), prm("secs","Over (seconds)","number",30,min=1,max=600)], **ca)
node("comp.crate_open", "Open Crate", "Components", "Opens a loot container for a player.", inputs=[X, P("container","entity","Container",required=True), P("player","player","Player",required=True)], outputs=[E()], **ca)
node("comp.wave_start", "Start Wave", "Components", "Starts a wave (0: the next one).", inputs=[X, P("wave","number","Wave",param="wave")], outputs=[E()], params=[prm("wave","Wave","number",0,min=0,max=1000)], **ca)
node("comp.wave_stop", "Stop Waves", "Components", "Stops the waves.", inputs=[X], outputs=[E()], **ca)
node("comp.extract_open", "Open Extraction", "Components", "Opens or closes an extraction point.", inputs=[X, P("point","entity","Point",required=True), P("open","bool","Open",param="open")], outputs=[E()], params=[prm("open","Open","bool",True)], **ca)
node("comp.door", "Door", "Components", "Opens or closes a timed door.", inputs=[X, P("door","entity","Door",required=True)], outputs=[E()], params=[prm("do","Do","choice","open",choices=[["open","Open"],["close","Close"]])], **ca)
node("comp.barricade_set_hp", "Set Barricade Health", "Components", "Sets a barricade's health (0 breaks it; above 0 stands it back up).", inputs=[X, P("barricade","entity","Barricade",required=True), P("hp","number","Health",param="hp")], outputs=[E()], params=[prm("hp","Health","number",100,min=0,max=100000)], **ca)
node("comp.barricade_repair", "Repair Barricade", "Components", "Mends a barricade fully, or one step as if the wired Player held USE on it.", inputs=[X, P("barricade","entity","Barricade",required=True), P("player","player","Player",optional=True)], outputs=[E()], **ca)
node("comp.loot_roll", "Roll Loot", "Components", "Rolls one item from a loot table (configs/gamemodes/loot.ini), only items of this tier or lower (0: any).", inputs=[X, P("tier","number","Tier",param="tier")], outputs=[E(), P("item","text","Item"), P("rolled","bool","Rolled")], params=[prm("table","Table","name","default"), prm("tier","Tier (0: any)","number",0,min=0,max=10)], **ca)
node("comp.zone_skip", "Next Zone Stage Now", "Components", "A Shrinking zone goes to its next stage now.", inputs=[X, P("zone","entity","Zone",required=True)], outputs=[E()], **ca)
node("comp.trap_fire", "Fire Trap", "Components", "Sets off a trap.", inputs=[X, P("trap","entity","Trap",required=True)], outputs=[E()], **ca)
node("comp.capture_lock", "Lock Point", "Components", "Locks a capture point for some seconds.", inputs=[X, P("point","entity","Point",required=True), P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",30,min=1,max=3600)], **ca)
node("comp.reset", "Reset Components", "Components", "Puts every component back as it was at the start of the map.", inputs=[X], outputs=[E()], **ca)

# ---- creatures
node("creature.health", "Creature Health", "Creatures", "A creature's health now (0 when it's gone).", inputs=[P("in","entity","Creature",required=True)], outputs=[P("out","number","Health")], pure=True)
node("creature.spawn", "Spawn Creature", "Creatures", "Makes a creature of this type at an entity (wired into At, or the one with this name). Needs the Loner-Strike bot brain with creatures (the debugger says so if not).",
     inputs=[X, P("at_ent","entity","At",optional=True), P("health","number","Health",param="health"), P("pay","number","Pay",param="pay")], outputs=[E(), P("out","entity","Creature")],
     params=[prm("kind","Type","name","zombie"), prm("at","At the entity named","name","spot1"), prm("health","Health (0: the type's own)","number",0,min=0,max=100000), prm("tier","Tier (looks and enrage; 0 none)","choice","0",choices=[["0","None"],["1","1"],["2","2"],["3","3"]]), prm("pay","Pay its killer ($)","number",0,min=0,max=16000)], needs="creatures")

# ---- milestone 3: what whole modes (Last Stand, Battle Royale) need
WHICH = [["all","Everyone in the game"],["alive","Everyone alive"],["dead","Everyone dead"],["t","Terrorists"],["ct","Counter-Terrorists"],["alive_t","Living Terrorists"],["alive_ct","Living Counter-Terrorists"],["bots","Bots"],["humans","People (not bots)"]]
BY = [["class","Of this kind (classname)"],["name","With this name"]]
node("event.player_hurt", "Player Hurt", "Events", "Runs when a player takes damage, with the health it took (blocked damage doesn't count).", outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("damage","number","Damage")], **ev)
node("event.team_changed", "Team Changed", "Events", "Runs once when a player joins a team or changes team (Spectator is No team).", outputs=[E(), P("player","player","Player"), P("team","team","Team")], **ev)
node("event.map_started", "Map Started", "Events", "Runs once when the map has loaded (before the first round): load map packs, set things up.", outputs=[E()], **ev)
node("event.new_round", "New Round", "Events", "Runs when a new round is set up (freeze time starts), before Round Started.", outputs=[E()], **ev)
node("event.chat_command", "Chat Command", "Events", "Runs when a player says /command or !command in chat (shop for /shop and !shop).", outputs=[E(), P("player","player","Player")], params=[prm("command","Command (without /)","name","shop")], **ev)
node("func.define", "Function", "Functions", "A piece of graph you can run from anywhere with Call Function. It gets a Player and a Number from the call.", outputs=[E(), P("player","player","Player"), P("number","number","Number")], params=[prm("name","Name","name","my_function")], event=True)
node("func.call", "Call Function", "Functions", "Runs a Function, giving it a Player and a Number, then carries on.", inputs=[X, P("player","player","Player",optional=True), P("number","number","Number",param="number")], outputs=[E()],
     params=[prm("func","Function","func",""), prm("number","Number","number",0,min=-100000,max=100000)])
node("flow.for_each_player", "For Each Player", "Flow", "Runs Each once for every player who fits, then Then.", inputs=[X], outputs=[E("each","Each"), P("player","player","Player"), E("then","Then")], params=[prm("which","Who","choice","alive",choices=WHICH)])
node("flow.for_each_entity", "For Each Entity", "Flow", "Runs Each once for every entity of a kind or with a name (at most 512), then Then.", inputs=[X], outputs=[E("each","Each"), P("entity","entity","Entity"), E("then","Then")],
     params=[prm("by","Which","choice","class",choices=BY), prm("name","Kind or name","name","ls_respawn_point")])
node("flow.pick_player", "Pick Random Player", "Flow", "Picks one player who fits at random (Found), or runs None.", inputs=[X], outputs=[E("then","Found"), E("none","None"), P("player","player","Player")], params=[prm("which","Who","choice","alive",choices=WHICH)])
node("flow.pick_entity", "Pick Random Entity", "Flow", "Picks one entity of a kind or with a name at random (Found), or runs None.", inputs=[X], outputs=[E("then","Found"), E("none","None"), P("entity","entity","Entity")],
     params=[prm("by","Which","choice","class",choices=BY), prm("name","Kind or name","name","info_player_deathmatch")])
node("text.equals", "Text Is", "Conditions", "Is the text the same as this (capitals don't matter)?", inputs=[P("a","text","Text",required=True), P("b","text","Same as",param="b")], outputs=[P("out","bool","Same")], params=[prm("b","Same as","text","",max=60)], pure=True)
node("timer.stop", "Stop Timer", "Timers", "Stops one Delay, Repeat or Countdown.", inputs=[X], outputs=[E()], params=[prm("timer","Timer","timer","")])
node("player.add_money", "Give Money", "Players", "Gives (or takes, with a minus) money; it stays between 0 and 16000.", inputs=[X, P("player","player","Player",required=True), P("amount","number","Amount",param="amount")], outputs=[E()], params=[prm("amount","Amount","number",500,min=-16000,max=16000)])
node("player.get_money", "Money Of", "Players", "How much money a player has.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Money")], pure=True)
node("player.add_frags", "Add Score", "Players", "Adds to a player's score (frags) on the scoreboard (a minus takes away).", inputs=[X, P("player","player","Player",required=True), P("amount","number","Amount",param="amount")], outputs=[E()], params=[prm("amount","Amount","number",1,min=-1000,max=1000)])
node("player.get_frags", "Score Of", "Players", "A player's score (frags).", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Score")], pure=True)
node("player.get_deaths", "Deaths Of", "Players", "How many times a player has died.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Deaths")], pure=True)
node("player.give_item", "Give Item", "Players", "Gives a living player armor, a defuse kit or night vision.", inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("item","Item","choice","vesthelm",choices=[["vest","Kevlar"],["vesthelm","Kevlar and helmet"],["defuser","Defuse kit"],["nvg","Night vision"]])])
node("player.drop_weapon", "Drop Weapon", "Players", "Makes a living player drop the weapon in their hands.", inputs=[X, P("player","player","Player",required=True)], outputs=[E()])
node("player.set_gravity", "Set Gravity", "Players", "How strongly a player falls (1 is normal, 0.5 half).", inputs=[X, P("player","player","Player",required=True), P("gravity","number","Gravity",param="gravity")], outputs=[E()], params=[prm("gravity","Gravity","number",0.5,min=0.05,max=4)])
node("player.set_glow", "Set Glow", "Players", "Makes a player glow in a color (or not).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("color","Color","choice","red",choices=[["off","No glow"],["red","Red"],["green","Green"],["blue","Blue"],["yellow","Yellow"],["white","White"]])])
node("player.set_model", "Set Model", "Players", "Dresses a player as one of the game's characters (or back to their own).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("model","Model","choice","gign",choices=[["reset","Their own"],["urban","SEAL Team 6"],["gsg9","GSG-9"],["sas","SAS"],["gign","GIGN"],["terror","Phoenix"],["leet","Elite Crew"],["arctic","Arctic"],["guerilla","Guerilla"]])])
node("player.respawn", "Respawn", "Players", "Brings a dead player back at a spawn point.", inputs=[X, P("player","player","Player",required=True)], outputs=[E()])
node("player.kill", "Kill", "Players", "Kills a living player.", inputs=[X, P("player","player","Player",required=True)], outputs=[E()])
node("player.freeze", "Freeze", "Players", "Freezes a player in place (or lets them move again).", inputs=[X, P("player","player","Player",required=True), P("on","bool","Frozen",param="on")], outputs=[E()], params=[prm("on","Frozen","bool",True)])
node("player.set_role", "Set Role", "Players", "Gives a player a role in this mode (traitor, medic...).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()], params=[prm("role","Role","name","medic")])
node("player.is_role", "Is Role", "Players", "Does the player have this role?", inputs=[P("player","player","Player",required=True)], outputs=[P("out","bool","Has it")], params=[prm("role","Role","name","medic")], pure=True)
node("game.count_role", "Count Role", "Players", "How many living players have this role.", outputs=[P("out","number","Count")], params=[prm("role","Role","name","medic")], pure=True)
node("msg.chat", "Chat Message", "Messages", "A chat line, to one player or everyone (no Player). A Value wired in goes after the text.", inputs=[X, P("player","player","Player",optional=True), P("value","any","Value",optional=True)], outputs=[E()],
     params=[prm("text","Text","text","Hello!",max=120)])
node("msg.bar", "Progress Bar", "Messages", "Shows the game's progress bar filling up over some seconds.", inputs=[X, P("player","player","Player",optional=True)], outputs=[E()], params=[prm("secs","Seconds","number",5,min=1,max=60)])
node("msg.menu", "Menu", "Messages", "Shows a player a menu; the choice they pick runs its output (later, when they pick).", inputs=[X, P("player","player","Player",required=True)],
     outputs=[E(), E("c1","1"), E("c2","2"), E("c3","3"), E("c4","4")],
     params=[prm("title","Title","text","Pick one",max=40), prm("o1","Choice 1","text","First",max=40), prm("o2","Choice 2","text","Second",max=40), prm("o3","Choice 3","text","",max=40), prm("o4","Choice 4","text","",max=40)])
node("game.play_sound", "Play Sound", "Messages", "Plays one of the game's sounds to one player or everyone.", inputs=[X, P("player","player","Player",optional=True)], outputs=[E()],
     params=[prm("sound","Sound","choice","radio/go",choices=[["radio/go","Radio: Go go go"],["radio/locknload","Radio: Lock and load"],["radio/ctwin","Radio: CTs win"],["radio/terwin","Radio: Terrorists win"],["radio/rounddraw","Radio: Round draw"],["radio/bombpl","Radio: Bomb planted"],["buttons/bell1","Bell"],["buttons/blip1","Blip"],["weapons/c4_beep1","C4 beep"],["ambience/thunder_clap","Thunder"]])])
node("game.set_cvar", "Set Setting", "Game", "Changes one of the game's settings now.", inputs=[X, P("value","number","Value",param="value")], outputs=[E()],
     params=[prm("cvar","Setting","choice","mp_freeforall",choices=[["mp_freeforall","Free for all (1 on)"],["mp_friendlyfire","Friendly fire (1 on)"],["sv_gravity","Gravity (800 normal)"],["mp_buytime","Buy time (minutes)"],["mp_startmoney","Start money"],["sv_airaccelerate","Air control (10 normal)"],["mp_roundtime","Round time (minutes, next round)"],["mp_freezetime","Freeze time (seconds, next round)"],["mp_round_infinite","Rounds only end when the mode says (1 on)"],["mp_autoteambalance","Team balance (1 on)"],["mp_limitteams","Most players more than the other team (0 any)"]]),
             prm("value","Value","number",1,min=0,max=100000)])
node("entity.use", "Use Entity", "Game", "Presses (uses) every entity with this name: a door, a button, a trigger.", inputs=[X], outputs=[E()], params=[prm("name","Name","name","door1")])
# the pieces our Last Stand and Battle Royale are built from (gm_components: W1's and W3's)
PERKS = [["tough","Tough (+50 % health)"],["hands","Quick hands (reload, swap)"],["doubletap","Double tap (+25 % damage)"],["revive","Quick revive"],["feet","Light feet (+10 % speed)"]]
MODS = [["health_max","Max health"],["speed","Speed"],["reload","Reload time"],["deploy","Weapon swap time"],["damage","Damage"],["revive_time","Revive time"],["fire_rate","Fire rate"]]
PL = P("player","player","Player",required=True)
node("up.mod", "Set Player Modifier", "Upgrades", "Multiplies something about a player until they die: max health, speed, reload or swap time (0.6 = 40 % faster), damage, revive time, fire rate (1.25 = 25 % faster). Perks multiply on top.", inputs=[X, PL, P("mult","number","Times",param="mult")], outputs=[E()], params=[prm("what","What","choice","speed",choices=MODS), prm("mult","Times","number",1.2,min=0.1,max=10)], **ca)
node("up.mod_get", "Player Modifier", "Upgrades", "What a player's modifier is now (the mode's times the perks').", inputs=[PL], outputs=[P("out","number","Times")], params=[prm("what","What","choice","speed",choices=MODS)], pure=True, needs="components")
node("up.perk_give", "Give Perk", "Upgrades", "Gives a player a perk (lost when downed or dead).", inputs=[X, PL], outputs=[E()], params=[prm("perk","Perk","choice","tough",choices=PERKS)], **ca)
node("up.perk_buy", "Buy Perk", "Upgrades", "A player buys a perk for its price (1500 + 1000 per perk they have, 4 at most); Bought says if it worked.", inputs=[X, PL], outputs=[E(), P("bought","bool","Bought")], params=[prm("perk","Perk","choice","tough",choices=PERKS)], **ca)
node("up.perk_has", "Has Perk", "Upgrades", "Does the player have this perk?", inputs=[PL], outputs=[P("out","bool","Has it")], params=[prm("perk","Perk","choice","tough",choices=PERKS)], pure=True, needs="components")
node("up.perk_count", "Perk Count", "Upgrades", "How many perks a player has.", inputs=[PL], outputs=[P("out","number","Perks")], pure=True, needs="components")
node("up.weapon_level", "Weapon Level", "Upgrades", "The level of the gun in the player's hand (1, 2 Mk II, 3 Mk III).", inputs=[PL], outputs=[P("out","number","Level")], pure=True, needs="components")
node("up.bench", "Upgrade Gun", "Upgrades", "Upgrades the gun in the player's hand at the bench's prices (or 5000 / 9000).", inputs=[X, PL], outputs=[E()], **ca)
node("up.box", "Roll Mystery Box", "Upgrades", "The player rolls the mystery box (the one that's out, or the wired one) for its price.", inputs=[X, PL, P("box","entity","Box",optional=True)], outputs=[E()], **ca)
node("up.pickup", "Drop Pickup", "Upgrades", "Drops a pickup at an entity; whoever walks over it takes it (Pickup Taken says what it was). A kind like resupply, doublepay, onehit, airstrike, or anything you handle yourself.", inputs=[X, P("at","entity","At",required=True)], outputs=[E(), P("out","entity","Pickup")],
     params=[prm("kind","Kind","name","resupply"), prm("life","Lasts (seconds)","number",25,min=1,max=600), prm("colour","Colour","choice","gold",choices=[["white","White"],["green","Green"],["blue","Blue"],["purple","Purple"],["gold","Gold"],["red","Red"]])], **ca)
node("up.on_perk", "Perk Bought", "Upgrades", "A player bought a perk (at a machine, a shop or Buy Perk).", outputs=[E(), P("player","player","Player"), P("perk","text","Perk"), P("price","number","Price")], **cev)
node("up.on_upgraded", "Gun Upgraded", "Upgrades", "A player's gun went up a level.", outputs=[E(), P("player","player","Player"), P("level","number","Level"), P("price","number","Price")], **cev)
node("up.on_box", "Mystery Box Gave", "Upgrades", "The mystery box gave a player a gun.", outputs=[E(), P("player","player","Player"), P("weapon","text","Gun")], **cev)
node("up.on_pickup", "Pickup Taken", "Upgrades", "A player took a pickup.", outputs=[E(), P("pickup","entity","Pickup"), P("player","player","Player"), P("kind","text","Kind")], **cev)
node("up.on_creature_hurt", "Creature Hurt", "Creatures", "A creature took damage (Headshot when the hit was a head).", outputs=[E(), P("creature","entity","Creature"), P("attacker","player","Attacker"), P("damage","number","Damage"), P("headshot","bool","Headshot")], **cev)
# knocked down (the components' downed state, as Last Stand and Battle Royale use it)
node("kd.enable", "Knocked Down On", "Downed", "On: a hit that would kill a player (of a team, or anyone) knocks them down instead: they crawl and bleed out over some seconds; a teammate (a squad mate, with squads) holding USE for 4 s gets them up with 50 health. Off: nobody new goes down.",
     inputs=[X], outputs=[E()], params=[prm("on","On","bool",True), prm("bleed","Bleed out after (seconds)","number",30,min=3,max=300), prm("team","Who","choice","0",choices=[["0","Anyone"],["1","Terrorists"],["2","Counter-Terrorists"]]), prm("perks","They lose their perks","bool",True)], **ca)
node("kd.is", "Is Knocked Down", "Downed", "Is the player knocked down?", inputs=[PL], outputs=[P("out","bool","Down")], pure=True, needs="components")
node("kd.left", "Bleed Out In", "Downed", "Seconds a knocked-down player has left (0 when they're not down).", inputs=[PL], outputs=[P("out","number","Seconds")], pure=True, needs="components")
node("kd.down", "Knock Down", "Downed", "Knocks a player down now.", inputs=[X, PL], outputs=[E()], **ca)
node("kd.revive", "Get Up", "Downed", "Gets a knocked-down player up (by the wired player, or nobody).", inputs=[X, PL, P("by","player","By",optional=True)], outputs=[E()], **ca)
node("kd.on_down", "Knocked Down", "Downed", "A player was knocked down.", outputs=[E(), P("player","player","Player"), P("attacker","entity","By")], **cev)
node("kd.on_up", "Got Up", "Downed", "A knocked-down player is back up (By: who helped; themselves for Quick revive; nobody when the mode did it).", outputs=[E(), P("player","player","Player"), P("by","player","By")], **cev)
node("kd.on_bled_out", "Bled Out", "Downed", "A knocked-down player bled out (they die right after).", outputs=[E(), P("player","player","Player")], **cev)
# the shop (ls_shop: a counter with a menu of priced items and a keeper)
node("shop.menu", "Open Shop Menu", "Shop", "Shows a player the shop's menu.", inputs=[X, P("shop","entity","Shop",required=True), PL], outputs=[E()], **ca)
node("shop.open", "Open or Close Shop", "Shop", "Opens the shop for business, or closes it.", inputs=[X, P("shop","entity","Shop",required=True), P("open","bool","Open",param="open")], outputs=[E()], params=[prm("open","Open","bool",True)], **ca)
node("shop.price", "Shop Price", "Shop", "An item's price now (by its name or what it gives); -1 if the shop doesn't sell it.", inputs=[P("shop","entity","Shop",required=True)], outputs=[P("out","number","Price")], params=[prm("item","Item","text","ammo",max=40)], pure=True, needs="components")
node("shop.on_buy", "Shop Purchase", "Shop", "A player bought something at a shop (an @item is the mode's to give here).", outputs=[E(), P("shop","entity","Shop"), P("player","player","Player"), P("item","text","Item"), P("price","number","Price")], **cev)
# Battle Royale pieces (any mode)
node("br.heal_give", "Give Healing Item", "Battle Royale", "Gives a player bandages (+15 up to 75), medkits (full health) or drinks (+40 over 10 s), up to the carry limit.", inputs=[X, PL, P("n","number","How many",param="n")], outputs=[E()], params=[prm("kind","Item","choice","bandage",choices=[["bandage","Bandage"],["medkit","Medkit"],["drink","Drink"]]), prm("n","How many","number",1,min=1,max=10)], **ca)
node("br.heal_count", "Healing Items", "Battle Royale", "How many healing items a player carries.", inputs=[PL], outputs=[P("out","number","How many")], pure=True, needs="components")
node("br.armor", "Set Armor Levels", "Battle Royale", "Sets a player's vest and helmet levels (1-3; 0 none; keep: as they are).", inputs=[X, PL], outputs=[E()], params=[prm("vest","Vest","choice","2",choices=[["-1","Keep"],["0","None"],["1","1"],["2","2"],["3","3"]]), prm("helmet","Helmet","choice","1",choices=[["-1","Keep"],["0","None"],["1","1"],["2","2"],["3","3"]])], **ca)
node("br.crate", "Make Crate", "Battle Royale", "Makes a crate at an entity with these items (loot items: ak47@30@60 bandage:2 vest2).", inputs=[X, P("at","entity","At",required=True)], outputs=[E(), P("out","entity","Crate")], params=[prm("items","Items","keys","bandage:2 ammo",max=120)], **ca)
node("br.glow", "Rarity Glow", "Battle Royale", "Makes an entity glow in a rarity's colour (0 none, 1 white, 2 green, 3 blue, 4 purple, 5 gold).", inputs=[X, P("ent","entity","Entity",required=True), P("tier","number","Rarity",param="tier")], outputs=[E()], params=[prm("tier","Rarity","number",3,min=0,max=5)], **ca)
node("br.hud", "Match HUD", "Battle Royale", "Turns the match HUD on or off (players left, kills, the zone's timer, armor, healing items, the kill feed and the squad).", inputs=[X], outputs=[E()], params=[prm("on","On","bool",True)], **ca)
node("br.hud_text", "Match HUD Words", "Battle Royale", "The mode's words in the match HUD in place of the zone's timer (empty: the zone again).", inputs=[X, P("text","text","Text",param="text")], outputs=[E()], params=[prm("text","Text","text","Loot up!",max=60)], **ca)
node("br.feed", "Kill Feed Line", "Battle Royale", "Adds a line to the match HUD's kill feed.", inputs=[X, P("text","text","Text",param="text")], outputs=[E()], params=[prm("text","Text","text","",max=60)], **ca)
node("br.kills", "Match Kills", "Battle Royale", "A player's kills this round (counted while the match HUD is on).", inputs=[PL], outputs=[P("out","number","Kills")], pure=True, needs="components")
node("br.drop_in", "Drop In", "Battle Royale", "Drops a player in from high over a spot (an entity wired into At, else a spread-out landing spot: the map's br_drop markers); they fall slowly and take no damage until they land.", inputs=[X, PL, P("at","entity","At",optional=True)], outputs=[E()], **ca)
node("br.on_landed", "Landed", "Battle Royale", "A player who dropped in is down on the ground.", outputs=[E(), P("player","player","Player")], **cev)
node("br.lobby", "Squad Lobby", "Battle Royale", "A lobby of some seconds where people team up (say /squad); then the squads are filled (Lobby Ended). 0: fill them now.", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",30,min=0,max=300)], **ca)
node("br.squad_size", "Squad Size", "Battle Royale", "Sets the squad size: 1 solo, 2 duos, 4 squads...", inputs=[X, P("size","number","Size",param="size")], outputs=[E()], params=[prm("size","Size","number",4,min=1,max=8)], **ca)
node("br.on_lobby_end", "Lobby Ended", "Battle Royale", "The squad lobby is over and the squads are filled.", outputs=[E(), P("squads","number","Squads")], **cev)
node("br.squads_alive", "Sides Alive", "Battle Royale", "How many sides have someone alive (squads, and players in none).", outputs=[P("out","number","Sides")], pure=True, needs="components")

# what the mode tells the bots' brain (ls_mode hints, through gm_components): so community modes get bots that play them well
BH = dict(needs="components")
BWHO = P("player","player","Bot",optional=True)
node("bots.phase", "Bots: Phase", "Bots", "Tells the bots what part of the game it is: Loot (nobody is an enemy: they gear up and keep apart), Fight, or back to normal.", inputs=[X], outputs=[E()], params=[prm("phase","Phase","choice","loot",choices=[["loot","Loot (no enemies)"],["fight","Fight"],["off","Normal"]])], **BH)
node("bots.no_buy", "Bots: Don't Buy", "Bots", "Bots skip buying (they leave the buy time at once and get on with the mode), or buy again.", inputs=[X, P("on","bool","Don't buy",param="on")], outputs=[E()], params=[prm("on","Don't buy","bool",True)], **BH)
node("bots.friends", "Bots: Friends", "Bots", "Makes two groups friends for the bots (they won't attack each other and may stick together), or not. A tag is set with Bots: Tag.", inputs=[X, P("a_player","player","Player A",optional=True), P("b_player","player","Player B",optional=True), P("on","bool","Friends",param="on")], outputs=[E()],
     params=[prm("a","Group A","choice","tag",choices=[["all","Everyone"],["ct","Counter-Terrorists"],["t","Terrorists"],["player","Player A (wired)"],["tag","Tagged"]]), prm("a_tag","A's tag","name","squad1"), prm("b","Group B","choice","tag",choices=[["all","Everyone"],["ct","Counter-Terrorists"],["t","Terrorists"],["player","Player B (wired)"],["tag","Tagged"]]), prm("b_tag","B's tag","name","squad1"), prm("on","Friends","bool",True)], **BH)
node("bots.tag", "Bots: Tag", "Bots", "Gives a player a tag (a squad, a job) so other Bots nodes can name them all at once.", inputs=[X, P("player","player","Player",required=True)], outputs=[E()], params=[prm("tag","Tag","name","squad1")], **BH)
node("bots.layer", "Bots: Prefer Layer", "Bots", "Bots look for loot and hold on this layer of the map: below (tunnels, basements), the surface, or high (roofs, ridges); or their own choice again.", inputs=[X, BWHO], outputs=[E()], params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("layer","Layer","choice","surface",choices=[["below","Below"],["surface","Surface"],["high","High"],["-","Their own choice"]])], **BH)
node("bots.creature_enemies", "Bots: Fight Creatures", "Bots", "Bots treat the mode's creatures as enemies (on by default where creatures attack), or not.", inputs=[X, P("on","bool","Fight them",param="on")], outputs=[E()], params=[prm("on","Fight them","bool",True)], **BH)
node("bots.goal", "Bots: Goal", "Bots", "Gives bots something to do at an entity: defend or capture it, use it (hold USE), loot it, or go and hold there, with a priority (0-100), for some seconds, within a radius.", inputs=[X, BWHO, P("at","entity","At",required=True), P("prio","number","Priority",param="prio"), P("secs","number","For (seconds)",param="secs")], outputs=[E()],
     params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("kind","Goal","choice","defend",choices=[["defend","Defend it"],["capture","Capture it"],["use","Use it"],["loot","Loot it"],["go_hold","Go and hold there"]]), prm("prio","Priority","number",50,min=0,max=100), prm("secs","For (seconds)","number",60,min=1,max=3600), prm("radius","Within (units)","number",64,min=8,max=4000)], **BH)
node("bots.goal_clear", "Bots: Clear Goals", "Bots", "Takes back goals given with Bots: Goal (all of them, or one kind).", inputs=[X, BWHO], outputs=[E()], params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("kind","Goals","choice","0",choices=[["0","All"],["defend","Defend"],["capture","Capture"],["use","Use"],["loot","Loot"],["go_hold","Go and hold"]])], **BH)
node("bots.offer", "Bots: Offer", "Bots", "Tells bots something here can be bought or used for a price (a perk, an upgrade bench, a mystery box, a trap, a turret spot); they take it like a person would, when they can afford it and it's calm.", inputs=[X, BWHO, P("at","entity","At",required=True), P("cost","number","Price",param="cost")], outputs=[E()],
     params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("kind","What","choice","perk",choices=[["perk","A perk"],["bench","An upgrade bench"],["box","A mystery box"],["trap","A trap's switch"],["turret","A turret spot"]]), prm("cost","Price","number",1000,min=0,max=100000), prm("order","Order (perks: lowest first)","number",0,min=0,max=20)], **BH)
node("bots.offer_clear", "Bots: Clear Offer", "Bots", "Takes back an offer (it's bought or used up), or all of them.", inputs=[X, BWHO, P("at","entity","At",optional=True)], outputs=[E()], params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name","")], **BH)
node("bot.goto", "Bot Go To", "Bots", "Sends a bot to the entity with this name (it still fights on the way), for some seconds (0: until freed).", inputs=[X, P("bot","player","Bot",required=True)], outputs=[E()], params=[prm("name","To the entity named","name","spot1"), prm("secs","For (seconds, 0 until freed)","number",0,min=0,max=600)])
node("bot.hold", "Bot Hold", "Bots", "Makes a bot hold where it is for some seconds.", inputs=[X, P("bot","player","Bot",required=True), P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",20,min=1,max=600)])
node("bot.follow", "Bot Follow", "Bots", "Makes a bot follow a player for some seconds.", inputs=[X, P("bot","player","Bot",required=True), P("who","player","Follow",required=True)], outputs=[E()], params=[prm("secs","Seconds","number",30,min=1,max=600)])
node("bot.release", "Bot Free", "Bots", "Lets a bot do as it likes again.", inputs=[X, P("bot","player","Bot",required=True)], outputs=[E()])
node("comp.load_pack", "Load Map Pack", "Components", "Loads another mode's map pack for this map (configs/gamemodes/components/<map>.<pack>.ini): br, laststand...", inputs=[X], outputs=[E()], params=[prm("pack","Pack","name","br")], needs="components")
node("comp.start", "Start Component", "Components", "Starts a component that waits to be started (a zone shrinker with start_delay -1).", inputs=[X, P("comp","entity","Component",required=True)], outputs=[E()], needs="components")
node("comp.stop", "Stop Component", "Components", "Stops a component.", inputs=[X, P("comp","entity","Component",required=True)], outputs=[E()], needs="components")
node("comp.barricade_hp", "Barricade Health", "Components", "A barricade's health now.", inputs=[P("barricade","entity","Barricade",required=True)], outputs=[P("out","number","Health")], pure=True, needs="components")
node("comp.zone_stage", "Zone Stage", "Components", "The Shrinking zone's stage (the zone or its shrinker); 0 before the first.", inputs=[P("zone","entity","Zone",required=True)], outputs=[P("out","number","Stage")], pure=True, needs="components")
node("comp.zone_next", "Zone Next", "Components", "When the zone next moves: seconds until it starts (or, while it's closing, until it stops), its next radius, whether it's closing now. Seconds is -1 when nothing is planned.", inputs=[P("zone","entity","Zone",required=True)], outputs=[P("secs","number","Seconds"), P("radius","number","Next radius"), P("closing","bool","Closing")], pure=True, needs="components")
node("comp.in_zone", "Is In Zone", "Components", "Is the player inside the zone?", inputs=[P("zone","entity","Zone",required=True), P("player","player","Player",required=True)], outputs=[P("out","bool","Inside")], pure=True, needs="components")
node("comp.respawn_at", "Respawn At Point", "Components", "Brings a dead player back at a respawn point (ls_respawn_point), or moves a living one there.", inputs=[X, P("player","player","Player",required=True)], outputs=[E()], needs="components")
node("comp.drop_call", "Call Supply Drop", "Components", "Calls a supply drop onto an entity (wired into At, or the one with this name).", inputs=[X, P("at_ent","entity","At",optional=True)], outputs=[E()], params=[prm("at","At the entity named","name","drop1")], needs="components")
node("creature.count", "Creatures Alive", "Creatures", "How many creatures are alive.", outputs=[P("out","number","Alive")], pure=True, needs="creatures")

# ---- milestone 4: revive, damage, squads, buying, saved stats, round clock
node("event.player_about_hurt", "Player About To Be Hurt", "Events", "Runs just before a player takes damage. Block, scale or set the damage here (it happens straight after this flow).",
     outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("damage","number","Damage"), P("kind","text","Kind")], **ev)
node("dmg.block", "Block Damage", "Damage", "The damage doesn't happen. Only in a Player About To Be Hurt flow.", inputs=[X], outputs=[E()])
node("dmg.scale", "Scale Damage", "Damage", "Multiplies the damage (0.5 halves it). Only in a Player About To Be Hurt flow.", inputs=[X, P("factor","number","Times",param="factor")], outputs=[E()], params=[prm("factor","Times","number",0.5,min=0,max=100)])
node("dmg.set", "Set Damage", "Damage", "Sets the damage to this. Only in a Player About To Be Hurt flow.", inputs=[X, P("amount","number","Damage",param="amount")], outputs=[E()], params=[prm("amount","Damage","number",10,min=0,max=10000)])
node("player.no_fall", "No Fall Damage", "Players", "Stops a player taking fall damage: always, or just for their next landing (a drop-in).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("mode","When","choice","once",choices=[["once","Their next landing"],["on","Always"],["off","Off again"]])])
node("player.down", "Make Downed", "Players", "A player who just died lies downed where they fell: a teammate (or squad mate) holding USE on them for a few seconds brings them back; if nobody does in time they're out. Bots come to help.",
     inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("secs","Out after (seconds)","number",30,min=3,max=300), prm("hold","Hold USE (seconds)","number",3,min=1,max=20), prm("health","Health when revived","number",50,min=1,max=255)])
node("player.is_downed", "Is Downed", "Players", "Is the player downed (waiting for a revive)?", inputs=[P("player","player","Player",required=True)], outputs=[P("out","bool","Downed")], pure=True)
node("event.player_revived", "Player Revived", "Events", "Runs when a downed player is brought back.", outputs=[E(), P("player","player","Player"), P("by","player","Revived by")], **ev)
node("event.down_timeout", "Downed Player Out", "Events", "Runs when nobody revived a downed player in time.", outputs=[E(), P("player","player","Player")], **ev)
node("squad.set", "Set Squad", "Squads", "Puts a player in a squad (1 to 32; 0 none). Squad mates are on one side for the bots, and can't hurt each other unless squad friendly fire is on.",
     inputs=[X, P("player","player","Player",required=True), P("squad","number","Squad",param="squad")], outputs=[E()], params=[prm("squad","Squad","number",1,min=0,max=32)])
node("squad.get", "Squad Of", "Squads", "The squad a player is in (0 none).", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Squad")], pure=True)
node("squad.alive", "Squads Left", "Squads", "How many squads still have someone alive (a living player in no squad counts as one).", outputs=[P("out","number","Squads")], pure=True)
node("squad.friendly_fire", "Squad Friendly Fire", "Squads", "Whether squad mates can hurt each other.", inputs=[X, P("on","bool","On",param="on")], outputs=[E()], params=[prm("on","On","bool",False)])
node("buy.where", "Buying", "Buying", "Where players can buy: anywhere, only in buy zones, or nowhere.", inputs=[X], outputs=[E()],
     params=[prm("where","Where","choice","anywhere",choices=[["anywhere","Anywhere"],["zones","Only in buy zones"],["nowhere","Nowhere (buying off)"]])])
node("buy.allow", "Allow Buying", "Buying", "Lets players buy only some things (or everything again).", inputs=[X], outputs=[E()],
     params=[prm("allow","What","choice","all",choices=[["all","Everything"],["pistols","Pistols, armor and grenades"],["no_snipers","Everything but sniper rifles"],["no_awp","Everything but the AWP"],["no_grenades","Everything but grenades"],["no_mg","Everything but machine guns"],["armor","Armor only"],["knife","Nothing at all (the spawn pistol stays)"]])])
node("save.get", "Saved Value", "Saved", "A number kept for this player across maps, restarts and crashes (0 until set). Each mode has its own file.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Value")], params=[prm("key","Name","name","wins")], pure=True)
node("save.set", "Save Value", "Saved", "Keeps a number for this player across maps and restarts.", inputs=[X, P("player","player","Player",required=True), P("value","number","Value",param="value")], outputs=[E()], params=[prm("key","Name","name","wins"), prm("value","Value","number",0,min=-100000,max=100000)])
node("save.add", "Add To Saved", "Saved", "Adds to a saved number (a minus takes away).", inputs=[X, P("player","player","Player",required=True), P("amount","number","Amount",param="amount")], outputs=[E()], params=[prm("key","Name","name","wins"), prm("amount","Amount","number",1,min=-100000,max=100000)])
node("clock.set", "Set Round Clock", "Round clock", "Runs the mode's own round clock: this many seconds, shown on everyone's round timer. When it reaches 0, Round Time Up runs (the game's own time limit is switched off while it runs).",
     inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",120,min=1,max=7200)])
node("clock.add", "Add Round Time", "Round clock", "Adds seconds to the mode's round clock (a minus takes away).", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",30,min=-7200,max=7200)])
node("clock.pause", "Pause Round Clock", "Round clock", "Stops or restarts the mode's round clock.", inputs=[X], outputs=[E()], params=[prm("do","Do","choice","pause",choices=[["pause","Pause"],["resume","Resume"]])])
node("clock.left", "Round Time Left", "Round clock", "Seconds left on the mode's round clock (0 when it isn't running).", outputs=[P("out","number","Seconds")], pure=True)
node("player.launch", "Launch Up", "Players", "Throws a living player straight up (with low gravity and No Fall Damage, a drop-in from the sky).", inputs=[X, P("player","player","Player",required=True), P("speed","number","Speed",param="speed")], outputs=[E()], params=[prm("speed","Speed","number",900,min=50,max=3000)])
node("event.time_up", "Round Time Up", "Events", "Runs when the mode's round clock reaches 0.", outputs=[E()], **ev)

# ---- milestone 5: Blueprint staples (graph version 3)
OF = lambda d="number": prm("of","Holds","type",d)
KEY = prm("key","Keys","choice","text",choices=[["text","Text"],["number","Numbers"],["player","Players"]])
# functions with their own graph, macros, locals
node("fn.entry", "Function Entry", "Functions", "Where a Function starts. Its outputs are the Function's inputs (add them in Details).", outputs=[E()], event=True)
node("fn.return", "Return Node", "Functions", "Ends the Function and hands back its outputs (add them in Details).", inputs=[X], outputs=[])
node("fn.call", "Call", "Functions", "Runs a Function (with its own graph), then carries on with its outputs.", inputs=[X], outputs=[E()], params=[prm("func","Function","fn","")])
node("fn.call_pure", "Call Pure", "Functions", "Works out a pure Function's outputs where they're used (no flow pins).", inputs=[], outputs=[], params=[prm("func","Function","fn","")], pure=True)
node("macro.entry", "Macro Entry", "Macros", "Where a Macro starts: its inputs (flow and values).", outputs=[], event=True)
node("macro.exit", "Macro Exit", "Macros", "Where a Macro ends: its outputs (flow and values).", inputs=[], outputs=[])
node("macro.use", "Macro", "Macros", "A Macro's graph, copied in here when the mode is made.", inputs=[], outputs=[], params=[prm("macro","Macro","macro","")])
# flow
node("flow.sequence", "Sequence", "Flow", "Runs each output in turn, top to bottom.", inputs=[X], outputs=[E("then0","Then 0"), E("then1","Then 1")], params=[prm("count","Outputs","number",2,min=1,max=8)])
node("flow.do_once", "Do Once", "Flow", "Lets the flow through the first time only, until Reset.", inputs=[X, E("reset","Reset")], outputs=[E("then","Completed")], params=[prm("closed","Start closed","bool",False)])
node("flow.do_n", "Do N", "Flow", "Lets the flow through N times, until Reset.", inputs=[X, E("reset","Reset"), P("n","number","N",param="n")], outputs=[E("then","Exit"), P("counter","number","Counter")], params=[prm("n","N","number",3,min=1,max=10000)])
node("flow.gate", "Gate", "Flow", "Flow coming into Enter goes through only while the gate is open.", inputs=[E("exec","Enter"), E("open","Open"), E("close","Close"), E("toggle","Toggle")], outputs=[E("then","Exit")], params=[prm("closed","Start closed","bool",True)])
node("flow.flipflop", "Flip Flop", "Flow", "Goes to A, then B, then A again... Is A says which it just did.", inputs=[X], outputs=[E("a","A"), E("b","B"), P("is_a","bool","Is A")])
node("flow.for_loop", "For Loop", "Flow", "Runs Loop Body once for each number from First to Last (at most 10000 times), then Completed.",
     inputs=[X, P("first","number","First",param="first"), P("last","number","Last",param="last")], outputs=[E("body","Loop Body"), P("index","number","Index"), E("then","Completed")],
     params=[prm("first","First","number",0,min=-100000,max=100000), prm("last","Last","number",9,min=-100000,max=100000)])
node("flow.for_loop_break", "For Loop with Break", "Flow", "A For Loop that stops early when flow comes into Break.",
     inputs=[X, E("break","Break"), P("first","number","First",param="first"), P("last","number","Last",param="last")], outputs=[E("body","Loop Body"), P("index","number","Index"), E("then","Completed")],
     params=[prm("first","First","number",0,min=-100000,max=100000), prm("last","Last","number",9,min=-100000,max=100000)])
node("flow.while", "While Loop", "Flow", "Runs Loop Body while the condition is true (checked each time; at most Limit times), then Completed.",
     inputs=[X, P("cond","bool","Condition",required=True)], outputs=[E("body","Loop Body"), E("then","Completed")], params=[prm("limit","Limit","number",1000,min=1,max=10000)])
node("flow.for_each_array", "For Each", "Arrays", "Runs Loop Body once for each item of an array (Break stops early), then Completed.",
     inputs=[X, E("break","Break"), P("array","array<$T>","Array",required=True)], outputs=[E("body","Loop Body"), P("item","$T","Item"), P("index","number","Index"), E("then","Completed")], params=[OF()])
node("flow.switch_num", "Switch on Number", "Flow", "Goes to the output whose case is the number (cases split by commas), or Default.",
     inputs=[X, P("in","number","Selection",required=True)], outputs=[E("default","Default")], params=[prm("cases","Cases","cases","1,2,3")])
node("flow.switch_text", "Switch on Text", "Flow", "Goes to the output whose case is the text (capitals don't matter; cases split by commas), or Default.",
     inputs=[X, P("in","text","Selection",required=True)], outputs=[E("default","Default")], params=[prm("cases","Cases","cases","yes,no")])
node("flow.switch_enum", "Switch on Enum", "Enums", "Goes to the output for the enum's value.", inputs=[X, P("in","$E","Selection",required=True)], outputs=[], params=[prm("enum","Enum","enum","")])
node("flow.if_valid", "Is Valid?", "Conditions", "Goes to Is Valid when the entity (or player) is there, else Is Not Valid.", inputs=[X, P("in","entity","Object",optional=True)], outputs=[E("valid","Is Valid"), E("invalid","Is Not Valid")])
node("timer.retrigger", "Retriggerable Delay", "Timers", "Waits, then carries on; flow coming in again while waiting starts the wait again.", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E(), P("handle","timer","Timer")], params=[prm("secs","Seconds","number",3,min=0.1,max=3600)])
node("timer.by_function", "Set Timer by Function", "Timers", "Runs a Function (with its own graph) after some seconds, again and again if Looping. Its inputs get nothing (0, empty).",
     inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E(), P("handle","timer","Timer")], params=[prm("func","Function","fn",""), prm("secs","Seconds","number",5,min=0.1,max=3600), prm("loop","Looping","bool",True)])
node("timer.clear", "Clear Timer", "Timers", "Stops a timer (a Delay, Repeat, Countdown or Set Timer's handle).", inputs=[X, P("handle","timer","Timer",required=True)], outputs=[E()])
node("timer.is_active", "Is Timer Active", "Timers", "Is the timer waiting to run?", inputs=[P("handle","timer","Timer",required=True)], outputs=[P("out","bool","Active")], pure=True)
node("debug.print", "Print String", "Messages", "Prints text (and a value wired in) to the server console and the Studio's output, and to chat if you like.", inputs=[X, P("value","any","Value",optional=True)], outputs=[E()],
     params=[prm("text","Text","text","Hello",max=100), prm("chat","Also in chat","bool",True)])
# conditions
node("logic.equal", "Equal", "Conditions", "Are the two values the same (or different)?", inputs=[P("a","$T","A",required=True), P("b","$T","B",required=True)], outputs=[P("out","bool","Result")],
     params=[OF(), prm("op","Is","choice","eq",choices=[["eq","equal"],["ne","not equal"]])], pure=True)
node("logic.select", "Select", "Conditions", "Picks A when the condition is true, else B.", inputs=[P("cond","bool","Pick A",param="cond"), P("a","$T","A",optional=True), P("b","$T","B",optional=True)], outputs=[P("out","$T","Result")],
     params=[OF(), prm("cond","Pick A","bool",True)], pure=True)
node("cond.is_valid", "Is Valid", "Conditions", "Is the entity (or player) there?", inputs=[P("in","entity","Object",optional=True)], outputs=[P("out","bool","Valid")], pure=True)
node("player.is_player", "Is Player", "Players", "Is the entity a player (or bot) in the game?", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","bool","Player")], pure=True)
node("creature.is", "Is Creature", "Creatures", "Is the entity one of the bot brain's creatures?", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","bool","Creature")], pure=True)
# math
node("math.fn", "Math Function", "Numbers", "Absolute value, rounding, square root and more.", inputs=[P("a","number","In",param="a")], outputs=[P("out","number","Result")],
     params=[prm("a","In","number",0,min=-100000,max=100000), prm("op","Do","choice","abs",choices=[["abs","Absolute (no minus)"],["round","Round"],["floor","Round down"],["ceil","Round up"],["sqrt","Square root"],["neg","Minus"],["sign","Sign (-1, 0, 1)"]])], pure=True)
node("math.clamp", "Clamp", "Numbers", "Keeps a number between Min and Max.", inputs=[P("a","number","Value",param="a"), P("min","number","Min",param="min"), P("max","number","Max",param="max")], outputs=[P("out","number","Result")],
     params=[prm("a","Value","number",0,min=-100000,max=100000), prm("min","Min","number",0,min=-100000,max=100000), prm("max","Max","number",100,min=-100000,max=100000)], pure=True)
node("math.lerp", "Lerp", "Numbers", "A number between A and B: Alpha 0 is A, 1 is B, 0.5 halfway.", inputs=[P("a","number","A",param="a"), P("b","number","B",param="b"), P("alpha","number","Alpha",param="alpha")], outputs=[P("out","number","Result")],
     params=[prm("a","A","number",0,min=-100000,max=100000), prm("b","B","number",100,min=-100000,max=100000), prm("alpha","Alpha","number",0.5,min=-100,max=100)], pure=True)
node("math.in_range", "In Range", "Numbers", "Is the number from Min to Max?", inputs=[P("a","number","Value",required=True), P("min","number","Min",param="min"), P("max","number","Max",param="max")], outputs=[P("out","bool","In range")],
     params=[prm("min","Min","number",0,min=-100000,max=100000), prm("max","Max","number",100,min=-100000,max=100000)], pure=True)
node("math.number", "Number", "Numbers", "A number.", outputs=[P("out","number","Number")], params=[prm("value","Number","number",0,min=-100000,max=100000)], pure=True)
# text
node("text.literal", "Text", "Text", "Some text.", outputs=[P("out","text","Text")], params=[prm("value","Text","text","",max=100)], pure=True)
node("text.append", "Append", "Text", "B straight after A (add a space in B if you want one).", inputs=[P("a","text","A",param="a"), P("b","text","B",param="b")], outputs=[P("out","text","Text")],
     params=[prm("a","A","text","",max=60), prm("b","B","text","",max=60)], pure=True)
node("text.format", "Format Text", "Text", "Text with [0] [1] [2] [3] replaced by the values wired in.", inputs=[P("a0","any","[0]",optional=True), P("a1","any","[1]",optional=True), P("a2","any","[2]",optional=True), P("a3","any","[3]",optional=True)], outputs=[P("out","text","Text")],
     params=[prm("text","Format","text","[0] has [1] points",max=100)], pure=True)
node("text.contains", "Contains Text", "Text", "Is B somewhere in A (capitals don't matter)?", inputs=[P("a","text","Text",required=True), P("b","text","Find",param="b")], outputs=[P("out","bool","Contains")], params=[prm("b","Find","text","",max=60)], pure=True)
node("text.length", "Text Length", "Text", "How many characters the text has.", inputs=[P("a","text","Text",required=True)], outputs=[P("out","number","Length")], pure=True)
node("text.to_number", "Text To Number", "Text", "The number written in the text (0 if none).", inputs=[P("a","text","Text",required=True)], outputs=[P("out","number","Number")], pure=True)
node("text.from", "To Text", "Text", "A value as text: a number, a player's name, a vector, an enum's name...", inputs=[P("a","$T","Value",required=True)], outputs=[P("out","text","Text")], params=[OF()], pure=True)
# vectors
# map markers (the mode map kit): ls_marker entities the map places, found by marker id
MK = prm("marker","Marker","marker","")
node("marker.find", "Find Marker", "Map markers", "The map's marker of this kind (1st, 2nd... in the map's order; a path's spots in their order), where it is, and whether the map has it.",
     inputs=[P("n","number","Which",param="n")], outputs=[P("out","entity","Marker"), P("pos","vector","Position"), P("found","bool","Found")],
     params=[MK, prm("n","Which (1st, 2nd...)","number",1,min=1,max=128)], pure=True)
node("marker.count", "Marker Count", "Map markers", "How many markers of this kind the map has.", outputs=[P("out","number","Count")], params=[MK], pure=True)
node("marker.get", "Marker Key", "Map markers", "A key the map set on a marker (the kit's default when the map left it out), as text and as a number.",
     inputs=[P("in","entity","Marker",required=True)], outputs=[P("text","text","Text"), P("num","number","Number")], params=[prm("key","Key","name","")], pure=True)
node("marker.for_each", "For Each Marker", "Map markers", "Runs Each once for every marker of this kind (a path's spots in order), then Then.",
     inputs=[X], outputs=[E("each","Each"), P("entity","entity","Marker"), P("index","number","Number (from 1)"), E("then","Then")], params=[MK])

node("vec.make", "Make Vector", "Vectors", "A position or direction from X, Y and Z.", inputs=[P("x","number","X",param="x"), P("y","number","Y",param="y"), P("z","number","Z",param="z")], outputs=[P("out","vector","Vector")],
     params=[prm("x","X","number",0,min=-100000,max=100000), prm("y","Y","number",0,min=-100000,max=100000), prm("z","Z","number",0,min=-100000,max=100000)], pure=True)
node("vec.break", "Break Vector", "Vectors", "A vector's X, Y and Z.", inputs=[P("in","vector","Vector",required=True)], outputs=[P("x","number","X"), P("y","number","Y"), P("z","number","Z")], pure=True)
node("vec.op", "Vector Math", "Vectors", "Adds or takes away two vectors.", inputs=[P("a","vector","A",required=True), P("b","vector","B",required=True)], outputs=[P("out","vector","Result")],
     params=[prm("op","Do","choice","add",choices=[["add","A + B"],["sub","A - B"]])], pure=True)
node("vec.scale", "Scale Vector", "Vectors", "Multiplies a vector by a number (makes it longer or shorter).", inputs=[P("a","vector","Vector",required=True), P("n","number","Times",param="n")], outputs=[P("out","vector","Result")], params=[prm("n","Times","number",2,min=-100000,max=100000)], pure=True)
node("vec.length", "Vector Length", "Vectors", "How long a vector is.", inputs=[P("a","vector","Vector",required=True)], outputs=[P("out","number","Length")], pure=True)
node("vec.distance", "Distance", "Vectors", "How far apart two positions are.", inputs=[P("a","vector","A",required=True), P("b","vector","B",required=True)], outputs=[P("out","number","Distance")], pure=True)
node("vec.direction", "Direction", "Vectors", "The direction (length 1) from one position to another.", inputs=[P("from","vector","From",required=True), P("to","vector","To",required=True)], outputs=[P("out","vector","Direction")], pure=True)
node("vec.normalize", "Normalize", "Vectors", "The same direction, length 1.", inputs=[P("a","vector","Vector",required=True)], outputs=[P("out","vector","Result")], pure=True)
node("vec.player", "Position Of Player", "Vectors", "Where a player is: their feet, their eyes, which way they're aiming, what they're aiming at, or how fast they're moving.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","vector","Vector")],
     params=[prm("what","What","choice","origin",choices=[["origin","Position"],["eyes","Eye position"],["aim","Aim direction"],["aim_point","Aiming at (where the aim hits)"],["velocity","Velocity"]])], pure=True)
node("vec.entity", "Position Of Entity", "Vectors", "Where an entity is (the middle of a brush entity).", inputs=[P("in","entity","Entity",required=True)], outputs=[P("out","vector","Position")], pure=True)
node("player.set_position", "Set Position", "Players", "Moves a living player to a position (pick one where they fit).", inputs=[X, P("player","player","Player",required=True), P("at","vector","Position",required=True)], outputs=[E()])
node("player.set_velocity", "Set Velocity", "Players", "Throws a living player: their speed and direction as a vector.", inputs=[X, P("player","player","Player",required=True), P("v","vector","Velocity",required=True)], outputs=[E()])
# structs and enums
node("struct.make", "Make Struct", "Structs", "A struct from its fields.", inputs=[], outputs=[P("out","$S","Struct")], params=[prm("struct","Struct","struct","")], pure=True)
node("struct.break", "Break Struct", "Structs", "A struct's fields.", inputs=[P("in","$S","Struct",required=True)], outputs=[], params=[prm("struct","Struct","struct","")], pure=True)
node("struct.set_field", "Set Field", "Structs", "Sets one field of a struct variable.", inputs=[X, P("value","$F","Value",optional=True)], outputs=[E()],
     params=[prm("var","Variable","var","",varType="struct"), prm("field","Field","field","")])
node("enum.value", "Enum Value", "Enums", "One of an enum's values.", outputs=[P("out","$E","Value")], params=[prm("enum","Enum","enum",""), prm("value","Value","enumvalue",0)], pure=True)
node("enum.to_number", "Enum To Number", "Enums", "An enum value's number (the first is 0).", inputs=[P("in","$E","Value",required=True)], outputs=[P("out","number","Number")], params=[prm("enum","Enum","enum","")], pure=True)
# arrays
node("array.make", "Make Array", "Arrays", "An array of the items wired in (a new one each time it's worked out).", inputs=[P("i","$T","Item",optional=True)], outputs=[P("out","array<$T>","Array")], params=[OF(), prm("count","Items","number",2,min=0,max=8)], pure=True)
node("array.players", "Players Array", "Arrays", "An array of every player who fits.", outputs=[P("out","array<player>","Players")], params=[prm("which","Who","choice","alive",choices=WHICH)], pure=True)
node("array.add", "Add", "Arrays", "Adds an item at the end (an array holds at most 10000).", inputs=[X, P("array","array<$T>","Array",required=True), P("item","$T","Item",optional=True)], outputs=[E(), P("index","number","Index")], params=[OF()])
node("array.insert", "Insert", "Arrays", "Puts an item in at an index (the rest move up).", inputs=[X, P("array","array<$T>","Array",required=True), P("item","$T","Item",optional=True), P("at","number","Index",param="at")], outputs=[E()],
     params=[OF(), prm("at","Index","number",0,min=0,max=10000)])
node("array.remove", "Remove Item", "Arrays", "Takes out every item equal to this one.", inputs=[X, P("array","array<$T>","Array",required=True), P("item","$T","Item",optional=True)], outputs=[E(), P("removed","bool","Removed")], params=[OF()])
node("array.remove_index", "Remove Index", "Arrays", "Takes out the item at an index.", inputs=[X, P("array","array<$T>","Array",required=True), P("at","number","Index",param="at")], outputs=[E()], params=[OF(), prm("at","Index","number",0,min=0,max=10000)])
node("array.set", "Set Array Item", "Arrays", "Changes the item at an index (nothing happens past the end).", inputs=[X, P("array","array<$T>","Array",required=True), P("at","number","Index",param="at"), P("item","$T","Item",optional=True)], outputs=[E()],
     params=[OF(), prm("at","Index","number",0,min=0,max=10000)])
node("array.clear", "Clear", "Arrays", "Empties an array.", inputs=[X, P("array","array<$T>","Array",required=True)], outputs=[E()], params=[OF()])
node("array.shuffle", "Shuffle", "Arrays", "Puts an array's items in a random order.", inputs=[X, P("array","array<$T>","Array",required=True)], outputs=[E()], params=[OF()])
node("array.sort", "Sort", "Arrays", "Sorts an array (numbers by size, text A to Z, the rest by their number).", inputs=[X, P("array","array<$T>","Array",required=True)], outputs=[E()],
     params=[OF(), prm("order","Order","choice","asc",choices=[["asc","Smallest first"],["desc","Biggest first"]])])
node("array.get", "Get", "Arrays", "The item at an index (nothing, 0 or empty past the end).", inputs=[P("array","array<$T>","Array",required=True), P("at","number","Index",param="at")], outputs=[P("out","$T","Item")], params=[OF(), prm("at","Index","number",0,min=0,max=10000)], pure=True)
node("array.length", "Length", "Arrays", "How many items an array has.", inputs=[P("array","array<$T>","Array",required=True)], outputs=[P("out","number","Length")], params=[OF()], pure=True)
node("array.contains", "Contains", "Arrays", "Is the item in the array?", inputs=[P("array","array<$T>","Array",required=True), P("item","$T","Item",optional=True)], outputs=[P("out","bool","Contains")], params=[OF()], pure=True)
node("array.find", "Find", "Arrays", "The index of the first item equal to this one (-1 if none).", inputs=[P("array","array<$T>","Array",required=True), P("item","$T","Item",optional=True)], outputs=[P("out","number","Index")], params=[OF()], pure=True)
node("array.random", "Random Item", "Arrays", "An item picked at random (a new pick each time it's worked out).", inputs=[P("array","array<$T>","Array",required=True)], outputs=[P("out","$T","Item")], params=[OF()], pure=True)
# maps
node("map.add", "Map Add", "Maps", "Puts a value in a map under a key (replacing what was there).", inputs=[X, P("map","map<$K,$T>","Map",required=True), P("key","$K","Key",optional=True), P("value","$T","Value",optional=True)], outputs=[E()], params=[KEY, OF()])
node("map.remove", "Map Remove", "Maps", "Takes a key (and its value) out of a map.", inputs=[X, P("map","map<$K,$T>","Map",required=True), P("key","$K","Key",optional=True)], outputs=[E()], params=[KEY, OF()])
node("map.clear", "Map Clear", "Maps", "Empties a map.", inputs=[X, P("map","map<$K,$T>","Map",required=True)], outputs=[E()], params=[KEY, OF()])
node("map.find", "Map Find", "Maps", "The value under a key, and whether it was there.", inputs=[P("map","map<$K,$T>","Map",required=True), P("key","$K","Key",optional=True)], outputs=[P("out","$T","Value"), P("found","bool","Found")], params=[KEY, OF()], pure=True)
node("map.contains", "Map Contains", "Maps", "Is the key in the map?", inputs=[P("map","map<$K,$T>","Map",required=True), P("key","$K","Key",optional=True)], outputs=[P("out","bool","Contains")], params=[KEY, OF()], pure=True)
node("map.length", "Map Length", "Maps", "How many keys a map has.", inputs=[P("map","map<$K,$T>","Map",required=True)], outputs=[P("out","number","Length")], params=[KEY, OF()], pure=True)
node("map.keys", "Map Keys", "Maps", "An array of a map's keys (a player who left is left out).", inputs=[P("map","map<$K,$T>","Map",required=True)], outputs=[P("out","array<$K>","Keys")], params=[KEY, OF()], pure=True)
node("map.values", "Map Values", "Maps", "An array of a map's values.", inputs=[P("map","map<$K,$T>","Map",required=True)], outputs=[P("out","array<$T>","Values")], params=[KEY, OF()], pure=True)

node("flow.reroute", "Reroute", "Flow", "A bend in a wire, to keep the graph tidy. Passes on whatever comes in.", inputs=[P("in","any","")], outputs=[P("out","any","")], reroute=True)

data = {
  "version": 4,
  "pinTypes": {
    "exec": {"label": "Flow", "color": "#f2f2f2"},
    "player": {"label": "Player", "color": "#5ab0ff"},
    "entity": {"label": "Entity", "color": "#4dd0c8"},
    "team": {"label": "Team", "color": "#ffb347"},
    "number": {"label": "Number", "color": "#9be36a"},
    "bool": {"label": "Yes/No", "color": "#ff6b8a"},
    "text": {"label": "Text", "color": "#e0a3ff"},
    "vector": {"label": "Vector", "color": "#ffd23f"},
    "timer": {"label": "Timer", "color": "#7fd6ff"},
    "enum": {"label": "Enum", "color": "#1f8a70"},
    "struct": {"label": "Struct", "color": "#2f6fd6"},
    "any": {"label": "Anything", "color": "#9a9a9a"},
    "var": {"label": "Variable", "color": "#cccccc"}
  },
  "categories": ["Events", "Conditions", "Numbers", "Text", "Variables", "Flow", "Functions", "Macros", "Arrays", "Maps", "Structs", "Enums", "Vectors", "Timers", "Round clock", "Players", "Damage", "Squads", "Buying", "Saved", "Messages", "Game", "Scoring", "Bots", "Components", "Map markers", "Upgrades", "Downed", "Shop", "Battle Royale", "Creatures"],
  "nodes": nodes,
}
json.dump(data, open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
print(len(nodes), "nodes")
