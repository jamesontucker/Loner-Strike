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
COMPONENTS = [["ls_capture_point","Capture point"],["ls_safe_zone","Safe zone"],["ls_zone_shrinker","Shrinking zone"],["ls_damage_zone","Damage zone"],["ls_loot_container","Loot container"],["ls_weapon_spawner","Weapon spawner"],["ls_supply_drop","Supply drop"],["ls_wave_spawner","Wave spawner"],["ls_extraction","Extraction point"],["ls_checkpoint","Checkpoint"],["ls_trap","Trap"],["ls_respawn_point","Respawn point"],["ls_shop","Shop (a counter, items=Name|price|give|what;...)"],["ls_perk_machine","Perk machine"],["ls_upgrade_bench","Upgrade bench"],["ls_mystery_box","Mystery box"],["ls_npc","NPC (model=barney name=... behavior=stand)"],["ls_interactable","Interactable (prompt=... once=1)"]]

nodes = []
def node(type, title, cat, help, inputs=(), outputs=(), params=(), **k):
    d = {"type": type, "title": title, "category": cat, "help": help, "inputs": list(inputs), "outputs": list(outputs), "params": list(params)}
    d.update(k); nodes.append(d)

# ---- events
ev = dict(event=True)
node("event.round_started", "Round Started", "Events", "Runs when a round begins: freeze time is over and everyone can move.", outputs=[E()], **ev)
node("event.round_ended", "Round Ended", "Events", "Runs when a round ends. Winner is no team for a draw.", outputs=[E(), P("winner","team","Winner"), P("round","number","Round number")], **ev)
node("event.player_killed", "Player Killed", "Events", "Runs when a player dies. Attacker is nobody when the world killed them (a fall, the bomb).", outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("headshot","bool","Headshot"), P("weapon","text","Weapon"), P("distance","number","Distance"), P("team_kill","bool","Team kill"), P("suicide","bool","Suicide"), P("world","bool","Killed by the world")], **ev)
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
node("comp.on_wave_spawn", "Wave Spawned One", "Components", "A wave's spawner is about to bring in one enemy. Spawner is the wave spawner, not the enemy (it isn't made yet): to act on its enemies, use For Each Creature (a moment later) or Creature Killed.", outputs=[E(), P("spawned","entity","Spawner"), P("wave","number","Wave")], **cev)
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
node("player.give_weapon", "Give Weapon", "Players", "Gives a living player a weapon and spare bullets. Does nothing for a dead player, or one who already has that weapon (the debugger marks that). A weapon's name can be wired in (from a data table, say): ak47, deagle, knife...",
     inputs=[X, P("player","player","Player",required=True), P("weapon","text","Weapon",param="weapon"), P("ammo","number","Spare bullets",param="ammo")], outputs=[E()], params=[prm("weapon","Weapon","choice","ak47",choices=WEAPONS), prm("ammo","Spare bullets","number",90,min=0,max=250)])
node("player.strip", "Strip Weapons", "Players", "Takes a player's weapons away (keeping the knife if you like).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()], params=[prm("knife","Keep the knife","bool",True)])
node("player.set_health", "Set Health", "Players", "Sets a living player's health.", inputs=[X, P("player","player","Player",required=True), P("amount","number","Health",param="amount")], outputs=[E()], params=[prm("amount","Health","number",100,min=1,max=10000)])
node("player.set_armor", "Set Armor", "Players", "Sets a living player's armor: kevlar and a helmet, or kevlar only.", inputs=[X, P("player","player","Player",required=True), P("amount","number","Armor",param="amount")], outputs=[E()], params=[prm("amount","Armor","number",100,min=0,max=1000), prm("type","Type","choice","vesthelm",choices=[["vesthelm","Kevlar and helmet"],["vest","Kevlar only"]])])
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
node("event.player_hurt", "Player Hurt", "Events", "Runs when a player takes damage, with the health it took (blocked damage doesn't count). Hit Position: where a bullet or the knife struck (else the middle of the victim); Direction: which way the hit was going (from the attacker). Use Get Transform on Victim/Attacker for where they were.", outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("damage","number","Damage"), P("headshot","bool","Headshot"), P("left","number","Health left"), P("hitgroup","text","Hit where"), P("weapon","text","Weapon"), P("hit_pos","vector","Hit Position"), P("hit_dir","vector","Direction")], **ev)
node("event.team_changed", "Team Changed", "Events", "Runs once when a player joins a team or changes team (Spectator is No team).", outputs=[E(), P("player","player","Player"), P("team","team","Team")], **ev)
node("event.map_started", "Map Started", "Events", "Runs once when the map has loaded (before the first round): load map packs, set things up.", outputs=[E()], **ev)
node("event.new_round", "New Round", "Events", "Runs when a new round is set up (freeze time starts), before Round Started.", outputs=[E()], **ev)
node("event.chat_command", "Chat Command", "Events", "Runs when a player says /command or !command in chat (shop for /shop and !shop); Arguments is the rest of the line (\"/give 5\": 5).", outputs=[E(), P("player","player","Player"), P("args","text","Arguments")], params=[prm("command","Command (without /)","name","shop")], **ev)
node("func.define", "Function", "Functions", "A piece of graph you can run from anywhere with Call Function. It gets a Player and a Number from the call.", outputs=[E(), P("player","player","Player"), P("number","number","Number")], params=[prm("name","Name","name","my_function")], event=True)
node("func.call", "Call Function", "Functions", "Runs a Function, giving it a Player and a Number, then carries on.", inputs=[X, P("player","player","Player",optional=True), P("number","number","Number",param="number")], outputs=[E()],
     params=[prm("func","Function","func",""), prm("number","Number","number",0,min=-100000,max=100000)])
# (X2: Who, Team, the kind or name and Only these can be wired; the settings are what's used when
# nothing is. For a list of your own, Arrays > For Each.)
WHO_IN = P("who","enum:ls_who","Who",optional=True)
TEAM_IN = P("team","team","Team (overrides Who)",optional=True)
node("flow.for_each_player", "For Each Player", "Flow", "Runs Each once for every player who fits, then Then. Who and Team can be wired (a variable, a Select); Only these goes through a list of players instead (Players In Radius...). Break stops early. For any list of your own, use Arrays > For Each.",
     inputs=[X, E("break","Break"), WHO_IN, TEAM_IN, P("only","array<player>","Only these",optional=True)], outputs=[E("each","Each"), P("player","player","Player"), P("index","number","Index"), E("then","Then")], params=[prm("which","Who","choice","alive",choices=WHICH)])
node("flow.for_each_entity", "For Each Entity", "Flow", "Runs Each once for every entity of a kind or with a name (at most 512), then Then. The kind or name can be wired; Only these goes through a list of entities instead. Break stops early. For any list of your own, use Arrays > For Each.",
     inputs=[X, E("break","Break"), P("name","text","Kind or name",optional=True), P("only","array<entity>","Only these",optional=True)], outputs=[E("each","Each"), P("entity","entity","Entity"), P("index","number","Index"), E("then","Then")],
     params=[prm("by","Which","choice","class",choices=BY), prm("name","Kind or name","name","ls_respawn_point")])
node("flow.pick_player", "Pick Random Player", "Flow", "Picks one player who fits at random (Found), or runs None. Who and Team can be wired; Only these picks from a list of players instead.",
     inputs=[X, WHO_IN, TEAM_IN, P("only","array<player>","Only these",optional=True)], outputs=[E("then","Found"), E("none","None"), P("player","player","Player")], params=[prm("which","Who","choice","alive",choices=WHICH)])
