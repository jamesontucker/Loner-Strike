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
COMPONENTS = [["ls_capture_point","Capture point"],["ls_zone_shrinker","Shrinking zone"],["ls_damage_zone","Damage zone"],["ls_loot_container","Loot container"],["ls_weapon_spawner","Weapon spawner"],["ls_supply_drop","Supply drop"],["ls_wave_spawner","Wave spawner"],["ls_extraction","Extraction point"],["ls_checkpoint","Checkpoint"],["ls_trap","Trap"],["ls_respawn_point","Respawn point"]]

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
     params=[prm("a","A","number",0,min=-100000,max=100000), prm("op","Do","choice","add",choices=[["add","A + B"],["sub","A - B"],["mul","A × B"],["div","A ÷ B"],["min","smaller"],["max","bigger"]]), prm("b","B","number",1,min=-100000,max=100000)], pure=True)
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
node("timer.delay", "Delay", "Timers", "Waits, then carries on. Paused with the world while debugging.", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",3,min=0.1,max=3600)])
node("timer.repeat", "Repeat", "Timers", "Runs Each every few seconds, a number of times (0: until the round ends). Then runs at once.", inputs=[X, P("every","number","Every (s)",param="every")], outputs=[E("then",""), E("tick","Each")],
     params=[prm("every","Every (seconds)","number",5,min=0.1,max=3600), prm("times","Times (0 = until the round ends)","number",0,min=0,max=10000)])
node("timer.countdown", "Countdown", "Timers", "Counts down on everyone's screen, then runs Done.", inputs=[X], outputs=[E("then",""), E("done","Done")],
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
node("comp.find", "Find Component", "Components", "The component (or any entity) with this name.", outputs=[P("out","entity","Entity")], params=[prm("name","Name","name","A")], pure=True, **ca)
node("comp.create", "Create Component", "Components", "Makes a component at an entity (wired into At, or the one with this name), with keys like radius=160 capture_secs=8.", inputs=[X, P("at_ent","entity","At",optional=True)], outputs=[E(), P("out","entity","Component")],
     params=[prm("kind","Component","choice","ls_capture_point",choices=COMPONENTS), prm("at","At the entity named","name","spot1"), prm("keys","Keys","keys","",max=120)], **ca)
node("comp.zone_set", "Set Zone", "Components", "Moves a shrinking zone to the entity with this name, with this radius.", inputs=[X, P("zone","entity","Zone",required=True), P("radius","number","Radius",param="radius")], outputs=[E()],
     params=[prm("at","Centre at the entity named","name","spot1"), prm("radius","Radius","number",1500,min=50,max=20000)], **ca)
node("comp.zone_shrink", "Shrink Zone", "Components", "Shrinks a zone to a new centre and radius over some seconds.", inputs=[X, P("zone","entity","Zone",required=True), P("radius","number","Radius",param="radius")], outputs=[E()],
     params=[prm("at","New centre at the entity named","name","spot1"), prm("radius","Radius","number",600,min=50,max=20000), prm("secs","Over (seconds)","number",30,min=1,max=600)], **ca)
node("comp.crate_open", "Open Crate", "Components", "Opens a loot container for a player.", inputs=[X, P("container","entity","Container",required=True), P("player","player","Player",required=True)], outputs=[E()], **ca)
node("comp.wave_start", "Start Wave", "Components", "Starts a wave (0: the next one).", inputs=[X, P("wave","number","Wave",param="wave")], outputs=[E()], params=[prm("wave","Wave","number",0,min=0,max=1000)], **ca)
node("comp.wave_stop", "Stop Waves", "Components", "Stops the waves.", inputs=[X], outputs=[E()], **ca)
node("comp.extract_open", "Open Extraction", "Components", "Opens or closes an extraction point.", inputs=[X, P("point","entity","Point",required=True), P("open","bool","Open",param="open")], outputs=[E()], params=[prm("open","Open","bool",True)], **ca)
node("comp.door", "Door", "Components", "Opens or closes a timed door.", inputs=[X, P("door","entity","Door",required=True)], outputs=[E()], params=[prm("do","Do","choice","open",choices=[["open","Open"],["close","Close"]])], **ca)
node("comp.trap_fire", "Fire Trap", "Components", "Sets off a trap.", inputs=[X, P("trap","entity","Trap",required=True)], outputs=[E()], **ca)
node("comp.capture_lock", "Lock Point", "Components", "Locks a capture point for some seconds.", inputs=[X, P("point","entity","Point",required=True), P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",30,min=1,max=3600)], **ca)
node("comp.reset", "Reset Components", "Components", "Puts every component back as it was at the start of the map.", inputs=[X], outputs=[E()], **ca)

# ---- creatures
node("creature.spawn", "Spawn Creature", "Creatures", "Makes a creature of this type at the entity with this name. Needs the Loner-Strike bot brain with creatures (the debugger says so if not).",
     inputs=[X], outputs=[E(), P("out","entity","Creature")], params=[prm("kind","Type","name","zombie"), prm("at","At the entity named","name","spot1")], needs="creatures")

# ---- milestone 3: what whole modes (Last Stand, Battle Royale) need
WHICH = [["all","Everyone in the game"],["alive","Everyone alive"],["dead","Everyone dead"],["t","Terrorists"],["ct","Counter-Terrorists"],["alive_t","Living Terrorists"],["alive_ct","Living Counter-Terrorists"],["bots","Bots"],["humans","People (not bots)"]]
BY = [["class","Of this kind (classname)"],["name","With this name"]]
node("event.player_hurt", "Player Hurt", "Events", "Runs when a player takes damage.", outputs=[E(), P("victim","player","Victim"), P("attacker","player","Attacker"), P("damage","number","Damage")], **ev)
node("event.team_changed", "Team Changed", "Events", "Runs when a player joins a team (or changes team).", outputs=[E(), P("player","player","Player"), P("team","team","Team")], **ev)
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
node("player.set_glow", "Set Glow", "Players", "Makes a player glow in a colour (or not).", inputs=[X, P("player","player","Player",required=True)], outputs=[E()],
     params=[prm("color","Colour","choice","red",choices=[["off","No glow"],["red","Red"],["green","Green"],["blue","Blue"],["yellow","Yellow"],["white","White"]])])
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
node("bot.goto", "Bot Go To", "Bots", "Sends a bot to the entity with this name (it still fights on the way), for some seconds (0: until freed).", inputs=[X, P("bot","player","Bot",required=True)], outputs=[E()], params=[prm("name","To the entity named","name","spot1"), prm("secs","For (seconds, 0 until freed)","number",0,min=0,max=600)])
node("bot.hold", "Bot Hold", "Bots", "Makes a bot hold where it is for some seconds.", inputs=[X, P("bot","player","Bot",required=True), P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",20,min=1,max=600)])
node("bot.follow", "Bot Follow", "Bots", "Makes a bot follow a player for some seconds.", inputs=[X, P("bot","player","Bot",required=True), P("who","player","Follow",required=True)], outputs=[E()], params=[prm("secs","Seconds","number",30,min=1,max=600)])
node("bot.release", "Bot Free", "Bots", "Lets a bot do as it likes again.", inputs=[X, P("bot","player","Bot",required=True)], outputs=[E()])
node("comp.load_pack", "Load Map Pack", "Components", "Loads another mode's map pack for this map (configs/gamemodes/components/<map>.<pack>.ini): br, laststand...", inputs=[X], outputs=[E()], params=[prm("pack","Pack","name","br")], needs="components")
node("comp.start", "Start Component", "Components", "Starts a component that waits to be started (a zone shrinker with start_delay -1).", inputs=[X, P("comp","entity","Component",required=True)], outputs=[E()], needs="components")
node("comp.stop", "Stop Component", "Components", "Stops a component.", inputs=[X, P("comp","entity","Component",required=True)], outputs=[E()], needs="components")
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
     params=[prm("allow","What","choice","all",choices=[["all","Everything"],["pistols","Pistols, armor and grenades"],["no_snipers","Everything but sniper rifles"],["no_awp","Everything but the AWP"],["no_grenades","Everything but grenades"],["no_mg","Everything but machine guns"],["armor","Armor only"],["knife","Nothing (knives)"]])])
node("save.get", "Saved Value", "Saved", "A number kept for this player across maps and restarts (0 until set). Each mode has its own.", inputs=[P("player","player","Player",required=True)], outputs=[P("out","number","Value")], params=[prm("key","Name","name","wins")], pure=True)
node("save.set", "Save Value", "Saved", "Keeps a number for this player across maps and restarts.", inputs=[X, P("player","player","Player",required=True), P("value","number","Value",param="value")], outputs=[E()], params=[prm("key","Name","name","wins"), prm("value","Value","number",0,min=-100000,max=100000)])
node("save.add", "Add To Saved", "Saved", "Adds to a saved number (a minus takes away).", inputs=[X, P("player","player","Player",required=True), P("amount","number","Amount",param="amount")], outputs=[E()], params=[prm("key","Name","name","wins"), prm("amount","Amount","number",1,min=-100000,max=100000)])
node("clock.set", "Set Round Clock", "Round clock", "Runs the mode's own round clock: this many seconds, shown on everyone's round timer. When it reaches 0, Round Time Up runs (the game's own time limit is switched off while it runs).",
     inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",120,min=1,max=7200)])
node("clock.add", "Add Round Time", "Round clock", "Adds seconds to the mode's round clock (a minus takes away).", inputs=[X, P("secs","number","Seconds",param="secs")], outputs=[E()], params=[prm("secs","Seconds","number",30,min=-7200,max=7200)])
node("clock.pause", "Pause Round Clock", "Round clock", "Stops or restarts the mode's round clock.", inputs=[X], outputs=[E()], params=[prm("do","Do","choice","pause",choices=[["pause","Pause"],["resume","Resume"]])])
node("clock.left", "Round Time Left", "Round clock", "Seconds left on the mode's round clock (0 when it isn't running).", outputs=[P("out","number","Seconds")], pure=True)
node("player.launch", "Launch Up", "Players", "Throws a living player straight up (with low gravity and No Fall Damage, a drop-in from the sky).", inputs=[X, P("player","player","Player",required=True), P("speed","number","Speed",param="speed")], outputs=[E()], params=[prm("speed","Speed","number",900,min=50,max=3000)])
node("event.time_up", "Round Time Up", "Events", "Runs when the mode's round clock reaches 0.", outputs=[E()], **ev)

node("flow.reroute", "Reroute", "Flow", "A bend in a wire, to keep the graph tidy. Passes on whatever comes in.", inputs=[P("in","any","")], outputs=[P("out","any","")], reroute=True)

data = {
  "version": 3,
  "pinTypes": {
    "exec": {"label": "Flow", "color": "#f2f2f2"},
    "player": {"label": "Player", "color": "#5ab0ff"},
    "entity": {"label": "Entity", "color": "#4dd0c8"},
    "team": {"label": "Team", "color": "#ffb347"},
    "number": {"label": "Number", "color": "#9be36a"},
    "bool": {"label": "Yes/No", "color": "#ff6b8a"},
    "text": {"label": "Text", "color": "#e0a3ff"},
    "any": {"label": "Anything", "color": "#9a9a9a"},
    "var": {"label": "Variable", "color": "#cccccc"}
  },
  "categories": ["Events", "Conditions", "Numbers", "Variables", "Flow", "Functions", "Timers", "Round clock", "Players", "Damage", "Squads", "Buying", "Saved", "Messages", "Game", "Scoring", "Bots", "Components", "Creatures"],
  "nodes": nodes,
}
json.dump(data, open(sys.argv[1], "w"), indent=1, ensure_ascii=False)
print(len(nodes), "nodes")