node("flow.pick_entity", "Pick Random Entity", "Flow", "Picks one entity of a kind or with a name at random (Found), or runs None. The kind or name can be wired; Only these picks from a list of entities instead.",
     inputs=[X, P("name","text","Kind or name",optional=True), P("only","array<entity>","Only these",optional=True)], outputs=[E("then","Found"), E("none","None"), P("entity","entity","Entity")],
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
     params=[prm("cvar","Setting","choice","mp_freeforall",choices=[["mp_freeforall","Free for all (1 on)"],["mp_friendlyfire","Friendly fire (1 on)"],["sv_gravity","Gravity (800 normal)"],["mp_buytime","Buy time (minutes)"],["mp_startmoney","Start money"],["sv_airaccelerate","Air control (10 normal)"],["mp_roundtime","Round time (minutes, next round)"],["mp_freezetime","Freeze time (seconds, next round)"],["mp_round_infinite","Rounds only end when the mode says (1 on)"],["mp_autoteambalance","Team balance (1 on)"],["mp_limitteams","Most players more than the other team (0 any)"],["mp_give_player_c4","Bomb for the Terrorists (1 on)"],["ls_down_finish","Downed players can be finished by enemies (1 on; components)"],["ls_down_hp","A downed player's health when they can be finished (components)"]]),
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
     params=[prm("kind","Kind","name","resupply"), prm("life","Lasts (seconds)","number",25,min=1,max=600), prm("colour","Color","choice","gold",choices=[["white","White"],["green","Green"],["blue","Blue"],["purple","Purple"],["gold","Gold"],["red","Red"]])], **ca)
node("up.on_perk", "Perk Bought", "Upgrades", "A player bought a perk (at a machine, a shop or Buy Perk).", outputs=[E(), P("player","player","Player"), P("perk","text","Perk"), P("price","number","Price")], **cev)
node("up.on_upgraded", "Gun Upgraded", "Upgrades", "A player's gun went up a level.", outputs=[E(), P("player","player","Player"), P("level","number","Level"), P("price","number","Price")], **cev)
node("up.on_box", "Mystery Box Gave", "Upgrades", "The mystery box gave a player a gun.", outputs=[E(), P("player","player","Player"), P("weapon","text","Gun")], **cev)
node("up.on_pickup", "Pickup Taken", "Upgrades", "A player took a pickup.", outputs=[E(), P("pickup","entity","Pickup"), P("player","player","Player"), P("kind","text","Kind")], **cev)
node("up.on_creature_hurt", "Creature Hurt", "Creatures", "A creature took damage (Headshot when the hit was a head). Hit Position: where a bullet or the knife struck (else its middle); Direction: which way the hit was going. Use Get Entity Transform on Creature, or Get Transform on Attacker, for where they were.", outputs=[E(), P("creature","entity","Creature"), P("attacker","player","Attacker"), P("damage","number","Damage"), P("headshot","bool","Headshot"), P("hit_pos","vector","Hit Position"), P("hit_dir","vector","Direction")], **cev)
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
# NPCs and interactables (gm_components ls_npc / ls_interactable: T1fix)
NPC_OPT = P("npc","entity","NPC",optional=True)
NPC_IN = P("npc","entity","NPC",required=True)
node("npc.on_talk", "Talked To NPC", "NPCs", "Runs when a player presses USE on an NPC (after its greeting). Leave the name empty for any NPC.", outputs=[E(), P("player","player","Player"), P("npc","entity","NPC")], params=[prm("name","NPC name","name","")], keywords="talk speak conversation dialogue npc character quest", **cev)
node("npc.on_use", "Used Interactable", "NPCs", "Runs when a player uses an interactable (a lever, a note, a terminal); its map targets fire too. Leave the name empty for any.", outputs=[E(), P("player","player","Player"), P("entity","entity","Interactable")], params=[prm("name","Interactable name","name","")], keywords="use press lever note terminal pickup interact", **cev)
node("npc.choose", "Say and Choose", "NPCs", "The NPC (or a narrator, with none wired) says a line and the player picks a reply; each reply runs its output (later, when they pick), and Picked says which (1-6; 0 for none). Timed out: no pick in time. Walked away: they went off, died, left or closed it. Every text can be wired (a variable, a table cell), so one node can run a whole conversation from a data table; Replies sets how many are offered (empty ones are left out). One conversation per player: a new one ends the old (as Walked away).",
     inputs=[X, PL, NPC_OPT, P("speaker","text","Speaker",param="speaker"), P("text","text","Line",param="text"), P("o1","text","Reply 1",param="o1"), P("o2","text","Reply 2",param="o2"), P("o3","text","Reply 3",param="o3"), P("o4","text","Reply 4",param="o4"), P("o5","text","Reply 5",param="o5"), P("o6","text","Reply 6",param="o6"), P("count","number","Replies",param="count"), P("secs","number","Time to answer",param="secs")],
     outputs=[E("r1","Reply 1"), E("r2","Reply 2"), E("r3","Reply 3"), E("r4","Reply 4"), E("r5","Reply 5"), E("r6","Reply 6"), E("timeout","Timed out"), E("away","Walked away"), P("picked","number","Picked")],
     params=[prm("speaker","Speaker (empty: the NPC)","text","",max=31), prm("text","Line","text","Can you help me?",max=120), prm("o1","Reply 1","text","Yes",max=40), prm("o2","Reply 2","text","No",max=40), prm("o3","Reply 3","text","",max=40), prm("o4","Reply 4","text","",max=40), prm("o5","Reply 5","text","",max=40), prm("o6","Reply 6","text","",max=40), prm("count","Replies","number",6,min=1,max=6), prm("secs","Time to answer (0: no limit)","number",30,min=0,max=300)],
     keywords="dialogue talk conversation reply answer menu quest table", **ca)
node("npc.say", "Say", "NPCs", "The NPC (or a narrator, with none wired) says a line to a player, or to everyone with no Player.", inputs=[X, P("player","player","Player",optional=True), NPC_OPT, P("speaker","text","Speaker",param="speaker"), P("text","text","Line",param="text")], outputs=[E()], params=[prm("speaker","Speaker (empty: the NPC)","text","",max=31), prm("text","Line","text","Hello there.",max=120)], keywords="talk speak line dialogue narrator", **ca)
node("npc.behave", "Set NPC Behavior", "NPCs", "What an NPC does by itself: stands, wanders near its spot, patrols its targets, or follows a player (wired into Follow) for a while.", inputs=[X, NPC_IN, P("follow","player","Follow",optional=True)], outputs=[E()],
     params=[prm("do","Behavior","choice","stand",choices=[["stand","Stand"],["wander","Wander"],["patrol","Patrol"],["follow","Follow"]])], keywords="ai wander patrol follow stand idle", **ca)
node("npc.look_at", "NPC Look At", "NPCs", "Turns an NPC to face a player or an entity.", inputs=[X, NPC_IN, P("target","entity","Target",required=True)], outputs=[E()], keywords="face turn look", **ca)
node("npc.spawn", "Spawn NPC", "NPCs", "Makes an NPC at an entity (wired into At), a point, or the entity with this name. Its model is loaded with the map (models/...mdl).", inputs=[X, P("at_ent","entity","At",optional=True), P("pos","vector","Or point",optional=True)], outputs=[E(), P("out","entity","NPC")],
     params=[prm("at","Or the entity named","name",""), prm("model","Model","path","models/barney.mdl"), prm("name","Name","text","Guide",max=31), prm("yaw","Facing (degrees)","number",0,min=-360,max=360), prm("behavior","Behavior","choice","stand",choices=[["stand","Stand"],["wander","Wander"]]), prm("unkillable","Unkillable","bool",True)],
     keywords="npc character create make quest giver", **ca)
node("npc.spawn_use", "Spawn Interactable", "NPCs", "Makes an interactable at an entity (wired into At), a point, or the entity with this name: a model players use (E), with a prompt when they look at it. Once: usable once a round.", inputs=[X, P("at_ent","entity","At",optional=True), P("pos","vector","Or point",optional=True)], outputs=[E(), P("out","entity","Interactable")],
     params=[prm("at","Or the entity named","name",""), prm("model","Model","path","models/w_weaponbox.mdl"), prm("prompt","Prompt","text","Press E to use",max=60), prm("once","Once a round","bool",True), prm("hide","Gone once used","bool",False), prm("bots","Bots may use","bool",False)],
     keywords="interactable lever note terminal pickup usable create", **ca)
# Projectiles (gm_components ls_proj_*: T1fix)
PROJ_IN = P("proj","entity","Projectile",required=True)
node("proj.spawn", "Spawn Projectile", "Projectiles", "Fires a model or sprite (loaded with the map). Where from: At position, else At entity (a player's eyes) moved by Local offset along its own forward, right and up (16 forward when nothing is wired there), else the owner's eyes. Which way: Direction, else Rotation, else the way At entity (or the owner) faces when Face the way At entity faces is on, turned by Rotation offset; Velocity, when wired, sets the motion outright. Gravity 0 flies straight; 1 falls like a grenade. Bounces: off walls before it stops. Damage: to what it hits, or everything within Radius (less further out with Falloff). Kills count for the owner. On Hit runs later for this projectile only, with what it hit (each bounce too); On Expired when its time runs out. 64 fly at most; a new round clears them.",
     inputs=[X, P("owner","player","Owner",optional=True), P("at_ent","entity","At entity",optional=True), P("pos","vector","At position",optional=True), P("offset","vector","Local offset (forward, right, up)",optional=True), P("angles","vector","Rotation (pitch, yaw, roll)",optional=True), P("angles_offset","vector","Rotation offset",optional=True), P("vel","vector","Velocity",optional=True), P("dir","vector","Direction",optional=True), P("speed","number","Speed",param="speed"), P("damage","number","Damage",param="damage"), P("scale","number","Scale",param="scale")], outputs=[E(), P("out","entity","Projectile"), E("on_hit","On Hit"), E("on_expire","On Expired"), P("hit_pos","vector","Hit Position"), P("hit_normal","vector","Hit Normal"), P("hit_player","player","Hit Player"), P("hit_ent","entity","Hit Entity"), P("hit_world","bool","Hit World"), P("hit_zone","number","Hit Zone"), P("hit_owner","player","Owner")],
     params=[prm("model","Model or sprite","path","models/w_hegrenade.mdl"), prm("copy_rot","Face the way At entity faces","bool",True), prm("speed","Speed","number",1000,min=1,max=5000), prm("scale","Scale","number",1,min=0.05,max=4), prm("gravity","Gravity","number",0,min=0,max=4), prm("life","Lifetime (seconds)","number",5,min=0.1,max=60), prm("bounce","Bounces","number",0,min=0,max=10), prm("damage","Damage","number",0,min=0,max=5000), prm("radius","Radius","number",0,min=0,max=2000), prm("falloff","Falloff","bool",True), prm("explode","Explosion","bool",False), prm("trail","Trail","bool",False), prm("spin","Spin","bool",False), prm("dangerous","Dangerous to bots","bool",False)],
     keywords="projectile rocket arrow fireball bullet throw shoot missile grenade", **ca)
node("proj.on_hit", "Projectile Hit", "Projectiles", "Runs when a projectile hits something: where, the surface's normal there, what it hit (World: a wall or the floor), and a player's hitgroup (1 head, 2 chest, 3 stomach, 4-5 arms, 6-7 legs; 0 unknown).", outputs=[E(), P("proj","entity","Projectile"), P("owner","player","Owner"), P("pos","vector","Hit position"), P("normal","vector","Normal"), P("hit","entity","Hit"), P("world","bool","World"), P("hitgroup","number","Hitgroup")], keywords="projectile impact collide hit headshot", **cev)
node("proj.remove", "Remove Projectile", "Projectiles", "Takes a projectile away at once (no hit, no damage).", inputs=[X, PROJ_IN], outputs=[E()], keywords="projectile delete destroy", **ca)
node("proj.velocity", "Projectile Velocity", "Projectiles", "How fast and which way a projectile flies (units a second).", inputs=[PROJ_IN], outputs=[P("out","vector","Velocity")], pure=True, needs="components")
node("proj.set_velocity", "Set Projectile Velocity", "Projectiles", "Sends a projectile off at a new velocity (its facing follows).", inputs=[X, PROJ_IN, P("vel","vector","Velocity",required=True)], outputs=[E()], keywords="projectile speed direction redirect homing", **ca)
# Battle Royale pieces (any mode)
node("br.heal_give", "Give Healing Item", "Battle Royale", "Gives a player bandages (+15 up to 75), medkits (full health) or drinks (+40 over 10 s), up to the carry limit.", inputs=[X, PL, P("n","number","How many",param="n")], outputs=[E()], params=[prm("kind","Item","choice","bandage",choices=[["bandage","Bandage"],["medkit","Medkit"],["drink","Drink"]]), prm("n","How many","number",1,min=1,max=10)], **ca)
node("br.heal_count", "Healing Items", "Battle Royale", "How many healing items a player carries.", inputs=[PL], outputs=[P("out","number","How many")], pure=True, needs="components")
node("br.armor", "Set Armor Levels", "Battle Royale", "Sets a player's vest and helmet levels (1-3; 0 none; keep: as they are).", inputs=[X, PL], outputs=[E()], params=[prm("vest","Vest","choice","2",choices=[["-1","Keep"],["0","None"],["1","1"],["2","2"],["3","3"]]), prm("helmet","Helmet","choice","1",choices=[["-1","Keep"],["0","None"],["1","1"],["2","2"],["3","3"]])], **ca)
node("br.crate", "Make Crate", "Battle Royale", "Makes a crate at an entity with these items (loot items: ak47@30@60 bandage:2 vest2).", inputs=[X, P("at","entity","At",required=True)], outputs=[E(), P("out","entity","Crate")], params=[prm("items","Items","keys","bandage:2 ammo",max=120)], **ca)
node("br.glow", "Rarity Glow", "Battle Royale", "Makes an entity glow in a rarity's color (0 none, 1 white, 2 green, 3 blue, 4 purple, 5 gold).", inputs=[X, P("ent","entity","Entity",required=True), P("tier","number","Rarity",param="tier")], outputs=[E()], params=[prm("tier","Rarity","number",3,min=0,max=5)], **ca)
node("br.hud", "Match HUD", "Battle Royale", "Turns the match HUD on or off (players left, kills, the zone's timer, armor, healing items, the kill feed and the squad).", inputs=[X], outputs=[E()], params=[prm("on","On","bool",True)], **ca)
node("br.hud_text", "Match HUD Words", "Battle Royale", "The mode's words in the match HUD in place of the zone's timer (empty: the zone again).", inputs=[X, P("text","text","Text",param="text")], outputs=[E()], params=[prm("text","Text","text","Loot up!",max=60)], **ca)
node("br.feed", "Match Feed Line", "Battle Royale", "Adds a text line to Battle Royale's match HUD feed (eliminations, zone news). Not the top-right kill notice: that's Kill Feed Line (Messages).", inputs=[X, P("text","text","Text",param="text")], outputs=[E()], params=[prm("text","Text","text","",max=60)], **ca)
node("br.kills", "Match Kills", "Battle Royale", "A player's kills this round (counted while the match HUD is on).", inputs=[PL], outputs=[P("out","number","Kills")], pure=True, needs="components")
node("br.drop_in", "Drop In", "Battle Royale", "Drops a player in from high over a spot (an entity wired into At, else a spread-out landing spot: the map's br_drop markers); they fall slowly and take no damage until they land.", inputs=[X, PL, P("at","entity","At",optional=True)], outputs=[E()], **ca)
node("br.on_landed", "Landed", "Battle Royale", "A player who dropped in is down on the ground.", outputs=[E(), P("player","player","Player")], **cev)
node("br.lobby", "Squad Lobby", "Battle Royale", "A lobby of some seconds where people team up (say /squad); then the squads are filled (Lobby Ended). 0: fill them now.", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",30,min=0,max=300)], **ca)
node("br.squad_size", "Squad Size", "Battle Royale", "Sets the squad size: 1 solo, 2 duos, 4 squads...", inputs=[X, P("size","number","Size",param="size")], outputs=[E()], params=[prm("size","Size","number",4,min=1,max=8)], **ca)
node("br.on_lobby_end", "Lobby Ended", "Battle Royale", "The squad lobby is over and the squads are filled.", outputs=[E(), P("squads","number","Squads")], **cev)
node("br.squads_alive", "Sides Alive", "Battle Royale", "How many sides have someone alive (squads, and players in none).", outputs=[P("out","number","Sides")], pure=True, needs="components")

# the round table (zombie-survival rounds on the wave spawners) and turrets: W1's components
node("wave.table", "Round Table On", "Waves", "Each Start Wave from now on plans a zombie-survival round on the map's wave spawners: how many (by round and team size), which creature types (unlocked by round, with stand-ins), their health, bosses, packs, heavies, tiers, how many alive at once. A waves.ini section replaces the built-in mix. Creatures hunt players: on, they go for the defenders from anywhere; off, each type keeps its own aggression (a hound only fights what comes near). Flee when badly hurt: Never, they keep coming; Creature's own, each type runs at its own flee point. A waves.ini section (hunt =, flee =) or one wave's line (hunt=0 flee=own) can still say otherwise for its waves.",
     inputs=[X], outputs=[E()], params=[prm("section","waves.ini section (empty: built in)","name",""), prm("team","Defenders","choice","2",choices=[["2","Counter-Terrorists"],["1","Terrorists"]]), prm("creatures","Creatures (no: dead players come back as the enemies)","bool",True), prm("hunt","Creatures hunt players (off: each type's own aggression)","bool",True), prm("flee","Flee when badly hurt","choice","never",choices=[["never","Never"],["own","Creature's own"]])], **ca)
node("wave.table_off", "Round Table Off", "Waves", "Waves go back to the spawners' own settings.", inputs=[X], outputs=[E()], **ca)
node("wave.scale", "Round Difficulty", "Waves", "Times the round table's counts and health (the mode's difficulty).", inputs=[X, P("count","number","Count times",param="count"), P("hp","number","Health times",param="hp")], outputs=[E()], params=[prm("count","Count times","number",1,min=0.1,max=10), prm("hp","Health times","number",1,min=0.1,max=10)], **ca)
node("wave.cap", "Alive Cap", "Waves", "How many creatures at once (0: 16 + 2 a defender, 20-48; never over the game's own cap).", inputs=[X, P("n","number","At once",param="n")], outputs=[E()], params=[prm("n","At once","number",0,min=0,max=64)], **ca)
node("wave.tiers", "Creature Tiers", "Waves", "Tiers on (yellow from round 15, red-orange from 20) or off.", inputs=[X], outputs=[E()], params=[prm("on","On","bool",True)], **ca)
node("wave.info", "This Round", "Waves", "This round's plan: how many, how many made so far, the health times, whether it's a boss or a pack round (and the type).", outputs=[P("planned","number","Planned"), P("spawned","number","Made so far"), P("hp","number","Health times"), P("boss","bool","Boss round"), P("pack","bool","Pack round"), P("type","text","Boss or pack type")], pure=True, needs="components")
node("wave.type_pay", "Kill Pay", "Waves", "What the round table pays for killing this creature type.", inputs=[P("type","text","Type",required=True)], outputs=[P("out","number","Pay")], pure=True, needs="components")
node("wave.creatures_ok", "Creatures Working", "Waves", "False once creatures can't spawn on this server (the table fell back to dead players).", outputs=[P("out","bool","Working")], pure=True, needs="components")
node("turret.setup", "Turrets", "Upgrades", "Turrets: friendly creatures that shoot the horde. A kit (Give Turret Kit, or a shop line with give=turret) is placed with USE where the player looks; bots are offered turret spots at this price. Price 0: off.", inputs=[X], outputs=[E()],
     params=[prm("price","Price","number",2500,min=0,max=100000), prm("life","Lasts (seconds, 0: until destroyed)","number",150,min=0,max=3600), prm("per","At once per player","number",1,min=1,max=8), prm("type","Creature type","name","turret"), prm("bots","Bots buy them","bool",True)], **ca)
node("turret.give", "Give Turret Kit", "Upgrades", "Gives a player turret kits to place.", inputs=[X, PL, P("n","number","How many",param="n")], outputs=[E()], params=[prm("n","How many","number",1,min=1,max=8)], **ca)
node("turret.place", "Place Turret", "Upgrades", "Places a turret where the player looks: from a kit, or paid at the turrets' price.", inputs=[X, PL], outputs=[E(), P("out","entity","Turret")], params=[prm("pay","Pay for it (no: use a kit)","bool",False)], **ca)
node("turret.kits", "Turret Kits", "Upgrades", "How many turret kits a player has.", inputs=[PL], outputs=[P("out","number","Kits")], pure=True, needs="components")
node("turret.count", "Turrets Out", "Upgrades", "How many turrets a player has out.", inputs=[PL], outputs=[P("out","number","Turrets")], pure=True, needs="components")
node("turret.on_placed", "Turret Placed", "Upgrades", "A turret was placed.", outputs=[E(), P("turret","entity","Turret"), P("owner","player","Owner")], **cev)

# ---- the game and the server: read-only facts (like Unreal's Get Game State), settings, map flow ----
G = dict(pure=True)
node("game.map_name", "Current Map Name", "Game", "The map being played (de_dust2).", outputs=[P("out","text","Map")], keywords="level", **G)
node("game.next_map", "Next Map", "Game", "The map the server goes to next (from the map cycle or vote); empty if it doesn't know.", outputs=[P("out","text","Map")], **G)
node("game.server_name", "Server Name", "Game", "The server's name (hostname).", outputs=[P("out","text","Name")], **G)
node("game.max_players", "Max Players", "Game", "How many players the server takes.", outputs=[P("out","number","Max")], **G)
node("game.player_count", "Player Count", "Game", "How many players there are: everyone, people, bots, a team, or spectators.", outputs=[P("out","number","Players")], params=[prm("which","Who","choice","all",choices=[["all","Everyone"],["humans","People"],["bots","Bots"],["t","Terrorists"],["ct","Counter-Terrorists"],["spec","Spectators"],["alive","Alive"]])], **G)
node("game.time", "Game Time", "Game", "Seconds since the map started (for timing things: keep it in a variable, compare later).", outputs=[P("out","number","Seconds")], keywords="clock timer seconds", **G)
node("game.clock", "Real Date and Time", "Game", "The server computer's date and time, as text (2026-11-13, 21:30, Friday) and as seconds since 1970.", outputs=[P("text","text","Text"), P("unix","number","Seconds since 1970")], params=[prm("format","As","choice","datetime",choices=[["datetime","Date and time"],["date","Date"],["time","Time"],["weekday","Day of the week"],["hour","Hour (0-23)"]])], keywords="date now today", **G)
node("game.mode_id", "Current Mode", "Game", "This mode's id (as the Game Mode Menu and modes.ini know it).", outputs=[P("out","text","Mode id")], **G)
node("game.round_number", "Round Number", "Game", "Which round this is on this map (1 for the first).", outputs=[P("out","number","Round")], **G)
node("game.team_score", "Team Score", "Game", "A team's rounds won on this map (as the scoreboard shows).", outputs=[P("out","number","Score")], params=[prm("team","Team","choice","ct",choices=TEAM2)], **G)
node("game.freeze_time", "Is Freeze Time", "Game", "Is it the freeze time at a round's start (nobody can move)?", outputs=[P("out","bool","Freeze time")], **G)
node("game.get_setting", "Get Setting", "Game", "Reads a server setting (a cvar), as a number and as text: the counterpart of Set Setting. Passwords and the like can't be read.", outputs=[P("num","number","Number"), P("text","text","Text")], params=[prm("cvar","Setting (cvar)","name","mp_roundtime")], keywords="cvar get_cvar", **G)
node("game.change_map", "Change Map", "Game", "Changes the map (if the server has it). The mode changes too only if the next map's preset says so. Asks first when an agent does it.", inputs=[X], outputs=[E()], params=[prm("map","Map","name","de_dust2")], keywords="level changelevel", advanced=True)
node("game.restart_round", "Restart Round", "Game", "Restarts the round now (scores stay).", inputs=[X], outputs=[E()], keywords="sv_restartround reset")
node("game.run_command", "Run Server Command", "Advanced", "Runs a server console command, like sv_gravity 400 or mp_restartgame 1. Commands that touch passwords, files, plugins or quit the server are refused, and only one command at a time (no ; ). Community modes that use it show \"runs server commands\".", inputs=[X, P("cmd","text","Command",param="cmd")], outputs=[E(), P("ok","bool","Ran")], params=[prm("cmd","Command","text","sv_gravity 800",max=100)], keywords="console server_cmd rcon", advanced=True)
node("game.amx_command", "Run AMX Command", "Advanced", "Runs an AMX Mod X admin command (amx_slay, amx_slap...). amx_rcon, amx_cvar on passwords, plugin loading, bans and admin edits are refused.", inputs=[X, P("cmd","text","Command",param="cmd")], outputs=[E(), P("ok","bool","Ran")], params=[prm("cmd","Command","text","amx_slap @T 0",max=100)], keywords="amxx admin", advanced=True)
# world settings: a named getter and setter each (no cvar names to know)
WORLD = [("gravity","Gravity","sv_gravity","number",800,"How strongly everyone falls (800 normal)."),("friction","Friction","sv_friction","number",4,"How quickly players stop sliding (4 normal)."),("max_speed","Max Speed","sv_maxspeed","number",320,"The fastest anyone can run (320 normal)."),("round_time","Round Time","mp_roundtime","number",2.5,"Minutes a round lasts."),("freeze_time","Freeze Time","mp_freezetime","number",6,"Seconds frozen at a round's start."),("buy_time","Buy Time","mp_buytime","number",0.25,"Minutes players can buy after the round starts."),("friendly_fire","Friendly Fire","mp_friendlyfire","bool",False,"Can teammates hurt each other?"),("team_balance","Team Balance","mp_autoteambalance","bool",True,"Does the game move players to even the teams?"),("team_limit","Team Limit","mp_limitteams","number",2,"How many more players one team may have (0: no limit)."),("start_money","Start Money","mp_startmoney","number",800,"Money at the start."),("c4_timer","C4 Timer","mp_c4timer","number",35,"Seconds until a planted bomb explodes."),("fall_damage","Fall Damage","mp_falldamage","bool",True,"Falls hurt (realistic) or not."),("footsteps","Footsteps","mp_footsteps","bool",True,"Can footsteps be heard?"),("alltalk","All Talk","sv_alltalk","bool",False,"Can everyone hear everyone's voice?"),("time_limit","Time Limit","mp_timelimit","number",20,"Minutes before the map changes (0: none)."),("win_limit","Win Limit","mp_winlimit","number",0,"Rounds won before the map changes (0: none)."),("frag_limit","Frag Limit","mp_fraglimit","number",0,"Kills before the map changes (0: none).")]
for key, title, cvar, ty, dflt, help in WORLD:
    node(f"world.{key}", title, "World", f"{help} ({cvar})", outputs=[P("out", ty, title)], pure=True, keywords=f"{cvar} get", cvar=cvar)
    node(f"world.set_{key}", f"Set {title}", "World", f"{help} ({cvar})", inputs=[X, P("v", ty, title, param="v")], outputs=[E()], params=[prm("v", title, ty, dflt) if ty == "bool" else prm("v", title, "number", dflt, min=0, max=100000)], keywords=f"{cvar} set", cvar=cvar)
# players: what they have and are
node("player.armor", "Armor Of", "Players", "A player's armor points.", inputs=[PL], outputs=[P("out","number","Armor")], **G)
node("player.has_helmet", "Has Helmet", "Players", "Does the player have a helmet?", inputs=[PL], outputs=[P("out","bool","Helmet")], **G)
node("player.weapon_held", "Weapon Held", "Players", "The gun in the player's hands (ak47, knife...).", inputs=[PL], outputs=[P("out","text","Weapon")], **G)
node("player.ammo", "Ammo Of", "Players", "Bullets in the held gun's magazine, and spare ones.", inputs=[PL], outputs=[P("clip","number","In the magazine"), P("reserve","number","Spare")], **G)
node("player.has_weapon", "Has Weapon", "Players", "Does the player carry this weapon?", inputs=[PL], outputs=[P("out","bool","Has it")], params=[prm("weapon","Weapon","choice","ak47",choices=WEAPONS)], **G)
node("player.is_admin", "Is Admin", "Players", "Is the player an admin (the app's admins: AMX Mod X kick rights)?", inputs=[PL], outputs=[P("out","bool","Admin")], **G)
node("player.is_vip", "Is VIP", "Players", "Is the player a VIP (a reserved slot)?", inputs=[PL], outputs=[P("out","bool","VIP")], **G)
node("player.ping", "Ping", "Players", "The player's ping in milliseconds.", inputs=[PL], outputs=[P("out","number","Ping")], **G)
node("player.steam_id", "Steam ID", "Players", "The player's Steam ID (STEAM_0:1:2345): the same each visit, for keeping things per player. Bots and LAN players have none (empty).", inputs=[PL], outputs=[P("out","text","Steam ID")], keywords="authid id", **G)
node("player.is_spectator", "Is Spectator", "Players", "Is the player watching (not on a team)?", inputs=[PL], outputs=[P("out","bool","Spectator")], **G)
node("player.time_connected", "Time Connected", "Players", "Seconds since the player joined.", inputs=[PL], outputs=[P("out","number","Seconds")], **G)
node("player.by_steam", "Player By Steam ID", "Players", "The connected player with this Steam ID (STEAM_0:1:2345, as Steam ID gives it), or none; Found says which. Case and the STEAM_0 / STEAM_1 digit don't matter (the same player either way). Bots and LAN players have no Steam ID.", inputs=[P("id","text","Steam ID",param="id")], outputs=[P("out","player","Player"), P("found","bool","Found")], params=[prm("id","Steam ID","text","STEAM_0:1:2345",max=40)], keywords="authid id steamid find lookup", **G)
node("player.is_supporter", "Is Supporter", "Players", "Has the player the Loner-Strike supporter role (from the Loner-Strike Discord, kept in sync by the app: access flag s)? Not the same as Is VIP (a reserved slot on this server). Bots: no.", inputs=[PL], outputs=[P("out","bool","Supporter")], keywords="supporter role patron donor discord", **G)
node("player.by_name", "Player By Name", "Players", "The player whose name has this text (none: no player).", inputs=[P("name","text","Name",param="name")], outputs=[P("out","player","Player")], params=[prm("name","Name has","text","",max=30)], **G)
node("player.by_index", "Player By Index", "Players", "The player in this slot (1 to Max Players; none if empty).", inputs=[P("i","number","Slot",param="i")], outputs=[P("out","player","Player")], params=[prm("i","Slot","number",1,min=1,max=32)], **G)
node("player.gravity", "Gravity Of", "Players", "A player's own gravity (1 normal; Set Gravity changes it).", inputs=[PL], outputs=[P("out","number","Gravity")], **G)
node("player.speed", "Speed Of", "Players", "A player's top speed now (Set Speed changes it).", inputs=[PL], outputs=[P("out","number","Speed")], **G)
node("player.model", "Model Of", "Players", "A player's model (Set Model changes it).", inputs=[PL], outputs=[P("out","text","Model")], **G)
node("player.transform", "Get Transform", "Players", "Where a player is, where they look (pitch, yaw), how fast they move, and whether they're on the ground or ducking.", inputs=[PL], outputs=[P("pos","vector","Position"), P("angles","vector","Rotation"), P("vel","vector","Velocity"), P("ground","bool","On ground"), P("duck","bool","Ducking")], keywords="location rotation velocity position transform angles", **G)
# players: actions
node("player.set_transform", "Set Transform", "Players", "Moves a player, turns their view (pitch, yaw) and sets their velocity at once (unwired parts stay).", inputs=[X, PL, P("pos","vector","Position",optional=True), P("angles","vector","Rotation",optional=True), P("vel","vector","Velocity",optional=True)], outputs=[E()], keywords="location rotation teleport transform")
node("player.set_rotation", "Set Rotation", "Players", "Turns a player's view (pitch, yaw).", inputs=[X, PL, P("angles","vector","Rotation",required=True)], outputs=[E()], keywords="angles view turn")
node("player.look_at", "Look At", "Players", "Turns a player's view toward an entity or a position.", inputs=[X, PL, P("at","entity","At",optional=True), P("pos","vector","Position",optional=True)], outputs=[E()], keywords="aim face rotation")
node("player.slap", "Slap", "Players", "Slaps a player: some damage and a push.", inputs=[X, PL, P("dmg","number","Damage",param="dmg")], outputs=[E()], params=[prm("dmg","Damage","number",5,min=0,max=100)])
node("player.fade", "Screen Fade", "Players", "Fills the player's screen with a color for some seconds (white: blinded).", inputs=[X, PL], outputs=[E()], params=[prm("colour","Color","choice","white",choices=[["white","White"],["black","Black"],["red","Red"],["green","Green"]]), prm("secs","Seconds","number",2,min=0.1,max=30), prm("alpha","How strong (0-255)","number",255,min=0,max=255)], keywords="blind flash")
node("player.shake", "Screen Shake", "Players", "Shakes the player's screen.", inputs=[X, PL], outputs=[E()], params=[prm("amp","How hard (1-16)","number",8,min=1,max=16), prm("secs","Seconds","number",2,min=0.1,max=30)])
node("player.godmode", "God Mode", "Players", "Nothing hurts the player (or back to normal).", inputs=[X, PL], outputs=[E()], params=[prm("on","On","bool",True)], keywords="invincible")
node("player.noclip", "No Clip", "Players", "The player flies through walls (or back to normal).", inputs=[X, PL], outputs=[E()], params=[prm("on","On","bool",True)], keywords="fly")
node("player.render", "Set Visibility", "Players", "How a player is drawn: normal, see-through (an amount), or invisible.", inputs=[X, PL], outputs=[E()], params=[prm("mode","Look","choice","invisible",choices=[["normal","Normal"],["see","See-through"],["invisible","Invisible"]]), prm("amount","See-through amount (0-255)","number",80,min=0,max=255)], keywords="invisible render alpha")
node("player.set_ammo", "Set Ammo", "Players", "Sets the held gun's magazine and spare bullets.", inputs=[X, PL, P("clip","number","Magazine",param="clip"), P("reserve","number","Spare",param="reserve")], outputs=[E()], params=[prm("clip","Magazine","number",30,min=0,max=200), prm("reserve","Spare","number",90,min=0,max=255)])
node("player.give_ammo", "Give Ammo", "Players", "Adds spare bullets for the held gun (a minus takes them).", inputs=[X, PL, P("n","number","Bullets",param="n")], outputs=[E()], params=[prm("n","Bullets","number",30,min=-255,max=255)])
node("player.take_weapon", "Remove Weapon", "Players", "Takes one weapon from a player (the others stay).", inputs=[X, PL], outputs=[E()], params=[prm("weapon","Weapon","choice","ak47",choices=WEAPONS + [["c4","C4 (the bomb)"]])], keywords="strip drop bomb")
node("player.switch_weapon", "Switch To Weapon", "Players", "Makes the player take out this weapon (if they have it).", inputs=[X, PL], outputs=[E()], params=[prm("weapon","Weapon","choice","knife",choices=WEAPONS + [["knife","Knife"]])])
node("player.set_frags", "Set Kills", "Players", "Sets a player's kills on the scoreboard.", inputs=[X, PL, P("n","number","Kills",param="n")], outputs=[E()], params=[prm("n","Kills","number",0,min=-1000,max=10000)], keywords="frags score")
node("player.set_deaths", "Set Deaths", "Players", "Sets a player's deaths on the scoreboard.", inputs=[X, PL, P("n","number","Deaths",param="n")], outputs=[E()], params=[prm("n","Deaths","number",0,min=0,max=10000)])
node("player.to_spectators", "Move To Spectators", "Players", "Moves a player out of the game to watch.", inputs=[X, PL], outputs=[E()])
node("player.kick", "Kick", "Players", "Kicks a player from the server with a reason. Asks first when an agent does it; community modes that use it show \"kicks players\". (Bans stay with the app's admins: use Notify Admins.)", inputs=[X, PL], outputs=[E()], params=[prm("reason","Reason","text","Kicked by the mode",max=60)], advanced=True)
node("player.mute", "Mute Voice", "Players", "The player's voice isn't heard for some seconds (0: until the map ends).", inputs=[X, PL], outputs=[E()], params=[prm("secs","Seconds","number",60,min=0,max=3600)])
node("player.gag", "Gag Chat", "Players", "The player's chat isn't shown for some seconds (0: until the map ends).", inputs=[X, PL], outputs=[E()], params=[prm("secs","Seconds","number",60,min=0,max=3600)])
node("msg.team_chat", "Chat To Team", "Messages", "A chat line only one team sees.", inputs=[X, P("value","any","Value",optional=True)], outputs=[E()], params=[prm("team","Team","choice","ct",choices=TEAM2), prm("text","Text","text","Hold the line!",max=100)])
node("msg.notify_admins", "Notify Admins", "Messages", "A chat line only admins see (for things a mode can't do itself, like a ban).", inputs=[X, P("value","any","Value",optional=True)], outputs=[E()], params=[prm("text","Text","text","Please check player:",max=100)])
# entities
node("ent.class", "Entity Class Of", "Entities", "An entity's class (func_door, ls_creature...).", inputs=[P("in","entity","Entity",required=True)], outputs=[P("out","text","Class")], **G)
node("ent.name", "Entity Name Of", "Entities", "An entity's name (targetname) as the map gives it.", inputs=[P("in","entity","Entity",required=True)], outputs=[P("out","text","Name")], **G)
node("ent.find_all", "Find Entities", "Entities", "Every entity of a class or with a name, as an array (at most 512). Looks through the whole map: keep it in a variable instead of in a loop.", outputs=[P("out","array<entity>","Entities")], params=[prm("by","By","choice","class",choices=[["class","Class"],["name","Name"]]), prm("name","Class or name","entname","info_target")], **G)
node("ent.spawn_prop", "Spawn Prop", "Entities", "A model (models/...mdl, loaded with the map) at an entity's place; solid or not.", inputs=[X, P("at","entity","At",required=True)], outputs=[E(), P("out","entity","Prop")], params=[prm("model","Model","path","models/w_c4.mdl"), prm("solid","Solid","bool",True)])
node("ent.remove", "Remove Entity", "Entities", "Removes an entity (never a player).", inputs=[X, P("in","entity","Entity",required=True)], outputs=[E()])
node("ent.set_position", "Set Entity Position", "Entities", "Moves an entity.", inputs=[X, P("in","entity","Entity",required=True), P("pos","vector","Position",required=True)], outputs=[E()], keywords="location")
node("ent.set_angles", "Set Entity Rotation", "Entities", "Turns an entity (pitch, yaw, roll).", inputs=[X, P("in","entity","Entity",required=True), P("angles","vector","Rotation",required=True)], outputs=[E()], keywords="angles")
node("ent.set_visible", "Set Entity Visibility", "Entities", "Shows or hides an entity.", inputs=[X, P("in","entity","Entity",required=True)], outputs=[E()], params=[prm("on","Visible","bool",True)])
# effects
node("fx.explosion", "Explosion", "Effects", "An explosion at a position: the look and sound, and damage to players within the radius (0: none).", inputs=[X, P("pos","vector","Position",required=True), P("dmg","number","Damage",param="dmg")], outputs=[E()], params=[prm("dmg","Damage","number",0,min=0,max=500), prm("radius","Radius","number",250,min=16,max=2000)])
node("fx.beam", "Beam", "Effects", "A beam of light from one position to another for some seconds.", inputs=[X, P("a","vector","From",required=True), P("b","vector","To",required=True)], outputs=[E()], params=[prm("colour","Color","choice","red",choices=[["red","Red"],["green","Green"],["blue","Blue"],["white","White"],["gold","Gold"]]), prm("secs","Seconds","number",1,min=0.1,max=25)])
node("fx.sound_at", "Play Sound At", "Effects", "A sound (sound/...wav, loaded with the map) heard around a position.", inputs=[X, P("pos","vector","Position",required=True)], outputs=[E()], params=[prm("sound","Sound","path","ambience/thunder_clap.wav")])
node("fx.sound_to", "Play Sound To Player", "Effects", "A sound only one player hears (from the game's own sounds).", inputs=[X, PL], outputs=[E()], params=[prm("sound","Sound","path","buttons/bell1.wav")])
node("fx.lights", "Set Light Style", "Effects", "How bright the whole map is: normal, dark, darkest, bright, or flickering.", inputs=[X], outputs=[E()], params=[prm("style","Light","choice","normal",choices=[["normal","Normal"],["dark","Dark"],["darkest","Darkest"],["bright","Bright"],["flicker","Flickering"]])], keywords="light dark night")
# HUD icons, objective markers, kill feed lines
STATUS_ICONS = [["buyzone","Buy zone (cart)"],["c4","C4"],["defuser","Defuser"],["rescue","Rescue zone"],["escape","Escape zone"],["vipsafety","VIP safety zone"],["stopwatch","Stopwatch"],["smallskull","Skull"],["smallc4","Small C4"],["smallvip","Small VIP"],["hostage","Hostage"],["dollar","Dollar sign"],["cross","Cross"],["item_healthkit","Health kit"],["item_battery","Battery"],["item_longjump","Long jump"],["dmg_poison","Poison"],["dmg_bio","Biohazard"],["dmg_chem","Chemical"],["dmg_drown","Drowning"],["dmg_gas","Gas"],["dmg_cold","Cold"],["dmg_heat","Heat"],["dmg_rad","Radiation"],["dmg_shock","Shock"]]
KILL_ICONS = [[w, w] for w in ["knife","ak47","m4a1","awp","deagle","usp","glock18","p228","fiveseven","elite","scout","g3sg1","sg550","sg552","aug","famas","galil","mp5navy","tmp","mac10","ump45","p90","m3","xm1014","m249","grenade","skull","tracktrain"]]
RGB = lambda r, g, b: [prm("r","Red","number",r,min=0,max=255), prm("g","Green","number",g,min=0,max=255), prm("b","Blue","number",b,min=0,max=255)]
node("msg.status_icon", "Status Icon", "Messages", "Shows, flashes or hides one of the game's own HUD status icons (the buy zone cart, C4, the defuser, the rescue zone...) for a player or everyone (no Player), in a color. Only the icons every player's game has (its hud.txt): for your own pictures use Objective Marker or Show HUD Text. Until death: it goes when they die or leave.",
     inputs=[X, P("player","player","Player",optional=True)], outputs=[E()],
     params=[prm("icon","Icon","choice","buyzone",choices=STATUS_ICONS), prm("show","Show","choice","on",choices=[["on","On"],["flash","Flashing"],["off","Off"]])] + RGB(0, 160, 0) + [prm("death","Until death","bool",True)],
     keywords="hud icon status buyzone c4 defuser objective")
node("fx.marker", "Objective Marker", "Effects", "A sprite that floats over an entity (wired into Follow: a player, an NPC, an interactable, a component; it follows it) or at a position, for everyone, one team or one player. The sprite is one of the game's (sprites/arrow1.spr) or your own (sprites/...spr: it goes with the mode, like its other files). Remove it with Remove Marker; it goes by itself when what it follows is gone.",
     inputs=[X, P("follow","entity","Follow"), P("pos","vector","Position",optional=True), P("player","player","Player (for one player)",optional=True)], outputs=[E(), P("out","entity","Marker")],
     params=[prm("sprite","Sprite","path","sprites/arrow1.spr"), prm("who","Seen by","choice","all",choices=[["all","Everyone"],["t","Terrorists"],["ct","Counter-Terrorists"],["player","The wired player"]]), prm("height","Height above","number",48,min=0,max=512), prm("scale","Size","number",0.5,min=0.05,max=5)] + RGB(255, 200, 0) + [prm("pulse","Pulse","bool",True)],
     keywords="waypoint objective arrow icon sprite marker quest")
node("fx.marker_remove", "Remove Marker", "Effects", "Takes away an Objective Marker (the one wired in, or all of them with none).", inputs=[X, P("marker","entity","Marker",optional=True)], outputs=[E()], keywords="waypoint objective arrow")
node("msg.killfeed", "Kill Feed Line", "Messages", "A line in the top-right kill feed for everyone: Killer (or none: the world), the weapon's icon, Victim, and the headshot mark. Only the feed: nobody dies, nothing is counted (Player Killed doesn't run).",
     inputs=[X, P("killer","player","Killer",optional=True), P("victim","player","Victim",required=True), P("hs","bool","Headshot",param="hs")], outputs=[E()],
     params=[prm("weapon","Weapon icon","choice","knife",choices=KILL_ICONS), prm("hs","Headshot","bool",False)],
     keywords="kill feed death notice frag")
# fun: fire, trails, bursts, swaps, burying, the camera, a drunk view
node("fun.burn", "Set On Fire", "Players", "Sets a living player on fire for some seconds: damage each second (Lit by gets the kill), flames on them. Water or the time running out puts it out; so does Put Out Fire, death or a new round.",
     inputs=[X, PL, P("by","player","Lit by",optional=True)], outputs=[E()], params=[prm("secs","Seconds","number",5,min=1,max=60), prm("dps","Damage a second","number",5,min=0,max=100)], keywords="fire burn flame ignite")
node("fun.extinguish", "Put Out Fire", "Players", "Puts out a player Set On Fire.", inputs=[X, PL], outputs=[E()], keywords="fire burn water")
node("fun.trail", "Trail", "Effects", "A colored trail behind a player or an entity as it moves (on), or none (off). It comes back after a respawn and when they start moving again.",
     inputs=[X, P("ent","entity","On",required=True)], outputs=[E()], params=[prm("on","On","bool",True)] + RGB(0, 160, 255) + [prm("width","Width","number",6,min=1,max=50), prm("life","Length (seconds)","number",2,min=0.1,max=25)], keywords="trail beam follow streak")
BURSTS = [["sparks","Sparks"],["smoke","Smoke"],["blood","Blood"],["firework","Firework"],["implosion","Implosion"],["tar","Tar explosion"],["teleport","Teleport flash"],["lava","Lava splash"],["flash","Colored flash (light)"]]
node("fun.burst", "Effect Burst", "Effects", "A burst at a position or an entity: sparks, smoke, blood, a firework, an implosion, a tar explosion, a teleport flash, a lava splash or a colored flash. Count repeats it (up to 10); Color is for the firework and the flash.",
     inputs=[X, P("at","entity","At"), P("pos","vector","Position",optional=True)], outputs=[E()], params=[prm("kind","Effect","choice","sparks",choices=BURSTS), prm("count","Count","number",1,min=1,max=10)] + RGB(255, 120, 0), keywords="effect particles sparks smoke blood firework explosion")
node("fun.swap", "Swap Places", "Players", "Two living players trade places, the way they face and their speed. Not when either would end up stuck in a wall (Swapped is No then).",
     inputs=[X, P("a","player","Player A",required=True), P("b","player","Player B",required=True)], outputs=[E(), P("ok","bool","Swapped")], keywords="swap trade teleport switch")
node("fun.bury", "Bury", "Players", "Sinks a living player into the floor up to the chest; they can't walk, but can look round and shoot. Unbury gets them out (also death and a new round).", inputs=[X, PL], outputs=[E()], keywords="bury sink ground stuck")
node("fun.unbury", "Unbury", "Players", "Gets a buried player out of the floor, onto it (never into a wall).", inputs=[X, PL], outputs=[E()], keywords="bury dig out")
node("fun.camera", "Set Camera", "Players", "A player's view: third person (from behind), back to first person, or the other of the two. Back to first person on death/respawn: on, death ends it; off, they stay in third person through deaths and respawns. Leaving always ends it.",
     inputs=[X, PL], outputs=[E()], params=[prm("view","View","choice","third",choices=[["third","Third person"],["first","First person"],["toggle","Switch"]]), prm("reset","Back to first person on death/respawn","bool",True)], keywords="camera third person view thirdperson")
node("fun.is_third", "Is Third Person", "Players", "Is the player's view third person (Set Camera)?", inputs=[PL], outputs=[P("out","bool","Third person")], keywords="camera view", pure=True)
node("fun.view_from", "View From", "Players", "A cinematic view: the chosen players (the wired one, a team or everyone; each gets a camera of their own) see from a position and angles, or from an entity (following it, Offset above it), for some seconds (0: until Return View). Look at: an entity or position it keeps pointing at (its angles worked out, over Angles). Move to: it glides there over Move time. Frame a conversation: with the wired player and an NPC in Look at, the camera stands to the side of both, looking at the NPC (for Say and Choose); Until the conversation ends gives the view back when it does. Death, leaving and a new round give the view back by themselves. Freeze: they can't move or shoot meanwhile. A map's own trigger_camera still works.",
     inputs=[X, P("player","player","Player",optional=True), P("from","entity","From entity",optional=True), P("pos","vector","Position",optional=True), P("angles","vector","Angles",optional=True), P("to","vector","Move to",optional=True), P("to_angles","vector","Move to angles",optional=True), P("look","entity","Look at",optional=True), P("look_pos","vector","Look at position",optional=True)], outputs=[E()],
     params=[prm("who","Who","choice","player",choices=[["player","The wired player"],["t","Terrorists"],["ct","Counter-Terrorists"],["all","Everyone"]]), prm("secs","Seconds (0: until Return View)","number",5,min=0,max=120), prm("offset","Offset above (from an entity)","number",64,min=-256,max=512), prm("move","Move time","number",3,min=0.1,max=60), prm("freeze","Freeze player while viewing","bool",True), prm("frame","Frame a conversation","bool",False), prm("talk","Until the conversation ends","bool",False)],
     keywords="camera cinematic cutscene view intro shot")
node("fun.view_return", "Return View", "Players", "Gives back the normal view (first or third person, as Set Camera left it) after View From: to the wired player, or everyone viewing.", inputs=[X, P("player","player","Player",optional=True)], outputs=[E()], keywords="camera cinematic cutscene view")
node("fun.drunk", "Drunk View", "Players", "Wobbles a player's view for some seconds, gently (1) to wildly (10); it stops cleanly (also on death).", inputs=[X, PL], outputs=[E()], params=[prm("secs","Seconds","number",8,min=1,max=60), prm("strength","Strength","number",4,min=1,max=10)], keywords="drunk dizzy wobble view shake")
# casting (Blueprint style): an Entity wire as a Player or a Creature, and what an entity is
COMP_KINDS = [["any","Any component"],["ls_capture_point","Capture point"],["ls_extraction_zone","Extraction zone"],["ls_checkpoint","Checkpoint"],["ls_shop","Shop"],["ls_barricade","Barricade"],["ls_trigger_trap","Trap"],["ls_loot_container","Loot container"],["ls_supply_drop","Supply drop"],["ls_supply_crate","Supply crate"],["ls_perk_machine","Perk machine"],["ls_upgrade_bench","Upgrade bench"],["ls_mystery_box","Mystery box"],["ls_meeting_point","Meeting point"],["ls_moving_platform","Moving platform"],["ls_timed_door","Timed door"],["ls_wave_spawner","Wave spawner"],["ls_safe_zone","Safe zone"],["ls_zone_shrinker","Shrinking zone"],["ls_damage_zone","Damage zone"],["ls_weapon_spawner","Weapon spawner"],["ls_respawn_point","Respawn point"],["ls_player_class","Player class"],["ls_pickup","Pickup"],["ls_npc","NPC"],["ls_interactable","Interactable"]]
node("cast.player", "As Player", "Players", "Casting: is this entity a player (a slot 1-32 that's connected, bots too)? Success with it as a Player, else Failed. Use it where an Entity wire should go into a Player pin.",
     inputs=[X, P("in","entity","Entity",required=True)], outputs=[E("ok","Success"), E("fail","Failed"), P("out","player","Player")], keywords="casting cast as player entity convert")
node("cast.creature", "As Creature", "Creatures", "Casting: is this entity one of the bot brain's creatures? Success with it as a Creature (and its type), else Failed.",
     inputs=[X, P("in","entity","Entity",required=True)], outputs=[E("ok","Success"), E("fail","Failed"), P("out","entity","Creature"), P("type","text","Type")], keywords="casting cast as creature entity convert")
node("ent.is_npc", "Is NPC", "Entities", "Casting: is the entity an NPC (ls_npc)?", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","bool","NPC")], keywords="casting npc character", pure=True)
node("ent.is_interactable", "Is Interactable", "Entities", "Casting: is the entity an interactable (ls_interactable, a point or a brush)?", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","bool","Interactable")], keywords="casting interactable lever button", pure=True)
node("ent.kind_of", "Kind Of", "Entities", "Casting: what the entity is, as the built-in Entity Kind enum: Player, Bot, Creature, NPC, Interactable, Component, Prop (Spawn Prop's), Map entity or None. Wire it into Switch on Enum: one output per kind, nothing to type.", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","enum:ls_kind","Kind")], keywords="casting kind type what switch", pure=True)
node("ent.comp_kind", "Component Kind Of", "Entities", "Casting: which kind of game mode component the entity is, as the built-in Component Kind enum (capture point, shop, barricade, timed door...), or None. Wire it into Switch on Enum for one output per kind.", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","enum:ls_comp_kind","Component kind")], keywords="casting component kind switch", pure=True)
node("ent.is_component", "Is Component", "Entities", "Casting: is the entity a game mode component (any, or of one kind: a capture point, a shop, a barricade...)?", inputs=[P("in","entity","Entity",optional=True)], outputs=[P("out","bool","Component")], params=[prm("kind","Kind","choice","any",choices=COMP_KINDS)], keywords="casting component kind", pure=True)
# AMX Mod X: other plugins
node("amx.call", "Call Plugin Function", "Advanced", "Calls a public function in another AMX Mod X plugin (by its file, like gm_components.amxx) with up to two numbers; Result is what it returns. Community modes that use it show \"calls other plugins\".", inputs=[X, P("a","number","Argument 1",param="a"), P("b","number","Argument 2",param="b")], outputs=[E(), P("out","number","Result"), P("ok","bool","Called")], params=[prm("plugin","Plugin file","name","gm_components.amxx"), prm("func","Function","name","plugin_cfg"), prm("args","Arguments","choice","0",choices=[["0","None"],["1","One"],["2","Two"]]), prm("a","Argument 1","number",0,min=-100000,max=100000), prm("b","Argument 2","number",0,min=-100000,max=100000)], keywords="callfunc interop", advanced=True)
node("amx.plugin_running", "Plugin Is Running", "Advanced", "Is this AMX Mod X plugin loaded and running?", outputs=[P("out","bool","Running")], params=[prm("plugin","Plugin file","name","gm_components.amxx")], pure=True)
node("amx.fire", "Fire Custom Event", "Advanced", "Sends an event with a name and two numbers that other plugins (and other Studio modes, with On Custom Event) can listen to (forward ls_studio_event(name[], a, b)).", inputs=[X, P("a","number","A",param="a"), P("b","number","B",param="b")], outputs=[E()], params=[prm("name","Event name","name","my_event"), prm("a","A","number",0,min=-100000,max=100000), prm("b","B","number",0,min=-100000,max=100000)], keywords="forward interop")
node("amx.on_event", "On Custom Event", "Advanced", "Runs when this mode or another plugin fires a custom event with this name.", outputs=[E(), P("a","number","A"), P("b","number","B")], params=[prm("name","Event name","name","my_event")], event=True)
# events
node("event.player_said", "Player Said", "Events", "Runs when a player chats (anything; Team Only for team chat).", outputs=[E(), P("player","player","Player"), P("text","text","Text"), P("team","bool","Team only")], **ev)
node("event.freeze_ended", "Freeze Time Ended", "Events", "Runs when the freeze time ends and players can move.", outputs=[E()], **ev)
node("event.map_ending", "Map Ending", "Events", "Runs when the map ends (the scoreboard shows), just before the next map.", outputs=[E()], **ev)
node("event.weapon_fired", "Weapon Fired", "Events", "Runs when a player shoots (at most 10 times a second per player). Frequent: keep the flow short.", outputs=[E(), P("player","player","Player"), P("weapon","text","Weapon")], **ev)
node("event.reloaded", "Reloaded", "Events", "Runs when a player starts reloading.", outputs=[E(), P("player","player","Player"), P("weapon","text","Weapon")], **ev)
node("event.hostage_rescued", "Hostage Rescued", "Events", "Runs when a player rescues a hostage.", outputs=[E(), P("player","player","Player")], **ev)
node("event.vip_escaped", "VIP Escaped", "Events", "Runs when the VIP escapes.", outputs=[E(), P("player","player","Player")], **ev)

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
     params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("kind","Goal","choice","defend",choices=[["defend","Defend it"],["capture","Capture it"],["use","Use it"],["loot","Loot it"],["go_hold","Go and hold there"],["hunt","Hunt it (a player or creature, wherever it goes)"]]), prm("prio","Priority","number",50,min=0,max=100), prm("secs","For (seconds)","number",60,min=1,max=3600), prm("radius","Within (units)","number",64,min=8,max=4000)], **BH)
node("bots.goal_clear", "Bots: Clear Goals", "Bots", "Takes back goals given with Bots: Goal (all of them, or one kind).", inputs=[X, BWHO], outputs=[E()], params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("kind","Goals","choice","0",choices=[["0","All"],["defend","Defend"],["capture","Capture"],["use","Use"],["loot","Loot"],["go_hold","Go and hold"]])], **BH)
node("bots.offer", "Bots: Offer", "Bots", "Tells bots something here can be bought or used for a price (a perk, an upgrade bench, a mystery box, a trap, a turret spot); they take it like a person would, when they can afford it and it's calm.", inputs=[X, BWHO, P("at","entity","At",required=True), P("cost","number","Price",param="cost")], outputs=[E()],
     params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name",""), prm("kind","What","choice","perk",choices=[["perk","A perk"],["bench","An upgrade bench"],["box","A mystery box"],["trap","A trap's switch"],["turret","A turret spot"],["use","An interactable (they press E on it)"],["pickup","A pickup (they walk over it; Price is its weight 0-100, Order 0 an item, 1 a weapon, 2 health)"]]), prm("cost","Price","number",1000,min=0,max=100000), prm("order","Order (perks: lowest first)","number",0,min=0,max=20)], **BH)
node("bots.offer_clear", "Bots: Clear Offer", "Bots", "Takes back an offer (it's bought or used up), or all of them.", inputs=[X, BWHO, P("at","entity","At",optional=True)], outputs=[E()], params=[prm("who","Which bots","choice","all",choices=[["all","All bots"],["ct","Counter-Terrorist bots"],["t","Terrorist bots"],["player","The wired bot"],["tag","Bots with this tag"]]), prm("tag","Tag","name","")], **BH)
node("bot.goto", "Bot Go To", "Bots", "Sends a bot to the entity with this name (it still fights on the way), for some seconds (0: until freed).", inputs=[X, P("bot","player","Bot",required=True)], outputs=[E()], params=[prm("name","To the entity named","name","spot1"), prm("secs","For (seconds, 0 until freed)","number",0,min=0,max=600)])
node("bot.hold", "Bot Hold", "Bots", "Makes a bot hold where it is for some seconds.", inputs=[X, P("bot","player","Bot",required=True), P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",20,min=1,max=600)])
node("bot.follow", "Bot Follow", "Bots", "Makes a bot follow a player for some seconds.", inputs=[X, P("bot","player","Bot",required=True), P("who","player","Follow",required=True)], outputs=[E()], params=[prm("secs","Seconds","number",30,min=1,max=600)])
node("bot.skill", "Bot Nature", "Bots", "A bot's nature, as the bot brain plays it: Skill, Aggression, Teamwork and Greed (0-100) and its playstyle (rusher, lurker, careful, camper...). People read 0 and empty.",
     inputs=[P("bot","player","Bot",required=True)], outputs=[P("skill","number","Skill"), P("aggression","number","Aggression"), P("teamwork","number","Teamwork"), P("greed","number","Greed"), P("style","text","Playstyle")], pure=True, keywords="personality skill aggression teamwork style")
node("bot.trait", "Bot Trait", "Bots", "Is the bot bold, cautious, a team player, a loner, greedy, skilled or a novice (as the bot brain sees it)? False for people.",
     inputs=[P("bot","player","Bot",required=True)], outputs=[P("out","bool","Has it")], params=[prm("trait","Trait","choice","bold",choices=[["bold","Bold"],["cautious","Cautious"],["team","Team player"],["loner","Loner"],["greedy","Greedy"],["skilled","Skilled"],["novice","Novice"]])], pure=True, keywords="personality bold careful")
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
     outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("damage","number","Damage"), P("kind","text","Kind"), P("headshot","bool","Headshot"), P("hitgroup","text","Hit where")], **ev)
node("dmg.block", "Block Damage", "Damage", "The damage doesn't happen. Only in a Player About To Be Hurt flow.", inputs=[X], outputs=[E()])
node("dmg.scale", "Scale Damage", "Damage", "Multiplies the damage (0.5 halves it). Only in a Player About To Be Hurt flow.", inputs=[X, P("factor","number","Times",param="factor")], outputs=[E()], params=[prm("factor","Times","number",0.5,min=0,max=100)])
node("dmg.set", "Set Damage", "Damage", "Sets the damage to this. Only in a Player About To Be Hurt flow.", inputs=[X, P("amount","number","Damage",param="amount")], outputs=[E()], params=[prm("amount","Damage","number",10,min=0,max=10000)])
node("player.no_fall", "No Fall Damage", "Players", "Stops a player taking fall damage: always, or just for their next landing (a drop-in).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("mode","When","choice","once",choices=[["once","Their next landing"],["on","Always"],["off","Off again"]])])
node("player.down", "Make Downed", "Players", "A player who just died lies downed where they fell: a teammate (or squad mate) holding USE on them for a few seconds brings them back; if nobody does in time they're out. Bots come to help.",
     inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("secs","Out after (seconds)","number",30,min=3,max=300), prm("hold","Hold USE (seconds)","number",3,min=1,max=20), prm("health","Health when revived","number",50,min=1,max=255)])
node("comp.revive_pay", "Revive Pay", "Upgrades", "Money for getting a teammate up (the components' downed players: Last Stand, Battle Royale), once a downing, never for getting yourself up; bots get it too. 0: off.",
     inputs=[X, P("amount","number","Amount ($)",param="amount")], outputs=[E()], params=[prm("amount","Amount ($)","number",150,min=0,max=16000)], keywords="revive bonus reward money", needs="components")
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
node("flow.sequence", "Sequence", "Flow", "Runs each output in turn, top to bottom.", inputs=[X], outputs=[E("then0","Then 0"), E("then1","Then 1")], params=[prm("count","Outputs","number",2,min=1,max=32)])
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
node("flow.if_valid", "Is Valid?", "Conditions", "Goes to Is Valid when the entity is there, else Is Not Valid. Any entity handle works: a player or bot, a prop, creature, NPC, interactable, component, spawned entity or weapon (a spawn node's Entity), a map entity. A removed one isn't valid.", inputs=[X, P("in","entity","Object",optional=True)], outputs=[E("valid","Is Valid"), E("invalid","Is Not Valid")])
node("timer.retrigger", "Retriggerable Delay", "Timers", "Waits, then carries on; flow coming in again while waiting starts the wait again.", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E(), P("handle","timer","Timer")], params=[prm("secs","Seconds","number",3,min=0.1,max=3600)])
node("timer.by_function", "Set Timer by Function", "Timers", "Runs a Function (with its own graph) after some seconds, again and again if Looping. Its inputs get nothing (0, empty).",
     inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E(), P("handle","timer","Timer")], params=[prm("func","Function","fn",""), prm("secs","Seconds","number",5,min=0.1,max=3600), prm("loop","Looping","bool",True)])
node("timer.by_event", "Set Timer by Event", "Timers", "Runs a Custom Event (with no inputs) after some seconds, again and again if Looping.",
     inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E(), P("handle","timer","Timer")], params=[prm("event","Custom event","cevent",""), prm("secs","Seconds","number",5,min=0.1,max=3600), prm("loop","Looping","bool",True)], keywords="custom event delay schedule")
node("event.custom", "Custom Event", "Events", "Runs when something calls this Custom Event (Call Custom Event, Set Timer by Event, or the map: an ls_event entity, or an info_target named lsev_<its map name>, when triggered). Its inputs are its own (My mode, Custom events).",
     outputs=[E()], params=[prm("event","Custom event","cevent","")], event=True, keywords="dispatcher signal trigger map")
node("event.call", "Call Custom Event", "Flow", "Runs a Custom Event now, with these values (every Custom Event node for it, in the graphs' order).",
     inputs=[X], outputs=[E()], params=[prm("event","Custom event","cevent","")], keywords="fire broadcast signal")
# ---- creature control (the bot brain's ls_creature commands; docs/notes/X.md "Creature control")
CRN = dict(needs="creatures")
CR_IN = P("creature","entity","Creature",required=True)
TYPE = prm("type","Type (empty: any)","name","")
node("creature.for_each", "For Each Creature", "Creatures", "Runs Each once for every creature alive (of a type, or any), then Then. Friendly ones (turrets) are left out. Every creature each time it runs: keep it out of per-frame flows.",
     inputs=[X], outputs=[E("each","Each"), P("creature","entity","Creature"), E("then","Then")], params=[TYPE], keywords="loop all monsters zombies", **CRN)
node("creature.of_type", "Creatures of Type", "Creatures", "Every creature alive of a type (empty: any), as an array. Looks through them all each time: keep it in a variable instead of in a loop.",
     outputs=[P("out","array<entity>","Creatures")], params=[TYPE], pure=True, keywords="list all monsters", **CRN)
node("creature.nearest", "Nearest Creature", "Creatures", "The creature alive nearest to a point or an entity (a player, a gate), of a type or any; nobody if there are none.",
     inputs=[P("pos","vector","To point",optional=True), P("to","entity","To entity",optional=True)], outputs=[P("out","entity","Creature")], params=[TYPE], pure=True, keywords="closest monster", **CRN)
node("creature.type", "Creature Type", "Creatures", "A creature's type (zombie, runner...); empty for anything else.", inputs=[CR_IN], outputs=[P("out","text","Type")], pure=True, **CRN)
node("creature.idle_for", "Idle For", "Creatures", "Seconds a creature has stood still (moved less than 32 units). Counted once a second.", inputs=[CR_IN], outputs=[P("out","number","Seconds")], pure=True, keywords="stuck still waiting", **CRN)
node("creature.can_see", "Can See", "Creatures", "Is there a clear line from the creature's eyes to a player or entity?", inputs=[CR_IN, P("what","entity","Player or entity",required=True)], outputs=[P("out","bool","Sees")], pure=True, keywords="sight visible line of sight", **CRN)
node("creature.go_to", "Creature Go To", "Creatures", "Sends a creature to a place: an entity wired in (a player, a marker's entity), a point, or the entity with this name. Guard there: it stays and fights there. Then resume: it goes back to hunting once there.",
     inputs=[X, CR_IN, P("to","entity","To entity",optional=True), P("pos","vector","To point",optional=True)], outputs=[E()],
     params=[prm("name","Or the entity named","entname","gate1"), prm("then","Then","choice","guard",choices=[["guard","Guard there"],["resume","Resume hunting"]])], keywords="move send walk gate guard", **CRN)
node("creature.attack", "Creature Attack", "Creatures", "Makes a creature go after a player or an entity (a gate, a turret) until it's gone.", inputs=[X, CR_IN, P("target","entity","Target",required=True)], outputs=[E()], keywords="target chase hunt", **CRN)
node("creature.hold", "Creature Hold", "Creatures", "Makes a creature stay where it is (it guards the spot: it still fights what comes close, then comes back) until Resume AI or Go To.", inputs=[X, CR_IN], outputs=[E()], keywords="stop freeze wait", **CRN)
node("creature.resume", "Creature Resume AI", "Creatures", "Gives a creature back to its own AI: what its type does by itself (wander, patrol, guard or hunt), its spawn spot as home, no target forced. With an older bot brain it hunts instead.", inputs=[X, CR_IN], outputs=[E()], keywords="release free default", **CRN)
CR_V2 = " Needs a bot brain with creature control 2 (ls_creature_ver 2); the mode says so in the server log when it's older."
node("creature.state", "Creature State", "Creatures", "What a creature is doing now: 1 idle, 2 wander, 3 patrol, 4 guard, 5 chase, 6 flee, 7 attack, 8 search, 9 dead (0: not a creature). Name gives the word (idle ... dead)." + CR_V2,
     inputs=[CR_IN], outputs=[P("state","number","State"), P("name","text","Name")], pure=True, keywords="mode doing ai behavior chasing fleeing", **CRN)
node("creature.target", "Creature Target", "Creatures", "Who a creature is after now: a player or another creature; nobody when none." + CR_V2,
     inputs=[CR_IN], outputs=[P("out","entity","Target")], pure=True, keywords="enemy chasing whom hunting", **CRN)
node("creature.set_speed", "Set Creature Speed", "Creatures", "Times a creature's normal walk and run speed (0.2 to 3; 1 is its own) until it dies. Enraged comes on top." + CR_V2,
     inputs=[X, CR_IN, P("mult","number","Times",param="mult")], outputs=[E()], params=[prm("mult","Times its speed","number",1.5,min=0.2,max=3)], keywords="fast slow haste", **CRN)
node("creature.set_tier", "Set Creature Tier", "Creatures", "Makes a creature tier 0 to 3 (2: a yellow shell, 3: red-orange and a bit bigger), as the spawn key tier does; its health keeps its share." + CR_V2,
     inputs=[X, CR_IN, P("tier","number","Tier",param="tier")], outputs=[E()], params=[prm("tier","Tier","number",2,min=0,max=3)], keywords="elite rank glow upgrade", **CRN)
node("creature.flee_to", "Creature Flee To", "Creatures", "Makes a creature run away to a place (an entity wired in, a point, or the entity with this name).",
     inputs=[X, CR_IN, P("to","entity","To entity",optional=True), P("pos","vector","To point",optional=True)], outputs=[E()], params=[prm("name","Or the entity named","entname","")], keywords="run away retreat", **CRN)
node("creature.set_health", "Set Creature Health", "Creatures", "Sets a creature's health (and its full health, so it isn't over).", inputs=[X, CR_IN, P("hp","number","Health",param="hp")], outputs=[E()], params=[prm("hp","Health","number",100,min=1,max=100000)], keywords="hp heal", **CRN)
node("creature.remove", "Remove Creature", "Creatures", "Takes a creature away. Kill: it dies (Creature Killed runs, waves count it; nobody is paid). Remove quietly: it's gone, nothing runs.", inputs=[X, CR_IN], outputs=[E()],
     params=[prm("how","How","choice","kill",choices=[["kill","Kill (counts as a death)"],["remove","Remove quietly"]])], keywords="despawn delete kill", **CRN)
node("creature.on_idle", "Creature Idle", "Creatures", "Runs when a creature has stood still this many seconds (and, if asked, can't see any player): send it somewhere with Creature Go To. Runs again after as many seconds if it's still stuck.",
     outputs=[E(), P("creature","entity","Creature"), P("secs","number","Idle for")], params=[prm("secs","Seconds","number",12,min=2,max=600), prm("unseen","Only when no player is in sight","bool",True)], event=True, keywords="stuck wait gate send", **CRN)

# ---- data tables (X2; tables.rs): a mode's own grids of values, read as constant arrays
TB = prm("table","Table","table","")
TCOL = prm("column","Column","column","")
node("table.get_row", "Get Row", "Data tables", "Finds the row whose key is this (the table's key column) and gives each of its cells. Found is No when no row has that key (the cells are then 0, No or empty).", inputs=[P("key","any","Key",required=True)], outputs=[P("found","bool","Found"), P("row","number","Row")], params=[TB], pure=True, keywords="lookup data table row key csv")
node("table.get_cell", "Get Cell", "Data tables", "One cell: a column of a row (rows count from 0). Past the end: 0, No or empty.", inputs=[P("row","number","Row",param="row")], outputs=[P("out","any","Value")], params=[TB, TCOL, prm("row","Row","number",0,min=0,max=255)], pure=True, keywords="lookup data table cell")
node("table.for_each", "For Each Row", "Data tables", "Runs Each once for every row of the table, in order, with that row's cells; then Then.", inputs=[X], outputs=[E("each","Each"), P("row","number","Row"), P("key","any","Key"), E("then","Then")], params=[TB], keywords="loop data table rows")
node("table.find", "Find Rows", "Data tables", "The rows (as row numbers) where a column has this value, in order. Use Get Cell on each.", inputs=[P("value","any","Value",required=True)], outputs=[P("out","any","Rows")], params=[TB, TCOL], pure=True, keywords="filter where search data table")
node("table.count", "Row Count", "Data tables", "How many rows the table has.", outputs=[P("out","number","Rows")], params=[TB], pure=True, keywords="length size data table")
node("table.keys", "Row Keys", "Data tables", "The table's keys (its key column), in row order.", outputs=[P("out","any","Keys")], params=[TB], pure=True, keywords="data table keys list")
node("timeline.play", "Timeline", "Timers", "Plays a Timeline (My mode, Timelines): values that change over time (a number, a position, a color) and events at set moments. Update runs every tick while it plays; Finished at the end. In a Function with a Player input, each player has their own. Double-click to edit its tracks.",
     inputs=[E("exec","Play"), E("play_start","Play from Start"), E("stop","Stop"), E("reverse","Reverse"), E("reverse_end","Reverse from End"), E("set_time","Set New Time"), P("new_time","number","New Time")],
     outputs=[E("update","Update"), E("finished","Finished"), P("forward","bool","Forward"), P("time","number","Time")], params=[prm("timeline","Timeline","tline","")], keywords="animation curve keyframe tween interpolate move platform")
node("timer.clear", "Clear Timer", "Timers", "Stops a timer (a Delay, Repeat, Countdown or Set Timer's handle).", inputs=[X, P("handle","timer","Timer",required=True)], outputs=[E()])
node("timer.is_active", "Is Timer Active", "Timers", "Is the timer waiting to run?", inputs=[P("handle","timer","Timer",required=True)], outputs=[P("out","bool","Active")], pure=True)
node("debug.print", "Print String", "Messages", "Prints text (and a value wired in) to the server console and the Studio's output, and to chat if you like.", inputs=[X, P("value","any","Value",optional=True)], outputs=[E()],
     params=[prm("text","Text","text","Hello",max=100), prm("chat","Also in chat","bool",True)])
# conditions
node("logic.equal", "Equal", "Conditions", "Are the two values the same (or different)?", inputs=[P("a","$T","A",required=True), P("b","$T","B",required=True)], outputs=[P("out","bool","Result")],
     params=[OF(), prm("op","Is","choice","eq",choices=[["eq","equal"],["ne","not equal"]])], pure=True)
node("logic.select", "Select", "Conditions", "Picks A when the condition is true, else B.", inputs=[P("cond","bool","Pick A",param="cond"), P("a","$T","A",optional=True), P("b","$T","B",optional=True)], outputs=[P("out","$T","Result")],
     params=[OF(), prm("cond","Pick A","bool",True)], pure=True)
node("cond.is_valid", "Is Valid", "Conditions", "Is the entity there? Any entity handle works: a player or bot, a prop, creature, NPC, interactable, component, spawned entity or weapon (a spawn node's Entity), a map entity. A removed one isn't valid.", inputs=[P("in","entity","Object",optional=True)], outputs=[P("out","bool","Valid")], pure=True)
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
node("ent.transform", "Get Entity Transform", "Entities", "Any entity's place and motion: a player, a creature, a prop, a map entity. Position (a brush entity's middle when it has no origin), Rotation (pitch, yaw, roll; a player's view), Velocity, its box (Mins, Maxs, Center) and whether it stands on the ground.", inputs=[P("in","entity","Entity",required=True)],
     outputs=[P("pos","vector","Position"), P("angles","vector","Rotation"), P("vel","vector","Velocity"), P("mins","vector","Mins"), P("maxs","vector","Maxs"), P("center","vector","Center"), P("ground","bool","On Ground")], pure=True, keywords="location origin where angles speed bounds box")
node("ent.position", "Get Entity Position", "Entities", "Where an entity is (a player, a creature, a prop; a brush entity's middle).", inputs=[P("in","entity","Entity",required=True)], outputs=[P("out","vector","Position")], pure=True, keywords="location origin where")
node("ent.rotation", "Get Entity Rotation", "Entities", "Which way an entity faces: pitch, yaw, roll (a player's view).", inputs=[P("in","entity","Entity",required=True)], outputs=[P("out","vector","Rotation")], pure=True, keywords="angles facing yaw")
node("ent.distance", "Distance Between", "Entities", "How far apart two things are: each an entity (a player, a creature...) or a point.", inputs=[P("a","entity","A",optional=True), P("a_pos","vector","Or point A",optional=True), P("b","entity","B",optional=True), P("b_pos","vector","Or point B",optional=True)], outputs=[P("out","number","Distance")], pure=True, keywords="range how far close near")
node("ent.direction", "Direction To", "Entities", "The direction (length 1) from one thing to another: each an entity or a point.", inputs=[P("a","entity","From",optional=True), P("a_pos","vector","Or from point",optional=True), P("b","entity","To",optional=True), P("b_pos","vector","Or to point",optional=True)], outputs=[P("out","vector","Direction")], pure=True, keywords="aim towards heading")
node("vec.entity", "Position Of Entity", "Vectors", "Where an entity is (the middle of a brush entity).", inputs=[P("in","entity","Entity",required=True)], outputs=[P("out","vector","Position")], pure=True)
node("player.set_position", "Set Position", "Players", "Moves a living player to a position (pick one where they fit).", inputs=[X, P("player","player","Player",required=True), P("at","vector","Position",required=True)], outputs=[E()])
node("player.set_velocity", "Set Velocity", "Players", "Throws a living player: their speed and direction as a vector.", inputs=[X, P("player","player","Player",required=True), P("v","vector","Velocity",required=True)], outputs=[E()])
# structs and enums
node("struct.make", "Make Struct", "Structs", "A struct from its fields. Each field is one value (a number, yes/no, text, a vector, a color, a file (model, sprite, sound or decal), a player, an entity or an entity handle (NPC, creature, projectile...), a team, a timer, a sound loop or an enum); for a list, an Array of the struct.", inputs=[], outputs=[P("out","$S","Struct")], params=[prm("struct","Struct","struct","")], pure=True)
node("struct.break", "Break Struct", "Structs", "A struct's fields, each one value (a number, text, a color, a file, an entity or handle, an enum...).", inputs=[P("in","$S","Struct",required=True)], outputs=[], params=[prm("struct","Struct","struct","")], pure=True)
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

# Personality influence (W2's protocol, ls_mode_ver 2): how much a bot's own personality and skill decide whether,
# when and how it follows an order (0 exactly as told, 50 as before, 100 its personality decides)
INFLUENCE_HELP = " Personality influence: 0 they obey exactly; 50 as usual; 100 each bot's personality and skill decide (a careful one may turn a risky job down, a slow one starts later)."
for d in nodes:
    if d["type"] in ("bots.goal", "bots.offer", "bots.phase", "bots.layer", "bot.follow"):
        d["inputs"].append(P("influence","number","Personality influence",param="influence"))
        d["params"].append(prm("influence","Personality influence (0-100)","number",50,min=0,max=100))
        d["help"] += INFLUENCE_HELP
        d["keywords"] = (d.get("keywords", "") + " personality obey skill").strip()

# ---- HUD & View: engine nodes (Y). Kept together, after the others: X edits the rest of this file.
PLO = P("player","player","Player (empty: everyone)",optional=True)
PLR = P("player","player","Player",required=True)
CD_TRACKS = [["2","2: Adrenaline Horror"],["3","3: Vague Voices"],["4","4: Klaxon Beat"],["5","5: Space Ocean"],["6","6: Cavern Ambiance"],["7","7: Apprehensive Short"],["8","8: Bass String Short"],["9","9: Hurricane Strings"],["10","10: Diabolical Adrenaline Guitar"],["11","11: Valve Theme (long)"],["12","12: Nepal Monastery"],["13","13: Alien Shock"],["14","14: Sirens in the Distance"],["15","15: Nuclear Mission Jam"],["16","16: Scared Confusion Short"],["17","17: Drums and Riffs"],["18","18: Hard Technology Rock"],["19","19: Steam in the Pipes"],["20","20: Electric Guitar Ambience"],["21","21: Dimensionless Deepness"],["22","22: Military Precision"],["23","23: Jungle Drums"],["24","24: Traveling Through Limbo"],["25","25: Credits / Closing Theme"],["26","26: Threatening Short"],["27","27: Dark Piano Short"],["28","28: Sharp Fear Short"]]
node("eng.hide_hud", "Hide HUD Parts", "HUD & View", "Hides parts of a player's HUD (or everyone's): the crosshair, money, the round timer, the radar with health and armor (one switch in the game), the weapon and ammo panel, the flashlight icon. Nothing ticked shows it all again. It stays through the game redrawing the HUD; Back to normal on death/respawn ends it there.",
     inputs=[X, PLO], outputs=[E()], params=[prm("crosshair","Crosshair","bool",True), prm("money","Money","bool",False), prm("timer","Round timer","bool",False), prm("radar","Radar, health and armor","bool",False), prm("weapons","Weapon and ammo panel","bool",False), prm("flashlight","Flashlight icon","bool",False), prm("reset","Back to normal on death/respawn","bool",True)],
     keywords="hud crosshair money timer radar hide interface hideweapon")
node("eng.fov", "Set Field of View", "HUD & View", "How wide a player sees (or everyone): 90 is normal, less zooms in, more is a fish-eye. A sniper scope still zooms and comes back to this. Back to normal on death/respawn puts 90 back there.",
     inputs=[X, PLO, P("fov","number","Degrees",param="fov")], outputs=[E()], params=[prm("fov","Degrees","number",90,min=10,max=170), prm("reset","Back to normal on death/respawn","bool",True)], keywords="fov zoom field of view setfov")
node("eng.view_model", "Set Weapon View Model", "HUD & View", "The model a player sees in their own hands (models/v_...mdl) for the weapon in hand or a chosen weapon, kept whenever they take it out again; also, if set, the model others see them hold (models/p_...mdl). Back to normal model puts the game's own back. Your own models are loaded with the map and go with the mode when you publish it.",
     inputs=[X, PLR], outputs=[E()], params=[prm("weapon","Weapon","choice","current",choices=[["current","The one in hand"],["knife","Knife"]]+WEAPONS), prm("model","View model","path","models/v_knife.mdl"), prm("pmodel","Model others see (empty: unchanged)","path",""), prm("restore","Back to normal model","bool",False)],
     keywords="viewmodel weapon model skin v_ p_ hands")
node("eng.flashlight", "Flashlight", "HUD & View", "Turns a player's flashlight on or off, lets them use it or not (F), and sets its battery (0 to 100; -1 leaves it).",
     inputs=[X, PLR], outputs=[E()], params=[prm("do","Do","choice","on",choices=[["on","Turn it on"],["off","Turn it off"],["allow","Let them use it"],["deny","Don't let them use it"]]), prm("battery","Battery","number",-1,min=-1,max=100)], keywords="flashlight torch light battery impulse 100")
node("eng.nightvision", "Night Vision", "HUD & View", "Gives a player night vision goggles or takes them, or turns them on or off (giving them first if needed). The same in Counter-Strike and Condition Zero.",
     inputs=[X, PLR], outputs=[E()], params=[prm("do","Do","choice","give",choices=[["give","Give goggles"],["take","Take goggles"],["on","Turn on"],["off","Turn off"]])], keywords="nightvision nvg goggles")
node("eng.footsteps", "Silent Footsteps", "HUD & View", "A player's footsteps make no sound (on) or sound again (off). It lasts until turned off or they leave.",
     inputs=[X, PLR], outputs=[E()], params=[prm("on","Silent","bool",True)], keywords="footsteps silent quiet steps sound")
node("eng.score_tag", "Scoreboard Tag", "HUD & View", "Shows VIP, Bomb or Dead next to a player's name on everyone's scoreboard (or none: the game's own again). Only the tag: they aren't really the VIP, carrying the bomb or dead. Clear on death/respawn ends it there.",
     inputs=[X, PLR], outputs=[E()], params=[prm("tag","Tag","choice","vip",choices=[["none","None (the game's own)"],["vip","VIP"],["bomb","Bomb"],["dead","Dead"]]), prm("reset","Clear on death/respawn","bool",False)], keywords="scoreboard tag vip bomb dead scoreattrib")
node("eng.score_color", "Scoreboard Team Color", "HUD & View", "Shows a player as a Terrorist, Counter-Terrorist or spectator in everyone's game (name color, scoreboard section) without moving them: their real team, spawns and rules stay. Real team undoes it. Side effects: the radar and name tags follow what's shown, and the player's own buy menu and team HUD do too.",
     inputs=[X, PLR], outputs=[E()], params=[prm("team","Show as","choice","t",choices=[["t","Terrorist"],["ct","Counter-Terrorist"],["spec","Spectator"],["real","Real team"]])], keywords="scoreboard team color teaminfo disguise")
node("eng.spectate", "Force Spectate", "HUD & View", "Makes a dead player or a spectator watch Target through their eyes, and keeps them on Target until Release (or Target leaves). A living player is refused.",
     inputs=[X, P("player","player","Watcher",required=True), P("target","player","Target",optional=True)], outputs=[E()], params=[prm("do","Do","choice","watch",choices=[["watch","Watch Target"],["release","Release"]])], keywords="spectate observer watch camera follow")
node("eng.soundtrack", "Play Soundtrack", "HUD & View", "Plays a Half-Life soundtrack track for everyone or one player (the game's own music files, as a map's trigger_cdaudio does), looping if asked; or stops it. Players hear it only if their game has the soundtrack (Steam's Half-Life files do).",
     inputs=[X, PLO], outputs=[E()], params=[prm("do","Do","choice","play",choices=[["play","Play"],["stop","Stop"]]), prm("track","Track","choice","2",choices=CD_TRACKS), prm("loop","Loop","bool",False)], keywords="music soundtrack cd audio track mp3 cdaudio")

# ---- P5: player input, cooldowns, pickups, weapon tuning, saving and leaderboards (gen/p5.rs).
# Kept together, here: X, Y, W1 and W2 edit the rest of this file.
BUTTONS = [["attack","Attack (fire)"],["attack2","Attack 2 (right click)"],["use","Use (E)"],["jump","Jump"],["duck","Duck"],["reload","Reload"],["walk","Walk (Shift)"],["forward","Forward"],["back","Back"],["left","Move left"],["right","Move right"],["any","Any of these"]]
BUTTON1 = [b for b in BUTTONS if b[0] != "any"]
CDSCOPE = [["player","Each player"],["team","Each team"],["global","Everyone (one for all)"]]
node("in.pressed", "Player Pressed Button", "Events", "Runs when a player presses (or lets go of) a button the game knows: fire, right click, use, jump, duck, reload, walk or a move key. It arrives with the player's ping, like everything they do. Only: wire a player (from a variable) to listen to one player only. Include bots: bots press buttons too.",
     inputs=[P("only","player","Only this player",optional=True)], outputs=[E(), P("player","player","Player"), P("button","text","Button")],
     params=[prm("button","Button","choice","use",choices=BUTTONS), prm("when","When","choice","pressed",choices=[["pressed","Pressed"],["released","Released"]]), prm("bots","Include bots","bool",True)], keywords="key input press button bind attack jump use reload duck walk", **ev)
node("in.holding", "Is Holding Button", "Input", "Is the player holding this button down right now? (What reached the server: it trails their screen by their ping.)", inputs=[PL], outputs=[P("out","bool","Holding")], params=[prm("button","Button","choice","attack",choices=BUTTON1)], keywords="key input held down pressing", pure=True)
node("in.command", "Player Command", "Events", "Runs when a player uses a console command the mode names (ls_ and letters, like ls_dash): they can bind any key to it (bind f \"ls_dash\"). Arguments is the rest of the line, as text only: it's never run. Each player can use it at most a few times a second (Player Command uses a second, in the mode's Limits settings).",
     outputs=[E(), P("player","player","Player"), P("args","text","Arguments")], params=[prm("command","Command (ls_...)","name","ls_dash")], keywords="key bind console command keybind hotkey ability", **ev)
node("in.tell_bind", "Tell Player To Bind", "Input", "Shows how to bind a key to a Player Command: the line to type in the console (bind f \"ls_dash\"). Nothing is bound for them: the game never lets a server change a player's keys.",
     inputs=[X, P("player","player","Player (empty: everyone)",optional=True)], outputs=[E()], params=[prm("key","Key","name","f"), prm("command","Command (ls_...)","name","ls_dash")], keywords="bind key help console")
node("cd.start", "Start Cooldown", "Cooldowns", "Starts a cooldown by name (like dash) for Seconds: for each player, each team or everyone. Is Ready says no until it's over.",
     inputs=[X, P("player","player","Player",optional=True), P("team","team","Team",optional=True), P("secs","number","Seconds",param="secs")], outputs=[E()],
     params=[prm("name","Ability","name","dash"), prm("scope","For","choice","player",choices=CDSCOPE), prm("secs","Seconds","number",5,min=0,max=86400)], keywords="cooldown timer ability wait recharge")
node("cd.ready", "Is Ready", "Cooldowns", "Is this ability's cooldown over (or never started)?", inputs=[P("player","player","Player",optional=True), P("team","team","Team",optional=True)], outputs=[P("out","bool","Ready")],
     params=[prm("name","Ability","name","dash"), prm("scope","For","choice","player",choices=CDSCOPE)], keywords="cooldown ready available", pure=True)
node("cd.left", "Cooldown Time Left", "Cooldowns", "Seconds until this ability's cooldown is over (0 when ready).", inputs=[P("player","player","Player",optional=True), P("team","team","Team",optional=True)], outputs=[P("out","number","Seconds")],
     params=[prm("name","Ability","name","dash"), prm("scope","For","choice","player",choices=CDSCOPE)], keywords="cooldown remaining left", pure=True)
node("cd.reset", "Reset Cooldown", "Cooldowns", "Ends a cooldown now (the ability is ready again).", inputs=[X, P("player","player","Player",optional=True), P("team","team","Team",optional=True)], outputs=[E()],
     params=[prm("name","Ability","name","dash"), prm("scope","For","choice","player",choices=CDSCOPE)], keywords="cooldown reset clear refresh")
node("item.picked_up", "Picked Up", "Events", "Runs when a player picks a weapon or an item up from the ground (not when they buy one or the mode gives it). Item is its short name: ak47, kevlar, hegrenade...",
     outputs=[E(), P("player","player","Player"), P("item","text","Item")], keywords="pickup pick up take weapon ground loot", **ev)
node("item.dropped", "Dropped", "Events", "Runs when a player drops a weapon (G, or the mode makes them). On death: also when a dying player's weapon falls.",
     outputs=[E(), P("player","player","Player"), P("weapon","text","Weapon")], params=[prm("death","Also on death","bool",False)], keywords="drop throw weapon", **ev)
node("wpn.infinite_ammo", "Infinite Ammo", "Weapons", "Keeps a player's ammo full: the clip (no reloading), the spare ammo, or both. Reset on death: back to normal when they die.",
     inputs=[X, PL], outputs=[E()], params=[prm("on","On","bool",True), prm("what","Keep full","choice","both",choices=[["clip","The clip"],["reserve","Spare ammo"],["both","Both"]]), prm("reset","Reset on death","bool",True)], keywords="ammo unlimited bullets clip reload")
node("wpn.fire_rate", "Fire Rate", "Weapons", "Makes a player's guns fire faster (2 = twice as fast) or slower (0.5 = half). 1 is normal. Reset on death: back to 1 when they die.",
     inputs=[X, PL, P("mult","number","Multiplier",param="mult")], outputs=[E()], params=[prm("mult","Multiplier","number",2,min=0.1,max=10), prm("reset","Reset on death","bool",True)], keywords="rate of fire speed shoot faster rapid")
node("wpn.recoil", "Recoil", "Weapons", "Scales how much a player's view kicks when they fire: 0 none, 0.5 half, 1 normal, 2 double. Reset on death: back to 1 when they die.",
     inputs=[X, PL, P("mult","number","Multiplier",param="mult")], outputs=[E()], params=[prm("mult","Multiplier","number",0.5,min=0,max=5), prm("reset","Reset on death","bool",True)], keywords="recoil kick punch spray accuracy")
node("save.text_get", "Saved Text", "Saved", "Text kept for this player across maps, restarts and crashes (empty until set). Each mode has its own file.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","text","Text")], params=[prm("key","Name","name","title")], keywords="save load text string persistent", pure=True)
node("save.text_set", "Save Text", "Saved", "Keeps text for this player across maps and restarts (up to 63 letters).", inputs=[X, P("player","player","Player",required=True), P("text","text","Text",param="text")], outputs=[E()], params=[prm("key","Name","name","title"), prm("text","Text","text","",max=63)], keywords="save text string persistent")
node("save.server_get", "Server Value", "Saved", "A value kept for the whole server (not per player) across maps and restarts: a number and a text (each 0 or empty until set).", outputs=[P("out","number","Number"), P("text","text","Text")], params=[prm("key","Name","name","record")], keywords="server global saved world record persistent", pure=True)
node("save.server_set", "Set Server Value", "Saved", "Keeps a number or a text for the whole server across maps and restarts.", inputs=[X, P("value","number","Number",param="value"), P("text","text","Text",optional=True)], outputs=[E()],
     params=[prm("key","Name","name","record"), prm("what","Keep","choice","number",choices=[["number","The number"],["text","The text"]]), prm("value","Number","number",0,min=-100000000,max=100000000)], keywords="server global save record persistent")
node("save.top", "Top Players", "Saved", "The best players by a saved value (Saved Value's name), highest first or lowest first, offline players too (the save file keeps their names): Each runs once per place with Rank, Name and Value, then Then. How many places: Top Players places, in the mode's Limits settings (10 to start).",
     inputs=[X], outputs=[E("each","Each"), P("rank","number","Rank"), P("name","text","Name"), P("value","number","Value"), E("then","Then")],
     params=[prm("key","Saved value","name","wins"), prm("order","Order","choice","high",choices=[["high","Highest first"],["low","Lowest first"]])], keywords="leaderboard top best ranking scores highscore")

# model points (Y): bones and attachments of an entity's model, read on the server
node("ent.bone_pos", "Get Bone Position", "Entities", "Where a bone of a player's or any model entity's skeleton is now (like Bip01 R Hand or Bip01 Head), and which way it points. Model is only for the list of bones; the bone is looked up by name in the model the entity really has, and Found is No when it has no such bone. Positions follow the server's animation: close, not pixel-exact.",
     inputs=[P("ent","entity","Entity",required=True)], outputs=[P("pos","vector","Position"), P("angles","vector","Angles"), P("found","bool","Found")], params=[prm("model","Model (for the list)","path","models/player.mdl"), prm("bone","Bone","bone","Bip01 R Hand")], pure=True, keywords="bone skeleton hand head position bonepos")
node("ent.attachment_pos", "Get Attachment Position", "Entities", "Where an attachment point of an entity's model is now (a gun's muzzle, a hand; models number them 0 to 3), and its angles. Model is only for the list; Found is No when the entity's real model has no such attachment. Positions follow the server's animation: close, not pixel-exact.",
     inputs=[P("ent","entity","Entity",required=True)], outputs=[P("pos","vector","Position"), P("angles","vector","Angles"), P("found","bool","Found")], params=[prm("model","Model (for the list)","path","models/player.mdl"), prm("attachment","Attachment","attachment",0)], pure=True, keywords="attachment muzzle point position")

# ---- Collision, overlap and hits (W2). Kept together, before the registry is written.
ENTR = P("ent","entity","Entity",required=True)
VOLFILTER = [["players","Players"],["creatures","Creatures"],["any","Anything"]]
node("col.semiclip", "Players Pass Through Each Other", "Collision", "Lets players walk through each other (semiclip): nobody, everyone, teammates only, or chosen players (each wired player set on or off; a chosen player passes through everyone). Bullets still hit. Turning it off nudges anyone left inside someone else free. Bots path as usual.",
     inputs=[X, P("player","player","Player (chosen players)",optional=True)], outputs=[E()], params=[prm("mode","Who","choice","all",choices=[["off","Nobody (off)"],["all","Everyone"],["team","Teammates only"],["chosen","Chosen players"]]), prm("on","This player passes through (chosen players)","bool",True)], keywords="semiclip pass through collide collision players solid ghost noclip block")
node("col.set_solid", "Set Entity Collision", "Collision", "How a prop, an NPC or a spawned thing collides: solid (blocks and gets hit), trigger (things pass through, it still feels touches) or pass-through (nothing). Not for players (Players Pass Through Each Other is for them).",
     inputs=[X, ENTR], outputs=[E(), P("ok","bool","Done")], params=[prm("kind","Collision","choice","solid",choices=[["solid","Solid"],["trigger","Trigger"],["none","Pass-through"]])], keywords="solid collision trigger passthrough nonsolid block")
node("col.set_size", "Set Entity Size", "Collision", "The box an entity takes up, as mins and maxs from its origin (each min below its max). Not for players: their box is the game's.",
     inputs=[X, ENTR], outputs=[E(), P("ok","bool","Done")], params=[prm("minx","Min X","number",-16,min=-4096,max=4096), prm("miny","Min Y","number",-16,min=-4096,max=4096), prm("minz","Min Z","number",0,min=-4096,max=4096), prm("maxx","Max X","number",16,min=-4096,max=4096), prm("maxy","Max Y","number",16,min=-4096,max=4096), prm("maxz","Max Z","number",32,min=-4096,max=4096)], keywords="size bounds box mins maxs hitbox collision")
node("col.takes_damage", "Entity Takes Damage", "Collision", "Whether a prop or an NPC can be hurt (on) or not (off). Not for players (Set God Mode is theirs).",
     inputs=[X, ENTR], outputs=[E(), P("ok","bool","Done")], params=[prm("on","Takes damage","bool",True)], keywords="damage invulnerable takedamage breakable god")
node("col.health", "Entity Health", "Collision", "An entity's health (a prop, an NPC, a creature, a player).", inputs=[ENTR], outputs=[P("out","number","Health")], keywords="health hp", pure=True)
node("col.set_health", "Set Entity Health", "Collision", "Sets a prop's or an NPC's health (at least 1; to destroy it use its own damage or removal). Not for players (Set Health is theirs).",
     inputs=[X, ENTR, P("hp","number","Health",param="hp")], outputs=[E(), P("ok","bool","Done")], params=[prm("hp","Health","number",100,min=1,max=1000000)], keywords="health hp set")
node("vol.spawn", "Spawn Trigger Volume", "Collision", "An invisible box or sphere that tells you who goes in and out (Begin Overlap, End Overlap). Where: a Position, or At entity plus a Local offset (X forward, Y right, Z up by its facing) - attached, it follows the entity and turns with it. Rotation (pitch, yaw, roll) turns a box; a sphere ignores it. Size is its own setting; Velocity and Scale don't apply (a volume doesn't move by itself). Who counts: players, creatures or anything. Checked ten times a second (one search round each volume). A name lets Player Touched and the overlap events pick it out. Then runs right away whatever happened; Spawned or Failed says which, and Failure reason why (Out of entity slots, Limit reached: Trigger volumes at once in Limits, or Bad input: At entity is gone and there's no Position). It isn't solid, so it never checks the spot.",
     inputs=[X, P("pos","vector","Position"), P("angles","vector","Rotation",optional=True), P("at_ent","entity","At entity",optional=True), P("offset","vector","Local offset",optional=True)], outputs=[E(), E("spawned","Spawned"), E("failed","Failed"), P("out","entity","Entity"), P("ok","bool","Success"), P("reason","enum:ls_spawn_fail","Failure reason")],
     params=[prm("shape","Shape","choice","box",choices=[["box","Box"],["sphere","Sphere"]]), prm("sx","Width (box)","number",128,min=8,max=8192), prm("sy","Depth (box)","number",128,min=8,max=8192), prm("sz","Height (box)","number",96,min=8,max=8192), prm("radius","Radius (sphere)","number",96,min=8,max=4096), prm("filter","Who counts","choice","players",choices=VOLFILTER), prm("name","Name","name","")], keywords="trigger volume overlap zone area box sphere region")
node("vol.remove", "Remove Trigger Volume", "Collision", "Removes a trigger volume (whoever was inside gets End Overlap).", inputs=[X, P("volume","entity","Volume",required=True)], outputs=[E()], keywords="trigger volume remove delete")
node("vol.is_overlapping", "Is Overlapping", "Collision", "Is this entity (a player, a creature...) inside the trigger volume right now (and one its filter counts)?", inputs=[P("volume","entity","Volume",required=True), ENTR], outputs=[P("out","bool","Overlapping")], keywords="trigger volume overlap inside", pure=True)
node("vol.on_begin", "Begin Overlap", "Events", "Runs when something its filter counts goes into a trigger volume (Spawn Trigger Volume). Name: only volumes with this name (empty: any).", outputs=[E(), P("volume","entity","Volume"), P("who","entity","Who")], params=[prm("name","Volume name","name","")], **ev)
node("vol.on_end", "End Overlap", "Events", "Runs when something leaves a trigger volume (or the volume is removed, or it died or left the server inside). Name: only volumes with this name (empty: any).", outputs=[E(), P("volume","entity","Volume"), P("who","entity","Who")], params=[prm("name","Volume name","name","")], **ev)
node("col.on_touch", "Entity Touched", "Events", "Runs when an entity touches another (props, projectiles, NPCs, creatures, players; not the world's walls and floors). Name: only an entity with this name (empty: any). The first touch per N seconds for each entity (players standing on a prop would touch it every frame).", outputs=[E(), P("entity","entity","Entity"), P("other","entity","Other")], params=[prm("name","Entity name","name",""), prm("secs","Only the first touch per (seconds)","number",1,min=0.1,max=60)], **ev)
node("col.on_bullet", "Bullet Hit", "Events", "Runs where a shot lands: the shooter, the hit position, the surface's normal, what it hit (an entity, or World), the weapon and, on a player, where (head, chest...). At most once per shooter every N seconds (automatic weapons and shotguns would flood it).", outputs=[E(), P("shooter","player","Shooter"), P("pos","vector","Hit Position"), P("normal","vector","Normal"), P("hit","entity","Hit Entity"), P("world","bool","Hit World"), P("weapon","text","Weapon"), P("hitgroup","text","Hit where")], params=[prm("secs","At most once per shooter every (seconds)","number",0.1,min=0.05,max=10)], **ev)
node("bots.avoid", "Bots Avoid Area", "Bots", "Marks a place bots keep away from: a Position with a radius, or At entity (it rides along: a carrier, a burning player, a trigger volume; it ends when that's gone). Strength 1-10: 5 and up they won't stand in it, below they only path round it. Seconds: 0 till Bots Stop Avoiding. Which bots: every bot, or one team's (the carrier's own team can be left out). The same tag again moves or changes it. The bot brain keeps 32 at once.",
     inputs=[X, P("pos","vector","Position"), P("at_ent","entity","At entity",optional=True)], outputs=[E()], params=[prm("tag","Tag","name","danger"), prm("radius","Radius","number",160,min=16,max=4096), prm("strength","Strength","number",6,min=1,max=10), prm("secs","Seconds (0: till stopped)","number",0,min=0,max=3600), prm("team","Which bots","choice","all",choices=[["all","Every bot"],["t","Terrorist bots"],["ct","Counter-Terrorist bots"]])], needs="components", keywords="avoid danger area bots path keep away hot potato fire")
node("bots.avoid_stop", "Bots Stop Avoiding", "Bots", "Bots stop avoiding the place with this tag (Bots Avoid Area).", inputs=[X], outputs=[E()], params=[prm("tag","Tag","name","danger")], needs="components", keywords="avoid danger area bots stop")
node("col.on_landed", "Player Landed", "Events", "Runs when a player lands after a fall: how fast they were falling (units a second; over 580 hurts) and the fall damage the game gives for it (0 if Fall Damage Off). Small hops don't count.", outputs=[E(), P("player","player","Player"), P("speed","number","Fall speed"), P("damage","number","Damage")], params=[prm("min","Only falls faster than","number",250,min=0,max=2000)], **ev)

# ==== EC: engine coverage nodes (the approved gaps 1-8, 10-15, and NPC moves). Kept together; codegen in gen/coverage.rs ====
EC_PL = P("player","player","Player",required=True)
EC_PLO = P("player","player","Player (empty: everyone)",optional=True)
EC_ENT = P("ent","entity","Entity",required=True)
EC_RGB = [prm("r","Red","number",255,min=0,max=255), prm("g","Green","number",255,min=0,max=255), prm("b","Blue","number",255,min=0,max=255)]
EC_FIELDS = [["rendermode","Render mode"],["renderfx","Render effect"],["color","Color"],["amount","Render amount (0-255)"],["skin","Skin"],["body","Body (submodel)"],["scale","Scale (sprites, models)"],["gravity","Gravity"],["movetype","Movement"],["solid","Collision"],["health","Health"],["framerate","Animation speed"]]
EC_PICKS = [["normal","Render mode: Normal"],["color","Render mode: Color"],["texture","Render mode: Texture (see-through)"],["glow","Render mode: Glow (sprites)"],["solid","Render mode: Solid (cut-out)"],["additive","Render mode: Additive (bright, see-through)"],
            ["fx_none","Effect: None"],["fx_pulse","Effect: Pulse"],["fx_pulsefast","Effect: Fast pulse"],["fx_fadeslow","Effect: Fade out slowly"],["fx_strobe","Effect: Strobe"],["fx_flicker","Effect: Flicker"],["fx_hologram","Effect: Hologram"],["fx_glowshell","Effect: Glow shell"],["fx_distort","Effect: Distort"],
            ["mv_none","Movement: None (stays put)"],["mv_toss","Movement: Falls"],["mv_bounce","Movement: Falls and bounces"],["mv_fly","Movement: Flies (no gravity)"],["mv_flymissile","Movement: Flies, hits things"],["mv_noclip","Movement: Through walls"],["mv_step","Movement: Steps on the floor"],
            ["so_not","Collision: Not solid"],["so_trigger","Collision: Touch only"],["so_box","Collision: Solid"]]
EC_KINDS = [["generic","Generic"],["bullet","Bullet"],["slash","Slash (knife)"],["burn","Burn"],["blast","Blast"],["fall","Fall"],["shock","Shock"],["crush","Crush"],["poison","Poison"],["freeze","Freeze"]]
EC_ZONES = [["generic","Anywhere (generic)"],["head","Head"],["chest","Chest"],["stomach","Stomach"],["leftarm","Left arm"],["rightarm","Right arm"],["leftleg","Left leg"],["rightleg","Right leg"]]
EC_ZONE_KIND = [["buy","Buy zone"],["bomb","Bomb site"],["rescue","Hostage rescue zone"],["escape","Escape zone"],["vip","VIP safety zone"]]
EC_GREN = [["any","Any grenade"],["he","HE grenade"],["flash","Flashbang"],["smoke","Smoke grenade"]]
EC_RADIO = [["go","Go go go!"],["fallback","Team, fall back!"],["sticktog","Stick together, team."],["getinpos","Get in position and wait."],["stormfront","Storm the front!"],["reportin","Report in, team."],["affirm","Affirmative / Roger that."],["roger","Roger that."],["enemyspot","Enemy spotted."],["backup","Need backup."],["clear","Sector clear."],["inpos","I'm in position."],["reportingin","Reporting in."],["blow","Get out of there, it's gonna blow!"],["negative","Negative."],["enemydown","Enemy down."],["coverme","Cover me!"],["takepoint","You take the point."],["position","Hold this position."],["regroup","Regroup, team."],["followme","Follow me."],["takingfire","Taking fire, need assistance!"],["fireinhole","Fire in the hole!"],
            ["a_terwin","Announcer: Terrorists win!"],["a_ctwin","Announcer: Counter-Terrorists win!"],["a_draw","Announcer: Round draw."],["a_bombpl","Announcer: The bomb has been planted."],["a_bombdef","Announcer: The bomb has been defused."],["a_rescued","Announcer: All hostages have been rescued."],["a_letsgo","Announcer: Let's go! (round start)"],["a_locknload","Announcer: Lock and load."],["a_moveout","Announcer: Move out."]]
ECT = dict(pure=True)
# 1. damage
node("damage.deal", "Deal Damage", "Damage", "Hurts a player or any entity that can be hurt (a breakable, an NPC, a creature) by Amount, as if Attacker did it: kills, the kill feed, scores and Player Hurt / Player Killed count as usual. No Attacker: the world did it. Kind is what it counts as (a burn, a fall...); Hit zone is where (a head hit shows as a headshot in the kill feed; the amount isn't multiplied). Armor takes its share unless Ignore armor.",
     inputs=[X, P("target","entity","Target",required=True), P("attacker","player","Attacker",optional=True), P("amount","number","Amount",param="amount")], outputs=[E()],
     params=[prm("amount","Amount","number",25,min=0,max=10000), prm("kind","Kind","choice","generic",choices=EC_KINDS), prm("zone","Hit zone","choice","generic",choices=EC_ZONES), prm("armor","Ignore armor","bool",False)],
     keywords="hurt damage harm takedamage attack kill credit")
# 2. entity properties
node("ent.prop_get", "Get Entity Property", "Entities", "Reads a property of any entity (a prop, an NPC, a map entity, a player): its render mode, effect, color, amount, skin, body, scale, gravity, movement, collision, health or animation speed. Value is the number the game keeps (render modes and the like are numbered as in the map editor); Color is red, green, blue.",
     inputs=[EC_ENT], outputs=[P("value","number","Value"), P("color","vector","Color")], params=[prm("field","Property","choice","amount",choices=EC_FIELDS)], keywords="render glow color skin body scale gravity movetype solid health pev property", **ECT)
node("ent.prop_set", "Set Entity Property", "Entities", "Changes a property of any entity: render mode, effect and color (so props and NPCs can glow or fade), amount, skin, body, scale, gravity, movement, collision, health or animation speed. Value is for the number properties; Pick is for the named ones (render mode, effect, movement, collision); Color for color. Players: render, color, skin, body, gravity and health work here; movement and collision are theirs to keep (use No Clip, Freeze).",
     inputs=[X, EC_ENT, P("value","number","Value",param="value"), P("color","vector","Color",optional=True)], outputs=[E()],
     params=[prm("field","Property","choice","renderfx",choices=EC_FIELDS), prm("value","Value","number",1,min=-10000,max=10000), prm("pick","Pick","choice","fx_glowshell",choices=EC_PICKS)] + EC_RGB,
     keywords="render glow color fade skin body scale gravity movetype solid health pev property transparency")
# 3. big HUD text, colored chat
node("msg.big_hud", "Big HUD Text", "Messages", "Big text on the screen (the director HUD: larger than HUD Text, its own slots), to one player or everyone, at a position (X and Y from 0 to 1; -1 centers), in a color, with an effect, for some seconds. A Value wired in goes after the text.",
     inputs=[X, EC_PLO, P("value","any","Value",optional=True)], outputs=[E()],
     params=[prm("text","Text","text","Round 1",max=100), prm("x","X (-1: center)","number",-1,min=-1,max=1), prm("y","Y (-1: center)","number",0.3,min=-1,max=1)] + EC_RGB + [prm("effect","Effect","choice","fade",choices=[["fade","Fade in and out"],["flicker","Flicker"],["type","Typed out"]]), prm("seconds","Seconds","number",3,min=0.5,max=30)],
     keywords="dhud director hud big text title announce center")
node("msg.color_chat", "Colored Chat", "Messages", "A chat line in color, to one player or everyone: a green Tag first (empty: none), then the text in a team color (red, blue, gray, or the color of the player wired into Color of), green, or the usual yellow. A Value wired in goes after the text; color codes in a player's name or text never get through.",
     inputs=[X, EC_PLO, P("value","any","Value",optional=True), P("of","player","Color of",optional=True)], outputs=[E()],
     params=[prm("tag","Tag","text","[Mode]",max=24), prm("text","Text","text","Hello!",max=120), prm("color","Color","choice","team",choices=[["team","Their team's color"],["red","Red (Terrorist)"],["blue","Blue (Counter-Terrorist)"],["gray","Gray"],["green","Green"],["normal","Yellow (normal)"]])],
     keywords="chat color colour green red blue team say print_color")
# 4. movement state
node("player.move_state", "Player Movement State", "Players", "What a player is doing with their feet: on the ground, ducking, in water (and how deep: 0 dry, 1 feet, 2 waist, 3 under), on a ladder, and how fast (across the ground, and up or down: falling is negative).",
     inputs=[EC_PL], outputs=[P("ground","bool","On ground"), P("duck","bool","Ducking"), P("water","bool","In water"), P("level","number","Water level"), P("ladder","bool","On ladder"), P("speed","number","Speed"), P("vspeed","number","Vertical speed")],
     keywords="ground ducking crouch water swim ladder speed velocity falling airborne flags", **ECT)
# 5. push and jump rules
node("player.push", "Push", "Players", "Pushes a player or an entity: along where they aim, where they face (flat), the Direction wired in, away from a Point, or straight up, by Strength (units a second). Add keeps their speed and adds to it; Replace sets it. Lift adds a little upward push so people on the ground aren't held by friction. Jump pads, dashes, knockback.",
     inputs=[X, P("target","entity","Player or entity",required=True), P("dir","vector","Direction",optional=True), P("point","vector","Away from point",optional=True), P("strength","number","Strength",param="strength")], outputs=[E()],
     params=[prm("along","Along","choice","aim",choices=[["aim","Where they aim"],["facing","Where they face (flat)"],["dir","The Direction"],["away","Away from the Point"],["up","Straight up"]]), prm("strength","Strength","number",500,min=0,max=3000), prm("lift","Lift","number",0,min=0,max=1000), prm("how","How","choice","add",choices=[["add","Add to their speed"],["replace","Replace their speed"]])],
     keywords="push knockback dash jump pad launch velocity impulse boost")
node("player.jump_rules", "Jump Rules", "Players", "Jumping rules for a player (or everyone): block jumping or ducking, extra jumps in the air (double jump and more), and automatic bunny hopping (hold jump to hop with no slowdown). Their own game predicts the stock moves, so a blocked jump can look like a small hop on their screen.",
     inputs=[X, EC_PLO], outputs=[E()],
     params=[prm("nojump","Block jumping","bool",False), prm("noduck","Block ducking","bool",False), prm("air","Extra air jumps","number",1,min=0,max=10), prm("power","Air jump strength","number",268,min=100,max=1000), prm("bhop","Auto bunny hop","bool",False), prm("reset_death","Back to normal on death","bool",False), prm("reset_round","Back to normal at a new round","bool",False)],
     keywords="jump double jump bhop bunnyhop duck crouch air jump block multi")
# 6. grenades
node("event.grenade_thrown", "Grenade Thrown", "Events", "Runs when a player throws a grenade: who, the grenade (an entity: Set Grenade Fuse, Push, Remove Entity work on it), which kind and where. Type: only that kind.",
     outputs=[E(), P("player","player","Thrower"), P("grenade","entity","Grenade"), P("kind","text","Kind"), P("pos","vector","Position")], params=[prm("type","Type","choice","any",choices=EC_GREN)], event=True, keywords="grenade throw he flash smoke nade")
node("event.grenade_exploded", "Grenade Exploded", "Events", "Runs when a thrown grenade goes off (a smoke grenade when it starts smoking): who threw it, the grenade, which kind and where. Type: only that kind.",
     outputs=[E(), P("player","player","Thrower"), P("grenade","entity","Grenade"), P("kind","text","Kind"), P("pos","vector","Position")], params=[prm("type","Type","choice","any",choices=EC_GREN)], event=True, keywords="grenade explode detonate blast he flash smoke nade")
node("ent.grenade_fuse", "Set Grenade Fuse", "Entities", "How many seconds until a thrown grenade goes off, from now (0: at once). Anything that isn't a thrown grenade is left alone.",
     inputs=[X, P("grenade","entity","Grenade",required=True), P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",1,min=0,max=60)], keywords="grenade fuse timer detonate dmgtime")
# 7. animation and attaching
node("ent.play_anim", "Play Animation", "Entities", "Plays an animation (a sequence of its model, by name or number) on a prop, an NPC or any model entity, at a speed (1 normal), from the start or not; Loop plays a one-time animation again every Length seconds (looping ones loop by themselves). Players, creatures and NPCs that are walking are animated by the game and will switch back.",
     inputs=[X, EC_ENT, P("number","number","Sequence number",optional=True)], outputs=[E()],
     params=[prm("sequence","Sequence name","name","idle"), prm("rate","Speed","number",1,min=0,max=10), prm("restart","From the start","bool",True), prm("loop","Loop","bool",False), prm("length","Length (seconds, for Loop)","number",2,min=0.1,max=60)],
     keywords="animation sequence animate model play anim frame")
node("ent.attach", "Attach To", "Entities", "Makes an entity (a prop, a sprite, an NPC) follow a player or another entity, keeping an offset (X forward is along the world, not their facing) or, at an attachment point of their model (0 to 3), following that point. It's let go when Detach runs, the parent is removed or dies (if asked), or the attached one is removed.",
     inputs=[X, P("ent","entity","Entity",required=True), P("to","entity","Parent",required=True), P("offset","vector","Offset",optional=True)], outputs=[E()],
     params=[prm("point","Follow","choice","origin",choices=[["origin","Its position (and turning)"],["a0","Attachment 0"],["a1","Attachment 1"],["a2","Attachment 2"],["a3","Attachment 3"]]), prm("on_death","Let go when the parent dies","bool",True)],
     keywords="attach follow parent hat carry bone attachment movetype follow aiment")
node("ent.detach", "Detach", "Entities", "Lets go of an entity attached with Attach To; it stays where it is (or falls, if Fall is on).",
     inputs=[X, P("ent","entity","Entity",required=True)], outputs=[E()], params=[prm("fall","Fall","bool",False)], keywords="detach unattach drop release unparent")
# 8. map entity fired
node("event.map_fired", "On Map Entity Fired", "Events", "Runs when a named entity of the map is triggered (by a button, a trigger, a timer or another entity) or used: its name, the entity, who set it off (a player, or nobody) and the entity that fired it. Name: only that entity (empty: any named one). Map logic drives the mode without an ls_event.",
     outputs=[E(), P("name","text","Entity name"), P("entity","entity","Entity"), P("activator","player","Activator"), P("caller","entity","Caller")], params=[prm("name","Entity name","name","")], event=True,
     keywords="map trigger fired target targetname use button door relay multi_manager io logic")
# 10. map objectives
node("game.remove_objectives", "Remove Map Objectives", "Game", "Takes the map's objectives away (tick which): buy zones, bomb sites, hostages, hostage rescue zones, VIP safety zones, escape zones. The game notices at the next round (the bomb isn't given, no hostages to save); removed buy zones don't fall back to buying at spawn. Best at Map Started.",
     inputs=[X], outputs=[E()],
     params=[prm("buy","Buy zones","bool",False), prm("bomb","Bomb sites","bool",True), prm("hostages","Hostages","bool",True), prm("rescue","Rescue zones","bool",True), prm("vip","VIP safety zones","bool",True), prm("escape","Escape zones","bool",True)],
     keywords="objectives remove bomb site hostage buyzone vip escape rescue map type deathmatch")
node("game.map_type", "Map Type", "Game", "What kind of map this is now: bomb (de_), hostage (cs_), VIP (as_), escape (es_) or other, from what it has in it; and each one as yes or no.",
     outputs=[P("type","text","Type"), P("bomb","bool","Has bomb sites"), P("hostage","bool","Has hostages"), P("vip","bool","Has VIP zone"), P("escape","bool","Has escape zone")], keywords="map type bomb hostage vip escape de cs as es objectives", **ECT)
# 11. weapon switched
node("event.weapon_switched", "Weapon Switched", "Events", "Runs when a player takes out a weapon (a switch, a pick-up that goes to hand, a spawn): the player, the new weapon and the one before (short names: ak47, knife; empty when there was none), and the weapon entity.",
     outputs=[E(), P("player","player","Player"), P("new","text","New weapon"), P("old","text","Old weapon"), P("weapon","entity","Weapon entity")], event=True, keywords="weapon switch change deploy draw holster slot")
# 12. voice
node("game.voice_rules", "Voice Rules", "Game", "Who hears whom on voice chat: everyone, only teammates, the dead only the dead and the living only the living, or only those within a distance (0: any distance, with the other rules). Spectators hear spectators. Voice Override changes it for one player; Mute Voice still mutes.",
     inputs=[X], outputs=[E()],
     params=[prm("who","Who hears","choice","team",choices=[["all","Everyone hears everyone"],["team","Teammates only"],["life","Dead with dead, alive with alive"],["team_life","Teammates, and dead with dead"],["near","Only within the distance"]]), prm("dist","Within (units, 0: any)","number",0,min=0,max=8192)],
     keywords="voice chat talk hear alltalk proximity dead team mic")
node("player.voice_override", "Voice Override", "Players", "Changes the Voice Rules for one player: everyone hears them, nobody does, they hear everyone, or back to the rules.",
     inputs=[X, EC_PL], outputs=[E()], params=[prm("how","Override","choice","heard",choices=[["heard","Everyone hears them"],["silent","Nobody hears them"],["hears","They hear everyone"],["rules","Back to the rules"]])],
     keywords="voice override mic hear talk admin")
# 13. radio
node("msg.radio", "Play Radio", "Messages", "Plays one of the game's radio lines or announcer words to a player, a team or everyone, with the radio text in chat (as From, if a player is wired in). Only the game's own sounds: nothing to load.",
     inputs=[X, P("player","player","Player",optional=True), P("from","player","From",optional=True)], outputs=[E()],
     params=[prm("line","Line","choice","go",choices=EC_RADIO), prm("to","To","choice","all",choices=[["all","Everyone (or the Player)"],["t","Terrorists"],["ct","Counter-Terrorists"]]), prm("text","Show the radio text","bool",True)],
     keywords="radio announcer sound voice line sendaudio go cover me fire in the hole")
# 14. map zones
node("player.in_zone", "In Map Zone", "Players", "Is a player in one of the map's zones now: a buy zone, a bomb site, a hostage rescue zone, an escape zone or a VIP safety zone (the map's own, as the game sees them).",
     inputs=[EC_PL], outputs=[P("out","bool","In it")], params=[prm("zone","Zone","choice","buy",choices=EC_ZONE_KIND)], keywords="zone buyzone bomb site rescue escape vip in area", **ECT)
node("event.zone_enter", "Entered Map Zone", "Events", "Runs when a living player walks into one of the map's zones (checked four times a second).",
     outputs=[E(), P("player","player","Player")], params=[prm("zone","Zone","choice","buy",choices=EC_ZONE_KIND)], event=True, keywords="zone enter buyzone bomb site rescue escape vip")
node("event.zone_leave", "Left Map Zone", "Events", "Runs when a living player walks out of one of the map's zones (checked four times a second).",
     outputs=[E(), P("player","player","Player")], params=[prm("zone","Zone","choice","buy",choices=EC_ZONE_KIND)], event=True, keywords="zone leave exit buyzone bomb site rescue escape vip")
# 15. info window
node("msg.info_window", "Show Info Window", "Messages", "Opens the game's info window (the one the server's message of the day uses) for a player, with a title and text: Text from here, then any Text wired in (from a data table or Format Text). [br] starts a new line, [b]...[b] makes words bold. Plain text only: never a web page or a link.",
     inputs=[X, EC_PL, P("more","text","Text",optional=True)], outputs=[E()],
     params=[prm("title","Title","text","Rules",max=40), prm("text","Text","text","Welcome![br]Have fun.",max=900)],
     keywords="motd info window rules help text page message of the day")
# NPC moves
node("npc.move_to", "NPC Move To", "NPCs", "Sends an NPC walking (or running) to an entity or a point now, along the map's bot navigation (straight when the map has none), facing where it goes. Arrived runs when it gets there, Failed when it can't (no way, stuck, or too long). Then it goes back to what it did, or stays there.",
     inputs=[X, P("npc","entity","NPC",required=True), P("to","entity","To entity",optional=True), P("pos","vector","Or point")], outputs=[E(), E("arrived","Arrived"), E("failed","Failed")],
     params=[prm("speed","Speed","choice","walk",choices=[["walk","Walk"],["run","Run"]]), prm("stay","Stay there after","bool",False)],
     keywords="npc move walk go to path navigate run goto", needs="components")
node("npc.follow_path", "NPC Follow Path", "NPCs", "Sends an NPC along points in order: an array of positions, or a map's path_track chain by its first name. At point runs at each one, Finished at the end (never with Loop), Failed when it can't get on. Then it goes back to what it did, or stays there.",
     inputs=[X, P("npc","entity","NPC",required=True), P("points","array<vector>","Points",optional=True)], outputs=[E(), E("point","At point"), E("finished","Finished"), E("failed","Failed")],
     params=[prm("track","Or path_track (first name)","name","path1"), prm("speed","Speed","choice","walk",choices=[["walk","Walk"],["run","Run"]]), prm("loop","Loop","bool",False), prm("stay","Stay at the end","bool",False)],
     keywords="npc path patrol route waypoints path_track follow walk", needs="components")
node("npc.stop", "NPC Stop", "NPCs", "Stops an NPC's Move To or Follow Path where it is (neither Arrived nor Failed runs); it goes back to what it did, or stays.",
     inputs=[X, P("npc","entity","NPC",required=True)], outputs=[E()], params=[prm("stay","Stay there","bool",True)], keywords="npc stop halt wait", needs="components")
# 16. round end rules
node("game.round_end_rules", "Round End Rules", "Game", "Stops rounds ending by themselves: when a team is wiped out, when the time runs out, or either (End Round is then the only way a round ends). Ending as usual undoes it; a new mode starts as usual.",
     inputs=[X], outputs=[E()], params=[prm("rule","Rounds don't end when","choice","elim",choices=[["normal","(Ending as usual)"],["elim","A team is wiped out"],["time","Time runs out"],["both","Either (only End Round ends them)"]])],
     keywords="round end infinite elimination time limit never end mp_round_infinite deathmatch")
# 19. admin flag, votes
EC_FLAGS = [["a","Immunity (a)"],["b","Reserved slot (b)"],["c","Kick (c)"],["d","Ban (d)"],["e","Slay and slap (e)"],["f","Change map (f)"],["g","Change settings (g)"],["h","Server config (h)"],["i","Admin chat (i)"],["j","Start votes (j)"],["k","Server password (k)"],["l","RCON (l)"],["m","Custom level A (m)"],["n","Custom level B (n)"],["o","Custom level C (o)"],["p","Custom level D (p)"],["q","Custom level E (q)"],["r","Custom level F (r)"],["s","Custom level G (s)"],["t","Custom level H (t)"],["u","Menus (u)"]]
node("player.has_flag", "Has Admin Flag", "Players", "Does a player have this admin flag (from the server's admin list): kick, ban, change map, a custom level and so on.",
     inputs=[EC_PL], outputs=[P("out","bool","Has it")], params=[prm("flag","Flag","choice","c",choices=EC_FLAGS)], keywords="admin flag access rights permission level", **ECT)
node("game.start_vote", "Start Vote", "Game", "Asks everyone (or a team, or the living) a question with 2 to 8 answers for some seconds; then runs the winning answer's output, Tie, or No votes. Bots vote at random if Bots vote is on. Only one vote at a time, and never over the server's map or mode vote (then No votes runs). Vote Count reads the result.",
     inputs=[X], outputs=[E(), E("w1","1"), E("w2","2"), E("w3","3"), E("w4","4"), E("w5","5"), E("w6","6"), E("w7","7"), E("w8","8"), E("tie","Tie"), E("none","No votes")],
     params=[prm("question","Question","text","Next round?",max=60), prm("count","Answers","number",2,min=2,max=8)] + [prm(f"o{k}",f"Answer {k}","text",["Knives","Pistols","Rifles","Snipers","Shotguns","Grenades","Random","Normal"][k-1],max=30) for k in range(1,9)] + [prm("secs","Seconds","number",15,min=5,max=60), prm("who","Who votes","choice","all",choices=[["all","Everyone"],["t","Terrorists"],["ct","Counter-Terrorists"],["alive","The living"]]), prm("bots","Bots vote","bool",False)],
     keywords="vote poll ballot choose menu majority")
node("game.vote_count", "Vote Count", "Game", "How many votes an answer got in the last Start Vote (0: all the votes).",
     inputs=[P("option","number","Answer",param="option")], outputs=[P("out","number","Votes")], params=[prm("option","Answer (0: all)","number",1,min=0,max=8)], keywords="vote count result poll", **ECT)
# 22. hostages
node("hostage.follow", "Hostage Follow", "NPCs", "Makes a hostage follow a player (as if they pressed USE on it), any team.",
     inputs=[X, P("hostage","entity","Hostage",required=True), P("player","player","Leader",required=True)], outputs=[E()], keywords="hostage follow escort lead rescue")
node("hostage.stop", "Hostage Stop", "NPCs", "Makes a hostage stop following and wait where it is.",
     inputs=[X, P("hostage","entity","Hostage",required=True)], outputs=[E()], keywords="hostage stop wait escort")
node("hostage.is_following", "Is Following", "NPCs", "Is a hostage following someone, and whom.",
     inputs=[P("hostage","entity","Hostage",required=True)], outputs=[P("out","bool","Following"), P("leader","player","Leader")], keywords="hostage following escort leader", **ECT)
# 24. bots
node("botctl.add", "Add Bots", "Bots", "Adds bots through the bot brain's own commands: how many, to which team, how good (new bots only). Never more than the free slots (one is kept open for a person). The host's bot count comes back at the next map.",
     inputs=[X, P("count","number","How many",param="count")], outputs=[E()], params=[prm("count","How many","number",2,min=1,max=31), prm("team","Team","choice","any",choices=[["any","Either (the game balances)"],["t","Terrorists"],["ct","Counter-Terrorists"]]), prm("skill","Difficulty","choice","host",choices=[["host","The host's setting"],["0","Easy"],["1","Normal"],["2","Hard"],["3","Expert"]])],
     keywords="bot add spawn quota fill difficulty")
node("botctl.remove", "Remove Bots", "Bots", "Takes bots off the server (of a team, or any), lowering the bot count so they don't come back this map. The host's bot count comes back at the next map.",
     inputs=[X, P("count","number","How many",param="count")], outputs=[E()], params=[prm("count","How many","number",1,min=1,max=31), prm("team","Team","choice","any",choices=[["any","Any team"],["t","Terrorists"],["ct","Counter-Terrorists"]])],
     keywords="bot kick remove quota")

# effects, radar, team score, weather and fog (Y, engine audit items 17, 18, 25)
BEAM_COLORS = [["red","Red"],["green","Green"],["blue","Blue"],["white","White"],["gold","Gold"]]
SEEN_BY = [["all","Everyone"],["t","Terrorists"],["ct","Counter-Terrorists"],["player","Only Viewer"]]
node("fx.beam_ents", "Beam Between Entities", "Effects", "A beam from one player or entity to another that follows both as they move, for some seconds. Effects are skipped (and the debugger says so) past 24 in one server frame, so many at once can't flood players' games.",
     inputs=[X, P("a","entity","From",required=True), P("b","entity","To",required=True)], outputs=[E()], params=[prm("colour","Color","choice","red",choices=BEAM_COLORS), prm("secs","Seconds","number",2,min=0.1,max=25), prm("width","Width","number",10,min=1,max=100), prm("noise","Wobble","number",0,min=0,max=100)], keywords="beam laser link tether rope")
node("fx.icon_over", "Icon Over Player", "Effects", "A sprite floating over a player's head (the game's own, like sprites/iplayervip.spr, or your own), for some seconds or until removed (0), seen by everyone, a team or only Viewer. Remove takes the icons off that player.",
     inputs=[X, P("player","player","Player",required=True), P("viewer","player","Viewer",optional=True)], outputs=[E()], params=[prm("do","Do","choice","show",choices=[["show","Show"],["remove","Remove"]]), prm("sprite","Sprite","path","sprites/iplayervip.spr"), prm("secs","Seconds (0: until removed)","number",0,min=0,max=600), prm("height","Height above","number",45,min=0,max=200), prm("who","Seen by","choice","all",choices=SEEN_BY)], keywords="icon sprite over head marker attachment")
node("fx.sprite_at", "Sprite At", "Effects", "Shows a sprite once at a position or an entity (the game's own or your own), at a size and brightness.",
     inputs=[X, P("at","entity","At",optional=True), P("pos","vector","Position",optional=True)], outputs=[E()], params=[prm("sprite","Sprite","path","sprites/glow01.spr"), prm("scale","Size","number",1,min=0.1,max=25), prm("bright","Brightness","number",200,min=0,max=255)], keywords="sprite flash glow effect")
node("fx.tracer", "Tracer", "Effects", "A bullet tracer streak from one position to another.",
     inputs=[X, P("from","vector","From",required=True), P("to","vector","To",required=True)], outputs=[E()], keywords="tracer bullet streak line")
node("hud.radar_blip", "Radar Blip", "HUD & View", "A dot on players' radar at a position or an entity (everyone's, a team's or only Viewer's), or takes it off. Hostage dots: four slots, each one dot (a hostage map's own hostages share the game's dots). Bomb dot: one, the dropped-bomb dot (the game moves it when the real bomb drops or is picked up).",
     inputs=[X, P("at","entity","At",optional=True), P("pos","vector","Position",optional=True), P("viewer","player","Viewer",optional=True)], outputs=[E()], params=[prm("do","Do","choice","show",choices=[["show","Show"],["clear","Take off"]]), prm("kind","Dot","choice","hostage",choices=[["hostage","Hostage dot"],["bomb","Bomb dot"]]), prm("slot","Slot (hostage dots)","choice","1",choices=[["1","1"],["2","2"],["3","3"],["4","4"]]), prm("who","Seen by","choice","all",choices=SEEN_BY)], keywords="radar dot blip map marker")
node("hud.team_score", "Show Team Score", "HUD & View", "Shows a team's score on everyone's scoreboard (the number only: the game's own count of rounds won is unchanged and is shown again when the next round ends).",
     inputs=[X, P("score","number","Score",param="score")], outputs=[E()], params=[prm("team","Team","choice","t",choices=TEAM2), prm("score","Score","number",0,min=0,max=32767)], keywords="score scoreboard team points")
node("world.weather", "Set Weather", "World", "Rain, snow or clear weather for everyone or one player, kept through respawns (Counter-Strike 1.6 and Condition Zero draw it; how much depends on the player's settings).",
     inputs=[X, P("player","player","Player (empty: everyone)",optional=True)], outputs=[E()], params=[prm("kind","Weather","choice","rain",choices=[["none","Clear"],["rain","Rain"],["snow","Snow"]])], keywords="weather rain snow")
node("world.fog", "Set Fog", "World", "Fog of a color and thickness for everyone or one player (0 thickness: none), kept through respawns. It depends on the player's renderer: OpenGL draws it, software mode doesn't.",
     inputs=[X, P("player","player","Player (empty: everyone)",optional=True)], outputs=[E()], params=RGB(128, 128, 128) + [prm("density","Thickness","number",0.002,min=0,max=0.05)], keywords="fog mist haze")

# ---- Effects, sounds and spawning (the effects agent; gen/fx.rs). Kept together, after the
# others: the transform every spawn node takes is added here, so their own lines stay as they are.
RGB = lambda r, g, b: [prm("r","Red","number",r,min=0,max=255), prm("g","Green","number",g,min=0,max=255), prm("b","Blue","number",b,min=0,max=255)]
FALLOFF = prm("falloff","Heard","choice","normal",choices=[["everywhere","Everywhere"],["large","Far (large)"],["normal","Normal"],["small","Close by (small)"]])
CHANNELS = [["auto","Any free channel"],["weapon","Weapon"],["voice","Voice"],["item","Item"],["body","Body"],["stream","Stream"],["static","Static"]]
MONO = " Map sounds are best mono: a stereo .wav is heard in the middle of your head, not from where it plays. Your own sounds (sound/custom/...wav) are loaded with the map and go with the mode when you publish it."
XF_HELP = " Where: At entity (its spot, moved by Local offset along its own forward, right and up; it faces the way the entity does if Face the way At entity faces is on, turned by Rotation offset) or At position. Rotation (pitch, yaw, roll) sets the facing outright; Velocity starts it moving (a still thing then falls and slides); Scale sizes models and sprites."
def XF(at="at_ent", scale=True):
    ins = [] if at is None else [P(at,"entity","At entity",optional=True)]
    ins += [P("pos","vector","At position",optional=True), P("offset","vector","Local offset (forward, right, up)",optional=True), P("angles","vector","Rotation (pitch, yaw, roll)",optional=True), P("angles_offset","vector","Rotation offset",optional=True)]
    if scale:
        ins += [P("vel","vector","Velocity",optional=True), P("scale","number","Scale",param="scale")]
    prms = [prm("copy_rot","Face the way At entity faces","bool",True)] + ([prm("scale","Scale","number",1,min=0.05,max=10)] if scale else [])
    return ins, prms

node("fx.dlight", "Dynamic Light", "Effects", "A light that brightens what's around it for a moment: at a position, or following an entity (a player, a projectile) for the seconds given. Fade dims it over time (units a second; 0 stays bright until the end). Players see it in their own game.",
     inputs=[X, P("follow","entity","Follow (optional)",optional=True), P("pos","vector","At position",optional=True), P("radius","number","Radius",param="radius"), P("life","number","Seconds",param="life"), P("decay","number","Fade (units a second)",param="decay")], outputs=[E()],
     params=RGB(255, 200, 120) + [prm("radius","Radius","number",200,min=10,max=2550), prm("life","Seconds","number",1,min=0.1,max=25), prm("decay","Fade (units a second)","number",0,min=0,max=2550)], keywords="light dlight elight glow flash muzzle lamp te_dlight")
node("fx.decal", "Decal", "Effects", "Puts a decal (a scorch, blood, a bullet hole...) on the surface nearest a position: it looks six ways from the point within Reach and paints the closest wall, floor or ceiling, or a door it hits. The game's own decals; they fade like any decal.",
     inputs=[X, P("pos","vector","Near position",required=True), P("reach","number","Reach",param="reach")], outputs=[E()],
     params=[prm("decal","Decal","choice","scorch",choices=[[d[0], d[1]] for d in [["shot","Bullet hole"],["bigshot","Big bullet hole"],["scorch","Scorch mark"],["smscorch","Small scorch"],["blood","Blood"],["yblood","Yellow blood"],["break","Broken glass"],["lambda","Lambda"]]]), prm("reach","Reach","number",64,min=8,max=512)], keywords="decal scorch blood bullet hole mark spray paint te_worlddecal")
node("fx.gibs", "Break Into Gibs", "Effects", "Pieces fly out from a position or an entity's middle, as when something breaks: wood, glass, metal, rocks, flesh, or your own model's parts (Custom model). Material is the sound they make. Size is how big the space they fly from is.",
     inputs=[X, P("at","entity","At entity",optional=True), P("pos","vector","At position",optional=True), P("count","number","Pieces",param="count"), P("speed","number","Speed",param="speed"), P("life","number","Seconds",param="life"), P("size","number","Size",param="size")], outputs=[E()],
     params=[prm("model","Model","choice","wood",choices=[["wood","Wood planks"],["glass","Glass shards"],["metal","Metal plates"],["cinder","Concrete chunks"],["ceiling","Ceiling tiles"],["computer","Computer parts"],["rock","Rocks"],["flesh","Flesh"]]), prm("custom","Custom model (empty: the one above)","path",""), prm("material","Material sound","choice","wood",choices=[["none","None"],["glass","Glass"],["metal","Metal"],["flesh","Flesh"],["wood","Wood"],["concrete","Concrete"]]), prm("count","Pieces","number",8,min=1,max=255), prm("speed","Speed","number",200,min=0,max=2000), prm("life","Seconds","number",2.5,min=0.1,max=25), prm("size","Size","number",16,min=1,max=512)],
     keywords="gibs break debris shatter breakmodel pieces chunks te_breakmodel")
node("fx.ring", "Beam Ring", "Effects", "A ring that grows out from a point, like a shockwave: radius, color, how long and how thick.",
     inputs=[X, P("at","entity","At entity",optional=True), P("pos","vector","At position",optional=True), P("radius","number","Radius",param="radius"), P("life","number","Seconds",param="life"), P("width","number","Width",param="width")], outputs=[E()],
     params=RGB(255, 255, 255) + [prm("radius","Radius","number",300,min=8,max=4096), prm("life","Seconds","number",0.5,min=0.1,max=25), prm("width","Width","number",16,min=1,max=255)], keywords="ring shockwave wave beam cylinder blast te_beamcylinder")
node("snd.play_2d", "Play Sound 2D", "Effects", "A sound heard as is, not from a place: by one player, a team or everyone. Any sound: the game's own or yours. Volume 0 to 1, pitch 50 to 255 (100 normal)." + MONO,
     inputs=[X, P("player","player","Player",optional=True), P("volume","number","Volume",param="volume"), P("pitch","number","Pitch",param="pitch")], outputs=[E()],
     params=[prm("who","Who hears it","choice","all",choices=[["all","Everyone"],["player","The player"],["t","Terrorists"],["ct","Counter-Terrorists"]]), prm("sound","Sound","path","buttons/bell1.wav"), prm("volume","Volume","number",1,min=0,max=1), prm("pitch","Pitch","number",100,min=50,max=255)], keywords="sound play 2d hear music sting announce spk")
node("snd.play_at", "Play Sound at Location", "Effects", "A sound from a place (a position, or where an entity is now), heard louder close by. Heard is how far it carries. Volume 0 to 1, pitch 50 to 255 (100 normal)." + MONO,
     inputs=[X, P("at","entity","At entity",optional=True), P("pos","vector","At position",optional=True), P("volume","number","Volume",param="volume"), P("pitch","number","Pitch",param="pitch")], outputs=[E()],
     params=[prm("sound","Sound","path","ambience/thunder_clap.wav"), prm("volume","Volume","number",1,min=0,max=1), prm("pitch","Pitch","number",100,min=50,max=255), FALLOFF], keywords="sound play 3d position location ambient world")
node("snd.play_on", "Play Sound on Entity", "Effects", "A sound that comes from an entity or a player and moves with it, on a channel (a new sound on the same channel replaces the last one). Volume 0 to 1, pitch 50 to 255 (100 normal)." + MONO,
     inputs=[X, P("ent","entity","Entity or player",required=True), P("volume","number","Volume",param="volume"), P("pitch","number","Pitch",param="pitch")], outputs=[E()],
     params=[prm("sound","Sound","path","items/gunpickup2.wav"), prm("channel","Channel","choice","auto",choices=CHANNELS), prm("volume","Volume","number",1,min=0,max=1), prm("pitch","Pitch","number",100,min=50,max=255), FALLOFF], keywords="sound play attach follow entity player channel emit")
node("snd.loop", "Loop Sound", "Effects", "A sound that keeps playing at a spot, or riding on an entity, until Stop Sound gets its Handle (or the round ends with the entity). A .wav with loop points loops by itself; for one without, set Play again every (seconds). How many play at once is a mode setting (Limits: Loop sounds at once, 16 to start)." + MONO,
     inputs=[X, P("on","entity","On entity",optional=True), P("pos","vector","At position",optional=True), P("volume","number","Volume",param="volume"), P("pitch","number","Pitch",param="pitch"), P("every","number","Play again every (seconds)",param="every")], outputs=[E(), P("out","entity","Handle")],
     params=[prm("sound","Sound","path","ambience/wind1.wav"), prm("volume","Volume","number",1,min=0,max=1), prm("pitch","Pitch","number",100,min=50,max=255), FALLOFF, prm("every","Play again every (seconds; 0: it loops by itself)","number",0,min=0,max=600)], keywords="loop sound ambient hum alarm siren music emitter")
node("snd.stop", "Stop Sound", "Effects", "Stops a Loop Sound (its Handle), whatever plays on an entity's channel, or every sound a player hears now.",
     inputs=[X, P("loop","entity","Loop handle",optional=True), P("ent","entity","Entity",optional=True), P("player","player","Player",optional=True)], outputs=[E()],
     params=[prm("what","Stop","choice","loop",choices=[["loop","A Loop Sound (Handle)"],["entity","An entity's channel"],["player","Everything a player hears"]]), prm("channel","Channel","choice","auto",choices=CHANNELS)], keywords="stop sound silence mute loop")
SPAWN_CLASS_CHOICES = [["env_sprite","Sprite (env_sprite: model=sprites/...spr)"],["env_glow","Glow (env_glow: model=sprites/...spr)"],["cycler_sprite","Animated sprite or model (cycler_sprite: model=...)"],["env_spark","Sparks (env_spark)"],["env_explosion","Explosion (env_explosion: iMagnitude=100)"],["env_shake","Screen shake (env_shake)"],["env_fade","Screen fade (env_fade)"],["env_render","Render changer (env_render)"],["env_blood","Blood spray (env_blood)"],["env_funnel","Particle funnel (env_funnel)"],["env_beverage","Soda can chute (env_beverage)"],["env_shooter","Model shooter (env_shooter: shootmodel=models/...mdl)"],["gibshooter","Gib shooter (gibshooter)"],["env_beam","Beam (env_beam: texture=sprites/laserbeam.spr)"],["env_laser","Laser (env_laser: texture=sprites/laserbeam.spr)"],["ambient_generic","Sound (ambient_generic: message=ambience/x.wav)"],["info_target","Target point (info_target)"],["path_corner","Path stop (path_corner)"],["trigger_relay","Relay (trigger_relay)"],["multi_manager","Multi manager (multi_manager)"],["armoury_entity","Weapon on the ground (armoury_entity: item=0..29)"],["item_healthkit","Health kit (item_healthkit)"],["item_battery","Battery (item_battery)"]]
_i, _p = XF()
node("ent.spawn_entity", "Spawn Entity", "Entities", "Makes one of the game's entities with your keys (key=value pairs as in a map, | between them: model=sprites/glow01.spr|rendercolor=255 0 0|targetname=door_fx). Only classes that are safe to make while the game runs are listed: effects, sounds, markers, relays, pickups. Never players, worldspawn, game_* server controls or anything that runs commands. Files its keys name are loaded with the map; if one isn't on the server it's skipped, never a crash." + XF_HELP,
     inputs=[X] + _i, outputs=[E(), P("out","entity","Entity")], params=[prm("class","Class","choice","env_sprite",choices=SPAWN_CLASS_CHOICES), prm("keys","Keys (key=value, | between)","kvpath","model=sprites/glow01.spr|rendermode=5|renderamt=255",max=400)] + _p,
     keywords="spawn create entity class keyvalue env_ make", advanced=False)
_i, _p = XF()
node("ent.spawn_weapon", "Spawn Weapon On Ground", "Entities", "A weapon or item lying on the ground, picked up like a map's own: any Counter-Strike gun, grenade, armor or the shield (the shield and the guns after the M249 in the list need Loner-Strike's game DLL). Reserve ammo is given with it on pickup (-1: the game's usual). Back after brings it back that many seconds after it's taken (0: at the next round, as the game does)." + XF_HELP,
     inputs=[X] + _i + [P("ammo","number","Reserve ammo",param="ammo"), P("respawn","number","Back after (seconds)",param="respawn")], outputs=[E(), P("out","entity","Weapon")],
     params=[prm("item","Weapon or item","choice","ak47",choices=[["mp5navy","MP5"],["tmp","TMP"],["p90","P90"],["mac10","MAC-10"],["ak47","AK-47"],["sg552","SG 552"],["m4a1","M4A1"],["aug","AUG"],["scout","Scout"],["g3sg1","G3SG1"],["awp","AWP"],["m3","M3 shotgun"],["xm1014","XM1014 shotgun"],["m249","M249"],["flashbang","Flashbang"],["hegrenade","HE grenade"],["kevlar","Kevlar"],["assault","Kevlar and helmet"],["smokegrenade","Smoke grenade"],["shield","Tactical shield"],["famas","FAMAS"],["sg550","SG 550"],["galil","Galil"],["ump45","UMP 45"],["glock18","Glock"],["usp","USP"],["elite","Dual Elites"],["fiveseven","Five-SeveN"],["p228","P228"],["deagle","Desert Eagle"]]), prm("ammo","Reserve ammo (-1: the game's usual)","number",-1,min=-1,max=1000), prm("respawn","Back after (seconds; 0: next round)","number",0,min=0,max=600)] + _p,
     keywords="weapon gun item spawn ground drop armoury pickup ammo respawn")

# solid spawn nodes: what happens when the spot is blocked (Unreal's Spawn Collision Handling), and
# the spawn outcome standard (docs/notes/FX.md): Then right away; Spawned / Failed once each
# (maybe later, with Retry); Entity, Success, Moved, Failure Reason (the Spawn Failure enum)
BLOCKED_CHOICES = [["force","Always spawn, ignore collisions"],["nearby_always","Try a nearby spot, but always spawn"],["nearby","Try a nearby spot, don't spawn if still blocked"],["exact","Don't spawn if blocked"],["retry","Retry every N seconds up to M tries"]]
BLOCKED_HELP = (" If the spot is blocked (Unreal's Spawn Collision Handling): Always spawn, ignore collisions (Unreal's Always Spawn: it may get stuck);"
    " Try a nearby spot, but always spawn (Adjust If Possible But Always Spawn); Try a nearby spot, don't spawn if still blocked (Adjust If Possible But Don't Spawn If Colliding);"
    " Don't spawn if blocked (Do Not Spawn); or Retry every N seconds up to M tries (0: until it's clear). Nearby means within Max distance; Moved says it was moved."
    " Outputs: Then goes on right away; Spawned once it exists, Failed once it gives up (Retry can make these come later); Entity, Success, Moved and Failure Reason"
    " (Blocked, Out of entity slots, Limit reached, Missing file, Retries used up, Bad input) let you try a fallback, like another spot.")
for d in nodes:
    if d["type"] in ("ent.spawn_prop", "creature.spawn", "comp.create", "ent.spawn_entity", "ent.spawn_weapon", "npc.spawn", "npc.spawn_use", "proj.spawn"):
        for o in d["outputs"]:
            if o["id"] == "out":
                o["label"] = "Entity"
        if not any(o["id"] == "spawned" for o in d["outputs"]):
            d["outputs"] += [E("spawned","Spawned"), E("failed","Failed"), P("ok","bool","Success"), P("moved","bool","Moved"), P("reason","enum:ls_spawn_fail","Failure reason")]
        # (T1fix: an interactable stays exactly where it's put; a projectile isn't solid: Always spawn)
        d["params"] += [prm("blocked","If the spot is blocked","choice",{"npc.spawn_use": "exact", "proj.spawn": "force"}.get(d["type"], "nearby"),choices=BLOCKED_CHOICES), prm("max_dist","Max distance for a nearby spot","number",128,min=16,max=1024), prm("retry_every","Retry every (seconds)","number",1,min=0.1,max=60), prm("retry_tries","Retry tries (0: until clear)","number",10,min=0,max=1000)]
        d["help"] += BLOCKED_HELP
        d["keywords"] = (d.get("keywords", "") + " collision handling force spawn always spawn blocked free spot fallback").strip()

# the older sound nodes: still working, left out of the node list (Problems says what replaced them)
SUPERSEDED = {"game.play_sound": "snd.play_2d", "fx.sound_at": "snd.play_at", "fx.sound_to": "snd.play_2d"}
# every spawn node takes the same transform (their saved pins and settings keep working)
for d in nodes:
    if d["type"] in SUPERSEDED:
        d["superseded"] = SUPERSEDED[d["type"]]
        d["help"] += " Superseded by " + {"snd.play_2d": "Play Sound 2D", "snd.play_at": "Play Sound at Location"}[SUPERSEDED[d["type"]]] + " (volume, pitch, falloff); this one still works."
    if d["type"] in ("ent.spawn_prop", "creature.spawn", "npc.spawn", "npc.spawn_use", "comp.create"):
        at = "at" if d["type"] == "ent.spawn_prop" else "at_ent"
        have = {p["id"] for p in d["inputs"]}
        ins, prms = XF(at=None if at in have else at, scale=d["type"] != "comp.create")
        for p in d["inputs"]:
            if p["id"] == at:
                p["label"] = "At entity"
                p.pop("required", None)
                p["optional"] = True
            if p["id"] == "pos":
                p["label"] = "At position"
        d["inputs"] += [p for p in ins if p["id"] not in have]
        d["params"] += [p for p in prms if p["id"] not in {q["id"] for q in d["params"]}]
        d["help"] += XF_HELP if d["type"] != "comp.create" else XF_HELP.split(" Rotation (pitch")[0] + " Rotation (pitch, yaw, roll) sets the facing outright (no Velocity or Scale: a component is a zone)."

data = {
  "version": 5,
  # (a renamed node's old type id -> its id now: StudioCompat.md, never remove one)
  "aliases": {},
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
    "model": {"label": "Model", "color": "#c98b5a", "mark": "file"},
    "sprite": {"label": "Sprite", "color": "#f29ad8", "mark": "file"},
    "sound": {"label": "Sound", "color": "#6fe3b0", "mark": "file"},
    "decal": {"label": "Decal", "color": "#b9a37a", "mark": "file"},
    "visual": {"label": "Model or Sprite", "color": "#dd92a0", "mark": "file"},
    "color": {"label": "Color", "color": "#ff5fa2", "mark": "rainbow"},
    "npc": {"label": "NPC", "color": "#4dd0c8", "badge": "N"},
    "creature": {"label": "Creature", "color": "#4dd0c8", "badge": "C"},
    "projectile": {"label": "Projectile", "color": "#4dd0c8", "badge": "P"},
    "volume": {"label": "Trigger Volume", "color": "#4dd0c8", "badge": "V"},
    "objmarker": {"label": "Objective Marker", "color": "#4dd0c8", "badge": "M"},
    "component": {"label": "Component", "color": "#4dd0c8", "badge": "K"},
    "soundloop": {"label": "Sound Loop", "color": "#9cc4ff"},
    "var": {"label": "Variable", "color": "#cccccc"}
  },
  "categories": ["Events", "Conditions", "Numbers", "Text", "Variables", "Flow", "Functions", "Macros", "Arrays", "Maps", "Structs", "Enums", "Vectors", "Timers", "Round clock", "Players", "Damage", "Squads", "Buying", "Saved", "Messages", "Game", "Scoring", "Bots", "Components", "Map markers", "Waves", "Upgrades", "Game", "World", "Entities", "Effects", "Advanced", "Downed", "Shop", "NPCs", "Battle Royale", "Creatures", "Data tables", "Projectiles", "HUD & View"],
  "nodes": nodes,
}
# ==== W1: utility nodes (spatial, random, math, text, arrays, vectors, aim, flow, On Tick) ====
FROM_IN = [P("pos","vector","Around point",optional=True), P("to","entity","Around entity")]
TEAM_ANY = prm("team","Team","choice","any",choices=[["any","Any team"],["t","Terrorists"],["ct","Counter-Terrorists"]])
SP = dict(pure=True)
node("spatial.players_in_radius", "Players In Radius", "Spatial", "Every player within Radius of a point or an entity, as an array (an entity wired in isn't counted itself).",
     inputs=FROM_IN + [P("radius","number","Radius",param="radius")], outputs=[P("out","array<player>","Players")],
     params=[prm("radius","Radius","number",300,min=1,max=8192), prm("alive","Alive only","bool",True), TEAM_ANY], keywords="near nearby around close range sphere area aoe", **SP)
node("spatial.nearest_player", "Nearest Player", "Spatial", "The player nearest to a point or an entity (not the entity itself), within Max distance; nobody if none.",
     inputs=FROM_IN, outputs=[P("out","player","Player"), P("dist","number","Distance")],
     params=[prm("max","Max distance","number",8192,min=1,max=16384), prm("alive","Alive only","bool",True), TEAM_ANY, prm("bots","Who","choice","all",choices=[["all","Bots and people"],["humans","People only"],["bots","Bots only"]])], keywords="closest near target", **SP)
node("spatial.entities_in_radius", "Entities In Radius", "Spatial", "Entities within Radius of a point or an entity, of a class or any, as an array (up to the mode's Limits setting, 64 at first). Players aren't listed (use Players In Radius).",
     inputs=FROM_IN + [P("radius","number","Radius",param="radius")], outputs=[P("out","array<entity>","Entities")],
     params=[prm("radius","Radius","number",300,min=1,max=32768), prm("class","Class (empty: any)","entname","")], keywords="near nearby around find sphere area props", **SP)
node("spatial.nearest_entity", "Nearest Entity", "Spatial", "The entity of a class nearest to a point or an entity, within Max distance; nothing if none. Players aren't counted.",
     inputs=FROM_IN, outputs=[P("out","entity","Entity"), P("dist","number","Distance")],
     params=[prm("class","Class","entname","info_target"), prm("max","Max distance","number",2048,min=1,max=32768)], keywords="closest near find", **SP)
node("spatial.line_of_sight", "Line of Sight", "Spatial", "Is there a clear line between A and B (each an entity or a point)? Entities wired in don't block it (a player's eyes are used). Hit Position is where something got in the way.",
     inputs=[P("a","entity","A"), P("a_pos","vector","Or point A",optional=True), P("b","entity","B"), P("b_pos","vector","Or point B",optional=True)],
     outputs=[P("clear","bool","Clear"), P("hit","vector","Hit position")], params=[prm("monsters","Creatures and players block","bool",False)], keywords="visible see trace los sight blocked wall", **SP)
# random
node("random.chance", "Random Chance", "Numbers", "Yes Percent % of the time (a new roll each time it's worked out).", inputs=[P("pct","number","Percent",param="pct")], outputs=[P("out","bool","Yes")],
     params=[prm("pct","Percent","number",25,min=0,max=100)], keywords="probability odds percent roll luck dice", **SP)
node("random.bool", "Random Bool", "Numbers", "Yes or no, half the time each.", outputs=[P("out","bool","Yes")], keywords="coin flip random yes no", **SP)
node("random.weighted", "Weighted Random Pick", "Numbers", "Picks an index from an array of weights (a weight of 3 is picked three times as often as 1); with Items wired, the item at that index too. -1 when every weight is 0.",
     inputs=[P("weights","array<number>","Weights",required=True), P("items","array<$T>","Items",optional=True)], outputs=[P("index","number","Index"), P("item","$T","Item")], params=[OF()], keywords="loot table chance roll pick weight drop", **SP)
# math
node("math.map_range", "Map Range", "Numbers", "A value from one range moved into another: In Min..In Max becomes Out Min..Out Max (Clamp keeps it inside).",
     inputs=[P("a","number","Value",param="a"), P("in_min","number","In Min",param="in_min"), P("in_max","number","In Max",param="in_max"), P("out_min","number","Out Min",param="out_min"), P("out_max","number","Out Max",param="out_max")], outputs=[P("out","number","Result")],
     params=[prm("a","Value","number",50,min=-100000,max=100000), prm("in_min","In Min","number",0,min=-100000,max=100000), prm("in_max","In Max","number",100,min=-100000,max=100000), prm("out_min","Out Min","number",0,min=-100000,max=100000), prm("out_max","Out Max","number",1,min=-100000,max=100000), prm("clamp","Clamp","bool",True)], keywords="remap scale normalize convert range", **SP)
UNIT = prm("unit","Angles in","choice","deg",choices=[["deg","Degrees"],["rad","Radians"]])
node("math.trig", "Sin / Cos / Tan", "Numbers", "The sine, cosine or tangent of an angle (degrees, or radians).", inputs=[P("a","number","Angle",param="a")], outputs=[P("out","number","Result")],
     params=[prm("a","Angle","number",45,min=-100000,max=100000), prm("op","Do","choice","sin",choices=[["sin","Sin"],["cos","Cos"],["tan","Tan"]]), UNIT], keywords="sine cosine tangent trigonometry angle wave", **SP)
node("math.atan2", "Atan2", "Numbers", "The angle of the direction X, Y (from the X axis, -180..180 degrees, or radians).", inputs=[P("y","number","Y",param="y"), P("x","number","X",param="x")], outputs=[P("out","number","Angle")],
     params=[prm("y","Y","number",1,min=-100000,max=100000), prm("x","X","number",1,min=-100000,max=100000), UNIT], keywords="angle arctangent direction heading yaw", **SP)
node("math.angle_convert", "Degrees / Radians", "Numbers", "Degrees to radians or radians to degrees.", inputs=[P("a","number","Angle",param="a")], outputs=[P("out","number","Result")],
     params=[prm("a","Angle","number",180,min=-100000,max=100000), prm("op","Do","choice","to_rad",choices=[["to_rad","Degrees to radians"],["to_deg","Radians to degrees"]])], keywords="deg rad convert angle", **SP)
node("flow.nearly_equal", "Nearly Equal", "Conditions", "Are A and B within Tolerance of each other (for numbers that came out of math, which are seldom exactly equal)?",
     inputs=[P("a","number","A",param="a"), P("b","number","B",param="b"), P("tol","number","Tolerance",param="tol")], outputs=[P("out","bool","Nearly equal")],
     params=[prm("a","A","number",0,min=-100000,max=100000), prm("b","B","number",0,min=-100000,max=100000), prm("tol","Tolerance","number",0.01,min=0,max=100000)], keywords="approximately close equal epsilon float compare", **SP)
# text
TX = lambda id="a", label="Text": P(id,"text",label,required=True)
node("text.split", "Split Text", "Text", "Text cut at each Separator, as an array of texts (at most 64 pieces; empty pieces kept).", inputs=[TX(), P("sep","text","Separator",param="sep")], outputs=[P("out","array<text>","Pieces")],
     params=[prm("sep","Separator","text",",",max=8)], keywords="explode tokens words cut parse", **SP)
node("text.replace", "Replace Text", "Text", "The text with every Find replaced by With (capitals matter).", inputs=[TX(), P("find","text","Find",param="find"), P("with","text","With",param="with")], outputs=[P("out","text","Text")],
     params=[prm("find","Find","text","",max=40), prm("with","With","text","",max=40)], keywords="swap substitute change", **SP)
node("text.substring", "Substring", "Text", "Part of the text: Length characters from Start (the first is 0).", inputs=[TX(), P("start","number","Start",param="start"), P("len","number","Length",param="len")], outputs=[P("out","text","Text")],
     params=[prm("start","Start","number",0,min=0,max=127), prm("len","Length","number",5,min=0,max=127)], keywords="part mid slice cut left right characters", **SP)
node("text.upper", "Upper Case", "Text", "THE TEXT IN CAPITALS.", inputs=[TX()], outputs=[P("out","text","Text")], keywords="capitals uppercase caps", **SP)
node("text.lower", "Lower Case", "Text", "the text in small letters.", inputs=[TX()], outputs=[P("out","text","Text")], keywords="lowercase small", **SP)
node("text.trim", "Trim Text", "Text", "The text without spaces at the start and end.", inputs=[TX()], outputs=[P("out","text","Text")], keywords="strip spaces whitespace", **SP)
node("text.starts_with", "Starts With", "Text", "Does the text start with this (capitals don't matter)?", inputs=[TX(), P("b","text","Start",param="b")], outputs=[P("out","bool","Starts with")], params=[prm("b","Start","text","",max=40)], keywords="prefix begins", **SP)
node("text.ends_with", "Ends With", "Text", "Does the text end with this (capitals don't matter)?", inputs=[TX(), P("b","text","End",param="b")], outputs=[P("out","bool","Ends with")], params=[prm("b","End","text","",max=40)], keywords="suffix", **SP)
node("text.join", "Join Array", "Text", "An array's texts in one, with Separator between them (as much as fits in 127 characters).", inputs=[P("array","array<text>","Texts",required=True), P("sep","text","Separator",param="sep")], outputs=[P("out","text","Text")],
     params=[prm("sep","Separator","text",", ",max=8)], keywords="implode combine list concat", **SP)
node("text.format_time", "Format Time", "Text", "Seconds as a clock: 1:05, or 1:02:05 (Auto: hours only when there are any).", inputs=[P("secs","number","Seconds",required=True)], outputs=[P("out","text","Text")],
     params=[prm("style","Style","choice","auto",choices=[["auto","Auto"],["ms","m:ss"],["hms","h:mm:ss"]])], keywords="clock timer duration minutes countdown", **SP)
node("text.format_number", "Format Number", "Text", "A number as text with a set number of decimals, and thousands separated by commas if you like (12,345.6).", inputs=[P("a","number","Number",required=True)], outputs=[P("out","text","Text")],
     params=[prm("decimals","Decimals","number",0,min=0,max=4), prm("thousands","Thousands separator","bool",True)], keywords="decimal round money comma digits", **SP)
# arrays of numbers
AN = P("array","array<number>","Numbers",required=True)
node("array.sum", "Sum", "Arrays", "All the numbers added up (0 for an empty array).", inputs=[AN], outputs=[P("out","number","Sum")], keywords="total add", **SP)
node("array.min", "Array Min", "Arrays", "The smallest number, and its index (-1 for an empty array).", inputs=[AN], outputs=[P("out","number","Min"), P("index","number","Index")], keywords="smallest lowest", **SP)
node("array.max", "Array Max", "Arrays", "The biggest number, and its index (-1 for an empty array).", inputs=[AN], outputs=[P("out","number","Max"), P("index","number","Index")], keywords="biggest highest largest", **SP)
node("array.average", "Average", "Arrays", "The numbers' average (0 for an empty array).", inputs=[AN], outputs=[P("out","number","Average")], keywords="mean", **SP)
node("array.reverse", "Reverse", "Arrays", "Turns an array round: the last item first.", inputs=[X, P("array","array<$T>","Array",required=True)], outputs=[E()], params=[OF()], keywords="flip backwards order")
node("array.slice", "Slice", "Arrays", "A new array with Count items from Start (the first is 0).", inputs=[P("array","array<$T>","Array",required=True), P("start","number","Start",param="start"), P("count","number","Count",param="count")], outputs=[P("out","array<$T>","Array")],
     params=[OF(), prm("start","Start","number",0,min=0,max=10000), prm("count","Count","number",3,min=0,max=10000)], keywords="sub part range copy", **SP)
# vectors
V = lambda id, label: P(id,"vector",label,required=True)
node("vec.dot", "Dot Product", "Vectors", "A·B: how much two directions point the same way (with both length 1: 1 same, 0 square, -1 opposite).", inputs=[V("a","A"), V("b","B")], outputs=[P("out","number","Dot")], keywords="dot product angle facing", **SP)
node("vec.cross", "Cross Product", "Vectors", "A×B: the direction square to both (its length is how far apart they point).", inputs=[V("a","A"), V("b","B")], outputs=[P("out","vector","Cross")], keywords="cross product perpendicular normal", **SP)
node("vec.random_in_radius", "Random Point In Radius", "Vectors", "A random point within Radius of Center: on a flat circle (2D, Z kept) or in a ball (3D).", inputs=[V("c","Center"), P("radius","number","Radius",param="radius")], outputs=[P("out","vector","Point")],
     params=[prm("radius","Radius","number",200,min=0,max=8192), prm("mode","Shape","choice","2d",choices=[["2d","Circle (2D)"],["3d","Ball (3D)"]])], keywords="scatter spawn spread random position", **SP)
node("vec.rotate", "Rotate Vector", "Vectors", "Turns a vector round the vertical by Yaw degrees (and up or down by Pitch).", inputs=[V("a","Vector"), P("yaw","number","Yaw",param="yaw"), P("pitch","number","Pitch",param="pitch")], outputs=[P("out","vector","Result")],
     params=[prm("yaw","Yaw","number",90,min=-360,max=360), prm("pitch","Pitch","number",0,min=-90,max=90)], keywords="turn spin angle", **SP)
node("vec.lerp", "Lerp Vector", "Vectors", "A point between A and B: Alpha 0 is A, 1 is B, 0.5 halfway.", inputs=[V("a","A"), V("b","B"), P("alpha","number","Alpha",param="alpha")], outputs=[P("out","vector","Result")],
     params=[prm("alpha","Alpha","number",0.5,min=-100,max=100)], keywords="interpolate blend between mix", **SP)
node("vec.to_angles", "Direction To Angles", "Vectors", "A direction as angles (pitch, yaw, 0), for aiming and Look At math.", inputs=[V("a","Direction")], outputs=[P("out","vector","Angles")], keywords="aim pitch yaw look rotation vector to angles", **SP)
node("vec.forward", "Angles To Forward Vector", "Vectors", "Angles (pitch, yaw, roll) as the direction they face, length 1.", inputs=[V("a","Angles")], outputs=[P("out","vector","Direction")], keywords="forward direction facing aim angle vector", **SP)
# relative space and aim
THING = P("thing","entity","Player or entity",required=True)
node("rel.to_local", "To Local Space", "Vectors", "A world position as seen from a player or entity: X forward, Y right, Z up (by its facing; a player's view yaw).", inputs=[THING, V("pos","World position")], outputs=[P("out","vector","Local offset")], keywords="relative local offset forward right up", **SP)
node("rel.to_world", "To World Space", "Vectors", "An offset from a player or entity (X forward, Y right, Z up, by its facing) as a world position.", inputs=[THING, V("off","Local offset")], outputs=[P("out","vector","World position")], keywords="relative offset in front behind spawn position", **SP)
node("rel.position_of", "Relative Position Of", "Vectors", "How far B is in front of, to the right of and above A (by A's facing; behind and left are negative).",
     inputs=[P("a","entity","A",required=True), P("b","entity","B",required=True)], outputs=[P("forward","number","Forward"), P("right","number","Right"), P("up","number","Up")], keywords="relative offset local", **SP)
node("rel.angle_to", "Angle To", "Vectors", "How far A would turn to face B: the yaw difference, -180..180 degrees (positive: to the left).", inputs=[P("a","entity","A",required=True), P("b","entity","B",required=True)], outputs=[P("out","number","Angle")], keywords="yaw turn facing direction", **SP)
CONE = prm("cone","Cone (degrees)","number",90,min=1,max=360)
node("rel.is_in_front", "Is In Front", "Vectors", "Is B in front of A, inside a cone of this many degrees round A's facing?", inputs=[P("a","entity","A",required=True), P("b","entity","B",required=True)], outputs=[P("out","bool","In front")], params=[CONE], keywords="facing ahead view cone fov", **SP)
node("rel.is_behind", "Is Behind", "Vectors", "Is B behind A, inside a cone of this many degrees round A's back (backstabs, sneaking)?", inputs=[P("a","entity","A",required=True), P("b","entity","B",required=True)], outputs=[P("out","bool","Behind")], params=[CONE], keywords="back backstab sneak rear", **SP)
PL = P("player","player","Player",required=True)
node("aim.eye_pos", "Eye Position", "Players", "Where a player's eyes are (their position plus view height).", inputs=[PL], outputs=[P("out","vector","Eyes")], keywords="view head camera", **SP)
node("aim.dir", "Aim Direction", "Players", "Which way a player is aiming, length 1 (from their view angles).", inputs=[PL], outputs=[P("out","vector","Direction")], keywords="look view forward facing", **SP)
node("aim.point", "Aim Point", "Players", "What a player is aiming at, up to Max distance: where the line from their eyes hits, the entity hit, the surface's normal, and whether it hit anything (the player is ignored).",
     inputs=[PL], outputs=[P("pos","vector","Hit position"), P("ent","entity","Hit entity"), P("normal","vector","Hit normal"), P("hit","bool","Hit")],
     params=[prm("max","Max distance","number",4096,min=16,max=32768)], keywords="trace crosshair looking at target raycast", **SP)
# flow
node("flow.multi_gate", "Multi Gate", "Flow", "Each time flow comes in, goes out the next output (in order, or random); after the last it stops, or starts over with Loop. Reset starts again from Start index.",
     inputs=[X, E("reset","Reset")], outputs=[E("then0","Out 0"), E("then1","Out 1")],
     params=[prm("count","Outputs","number",3,min=1,max=32), prm("random","Random","bool",False), prm("loop","Loop","bool",False), prm("start","Start index","number",0,min=0,max=7)], keywords="round robin cycle rotate sequence each")
node("flow.do_once_per_player", "Do Once Per Player", "Flow", "Lets the flow through once for each player, until Reset (everyone) or Reset Player (the player wired in then).",
     inputs=[X, E("reset","Reset"), E("reset_player","Reset Player"), P("player","player","Player",required=True)], outputs=[E("then","Completed")], keywords="once each player first time")
node("var.toggle", "Toggle Variable", "Variables", "Flips a yes/no variable (yes becomes no, no becomes yes); New Value is what it is now. A per-player one needs a Player, a per-team one a Team.",
     inputs=[X, P("player","player","Player",optional=True), P("team","team","Team",optional=True)], outputs=[E(), P("value","bool","New value")], params=[prm("var","Variable","var","",varType="bool")], keywords="flip switch invert not bool toggle")
node("event.on_tick", "On Tick", "Events", "Runs again and again, every Interval seconds (0.1: ten times a second; 0: as often as the mode's Limits setting lets it, 100 a second at first), all through the map. Delta is the seconds since the last time.",
     outputs=[E(), P("delta","number","Delta")], params=[prm("interval","Interval (seconds)","number",0.1,min=0,max=60)], event=True, keywords="update frame loop every tick think interval")

# testing: debug drawings, Log Line
COLOR = prm("color","Color","choice","red",choices=[["red","Red"],["green","Green"],["blue","Blue"],["yellow","Yellow"],["white","White"],["orange","Orange"],["purple","Purple"]])
SECS = prm("secs","Seconds","number",2,min=0.1,max=60)
TESTONLY = " Shown only while testing (Playtest and Test run); a finished mode draws nothing."
node("debug.draw_line", "Draw Debug Line", "Advanced", "Draws a line from A to B for some seconds, to see where things are." + TESTONLY,
     inputs=[X, P("a","vector","From",required=True), P("b","vector","To",required=True)], outputs=[E()], params=[COLOR, SECS], keywords="debug draw line visualize show trace ray")
node("debug.draw_box", "Draw Debug Box", "Advanced", "Draws a box between two opposite corners for some seconds." + TESTONLY,
     inputs=[X, P("a","vector","Corner",required=True), P("b","vector","Opposite corner",required=True)], outputs=[E()], params=[COLOR, SECS], keywords="debug draw box area bounds visualize")
node("debug.draw_point", "Draw Debug Point", "Advanced", "Draws a small box round a point for some seconds." + TESTONLY,
     inputs=[X, P("pos","vector","Position",required=True)], outputs=[E()], params=[COLOR, SECS, prm("size","Size","number",8,min=1,max=256)], keywords="debug draw point marker dot visualize")
node("debug.log_line", "Log Line", "Messages", "Writes a line to the AMX Mod X log (addons/amxmodx/logs), up to Most a second (that setting's number of lines) from this node; the rest are counted in the next line.",
     inputs=[X, P("text","text","Text",param="text")], outputs=[E()], params=[prm("text","Text","text","Something happened",max=120), prm("most","Most a second","number",5,min=1,max=100)], keywords="log file write record amxx logs")
node("math.bitwise", "Bitwise", "Numbers", "Bits of whole numbers (the fractions are dropped): And, Or, Xor, Not (of A), or A shifted left or right by B bits. Exact up to 16,777,216.",
     inputs=[P("a","number","A",param="a"), P("b","number","B",param="b")], outputs=[P("out","number","Result")],
     params=[prm("a","A","number",6,min=-100000,max=100000), prm("b","B","number",3,min=-100000,max=100000), prm("op","Do","choice","and",choices=[["and","And (&)"],["or","Or (|)"],["xor","Xor (^)"],["not","Not (~A)"],["shl","Shift left (<<)"],["shr","Shift right (>>)"]])],
     keywords="bits bitwise and or xor not shift flags mask & | ^ ~ << >>", **SP)
node("spatial.free_spot", "Find Free Spot Near", "Spatial", "A spot near a point where a player (or the hull you pick) fits: the point itself if it's free, else the nearest free one in rings round it, up to Max distance (at most the mode's Limits setting of tries, 64 at first). Found is no when there's none (Position is then the point).",
     inputs=[P("pos","vector","Near",required=True)], outputs=[P("found","bool","Found"), P("out","vector","Position")],
     params=[prm("hull","Size","choice","stand",choices=[["stand","Player standing"],["duck","Player ducking"],["large","Large (big creatures)"],["point","A point"]]), prm("max","Max distance","number",256,min=0,max=4096)],
     keywords="free spot space spawn place teleport unstuck empty fits", **SP)
node("player.language", "Player Language", "Players", "A player's language: AMX Mod X's for them (their lang setting, else the server's, like en), or their game's (cl_language, like english: empty for bots, and for a moment until their game answers).",
     inputs=[P("player","player","Player",required=True)], outputs=[P("out","text","Language")],
     params=[prm("source","From","choice","amxx",choices=[["amxx","AMX Mod X (en, de...)"],["game","The game (english...)"]])], keywords="language locale lang translate", **SP)
# findable: what already exists under other words
def _kw(t, words):
    for d in nodes:
        if d["type"] == t:
            d["keywords"] = (d.get("keywords", "") + " " + words).strip()
_kw("math.op", "mod modulo % remainder plus minus times divide power")
_kw("math.fn", "sign abs absolute round floor ceil sqrt truncate trunc")
_kw("flow.flipflop", "toggle switch alternate")
_kw("flow.for_each_array", "break loop each array")
_kw("math.compare", "> >= < <= == != = greater less more than equal compare bigger smaller")
for d in nodes:
    if d["type"] == "math.compare":
        for p in d["params"]:
            if p["id"] == "op":
                p["choices"] = [["gt","> more than"],["ge","≥ at least"],["lt","< less than"],["le","≤ at most"],["eq","= equal to"],["ne","≠ not equal to"]]
        d["presets"] = [{"title": "Greater Than (A > B)", "params": {"op": "gt"}, "keywords": "greater than > more bigger"},
                        {"title": "At Least (A ≥ B)", "params": {"op": "ge"}, "keywords": "greater or equal >= at least"},
                        {"title": "Less Than (A < B)", "params": {"op": "lt"}, "keywords": "less than < smaller fewer"},
                        {"title": "At Most (A ≤ B)", "params": {"op": "le"}, "keywords": "less or equal <= at most"},
                        {"title": "Equal (A = B)", "params": {"op": "eq"}, "keywords": "equal == same"},
                        {"title": "Not Equal (A ≠ B)", "params": {"op": "ne"}, "keywords": "not equal != different"}]
    if d["type"] == "math.fn":
        for p in d["params"]:
            if p["id"] == "op" and not any(c[0] == "trunc" for c in p["choices"]):
                p["choices"].append(["trunc","Truncate (toward zero)"])
        d["help"] = d["help"].rstrip(".") + "; Truncate drops the fraction toward zero (-2.7 becomes -2, where Round down gives -3)."
if "Spatial" not in data["categories"]:
    data["categories"].insert(data["categories"].index("Vectors") + 1, "Spatial")
# ==== end W1 ====
# ==== P5: categories for the input, cooldown and weapon nodes ====
for c, after in [("Input", "Players"), ("Cooldowns", "Input"), ("Weapons", "Cooldowns")]:
    if c not in data["categories"]:
        data["categories"].insert(data["categories"].index(after) + 1, c)
# ==== categories (X) ====
# every node's category is in the list once (the palette and MCP's studio_nodes read it)
for _c in [d["category"] for d in nodes]:
    if _c not in data["categories"]:
        data["categories"].append(_c)
data["categories"] = [c for i, c in enumerate(data["categories"]) if c not in data["categories"][:i]]

# ==== types: whole numbers (X) ====
# one Number type; pins and settings the game takes as whole numbers (counts, ammo, money, slots,
# rounds...) get a # mark, and Problems says when a constant there has a fraction (it's rounded)
WHOLE_IDS = {"count", "ammo", "clip", "reserve", "money", "amount_money", "slot", "index", "tier", "times", "round", "rounds", "lives", "frags", "deaths", "kills", "places", "wave", "level", "max_count", "n"}
_scale = lambda p: any(w in p["label"].lower() for w in ("times", "scale", "multiplier", "×"))
for d in nodes:
    for p in d["inputs"] + d["outputs"]:
        if p["type"] == "number" and p["id"] in WHOLE_IDS and not _scale(p):
            p["whole"] = True
    for p in d["params"]:
        if p["kind"] == "number" and p["id"] in WHOLE_IDS and not _scale(p):
            p["whole"] = True

# ==== types: handles (X) ====
# the entity handles: what the spawn and cast nodes make is typed (an NPC, a Creature...) and still
# goes into every Entity pin (upcast); inputs stay Entity (StudioCompat.md: never narrow one)
for _t, _pin, _ty in [("creature.spawn","out","creature"), ("npc.spawn","out","npc"), ("npc.spawn_use","out","component"), ("fx.marker","out","objmarker"),
                      ("comp.create","out","component"), ("vol.spawn","out","volume"), ("proj.spawn","out","projectile"), ("proj.on_hit","proj","projectile"), ("cast.creature","out","creature")]:
    for o in next(d for d in nodes if d["type"] == _t)["outputs"]:
        if o["id"] == _pin and o["type"] == "entity":
            o["type"] = _ty
for _k, _title, _ty, _what, _cat in [("npc","As NPC","npc","an NPC (ls_npc, gm_components)","NPCs"), ("projectile","As Projectile","projectile","a projectile (Spawn Projectile's)","Projectiles"),
                                      ("volume","As Trigger Volume","volume","a trigger volume (Spawn Trigger Volume's)","Collision"), ("objmarker","As Objective Marker","objmarker","an objective marker","Effects"),
                                      ("component","As Component","component","one of gm_components' components (any kind)","Components")]:
    node("cast." + _k, _title, _cat if _cat in data["categories"] else "Entities", f"Casting: is this entity {_what}? Success with it as that kind, else Failed. The one-click fix where an Entity wire goes into a pin of that kind.",
         inputs=[X, P("in","entity","Entity",required=True)], outputs=[E("ok","Success"), E("fail","Failed"), P("out",_ty,_title[3:])], keywords=f"casting cast as {_k} entity convert", **({"needs": "components"} if _k in ("npc", "component") else {}))

# ==== types: transform (X) ====
# Transform: the app's own struct (graph.rs transform_struct): Position, Rotation, Scale, Velocity.
# Make and Break Transform are Make and Break Struct with it picked; the transform nodes gain a
# Transform pin beside their separate ones (which win when both are wired)
TR = "struct:ls_transform"
for d in nodes:
    if d["type"] == "struct.make":
        d.setdefault("presets", []).append({"title": "Make Transform", "params": {"struct": "ls_transform"}, "keywords": "transform position rotation scale velocity where facing"})
    if d["type"] == "struct.break":
        d.setdefault("presets", []).append({"title": "Break Transform", "params": {"struct": "ls_transform"}, "keywords": "transform position rotation scale velocity split"})
_add_out_t = lambda t, p: next(d for d in nodes if d["type"] == t)["outputs"].append(p)
_add_out_t("player.transform", P("transform","struct:ls_transform","Transform"))
_add_out_t("ent.transform", P("transform","struct:ls_transform","Transform"))
next(d for d in nodes if d["type"] == "player.set_transform")["inputs"].append(P("transform","struct:ls_transform","Transform (separate pins win)",optional=True))
node("ent.set_transform", "Set Entity Transform", "Entities", "Puts an entity where a Transform says: its Position, Rotation, Scale (props and sprites; players can't be resized) and Velocity.",
     inputs=[X, P("in","entity","Entity",required=True), P("transform","struct:ls_transform","Transform",required=True)], outputs=[E()], keywords="move teleport place restore position rotation scale")

# ==== types: color (X) ====
# a Color is one value (red, green, blue, alpha); the nodes that took three numbers or a fixed
# choice keep them and gain an optional Color pin that wins when wired
CP = dict(pure=True)
node("color.literal", "Color", "Colors", "A color picked with the color picker (and how opaque: alpha).",
     outputs=[P("out","color","Color")], params=[prm("value","Color","color","#ff4040ff")], keywords="colour rgb pick swatch constant", **CP)
node("color.make", "Make Color", "Colors", "A color from red, green, blue and alpha (0 to 255 each).",
     inputs=[P("r","number","Red",param="r",whole=True), P("g","number","Green",param="g",whole=True), P("b","number","Blue",param="b",whole=True), P("a","number","Alpha",param="a",whole=True)], outputs=[P("out","color","Color")],
     params=[prm("r","Red","number",255,min=0,max=255), prm("g","Green","number",255,min=0,max=255), prm("b","Blue","number",255,min=0,max=255), prm("a","Alpha","number",255,min=0,max=255)], keywords="colour rgb rgba", **CP)
node("color.break", "Break Color", "Colors", "A color's red, green, blue and alpha (0 to 255 each).",
     inputs=[P("in","color","Color",required=True)], outputs=[P("r","number","Red",whole=True), P("g","number","Green",whole=True), P("b","number","Blue",whole=True), P("a","number","Alpha",whole=True)], keywords="colour rgb split", **CP)
node("color.lerp", "Blend Colors", "Colors", "Between color A (Amount 0) and color B (Amount 1): 0.5 is halfway.",
     inputs=[P("a","color","A",required=True), P("b","color","B",required=True), P("t","number","Amount",param="t")], outputs=[P("out","color","Color")],
     params=[prm("t","Amount","number",0.5,min=0,max=1)], keywords="colour mix lerp fade between gradient", **CP)
node("color.hsv", "Color From Hue", "Colors", "A color from hue (0 to 360: red, yellow, green, cyan, blue, magenta), saturation and brightness (0 to 1).",
     inputs=[P("h","number","Hue",param="h"), P("s","number","Saturation",param="s"), P("v","number","Brightness",param="v")], outputs=[P("out","color","Color")],
     params=[prm("h","Hue","number",0,min=0,max=360), prm("s","Saturation","number",1,min=0,max=1), prm("v","Brightness","number",1,min=0,max=1)], keywords="colour hsv hsb rainbow hue", **CP)
if "Colors" not in data["categories"]:
    data["categories"].insert(data["categories"].index("Vectors") + 1, "Colors")
def _add_in(t, *pins):
    d = next(d for d in nodes if d["type"] == t)
    for p in pins:
        if not any(o["id"] == p["id"] for o in d["inputs"]):
            d["inputs"].append(p)
for _t in ["player.set_glow", "player.fade", "fx.beam", "fun.trail", "fun.burst", "msg.status_icon", "fx.marker"]:
    _add_in(_t, P("color","color","Color (wins over the setting)",optional=True))

# ==== types: enums (X) ====
# typed outputs beside the old text ones (StudioCompat.md: new outputs only, nothing removed)
def _add_out(t, *pins):
    d = next(d for d in nodes if d["type"] == t)
    for p in pins:
        if not any(o["id"] == p["id"] for o in d["outputs"]):
            d["outputs"].append(p)
_add_out("player.weapon_held", P("weapon","enum:ls_weapon","Weapon kind"))
_add_out("event.player_killed", P("weapon_kind","enum:ls_weapon","Weapon kind"))
_add_out("event.player_hurt", P("zone","enum:ls_hitzone","Hit zone"))
_add_out("event.player_about_hurt", P("zone","enum:ls_hitzone","Hit zone"), P("dmg_kind","enum:ls_dmgkind","Damage kind"))
_add_out("proj.on_hit", P("zone","enum:ls_hitzone","Hit zone"))

# ==== assets (X-fx) ====
# files as values (gen/assets.rs): a Model, Sprite or Sound is picked (never built from text), so
# every file a mode can use is loaded with the map
if "Files" not in data["categories"]:
    data["categories"].insert(data["categories"].index("Text") + 1, "Files")
FILE_HELP = " Files are picked, never made from text: the game loads every file a mode can use before the map starts. Keep it in a variable, an array or a Function's input like any value."
node("asset.model", "Model", "Files", "A model file (models/....mdl), loaded with the map." + FILE_HELP, outputs=[P("out","model","Model")], params=[prm("file","Model","model","models/w_weaponbox.mdl")], keywords="model file mdl asset constant pick", pure=True)
node("asset.sprite", "Sprite", "Files", "A sprite file (sprites/....spr), loaded with the map." + FILE_HELP, outputs=[P("out","sprite","Sprite")], params=[prm("file","Sprite","sprite","sprites/arrow1.spr")], keywords="sprite file spr asset constant pick", pure=True)
node("asset.sound", "Sound", "Files", "A sound file (a .wav from inside sound/), loaded with the map." + FILE_HELP, outputs=[P("out","sound","Sound")], params=[prm("file","Sound","sound","items/gunpickup2.wav")], keywords="sound file wav asset constant pick audio", pure=True)
node("asset.file_name", "File Name Of", "Files", "A Model's, Sprite's or Sound's file name as text (models/....mdl; a sound from inside sound/), to show or log it. Empty when it's none.", inputs=[P("file","any","File",required=True)], outputs=[P("out","text","Name")], keywords="file name path text model sound sprite to text", pure=True)
# the path settings, typed (graph version 4): same ids and stored values, so saved graphs read as
# they are; a bound pin of the same id takes a wired file (unwired, the setting is the value).
# gen/assets.rs TYPED is the same list.
TYPED_FILES = [("npc.spawn","model","model",True), ("npc.spawn_use","model","model",True), ("proj.spawn","model","visual",True), ("ent.spawn_prop","model","model",True),
    ("fx.sound_at","sound","sound",True), ("fx.sound_to","sound","sound",True), ("fx.marker","sprite","sprite",True), ("fx.gibs","custom","model",True),
    ("snd.play_2d","sound","sound",True), ("snd.play_at","sound","sound",True), ("snd.play_on","sound","sound",True), ("snd.loop","sound","sound",True),
    ("eng.view_model","model","model",False), ("eng.view_model","pmodel","model",False), ("fx.icon_over","sprite","sprite",False), ("fx.sprite_at","sprite","sprite",False)]
for t, pid, ty, pin in TYPED_FILES:
    d = next(d for d in nodes if d["type"] == t)
    prm_ = next(p for p in d["params"] if p["id"] == pid)
    prm_["kind"] = ty
    if pin and not any(i["id"] == pid for i in d["inputs"]):
        d["inputs"].append(P(pid, ty, prm_["label"], optional=True, param=pid))
# Decal values: one of Place Decal's kinds (gen/assets.rs: its number, a name of it picked where
# it's placed); Place Decal takes one wired into a bound pin
_fxd = next(d for d in nodes if d["type"] == "fx.decal")
_dch = next(p for p in _fxd["params"] if p["id"] == "decal")
node("asset.decal", "Decal", "Files", "A kind of decal from the game's decals.wad (bullet hole, scorch, blood...): keep it in a variable or an array, and wire it into Place Decal, which picks one of that kind.", outputs=[P("out","decal","Decal")], params=[prm("decal","Decal","choice",_dch["default"],choices=_dch["choices"])], keywords="decal spray mark scorch blood bullet hole kind constant", pure=True)
if not any(i["id"] == "decal" for i in _fxd["inputs"]):
    _fxd["inputs"].append(P("decal","decal","Decal",optional=True,param="decal"))
# Sound Loop values: Loop Sound's Handle is a playing loop (an entity underneath, so it still goes
# into Entity pins: graph.rs upcast); keep it to stop that loop and no other
_lp = next(d for d in nodes if d["type"] == "snd.loop")
for o in _lp["outputs"]:
    if o["id"] == "out":
        o["type"] = "soundloop"
# Collision (the map editor's names): what a spawned model blocks. The first choice keeps what an
# older graph did (Spawn Prop: what Solid says; Spawn Entity: what the class does)
COLL_HELP = " Collision: None (walk through), Box, players only, or Box, players and bullets: a box the model's own size. At run time both boxes stop bullets too (the engine has no players-only solid for an entity; in the map editor, players only is an invisible CLIP brush)."
for t, first in [("ent.spawn_prop", ["solid", "As Solid says (a 32-unit box)"]), ("ent.spawn_entity", ["class", "As the class makes it"])]:
    d = next(d for d in nodes if d["type"] == t)
    if not any(p["id"] == "collision" for p in d["params"]):
        d["params"].append(prm("collision", "Collision", "choice", first[0], choices=[first, ["none", "None (walk through)"], ["box", "Box, players only"], ["box_bullets", "Box, players and bullets"]]))
        d["help"] += COLL_HELP if t == "ent.spawn_prop" else " Collision (a model class: cycler_sprite, cycler):" + COLL_HELP[len(" Collision:"):]
        d["keywords"] = (d.get("keywords", "") + " collision solid box clip block walk through").strip()
# ==== end assets (X-fx) ====
json.dump(data, open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
print(len(nodes), "nodes")
# the node help (X2's node_help.py), remade with the registry
try:
    import node_help
    node_help.write(data, sys.argv[1].replace("nodes.json", "nodehelp.json"))
except ImportError:
    pass

