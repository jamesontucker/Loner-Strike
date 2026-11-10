/*
	gm_botchat - bots talk like people on a public server.

	Every bot gets a chat personality from its name, so a regular keeps the same one
	every time it shows up:
	  f friendly    t tryhard    s salty    j joker
	  q quiet       n new player c chill    o old-timer
	  k kid         x English isn't their first language     h hype
	  z zoomer      p streamer wannabe   b backseat gamer    i the team's caller
	  d half asleep u blames themselves  w wholesome         r military roleplay
	  m meme lord   and in capitals: A anime fan, B Brit, C conspiracy theorist, D dad
	  gamer, E esports wannabe, F foodie, G grammar nerd, H hacker-accuser, L lurker,
	  M music guy, N stats nerd, O overconfident, P pacifist, S movement nerd,
	  T map tourist, Y yapper
	On top of that each bot has its own habits, also from its name: how chatty it is,
	its laugh (lol, lmao, xD, kkkk, jajaja, mdr, www...), a word it keeps using (bro,
	mate, dude...), text-speak or not, an emoticon, how often it makes typos and shouts.
	So no two bots in a lobby type alike, and a regular always types the same way.
	What they say comes from configs/gamemodes/botchat.ini: sections per event (a
	headshot, a knife kill, a teamkill, a round won, a map ending, someone saying gg...),
	each line marked with the personalities that would say it. A line can ask for a
	reply ("nice shot" gets "ty" or "lucky lol" from the one it was about), so short
	conversations happen. Bots answer people too: say a bot's name and that one answers.

	Moods: dying again and again tilts a bot (salty lines), and a long run of deaths
	makes it crash out: a few angry lines in a row, sometimes in capitals, and a salty
	one may rage quit (the game fills its slot). Friendly and chill bots cheer up a
	teammate who keeps dying, in team chat. A line starting with @ goes to team chat.

	They type like people: a delay that depends on how long the line is, lowercase and
	the odd typo for the casual ones, and not every time. Most of the time nobody says
	anything, like a real pub. How often is gm_botchat:
	  0 off, 1 now and then, 2 normal, 3 chatty
	gm_personality (0-200, percent, from gm_humanbots) scales it: at 0 bots only say gg
	and answer when spoken to, at 200 they talk about twice as much and lines that fit
	their personality come up more. Lines marked * are anyone's; lines marked with
	personality letters are the "characterful" ones.
	No repeats: a line said in the last while isn't picked again (short ones like "gg"
	or "lol" are fine), and a theme (noscopes, lag, hackers, the map...) rests for a few
	minutes once someone brings it up.
	In a match (and with the Matchmaking style on Classic) they talk less, callouts aside.
	Callouts: a bot just killed may tell its team where the killer was, with what, or that
	they're low ([callout], [callout_low]); the team-minded ones mostly.
	gm_botchat_stop 1: with Loner-Strike's game DLL, a bot typing a long line in the round
	now and then stands still for the last few seconds of it (not in deathmatch, a match, or
	modes a plugin steers bots through).
	Other cvars: gm_botchat_humans 1 (bots answer what people say: greetings, gg/gl,
	"ns", insults, "bots?", lag, "rush b"/"wait", their name. One bot answers, now and
	then two, and not every time; the one addressed, nearby or involved answers first)
	Answers use the game: "where is he" gets where their killer was or where the team last
	saw someone (the radar's place names, from the map's nav file), "wheres the bomb" who
	has it or where it's planted, "score?" the score, "timeleft" / "next map" / "what map"
	the real ones, "how" after dying what a teammate saw (the gun, a headshot, the place).
	Moods: lines can be marked for a bot that's tilted (-), on a streak (+), new (^) or one
	that has played the person before (=).
	Threads: when a person answers the bot that just spoke to them (within 20 seconds, by
	name, or quoting it), that bot carries on, two more lines at most; it stops when it's
	fighting or the round ends, and no other bot joins in.

	Clans: bots that share a clan tag ("[XyZ] Name", "XyZ|Name") are clanmates. They
	talk to each other in team chat ([clan]: a clanmate's kill, a clanmate joining),
	and back each other up when one keeps dying.

	Rivals: a player who keeps killing a bot this map becomes its nemesis ([nemesis],
	and [nemesis_won] when it finally gets them).
	Memory: bots remember the people they've played with on earlier days (data/vault
	gm_botmemory) and greet them like old faces ([remember], [remember_rival]).
	Leaving: a bot that leaves mid-map says so ([leave]); one that left may come back
	later ([rejoin]). Other plugins use the botchat_say / botchat_say_now / botchat_persona
	natives.

	Questions with real answers (teammates, in team chat; never in free for all): "money?"
	(what it has), "what gun u got", "hp?" / "u hurt?", "how many left", "where'd he go" (the
	place and how long ago; dead teammates were watching), "where u at" (its own place),
	"who has awp", "wheres the bomb", "time?". Requests a bot carries out, or doesn't, as it
	is (botchat-requests.ini): "cover me" / "follow me", "wait" / "hold", "go b" / "rush a" /
	"go mid" / "go long" (any place someone has stood in) / "go there" (where the person
	aims), "come here", "spray that wall", "jump", "crouch", "drop pls" (gm_botfun), "kys"
	(a toxic or goofy one now and then, only with rude bots on or a toxic lobby); "ok go" /
	"nvm" lets it go again. Warm and team-minded bots mostly do it, cold or tilted ones say
	no now and then, one in a fight says it's busy or "1 sec". Nothing silly in a match,
	nothing in modes a plugin steers. Text-speak and typos are read as the words people mean
	("wat u got", "wehre r they"), several things in one line work ("money? and cover me"),
	names can be shortened or misspelt a little, and "you" is the bot that just spoke.
	Follow-ups to the bot just asked: "where?", "how many?", "u sure?", "ok go".
	gm_botchat_ask <userid> "<text>" [team]: as if that person said it (voice, later).

	Placeholders in lines: {name} the bot, {other} who it's about or answering,
	{map} the map, {tag} the bot's clan tag; in answers {money}, {gun}, {hp}, {left},
	{ago}, {site}, {place}. Conditions also take mode=<mode id> (a prefix: mode=csdm is both
	deathmatches), round=pistol|eco|force|full|last, enemies<n> (enemies alive),
	clanmate=1. Edit botchat.ini to add your own; gm_botchat_reload re-reads it.
*/

#include <amxmodx>
#include <amxmisc>
#include <nvault>
#include <cstrike>
#include <fakemeta>
#include <hamsandwich>
// sockets (optional, for gm_ai_url): without the module the plugin still loads, the
// feature just stays off (module_filter / native_filter)
#include <sockets>

// gm_community: true for chat that never shows (muted player, filtered out)
native gm_chat_blocked(id, const text[])
native gm_voice_blocked(id, const text[])

// gm_people (optional): who the bots are and what they remember. people_match: 1 the
// condition holds, 0 it doesn't, -1 it doesn't know the key
native people_match(id, other, const key[], const op[], const value[])
native people_letter(id)
native people_hobby(id, which, out[], len)
native people_note(bot, other, const kind[])
native people_get(id, const key[])
native people_mood()
native people_name(id, out[], len)

// gm_humanbots (optional): the bot is afk; the enemy it has in view (0 none)
native hb_bot_afk(id)
native hb_bot_target(id)

// gm_botfun (optional): danger around the bot (shots, someone hurt, an enemy seen lately);
// a drop for someone who asked (1: one's on its way)
native bf_danger(id)
native bf_ask_drop(asker, want, prefer)
// gm_humanbots (optional): the bot sprays its logo there (normal all 0: a wall it finds)
native hb_spray(id, const Float:at[3], const Float:normal[3])
// gm_humanbots (optional): its own hold, crouch and afk on the bot end (a person's order comes first)
native hb_release(id)

#define PLUGIN  "Bot chat"
#define VERSION "1.0"
#define AUTHOR  "gamemodes"

#define MAX_SECTIONS 400
#define TASK_SAY     8100
#define TASK_STOP    8200                 // standing still to type (+ slot)
#define TASK_RAGEQUIT 2100000             // a rage quit (+ userid)
#define MAX_PENDING  16
#define MAX_CHAIN    3

new g_pAskTest		// gm_botchat_test (the bot simulation only)
new const PERSONAS[] = "ftsjqncokxhzpbiduwrmABCDEFGHLMNOPSTYIJKQRUVWXZ0123aeglvy456789"
// how much each personality talks (same order as PERSONAS)
new const Float:CHATTY[] = { 1.0, 0.8, 1.2, 1.4, 0.3, 1.0, 0.7, 0.8, 1.5, 0.8, 1.6, 1.2, 1.3, 1.3, 1.1, 0.6, 0.9, 1.0, 1.0, 1.3,
	1.1, 1.0, 1.1, 0.8, 1.0, 1.0, 1.0, 1.2, 0.15, 1.0, 1.0, 1.3, 0.9, 1.0, 1.0, 1.6,
	1.0, 1.0, 1.2, 1.0, 0.9, 1.0, 1.0, 0.8, 1.2, 0.6, 1.0, 1.0, 1.0, 1.1,
	1.0, 1.1, 1.0, 1.1, 1.3, 1.0, 0.6, 0.9, 0.4, 1.2, 1.2, 0.9 }
// how common each personality is (per 1000; same order): most people on a pub are
// ordinary players who say little; the gimmicks (pirates, anime fans, streamers...) are
// left out, they read like someone wrote them
new const WEIGHT[] = { 90, 70, 70, 35, 120, 60, 100, 70, 12, 40, 5, 0, 0, 20, 30, 30, 35, 10, 0, 0,
	0, 25, 0, 20, 3, 0, 3, 15, 110, 0, 5, 10, 0, 8, 0, 3,
	0, 0, 4, 0, 5, 0, 0, 0, 0, 0, 0, 0, 0, 2,
	0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 }

// habits, picked per bot from its name
// the first ones are for anyone; the last six only for people whose English isn't first
new const LAUGHS[][] = { "lol", "lol", "lol", "lol", "haha", "hehe", "lol", "xD", "hah", "rofl", "lmao", "haha", "kkkk", "jajaja", "mdr", "www", "555", "hehe" }
#define LAUGHS_ANYONE 12
new const WORDS[][] = { "", "", "", "", "", "", "", "", "man", "dude", "mate", "guys", "m8", "lads", "bro", "" }
new const EMOTES[][] = { "", "", "", "", "", ":)", ":D", "^^", ":P", ";)", ":/", ":(", "=)" }
new g_Laugh[33], g_Word[33], g_Emote[33], bool:g_TextSpeak[33], g_Typos[33], g_Shouty[33], Float:g_Chatty[33]

new g_pLevel, g_pHumans, g_pDeadChat, g_pStop
new bool:g_BotFun                   // gm_botfun is running (bf_danger)
new bool:g_Fork                     // Loner-Strike's game DLL (ls_bot)
new bool:g_MatchMode, bool:g_ClassicMode, bool:g_SteeredMode   // from the mode's id
new g_pPersonality                  // gm_personality (gm_humanbots), 0 if it isn't there
new g_CfgDir[128]
new Trie:g_Sections                 // section name -> Array of "letters|text"
new Array:g_Lists[MAX_SECTIONS], g_ListCount
new bool:g_Career

new g_Persona[33]                   // index into PERSONAS
new Float:g_NextBot[33]             // when this bot may talk again
new Float:g_NextAny                 // when any bot may start a new line
new Float:g_LastRound               // when the last round started (none in Gun Game or Deathmatch)
new g_RoundKills[33], bool:g_Streaked[33]
new g_DeathRow[33]                  // deaths since this player's last kill
new bool:g_Raged[33]
new Float:g_NextHeard
new g_KilledBy[33][33]
new g_WinRun[3], g_LoseRun[3]        // rounds won / lost in a row, per side               // this map: how often [killer][victim]
new g_Vault = INVALID_HANDLE
new Trie:g_Left                      // bots that left mid-map: name (without its clan tag) -> when
new Trie:g_LastMap                   // who was here at the end of the last map (bots by name, people by SteamID)
new bool:g_GunGame                   // the Gun Game mode
new g_FF[33][33], Float:g_FFAt[33][33]   // a person's hits on a teammate bot lately, and when
new g_GGKnifeSaid[33]                // the life (spawn count) we last called out that this one's on the knife
new Float:g_TypeHoldUntil[33], Float:g_TypeBlocked[33]
new bool:g_MapEnding

// lines said lately (hashes), so the same one doesn't come round again soon
#define RECENT 160
new g_Recent[RECENT], g_RecentPos

// themes rest for a while once someone brings them up: a word in the line -> theme
new const THEME_WORD[][] = { "noscope", "no scope", "scope", "lag", "laggy", "hitreg", "reg", "choke", "ping",
	"hack", "hacks", "hacking", "aimbot", "sus", "demo", "sleep", "tired", "bed", "3am", "zzz",
	"this map", "map again", "{map}", "mouse", "fps", "camp", "camper", "camping", "awp", "1.5", "beta", "old" }
new const THEME_ID[] = { 0, 0, 0, 1, 1, 1, 1, 1, 1,
	2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3,
	4, 4, 4, 5, 5, 6, 6, 6, 7, 8, 8, 8 }
// seconds each theme rests: noscope, lag, hackers, sleep, the map, gear, camping, awp, the old days
new const Float:THEME_REST[] = { 900.0, 150.0, 240.0, 600.0, 300.0, 600.0, 180.0, 90.0, 900.0 }
new Float:g_ThemeAt[sizeof THEME_REST]

// who's involved, for answering people: the last bot to get a kill, who killed each
// person last and whom they killed last
new g_LastBotKiller, Float:g_LastBotKillAt
new g_KillerOf[33], Float:g_KillerOfAt[33], g_VictimOf[33], Float:g_VictimOfAt[33]
// answers lately: the more bots have answered, the less they answer the next thing
new g_HeardRun, Float:g_HeardRunAt

// Game state, for answers that use it (empty when the game doesn't say)
new g_Place[33][32]                  // where each player last was: the map's place names (CZ navs)
new g_KillerPlace[33][32]            // how each person last died: where the killer was,
new g_KillWeapon[33][16], bool:g_KillHs[33], Float:g_DiedAt[33]   // with what, a headshot, when
new g_SeenPlace[3][32], Float:g_SeenAt[3]  // where each team last saw an enemy (a teammate's killer)
new g_BombPlace[32], bool:g_BombPlanted, bool:g_BombDropped
new g_Score[3]                       // team scores: 1 T, 2 CT
new Float:g_FightAt[33]              // last time a player hurt someone or got hurt
new Float:g_JoinedAt[33]
new Float:g_FillerAt                 // last "?", "idk", "ya": they read as robotic when they come often
// What pick_line may put in a line ({place} {weapon} {carrier} {hs}); a line asking for one
// that's empty isn't picked
new g_CtxPlace[32], g_CtxWeapon[16], g_CtxCarrier[32], bool:g_CtxHs, g_CtxOther
// ... and for answers to questions: the bot's money, gun, health, enemies left, how long ago
// something was seen, where it's going
new g_CtxMoney[16], g_CtxGun[24], g_CtxHp[8], g_CtxLeft[8], g_CtxAgo[16], g_CtxSite[16]

// ---- questions and requests (state) ----
#define TASK_WATCH    8490                // who sees whom, who stands still (once a second)
#define TASK_DEADTALK 8491                // the dead chatting now and then
#define TASK_REQ      8500                // a request tried again after "1 sec" (+ bot)
#define TASK_SPRAYGO  8540                // walking over to spray where someone pointed (+ bot)
#define TASK_KYS      8580                // "kys": typed, then done (+ bot)
new g_Shots[33], Float:g_ShotAt[33], Float:g_WhiffAt[33]   // shots since its last hit; a whiffed spray
// what each bot first saw this round (two or more together, or one at a site), for a callout
new g_SpotSeenRound[33], g_SpotRound[33], Float:g_SpotSawAt[33], g_SpotN[33], g_SpotPlace[33][32]
new g_ModeId[32], bool:g_BuyMode, bool:g_DmMode, bool:g_PeopleLib            // the mode's id; buying is normal here
new g_BotSawPlace[33][32], Float:g_BotSawAt[33]   // where this bot last had an enemy in view, when
new g_LastSpoke, Float:g_LastSpokeAt        // the last bot to say something
new g_AskBot[33], g_AskIntent[33], Float:g_AskAt[33]  // per person: the bot they last asked, what, when
// what a bot was asked to do: the request, by whom, tries left after "1 sec", where to
new g_ReqAct[33], g_ReqFor[33], g_ReqForUser[33], g_ReqArg[33], g_ReqTries[33], g_ReqPlace[33][32], bool:g_ReqTeam[33]
#define TASK_REQCHECK 8620                 // did it take? (+ bot)
#define TASK_REQLIVE  8660                 // asked in freeze time: once the round starts (+ bot)
// what a bot was asked: say the "ok" line when it takes, say "1 sec" / "cant rn" when it doesn't;
// the first try or a later one; asked in freeze time, waiting for the round
#define ITEM_KIT 99                         // a defuse kit (not a weapon: cs_get_user_defuse)

// gm_botchat_test 2: how each line was understood, printed on one line (the simulation reads it)
new g_TestAsker                             // the bot standing in as a person (gm_botchat_ask)
new bool:g_Tracing, g_TrIntents[64], g_TrTarget[64], g_TrSection[64], g_TrDecision[64], g_TrReply[192]
new bool:g_Direct                          // the line asked straight out ("?", "how many", "where")
new g_ReqPrevFor[33], g_ReqPrevUser[33], g_ReqSite[33][16]   // whose order this one replaced; where it's going
new bool:g_WasBusy[33]                      // typing (or waiting on the model) when the line came in
new bool:g_ReqSayYes[33], bool:g_ReqSayFail[33], bool:g_ReqFirst[33], bool:g_ReqWaitLive[33], g_ReqRound[33]
new Float:g_CheckStart[33], Float:g_CheckDist[33]
new g_SiteLetter[3]                         // each bomb site's letter ('a', 'b', 'c')
new bool:g_SiteFixed[2]                     // a and b placed by the nav's names
new Float:g_SprayAt[33][3], Float:g_SprayNormal[33][3], Float:g_SprayUntil[33]
new Float:g_Sites[3][3], g_SiteCount        // the bomb sites' middles, A first
new Float:g_SpawnT[3], Float:g_SpawnCT[3], bool:g_HaveT, bool:g_HaveCT
new Trie:g_PlaceAt                          // a place as people say it -> where someone stood in it
new Trie:g_PlaceWord                        // a place's telling word -> the place ("long" -> "long a"; "" two share it)
new bool:g_NavPlaces                        // the nav names its places (Location messages come)
new bool:g_NoReleaseNative, bool:g_NoPeopleName
new g_Bare[33][32]                          // each player's name without its tag (bare_of)
#define TASK_FORGET 8700                    // a slot emptied: its name forgotten (+ slot)
new Float:g_GreetAt, g_GreetCount, g_GreetCap   // hellos lately (a lobby filling up all at once)
new bool:g_NoDropNative, bool:g_NoSprayNative, bool:g_NoDangerNative, bool:g_NoTargetNative, bool:g_NoAfkNative
new bool:g_NoVoiceBlocked                   // gm_community is older: no gm_voice_blocked
new Float:g_KysAt                           // the last "kys" a bot went along with
new bool:g_KysDone[33]
new g_AiAction[33][12]                      // what the model said the bot should do
// how people play, for the bots to notice: awp kills this map, standing still since,
// bunnyhops seen, kills this round, the last thing said about them (a bit per kind)
new g_AwpKills[33], Float:g_StillSince[33], Float:g_StillAt[33][3], g_Bhops[33], g_SaidAbout[33]
new Float:g_SpamAt[33][8], g_SpamN[33], Float:g_SpamUntil[33]   // someone typing a lot
new Float:g_NamedNext[33]                   // a bot answered by name just now: not again at once
new g_FlashBy[3], Float:g_FlashAt[3]        // the last flash thrown, per team, and when
new g_RoundNo, g_LastTeam[33]               // rounds this map; each one's side last round
new bool:g_Overtime
// the last few kills and deaths, for the model: who, whom, with what
#define EVENTS 6
new g_EvKiller[EVENTS], g_EvVictim[EVENTS], g_EvText[EVENTS][48], Float:g_EvAt[EVENTS], g_EvPos
// A short thread with a person (per person): the bot talking to them, what about, how many
// lines it has said in it, when it last spoke and what it said
new g_ThreadBot[33], g_ThreadTopic[33][24], g_ThreadTurns[33], Float:g_ThreadAt[33], g_ThreadLine[33][128]

// lines waiting to be "typed"
new g_PendBot[MAX_PENDING], g_PendUser[MAX_PENDING], g_PendOther[MAX_PENDING], g_PendChain[MAX_PENDING], bool:g_PendTeam[MAX_PENDING]
new g_PendText[MAX_PENDING][128], g_PendSec[MAX_PENDING][24], bool:g_PendQuiet[MAX_PENDING]   // quiet: starts no thread
new bool:g_PendRaw[MAX_PENDING]     // the model's text: said as it is
new bool:g_PendAlive[MAX_PENDING]   // alive when it started typing (dying cuts it off)
new g_PendWaited[MAX_PENDING]       // times put off already (it was in a fight)
new bool:g_HumanBots                // gm_humanbots is running

// smarter chat (gm_ai_url): one request per bot
#define TASK_AI    2000000
#define AI_TIMEOUT 4.5
#define HIST       12
new g_pAiUrl, bool:g_Sockets
new g_AiSock[33], g_AiState[33]     // state: 0 none, 1 connecting, 2 waiting for the answer
new Float:g_AiStart[33], g_AiUser[33], g_AiHuman[33], g_AiHumanUser[33], bool:g_AiTeam[33], Float:g_AiExtra[33], g_AiRound[33]
new g_AiFallback[33][128], g_AiSection[33][32], g_AiReq[33][3072], g_AiSent[33], g_AiResp[33][1024], g_AiGot[33]
new g_Round                         // counts rounds, so an answer from last round is dropped
new g_Hist[HIST][100], g_HistTeam[HIST], g_HistPos, g_HistCount   // recent chat
new g_HumanMsg[128]                 // what the person typed, as they typed it

// The rules over lines (docs/bot-people.md): a line's conditions say who would say it and
// when; the most specific one that fits wins. Per-line cooldowns, once-a-map lines, and
// flags lines set for a while
new Trie:g_LineAt, Trie:g_LineOnce, Trie:g_Flags
new bool:g_Freeze, bool:g_RoundOver
new g_PickCond[96], g_PickBase[128], g_PickScore     // the line pick_line chose last
#define MAX_CANDIDATES 256
#define BOT_RECENT     24                     // a bot doesn't say the same longer line twice in a while
new g_BotRecent[33][BOT_RECENT], g_BotRecentPos[33]

// The director: bots that would say something about the same moment make an offer; one
// of them speaks, now and then a second, almost never a third (silence is the default)
#define MAX_OFFERS    16
#define TASK_DIRECTOR 8290
#define TASK_BLOCK   8295                 // who's in whose way (once a second)
#define TASK_TBAG    8400                 // watching a killer over a bot's body (+ the victim)
#define TASK_TYPEHOLD 8440                // a bot stopped to type: still not in a teammate's way (+ the bot)
#define TASK_GGLEAD  8480                 // Gun Game: someone on the last level
#define TASK_NAV     8481                 // no nav for this map yet: a hint
new g_OfferBot[MAX_OFFERS], g_OfferOther[MAX_OFFERS], g_OfferText[MAX_OFFERS][128], g_OfferBase[MAX_OFFERS][128]
new g_OfferCond[MAX_OFFERS][96], g_OfferSec[MAX_OFFERS][24], bool:g_OfferTeam[MAX_OFFERS], Float:g_OfferDelay[MAX_OFFERS]
new Float:g_OfferBid[MAX_OFFERS], g_OfferCount

// How much this person talks at all (gm_people's talk trait): the silent hardly ever, a
// spammer all the time
Float:talk_factor(id)
{
	if (!g_PeopleLib)
		return 1.0
	static const Float:TALK[] = { 0.12, 0.5, 1.0, 1.6, 2.4 }
	new t = people_get(id, "talk")
	return (t >= 0 && t < sizeof TALK) ? TALK[t] : 1.0
}

// When people talk: dead, in freeze time and at round end (the dead commentate). Alive in
// the middle of a round only for something that matters
Float:phase_factor(id, const section[])
{
	if (!is_user_alive(id) || g_Freeze || g_RoundOver)
		return 1.4
	// deathmatch has no quiet phase: kill and death reactions are the chat; a callout is what
	// one says mid-round
	if (dm_like() || equal(section, "callout", 7))
		return 1.0
	static const BIG[][] = { "clutch", "ace", "teamkilled", "teamkill", "nemesis", "nemesis_won", "rage", "last_alive", "low_hp", "bomb_planted", "defuse_ninja" }
	for (new i = 0; i < sizeof BIG; i++)
		if (equal(section, BIG[i]))
			return 1.0
	return 0.45
}

// No more than 2 / 4 / 6 new bot lines and follow-ups in any 30 seconds (gm_botchat 1 / 2 / 3;
// personality only shortens the gaps, it never lifts this). Answers to a person who spoke to
// a bot still go through
#define CAP_LINES  6
#define CAP_WINDOW 30.0
new Float:g_CapAt[CAP_LINES], g_CapPos

bool:rate_capped(Float:now)
{
	new cap = clamp(get_pcvar_num(g_pLevel) * 2, 2, CAP_LINES), n
	for (new i = 0; i < CAP_LINES; i++)
		if (g_CapAt[i] > 0.0 && now - g_CapAt[i] < CAP_WINDOW)
			n++
	return n >= cap
}

// At most three lines a minute from any one bot, however chatty
new Float:g_BotLineAt[33][3], g_BotLinePos[33]

bool:bot_capped(id, Float:now)
{
	for (new i = 0; i < 3; i++)
		if (g_BotLineAt[id][i] <= 0.0 || now - g_BotLineAt[id][i] >= 60.0)
			return false
	return true
}

bot_line(id, Float:now)
{
	g_BotLineAt[id][g_BotLinePos[id]] = now
	g_BotLinePos[id] = (g_BotLinePos[id] + 1) % 3
}

cap_count(Float:now)
{
	g_CapAt[g_CapPos] = now
	g_CapPos = (g_CapPos + 1) % CAP_LINES
}

// After a line, who might answer it and from where: the one it's about (a bot), a teammate,
// or anyone. Most lines get no answer; a reply comes quickly or not at all
follow_up(id, other, const section[], bool:team)
{
	static const FROM[][] = { "kill", "kill_headshot", "kill_knife", "kill_nade", "kill_awp", "noscope",
		"death", "death_headshot", "death_knife", "death_nade", "nice_shot", "round_win", "round_lose",
		"greet", "idle", "late_night", "map_end", "teamkilled" }
	static const TO[][] = { "r_kill", "r_kill", "r_kill", "r_kill", "r_kill", "r_kill",
		"r_death", "r_death", "r_death", "r_death", "r_ns_thanks", "r_round", "r_round",
		"r_hi", "r_idle", "r_idle", "r_gg", "r_sorry_tk" }
	// 1 the one it's about, 2 a teammate, 3 anyone
	static const WHO[] = { 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 3, 3, 3, 1 }
	static const Float:ODDS[] = { 0.45, 0.45, 0.6, 0.45, 0.45, 0.45, 0.45, 0.45, 0.55, 0.45, 0.65, 0.5, 0.5, 0.55, 0.5, 0.5, 0.65, 0.75 }
	for (new i = 0; i < sizeof FROM; i++)
	{
		if (!equal(section, FROM[i]))
			continue
		if (random_float(0.0, 1.0) >= ODDS[i])
			return
		new who
		switch (WHO[i])
		{
			case 1: who = (other && is_user_connected(other) && is_user_bot(other) && other != id) ? other : 0
			case 2: who = any_bot_on(get_user_team(id), id)
			default: who = team ? any_bot_on(get_user_team(id), id) : any_bot(id)
		}
		// team chat stays in the team
		if (who && team && get_user_team(who) != get_user_team(id))
			who = 0
		if (who)
			set_task(random_float(0.3, 1.2), "task_follow_up", who, fmt("%d %d %s", id, team, TO[i]), 40)
		return
	}
}

public task_follow_up(const data[], who)
{
	new a[8], b[4], sec[24]
	parse(data, a, charsmax(a), b, charsmax(b), sec, charsmax(sec))
	new id = str_to_num(a)
	if (!is_user_connected(who) || !is_user_connected(id))
		return
	// a reply counts toward the cap too (a chain of them could otherwise flood the chat)
	new Float:now = get_gametime()
	if (rate_capped(now))
		return
	cap_count(now)
	want(who, sec, id, 1.0, -1.0, 1, str_to_num(b) == 1)
}

// ---- voice: people talking on the mic (Loner-Strike's engine tells us; dedicated servers) ----

new g_VoiceCount[33], Float:g_VoiceTotal[33], bool:g_VoiceLongSaid[33], bool:g_VoiceSpamSaid[33]

// gm_voice <index> start|still|stop <seconds> <bytes>
public cmd_voice()
{
	new arg[16]
	read_argv(1, arg, charsmax(arg))
	new id = str_to_num(arg)
	if (id < 1 || id > MaxClients || !is_user_connected(id) || is_user_bot(id) || g_Career || !get_pcvar_num(g_pLevel))
		return PLUGIN_HANDLED
	new what[8]
	read_argv(2, what, charsmax(what))
	read_argv(3, arg, charsmax(arg))
	new Float:secs = str_to_float(arg)
	if (equal(what, "start"))
	{
		g_VoiceLongSaid[id] = false
		return PLUGIN_HANDLED
	}
	if (equal(what, "still"))
	{
		// a long one: mic spam, once per go
		if (secs >= 20.0 && !g_VoiceLongSaid[id])
		{
			g_VoiceLongSaid[id] = true
			want(voice_listener(id), "voice_long", id, 0.45)
		}
		return PLUGIN_HANDLED
	}
	// stopped
	g_VoiceCount[id]++
	g_VoiceTotal[id] += secs
	new listener = voice_listener(id)
	if (!listener)
		return PLUGIN_HANDLED
	new Float:died = get_gametime() - g_DiedAt[id]
	if (g_VoiceTotal[id] >= 90.0 && g_VoiceCount[id] >= 6 && !g_VoiceSpamSaid[id])
	{
		g_VoiceSpamSaid[id] = true
		want(listener, "voice_spam", id, 0.5)
	}
	else if (!is_user_alive(id) && died < secs + 4.0 && secs >= 3.0)
		want(listener, "voice_rage", id, 0.3)
	else if (g_VoiceCount[id] == 1 && secs >= 1.5)
		want(listener, "voice_first", id, 0.3)
	else if (secs < 1.5)
		want(listener, "voice_short", id, 0.06)
	else if (!g_VoiceLongSaid[id])
		want(listener, "voice", id, 0.12)
	return PLUGIN_HANDLED
}

// A bot who could hear them: a teammate, unless everyone hears everyone (sv_alltalk)
voice_listener(id)
{
	if (get_cvar_num("sv_alltalk"))
		return any_bot(id)
	return any_bot_on(get_user_team(id), id)
}

// What a person said, for how the bot it's about (or every bot) feels about them
note_feeling(human, bot, const section[])
{
	if (!LibraryExists("gm_people", LibType_Library))
		return
	new kind[12]
	if (equal(section, "r_nice_from_human") || equal(section, "r_ns_thanks"))
		copy(kind, charsmax(kind), "praise")
	else if (equal(section, "r_sorry") || equal(section, "r_np"))
		copy(kind, charsmax(kind), "thanks")
	else if (equal(section, "r_hi") || equal(section, "r_wb") || equal(section, "r_gg") || equal(section, "r_gl"))
		copy(kind, charsmax(kind), "greet")
	else if (equal(section, "r_lol"))
		copy(kind, charsmax(kind), "joke")
	else if (equal(section, "r_noob") || equal(section, "r_ez") || equal(section, "r_trash"))
		copy(kind, charsmax(kind), "insult")
	else if (equal(section, "r_hack") || equal(section, "r_bot") || equal(section, "r_camping"))
		copy(kind, charsmax(kind), "accuse")
	else
		return
	if (bot)
	{
		people_note(bot, human, kind)
		return
	}
	if (equal(kind, "greet"))
		for (new b = 1; b <= MaxClients; b++)
			if (b != human && is_user_connected(b) && is_user_bot(b))
				people_note(b, human, kind)
}

// Inline choices: "{nice|ns|good one} man" says one of them
choose(text[], len)
{
	for (new guard = 0; guard < 8; guard++)
	{
		new open = -1, bar = -1, close = -1
		for (new i = 0; text[i]; i++)
		{
			if (text[i] == '{')
			{
				open = i
				bar = -1
			}
			else if (text[i] == '|' && open >= 0 && bar < 0)
				bar = i
			else if (text[i] == '}' && open >= 0)
			{
				if (bar > 0)
				{
					close = i
					break
				}
				open = -1
			}
		}
		if (close < 0)
			return
		// count the options and pick one
		new n = 1
		for (new i = open + 1; i < close; i++)
			if (text[i] == '|')
				n++
		new want = random(n), k, start = open + 1
		for (new i = open + 1; i <= close; i++)
		{
			if (text[i] == '|' || i == close)
			{
				if (k == want)
				{
					new pick[64], rest[128]
					copy(pick, min(i - start, charsmax(pick)), text[start])
					copy(rest, charsmax(rest), text[close + 1])
					text[open] = 0
					add(text, len, pick)
					add(text, len, rest)
					break
				}
				k++
				start = i + 1
			}
		}
	}
}

// gm_people (optional): traits, archetypes and memory. 1 the condition holds, 0 it doesn't,
// -1 it doesn't know the key
// (gm_people natives: declared at the top)

public plugin_init()
{
	register_plugin(PLUGIN, VERSION, AUTHOR)
	g_pLevel  = register_cvar("gm_botchat", "2")
	g_pHumans = register_cvar("gm_botchat_humans", "1")
	g_pDeadChat = register_cvar("gm_botchat_deadchat", "1")
	g_pStop = register_cvar("gm_botchat_stop", "1")
	register_srvcmd("gm_botchat_reload", "cmd_reload")
	// Loner-Strike's engine (ReHLDS fork) runs this when a person starts and stops talking
	register_srvcmd("gm_voice", "cmd_voice")
	register_srvcmd("gm_botchat_ask", "cmd_ask")
	// (testing only, the bot simulation: with 1, a bot can stand in for the person asking)
	g_pAskTest = register_cvar("gm_botchat_test", "0")
	// the app sets it to "127.0.0.1:port" while its model is ready, "" when not
	g_pAiUrl = get_cvar_pointer("gm_ai_url")
	if (!g_pAiUrl)
		g_pAiUrl = register_cvar("gm_ai_url", "")

	register_event("DeathMsg", "ev_death", "a")
	register_event("SendAudio", "ev_twin", "a", "2&%!MRAD_terwin")
	register_event("SendAudio", "ev_ctwin", "a", "2&%!MRAD_ctwin")
	register_event("SendAudio", "ev_draw", "a", "2&%!MRAD_rounddraw")
	// a teammate's flash: who threw it (the grenade gets its model), who it blinded
	register_forward(FM_SetModel, "fw_setmodel", 1)
	register_event("ScreenFade", "ev_fade", "be", "4=255", "5=255", "6=255", "7>199")
	// who sees whom, who stands still, who bunnyhops (once a second)
	set_task(1.0, "task_watch", TASK_WATCH, _, _, "b")
	set_task(30.0, "task_dead_talk", TASK_DEADTALK, _, _, "b")
	register_event("HLTV", "ev_newround", "a", "1=0", "2=0")
	set_task(40.0, "task_idle_chatter")
	set_task(1.0, "task_blocked", TASK_BLOCK, _, _, "b")
	register_event("30", "ev_intermission", "a")
	register_logevent("ev_planted", 3, "2=Planted_The_Bomb")
	register_logevent("ev_defused", 3, "2=Defused_The_Bomb")
	// who is defusing or planting right now (a death in the middle of it says so)
	register_logevent("ev_defuse_begin", 3, "2=Begin_Bomb_Defuse_Without_Kit")
	register_logevent("ev_defuse_begin", 3, "2=Begin_Bomb_Defuse_With_Kit")
	RegisterHam(Ham_Weapon_PrimaryAttack, "weapon_c4", "fw_c4_attack", 1)
	// a bot's shots, for a spray that hit nothing ("wtf my aim")
	static const GUNS[][] = { "weapon_ak47", "weapon_m4a1", "weapon_famas", "weapon_galil", "weapon_aug", "weapon_sg552",
		"weapon_mp5navy", "weapon_p90", "weapon_ump45", "weapon_mac10", "weapon_tmp", "weapon_m249", "weapon_glock18",
		"weapon_usp", "weapon_p228", "weapon_fiveseven", "weapon_elite", "weapon_deagle", "weapon_m3", "weapon_xm1014" }
	for (new i = 0; i < sizeof GUNS; i++)
		RegisterHam(Ham_Weapon_PrimaryAttack, GUNS[i], "fw_shot", 1)
	RegisterHamPlayer(Ham_TakeDamage, "fw_ff_post", 1)
	register_logevent("ev_bomb_dropped", 3, "2=Dropped_The_Bomb")
	register_logevent("ev_bomb_got", 3, "2=Got_The_Bomb")
	register_logevent("ev_bomb_got", 3, "2=Spawned_With_The_Bomb")
	// where people are (the place names on the radar, from the map's nav file), the score,
	// who's fighting
	if (get_user_msgid("Location"))
		register_event("Location", "ev_location", "ab")
	register_event("TeamScore", "ev_teamscore", "a")
	register_event("Damage", "ev_damage", "b", "2!0")
	register_clcmd("say", "cmd_say")
	register_clcmd("say_team", "cmd_say_team")

	register_logevent("ev_round_live", 2, "1=Round_Start")

	get_configsdir(g_CfgDir, charsmax(g_CfgDir))
	g_Sections = TrieCreate()
	g_LineAt = TrieCreate()
	g_LineOnce = TrieCreate()
	g_Flags = TrieCreate()
	g_PlaceAt = TrieCreate()
	g_PlaceWord = TrieCreate()
	load_lines()
}

new Float:g_RoundLiveAt      // freeze time ended then (everyone's still in spawn just after)

public ev_round_live()
{
	g_Freeze = false
	g_RoundLiveAt = get_gametime()
	start_waiting_requests()
}

new Float:g_DefuseAt[33], Float:g_PlantAt[33]

public ev_defuse_begin()
{
	new id = get_loguser_index()
	if (id)
		g_DefuseAt[id] = get_gametime()
}

// the C4's trigger held: planting (the game calls this every frame while it's held)
public fw_c4_attack(ent)
{
	new id = get_ent_data_entity(ent, "CBasePlayerItem", "m_pPlayer")
	if (1 <= id <= MaxClients)
		g_PlantAt[id] = get_gametime()
}

// How a kill happened, for what's said about it: through a wall (the killer couldn't see the
// one it killed: something solid in the way of both the middle of the body and a little
// above it; glass and other things that break don't count, a kill through a window isn't a
// wallbang), the victim in the middle of defusing (most of the way through, or not) or
// planting, or reloading
enum { HOW_PLAIN, HOW_WALL, HOW_DEFUSE, HOW_DEFUSE_CLOSE, HOW_PLANT, HOW_RELOAD }

// What a trace stopped at counts as a wall: the world, or a solid entity (a door) that isn't
// glass or something breakable
bool:wall_hit(tr)
{
	new Float:fr
	get_tr2(tr, TR_flFraction, fr)
	if (fr >= 1.0)
		return false
	new hit = get_tr2(tr, TR_pHit)
	if (hit <= 0 || !pev_valid(hit))
		return true
	new cls[32]
	pev(hit, pev_classname, cls, charsmax(cls))
	return contain(cls, "breakable") == -1 && pev(hit, pev_rendermode) == kRenderNormal
}

how_killed(killer, victim, const weapon[])
{
	new Float:now = get_gametime()
	if (now - g_DefuseAt[victim] < 10.5 && (pev(victim, pev_button) & IN_USE) && g_BombPlanted)
	{
		// how far along: 5 s with a kit, 10 without
		new Float:takes = cs_get_user_defuse(victim) ? 5.0 : 10.0
		return (now - g_DefuseAt[victim]) / takes >= 0.6 ? HOW_DEFUSE_CLOSE : HOW_DEFUSE
	}
	if (now - g_PlantAt[victim] < 0.4)
		return HOW_PLANT
	if (!equal(weapon, "knife") && !equal(weapon, "grenade") && is_user_alive(killer))
	{
		new Float:from[3], Float:ofs[3], Float:to[3]
		pev(killer, pev_origin, from)
		pev(killer, pev_view_ofs, ofs)
		from[2] += ofs[2]
		pev(victim, pev_origin, to)
		new tr = create_tr2()
		engfunc(EngFunc_TraceLine, from, to, IGNORE_MONSTERS, killer, tr)
		new bool:w1 = wall_hit(tr)
		to[2] += 20.0
		engfunc(EngFunc_TraceLine, from, to, IGNORE_MONSTERS, killer, tr)
		new bool:w2 = wall_hit(tr)
		free_tr2(tr)
		if (w1 && w2)
			return HOW_WALL
	}
	new gun = get_ent_data_entity(victim, "CBasePlayer", "m_pActiveItem")
	if (gun > 0 && pev_valid(gun) && get_ent_data(gun, "CBasePlayerWeapon", "m_fInReload"))
		return HOW_RELOAD
	return HOW_PLAIN
}

public plugin_natives()
{
	register_library("gm_botchat")
	register_native("botchat_persona", "n_persona")
	register_native("botchat_say", "n_say")
	register_native("botchat_say_now", "n_say_now")
	register_native("botchat_left", "n_left")
	// still loads without gm_community, and without the sockets module
	set_native_filter("native_filter")
	set_module_filter("module_filter")
}

public module_filter(const library[], LibType:type)
{
	return equal(library, "sockets") ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

public native_filter(const name[], index, trap)
{
	// (an older gm_botfun or gm_humanbots: those two aren't there, so they're never called)
	if (!trap && equal(name, "bf_ask_drop"))
		g_NoDropNative = true
	if (!trap && equal(name, "hb_spray"))
		g_NoSprayNative = true
	if (!trap && equal(name, "people_name"))
		g_NoPeopleName = true
	if (!trap && equal(name, "hb_release"))
		g_NoReleaseNative = true
	if (!trap && equal(name, "bf_danger"))
		g_NoDangerNative = true
	if (!trap && equal(name, "hb_bot_target"))
		g_NoTargetNative = true
	if (!trap && equal(name, "hb_bot_afk"))
		g_NoAfkNative = true
	if (!trap && (equal(name, "people_", 7) || equal(name, "hb_", 3) || equal(name, "bf_", 3)))
		return PLUGIN_HANDLED
	// (an older gm_community without the voice filter: voice goes unfiltered there)
	if (!trap && equal(name, "gm_voice_blocked"))
	{
		g_NoVoiceBlocked = true
		return PLUGIN_HANDLED
	}
	return (!trap && (equal(name, "gm_chat_blocked") || equal(name, "socket_", 7))) ? PLUGIN_HANDLED : PLUGIN_CONTINUE
}

// botchat_persona(id): the bot's personality letter (0 for people)
public n_persona(plugin, params)
{
	new id = get_param(1)
	return (id >= 1 && id <= MaxClients && is_user_bot(id)) ? PERSONAS[g_Persona[id]] : 0
}

// botchat_say(id, const section[], other, Float:chance): a line from `section`, maybe
public n_say(plugin, params)
{
	new id = get_param(1), section[32]
	if (id < 1 || id > MaxClients || !is_user_connected(id) || !is_user_bot(id))
		return 0
	get_string(2, section, charsmax(section))
	strtolower(section)
	new Float:chance = get_param_f(4)
	if (equal(section, "leave"))
	{
		// remembered, so it can come back later
		new name[32]
		bare_of(id, name, charsmax(name))
		TrieSetCell(g_Left, name, floatround(get_gametime()))
		// about half say bye (fewer at a low gm_personality); the rest just go
		new text[128]
		if (get_pcvar_num(g_pLevel) && !g_Career && random_float(0.0, 100.0) <= 15.0 + 30.0 * floatmin(personality(), 1.5)
			&& pick_line(id, "leave", text, charsmax(text), true))
			queue(id, 0, text, MAX_CHAIN, false, random_float(0.3, 1.2))
		return 1
	}
	want(id, section, get_param(3), chance)
	return 1
}

// botchat_left(id): this bot is leaving (gm_pubserver making room: it says its own bye),
// remembered so it can come back later ("wb")
public n_left(plugin, params)
{
	new id = get_param(1)
	if (id < 1 || id > MaxClients || !is_user_connected(id) || !is_user_bot(id))
		return 0
	new name[32]
	bare_of(id, name, charsmax(name))
	TrieSetCell(g_Left, name, floatround(get_gametime()))
	return 1
}

// botchat_say_now(id, const section[], other, bool:team): a line from `section`, typed now
// (no waiting its turn, no chattiness roll); true if it's on its way. For things that
// happen because it was said (gm_botfun: "drop pls")
public n_say_now(plugin, params)
{
	new id = get_param(1), section[32]
	if (id < 1 || id > MaxClients || !is_user_connected(id) || !is_user_bot(id) || g_Career || !get_pcvar_num(g_pLevel) || bot_afk(id))
		return 0
	get_string(2, section, charsmax(section))
	strtolower(section)
	new other = get_param(3), text[128]
	g_CtxOther = other
	new bool:ok = pick_line(id, section, text, charsmax(text))
	g_CtxOther = 0
	if (!ok)
		return 0
	queue(id, other, text, MAX_CHAIN - 1, get_param(4) != 0, -1.0, section, true)
	return 1
}

public plugin_cfg()
{
	personality_cvar()
	ai_init()
	for (new i = 0; i < sizeof g_ThemeAt; i++)
		g_ThemeAt[i] = -10000.0
	g_Vault = nvault_open("gm_botmemory")
	g_Left = TrieCreate()
	g_LastMap = TrieCreate()
	load_lastmap()
	g_HumanBots = bool:LibraryExists("gm_humanbots", LibType_Library)
	g_BotFun = bool:LibraryExists("gm_botfun", LibType_Library)
	g_Fork = get_cvar_pointer("bot_ls_routes") != 0
	// the mode: matches talk less; nobody stops to type where a plugin steers the bots
	new mode[32], path[128]
	get_datadir(path, charsmax(path))
	new f = fopen(fmt("%s/gamemodes/current.txt", path), "rt")
	if (f)
	{
		fgets(f, mode, charsmax(mode))
		trim(mode)
		fclose(f)
	}
	copy(g_ModeId, charsmax(g_ModeId), mode)
	g_DmMode = equal(mode, "csdm", 4) || equal(mode, "gungame", 7)
	g_PeopleLib = bool:LibraryExists("gm_people", LibType_Library)
	g_BuyMode = !mode[0] || equal(mode, "classic") || equal(mode, "match") || equal(mode, "war3") || equal(mode, "shero")
	find_spots()
	g_MatchMode = bool:equal(mode, "match")
	g_GunGame = bool:equal(mode, "gungame") || bool:equal(mode, "gungame_team")
	if (g_GunGame)
		set_task(4.0, "task_gg_lead", TASK_GGLEAD, _, _, "b")
	// a map the bots haven't learned yet: they wander while they do; say so once
	new map[32]
	get_mapname(map, charsmax(map))
	if (!file_exists(fmt("maps/%s.nav", map)))
		set_task(20.0, "task_nav_hint", TASK_NAV)
	g_ClassicMode = bool:equal(mode, "classic")
	static const STEERED[][] = { "hns", "deathrun", "kreedz", "surf", "soccerjam", "zombie", "zp5", "basebuilder" }
	for (new i = 0; i < sizeof STEERED; i++)
		if (equal(mode, STEERED[i]))
			g_SteeredMode = true
	// Tour of Duty missions keep their designed bots
	new pass[64], team[8]
	get_cvar_string("sv_password", pass, charsmax(pass))
	get_cvar_string("humans_join_team", team, charsmax(team))
	g_Career = pass[0] && (equali(team, "CT") || equali(team, "T"))
}

public cmd_reload()
{
	TrieClear(g_Sections)
	for (new i = 0; i < g_ListCount; i++)
		ArrayDestroy(g_Lists[i])
	g_ListCount = 0
	load_lines()
	server_print("[botchat] %d sections", g_ListCount)
	return PLUGIN_HANDLED
}

// botchat.ini, then every botchat-*.ini beside it (sections in them add to the same ones)
load_lines()
{
	load_lines_from("botchat.ini")
	new dir[160], file[64]
	formatex(dir, charsmax(dir), "%s/gamemodes", g_CfgDir)
	new h = open_dir(dir, file, charsmax(file))
	if (!h)
		return
	do
	{
		if (equali(file, "botchat-", 8) && strlen(file) > 12 && equali(file[strlen(file) - 4], ".ini"))
			load_lines_from(file)
	}
	while (next_file(h, file, charsmax(file)))
	close_dir(h)
}

load_lines_from(const name[])
{
	new f = fopen(fmt("%s/gamemodes/%s", g_CfgDir, name), "rt")
	if (!f)
		return
	new line[192], Array:cur = Invalid_Array
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (!line[0] || line[0] == ';')
			continue
		if (line[0] == '[')
		{
			new name[48]
			copyc(name, charsmax(name), line[1], ']')
			strtolower(name)
			if (!TrieGetCell(g_Sections, name, cur))
			{
				if (g_ListCount >= MAX_SECTIONS)
				{
					cur = Invalid_Array
					continue
				}
				cur = g_Lists[g_ListCount++] = ArrayCreate(128)
				TrieSetCell(g_Sections, name, cur)
			}
			continue
		}
		if (cur != Invalid_Array && contain(line, "|") > 0)
			ArrayPushString(cur, line)
	}
	fclose(f)
}

// ---- who the bots are ----------------------------------------------------------------

public client_putinserver(id)
{
	g_Bare[id][0] = 0
	g_VoiceCount[id] = 0
	g_VoiceTotal[id] = 0.0
	g_VoiceLongSaid[id] = g_VoiceSpamSaid[id] = false
	g_NextBot[id] = get_gametime() + random_float(5.0, 20.0)
	g_RoundKills[id] = 0
	g_DeathRow[id] = 0
	g_KillerOf[id] = g_VictimOf[id] = 0
	g_Raged[id] = false
	g_Place[id][0] = g_KillerPlace[id][0] = 0
	g_DiedAt[id] = g_FightAt[id] = -100.0
	g_JoinedAt[id] = get_gametime()
	g_ThreadBot[id] = 0
	g_AskBot[id] = g_ReqAct[id] = g_ReqFor[id] = 0
	g_BotSawAt[id] = g_StillSince[id] = -100.0
	g_BotSawPlace[id][0] = 0
	g_AwpKills[id] = g_Bhops[id] = 0
	g_SaidAbout[id] = 0
	g_SpamN[id] = 0
	for (new k = 1; k <= MaxClients; k++)
		g_KilledBy[id][k] = g_KilledBy[k][id] = 0
	if (is_user_bot(id))
	{
		g_Persona[id] = persona_of(id)
		habits_of(id)
		new name[32], when
		bare_of(id, name, charsmax(name))
		if (TrieGetCell(g_Left, name, when))
		{
			TrieDeleteKey(g_Left, name)
			want(id, "rejoin", 0, 0.8, random_float(3.0, 8.0), 1)
			return
		}
		// (on the last map too: just a map change, no hello again, and not "new" either)
		if (TrieKeyExists(g_LastMap, name))
		{
			g_JoinedAt[id] = get_gametime() - 1000.0
			return
		}
		if (random_num(1, 100) <= 30 && greet_slot())
			want(id, "greet", 0, 0.5, random_float(2.0, 8.0) + 2.0 * float(g_GreetCount))
		// a clanmate already here says hi to them
		set_task(random_float(6.0, 15.0), "task_clan_hello", id)
	}
	else if (!is_user_hltv(id))
	{
		new auth[40]
		person_key(id, auth, charsmax(auth))
		if (!TrieKeyExists(g_LastMap, auth))
			set_task(random_float(5.0, 12.0), "task_welcome", id)
	}
}

// A hello may be said: two or three at most when a lobby fills up all at once (the first person
// joining, "Game Commencing"), then quiet for a bit
bool:greet_slot()
{
	new Float:now = get_gametime()
	if (now - g_GreetAt > 12.0)
	{
		g_GreetAt = now
		g_GreetCount = 0
		g_GreetCap = random_num(2, 3)
	}
	if (g_GreetCount >= g_GreetCap)
		return false
	g_GreetCount++
	return true
}

public task_clan_hello(id)
{
	new mate = is_user_connected(id) ? clanmate_bot(id, false) : 0
	if (mate && greet_slot())
		want(mate, "clan_join", id, 0.6)
}

public task_welcome(id)
{
	if (!is_user_connected(id))
		return
	// back within half an hour of leaving: "wb"
	if (g_Vault != INVALID_HANDLE)
	{
		new auth[40], val[16], ts
		person_key(id, auth, charsmax(auth))
		if (nvault_lookup(g_Vault, fmt("left|%s", auth), val, charsmax(val), ts) && get_systime() - str_to_num(val) < 1800)
		{
			want(any_bot(0), "welcome_back", id, 0.6)
			return
		}
	}
	// an old face? the bot that knows them best says hi like it. gm_people's memory (how
	// many days they've played together, when last) is the one the lines' conditions read,
	// so it decides when it's there; gm_botmemory is the fallback without gm_people
	new best, bestGames, bool:rival
	if (LibraryExists("gm_people", LibType_Library))
	{
		static const FAM[][] = { "20", "10", "5", "3" }
		for (new f = 0; f < sizeof FAM && !best; f++)
		{
			for (new b = 1; b <= MaxClients && !best; b++)
			{
				if (!is_user_connected(b) || !is_user_bot(b))
					continue
				// played together before, and not already today
				if (people_match(b, id, "fam", ">=", FAM[f]) == 1 && people_match(b, id, "days", ">=", "1") == 1)
				{
					best = b
					rival = people_match(b, id, "rival", "=", "1") == 1
				}
			}
		}
		if (best)
			want(best, rival ? "remember_rival" : "remember", id, 0.7, -1.0, 1)
		else if (greet_slot())
			want(any_bot(0), "welcome", id, 0.4)
		return
	}
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b))
			continue
		new k, d, day
		if (recall(b, id, k, d, day) && day < today() && k + d > bestGames)
		{
			best = b
			bestGames = k + d
			rival = k + d >= 12
		}
	}
	if (best)
		want(best, rival ? "remember_rival" : "remember", id, 0.7, -1.0, 1)
	else if (greet_slot())
		want(any_bot(0), "welcome", id, 0.4)
}

// ---- memory between days --------------------------------------------------------------

today()
{
	return get_systime() / 86400
}

memory_key(bot, human, key[], len)
{
	new a[32], b[32]
	bare_of(bot, a, charsmax(a))
	bare_of(human, b, charsmax(b))
	formatex(key, len, "%s|%s", a, b)
}

// How a person is remembered (gm_people's memory_name): their Steam ID, or on a LAN (no real
// Steam ID) "h:" and their name without its tag
person_key(id, out[], len)
{
	get_user_authid(id, out, len)
	if (equal(out, "STEAM_", 6) && !equal(out, "STEAM_ID_", 9))
		return
	new bare[32]
	bare_of(id, bare, charsmax(bare))
	formatex(out, len, "h:%s", bare)
}

public task_forget_name(tid)
{
	new id = tid - TASK_FORGET
	if (!is_user_connected(id))
		g_Bare[id][0] = 0
}

// A name without its clan tag, the rule every plugin and the bot brain share (ls_bare_name)
bare_of(id, out[], len)
{
	// (worked out once per player in the slot)
	if (g_Bare[id][0])
	{
		copy(out, len, g_Bare[id])
		return
	}
	bare_lookup(id, g_Bare[id], charsmax(g_Bare[]))
	copy(out, len, g_Bare[id])
}

bare_lookup(id, out[], len)
{
	// a bot as gm_people knows it (a "Dave_" the game renamed is still Dave)
	if (is_user_bot(id) && !g_NoPeopleName && LibraryExists("gm_people", LibType_Library))
	{
		people_name(id, out, len)
		if (out[0])
			return
	}
	new name[32]
	get_user_name(id, name, charsmax(name))
	trim(name)
	ls_bare_name(name, out, len)
}

// What `bot` remembers of `human`: kills each way and the day they last played
bool:recall(bot, human, &kills, &deaths, &day)
{
	if (g_Vault == INVALID_HANDLE)
		return false
	new key[72], val[32], ts
	memory_key(bot, human, key, charsmax(key))
	if (!nvault_lookup(g_Vault, key, val, charsmax(val), ts))
		return false
	new a[12], b[12], c[12]
	parse(val, a, charsmax(a), b, charsmax(b), c, charsmax(c))
	kills = str_to_num(a)
	deaths = str_to_num(b)
	day = str_to_num(c)
	return true
}

// Adds this map's kills between `human` and every bot to what they remember
remember_all(human)
{
	if (g_Vault == INVALID_HANDLE || !is_user_connected(human) || is_user_bot(human))
		return
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b))
			continue
		new k, d, day
		recall(b, human, k, d, day)
		k += g_KilledBy[b][human]
		d += g_KilledBy[human][b]
		new key[72]
		memory_key(b, human, key, charsmax(key))
		// only people who actually played each other
		if (g_KilledBy[b][human] || g_KilledBy[human][b])
			nvault_set(g_Vault, key, fmt("%d %d %d", k, d, today()))
		g_KilledBy[b][human] = g_KilledBy[human][b] = 0
	}
}

public client_disconnected(id)
{
	// (the name is needed once more below; the next one in the slot works it out again)
	set_task(0.1, "task_forget_name", TASK_FORGET + id)
	// nothing this slot was asked to do (or about to do) carries over to whoever gets it next
	req_clear(id)
	remove_task(TASK_KYS + id)
	g_KysDone[id] = false
	for (new b = 1; b <= MaxClients; b++)
		if (g_ReqFor[b] == id)
			req_clear(b)
	remove_task(TASK_TBAG + id)
	remove_task(TASK_TYPEHOLD + id)
	if (!is_user_bot(id))
	{
		remember_all(id)
		// when they left (a "wb" if they're back soon)
		if (g_Vault != INVALID_HANDLE && !g_MapEnding)
		{
			new auth[40]
			person_key(id, auth, charsmax(auth))
			nvault_set(g_Vault, fmt("left|%s", auth), fmt("%d", get_systime()))
		}
	}
	if (g_AiState[id])
		ai_close(id)
	// threads with this bot end (the slot may go to someone else)
	for (new h = 1; h <= MaxClients; h++)
		if (g_ThreadBot[h] == id)
			g_ThreadBot[h] = 0
	g_ThreadBot[id] = 0
	// nothing left waiting to be said by whoever had this slot
	for (new i = 0; i < MAX_PENDING; i++)
		if (g_PendBot[i] == id)
		{
			g_PendBot[i] = 0
			remove_task(TASK_SAY + i)
		}
}

// A dead bot's line, shown to the living too (with the game's own *DEAD* tag)
dead_chat(id, const text[], bool:team)
{
	static msg
	if (!msg)
		msg = get_user_msgid("SayText")
	new t = get_user_team(id)
	new fmt_[32]
	copy(fmt_, charsmax(fmt_), team ? (t == 1 ? "#Cstrike_Chat_T_Dead" : "#Cstrike_Chat_CT_Dead") : "#Cstrike_Chat_AllDead")
	for (new r = 1; r <= MaxClients; r++)
	{
		if (!is_user_connected(r) || is_user_bot(r) || !is_user_alive(r) || (team && get_user_team(r) != t))
			continue
		message_begin(MSG_ONE, msg, _, r)
		write_byte(id)
		write_string(fmt_)
		write_string("")
		write_string(text)
		message_end()
	}
}

// Who's here as the map ends, for the next map (bots by name without the tag, people by
// SteamID): a map change isn't anyone joining, so no hellos and welcomes again
lastmap_path(path[], len)
{
	get_datadir(path, len)
	add(path, len, "/gamemodes/lastmap_people.txt")
}

save_lastmap()
{
	new path[128]
	lastmap_path(path, charsmax(path))
	new f = fopen(path, "wt")
	if (!f)
		return
	fprintf(f, "%d^n", get_systime())
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || is_user_hltv(i))
			continue
		new key[40]
		if (is_user_bot(i))
			bare_of(i, key, charsmax(key))
		else
			person_key(i, key, charsmax(key))
		fprintf(f, "%s^n", key)
	}
	fclose(f)
}

load_lastmap()
{
	new path[128], line[40]
	lastmap_path(path, charsmax(path))
	new f = fopen(path, "rt")
	if (!f)
		return
	fgets(f, line, charsmax(line))
	// (only a map change: from minutes ago, not last night)
	if (get_systime() - str_to_num(line) > 180)
	{
		fclose(f)
		return
	}
	while (!feof(f))
	{
		fgets(f, line, charsmax(line))
		trim(line)
		if (line[0])
			TrieSetCell(g_LastMap, line, 1)
	}
	fclose(f)
}

// Gun Game: someone on the knife (the last level) gets called out now and then
public task_gg_lead()
{
	if (!get_pcvar_num(g_pLevel) || g_Career)
		return
	// the knife warmup at the start of the map: everyone's on the knife, nobody's leading
	if (get_gametime() < get_cvar_float("gg_warmup_timer_setting") + 10.0)
		return
	new knives
	for (new id = 1; id <= MaxClients; id++)
	{
		new weapons[32], num
		if (!is_user_alive(id))
			continue
		get_user_weapons(id, weapons, num)
		if (num == 1 && weapons[0] == CSW_KNIFE)
			knives++
	}
	if (knives >= 2)
		return
	for (new id = 1; id <= MaxClients; id++)
	{
		if (!is_user_alive(id))
			continue
		new weapons[32], num
		get_user_weapons(id, weapons, num)
		if (num != 1 || weapons[0] != CSW_KNIFE)
			continue
		new deaths = get_user_deaths(id)
		if (g_GGKnifeSaid[id] == deaths + 1)
			continue
		g_GGKnifeSaid[id] = deaths + 1
		new b = any_bot(id)
		if (b)
			want(b, "gg_leader", id, 0.5)
		return
	}
}

// No nav for this map yet: the bots are learning it, and wander meanwhile
public task_nav_hint()
{
	new map[32]
	get_mapname(map, charsmax(map))
	if (!file_exists(fmt("maps/%s.nav", map)))
		client_print(0, print_chat, "* The bots are learning this map, so they may wander for a few minutes.")
}

// A person shooting a teammate bot (friendly fire on): it says something, more each time
// ("?", then "stop", then "wtf stop shooting me"), and after the third steps out of the way.
// The tilted and the cold say it sooner. It never shoots back
public fw_ff_post(victim, inflictor, attacker, Float:damage, bits)
{
	if (!(1 <= attacker <= MaxClients) || attacker == victim || !is_user_connected(attacker) || is_user_bot(attacker)
		|| !is_user_connected(victim) || !is_user_bot(victim) || teams_off() || get_user_team(attacker) != get_user_team(victim) || damage <= 0.0)
		return
	new Float:now = get_gametime()
	if (now - g_FFAt[victim][attacker] > 20.0)
		g_FF[victim][attacker] = 0
	g_FFAt[victim][attacker] = now
	new n = ++g_FF[victim][attacker]
	new bool:short = has_persona(victim, "rtsHOo") || (LibraryExists("gm_people", LibType_Library) && people_get(victim, "temper") == 2)
	if (n == (short ? 1 : 2))
		want(victim, "ff_1", attacker, 0.7, random_float(0.3, 0.8))
	else if (n == (short ? 2 : 3))
		want(victim, "ff_2", attacker, 0.8, random_float(0.3, 0.8))
	else if (n == (short ? 3 : 5))
	{
		want(victim, "ff_3", attacker, 0.9, random_float(0.2, 0.6))
		// a step out of their way
		if (is_user_alive(victim) && cvar_exists("bot_ls_routes") && !g_SteeredMode)
		{
			new Float:me[3], Float:them[3], Float:to[3]
			pev(victim, pev_origin, me)
			pev(attacker, pev_origin, them)
			new Float:dx = me[0] - them[0], Float:dy = me[1] - them[1], Float:len = floatmax(floatsqroot(dx * dx + dy * dy), 1.0)
			to[0] = me[0] + dx / len * 140.0 + random_float(-40.0, 40.0)
			to[1] = me[1] + dy / len * 140.0 + random_float(-40.0, 40.0)
			to[2] = me[2]
			server_cmd("ls_bot %d goto %.0f %.0f %.0f", victim, to[0], to[1], to[2])
		}
	}
}

public plugin_end()
{
	g_MapEnding = true
	save_lastmap()
	for (new i = 1; i <= MaxClients; i++)
		if (g_AiState[i])
			ai_close(i)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && !is_user_bot(i))
			remember_all(i)
	if (g_Vault != INVALID_HANDLE)
	{
		// faces not seen for two months are forgotten
		nvault_prune(g_Vault, 0, get_systime() - 60 * 86400)
		nvault_close(g_Vault)
	}
}

// The clan tag in a name, as the clan's letters ("WoW" for "~WoW~ Squishy"): a first
// word wrapped in symbols ("[XyZ] Name", "~WoW~ Name", "|LoL| Name", "=DD= yuki",
// "<<OGs>> Munch"), or a short word before a bar ("GG | Phantom", "XyZ|Name")
clan_of(id, tag[], len)
{
	tag[0] = 0
	new name[32], bare[32]
	get_user_name(id, name, charsmax(name))
	trim(name)
	// the clan is the tag the shared rules take off ("-=[CoC]=- Bob": CoC); "(1)" and
	// other numbers aren't a clan, and neither is a name that's only decorated
	ls_bare_name(name, bare, charsmax(bare))
	if (equal(bare, name))
		return
	new at = contain(name, bare), k, bool:letters
	for (new i = 0; name[i] && k < len; i++)
	{
		if (at >= 0 && i >= at && i < at + strlen(bare))
			continue
		if (isalnum(name[i]))
		{
			tag[k++] = name[i]
			if (isalpha(name[i]))
				letters = true
		}
	}
	tag[k] = 0
	if (strlen(tag) < 2 || !letters)
		tag[0] = 0
}

// A name without its clan tag, by the bot brain's rules (LsBareName in cs_bot_ls.cpp; every
// plugin uses this same copy, so a regular is the same person to all of them): a bracketed
// tag at the start or the end ([TAG], (TAG), {TAG}, <TAG>, |TAG|, -=TAG=-) goes, twice over,
// so "-=[CoC]=- Bob", "[FX]Dave", "(1)Dave" and "Dave |AX|" are Bob and Dave. A bar inside
// a name ("GG | Glacier") is left alone. A name that's all tag stays as it is
ls_bare_name(const name[], out[], len)
{
	new const opens[] = "[({<|", closes[] = "])}>|"
	new n = strlen(name), a = 0, b = n
	for (new pass = 0; pass < 2; pass++)
	{
		// leading
		while (a < b && name[a] == ' ')
			a++
		new o = a < b ? ls_char_in(opens, name[a]) : -1
		if (o >= 0)
		{
			for (new k = a + 1; k < b; k++)
				if (name[k] == closes[o])
				{
					a = k + 1
					break
				}
		}
		else if (b - a > 4 && name[a] == '-' && name[a + 1] == '=')
		{
			for (new k = a + 2; k + 1 < b; k++)
				if (name[k] == '=' && name[k + 1] == '-')
				{
					a = k + 2
					break
				}
		}
		// trailing
		while (b > a && name[b - 1] == ' ')
			b--
		new c = b > a ? ls_char_in(closes, name[b - 1]) : -1
		if (c >= 0)
		{
			for (new k = b - 2; k >= a; k--)
				if (name[k] == opens[c])
				{
					b = k
					break
				}
		}
	}
	if (b <= a)
	{
		a = 0
		b = n
	}
	new m = min(b - a, len)
	for (new i = 0; i < m; i++)
		out[i] = name[a + i]
	out[m] = 0
	trim(out)
	return m
}

ls_char_in(const set[], ch)
{
	for (new i = 0; set[i]; i++)
		if (set[i] == ch)
			return i
	return -1
}

bool:clanmates(a, b)
{
	if (a == b || !is_user_connected(a) || !is_user_connected(b))
		return false
	new ta[16], tb[16]
	clan_of(a, ta, charsmax(ta))
	clan_of(b, tb, charsmax(tb))
	return ta[0] && equali(ta, tb)
}

// A bot clanmate of `id` (on its team if `team` is set), or 0
clanmate_bot(id, bool:sameteam)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (i != id && is_user_connected(i) && is_user_bot(i) && clanmates(i, id) && (!sameteam || get_user_team(i) == get_user_team(id)))
			list[n++] = i
	return n ? list[random(n)] : 0
}

// The playstyle personalities (an AWPer, a rusher, a camper...), picked on a roll of their
// own (NEW_SHARE per 1000 bots) so adding them left every other regular's personality as it
// was. Letters and weights; Z is the clutch hero, K wants the admin (the rule enforcer)
new const NEW_KINDS[] = "aeg4567l89vyZK"
new const NEW_WEIGHT[] = { 25, 30, 10, 25, 10, 8, 6, 15, 15, 6, 10, 8, 8, 10 }
#define NEW_SHARE 170

// From the name, so the same regular always has the same personality. New players
// in the Newbie lobby style are more often new to the game too.
persona_of(id)
{
	// gm_people: the archetype says which of the old personalities this person acts as
	if (LibraryExists("gm_people", LibType_Library))
	{
		new l = people_letter(id)
		new at = l ? contain(PERSONAS, fmt("%c", l)) : -1
		if (at >= 0)
			return at
	}
	new name[32], h = 7
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h * 31 + name[i]) & 0xFFFFFF
	// the playstyle roll, from another mix of the name
	new h2 = 2166136261
	for (new i = 0; name[i]; i++)
		h2 = ((h2 ^ name[i]) * 16777619) & 0x7FFFFFFF
	if ((h2 / 13) % 1000 < NEW_SHARE && get_cvar_num("gm_botstyle") != 4)
	{
		new total
		for (new i = 0; i < sizeof NEW_WEIGHT; i++)
			total += NEW_WEIGHT[i]
		new r = (h2 / 13001) % total
		for (new i = 0; i < sizeof NEW_WEIGHT; i++)
		{
			if (r < NEW_WEIGHT[i])
				return contain(PERSONAS, fmt("%c", NEW_KINDS[i]))
			r -= NEW_WEIGHT[i]
		}
	}
	if (get_cvar_num("gm_botstyle") == 4 && h % 3 == 0)
		return 5
	// weighted: plain players are common, gimmicks rare
	new total
	for (new i = 0; i < sizeof WEIGHT; i++)
		total += WEIGHT[i]
	new r = h % total
	for (new i = 0; i < sizeof WEIGHT; i++)
	{
		if (r < WEIGHT[i])
			return i
		r -= WEIGHT[i]
	}
	return 0
}

// A bot's own habits, from its name (a different mix of the name than its personality)
habits_of(id)
{
	new name[32], h = 2166136261
	get_user_name(id, name, charsmax(name))
	for (new i = 0; name[i]; i++)
		h = (h ^ name[i]) * 16777619
	h &= 0x7FFFFFFF
	g_Laugh[id] = h % LAUGHS_ANYONE
	g_Word[id] = (h / 7) % sizeof WORDS
	g_Emote[id] = (h / 31) % sizeof EMOTES
	g_TextSpeak[id] = (h / 101) % 3 == 0
	g_Typos[id] = (h / 211) % 15             // percent
	g_Shouty[id] = (h / 401) % 4 == 0 ? 8 : 1
	// a few people do most of the talking: most say little, some a lot
	new tier = (h / 977) % 100
	g_Chatty[id] = tier < 45 ? 0.15 : tier < 75 ? 0.6 : tier < 93 ? 1.5 : 3.0
	// with gm_people the talk trait does most of the spreading: gentler steps here
	if (LibraryExists("gm_people", LibType_Library))
		g_Chatty[id] = tier < 45 ? 0.5 : tier < 75 ? 0.8 : tier < 93 ? 1.2 : 1.7
	new p = PERSONAS[g_Persona[id]]
	if (p == 'x' && g_Laugh[id] < 6)
		g_Laugh[id] = LAUGHS_ANYONE + (h % 6)   // kkkk, jajaja, mdr, www, 555, kek
	// gm_people: where they're from shows in the laugh; kids type like kids
	if (LibraryExists("gm_people", LibType_Library))
	{
		switch (people_get(id, "region"))
		{
			case 6: g_Laugh[id] = LAUGHS_ANYONE      // br: kkkk
			case 9: g_Laugh[id] = LAUGHS_ANYONE + 1  // es: jajaja
			case 7: if (h % 2) g_Laugh[id] = LAUGHS_ANYONE + 2   // fr: mdr, half of them
		}
		new age = people_get(id, "age")
		if (age == 0)
		{
			g_TextSpeak[id] = true
			g_Typos[id] += 5
		}
		else if (age == 3)
		{
			g_TextSpeak[id] = false
			g_Typos[id] = g_Typos[id] / 2
		}
	}
	if (p == 'k')
		g_TextSpeak[id] = true
}

bool:has_persona(id, const letters[])
{
	if (letters[0] == '*')
		return true
	return contain(letters, fmt("%c", PERSONAS[g_Persona[id]])) != -1
}

// How a bot is doing, as the mood marks lines can carry: - tilted or losing (short, salty),
// + on a streak or winning (cocky, relaxed), ^ just joined (polite), = knows the person it's
// talking to (casual; `other`, a person)
mood_of(id, other, out[], len)
{
	out[0] = 0
	new team = get_user_team(id)
	new diff = get_user_frags(id) - get_user_deaths(id)
	new bool:losing = team >= 1 && team <= 2 && g_LoseRun[team] >= 3
	new bool:winning = team >= 1 && team <= 2 && g_WinRun[team] >= 3
	if (g_DeathRow[id] >= 3 || losing || diff <= -6)
		add(out, len, "-")
	else if (g_RoundKills[id] >= 2 || winning || diff >= 8)
		add(out, len, "+")
	if (get_gametime() - g_JoinedAt[id] < 150.0)
		add(out, len, "^^")
	if (other && is_user_connected(other) && !is_user_bot(other))
	{
		new k, d, day
		if ((recall(id, other, k, d, day) && day < today()) || g_KilledBy[id][other] + g_KilledBy[other][id] >= 6)
			add(out, len, "=")
	}
}

// Fights in the last few seconds: no time to type
bool:in_combat(id)
{
	return get_gametime() - g_FightAt[id] < 5.0
}

// ... or an enemy in view right now (gm_humanbots knows), or danger around it: gunfire,
// someone hurt close by, an enemy seen a moment ago (gm_botfun knows)
bool:fighting(id)
{
	return in_combat(id) || bot_target(id) != 0 || bot_danger(id)
}

// Deathmatch-type play: no teams underneath, bots respawning (bot_deathmatch), the deathmatch
// and Gun Game modes
bool:dm_like()
{
	static pBotDm = -1
	if (pBotDm < 0)
		pBotDm = get_cvar_pointer("bot_deathmatch")
	return g_DmMode || teams_off() || (pBotDm && get_pcvar_num(pBotDm) != 0)
}

// No teams underneath (Gun Game, free for all: mp_freeforall): everyone is against everyone
bool:teams_off()
{
	static pFfa
	if (!pFfa)
		pFfa = get_cvar_pointer("mp_freeforall")
	return pFfa && get_pcvar_num(pFfa) != 0
}

// What only makes sense with teams: teamkills and friendly fire, teammates, the team's score,
// the last one alive for the team, callouts, someone in the way
// Teabagged: the killer crouching three times or more within 150 units of the body in the
// 5 s after the kill. The bot it was done to says something (it's dead: everyone sees it) and
// remembers it as an insult (gm_people)
new g_TbagKiller[33], g_TbagDucks[33], bool:g_TbagDown[33], Float:g_TbagAt[33], Float:g_TbagBody[33][3]

watch_tbag(victim, killer)
{
	g_TbagKiller[victim] = killer
	g_TbagDucks[victim] = 0
	g_TbagDown[victim] = (pev(killer, pev_flags) & FL_DUCKING) != 0
	g_TbagAt[victim] = get_gametime()
	pev(victim, pev_origin, g_TbagBody[victim])
	remove_task(TASK_TBAG + victim)
	set_task(0.1, "task_tbag", TASK_TBAG + victim, _, _, "b")
}

public task_tbag(tid)
{
	new victim = tid - TASK_TBAG
	new killer = g_TbagKiller[victim]
	if (!is_user_connected(victim) || !is_user_bot(victim) || !is_user_alive(killer) || get_gametime() - g_TbagAt[victim] > 5.0)
	{
		remove_task(tid)
		return
	}
	new Float:at[3]
	pev(killer, pev_origin, at)
	new bool:down = (pev(killer, pev_flags) & FL_DUCKING) != 0
	if (down && !g_TbagDown[victim] && get_distance_f(at, g_TbagBody[victim]) < 150.0)
		g_TbagDucks[victim]++
	g_TbagDown[victim] = down
	if (g_TbagDucks[victim] >= 3)
	{
		remove_task(tid)
		want(victim, "death_tbag", killer, 0.75)
		if (LibraryExists("gm_people", LibType_Library))
			people_note(victim, killer, "insult")
	}
}

bool:team_section(const section[])
{
	static const TEAM[][] = { "teamkill", "teamkilled", "r_sorry_tk", "r_ok_tk", "teammate_clutch", "losing_streak_team", "winning_streak_team",
		"support", "nice_shot", "spectating", "last_alive", "callout", "callout_low", "callout_vague", "comeback", "blocked", "in_my_way",
		"clan", "clan_avenge" }
	for (new i = 0; i < sizeof TEAM; i++)
		if (equal(section, TEAM[i]))
			return true
	// a teammate's praise for a person's kill (human_kill, _hs, _knife, _nade)
	return equal(section, "human_kill", 10) != 0
}

// Someone in the way: a bot pushing forward into a teammate in front of it (a doorway, a
// corridor) for a couple of seconds, or with a teammate between it and where it's shooting.
// Team modes only; each bot says it now and then at most
new g_Blocked[33], Float:g_BlockSaid[33], Float:g_WasMoving[33]
new bool:g_Spotted[33], Float:g_VagueSaid[33]   // an enemy in sight at the last look; when it last shouted one

public task_blocked()
{
	if (teams_off() || g_SteeredMode || g_Freeze || g_Career || !get_pcvar_num(g_pLevel))
		return
	new Float:now = get_gametime()
	for (new id = 1; id <= MaxClients; id++)
	{
		if (is_user_alive(id) && is_user_bot(id))
			spot_check(id, now)
		if (!is_user_alive(id) || !is_user_bot(id) || now < g_BlockSaid[id])
		{
			g_Blocked[id] = 0
			continue
		}
		new buttons = pev(id, pev_button)
		new Float:v[3], Float:me[3], Float:ang[3], Float:fwd[3]
		pev(id, pev_velocity, v)
		pev(id, pev_origin, me)
		pev(id, pev_v_angle, ang)
		angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
		new team = get_user_team(id)
		// a teammate in the line of fire
		if (buttons & IN_ATTACK)
		{
			new Float:eye[3], Float:end[3], Float:ofs[3]
			pev(id, pev_view_ofs, ofs)
			for (new i = 0; i < 3; i++)
			{
				eye[i] = me[i] + ofs[i]
				end[i] = eye[i] + fwd[i] * 1500.0
			}
			new tr = create_tr2()
			engfunc(EngFunc_TraceLine, eye, end, DONT_IGNORE_MONSTERS, id, tr)
			new hit = get_tr2(tr, TR_pHit)
			free_tr2(tr)
			if (hit >= 1 && hit <= MaxClients && hit != id && is_user_alive(hit) && get_user_team(hit) == team)
			{
				g_BlockSaid[id] = now + random_float(25.0, 60.0)
				want(id, "in_my_way", hit, 0.5)
				continue
			}
		}
		// pushing forward and going nowhere, or just stopped (a bot waits behind a teammate in
		// a doorway for a moment before it pushes): a teammate right in front
		new mate
		new Float:speed = floatsqroot(v[0] * v[0] + v[1] * v[1])
		new bool:stopped = speed < 40.0 && now - g_WasMoving[id] < 1.6 && now - g_RoundLiveAt > 8.0
		if (speed > 120.0)
			g_WasMoving[id] = now
		if (((buttons & IN_FORWARD) && speed < 40.0) || stopped)
		{
			for (new m = 1; m <= MaxClients && !mate; m++)
			{
				if (m == id || !is_user_alive(m) || get_user_team(m) != team)
					continue
				new Float:o[3], Float:d[3]
				pev(m, pev_origin, o)
				for (new i = 0; i < 3; i++)
					d[i] = o[i] - me[i]
				new Float:len = floatsqroot(d[0] * d[0] + d[1] * d[1])
				if (len < 72.0 && floatabs(d[2]) < 48.0 && (d[0] * fwd[0] + d[1] * fwd[1]) > 0.6 * len)
					mate = m
			}
		}
		if (!mate)
		{
			g_Blocked[id] = 0
			continue
		}
		// (just stopped behind them: said straight away, as people do; pushing: after a moment)
		if (++g_Blocked[id] >= 2 || stopped)
		{
			g_Blocked[id] = 0
			g_BlockSaid[id] = now + random_float(30.0, 90.0)
			want(id, "blocked", mate, 0.45)
		}
	}
}

// Someone just came into sight: now and then, on a pub, a bot shouts it in team chat without
// saying where ("THERE!", "one there"), mostly newer players (the lines lean on that). Not in
// a match (callouts there name the place), nor in deathmatch
spot_check(id, Float:now)
{
	new bool:sees = sees_enemy_now(id)
	new bool:fresh = sees && !g_Spotted[id]
	g_Spotted[id] = sees
	if (!fresh || now < g_VagueSaid[id] || match_like() || get_cvar_num("bot_deathmatch"))
		return
	g_VagueSaid[id] = now + random_float(60.0, 150.0)
	want(id, "callout_vague", 0, 0.08, random_float(0.2, 0.8), 0, true)
}

// An enemy in front of it and in plain sight (eyes to eyes), within 2000 units
bool:sees_enemy_now(id)
{
	new Float:eye[3], Float:ofs[3], Float:ang[3], Float:fwd[3]
	pev(id, pev_origin, eye)
	pev(id, pev_view_ofs, ofs)
	pev(id, pev_v_angle, ang)
	angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
	for (new i = 0; i < 3; i++)
		eye[i] += ofs[i]
	new team = get_user_team(id)
	new tr = create_tr2()
	new bool:seen
	for (new e = 1; e <= MaxClients && !seen; e++)
	{
		if (e == id || !is_user_alive(e) || get_user_team(e) == team)
			continue
		new Float:at[3], Float:d[3]
		pev(e, pev_origin, at)
		pev(e, pev_view_ofs, ofs)
		for (new i = 0; i < 3; i++)
		{
			at[i] += ofs[i]
			d[i] = at[i] - eye[i]
		}
		new Float:len = vector_length(d)
		if (len > 2000.0 || len < 1.0 || (d[0] * fwd[0] + d[1] * fwd[1] + d[2] * fwd[2]) < 0.6 * len)
			continue
		engfunc(EngFunc_TraceLine, eye, at, IGNORE_MONSTERS, id, tr)
		new Float:frac
		get_tr2(tr, TR_flFraction, frac)
		seen = frac >= 1.0
	}
	free_tr2(tr)
	return seen
}

// The enemy a bot has in view (gm_humanbots knows; 0 none, or it isn't there)
bot_target(id)
{
	return (g_HumanBots && !g_NoTargetNative) ? hb_bot_target(id) : 0
}

// Danger around it: gunfire, someone hurt close by, an enemy seen lately (gm_botfun knows)
bool:bot_danger(id)
{
	return g_BotFun && !g_NoDangerNative && bf_danger(id)
}

// Played like a match: a match, match mode, the Matchmaking style on Classic. Chat is sparse
bool:match_like()
{
	new phase = get_cvar_num("gm_scrim_phase")
	return (phase > 0 && phase < 5) || g_MatchMode || (g_ClassicMode && get_cvar_num("gm_botstyle") == 2)
}

// Standing there afk (gm_humanbots): nobody at the keyboard to type
bool:bot_afk(id)
{
	return g_HumanBots && !g_NoAfkNative && is_user_alive(id) && hb_bot_afk(id)
}

// ---- deciding to say something ---------------------------------------------------------

personality_cvar()
{
	g_pPersonality = get_cvar_pointer("gm_personality")
}

// gm_personality as a factor: 0.0 (bots keep to themselves) to 2.0 (chatty), 1.0 without it
Float:personality()
{
	if (!g_pPersonality)
		personality_cvar()
	if (!g_pPersonality)
		return 1.0
	return float(clamp(get_pcvar_num(g_pPersonality), 0, 200)) / 100.0
}

Float:level_mult()
{
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: return 0.3
		case 3: return 1.8
	}
	return 1.0
}

// gaps get longer as personality goes down (x1.5 at 0) and shorter as it goes up (x0.5 at 200)
Float:gap_mult()
{
	return 1.5 - 0.5 * personality()
}

Float:global_gap()
{
	new Float:g
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: g = 30.0
		case 3: g = 4.0
		default: g = 7.0
	}
	// deathmatch: everyone dies all the time, so the gap between anyone's lines is longer
	return g * gap_mult() * (dm_like() ? 1.5 : 1.0)
}

Float:bot_gap()
{
	new Float:g
	switch (get_pcvar_num(g_pLevel))
	{
		case 1: g = 120.0
		case 3: g = 20.0
		default: g = 35.0
	}
	return g * gap_mult()
}

// A bot might say a line from `section` about `other`, with the chance `base`
// (before the chattiness setting, gm_personality and its own personality). chain > 0: a
// reply. Answers to people go through answer() instead.
// The lobby mood (gm_mood, from gm_people; the app's Lobby mood): in a toxic lobby the
// trash talk, rage, hack accusations and blaming come up far more and the nice things less;
// in a friendly one the other way round. Normal: as the people are
// The lobby mood: gm_people's (one rule for every plugin), else gm_mood's own
lobby_mood()
{
	if (LibraryExists("gm_people", LibType_Library))
		return people_mood()
	new p = get_cvar_pointer("gm_mood")
	new m = p ? clamp(get_pcvar_num(p), 0, 2) : 1
	// (a LAN party is friends: friendly unless the lobby's set to something else)
	return m == 1 && get_cvar_num("gm_botstyle") == 9 ? 0 : m
}

// Rage, tilt, trash talk and the like (the lobby's mood scales these)
bool:toxic_section(const section[])
{
	static const TOXIC[][] = { "rage", "ragequit", "tilt", "r_hack", "t_hack", "r_trash", "r_noob", "t_noob", "r_ez", "t_ez",
		"nemesis", "nemesis_won", "r_camping", "t_camping", "voice_rage", "voice_spam", "teamkilled", "death_wallbang",
		"r_wtf", "r_lucky_back", "losing_streak_team", "in_my_way", "blocked", "human_camping", "human_awp", "r_kys_no" }
	for (new i = 0; i < sizeof TOXIC; i++)
		if (equal(section, TOXIC[i]))
			return true
	return false
}

Float:mood_factor(const section[])
{
	// insults and gloating: "Bots can be rude" off (gm_hb_disrespect, the default) is a cap
	// whatever the mood (rare, never gone: people do say it); on, the mood scales them
	static const INSULT[][] = { "r_trash", "r_noob", "t_noob", "r_ez", "t_ez", "r_camping", "t_camping", "nemesis", "nemesis_won", "r_kys_no" }
	new bool:insult
	for (new i = 0; i < sizeof INSULT; i++)
		if (equal(section, INSULT[i]))
			insult = true
	if (insult && !rude_on())
		return 0.15
	// complaining, tilt, rage and hack accusations: the mood (no setting caps a sore loser)
	static const TOXIC[][] = { "rage", "ragequit", "tilt", "r_hack", "t_hack", "r_trash", "r_noob", "t_noob", "r_ez", "t_ez",
		"nemesis", "nemesis_won", "r_camping", "t_camping", "voice_rage", "voice_spam", "teamkilled", "death_wallbang",
		"r_wtf", "r_lucky_back", "losing_streak_team", "in_my_way", "blocked" }
	static const NICE[][] = { "nice_shot", "r_gg", "t_gg", "r_gl", "t_gl", "r_np", "r_ns_thanks", "support", "r_nice_from_human",
		"t_nice_from_human", "r_sorry", "t_sorry", "teammate_clutch", "greet", "welcome", "r_hi", "t_hi", "map_end", "drop_give", "gift_ty" }
	new mood = lobby_mood()
	if (mood == 1)
		return 1.0
	for (new i = 0; i < sizeof TOXIC; i++)
		if (equal(section, TOXIC[i]))
			return mood == 2 ? 2.2 : 0.3
	for (new i = 0; i < sizeof NICE; i++)
		if (equal(section, NICE[i]))
			return mood == 2 ? 0.6 : 1.6
	return 1.0
}

// The bot style's own chattiness: a LAN party never shuts up, the old-timers rarely talk
Float:style_chat()
{
	switch (get_cvar_num("gm_botstyle"))
	{
		case 9: return 1.6
		case 10: return 0.45
	}
	return 1.0
}

// "Bots can be rude" (gm_hb_disrespect, gm_humanbots' setting; off by default)
bool:rude_on()
{
	static p
	if (!p)
		p = get_cvar_pointer("gm_hb_disrespect")
	return p && get_pcvar_num(p) != 0
}

want(id, const section[], other, Float:base, Float:delay = -1.0, chain = 0, bool:team = false)
{
	if (!id || !is_user_connected(id) || !is_user_bot(id) || g_Career || !get_pcvar_num(g_pLevel) || bot_afk(id))
		return
	if (teams_off() && team_section(section))
		return
	new Float:now = get_gametime()
	if (!chain && (now < g_NextAny || now < g_NextBot[id] || rate_capped(now)))
		return
	if (bot_capped(id, now))
		return
	// at gm_personality 0 this is 0: bots only answer people and say gg. Who the bot is and the
	// lobby's mood boost rage, tilt and trash talk three times at most
	new Float:boost = personality() * CHATTY[g_Persona[id]] * g_Chatty[id] * talk_factor(id) * mood_factor(section)
	if (toxic_section(section))
		boost = floatmin(boost, 3.0)
	if (random_float(0.0, 1.0) > base * level_mult() * boost * phase_factor(id, section)
		* ((match_like() && !equal(section, "callout", 7)) ? 0.4 : 1.0) * style_chat())
		return
	new text[128]
	g_CtxOther = other
	new bool:ok = pick_line(id, section, text, charsmax(text), false, chain > 0)
	// deathmatch: quick reactions ("ns", "wtf", "again?"); up to three tries for a short one,
	// and a long one only now and then
	if (ok && dm_like())
	{
		for (new t = 0; t < 3 && ok && strlen(text) > 24; t++)
			ok = pick_line(id, section, text, charsmax(text), false, chain > 0)
		if (ok && strlen(text) > 24 && random_num(1, 100) <= 70)
			ok = false
	}
	g_CtxOther = 0
	if (!ok)
		return
	// a reply someone asked for: said, not bid for
	if (chain)
	{
		queue(id, other, text, chain, team, delay, section)
		return
	}
	offer(id, other, text, section, delay, team)
}

offer(id, other, const text[], const section[], Float:delay, bool:team)
{
	if (g_OfferCount >= MAX_OFFERS)
		return
	new o = g_OfferCount++
	g_OfferBot[o] = id
	g_OfferOther[o] = other
	copy(g_OfferText[o], charsmax(g_OfferText[]), text)
	copy(g_OfferBase[o], charsmax(g_OfferBase[]), g_PickBase)
	copy(g_OfferCond[o], charsmax(g_OfferCond[]), g_PickCond)
	copy(g_OfferSec[o], charsmax(g_OfferSec[]), section)
	g_OfferTeam[o] = team
	g_OfferDelay[o] = delay
	// a more specific line, and a chattier bot, bid higher; luck decides the rest
	g_OfferBid[o] = (1.0 + 0.5 * float(g_PickScore)) * CHATTY[g_Persona[id]] * floatsqroot(floatmax(g_Chatty[id], 0.15)) * random_float(0.5, 1.5)
	if (!task_exists(TASK_DIRECTOR))
		set_task(0.15, "task_director", TASK_DIRECTOR)
}

public task_director()
{
	new n = g_OfferCount
	g_OfferCount = 0
	if (!n)
		return
	new said[3], count
	// one speaks; a second now and then, a third almost never
	static const Float:MORE[] = { 1.0, 0.25, 0.05 }
	for (new turn = 0; turn < 3; turn++)
	{
		if (turn && (random_float(0.0, 1.0) >= MORE[turn] || rate_capped(get_gametime())))
			break
		new best = -1
		for (new o = 0; o < n; o++)
		{
			if (g_OfferBid[o] <= 0.0 || !is_user_connected(g_OfferBot[o]))
				continue
			new bool:spoke
			for (new k = 0; k < count; k++)
				if (g_OfferBot[said[k]] == g_OfferBot[o])
					spoke = true
			if (!spoke && (best < 0 || g_OfferBid[o] > g_OfferBid[best]))
				best = o
		}
		if (best < 0)
			break
		commit_line(g_OfferBot[best], g_OfferBase[best], g_OfferCond[best])
		// the others type after the first, not over it
		new Float:delay = g_OfferDelay[best]
		if (turn)
			delay = (delay < 0.0 ? typing_time(g_OfferText[best]) : delay) + random_float(1.0, 3.0) * float(turn)
		queue(g_OfferBot[best], g_OfferOther[best], g_OfferText[best], 0, g_OfferTeam[best], delay, g_OfferSec[best])
		cap_count(get_gametime())
		g_OfferBid[best] = 0.0
		said[count++] = best
	}
}

// A line's theme (THEME_REST), or -1
theme_of(const text[])
{
	new low[128]
	copy(low, charsmax(low), text)
	strtolower(low)
	for (new w = 0; w < sizeof THEME_WORD; w++)
		if (word_in(low, THEME_WORD[w]))
			return THEME_ID[w]
	return -1
}

line_hash(const text[])
{
	new h = 5381
	for (new i = 0; text[i]; i++)
	{
		new c = text[i]
		if (c >= 'A' && c <= 'Z')
			c += 32
		h = ((h << 5) + h + c) & 0x7FFFFFFF
	}
	return h ? h : 1
}

bool:said_lately(h)
{
	for (new i = 0; i < RECENT; i++)
		if (g_Recent[i] == h)
			return true
	return false
}

// "?", "idk", "ya" and the like: fine now and then, robotic when they keep coming
bool:is_filler(const text[])
{
	static const FILLER[][] = { "?", "??", "idk", "ya", "k", "hm", "hm?", "huh", "dunno", "wat", "what", "yep", "nope", "idk lol" }
	new t[16]
	copy(t, charsmax(t), text)
	trim(t)
	for (new i = 0; i < sizeof FILLER; i++)
		if (equali(t, FILLER[i]))
			return true
	return false
}

// A line asking for game state ({place}, {weapon}...) only when that's known now
bool:ctx_ok(const text[])
{
	if (contain(text, "{") == -1)
		return true
	if (contain(text, "{place}") != -1 && !g_CtxPlace[0])
		return false
	if (contain(text, "{weapon}") != -1 && !g_CtxWeapon[0])
		return false
	if (contain(text, "{carrier}") != -1 && !g_CtxCarrier[0])
		return false
	if (contain(text, "{hs}") != -1 && !g_CtxHs)
		return false
	if (contain(text, "{timeleft}") != -1 && get_timeleft() <= 0)
		return false
	if ((contain(text, "{money}") != -1 && !g_CtxMoney[0]) || (contain(text, "{gun}") != -1 && !g_CtxGun[0])
		|| (contain(text, "{hp}") != -1 && !g_CtxHp[0]) || (contain(text, "{left}") != -1 && !g_CtxLeft[0])
		|| (contain(text, "{ago}") != -1 && !g_CtxAgo[0]) || (contain(text, "{site}") != -1 && !g_CtxSite[0]))
		return false
	if (contain(text, "{nextmap}") != -1)
	{
		new next[32]
		next_map(next, charsmax(next))
		if (!next[0])
			return false
	}
	return true
}

bool:uses_state(const text[])
{
	return contain(text, "{place}") != -1 || contain(text, "{weapon}") != -1 || contain(text, "{carrier}") != -1
		|| contain(text, "{us}") != -1 || contain(text, "{diff}") != -1 || contain(text, "{timeleft}") != -1
		|| contain(text, "{nextmap}") != -1 || contain(text, "{hs}") != -1 || contain(text, "{money}") != -1
		|| contain(text, "{gun}") != -1 || contain(text, "{hp}") != -1 || contain(text, "{left}") != -1
		|| contain(text, "{ago}") != -1 || contain(text, "{site}") != -1
}

// The game state in a line, as people would put it ("b", "deag", "7 5", "3 min")
fill_state(id, text[], len)
{
	if (contain(text, "{") == -1)
		return
	replace_string(text, len, "{place}", g_CtxPlace)
	replace_string(text, len, "{weapon}", g_CtxWeapon)
	replace_string(text, len, "{carrier}", g_CtxCarrier)
	replace_string(text, len, "{hs}", "hs")
	replace_string(text, len, "{money}", g_CtxMoney)
	replace_string(text, len, "{gun}", g_CtxGun)
	replace_string(text, len, "{hp}", g_CtxHp)
	replace_string(text, len, "{left}", g_CtxLeft)
	replace_string(text, len, "{ago}", g_CtxAgo)
	replace_string(text, len, "{site}", g_CtxSite)
	new team = get_user_team(id)
	if (team == 1 || team == 2)
	{
		new us = g_Score[team], them = g_Score[3 - team]
		replace_string(text, len, "{us}", fmt("%d", us))
		replace_string(text, len, "{them}", fmt("%d", them))
		replace_string(text, len, "{diff}", fmt("%d", abs(us - them)))
	}
	if (contain(text, "{timeleft}") != -1)
	{
		new left = get_timeleft(), when[16]
		if (left >= 90)
			formatex(when, charsmax(when), "%d min", (left + 30) / 60)
		else if (left >= 45)
			copy(when, charsmax(when), "1 min")
		else
			formatex(when, charsmax(when), "%d sec", max(left, 5))
		replace_string(text, len, "{timeleft}", when)
	}
	if (contain(text, "{nextmap}") != -1)
	{
		new next[32]
		next_map(next, charsmax(next))
		replace_string(text, len, "{nextmap}", next)
	}
}

// A map's name as people say it: "dust2" for de_dust2
casual_map(const map[], out[], len)
{
	new at = 0
	if (strlen(map) > 3 && map[2] == '_')
		at = 3
	copy(out, len, map[at])
}

// The next map (nextmap.amxx / gm_extras put it in amx_nextmap), as people say it; "" if
// unknown or this one again
next_map(out[], len)
{
	out[0] = 0
	if (!get_cvar_pointer("amx_nextmap"))
		return
	new next[32], map[32]
	get_cvar_string("amx_nextmap", next, charsmax(next))
	get_mapname(map, charsmax(map))
	if (!next[0] || equali(next, map) || containi(next, "[") != -1)
		return
	casual_map(next, out, len)
}

// A line from `section` for this bot: the lines whose conditions it meets (anyone's (*),
// its personality's, its mood's, and the rules' key=value conditions: docs/bot-people.md),
// the most specific first, now and then one less specific so it isn't always the same line.
// Not one said lately, not a theme that's resting, and only lines whose game state is
// known; lines with game state in them come up more.
// answer: answering a person, so a fitting line beats a fresh one
// commit false: the director decides later whether it's said (commit_line)
bool:pick_line(id, const section[], text[], len, bool:answer = false, bool:commit = true)
{
	new Array:list
	if (!TrieGetCell(g_Sections, section, list))
		return false
	new n = ArraySize(list)
	if (!n)
		return false
	new Float:p = personality(), Float:now = get_gametime()
	new mood[8]
	mood_of(id, g_CtxOther, mood, charsmax(mood))
	static raw[192], cond[96], Float:cw[MAX_CANDIDATES], cs[MAX_CANDIDATES], ci[MAX_CANDIDATES]
	new cn, best = -1, fallback = -1, fseen
	for (new i = 0; i < n; i++)
	{
		ArrayGetString(list, i, raw, charsmax(raw))
		new bar = contain(raw, "|")
		copy(cond, min(bar, charsmax(cond)), raw)
		new body = bar + 1
		new h = line_hash(raw[body]), score
		new Float:w = line_weight(id, g_CtxOther, cond, mood, p, h, score)
		if (w <= 0.0 || !ctx_ok(raw[body]))
			continue
		// ("@" lines are team chat; with no teams the game sends say_team to everyone)
		if (raw[body] == '@' && teams_off())
			continue
		if (uses_state(raw[body]))
			w *= 2.0
		new bool:fresh = true
		// short lines ("gg", "lol", "ns") come up all the time on a real server
		if (strlen(raw[body]) > 6 && (said_lately(h) || bot_said(id, h)))
			fresh = false
		new t = theme_of(raw[body])
		if (t >= 0 && now - g_ThemeAt[t] < THEME_REST[t] && !answer)
			fresh = false
		if (is_filler(raw[body]) && now - g_FillerAt < 120.0)
			fresh = false
		if (!fresh)
		{
			if (random(++fseen) == 0)
				fallback = i
			continue
		}
		if (cn < MAX_CANDIDATES)
		{
			cw[cn] = w
			cs[cn] = score
			ci[cn] = i
			cn++
			if (score > best)
				best = score
		}
	}
	new pick = -1
	g_PickScore = 0
	if (cn)
	{
		// the most specific tier; one lower now and then, and always when the top one has
		// fewer than three lines (one line for a bot's region would be all it ever says)
		new top
		for (new k = 0; k < cn; k++)
			if (cs[k] == best)
				top++
		new tier = (best > 0 && (top < 3 || random_num(1, 5) == 1)) ? best - 1 : best
		// when the top tier is only joined because it's small, it still gets half the chance
		new Float:topw, Float:restw
		for (new k = 0; k < cn; k++)
		{
			if (cs[k] == best)
				topw += cw[k]
			else if (cs[k] >= tier)
				restw += cw[k]
		}
		new Float:boost = (tier < best && top < 3 && topw > 0.0) ? floatmax(1.0, restw / topw) : 1.0
		new Float:total
		for (new k = 0; k < cn; k++)
		{
			if (cs[k] < tier)
				continue
			new Float:w = cs[k] == best ? cw[k] * boost : cw[k]
			total += w
			if (random_float(0.0, total) < w)
			{
				pick = ci[k]
				g_PickScore = cs[k]
			}
		}
	}
	if (pick < 0)
	{
		// everything fitting was said lately: keep quiet, unless answering someone
		if (!answer || fallback < 0)
			return false
		pick = fallback
	}
	ArrayGetString(list, pick, raw, charsmax(raw))
	new bar = contain(raw, "|")
	copy(g_PickCond, min(bar, charsmax(g_PickCond)), raw)
	copy(text, len, raw[bar + 1])
	trim(text)
	copy(g_PickBase, charsmax(g_PickBase), text)
	if (commit)
		commit_line(id, text, g_PickCond)
	fill_state(id, text, len)
	return true
}

bool:bot_said(id, h)
{
	for (new i = 0; i < BOT_RECENT; i++)
		if (g_BotRecent[id][i] == h)
			return true
	return false
}

// A chosen line is said: it rests (the line, its theme, for this bot), and its options
// take effect
commit_line(id, const text[], const cond[])
{
	new Float:now = get_gametime()
	new h = line_hash(text)
	if (id >= 1 && id <= MaxClients)
	{
		g_BotRecent[id][g_BotRecentPos[id]] = h
		g_BotRecentPos[id] = (g_BotRecentPos[id] + 1) % BOT_RECENT
	}
	g_Recent[g_RecentPos] = h
	g_RecentPos = (g_RecentPos + 1) % RECENT
	new t = theme_of(text)
	if (t >= 0)
		g_ThemeAt[t] = now
	if (is_filler(text))
		g_FillerAt = now
	if (contain(cond, "!") == -1)
		return
	new key[16]
	num_to_str(h, key, charsmax(key))
	new tok[48], pos
	while (next_token(cond, pos, tok, charsmax(tok)))
	{
		if (equal(tok, "!cd=", 4))
			TrieSetCell(g_LineAt, key, now)
		else if (equal(tok, "!once=", 6))
			TrieSetCell(g_LineOnce, key, 1)
		else if (equal(tok, "!set=", 5))
		{
			// !set=name/seconds: a flag lines can check (flag=name) for that long
			new name[32], slash = contain(tok[5], "/")
			new Float:ttl = 120.0
			if (slash >= 0)
			{
				copy(name, min(slash, charsmax(name)), tok[5])
				ttl = str_to_float(tok[5 + slash + 1])
			}
			else
				copy(name, charsmax(name), tok[5])
			TrieSetCell(g_Flags, name, now + ttl)
		}
	}
}

// The next space-separated word of `src` from `pos`; false at the end
bool:next_token(const src[], &pos, out[], len)
{
	while (src[pos] == ' ')
		pos++
	if (!src[pos])
		return false
	new n
	while (src[pos] && src[pos] != ' ')
	{
		if (n < len)
			out[n++] = src[pos]
		pos++
	}
	out[n] = 0
	return true
}

// How much a bot would say a line with these conditions (0: it wouldn't), and how many
// specific conditions it meets (score). Old personality letters and mood marks work as
// they always did; key=value conditions and !options are the rules (docs/bot-people.md)
Float:line_weight(id, other, const cond[], const mood[], Float:p, h, &score)
{
	score = 0
	new Float:w = 1.0, bool:trait, bool:moodset
	new tok[48], pos, key[16]
	num_to_str(h, key, charsmax(key))
	new Float:now = get_gametime()
	while (next_token(cond, pos, tok, charsmax(tok)))
	{
		if (tok[0] == '!')
		{
			if (equal(tok, "!odds=", 6))
			{
				if (random_num(1, 100) > str_to_num(tok[6]))
					return 0.0
			}
			else if (equal(tok, "!cd=", 4))
			{
				new Float:at
				if (TrieGetCell(g_LineAt, key, at) && now - at < str_to_float(tok[4]))
					return 0.0
			}
			else if (equal(tok, "!once=", 6))
			{
				if (TrieKeyExists(g_LineOnce, key))
					return 0.0
			}
			continue
		}
		new op = op_at(tok)
		if (op > 0)
		{
			new k[24], o[3], v[24]
			split_condition(tok, op, k, charsmax(k), o, charsmax(o), v, charsmax(v))
			if (fact_match(id, other, k, o, v) != 1)
				return 0.0
			score++
			if (is_trait_key(k))
				trait = true
			continue
		}
		// the old way: personality letters, mood marks, * for anyone
		new bool:moody
		for (new m = 0; mood[m]; m++)
			if (contain(tok, fmt("%c", mood[m])) != -1)
				moody = true
		if (moody)
		{
			// how it's doing shows whatever the setting
			w = 1.5 + 0.5 * p
			moodset = true
		}
		else if (tok[0] == '*')
			continue
		else if (has_persona(id, tok))
		{
			if (!moodset)
				w *= p
			score++
		}
		else
			return 0.0
	}
	return trait ? w * p : w
}

// Where a condition's operator is (>= <= != = < >), or 0
op_at(const tok[])
{
	for (new i = 1; tok[i]; i++)
		if (tok[i] == '=' || tok[i] == '<' || tok[i] == '>' || (tok[i] == '!' && tok[i + 1] == '='))
			return i
	return 0
}

split_condition(const tok[], op, k[], klen, o[], olen, v[], vlen)
{
	copy(k, min(op, klen), tok)
	new width = (tok[op + 1] == '=') ? 2 : 1
	copy(o, min(width, olen), tok[op])
	copy(v, vlen, tok[op + width])
}

// Conditions on who the bot is (they show more or less with gm_personality), not on what's
// going on
bool:is_trait_key(const k[])
{
	static const TRAITS[][] = { "warmth", "ego", "temper", "talk", "humor", "age", "skill", "region", "role", "hobby", "arch", "style" }
	for (new i = 0; i < sizeof TRAITS; i++)
		if (equal(k, TRAITS[i]))
			return true
	return false
}

// 1 the condition holds, 0 it doesn't, -1 unknown. What's going on is known here; who the
// bot is and what it remembers comes from gm_people
fact_match(id, other, const k[], const o[], const v[])
{
	new Float:have, bool:num = true, str[24]
	if (equal(k, "kills"))
		have = float(g_RoundKills[id])
	else if (equal(k, "deaths"))
		have = float(g_DeathRow[id])
	else if (equal(k, "frags"))
		have = float(get_user_frags(id))
	else if (equal(k, "kd"))
		have = float(get_user_frags(id)) / float(max(get_user_deaths(id), 1))
	else if (equal(k, "alive"))
	{
		new team = get_user_team(id)
		for (new i = 1; i <= MaxClients; i++)
			if (is_user_alive(i) && get_user_team(i) == team)
				have += 1.0
	}
	else if (equal(k, "diff"))
	{
		new team = get_user_team(id)
		if (team != 1 && team != 2)
			return -1
		have = float(g_Score[team] - g_Score[3 - team])
	}
	else if (equal(k, "vs"))
		have = (other > 0 && other <= MaxClients) ? float(g_KilledBy[id][other]) : 0.0
	else if (equal(k, "vsby"))
		have = (other > 0 && other <= MaxClients) ? float(g_KilledBy[other][id]) : 0.0
	else if (equal(k, "joined"))
		have = get_gametime() - g_JoinedAt[id]
	else if (equal(k, "phase"))
	{
		num = false
		copy(str, charsmax(str), !is_user_alive(id) ? "dead" : g_RoundOver ? "end" : g_Freeze ? "freeze" : "live")
	}
	else if (equal(k, "other"))
	{
		num = false
		copy(str, charsmax(str), (!other || !is_user_connected(other)) ? "none" : is_user_bot(other) ? "bot" : "human")
	}
	else if (equal(k, "team"))
	{
		num = false
		new team = get_user_team(id)
		copy(str, charsmax(str), team == 1 ? "t" : team == 2 ? "ct" : "spec")
	}
	else if (equal(k, "time"))
	{
		num = false
		new hr
		time(hr)
		copy(str, charsmax(str), hr < 5 ? "night" : hr < 12 ? "morning" : hr < 18 ? "day" : "evening")
	}
	else if (equal(k, "map"))
	{
		new map[32]
		get_mapname(map, charsmax(map))
		new bool:in = containi(map, v) != -1
		return (o[0] == '!') ? !in : in
	}
	else if (equal(k, "mode"))
	{
		// a prefix: mode=csdm is both deathmatches, mode=gungame both Gun Games
		new bool:in = g_ModeId[0] && equal(g_ModeId, v, strlen(v))
		return (o[0] == '!') ? !in : in
	}
	else if (equal(k, "round"))
	{
		num = false
		round_kind(id, str, charsmax(str))
		if (!str[0])
			return equal(v, "last") ? 0 : -1
		// "last" is on top of the buy (the last round can be an eco too)
		if (equal(v, "last"))
		{
			new left = get_timeleft()
			new bool:last = left > 0 && left < 150
			return (o[0] == '!') ? !last : last
		}
	}
	else if (equal(k, "enemies"))
	{
		new team = get_user_team(id)
		if (team != 1 && team != 2)
			return -1
		for (new i = 1; i <= MaxClients; i++)
			if (is_user_alive(i) && get_user_team(i) == 3 - team)
				have += 1.0
	}
	else if (equal(k, "rude"))
		have = rude_on() ? 1.0 : 0.0
	else if (equal(k, "clanmate"))
		have = (other > 0 && other <= MaxClients && is_user_connected(other) && clanmates(id, other)) ? 1.0 : 0.0
	else if (equal(k, "flag"))
	{
		new Float:until
		new bool:set = TrieGetCell(g_Flags, v, until) && get_gametime() < until
		return (o[0] == '!') ? !set : set
	}
	else
	{
		// traits, archetype, memory: gm_people
		if (!LibraryExists("gm_people", LibType_Library))
			return -1
		return people_match(id, other, k, o, v)
	}
	if (!num)
	{
		new bool:eq = bool:equali(str, v)
		return (o[0] == '!') ? !eq : eq
	}
	new Float:want = str_to_float(v)
	switch (o[0])
	{
		case '>': return o[1] == '=' ? have >= want : have > want
		case '<': return o[1] == '=' ? have <= want : have < want
		case '!': return have != want
	}
	return have == want
}

// Reading what happened, then typing it: people type about 6 to 10 characters a second
// in game, and take a moment before they start
Float:typing_time(const text[], Float:read_min = 0.8, Float:read_max = 2.2)
{
	return random_float(read_min, read_max) + float(strlen(text)) * random_float(0.1, 0.17)
}

queue(id, other, const text[], chain, bool:team, Float:delay, const section[] = "", bool:quiet = false, bool:raw = false)
{
	new slot = -1
	for (new i = 0; i < MAX_PENDING; i++)
		if (!g_PendBot[i])
		{
			slot = i
			break
		}
	if (slot < 0)
		return
	g_PendBot[slot] = id
	g_PendUser[slot] = get_user_userid(id)
	bot_line(id, get_gametime())
	g_PendOther[slot] = other
	g_PendChain[slot] = chain
	g_PendTeam[slot] = team
	copy(g_PendText[slot], charsmax(g_PendText[]), text)
	copy(g_PendSec[slot], charsmax(g_PendSec[]), section)
	g_PendQuiet[slot] = quiet
	g_PendAlive[slot] = bool:is_user_alive(id)
	g_PendRaw[slot] = raw
	g_PendWaited[slot] = 0
	if (delay < 0.0)
		delay = typing_time(text)
	new Float:now = get_gametime()
	g_NextBot[id] = now + delay + bot_gap() * random_float(0.6, 1.4) / floatsqroot(floatmax(g_Chatty[id], 0.15))
	if (!chain)
		g_NextAny = now + delay + global_gap() * random_float(0.6, 1.4)
	set_task(delay, "task_say", TASK_SAY + slot)
	remove_task(TASK_STOP + slot)
	// now and then it stands still for the last of the typing (a long line, nothing going on)
	if (g_Fork && get_pcvar_num(g_pStop) && strlen(text) >= 20 && random_num(1, 100) <= 20 && stop_ok(id))
	{
		new Float:hold = floatmin(delay, 3.0)
		set_task(delay - hold, "task_stop_type", TASK_STOP + slot, fmt("%.1f", hold), 8)
	}
}

// Standing still to type fits: alive in the round with nothing going on, in a mode where
// people stand about (not deathmatch, a match, or a mode a plugin steers the bots through)
bool:stop_ok(id)
{
	if (!is_user_alive(id) || g_Freeze || fighting(id) || g_SteeredMode || match_like())
		return false
	return !get_cvar_num("bot_deathmatch") && get_cvar_num("gm_botstyle") != 0
}

public task_stop_type(const secs[], tid)
{
	new slot = tid - TASK_STOP
	new id = g_PendBot[slot]
	if (!id || !is_user_connected(id) || get_user_userid(id) != g_PendUser[slot] || !stop_ok(id))
		return
	server_cmd("ls_bot %d hold %s bc", id, secs)
	g_TypeHoldUntil[id] = get_gametime() + str_to_float(secs)
	g_TypeBlocked[id] = 0.0
	remove_task(TASK_TYPEHOLD + id)
	set_task(0.3, "task_type_hold", TASK_TYPEHOLD + id, _, _, "b")
}

// Still typing: a teammate stuck behind it (close, barely moving, facing it) for a moment
// gets let past
public task_type_hold(tid)
{
	new id = tid - TASK_TYPEHOLD
	new Float:now = get_gametime()
	if (!is_user_alive(id) || now >= g_TypeHoldUntil[id])
	{
		remove_task(tid)
		return
	}
	new Float:me[3], Float:other[3], Float:vel[3], Float:view[3], Float:fwd[3]
	pev(id, pev_origin, me)
	new team = get_user_team(id), bool:waiting
	for (new m = 1; m <= MaxClients && !waiting; m++)
	{
		if (m == id || !is_user_alive(m) || get_user_team(m) != team)
			continue
		pev(m, pev_origin, other)
		if (get_distance_f(me, other) > 120.0)
			continue
		pev(m, pev_velocity, vel)
		if (floatsqroot(vel[0] * vel[0] + vel[1] * vel[1]) > 40.0)
			continue
		pev(m, pev_v_angle, view)
		angle_vector(view, ANGLEVECTOR_FORWARD, fwd)
		new Float:dx = me[0] - other[0], Float:dy = me[1] - other[1], Float:len = floatsqroot(dx * dx + dy * dy)
		if (len > 1.0 && (dx * fwd[0] + dy * fwd[1]) / len > 0.7)
			waiting = true
	}
	if (!waiting)
	{
		g_TypeBlocked[id] = 0.0
		return
	}
	if (g_TypeBlocked[id] == 0.0)
		g_TypeBlocked[id] = now
	else if (now - g_TypeBlocked[id] >= 0.9)
	{
		server_cmd("ls_bot %d release bc", id)
		g_TypeHoldUntil[id] = 0.0
		remove_task(tid)
	}
}

public task_say(task)
{
	new slot = task - TASK_SAY
	new id = g_PendBot[slot], other = g_PendOther[slot], chain = g_PendChain[slot]
	// still the same bot (the slot may have gone to someone else meanwhile)
	if (!is_user_connected(id) || !is_user_bot(id) || get_user_userid(id) != g_PendUser[slot])
	{
		g_PendBot[slot] = 0
		return
	}
	// alive and busy: nobody types while afk; in a fight (or with danger around) it waits a
	// moment, and if it's still fighting it lets it go, a reply too (lines with chain
	// MAX_CHAIN, like leaving, always go)
	if (is_user_alive(id) && chain < MAX_CHAIN)
	{
		if (bot_afk(id))
		{
			g_PendBot[slot] = 0
			return
		}
		// (deathmatch is one long fight: a short reaction goes out between shots, a longer
		// line waits for a gap; elsewhere an enemy in view or being shot puts it off, not just
		// danger somewhere around)
		new bool:dm = dm_like()
		if (in_combat(id) || bot_target(id))
		{
			if (dm && strlen(g_PendText[slot]) <= 16 && !bot_target(id))
			{
				// said
			}
			else if (g_PendWaited[slot] < (dm ? 3 : 2))
			{
				g_PendWaited[slot]++
				set_task(dm ? random_float(1.0, 2.0) : random_float(2.0, 4.0), "task_say", task)
				return
			}
			else
			{
				g_PendBot[slot] = 0
				return
			}
		}
	}
	g_PendBot[slot] = 0
	if (other && !is_user_connected(other))
		other = 0

	new text[128], tag[24]
	copy(text, charsmax(text), g_PendText[slot])
	new bool:team = g_PendTeam[slot]
	// the model's text is said as it came (no tags, placeholders or added typos)
	if (!g_PendRaw[slot])
	{
		// a reply asked for: "...  >tag"
		new at = contain(text, ">")
		if (at > 0)
		{
			copy(tag, charsmax(tag), text[at + 1])
			trim(tag)
			text[at] = 0
			trim(text)
		}
		// "@..." is said to the team
		if (text[0] == '@')
		{
			team = true
			copy(text, charsmax(text), text[1])
			trim(text)
		}
		fill(id, other, text, charsmax(text), g_PendSec[slot])
		style_text(id, text)
		decorate(id, text, charsmax(text))
		// killed while typing: it hits enter on what it had ("nice sh")
		new l = strlen(text)
		if (g_PendAlive[slot] && !is_user_alive(id) && l > 8 && random_num(1, 3) == 1)
			text[random_num(l * 2 / 5, l * 3 / 4)] = 0
		// the chat key's letter in front, now and then ("ygo b", "uok"): typing as the key's pressed
		else if (g_Typos[id] >= 8 && random_num(1, 150) == 1)
		{
			new out[128]
			formatex(out, charsmax(out), "%c%s", team ? 'u' : 'y', text)
			copy(text, charsmax(text), out)
		}
	}
	if (!text[0])
		return
	engclient_cmd(id, team ? "say_team" : "say", text)
	hist_add(id, text, team ? get_user_team(id) : 0)
	g_LastSpoke = id
	g_LastSpokeAt = get_gametime()
	// the living don't see the dead talk; like pub servers' dead-chat plugins, show it to them
	if (!is_user_alive(id) && get_pcvar_num(g_pDeadChat))
		dead_chat(id, text, team)
	// said to a person who can see it: they may answer, and this bot carries on (a thread)
	if (other && !is_user_bot(other) && !g_PendQuiet[slot] && (!team || get_user_team(other) == get_user_team(id)))
		thread_said(other, id, g_PendSec[slot], text)

	// no answer asked for: now and then someone answers anyway, like on a real server
	// (the victim to the killer's gloat, the killer to "wtf", a thanks for "nice shot")
	if (!tag[0] && !chain && !g_PendRaw[slot])
		follow_up(id, other, g_PendSec[slot], team)

	// someone answers: the one it was about if that's a bot (and can see it), else
	// another bot (a teammate, for team chat)
	if (tag[0] && chain < MAX_CHAIN)
	{
		new who
		if (other && is_user_bot(other) && other != id && (!team || get_user_team(other) == get_user_team(id)))
			who = other
		else
			who = team ? any_bot_on(get_user_team(id), id) : any_bot(id)
		want(who, fmt("r_%s", tag), id, 0.5, -1.0, chain + 1, team)
	}
}

// Sections where the name is the point: greetings, welcomes, someone coming back, a clan, a
// player remembered
bool:names_kept(const section[])
{
	static const KEEP[][] = { "greet", "welcome", "rejoin", "remember", "remember_rival", "clan", "clan_join", "clan_avenge", "r_hi" }
	for (new i = 0; i < sizeof KEEP; i++)
		if (equal(section, KEEP[i]))
			return true
	return false
}

// A name only there to say who the line's to: "{other}, nice" or "np {other}". Only then:
// "{other} is hacking" or "i hate {other}" are about them, and stay as they are
drop_address(text[], len)
{
	static const ADDRESS[][] = { "np", "ty", "gg", "gj", "nice", "sry", "sorry", "lol", "thx", "move", "hi", "hey", "yo" }
	// (team chat's leading @ stays where it is)
	new at = text[0] == '@' ? 1 : 0
	new l = strlen(text)
	if (l > at + 8 && equal(text[at], "{other}", 7) && (text[at + 7] == ',' || text[at + 7] == ':'))
	{
		new rest = at + 8
		while (text[rest] == ' ')
			rest++
		copy(text[at], len - at, text[rest])
		return
	}
	if (l > 8 && equal(text[l - 7], "{other}"))
	{
		// the word before the name
		new e = l - 7
		while (e > 0 && (text[e - 1] == ' ' || text[e - 1] == ','))
			e--
		new b = e
		while (b > 0 && text[b - 1] != ' ' && text[b - 1] != '@')
			b--
		new word[16]
		copy(word, min(e - b, charsmax(word)), text[b])
		for (new i = 0; i < sizeof ADDRESS; i++)
		{
			if (equali(word, ADDRESS[i]))
			{
				text[e] = 0
				return
			}
		}
	}
}

fill(id, other, text[], len, const section[] = "")
{
	choose(text, len)
	new name[32]
	if (contain(text, "{hobby}") != -1)
	{
		new hobby[16]
		if (LibraryExists("gm_people", LibType_Library))
			people_hobby(id, random_num(0, 1), hobby, charsmax(hobby))
		replace_string(text, len, "{hobby}", hobby[0] ? hobby : "wc3")
	}
	if (contain(text, "{name}") != -1)
	{
		short_name(id, name, charsmax(name))
		replace_string(text, len, "{name}", name)
	}
	// said to someone ("np {other}", "{other} learn to aim"): people mostly leave the name
	// off once they're talking, so only greetings and first mentions keep it every time
	if (contain(text, "{other}") != -1 && !names_kept(section) && random_num(1, 100) > 30)
		drop_address(text, len)
	if (contain(text, "{other}") != -1)
	{
		if (other && is_user_connected(other))
			short_name(other, name, charsmax(name))
		else
			copy(name, charsmax(name), "man")
		replace_string(text, len, "{other}", name)
	}
	if (contain(text, "{tag}") != -1)
	{
		new tag[16]
		clan_of(id, tag, charsmax(tag))
		replace_string(text, len, "{tag}", tag[0] ? tag : "the clan")
	}
	if (contain(text, "{map}") != -1)
	{
		new map[32]
		get_mapname(map, charsmax(map))
		casual_map(map, name, charsmax(name))
		replace_string(text, len, "{map}", name)
	}
}

// What people call each other: the name without its clan tag, often just the start
short_name(id, out[], len)
{
	base_name(id, out, len)
	if (strlen(out) > 8 && random_num(1, 100) <= 40)
		out[random_num(4, 6)] = 0
}

// The name without its clan tag
base_name(id, out[], len)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	trim(name)
	ls_bare_name(name, out, len)
}

// Casual typers: lowercase, no full stop, the odd swapped letters
style_text(id, text[])
{
	new p = PERSONAS[g_Persona[id]]
	// a line already in capitals is shouted on purpose
	new bool:shout = true
	for (new i = 0; text[i]; i++)
		if (text[i] >= 'a' && text[i] <= 'z')
			shout = false
	new bool:old = LibraryExists("gm_people", LibType_Library) && people_get(id, "age") == 3
	if (!shout && !old && contain("njcstkxhzpbdumAHLOS", fmt("%c", p)) != -1)
	{
		if (random_num(1, 100) <= 80)
			strtolower(text)
		new l = strlen(text)
		if (l > 1 && text[l - 1] == '.')
			text[l - 1] = 0
	}
	if (random_num(1, 100) <= g_Typos[id] + ((p == 'n' || p == 'k') ? 8 : 0))
	{
		new l = strlen(text)
		if (l > 4)
		{
			new i = random_num(1, l - 3)
			if (text[i] != ' ' && text[i + 1] != ' ')
			{
				new c = text[i]
				text[i] = text[i + 1]
				text[i + 1] = c
			}
		}
	}
}

// The same line never comes out quite the same: a stretched word, an ending each
// personality likes, very rarely shouting. All of it scales with gm_personality.
decorate(id, text[], len)
{
	new p = PERSONAS[g_Persona[id]]
	new Float:pm = personality()
	new l = strlen(text)
	if (!l || text[0] == '/')
		return
	// stretch the last vowel of a short word: nice -> niiice, no -> nooo
	if (l <= 8 && (p == 'j' || p == 'f' || p == 'n' || p == 's') && random_float(0.0, 100.0) <= 5.0 * pm)
	{
		for (new i = l - 1; i >= 0; i--)
		{
			if (contain("aeiouAEIOU", fmt("%c", text[i])) != -1)
			{
				new out[128], k
				for (new j = 0; j < l && k < charsmax(out) - 3; j++)
				{
					out[k++] = text[j]
					if (j == i)
					{
						out[k++] = text[j]
						out[k++] = text[j]
					}
				}
				out[k] = 0
				copy(text, len, out)
				break
			}
		}
	}
	// its own laugh and text-speak
	if (g_Laugh[id] > 2)
	{
		replace_word(text, len, "lol", LAUGHS[g_Laugh[id]])
		replace_word(text, len, "haha", LAUGHS[g_Laugh[id]])
	}
	// "c1" for ok: an old habit from the radio menu days
	if (LibraryExists("gm_people", LibType_Library) && people_get(id, "age") >= 2 && people_get(id, "skill") >= 2 && random_num(1, 4) == 1)
		replace_word(text, len, "ok", "c1")
	if (g_TextSpeak[id])
	{
		static const FROM[][] = { "you're", "your", "you", "thanks", "please", "though", "probably", "because", "okay", "really", "people", "tonight" }
		static const TO[][] = { "ur", "ur", "u", "thx", "pls", "tho", "prob", "cuz", "k", "rly", "ppl", "tn" }
		for (new w = 0; w < sizeof FROM; w++)
			replace_word(text, len, FROM[w], TO[w])
	}
	// already ends in a laugh, an emoticon or a question: nothing more on the end
	l = strlen(text)
	new last = l ? text[l - 1] : 0
	new bool:ended = last == '?' || last == ')' || last == '(' || last == 'D' || last == 'P' || last == '3' || last == '>'
		|| (l >= 3 && (containi(text[l - 3], "lol") != -1 || containi(text[l - 3], "kek") != -1 || containi(text[l - 2], "xd") != -1 || containi(text[l - 3], "mao") != -1 || containi(text[l - 3], "aha") != -1 || containi(text[l - 3], "ehe") != -1))
	if (!ended)
	{
		// the word it keeps using
		if (WORDS[g_Word[id]][0] && strlen(text) < 30 && random_float(0.0, 100.0) <= 7.0 * pm)
			add(text, len, fmt(" %s", WORDS[g_Word[id]]))
		// its emoticon, or an ending its personality likes (with its own laugh)
		else if (EMOTES[g_Emote[id]][0] && random_float(0.0, 100.0) <= 6.0 * pm)
			add(text, len, fmt(" %s", EMOTES[g_Emote[id]]))
		else
		{
			new laugh[12]
			copy(laugh, charsmax(laugh), LAUGHS[g_Laugh[id]])
			new Float:r = random_float(0.0, 100.0) / floatmax(pm, 0.01)
			switch (p)
			{
				case 'f': if (r <= 8.0) add(text, len, random_num(0, 1) ? " :)" : " :D")
				case 'j': if (r <= 10.0) add(text, len, random_num(0, 1) ? fmt(" %s", laugh) : " :P")
				case 'n': if (r <= 8.0) add(text, len, "?")
				case 's': if (r <= 8.0) add(text, len, random_num(0, 1) ? "..." : " -_-")
				case 'c': if (r <= 6.0) add(text, len, fmt(" %s", laugh))
				case 'o': if (r <= 8.0) add(text, len, ".")
				case 'k': if (r <= 10.0) add(text, len, random_num(0, 1) ? "!" : fmt(" %s", laugh))
			}
		}
	}
	// shouting is rare: only the loud ones, now and then, and short lines
	if (g_Shouty[id] > 1 && (p == 'j' || p == 's' || p == 'k' || p == 'h') && random_float(0.0, 100.0) <= 1.5 * pm && strlen(text) <= 20)
		strtoupper(text)
}

// `from` as a whole word (any case) replaced by `to`
replace_word(text[], len, const from[], const to[])
{
	new fl = strlen(from), at, start = 0
	while ((at = containi(text[start], from)) != -1)
	{
		new s = start + at
		new before = s > 0 ? text[s - 1] : ' ', after = text[s + fl]
		if (!isalnum(before) && !isalnum(after) && before != 39 && after != 39)
		{
			new rest[128]
			copy(rest, charsmax(rest), text[s + fl])
			text[s] = 0
			add(text, len, to)
			add(text, len, rest)
			start = s + strlen(to)
		}
		else
			start = s + 1
		if (start >= strlen(text))
			break
	}
}

// A random bot other than `not`, or 0
any_bot(not)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (i != not && is_user_connected(i) && is_user_bot(i))
			list[n++] = i
	return n ? list[random(n)] : 0
}

any_bot_on(team, not)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (i != not && is_user_connected(i) && is_user_bot(i) && get_user_team(i) == team)
			list[n++] = i
	return n ? list[random(n)] : 0
}

// ---- what happens ---------------------------------------------------------------------

any_dead_bot_on(team)
{
	new list[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && is_user_bot(i) && !is_user_alive(i) && get_user_team(i) == team)
			list[n++] = i
	return n ? list[random(n)] : 0
}

// One bot left on its side against three or more
last_alive_check(team)
{
	new alive, last, enemies
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_alive(i))
			continue
		new t = get_user_team(i)
		if (t == team)
		{
			alive++
			last = i
		}
		else if (t == 1 || t == 2)
			enemies++
	}
	if (alive == 1 && enemies >= 3 && is_user_bot(last))
		want(last, "last_alive", 0, 0.45, -1.0, 0, random_num(0, 1) == 1)
}

// Modes without rounds (Gun Game, Deathmatch) have no freeze time, when most chat happens:
// every so often someone says something anyway
public task_idle_chatter()
{
	set_task(random_float(25.0, 50.0), "task_idle_chatter")
	if (get_gametime() - g_LastRound < 150.0)
		return
	new h
	time(h)
	want(any_bot(0), (h >= 0 && h < 5 && random_num(0, 1)) ? "late_night" : "idle", 0, 0.35, random_float(1.0, 4.0))
}

public ev_newround()
{
	g_Freeze = true
	g_RoundOver = false
	g_LastRound = get_gametime()
	g_Round++
	arrayset(g_RoundKills, 0, sizeof g_RoundKills)
	arrayset(g_Streaked, false, sizeof g_Streaked)
	g_BombPlanted = g_BombDropped = false
	g_BombPlace[0] = g_SeenPlace[1][0] = g_SeenPlace[2][0] = 0
	// last round's lines still being typed are dropped, apart from answers to people (and
	// lines like leaving)
	for (new i = 0; i < MAX_PENDING; i++)
	{
		if (!g_PendBot[i] || g_PendChain[i] >= MAX_CHAIN)
			continue
		new o = g_PendOther[i]
		if (g_PendChain[i] > 0 && o && is_user_connected(o) && !is_user_bot(o))
			continue
		g_PendBot[i] = 0
		remove_task(TASK_SAY + i)
	}
	g_RoundNo++
	for (new i = 1; i <= MaxClients; i++)
		req_clear(i)
	set_task(0.5, "task_start_money")
	// a match: half-time (everyone's on the other side now) or overtime (everyone has $10000)
	if (match_like() && !teams_off())
	{
		new swapped, stayed, rich, players
		for (new i = 1; i <= MaxClients; i++)
		{
			if (!is_user_connected(i))
				continue
			new t = get_user_team(i)
			if (t == 1 || t == 2)
			{
				players++
				if (g_LastTeam[i] == 3 - t)
					swapped++
				else if (g_LastTeam[i] == t)
					stayed++
				if (cs_get_user_money(i) >= 10000)
					rich++
			}
		}
		if (swapped >= 2 && swapped > stayed * 2)
			want(any_bot(0), "halftime", 0, 0.8, random_float(1.5, 5.0), 0, random_num(0, 1) == 1)
		else if (players >= 2 && rich == players && g_Score[1] + g_Score[2] == 0 && !g_Overtime && g_RoundNo > 3)
		{
			g_Overtime = true
			want(any_bot(0), "overtime", 0, 0.8, random_float(1.5, 5.0))
		}
	}
	for (new i = 1; i <= MaxClients; i++)
		g_LastTeam[i] = is_user_connected(i) ? get_user_team(i) : 0
	// freeze time is when people chat most: the round (a pistol round, an eco, the last
	// one), small talk, or the usual
	if (random_num(1, 100) <= 22)
	{
		new h, b = any_bot(0)
		time(h)
		new r = random_num(1, 100)
		if (h >= 0 && h < 5 && r <= 40)
			want(b, "late_night", 0, 0.6, random_float(2.0, 9.0))
		else if (r <= 35 && !teams_off() && !g_SteeredMode)
			want(b, "freeze_round", 0, 0.6, random_float(2.0, 8.0), 0, random_num(1, 100) <= 60)
		else if (r <= 55 && !match_like())
			want(b, "smalltalk", 0, 0.5, random_float(2.0, 9.0))
		else
			want(b, "idle", 0, 0.6, random_float(2.0, 9.0))
	}
}

public ev_death()
{
	new killer = read_data(1), victim = read_data(2), hs = read_data(3)
	new weapon[24]
	read_data(4, weapon, charsmax(weapon))
	if (victim < 1 || victim > MaxClients)
		return
	// (no teams underneath, as in Gun Game or free for all: a kill on the same side is just a kill)
	new bool:tk = !teams_off() && killer >= 1 && killer <= MaxClients && killer != victim && get_user_team(killer) == get_user_team(victim)
	note_event(killer, victim, weapon, hs != 0)
	if (is_user_bot(victim))
		req_clear(victim)

	if (killer < 1 || killer > MaxClients || killer == victim)
	{
		g_DeathRow[victim]++
		want(victim, "death_self", 0, 0.25)
		return
	}
	if (tk)
	{
		if (!is_user_bot(victim))
		{
			g_KillerOf[victim] = killer
			g_KillerOfAt[victim] = get_gametime()
		}
		want(victim, "teamkilled", killer, 0.8)
		want(killer, "teamkill", victim, 0.7)
		return
	}

	// a bot just killed: watched for a teabag over its body
	if (is_user_bot(victim) && is_user_connected(killer))
		watch_tbag(victim, killer)

	// who was involved, for when a person says something about it
	new Float:now = get_gametime()
	if (is_user_bot(killer))
	{
		g_LastBotKiller = killer
		g_LastBotKillAt = now
	}
	else
	{
		g_VictimOf[killer] = victim
		g_VictimOfAt[killer] = now
	}
	if (!is_user_bot(victim))
	{
		g_KillerOf[victim] = killer
		g_KillerOfAt[victim] = now
		// how they died, for "how?" and "where is he"
		g_DiedAt[victim] = now
		g_KillHs[victim] = hs != 0
		copy(g_KillerPlace[victim], charsmax(g_KillerPlace[]), g_Place[killer])
		casual_weapon(weapon, g_KillWeapon[victim], charsmax(g_KillWeapon[]))
	}
	// the victim's team now knows where one of them is
	new vt = get_user_team(victim)
	if ((vt == 1 || vt == 2) && g_Place[killer][0])
	{
		copy(g_SeenPlace[vt], charsmax(g_SeenPlace[]), g_Place[killer])
		g_SeenAt[vt] = now
	}
	// the dead tell their team where the killer was, with what, how hurt; one that saw a group
	// just before dying says that instead, more often than not ("3 b")
	if (is_user_bot(victim) && (vt == 1 || vt == 2) && !get_cvar_num("mp_freeforall") && !spot_last_words(victim))
		callout(victim, killer, weapon)

	g_RoundKills[killer]++
	// deathmatch: a streak is per life
	if (dm_like())
	{
		g_RoundKills[victim] = 0
		g_Streaked[victim] = false
	}
	// rivals: someone who keeps killing a bot this map, and the bot finally getting them
	new was = g_KilledBy[victim][killer]
	g_KilledBy[killer][victim]++
	if (is_user_bot(victim) && g_KilledBy[killer][victim] == 4)
		want(victim, "nemesis", killer, 0.6, -1.0, 1)
	else if (is_user_bot(killer) && was >= 4 && random_num(1, 100) <= 70)
		want(killer, "nemesis_won", victim, 0.8, -1.0, 1)
	g_DeathRow[killer] = 0
	g_Raged[killer] = false
	if (++g_DeathRow[victim] >= 3 && mood(victim))
		return
	if (g_RoundKills[killer] >= 3 && !g_Streaked[killer])
	{
		g_Streaked[killer] = true
		want(killer, "streak", victim, 0.35)
		want(any_bot_on(get_user_team(victim), victim), "enemy_streak", killer, 0.35)
	}

	if (g_RoundKills[killer] == 5 && !dm_like())
		want(killer, "ace", 0, 0.8, -1.0, 1)
	// scoped rifles without the scope (the line's theme keeps it to one now and then)
	if (is_user_bot(killer) && (equal(weapon, "awp") || equal(weapon, "scout")) && cs_get_user_zoom(killer) == CS_SET_NO_ZOOM)
		want(killer, "noscope", victim, 0.3)
	// someone dead and watching says something about a teammate's kill
	new watcher = any_dead_bot_on(get_user_team(killer))
	if (watcher && random_num(1, 100) <= 4)
		want(watcher, "spectating", killer, 0.6, -1.0, 0, true)
	last_alive_check(get_user_team(victim))

	new bool:knife = bool:equal(weapon, "knife"), bool:nade = bool:equal(weapon, "grenade")
	new how = how_killed(killer, victim, weapon)
	if (how == HOW_WALL)
	{
		want(victim, "death_wallbang", killer, 0.5)
		want(killer, "kill_wallbang", victim, 0.3)
		// a dead teammate of the victim's, watching, says something about it (never the
		// killer's own side: they'd be calling their teammate a hacker)
		new other = random_num(1, 100) <= 30 ? any_dead_bot_on(get_user_team(victim)) : 0
		if (other && other != victim && other != killer)
			want(other, "wallbang_seen", killer, 0.4)
	}
	else if (how == HOW_DEFUSE)
		want(victim, "death_defusing", killer, 0.6)
	else if (how == HOW_DEFUSE_CLOSE)
		want(victim, "death_defusing_close", killer, 0.65)
	else if (how == HOW_PLANT)
		want(victim, "death_planting", killer, 0.6)
	else if (how == HOW_RELOAD)
		want(victim, "death_reloading", killer, 0.25)
	else if (knife)
	{
		want(killer, "kill_knife", victim, 0.35)
		want(victim, "death_knife", killer, 0.45)
	}
	else if (nade)
	{
		want(killer, "kill_nade", victim, 0.2)
		want(victim, "death_nade", killer, 0.3)
	}
	else if (hs)
	{
		want(killer, "kill_headshot", victim, 0.15)
		want(victim, "death_headshot", killer, 0.18)
	}
	else
	{
		want(killer, equal(weapon, "awp") ? "kill_awp" : "kill", victim, 0.06)
		want(victim, "death", killer, 0.1)
	}
	// a clanmate on the killer's team cheers them on; one on the victim's side is out for revenge
	new mate = clanmate_bot(killer, true)
	if (mate)
		want(mate, "clan", killer, 0.18, -1.0, 0, true)
	new avenger = clanmate_bot(victim, true)
	if (avenger && random_num(1, 100) <= 50)
		want(avenger, "clan_avenge", killer, 0.25, -1.0, 0, true)
	// how a person plays: awping, camping, rushing (the bots notice)
	if (!is_user_bot(killer) && is_user_bot(victim))
		notice_play(killer, victim, weapon)
	// a teammate of the killer says something about it; about a person's kill, now and
	// then with the gun or the kind of kill in it ("deag ns", "clean hs")
	if (!is_user_bot(killer))
	{
		new mate = any_bot_on(get_user_team(killer), killer)
		casual_weapon(weapon, g_CtxWeapon, charsmax(g_CtxWeapon))
		g_CtxHs = hs != 0
		want(mate, knife ? "human_kill_knife" : nade ? "human_kill_nade" : hs ? "human_kill_hs" : "human_kill", killer,
			(hs || knife || nade) ? 0.2 : 0.07, -1.0, 0, random_num(1, 100) <= 40)
		g_CtxWeapon[0] = 0
		g_CtxHs = false
	}
	else if (hs || knife)
		want(any_bot_on(get_user_team(killer), killer), "nice_shot", killer, 0.1, -1.0, 0, true)
}

// A callout in team chat from someone just killed ("1 long", "awp mid", "hes low"): mostly
// the team-minded (callers, tryhards, would-be pros, backseaters, the military ones), the
// odd one from anyone. The place is the killer's on the radar (maps whose nav file names
// its places), else the gun, else something plain
callout(victim, killer, const weapon[])
{
	new p = PERSONAS[g_Persona[victim]]
	new Float:base = contain("itrEbN", fmt("%c", p)) != -1 ? 0.4 : contain("qLdu", fmt("%c", p)) != -1 ? 0.0 : 0.07
	if (base <= 0.0)
		return
	copy(g_CtxPlace, charsmax(g_CtxPlace), g_Place[killer])
	casual_weapon(weapon, g_CtxWeapon, charsmax(g_CtxWeapon))
	// (the knife and a grenade aren't worth naming)
	if (equal(g_CtxWeapon, "knife") || equal(g_CtxWeapon, "nade"))
		g_CtxWeapon[0] = 0
	new hp = is_user_alive(killer) ? get_user_health(killer) : 0
	want(victim, (hp > 0 && hp < 40) ? "callout_low" : "callout", killer, base, random_float(0.8, 2.0), 0, true)
	g_CtxPlace[0] = g_CtxWeapon[0] = 0
}

// A gun's name as people say it ("deag" for deagle); "" for ones nobody would name
casual_weapon(const weapon[], out[], len)
{
	static const FROM[][] = { "deagle", "ak47", "m4a1", "awp", "scout", "usp", "glock18", "mp5navy", "famas", "galil",
		"p90", "m3", "xm1014", "aug", "sg552", "m249", "elite", "fiveseven", "ump45", "mac10", "tmp", "p228", "knife", "grenade", "sg550", "g3sg1" }
	static const TO[][] = { "deag", "ak", "m4", "awp", "scout", "usp", "glock", "mp5", "famas", "galil",
		"p90", "pump", "auto shotty", "aug", "krieg", "para", "dualies", "five seven", "ump", "mac10", "tmp", "p228", "knife", "nade", "auto", "auto" }
	out[0] = 0
	for (new i = 0; i < sizeof FROM; i++)
		if (equal(weapon, FROM[i]))
		{
			copy(out, len, TO[i])
			return
		}
}

// A place name from the game ("#BombsiteB", "LongHall", "CTSpawn") as people say it
// ("b", "long", "ct spawn")
casual_place(const raw[], out[], len)
{
	new name[32]
	copy(name, charsmax(name), raw[raw[0] == '#' ? 1 : 0])
	out[0] = 0
	if (!name[0])
		return
	// what people call a place on the well-known maps (the nav's name, as the map's regulars
	// say it)
	static const MAPS[][] = { "dust2", "dust2", "dust2", "dust2", "dust2", "dust2", "dust2", "dust2",
		"inferno", "inferno", "inferno", "inferno", "inferno", "inferno", "nuke", "nuke", "nuke", "nuke", "nuke",
		"train", "train", "train", "aztec", "aztec", "aztec", "office", "office", "office", "office",
		"assault", "assault", "assault", "italy", "italy", "italy", "italy" }
	static const MFROM[][] = { "Catwalk", "UpperTunnel", "LowerTunnel", "Tunnels", "LongDoors", "OutsideLong", "TopofMid", "ARamp",
		"Apartments", "Banana", "Library", "Balcony", "Boiler", "Arch", "Ramp", "Lobby", "Hut", "Secret", "Vents",
		"Ladder", "Tracks", "Bridge", "Bridge", "Water", "DoubleDoors", "Garage", "Kitchen", "ConferenceRoom", "SideHall",
		"Roof", "Vents", "Warehouse", "Market", "Apartments", "WineCellar", "LongHall" }
	static const MTO[][] = { "cat", "upper", "lower", "tunnels", "long doors", "outside long", "top mid", "a ramp",
		"apps", "banana", "library", "balcony", "boiler", "arch", "ramp", "lobby", "hut", "secret", "vent",
		"ladder", "tracks", "bridge", "bridge", "water", "double doors", "garage", "kitchen", "conference", "side hall",
		"roof", "vents", "warehouse", "market", "apartments", "wine cellar", "long hall" }
	new map[32]
	get_mapname(map, charsmax(map))
	for (new i = 0; i < sizeof MFROM; i++)
		if (equali(name, MFROM[i]) && contain(map, MAPS[i]) != -1)
		{
			copy(out, len, MTO[i])
			return
		}
	static const FROM[][] = { "BombsiteA", "BombsiteB", "BombsiteC", "Middle", "MiddleHall", "CTSpawn", "TSpawn", "LongHall", "ShortHall", "Hostages", "HostageRescueZone" }
	static const TO[][] = { "a", "b", "c", "mid", "mid", "ct spawn", "t spawn", "long", "short", "hostages", "rescue" }
	for (new i = 0; i < sizeof FROM; i++)
		if (equali(name, FROM[i]))
		{
			copy(out, len, TO[i])
			return
		}
	// "UpperTunnel" -> "upper tunnel"
	new k
	for (new i = 0; name[i] && k < len - 1; i++)
	{
		new c = name[i]
		if (i > 0 && c >= 'A' && c <= 'Z' && !(name[i - 1] >= 'A' && name[i - 1] <= 'Z') && k < len - 2)
			out[k++] = ' '
		else if (c == '_')
			c = ' '
		out[k++] = tolower(c)
	}
	out[k] = 0
}

// ---- game state -------------------------------------------------------------------------

// Where someone is: the place names on the radar (only maps whose nav file names its places)
public ev_location()
{
	new id = read_data(1), raw[32]
	read_data(2, raw, charsmax(raw))
	if (id >= 1 && id <= MaxClients && raw[0] && raw[1])
	{
		casual_place(raw, g_Place[id], charsmax(g_Place[]))
		g_NavPlaces = true
		// where that place is, for "go long"
		if (is_user_alive(id) && g_Place[id][0])
		{
			new Float:o[3]
			pev(id, pev_origin, o)
			TrieSetArray(g_PlaceAt, g_Place[id], o, 3)
			place_words(g_Place[id])
			place_learned(g_Place[id], o)
		}
	}
}

public ev_teamscore()
{
	new team[16]
	read_data(1, team, charsmax(team))
	new score = read_data(2)
	if (equal(team, "TERRORIST"))
		g_Score[1] = score
	else if (equal(team, "CT"))
		g_Score[2] = score
}

public ev_damage(victim)
{
	new Float:now = get_gametime()
	if (victim >= 1 && victim <= MaxClients)
	{
		g_FightAt[victim] = now
		new attacker = get_user_attacker(victim)
		if (attacker >= 1 && attacker <= MaxClients)
		{
			g_FightAt[attacker] = now
			g_Shots[attacker] = 0
		}
	}
}

public ev_bomb_dropped()
{
	new id = get_loguser_index()
	g_BombDropped = true
	g_BombPlace[0] = 0
	if (id)
		copy(g_BombPlace, charsmax(g_BombPlace), g_Place[id])
}

public ev_bomb_got()
{
	g_BombDropped = false
}

// Dying again and again: a bot tilts, then crashes out; teammates may cheer them up.
// True when something was said about it (the usual death line is skipped then).
bool:mood(victim)
{
	new row = g_DeathRow[victim], team = get_user_team(victim)
	// someone on the team cheers them up (people too)
	if (row == 5 || row == 8)
	{
		new mate = clanmate_bot(victim, true)
		want(mate ? mate : any_bot_on(team, victim), "support", victim, mate ? 0.45 : 0.2, -1.0, 0, true)
	}
	if (!is_user_bot(victim))
		return false
	new p = PERSONAS[g_Persona[victim]]
	if (row >= 6 && !g_Raged[victim] && (p == 's' || p == 'j' || p == 't' || p == 'O' || p == 'H' || random_num(1, 100) <= 25))
	{
		g_Raged[victim] = true
		crash_out(victim)
		return true
	}
	if (row == 4)
	{
		want(victim, "tilt", 0, 0.35)
		return true
	}
	return false
}

// A few angry lines in quick succession; a salty one may leave for good
new Float:g_RageAt                          // the last rage burst (one a minute, server-wide)

crash_out(id)
{
	new Float:now = get_gametime()
	if (g_Career || !get_pcvar_num(g_pLevel) || now - g_RageAt < 60.0 || rate_capped(now)
		|| random_float(0.0, 1.0) > 0.8 * level_mult() * floatmin(personality() * mood_factor("rage"), 3.0))
		return
	g_RageAt = now
	new text[128], Float:at = random_float(1.0, 2.0)
	new n = random_num(1, 3)
	for (new i = 0; i < n; i++)
	{
		if (!pick_line(id, "rage", text, charsmax(text)))
			break
		queue(id, 0, text, MAX_CHAIN, false, at)
		cap_count(now)
		at += typing_time(text, 0.3, 1.0)
	}
	// the hot-headed quit a third of the time, anyone now and then
	new p = PERSONAS[g_Persona[id]]
	if (random_num(1, 100) <= (contain("stOH", fmt("%c", p)) != -1 ? 25 : 5) && pick_line(id, "ragequit", text, charsmax(text)))
	{
		queue(id, 0, text, MAX_CHAIN, false, at + 0.8)
		set_task(at + 2.5, "task_ragequit", TASK_RAGEQUIT + get_user_userid(id))
	}
	g_NextAny = get_gametime() + at + global_gap()
}

public task_ragequit(tid)
{
	new userid = tid - TASK_RAGEQUIT
	new id = find_player("k", userid)
	if (id && is_user_bot(id))
		server_cmd("kick #%d", userid)
}

public ev_twin()
{
	round_end(1)
}

public ev_ctwin()
{
	round_end(2)
}

// a draw (the time ran out on a map with no objective): the round's over all the same
public ev_draw()
{
	round_end(0)
}

round_end(winner)
{
	g_RoundOver = true
	// the round's over: threads with people end here, and answers still being written
	arrayset(g_ThreadBot, 0, sizeof g_ThreadBot)
	g_Round++
	notice_topfrag()
	// deathmatch-type modes don't celebrate rounds (if one ends there at all)
	if (winner != 1 && winner != 2 || g_GunGame || equal(g_ModeId, "csdm", 4))
	{
		g_WinRun[1] = g_WinRun[2] = 0
		return
	}
	new loser = 3 - winner
	new bool:comeback = g_LoseRun[winner] >= 4
	g_WinRun[winner]++
	g_LoseRun[winner] = 0
	g_LoseRun[loser]++
	g_WinRun[loser] = 0
	if (comeback)
		want(any_bot_on(winner, 0), "comeback", 0, 0.6, -1.0, 0, random_num(0, 1) == 1)
	else if (g_WinRun[winner] == 4 || g_WinRun[winner] == 7 || g_WinRun[winner] == 10)
		want(any_bot_on(winner, 0), "winning_streak_team", 0, 0.6, -1.0, 0, true)
	if (g_LoseRun[loser] == 4 || g_LoseRun[loser] == 7 || g_LoseRun[loser] == 10)
		want(any_bot_on(loser, 0), "losing_streak_team", 0, 0.6, -1.0, 0, true)
	// survived with almost nothing left
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && is_user_bot(i) && get_user_health(i) <= 15 && random_num(1, 100) <= 40)
		{
			want(i, "low_hp", 0, 0.6)
			break
		}
	// the last one standing on the winning side, if it did a lot of the work
	new alive, last
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == winner)
		{
			alive++
			last = i
		}
	if (alive == 1 && g_RoundKills[last] >= 2)
	{
		want(last, "clutch", 0, 0.5)
		want(any_bot_on(winner, last), "teammate_clutch", last, 0.5)
		return
	}
	want(any_bot_on(winner, 0), "round_win", 0, 0.12, -1.0, 0, random_num(0, 1) == 1)
	want(any_bot_on(3 - winner, 0), "round_lose", 0, 0.12, -1.0, 0, random_num(0, 1) == 1)
}

public ev_planted()
{
	new id = get_loguser_index()
	g_BombPlanted = true
	g_BombDropped = false
	if (id)
		copy(g_BombPlace, charsmax(g_BombPlace), g_Place[id])
	// the terrorists say so in team chat
	want(any_bot_on(1, 0), "bomb_planted", 0, 0.2, -1.0, 0, true)
}

public ev_defused()
{
	new id = get_loguser_index()
	// defused with terrorists still alive: a ninja defuse
	new ts
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == 1)
			ts++
	want(id, ts ? "defuse_ninja" : "defused", 0, ts ? 0.7 : 0.4)
	want(any_bot_on(1, 0), "they_defused", id, 0.3)
}

public ev_intermission()
{
	if (g_Career || !get_pcvar_num(g_pLevel))
		return
	// the end of the map: a few gg's, not every bot (some gg's even at personality 0)
	new said, most = 2 + floatround(personality()), Float:pct = 25.0 + 10.0 * personality()
	new order[32], n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_connected(i) && is_user_bot(i))
			order[n++] = i
	for (new k = 0; k < n && said < most; k++)
	{
		// in random order, so it isn't always the lowest slots
		new j = random_num(k, n - 1), i = order[j]
		order[j] = order[k]
		if (random_float(0.0, 100.0) > pct)
			continue
		new text[128]
		if (pick_line(i, "map_end", text, charsmax(text), true))
		{
			queue(i, 0, text, MAX_CHAIN, false, random_float(0.8, 3.0) + said * random_float(0.8, 2.0) + typing_time(text, 0.0, 0.0))
			said++
		}
	}
}

get_loguser_index()
{
	new user[80], name[32]
	read_logargv(0, user, charsmax(user))
	parse_loguser(user, name, charsmax(name))
	return get_user_index(name)
}

// ---- what people say ------------------------------------------------------------------

public cmd_say(id)
{
	heard(id, false)
	return PLUGIN_CONTINUE
}

public cmd_say_team(id)
{
	heard(id, true)
	return PLUGIN_CONTINUE
}

// Words people type, and the section the answer comes from (first match wins, so
// "gg ez" is an ez, "nice try" isn't a "nice", "my bad" isn't a "bad" and "how are you"
// isn't a "how")
new const HEARD[][][] =
{
	{ "how are you", "r_howru" }, { "how r u", "r_howru" }, { "how are u", "r_howru" }, { "how r you", "r_howru" },
	{ "hows it going", "r_howru" }, { "how is it going", "r_howru" }, { "how u doing", "r_howru" },
	{ "how you doing", "r_howru" }, { "hru", "r_howru" }, { "whats up with you", "r_howru" },
	{ "where", "r_where" }, { "wheres", "r_where" }, { "where r they", "r_where" }, { "where is he", "r_where" },
	{ "score", "r_score" }, { "whats the score", "r_score" },
	{ "timeleft", "r_timeleft" }, { "time left", "r_timeleft" }, { "how long left", "r_timeleft" },
	{ "how much time", "r_timeleft" }, { "how long is this map", "r_timeleft" },
	{ "nextmap", "r_nextmap" }, { "next map", "r_nextmap" }, { "whats next", "r_nextmap" }, { "what map is next", "r_nextmap" },
	{ "what map", "r_whatmap" }, { "which map", "r_whatmap" }, { "map name", "r_whatmap" }, { "what is this map", "r_whatmap" },
	{ "hack", "r_hack" }, { "hacks", "r_hack" }, { "hacker", "r_hack" }, { "hacking", "r_hack" }, { "haxor", "r_hack" },
	{ "aimbot", "r_hack" }, { "wallhack", "r_hack" }, { "wh", "r_hack" }, { "cheat", "r_hack" }, { "cheater", "r_hack" },
	{ "cheating", "r_hack" }, { "sus", "r_hack" },
	{ "bot", "r_bot" }, { "bots", "r_bot" }, { "botz", "r_bot" }, { "a bot", "r_bot" },
	{ "nt", "r_nt" }, { "nice try", "r_nt" },
	{ "not bad", "r_nice_from_human" }, { "not too bad", "r_nice_from_human" },
	{ "good job", "r_nice_from_human" }, { "well done", "r_nice_from_human" }, { "great shot", "r_nice_from_human" },
	{ "nice round", "r_nice_from_human" }, { "good round", "r_nice_from_human" },
	{ "danke", "r_np" }, { "merci", "r_np" }, { "gracias", "r_np" }, { "obrigado", "r_np" }, { "obrigada", "r_np" },
	{ "spasibo", "r_np" }, { "dzieki", "r_np" }, { "dziekuje", "r_np" }, { "grazie", "r_np" }, { "kiitos", "r_np" },
	{ "bonjour", "r_hi" }, { "salut", "r_hi" }, { "hallo", "r_hi" }, { "moin", "r_hi" }, { "privet", "r_hi" }, { "ola", "r_hi" },
	{ "ciao", "r_hi" }, { "hej", "r_hi" }, { "siema", "r_hi" }, { "hoi", "r_hi" },
	{ "ns", "r_nice_from_human" }, { "nice shot", "r_nice_from_human" }, { "nice", "r_nice_from_human" },
	{ "good shot", "r_nice_from_human" }, { "wp", "r_nice_from_human" }, { "gj", "r_nice_from_human" },
	{ "well played", "r_nice_from_human" }, { "nice one", "r_nice_from_human" },
	{ "ez", "r_ez" }, { "ezpz", "r_ez" }, { "easy", "r_ez" }, { "too easy", "r_ez" },
	{ "sorry", "r_sorry" }, { "sry", "r_sorry" }, { "my bad", "r_sorry" }, { "mb", "r_sorry" }, { "oops", "r_sorry" },
	{ "noob", "r_noob" }, { "nub", "r_noob" }, { "n00b", "r_noob" }, { "newb", "r_noob" }, { "noobs", "r_noob" },
	{ "trash", "r_trash" }, { "garbage", "r_trash" }, { "rubbish", "r_trash" }, { "suck", "r_noob" }, { "sucks", "r_noob" }, { "bad", "r_noob" }, { "idiot", "r_noob" },
	{ "loser", "r_noob" }, { "l2p", "r_noob" }, { "owned", "r_noob" }, { "pwned", "r_noob" }, { "stfu", "r_noob" },
	{ "shut up", "r_noob" }, { "dumb", "r_noob" }, { "lame", "r_noob" },
	{ "kys", "r_noob" }, { "kill yourself", "r_noob" }, { "go die", "r_noob" }, { "just die", "r_noob" },
	{ "camp", "r_camping" }, { "camper", "r_camping" }, { "camping", "r_camping" }, { "campers", "r_camping" },
	{ "gg", "r_gg" }, { "ggs", "r_gg" }, { "ggwp", "r_gg" }, { "good game", "r_gg" },
	{ "gl", "r_gl" }, { "hf", "r_gl" }, { "glhf", "r_gl" }, { "good luck", "r_gl" },
	{ "hi", "r_hi" }, { "hey", "r_hi" }, { "yo", "r_hi" }, { "sup", "r_hi" }, { "hello", "r_hi" }, { "hiya", "r_hi" },
	{ "heya", "r_hi" }, { "hola", "r_hi" }, { "howdy", "r_hi" }, { "evening", "r_hi" }, { "whats up", "r_hi" },
	{ "wassup", "r_hi" }, { "hai", "r_hi" }, { "ello", "r_hi" },
	{ "lag", "r_lag" }, { "lags", "r_lag" }, { "laggy", "r_lag" }, { "lagging", "r_lag" }, { "ping", "r_lag" }, { "hitreg", "r_lag" },
	{ "reg", "r_lag" }, { "choke", "r_lag" }, { "loss", "r_lag" },
	{ "wait", "r_wait" }, { "hold", "r_wait" }, { "stay", "r_wait" }, { "dont push", "r_wait" }, { "slow", "r_wait" },
	{ "rush", "r_plan" }, { "rush b", "r_plan" }, { "go a", "r_plan" }, { "go b", "r_plan" }, { "come a", "r_plan" },
	{ "come b", "r_plan" }, { "follow me", "r_plan" }, { "follow", "r_plan" }, { "cover me", "r_plan" }, { "cover", "r_plan" },
	{ "push", "r_plan" }, { "stack", "r_plan" }, { "help", "r_plan" }, { "eco", "r_plan" }, { "save", "r_plan" },
	{ "lets go", "r_plan" }, { "go go", "r_plan" }, { "with me", "r_plan" },
	{ "ty", "r_np" }, { "thx", "r_np" }, { "thanks", "r_np" }, { "thank", "r_np" }, { "tyvm", "r_np" },
	{ "im back", "r_wb" },
	{ "how", "r_how" },
	{ "wtf", "r_wtf" }, { "omg", "r_wtf" },
	{ "lol", "r_lol" }, { "haha", "r_lol" }, { "lmao", "r_lol" }, { "rofl", "r_lol" }, { "xd", "r_lol" }, { "hehe", "r_lol" },
	{ "?", "r_question" }
}

// How bots take what people say, per answer section: the chance one answers, who it
// should be and whether a second one may chime in
enum
{
	WHO_ANY,        // anyone, nearby and alive first
	WHO_INVOLVED,   // the bot that killed them or that they killed, just now
	WHO_KILLER,     // the bot that got the kill they're talking about
	WHO_TEAM,       // a teammate, alive if possible
	WHO_WATCHER     // a teammate, dead (watching) if possible
}

reply_style(const section[], &Float:chance, &who, &Float:second)
{
	who = WHO_ANY
	second = 0.0
	if (equal(section, "r_where"))          { chance = 0.5;  who = WHO_TEAM; }
	else if (equal(section, "r_bomb"))      { chance = 0.5;  who = WHO_TEAM; }
	else if (equal(section, "r_score"))     { chance = 0.45; who = WHO_TEAM; }
	else if (equal(section, "r_timeleft"))  { chance = 0.45; }
	else if (equal(section, "r_nextmap"))   { chance = 0.45; }
	else if (equal(section, "r_whatmap"))   { chance = 0.35; }
	else if (equal(section, "r_how"))       { chance = 0.4;  who = WHO_WATCHER; }
	else if (equal(section, "r_howru"))     { chance = 0.5; }
	else if (equal(section, "r_hack"))      { chance = 0.4;  who = WHO_INVOLVED; }
	else if (equal(section, "r_bot"))       { chance = 0.45; second = 0.15; }
	else if (equal(section, "r_nt"))        { chance = 0.2;  who = WHO_TEAM; }
	else if (equal(section, "r_nice_from_human")) { chance = 0.55; who = WHO_KILLER; }
	else if (equal(section, "r_ez"))        { chance = 0.3;  who = WHO_INVOLVED; }
	else if (equal(section, "r_noob"))      { chance = 0.35; who = WHO_INVOLVED; }
	else if (equal(section, "r_trash"))     { chance = 0.35; who = WHO_INVOLVED; }
	else if (equal(section, "r_camping"))   { chance = 0.3;  who = WHO_INVOLVED; }
	else if (equal(section, "r_gg"))        { chance = 0.5;  second = 0.3; }
	else if (equal(section, "r_gl"))        { chance = 0.35; second = 0.15; }
	else if (equal(section, "r_hi"))        { chance = 0.45; second = 0.2; }
	else if (equal(section, "r_lag"))       { chance = 0.3; }
	else if (equal(section, "r_wait"))      { chance = 0.35; who = WHO_TEAM; }
	else if (equal(section, "r_plan"))      { chance = 0.45; who = WHO_TEAM; }
	else if (equal(section, "r_sorry"))     { chance = 0.35; who = WHO_INVOLVED; }
	else if (equal(section, "r_np"))        { chance = 0.25; }
	else if (equal(section, "r_sorry_tk"))  { chance = 0.7;  who = WHO_INVOLVED; }
	else if (equal(section, "r_map_good") || equal(section, "r_map_bad")) { chance = 0.35; second = 0.1; }
	else if (equal(section, "r_team_bad") || equal(section, "r_team_good")) { chance = 0.4; who = WHO_TEAM; }
	else if (equal(section, "r_here"))      { chance = 0.6;  second = 0.3; }
	else if (equal(section, "r_whokilled")) { chance = 0.65; who = WHO_WATCHER; }
	else if (equal(section, "r_where_go"))  { chance = 0.6;  who = WHO_TEAM; }
	else if (equal(section, "r_lost_round")) { chance = 0.45; who = WHO_TEAM; }
	else if (equal(section, "r_round_gj"))  { chance = 0.5;  who = WHO_TEAM; second = 0.2; }
	else if (equal(section, "r_wb"))        { chance = 0.35; }
	else if (equal(section, "r_wtf"))       { chance = 0.12; }
	else if (equal(section, "r_lol"))       { chance = 0.06; }
	else if (equal(section, "r_question"))  { chance = 0.08; }
	else if (equal(section, "r_named"))     { chance = 0.85; }
	else                                    { chance = 0.015; }
}

// The section for what was said, or "" (msg in lowercase)
classify(const msg[], section[], len)
{
	section[0] = 0
	if (equal(msg, "o/") || equal(msg, "\o"))
	{
		copy(section, len, "r_hi")
		return
	}
	if (equal(msg, "re") || equal(msg, "back") || equal(msg, "back."))
	{
		copy(section, len, "r_wb")
		return
	}
	// "die" on its own: at whoever just killed them (or they killed)
	if (equal(msg, "die") || equal(msg, "die!"))
	{
		copy(section, len, "r_noob")
		return
	}
	// "who killed me", "what got me"
	static const WHOKILL[][] = { "who killed me", "who got me", "what killed me", "what got me", "who was that", "who shot me" }
	if (any_in(msg, WHOKILL, sizeof WHOKILL))
	{
		copy(section, len, "r_whokilled")
		return
	}
	// "where do i go", "what do i do"
	static const WHEREGO[][] = { "where do i go", "where should i go", "where to go", "what do i do", "what should i do",
		"where do we go", "where should we go", "what do we do", "whats the plan", "what is the plan" }
	if (any_in(msg, WHEREGO, sizeof WHEREGO))
	{
		copy(section, len, "r_where_go")
		return
	}
	// "why did we lose", "how did we lose that"
	if ((word_in(msg, "why") || word_in(msg, "how")) && (word_in(msg, "lose") || word_in(msg, "lost")) && (word_in(msg, "we") || word_in(msg, "did")))
	{
		copy(section, len, "r_lost_round")
		return
	}
	// "time?": the map's time left
	if (equal(msg, "time") || equal(msg, "time?") || equal(msg, "time??"))
	{
		copy(section, len, "r_timeleft")
		return
	}
	// "nice one bot", "gg bots", "not bad bot": praise, not a bot accusation
	if (word_in(msg, "bot") || word_in(msg, "bots"))
	{
		static const PRAISE[][] = { "nice", "ns", "gj", "wp", "good", "well played", "not bad", "great", "ty", "thanks" }
		for (new i = 0; i < sizeof PRAISE; i++)
			if (word_in(msg, PRAISE[i]))
			{
				copy(section, len, "r_nice_from_human")
				return
			}
		if (word_in(msg, "gg") || word_in(msg, "ggs"))
		{
			copy(section, len, "r_gg")
			return
		}
	}
	// "wheres the bomb", "bomb?", "who has c4"
	if ((word_in(msg, "bomb") || word_in(msg, "c4")) && (contain(msg, "?") != -1 || word_in(msg, "where") || word_in(msg, "wheres") || word_in(msg, "who")))
	{
		copy(section, len, "r_bomb")
		return
	}
	// "anyone here?", "anyone alive"
	static const HERE[][] = { "anyone here", "anybody here", "is anyone here", "anyone alive", "anyone on", "hello anyone", "anyone there" }
	if (any_in(msg, HERE, sizeof HERE) || phrase_is(msg, "anyone") || phrase_is(msg, "anybody"))
	{
		copy(section, len, "r_here")
		return
	}
	// what the usual words say comes first ("this server lags" is lag), the insults after a topic
	// (a map or team word with an opinion: the opinion wins, "this map is slow" isn't a "wait")
	new pol = polarity(msg)
	static const MAPW[][] = { "map", "this map", "server", "this server", "maps" }
	static const TEAMW[][] = { "our team", "this team", "my team", "we", "we are", "were", "team" }
	new bool:topic = pol != 0 && (any_in(msg, MAPW, sizeof MAPW) || any_in(msg, TEAMW, sizeof TEAMW))
	new first_hit[24]
	heard_word(msg, first_hit, charsmax(first_hit))
	// lag is lag, whatever else is said about the server
	if (equal(first_hit, "r_lag") || word_in(msg, "lags") || word_in(msg, "lagging") || word_in(msg, "laggy"))
	{
		copy(section, len, "r_lag")
		return
	}
	if (!topic && first_hit[0] && !insult_section(first_hit) && !equal(first_hit, "r_nice_from_human") && !equal(first_hit, "r_lol")
		&& !equal(first_hit, "r_question"))
	{
		copy(section, len, first_hit)
		return
	}
	// a topic before an insult: the map or the server ("this map sucks", "love this map"), the
	// team ("we suck", "good team", "we are not bad"), each said well or badly
	if (pol && any_in(msg, MAPW, sizeof MAPW) && !word_in(msg, "next") && !word_in(msg, "what"))
	{
		copy(section, len, pol > 0 ? "r_map_good" : "r_map_bad")
		return
	}
	static const TEAM_BAD[][] = { "we suck", "team sucks", "we are bad", "were bad", "we are trash", "were trash", "we lost again",
		"worst team", "team is bad", "team is trash" }
	static const TEAM_GOOD[][] = { "good team", "nice team", "great team", "best team", "we are good", "were good", "we are doing good",
		"we are winning", "love this team" }
	if (!word_in(msg, "join") && !word_in(msg, "switch"))
	{
		if (any_in(msg, TEAM_BAD, sizeof TEAM_BAD) || (pol < 0 && any_in(msg, TEAMW, sizeof TEAMW)))
		{
			copy(section, len, "r_team_bad")
			return
		}
		if (any_in(msg, TEAM_GOOD, sizeof TEAM_GOOD) || (pol > 0 && any_in(msg, TEAMW, sizeof TEAMW)))
		{
			copy(section, len, "r_team_good")
			return
		}
	}
	// "sick", "insane", "clutch", "beast": praise when it's about something just now ("that was
	// sick", "so clutch", a short line, a bot's kill a moment ago)
	static const WOW[][] = { "sick", "insane", "clutch", "beast" }
	static const SOWOW[][] = { "so sick", "so insane", "so clutch", "that was sick", "that was insane", "that was clutch" }
	static const ME[][] = { "i", "im", "of", "feel" }
	if (any_in(msg, WOW, sizeof WOW) && (count_words(msg) <= 3 || any_in(msg, SOWOW, sizeof SOWOW) || word_in(msg, "nice")
		|| (get_gametime() - g_LastBotKillAt < 10.0 && !any_in(msg, ME, sizeof ME))))
	{
		copy(section, len, "r_nice_from_human")
		return
	}
	heard_word(msg, section, len)
	if (section[0])
		return
	// a question without the "?" ("is this a pug", "anyone up for a scrim"): the model may know;
	// not "will do", "who cares", "why not"
	static const ASK[][] = { "is", "are", "do", "does", "should", "why", "who", "anyone", "can", "could", "would", "will" }
	static const ACK[][] = { "will do", "can do", "do it", "do that", "who cares", "why not", "is fine", "can you not", "could be",
		"would be", "should be", "does it matter", "are you sure" }
	new first[16]
	copyc(first, charsmax(first), msg, ' ')
	if (any_in(msg, ACK, sizeof ACK) || (count_words(msg) < 3 && !word_in(msg, "you")))
		return
	for (new i = 0; i < sizeof ASK; i++)
		if (equal(first, ASK[i]) && msg[strlen(first)])
		{
			copy(section, len, "r_question")
			return
		}
}

// The first HEARD word in msg, and its section ("" none); a foreign hello that's part of a
// bot's name here ("ola" with Olaf in) is the name
heard_word(const msg[], section[], len)
{
	static const HELLOS[][] = { "bonjour", "salut", "hallo", "moin", "privet", "ola", "ciao", "hej", "siema", "hoi" }
	section[0] = 0
	// the word that comes first in the line ("gg wp merci" is a gg); at the same place the
	// longer one ("nice try", not "nice"); "ez" anywhere is gloating ("gg ez")
	new best = -1, bestat = 9999, bestlen
	for (new w = 0; w < sizeof HEARD; w++)
	{
		new at = word_at(msg, HEARD[w][0])
		if (at < 0)
			continue
		new bool:hello
		for (new h = 0; h < sizeof HELLOS; h++)
			if (equal(HEARD[w][0], HELLOS[h]))
				hello = true
		if (hello && in_bot_name(HEARD[w][0]))
			continue
		if (equal(HEARD[w][1], "r_ez"))
			at = -1
		new l = strlen(HEARD[w][0])
		if (at < bestat || (at == bestat && l > bestlen))
		{
			best = w
			bestat = at
			bestlen = l
		}
	}
	if (best >= 0)
		copy(section, len, HEARD[best][1])
}

// Where `word` first stands as a whole word in msg (-1 not at all; "?" anywhere)
word_at(const msg[], const word[])
{
	if (word[0] == '?')
	{
		new q = contain(msg, "?")
		return q < 0 ? -1 : q
	}
	new len = strlen(word), from = 0, at
	while ((at = contain(msg[from], word)) != -1)
	{
		new s = from + at
		new before = s > 0 ? msg[s - 1] : ' ', after = msg[s + len]
		if (!isalnum(before) && !isalnum(after))
			return s
		from = s + 1
	}
	return -1
}

// `word` is part of a bot's name here
bool:in_bot_name(const word[])
{
	new parts[4][24]
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i))
			continue
		new n = name_parts(i, parts, 4)
		for (new p = 0; p < n; p++)
			if (equal(parts[p], word))
				return true
	}
	return false
}

bool:insult_section(const section[])
{
	return equal(section, "r_noob") || equal(section, "r_trash") || equal(section, "r_ez") || equal(section, "r_camping")
}

// What a line thinks of something: 1 good, -1 bad, 0 neither ("not fun" is bad, "not bad" good)
polarity(const msg[])
{
	static const BAD[][] = { "suck", "sucks", "sux", "suks", "sucky", "bad", "trash", "awful", "boring", "hate", "worst", "lame",
		"ugly", "garbage", "rubbish", "terrible", "horrible", "crap", "slow" }
	static const GOOD[][] = { "love", "best", "great", "good", "fun", "nice", "favourite", "favorite", "cool", "awesome", "amazing",
		"like", "enjoy" }
	static const NEG[][] = { "not", "isnt", "never", "dont", "doesnt", "didnt", "aint", "wasnt", "arent" }
	if (word_in(msg, "cant stand"))
		return -1
	// the words in order; the first opinion word decides, turned round by a "not" one or two
	// words before it ("not that bad" is good, "not fun" bad)
	new words[16][16], n
	for (new i = 0; msg[i] && n < 16; )
	{
		while (msg[i] && !isalnum(msg[i]))
			i++
		new k
		while (msg[i] && isalnum(msg[i]))
		{
			if (k < 15)
				words[n][k++] = msg[i]
			i++
		}
		words[n][k] = 0
		if (k)
			n++
	}
	for (new w = 0; w < n; w++)
	{
		new p
		for (new b = 0; b < sizeof BAD && !p; b++)
			if (equal(words[w], BAD[b]))
				p = -1
		for (new g = 0; g < sizeof GOOD && !p; g++)
			if (equal(words[w], GOOD[g]))
				p = 1
		if (!p)
			continue
		for (new back = 1; back <= 2 && w - back >= 0; back++)
			for (new x = 0; x < sizeof NEG; x++)
				if (equal(words[w - back], NEG[x]))
					return -p
		return p
	}
	return 0
}

// Lines that go nowhere on their own: in a thread they carry on what was being said
bool:weak_section(const section[])
{
	return !section[0] || equal(section, "r_lol") || equal(section, "r_ack") || equal(section, "r_question")
		|| equal(section, "r_named") || equal(section, "r_wtf")
}

// What people type, made easier to match: lowercase, "heyyy" -> "hey", "ggggg" -> "gg"
normalize(msg[])
{
	strtolower(msg)
	new out[128], k
	for (new i = 0; msg[i] && k < charsmax(out); i++)
	{
		new c = msg[i]
		new run = 1
		while (msg[i + run] == c)
			run++
		// a run of three or more: two of the letters English doubles ("goood" -> "good",
		// "ggg" -> "gg"), one of the rest ("heyyy" -> "hey")
		if (run >= 3)
		{
			out[k++] = c
			if (contain("oelstfpmg", fmt("%c", c)) != -1 && k < charsmax(out))
				out[k++] = c
			i += run - 1
			continue
		}
		out[k++] = c
	}
	out[k] = 0
	copy(msg, 127, out)
	slang(msg)
}

// Text-speak and common typos, as the words people mean ("u" -> "you", "plz" -> "please",
// "wehre" -> "where"); apostrophes go ("where's" -> "wheres")
slang(msg[])
{
	static const FROM[][] = { "u", "yu", "ur", "urself", "r", "plz", "pls", "plox", "wat", "wut", "wot", "wats", "wuts",
		"k", "kk", "okk", "okay", "thx", "thnx", "thanx", "bc", "cuz", "coz", "cos", "gimme", "lemme", "wanna", "gonna",
		"teh", "waht", "wehre", "whre", "wher", "hwere", "hw", "mony", "moeny", "helth", "folow", "follw", "flw", "cvr",
		"wiat", "w8", "hlod", "jmp", "spary", "sprya", "ppl", "smth", "every1", "some1", "any1", "no1", "whered",
		"n", "hes", "theyre", "wep", "weap", "wpn", "yea", "yeh", "yep" }
	static const TO[][] = { "you", "you", "your", "yourself", "are", "please", "please", "please", "what", "what", "what", "whats", "whats",
		"ok", "ok", "ok", "ok", "thanks", "thanks", "thanks", "because", "because", "because", "because", "give me", "let me", "want to", "going to",
		"the", "what", "where", "where", "where", "where", "how", "money", "money", "health", "follow", "follow", "follow", "cover",
		"wait", "wait", "hold", "jump", "spray", "spray", "people", "something", "everyone", "someone", "anyone", "no one", "where did",
		"and", "he is", "they are", "weapon", "weapon", "weapon", "yes", "yes", "yes" }
	new out[128], k, word[24]
	for (new i = 0; msg[i] && k < charsmax(out); )
	{
		if (msg[i] == 39)
		{
			i++
			continue
		}
		if (!isalnum(msg[i]))
		{
			out[k++] = msg[i++]
			continue
		}
		new w = 0
		while (msg[i] && (isalnum(msg[i]) || msg[i] == 39))
		{
			if (msg[i] != 39 && w < charsmax(word))
				word[w++] = msg[i]
			i++
		}
		word[w] = 0
		for (new f = 0; f < sizeof FROM; f++)
			if (equal(word, FROM[f]))
			{
				copy(word, charsmax(word), TO[f])
				break
			}
		k += copy(out[k], charsmax(out) - k, word)
	}
	out[k] = 0
	copy(msg, 127, out)
}

// The words in a player's name people would call them by, lowercase: "[XyZ] xX_eLiTe_Xx" ->
// "xx", "elite", "xx"; "dylan52" -> "dylan"; the game's "(1)" in front goes
name_parts(id, parts[][24], max)
{
	new name[32]
	base_name(id, name, charsmax(name))
	strtolower(name)
	new i, n
	if (name[0] == '(')
		while (name[i] && name[i] != ')')
			i++
	for (new p = 0; p < max; p++)
		parts[p][0] = 0
	while (name[i] && n < max)
	{
		while (name[i] && !isalpha(name[i]))
			i++
		new k
		while (name[i] && isalpha(name[i]) && k < 23)
			parts[n][k++] = name[i++]
		parts[n][k] = 0
		while (name[i] && isalpha(name[i]))
			i++
		if (k)
			n++
	}
	return n
}

// `part` at the start of a word in msg ("tari" in "nice tariq")
bool:word_start_in(const msg[], const part[])
{
	new from = 0, at
	while ((at = contain(msg[from], part)) != -1)
	{
		new s = from + at
		if (s == 0 || !isalnum(msg[s - 1]))
			return true
		from = s + 1
	}
	return false
}

// Words people type all the time: a name part like these ("Good", "There", "Rush") only
// counts as the whole name, and a word like these isn't a misspelt name
bool:common_word(const w[])
{
	static const WORDS[][] = { "the", "you", "are", "and", "for", "not", "but", "get", "got", "how", "who", "why", "can",
		"all", "any", "one", "two", "out", "off", "way", "see", "saw", "now", "new", "old", "man", "its", "his", "her", "him",
		"she", "lol", "omg", "wtf", "yes", "yea", "nah", "gun", "awp", "bot", "mid", "good", "game", "there", "their", "where",
		"what", "when", "with", "this", "that", "then", "them", "they", "here", "have", "just", "like", "nice", "shot", "come",
		"cover", "follow", "money", "drop", "wait", "hold", "rush", "push", "jump", "spray", "crouch", "stop", "please",
		"thanks", "sorry", "great", "about", "again", "back", "dead", "kill", "killed", "camp", "camper", "noob", "lucky",
		"hack", "hacker", "admin", "server", "round", "team", "everyone", "anyone", "someone", "really", "right", "think",
		"would", "could", "should", "going", "gonna", "doing", "being", "after", "before", "still", "maybe", "never", "always",
		"other", "enemy", "enemies", "bomb", "site", "long", "short", "left", "alive", "health", "weapon", "where", "while",
		"knife", "pistol", "rifle", "sniper", "player", "people", "friend", "dude", "bro", "mate", "guys", "fast", "slow",
		// four letters, the ones a short name is one letter off from
		"have", "save", "gave", "wave", "made", "make", "take", "fake", "game", "came", "name", "same", "fine", "nine", "mine",
		"line", "time", "life", "love", "live", "give", "lose", "lost", "best", "rest", "last", "help", "well", "will",
		"wall", "ball", "call", "tell", "were", "more", "some", "sure", "much", "such", "pack", "kill", "fill", "late",
		"rate", "gate", "plan", "play", "okay", "face", "race", "case", "base", "boom", "room", "door", "show", "know",
		"want", "went", "need", "feel", "look", "took", "said", "says", "done", "gone", "dead", "deag", "nade", "nades",
		"lmao", "haha", "hehe", "yeah", "nope", "dont", "cant", "wont", "didnt", "isnt", "lets", "gogo", "rush", "camp",
		"lag", "ping", "hits", "shot", "spot", "stay", "hold", "wait", "move", "back", "next", "even", "only", "real",
		"also", "into", "from", "your", "yours", "both", "each", "away", "down", "mode", "maps", "nice", "cool", "true",
		"player", "mile", "date", "mild", "male", "mall", "mail", "tile", "pile", "mice", "rice", "dice", "nick", "rock",
		"lane", "jade", "bill", "mark", "pat", "rob", "sue", "will" }
	for (new i = 0; i < sizeof WORDS; i++)
		if (equal(w, WORDS[i]))
			return true
	return false
}

// "@name" anywhere, or the name as the line's first word
bool:at_name(const msg[], const part[])
{
	new l = strlen(part)
	if (equal(msg, part, l) && !isalnum(msg[l]))
		return true
	new at[26]
	formatex(at, charsmax(at), "@%s", part)
	return word_in(msg, at) || contain(msg, at) == 0
}

// a and b differ by one letter at most (one changed, added or left out, or two next to each
// other swapped): "tarik" and "tariq", "tairq" and "tariq"
bool:one_off(const a[], const b[])
{
	new la = strlen(a), lb = strlen(b)
	if (abs(la - lb) > 1)
		return false
	if (la == lb)
	{
		new first = -1, diffs
		for (new i = 0; i < la; i++)
			if (a[i] != b[i])
			{
				if (first < 0)
					first = i
				diffs++
			}
		if (diffs == 2 && first + 1 < la && a[first] == b[first + 1] && a[first + 1] == b[first])
			return true
	}
	new i, j, diff
	while (i < la && j < lb)
	{
		if (a[i] == b[j])
		{
			i++
			j++
			continue
		}
		if (++diff > 1)
			return false
		if (la > lb)
			i++
		else if (lb > la)
			j++
		else
		{
			i++
			j++
		}
	}
	return diff + (la - i) + (lb - j) <= 1
}

// A person in the game goes by this name part too: then it's them being talked about, not a bot
bool:human_named(const part[])
{
	new parts[4][24]
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || is_user_bot(i))
			continue
		new n = name_parts(i, parts, 4)
		for (new p = 0; p < n; p++)
			if (equal(parts[p], part))
				return true
	}
	return false
}

// How sure it is that msg is talking to this bot: 4 its first name, 3 another word of its
// name, 2 the first five letters or more of a long one, 1 one letter off; 0 not at all
name_score(const msg[], id)
{
	new parts[4][24], sec[24], best
	new n = name_parts(id, parts, 4)
	for (new p = 0; p < n; p++)
	{
		new l = strlen(parts[p])
		if (l < 3)
			continue
		// a bot called "Lag" or "Ghost": "lag" in chat is about lag, not about them
		classify(parts[p], sec, charsmax(sec))
		if (sec[0] || human_named(parts[p]))
			continue
		if (word_in(msg, parts[p]))
		{
			// (a name that's an everyday word, "Player" or "Good": only "@player", or the first
			// word of the line, and only the first word of the name)
			if (common_word(parts[p]) && (p > 0 || !at_name(msg, parts[p])))
				continue
			best = max(best, p == 0 ? 4 : 3)
			continue
		}
		if (l < 6)
		{
			if (l >= 4)
				best = max(best, typo_score(msg, parts[p]))
			continue
		}
		// people shorten names: "dylan" for "dylanator", "tariq" for "tariqzz" (five letters,
		// and not an everyday word: "Goodman" isn't "good game")
		new hit[6], word[24]
		copy(hit, 5, parts[p])
		if (word_start_in(msg, hit))
		{
			// the word it starts must be part of the name too ("thermal" in "thermals", not "there")
			new at = contain(msg, hit), k
			while (msg[at] && isalnum(msg[at]) && k < 23)
				word[k++] = msg[at++]
			word[k] = 0
			if (!common_word(word) && equal(parts[p], word, strlen(word)))
				best = max(best, 2)
		}
		best = max(best, typo_score(msg, parts[p]))
		best = max(best, short_score(msg, parts[p]))
	}
	return best
}

// 2 when a word in msg is a short form of the name part: its start, three letters or more
// ("jon" for "jonny", "alex" for "alexander"), or the usual nickname ("john" for "jonny",
// "mike" for "michael"). addressed() drops it when two bots fit as well
short_score(const msg[], const part[])
{
	static const NICK[][2][] = { {"john", "jon"}, {"johnny", "jon"}, {"jonny", "john"}, {"jon", "john"}, {"mike", "mich"},
		{"mikey", "mich"}, {"dave", "david"}, {"davey", "david"}, {"bob", "rob"}, {"bobby", "rob"}, {"bill", "will"},
		{"billy", "will"}, {"tom", "thom"}, {"tommy", "tom"}, {"jim", "jam"}, {"jimmy", "jam"}, {"chris", "kris"},
		{"kris", "chris"}, {"nick", "nik"}, {"dan", "dann"}, {"danny", "dan"}, {"matt", "math"}, {"sam", "samu"},
		{"alex", "alek"}, {"steve", "stev"}, {"steve", "steph"}, {"joe", "jos"}, {"joey", "jos"}, {"tony", "anto"},
		{"ben", "benj"}, {"benny", "ben"}, {"kenny", "ken"}, {"rick", "rich"}, {"ricky", "rich"}, {"rich", "rick"} }
	new pl = strlen(part)
	if (pl < 4 || common_word(part))
		return 0
	// (only the first two words, where people put a name, and never a game word: "rush b" isn't
	// for a bot called Rusher)
	static const GAME[][] = { "rush", "push", "flash", "smoke", "nade", "mid", "long", "short", "drop", "buy", "eco", "force",
		"save", "plant", "bomb", "follow", "wait", "hold", "cover", "fake", "split", "awp", "come", "watch", "fall", "back",
		"stack", "rotate", "trade", "peek", "defuse", "money", "where", "what", "who", "how", "nice", "good", "lol", "sorry" }
	new word[24], seen
	for (new i = 0; msg[i] && seen < 2; )
	{
		while (msg[i] && !isalnum(msg[i]))
			i++
		new k
		while (msg[i] && isalnum(msg[i]))
		{
			if (k < 23)
				word[k++] = msg[i]
			i++
		}
		word[k] = 0
		if (!k)
			break
		seen++
		if (k < 3 || common_word(word))
			continue
		new bool:game
		for (new g = 0; g < sizeof GAME; g++)
			if (equal(word, GAME[g]))
				game = true
		if (game)
			continue
		if (k < pl && equal(part, word, k))
			return 2
		for (new n = 0; n < sizeof NICK; n++)
			if (equal(word, NICK[n][0]) && equal(part, NICK[n][1], strlen(NICK[n][1])))
				return 2
	}
	return 0
}

// 1 when a word in msg is the name part with one letter off ("tarik" for "tariq")
typo_score(const msg[], const part[])
{
	// (four letters: only a name that isn't a word itself, and only a word as long)
	new pl = strlen(part)
	if (pl < 4 || common_word(part))
		return 0
	new word[24]
	for (new i = 0; msg[i]; )
	{
		while (msg[i] && !isalnum(msg[i]))
			i++
		new k
		while (msg[i] && isalnum(msg[i]))
		{
			if (k < 23)
				word[k++] = msg[i]
			i++
		}
		word[k] = 0
		if (k >= 4 && (pl >= 5 || k == 4) && !common_word(word) && one_off(word, part))
			return 1
	}
	return 0
}

// The bot someone is talking to by name, or 0 (the surest match; two equally sure: neither)
addressed(const msg[])
{
	new best, score, tie
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i) || !is_user_bot(i))
			continue
		new sc = name_score(msg, i)
		if (sc > score)
		{
			score = sc
			best = i
			tie = 0
		}
		else if (sc && sc == score)
			tie = i
	}
	return (tie && score < 4) ? 0 : best
}

// "you" in what someone said, to a bot that was just talking (to them, or in chat a moment
// ago): that bot
you_bot(id, const msg[], bool:team = false)
{
	if (!word_in(msg, "you") && !word_in(msg, "your") && !word_in(msg, "yourself"))
		return 0
	new Float:now = get_gametime()
	new b = g_ThreadBot[id]
	if (b && b != id && is_user_connected(b) && now - g_ThreadAt[id] < 20.0)
		return b
	b = g_AskBot[id]
	if (b && b != id && is_user_connected(b) && now - g_AskAt[id] < 20.0)
		return b
	// (whoever spoke last: a teammate, or someone on the other side only in all chat)
	b = g_LastSpoke
	if (b && b != id && is_user_connected(b) && is_user_bot(b) && now - g_LastSpokeAt < 8.0 && (get_user_team(b) == get_user_team(id) || !team))
		return b
	return 0
}

// Something the bot said, quoted back ("lol no" -> "lol no?", or a longer word from it)
bool:quoted(const msg[], const line[])
{
	new low[128]
	copy(low, charsmax(low), line)
	strtolower(low)
	if (strlen(low) >= 4 && contain(msg, low) != -1)
		return true
	new word[32], rest[128]
	copy(rest, charsmax(rest), low)
	while (rest[0])
	{
		strtok2(rest, word, charsmax(word), rest, charsmax(rest), ' ', TRIM_FULL)
		if (strlen(word) >= 5 && word_in(msg, word))
			return true
	}
	return false
}

// The bot that should answer: `prefer` if it can, else one of the rest, likelier when
// it's alive, nearby, on their team, and not in the middle of a fight
responder(human, prefer, who, bool:team)
{
	new t = get_user_team(human)
	new bool:teamonly = team || who == WHO_TEAM || who == WHO_WATCHER
	if (prefer && is_user_connected(prefer) && is_user_bot(prefer) && (!teamonly || get_user_team(prefer) == t) && can_answer(prefer, human))
		return prefer
	new list[32], Float:weight[32], n, Float:total
	new org[3], o2[3]
	new bool:alive = bool:is_user_alive(human)
	if (alive)
		get_user_origin(human, org)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == human || !is_user_connected(i) || !is_user_bot(i) || !can_answer(i, human))
			continue
		new bool:mate = get_user_team(i) == t
		if (teamonly && !mate)
			continue
		new Float:w = 1.0
		if (who == WHO_WATCHER)
			w += is_user_alive(i) ? 0.0 : 3.0
		else if (is_user_alive(i))
		{
			w += who == WHO_TEAM ? 3.0 : 1.5
			if (alive)
			{
				get_user_origin(i, o2)
				if (get_distance(org, o2) < 1000)
					w += 2.0
			}
		}
		if (mate)
			w += 1.0
		if (in_combat(i))
			w *= 0.15
		list[n] = i
		weight[n++] = w
		total += w
	}
	if (!n)
		return 0
	new Float:r = random_float(0.0, total)
	for (new k = 0; k < n; k++)
	{
		r -= weight[k]
		if (r <= 0.0)
			return list[k]
	}
	return list[n - 1]
}

// The game state an answer needs, put where pick_line finds it; the section may change
// to the one for what's known ("r_where" -> "r_where_unknown" when nobody saw anything)
resolve_state(section[], len, human, bot)
{
	new Float:now = get_gametime()
	new t = get_user_team(human)
	if (equal(section, "r_where"))
	{
		// what it saw itself, else how the person died, else what the team last saw
		new Float:at
		if (now - g_BotSawAt[bot] < 30.0 && g_BotSawPlace[bot][0])
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_BotSawPlace[bot])
			at = g_BotSawAt[bot]
		}
		else if (now - g_DiedAt[human] < 30.0 && g_KillerPlace[human][0])
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_KillerPlace[human])
			at = g_DiedAt[human]
		}
		else if ((t == 1 || t == 2) && now - g_SeenAt[t] < 25.0 && g_SeenPlace[t][0])
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_SeenPlace[t])
			at = g_SeenAt[t]
		}
		if (!g_CtxPlace[0])
			copy(section, len, "r_where_unknown")
		else
			ago_text(now - at, g_CtxAgo, charsmax(g_CtxAgo))
	}
	else if (equal(section, "r_whokilled"))
	{
		// what a teammate saw: who it was, with what, from where
		new k = g_KillerOf[human]
		if (k && now - g_KillerOfAt[human] < 40.0 && is_user_connected(k))
		{
			short_name(k, g_CtxCarrier, charsmax(g_CtxCarrier))
			copy(g_CtxWeapon, charsmax(g_CtxWeapon), g_KillWeapon[human])
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_KillerPlace[human])
			g_CtxHs = g_KillHs[human]
		}
		else
			copy(section, len, "r_whokilled_unknown")
	}
	else if (equal(section, "r_money"))
	{
		new m = cs_get_user_money(bot)
		if (!g_BuyMode)
			copy(section, len, "r_money_none")
		else
		{
			money_text(m, g_CtxMoney, charsmax(g_CtxMoney))
			copy(section, len, m >= 10000 ? "r_money_rich" : m < 2000 ? "r_money_broke" : "r_money")
		}
	}
	else if (equal(section, "r_gun"))
	{
		new kind = gun_text(bot, g_CtxGun, charsmax(g_CtxGun))
		copy(section, len, !is_user_alive(bot) ? "r_dead" : kind == 2 ? "r_gun" : kind == 1 ? "r_gun_pistol" : "r_gun_knife")
	}
	else if (equal(section, "r_hp"))
	{
		new hp = is_user_alive(bot) ? get_user_health(bot) : 0
		formatex(g_CtxHp, charsmax(g_CtxHp), "%d", hp)
		copy(section, len, !hp ? "r_dead" : hp >= 100 ? "r_hp_full" : hp < 35 ? "r_hp_low" : "r_hp")
	}
	else if (equal(section, "r_left"))
	{
		new n
		for (new i = 1; i <= MaxClients; i++)
			if (is_user_alive(i) && (t == 1 || t == 2) && get_user_team(i) == 3 - t)
				n++
		formatex(g_CtxLeft, charsmax(g_CtxLeft), "%d", n)
		copy(section, len, n == 0 ? "r_left_none" : n == 1 ? "r_left_one" : "r_left")
	}
	else if (equal(section, "r_whereme"))
	{
		if (!is_user_alive(bot))
			copy(section, len, "r_dead")
		else
		{
			place_of(bot, g_CtxPlace, charsmax(g_CtxPlace))
			new Float:a[3], Float:b[3]
			pev(bot, pev_origin, a)
			pev(human, pev_origin, b)
			if (is_user_alive(human) && get_distance_f(a, b) < 350.0)
				copy(section, len, "r_whereme_near")
			else if (!g_CtxPlace[0])
				copy(section, len, "r_whereme_unknown")
		}
	}
	else if (equal(section, "r_bomb"))
	{
		new carrier
		if (!g_BombPlanted && t == 1)
			for (new i = 1; i <= MaxClients && !carrier; i++)
				if (is_user_alive(i) && get_user_team(i) == 1 && user_has_weapon(i, CSW_C4))
					carrier = i
		if (g_BombPlanted && g_BombPlace[0])
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_BombPlace)
			copy(section, len, "r_bomb_planted")
		}
		else if (carrier == human)
			copy(section, len, "r_bomb_you")
		else if (carrier)
		{
			if (carrier == bot)
				copy(section, len, "r_bomb_me")
			else
			{
				short_name(carrier, g_CtxCarrier, charsmax(g_CtxCarrier))
				copy(section, len, "r_bomb_carrier")
			}
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_Place[carrier])
		}
		else if (t == 1 && g_BombDropped)
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_BombPlace)
			copy(section, len, "r_bomb_dropped")
		}
		else
			copy(section, len, "r_bomb_unknown")
	}
	else if (equal(section, "r_score"))
	{
		new bt = get_user_team(bot)
		new us = (bt == 1 || bt == 2) ? g_Score[bt] : 0, them = (bt == 1 || bt == 2) ? g_Score[3 - bt] : 0
		copy(section, len, us > them ? "r_score_ahead" : us < them ? "r_score_behind" : "r_score_tied")
	}
	else if (equal(section, "r_how"))
	{
		// "how" about their own death, just now; otherwise it's just a question
		if (now - g_DiedAt[human] < 15.0)
		{
			copy(g_CtxPlace, charsmax(g_CtxPlace), g_KillerPlace[human])
			copy(g_CtxWeapon, charsmax(g_CtxWeapon), g_KillWeapon[human])
			g_CtxHs = g_KillHs[human]
		}
		else
			copy(section, len, "r_question")
	}
	else if (equal(section, "r_timeleft") && get_timeleft() <= 0)
		copy(section, len, "r_timeleft_none")
	else if (equal(section, "r_nextmap"))
	{
		new next[32]
		next_map(next, charsmax(next))
		if (!next[0])
			copy(section, len, "r_nextmap_none")
	}
}

clear_state()
{
	g_CtxPlace[0] = g_CtxWeapon[0] = g_CtxCarrier[0] = 0
	g_CtxMoney[0] = g_CtxGun[0] = g_CtxHp[0] = g_CtxLeft[0] = g_CtxAgo[0] = g_CtxSite[0] = 0
	g_CtxHs = false
}

// One bot's answer to a person, typed after reading it, with the game state it needs
bool:answer(bot, const what[], human, bool:team, Float:extra, bool:quiet = false, bool:thread = false)
{
	// still typing an answer from the model
	if (!bot || g_AiState[bot])
		return false
	new section[32], text[128]
	copy(section, charsmax(section), what)
	resolve_state(section, charsmax(section), human, bot)
	g_CtxOther = human
	new bool:ok = pick_line(bot, section, text, charsmax(text), true)
	g_CtxOther = 0
	if (g_Tracing && ok)
	{
		trace_add(g_TrSection, charsmax(g_TrSection), section)
		if (g_TrReply[0])
			add(g_TrReply, charsmax(g_TrReply), " | ")
		// (as it'll read: the names in; the rest of the typing is done when it's said)
		new shown[128], nm[32]
		copy(shown, charsmax(shown), text)
		if (human && is_user_connected(human))
		{
			short_name(human, nm, charsmax(nm))
			replace_string(shown, charsmax(shown), "{other}", nm)
		}
		short_name(bot, nm, charsmax(nm))
		replace_string(shown, charsmax(shown), "{name}", nm)
		add(g_TrReply, charsmax(g_TrReply), shown)
	}
	// with gm_ai_url set the model writes it (the written line is the fallback); a second
	// bot chiming in ("gg") keeps to the written lines
	if (ok && !quiet && !factual(section) && ai_ask(bot, human, text, section, team, extra, thread))
	{
		clear_state()
		return true
	}
	clear_state()
	if (!ok)
		return false
	queue(bot, human, text, 1, team, extra + typing_time(text, 1.2, 3.0), section, quiet)
	return true
}

// A bot said something to a person: that's a thread they may carry on
thread_said(human, bot, const section[], const text[])
{
	if (g_ThreadBot[human] != bot)
	{
		g_ThreadBot[human] = bot
		g_ThreadTurns[human] = 1
		copy(g_ThreadTopic[human], charsmax(g_ThreadTopic[]), section)
	}
	else
	{
		g_ThreadTurns[human]++
		if (section[0] && !equal(section, "t_", 2))
			copy(g_ThreadTopic[human], charsmax(g_ThreadTopic[]), section)
	}
	g_ThreadAt[human] = get_gametime()
	copy(g_ThreadLine[human], charsmax(g_ThreadLine[]), text)
}

// The carry-on section for a thread about `topic`: t_bot for r_bot, t_score for
// r_score_ahead, t_human for human_kill_hs..., t_any when there's none
thread_section(const topic[], out[], len)
{
	new name[32]
	copy(name, charsmax(name), topic[equal(topic, "r_", 2) ? 2 : 0])
	new Array:list
	while (name[0])
	{
		formatex(out, len, "t_%s", name)
		if (TrieGetCell(g_Sections, out, list))
			return
		new cut = -1
		for (new i = 0; name[i]; i++)
			if (name[i] == '_')
				cut = i
		if (cut < 0)
			break
		name[cut] = 0
	}
	copy(out, len, "t_any")
}

// How many lines a bot keeps a thread going: 3, 4 for warm ones, 5 for chatty ones
thread_cap(bot)
{
	if (trait(bot, "talk", "chatty") || trait(bot, "talk", "spammer") || has_persona(bot, "Yhk"))
		return 5
	if (trait(bot, "warmth", "warm") || has_persona(bot, "fw"))
		return 4
	return 3
}

// The person answers the bot that was just talking to them: it carries on, a couple of
// lines at most, and only it (true: this was a thread, nobody else answers)
bool:continue_thread(id, const msg[], const said[], to, bool:team)
{
	new bot = g_ThreadBot[id]
	if (!bot)
		return false
	new Float:now = get_gametime()
	if (!is_user_connected(bot) || !is_user_bot(bot) || g_ThreadTurns[id] >= thread_cap(bot) || in_combat(bot) || !can_answer(bot, id)
		|| (to && to != bot) || (team && get_user_team(bot) != get_user_team(id))
		|| (now - g_ThreadAt[id] >= 20.0 && to != bot && !quoted(msg, g_ThreadLine[id])))
	{
		g_ThreadBot[id] = 0
		return false
	}
	// something new ("where is he", "gg") gets its own answer, from this bot; anything
	// else ("sure", "lol", the same thing again) carries on what it was saying
	new section[32], topic[24]
	copy(topic, charsmax(topic), g_ThreadTopic[id])
	new bool:same = said[0] && equal(topic, said, strlen(said))
	if (weak_section(said) || same)
		thread_section(topic, section, charsmax(section))
	else
	{
		// "where is he" is for teammates, not the enemy who was talking
		new Float:c, who, Float:sec
		reply_style(said, c, who, sec)
		if ((who == WHO_TEAM || who == WHO_WATCHER) && get_user_team(bot) != get_user_team(id))
		{
			g_ThreadBot[id] = 0
			return false
		}
		copy(section, charsmax(section), said)
	}
	new Float:chance = g_ThreadTurns[id] <= 1 ? 0.8 : 0.55
	if (equal(section, "t_any"))
		chance *= 0.6
	chance *= 0.6 + 0.4 * personality()
	if (random_float(0.0, 1.0) > chance || !answer(bot, section, id, team, 0.0, false, true))
		g_ThreadBot[id] = 0
	return true
}

// ---- questions with real answers, and asking a bot to do something ------------------------
// "money?" gets what it has, "where'd he go" where it saw someone and how long ago, "cover
// me" has it tag along. A request goes to the bot named, the one that just spoke ("you"), or
// the nearest teammate; it does it or not as it is (warm and team-minded mostly yes, cold or
// tilted ones now and then no, one in a fight says it's busy) and says so. Loner-Strike's
// game DLL does the moving (ls_bot), which a bot that isn't calm turns down: then "1 sec",
// and it tries again in a moment.

enum
{
	I_NONE,
	I_MONEY, I_GUN, I_WHOHAS, I_HP, I_LEFT, I_SEEN, I_WHEREYOU, I_BOMB, I_SURE,
	// "what do they have", "are they eco": what the team has seen them carry this round
	I_THEIRS,
	// what someone saw ("two long", "awp mid"): told to the team, not asked of anyone
	I_INFO,
	// (requests from here on)
	I_DROP, I_RELEASE, I_FOLLOW, I_HOLD, I_GOTO, I_COME, I_SPRAY, I_JUMP, I_CROUCH, I_KYS, I_PLANT, I_GIVEBOMB,
	// "eco", "force", "full buy" (arg: 1 save, 2 force, 0 buy); "flash for me", "smoke mid"
	// (arg: the spot, and the grenade times 16); "fake b" (arg: the site faked); "go defuse"
	I_BUYCALL, I_UTIL, I_FAKE, I_DEFUSE,
	// "someone buy me an awp" (arg: the gun): one teammate at most, now and then
	I_BUYFOR
}

// where a request is about: a bomb site, mid, where the person aims, the hostages, a place
// someone has stood in, the person
enum
{
	SPOT_NONE, SPOT_A, SPOT_B, SPOT_C, SPOT_MID, SPOT_THERE, SPOT_HOST, SPOT_PLACE, SPOT_HUMAN,
	// "fall back": its own side's spawn; "watch flank": a little behind the person, towards
	// their spawn; "go defuse": the planted bomb
	SPOT_BACK, SPOT_FLANK, SPOT_BOMB
}
// (a goto said with "slow", "walk", "quiet": walking, not running; the arg's bit)
#define SPOT_WALK 0x100
#define TASK_UTIL 8740
#define TASK_FAKE 8780
new bool:g_ReqWalk[33]                      // walking there ("go b slow")
new g_ReqFakeTo[33]                         // a fake: the site it goes to after (SPOT_A/B), 0 none
new bool:g_ReqBomb                          // the request being decided is a CT's "go defuse" (the bomb down is why)
new g_BuyForRound[33]                       // the round someone last had a gun bought for them (one a round)
new g_SawGuns[3], g_SawGunsRound[3]         // what a team has seen the other carry this round (1 awp, 2 a rifle or smg, 4 a pistol only)

enum
{
	REQ_YES,        // does it
	REQ_NO,         // won't
	REQ_BUSY,       // in a fight
	REQ_LATER,      // not calm yet: "1 sec", and again in a moment
	REQ_NOPE        // can't (dead, the mode, the bomb)
}

new Float:g_ReqTo[33][3]                    // where a bot was asked to go
new g_StartMoney[33]                        // money at the start of the round (before buying)

// ---- game state, as people say it ----

// "just now", "15 sec ago", "a min ago"
ago_text(Float:secs, out[], len)
{
	if (secs < 5.0)
		copy(out, len, "just now")
	else if (secs < 60.0)
		formatex(out, len, "%d sec ago", max(5, floatround(secs / 5.0) * 5))
	else if (secs < 120.0)
		copy(out, len, "a min ago")
	else
		formatex(out, len, "%d min ago", floatround(secs / 60.0, floatround_floor))
}

// "800", "$4350", "4k", "16k"
money_text(m, out[], len)
{
	if (m >= 10000)
		formatex(out, len, "%dk", m / 1000)
	else if (m >= 2000 && random_num(1, 100) <= 35)
		formatex(out, len, "%dk", m / 1000)
	else if (random_num(1, 100) <= 25)
		formatex(out, len, "$%d", m)
	else
		formatex(out, len, "%d", m)
}

#define PRIMARY_BITS ((1<<CSW_SCOUT)|(1<<CSW_XM1014)|(1<<CSW_MAC10)|(1<<CSW_AUG)|(1<<CSW_UMP45)|(1<<CSW_SG550)|(1<<CSW_GALIL)|(1<<CSW_FAMAS)|(1<<CSW_AWP)|(1<<CSW_MP5NAVY)|(1<<CSW_M249)|(1<<CSW_M3)|(1<<CSW_M4A1)|(1<<CSW_TMP)|(1<<CSW_G3SG1)|(1<<CSW_SG552)|(1<<CSW_AK47)|(1<<CSW_P90))
#define PISTOL_BITS ((1<<CSW_P228)|(1<<CSW_ELITE)|(1<<CSW_FIVESEVEN)|(1<<CSW_USP)|(1<<CSW_GLOCK18)|(1<<CSW_DEAGLE))

// What it's carrying, as people say it ("ak", "m4 and deag", "usp"): 2 a primary, 1 a pistol
// only, 0 the knife
gun_text(id, out[], len)
{
	out[0] = 0
	if (!is_user_alive(id))
		return 0
	new weapons[32], num, primary, pistol
	get_user_weapons(id, weapons, num)
	for (new i = 0; i < num; i++)
	{
		if ((1 << weapons[i]) & PRIMARY_BITS)
			primary = weapons[i]
		else if ((1 << weapons[i]) & PISTOL_BITS)
			pistol = weapons[i]
	}
	new a[16], b[16]
	if (primary)
		weapon_casual(primary, a, charsmax(a))
	if (pistol)
		weapon_casual(pistol, b, charsmax(b))
	if (a[0] && b[0] && (pistol == CSW_DEAGLE || pistol == CSW_ELITE) && random_num(1, 100) <= 60)
		formatex(out, len, "%s and %s", a, b)
	else if (a[0])
		copy(out, len, a)
	else if (b[0])
		copy(out, len, b)
	return a[0] ? 2 : b[0] ? 1 : 0
}

weapon_casual(csw, out[], len)
{
	new name[24]
	get_weaponname(csw, name, charsmax(name))
	casual_weapon(name[7], out, len)
	if (!out[0])
		copy(out, len, name[7])
}

// Where someone is: the radar's place name, or on maps whose nav doesn't name its places,
// near a bomb site, a spawn or the hostages ("" in between)
place_of(id, out[], len)
{
	if (g_Place[id][0])
	{
		copy(out, len, g_Place[id])
		return
	}
	new Float:o[3]
	pev(id, pev_origin, o)
	rough_place(o, out, len)
}

rough_place(const Float:o[3], out[], len)
{
	out[0] = 0
	for (new i = 0; i < g_SiteCount; i++)
		if (get_distance_f(o, g_Sites[i]) < 800.0)
		{
			formatex(out, len, "%c", g_SiteLetter[i])
			return
		}
	if (g_HaveT && get_distance_f(o, g_SpawnT) < 700.0)
		copy(out, len, "t spawn")
	else if (g_HaveCT && get_distance_f(o, g_SpawnCT) < 700.0)
		copy(out, len, "ct spawn")
}

// The bomb sites' middles (the map's first is A, as the game's bots call them), the spawns
find_spots()
{
	g_SiteCount = 0
	new ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "func_bomb_target")) > 0 && g_SiteCount < 3)
	{
		new Float:mins[3], Float:maxs[3]
		pev(ent, pev_absmin, mins)
		pev(ent, pev_absmax, maxs)
		for (new i = 0; i < 3; i++)
			g_Sites[g_SiteCount][i] = (mins[i] + maxs[i]) * 0.5
		g_SiteCount++
	}
	ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "info_bomb_target")) > 0 && g_SiteCount < 3)
		pev(ent, pev_origin, g_Sites[g_SiteCount++])
	name_sites()
	g_HaveT = spawn_middle("info_player_deathmatch", g_SpawnT)
	g_HaveCT = spawn_middle("info_player_start", g_SpawnCT)
}

bool:spawn_middle(const cls[], Float:out[3])
{
	new ent = -1, n, Float:o[3]
	out[0] = out[1] = out[2] = 0.0
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", cls)) > 0)
	{
		pev(ent, pev_origin, o)
		for (new i = 0; i < 3; i++)
			out[i] += o[i]
		n++
	}
	if (!n)
		return false
	for (new i = 0; i < 3; i++)
		out[i] /= float(n)
	return true
}

// The round, for this bot: "pistol", "eco", "force" or "full" ("" where nobody buys)
round_kind(id, out[], len)
{
	out[0] = 0
	if (!g_BuyMode || teams_off() || g_GunGame)
		return
	if (g_Score[1] + g_Score[2] == 0)
		copy(out, len, "pistol")
	else
	{
		new m = g_StartMoney[id]
		copy(out, len, m < 2500 ? "eco" : m < 4000 ? "force" : "full")
	}
}

// Answers that are facts: the written line says them (a model might get the number wrong)
bool:factual(const section[])
{
	static const FACTS[][] = { "r_money", "r_gun", "r_hp", "r_left", "r_whereme", "r_whohas", "r_req_", "r_kys", "r_dead", "r_sure", "r_where_unknown", "drop_no", "drop_broke", "r_whokilled",
		"r_plant", "r_givebomb", "r_bomb_come", "r_bomb_almost", "r_bomb_notme", "r_bomb_none" }
	for (new i = 0; i < sizeof FACTS; i++)
		if (equal(section, FACTS[i], strlen(FACTS[i])))
			return true
	return false
}

// ---- what happened, for the model ----

note_event(killer, victim, const weapon[], bool:hs)
{
	new k[32], v[32], w[16]
	if (victim < 1 || victim > MaxClients)
		return
	get_user_name(victim, v, charsmax(v))
	if (killer < 1 || killer > MaxClients || killer == victim)
	{
		formatex(g_EvText[g_EvPos], charsmax(g_EvText[]), "%s died", v)
		killer = 0
	}
	else
	{
		get_user_name(killer, k, charsmax(k))
		casual_weapon(weapon, w, charsmax(w))
		formatex(g_EvText[g_EvPos], charsmax(g_EvText[]), "%s killed %s%s%s%s", k, v, w[0] ? " with " : "", w, hs ? ", headshot" : "")
	}
	g_EvKiller[g_EvPos] = killer
	g_EvVictim[g_EvPos] = victim
	g_EvAt[g_EvPos] = get_gametime()
	g_EvPos = (g_EvPos + 1) % EVENTS
}

// ---- noticing how a person plays ----

#define SAID_AWP3   1
#define SAID_AWP6   2
#define SAID_CAMP   4      // (this round)
#define SAID_RUSH   8      // (this round)
#define SAID_BHOP   16
#define SAID_TOP    32
#define SAID_LAG    64

// A person's kill on a bot: their third and sixth awp kill, a kill from where they've stood
// for a while, two kills in the first seconds of the round
notice_play(killer, victim, const weapon[])
{
	if (g_SteeredMode || g_Career)
		return
	new Float:now = get_gametime()
	// (where they are, for "@awp is long")
	place_of(killer, g_CtxPlace, charsmax(g_CtxPlace))
	notice_kill(killer, victim, weapon, now)
	g_CtxPlace[0] = 0
}

notice_kill(killer, victim, const weapon[], Float:now)
{
	if (equal(weapon, "awp"))
	{
		new n = ++g_AwpKills[killer]
		new bit = n == 3 ? SAID_AWP3 : n == 6 ? SAID_AWP6 : 0
		if (bit && !(g_SaidAbout[killer] & bit))
		{
			g_SaidAbout[killer] |= bit
			want(victim, "human_awp", killer, 0.5)
			return
		}
	}
	if (!g_Freeze && now - g_StillSince[killer] > 15.0 && now - g_RoundLiveAt > 25.0 && !(g_SaidAbout[killer] & SAID_CAMP))
	{
		g_SaidAbout[killer] |= SAID_CAMP
		want(victim, "human_camping", killer, 0.4)
		return
	}
	if (!dm_like() && now - g_RoundLiveAt < 22.0 && g_RoundKills[killer] >= 2 && !(g_SaidAbout[killer] & SAID_RUSH))
	{
		g_SaidAbout[killer] |= SAID_RUSH
		want(victim, "human_rush", killer, 0.35)
	}
}

// The round's over: a person well ahead of everyone on the scoreboard gets noticed (once)
notice_topfrag()
{
	if (g_SteeredMode || g_Career)
		return
	new top, best, second
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_connected(i))
			continue
		new f = get_user_frags(i)
		if (f > best)
		{
			second = best
			best = f
			top = i
		}
		else if (f > second)
			second = f
	}
	if (top && !is_user_bot(top) && best >= 12 && best >= second + 6 && !(g_SaidAbout[top] & SAID_TOP))
	{
		g_SaidAbout[top] |= SAID_TOP
		new b = any_bot(0)
		if (b)
			want(b, "human_topfrag", top, 0.5, random_float(2.0, 5.0))
	}
}

// Once a second: where each bot last saw an enemy, who has stood still how long, who
// bunnyhops, whose ping is bad; on maps whose nav names no places, rough ones
public task_watch()
{
	new Float:now = get_gametime()
	for (new i = 1; i <= MaxClients; i++)
	{
		if (!is_user_alive(i))
			continue
		new Float:o[3]
		pev(i, pev_origin, o)
		if (!g_NavPlaces)
			rough_place(o, g_Place[i], charsmax(g_Place[]))
		if (is_user_bot(i))
		{
			whiff_check(i, now)
			new e = bot_target(i)
			if (e >= 1 && e <= MaxClients && is_user_alive(e))
			{
				new place[32]
				place_of(e, place, charsmax(place))
				if (place[0])
				{
					copy(g_BotSawPlace[i], charsmax(g_BotSawPlace[]), place)
					g_BotSawAt[i] = now
					new t = get_user_team(i)
					if (t == 1 || t == 2)
					{
						copy(g_SeenPlace[t], charsmax(g_SeenPlace[]), place)
						g_SeenAt[t] = now
						saw_guns(t, e)
					}
					spot_seen(i, e, place, now)
				}
			}
			continue
		}
		if (get_distance_f(o, g_StillAt[i]) > 120.0 || g_Freeze)
		{
			g_StillAt[i] = o
			g_StillSince[i] = now
		}
		// fast in the air again and again: bunnyhopping
		new Float:v[3]
		pev(i, pev_velocity, v)
		if (!(pev(i, pev_flags) & FL_ONGROUND) && floatsqroot(v[0] * v[0] + v[1] * v[1]) > 290.0)
		{
			if (++g_Bhops[i] == 12 && !(g_SaidAbout[i] & SAID_BHOP) && !g_SteeredMode)
			{
				g_SaidAbout[i] |= SAID_BHOP
				want(any_bot(0), "human_bhop", i, 0.5, random_float(1.0, 3.0))
			}
		}
		new ping, loss
		get_user_ping(i, ping, loss)
		if (ping > 170 && now - g_JoinedAt[i] > 60.0 && !(g_SaidAbout[i] & SAID_LAG) && random_num(1, 100) <= 3)
		{
			g_SaidAbout[i] |= SAID_LAG
			want(any_bot(0), "human_lag", i, 0.4, random_float(1.0, 3.0))
		}
	}
}

// A bot sees an enemy: the first time this round it's two or more together, or one at a bomb
// site, it may tell the team ("2 long", "rush b", "1 short"). Team-minded ones mostly; more,
// and shorter, in a match; never in deathmatch
spot_seen(bot, enemy, const place[], Float:now)
{
	if (g_SpotSeenRound[bot] == g_Round || dm_like() || g_SteeredMode || g_Freeze)
		return
	new et = get_user_team(enemy), n
	new Float:at[3], Float:o[3]
	pev(enemy, pev_origin, at)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == et)
		{
			pev(i, pev_origin, o)
			if (get_distance_f(at, o) < 500.0)
				n++
		}
	new bool:site = !place[1] && (place[0] == 'a' || place[0] == 'b' || place[0] == 'c')
	if (n < 2 && !site)
		return
	g_SpotSeenRound[bot] = g_Round
	g_SpotSawAt[bot] = now
	g_SpotN[bot] = n
	copy(g_SpotPlace[bot], charsmax(g_SpotPlace[]), place)
	if (random_float(0.0, 1.0) < spot_rate(bot, false))
		spot_call(bot, random_float(0.3, 1.0))
}

Float:spot_rate(bot, bool:dying)
{
	new p = PERSONAS[g_Persona[bot]]
	new Float:r = contain("itrEbN", fmt("%c", p)) != -1 ? 0.35 : contain("qLdu", fmt("%c", p)) != -1 ? 0.03 : 0.1
	if (dying)
		r = floatmin(r * 2.0 + 0.15, 0.8)
	return match_like() ? floatmin(r * 1.8, 0.9) : r
}

spot_call(bot, Float:delay)
{
	g_SpotRound[bot] = g_Round
	copy(g_CtxPlace, charsmax(g_CtxPlace), g_SpotPlace[bot])
	formatex(g_CtxLeft, charsmax(g_CtxLeft), "%d", g_SpotN[bot])
	new n = g_SpotN[bot]
	want(bot, n >= 3 ? "callout_rush" : n == 2 ? "callout_spot" : "callout_spot_one", 0, 1.0, delay, 0, true)
	g_CtxPlace[0] = g_CtxLeft[0] = 0
}

// Killed within 3 s of seeing a group it hadn't called: its last words are that (true: said)
bool:spot_last_words(bot)
{
	if (g_SpotSeenRound[bot] != g_Round || g_SpotRound[bot] == g_Round || get_gametime() - g_SpotSawAt[bot] > 3.0 || dm_like())
		return false
	if (random_float(0.0, 1.0) >= spot_rate(bot, true))
		return false
	spot_call(bot, random_float(0.4, 1.2))
	return true
}

// A bot's shot: ten in a row (no more than 3 s apart) with no hit is a whiffed spray

public fw_shot(ent)
{
	new id = pev(ent, pev_owner)
	if (id < 1 || id > MaxClients || !is_user_bot(id))
		return HAM_IGNORED
	new Float:now = get_gametime()
	if (now - g_ShotAt[id] > 3.0)
		g_Shots[id] = 0
	g_ShotAt[id] = now
	if (++g_Shots[id] == 10)
		g_WhiffAt[id] = now
	return HAM_IGNORED
}

// Out of the fight after a whiffed spray: now and then it says so ("how did i miss that")
whiff_check(id, Float:now)
{
	if (g_WhiffAt[id] <= 0.0 || now - g_WhiffAt[id] > 15.0 || in_combat(id) || bot_target(id))
		return
	g_WhiffAt[id] = 0.0
	new p = PERSONAS[g_Persona[id]]
	// the hot-headed and the self-critical say it most, the quiet ones hardly
	new Float:base = contain("stHuOk", fmt("%c", p)) != -1 ? 0.25 : contain("qLd", fmt("%c", p)) != -1 ? 0.02 : 0.08
	want(id, "whiff", 0, base, random_float(0.5, 2.0))
}

// Every half minute: someone dead in the round chats about this and that now and then
public task_dead_talk()
{
	if (g_Freeze || g_RoundOver || match_like() || g_SteeredMode || teams_off() || !get_pcvar_num(g_pLevel))
		return
	new b = any_dead_bot_on(random_num(1, 2))
	if (b)
		want(b, "smalltalk", 0, 0.12, random_float(1.0, 4.0))
}

// A flashbang thrown (it gets its model): who threw it, for "nice flash" when it blinds a teammate
public fw_setmodel(ent, const model[])
{
	if (!pev_valid(ent) || contain(model, "w_flashbang") == -1)
		return FMRES_IGNORED
	new cls[16]
	pev(ent, pev_classname, cls, charsmax(cls))
	new owner = pev(ent, pev_owner)
	if (!equal(cls, "grenade") || owner < 1 || owner > MaxClients || !is_user_connected(owner))
		return FMRES_IGNORED
	new t = get_user_team(owner)
	if (t == 1 || t == 2)
	{
		g_FlashBy[t] = owner
		g_FlashAt[t] = get_gametime()
	}
	return FMRES_IGNORED
}

// A bot gone white: a teammate's flash, just now
public ev_fade(id)
{
	if (!is_user_alive(id) || !is_user_bot(id) || teams_off())
		return
	new t = get_user_team(id)
	if (t != 1 && t != 2)
		return
	new by = g_FlashBy[t], Float:ago = get_gametime() - g_FlashAt[t]
	if (by && by != id && ago > 0.5 && ago < 4.0 && is_user_connected(by) && get_user_team(by) == t)
		want(id, "teamflash", by, 0.45, random_float(0.8, 2.0), 0, true)
}

// ---- reading what was said ----

// Without the ?, ! or . at the end, exactly this ("where?" is "where")
bool:phrase_is(const c[], const p[])
{
	new t[64]
	copy(t, charsmax(t), c)
	new l = strlen(t)
	while (l > 0 && (t[l - 1] == '?' || t[l - 1] == '!' || t[l - 1] == '.' || t[l - 1] == ' '))
		t[--l] = 0
	return bool:equal(t, p)
}

bool:any_in(const c[], const list[][], n)
{
	for (new i = 0; i < n; i++)
		if (word_in(c, list[i]))
			return true
	return false
}

count_words(const c[])
{
	new n, bool:in
	for (new i = 0; c[i]; i++)
	{
		if (isalnum(c[i]) || c[i] == '$')
		{
			if (!in)
				n++
			in = true
		}
		else
			in = false
	}
	return n
}

// Has the gun, grenade or kit someone asked about
bool:has_item(id, item)
{
	if (item == ITEM_KIT)
		return get_user_team(id) == 2 && cs_get_user_defuse(id) != 0
	return item > 0 && user_has_weapon(id, item) != 0
}

// The first or second word is `w` ("jump", "tariq jump", "pls jump"): said as a request,
// not in passing ("i cant jump")
bool:lead_word(const c[], const w[])
{
	new word[24]
	for (new i = 0, n = 0; c[i] && n < 2; n++)
	{
		while (c[i] && !isalnum(c[i]))
			i++
		new k
		while (c[i] && isalnum(c[i]))
		{
			if (k < 23)
				word[k++] = c[i]
			i++
		}
		word[k] = 0
		if (k && equal(word, w))
			return true
	}
	return false
}

// The gun someone asks for by name ("awp", "ak", "m4", "deag"), as a CSW id
gun_asked(const c[])
{
	new g = gun_named(c)
	if (g)
		return g
	if (word_in(c, "flash") || word_in(c, "flashbang") || word_in(c, "flashes"))
		return CSW_FLASHBANG
	if (word_in(c, "smoke") || word_in(c, "smokes"))
		return CSW_SMOKEGRENADE
	if (word_in(c, "kit") || word_in(c, "defuser") || word_in(c, "defuse kit") || word_in(c, "kits"))
		return ITEM_KIT
	// "he" is mostly a word: a grenade only as "a he", "he nade", or in a short line ("he?")
	static const HE[][] = { "a he", "an he", "the he", "1 he", "2 he", "he nade", "he grenade", "he nades" }
	if (word_in(c, "nade") || word_in(c, "nades") || word_in(c, "grenade") || word_in(c, "hegrenade") || any_in(c, HE, sizeof HE)
		|| (word_in(c, "he") && count_words(c) <= 3))
		return CSW_HEGRENADE
	return 0
}

gun_named(const c[])
{
	if (word_in(c, "awp"))
		return CSW_AWP
	if (word_in(c, "ak") || word_in(c, "ak47") || word_in(c, "ak 47"))
		return CSW_AK47
	if (word_in(c, "m4") || word_in(c, "m4a1") || word_in(c, "colt"))
		return CSW_M4A1
	if (word_in(c, "deag") || word_in(c, "deagle") || word_in(c, "deserteagle"))
		return CSW_DEAGLE
	if (word_in(c, "scout"))
		return CSW_SCOUT
	if (word_in(c, "famas"))
		return CSW_FAMAS
	if (word_in(c, "galil"))
		return CSW_GALIL
	if (word_in(c, "mp5"))
		return CSW_MP5NAVY
	if (word_in(c, "shotgun") || word_in(c, "pump") || word_in(c, "m3"))
		return CSW_M3
	if (word_in(c, "para") || word_in(c, "m249"))
		return CSW_M249
	return 0
}

// A bomb site named in a request ("go b", "b rush", "site a"): "a" only where it can't be the
// word ("go a bit")
bool:site_named(const c[], const letter[])
{
	if (!word_in(c, letter))
		return false
	if (!equal(letter, "a"))
		return true
	new l = strlen(c), k = l
	while (k > 0 && !isalnum(c[k - 1]))
		k--
	if (k >= 1 && c[k - 1] == 'a' && (k == 1 || !isalnum(c[k - 2])))
		return true
	static const AFTER[][] = { "a site", "site a", "bombsite a", "a rush", "a go", "a now", "a please", "a pls", "a long", "a short", "a ramp" }
	return any_in(c, AFTER, sizeof AFTER)
}

// Where "go ..." is about, if anywhere
bool:goto_target(const c[], &arg, place[], plen)
{
	static const VERBS[][] = { "go", "rush", "push", "come", "hit", "take", "head", "move", "run", "all", "everyone",
		"everybody", "to", "cover", "check", "lets", "stack", "retake", "defend", "split", "execute", "exec", "walk", "sneak" }
	// ("b b b", "bbb" said alone is a call too)
	new bool:alone = phrase_is(c, "b b") || phrase_is(c, "b b b") || phrase_is(c, "a a a")
	if (!alone && !any_in(c, VERBS, sizeof VERBS))
		return false
	if (site_named(c, "b"))
		arg = SPOT_B
	else if (site_named(c, "a"))
		arg = SPOT_A
	else if (word_in(c, "c") && (word_in(c, "site") || count_words(c) <= 3))
		arg = SPOT_C
	else if (word_in(c, "mid") || word_in(c, "middle"))
		arg = SPOT_MID
	else if (word_in(c, "there") || word_in(c, "that way"))
		arg = SPOT_THERE
	else if (word_in(c, "hostages") || word_in(c, "hostage") || word_in(c, "hosties") || word_in(c, "hostys"))
		arg = SPOT_HOST
	if (arg)
		return true
	// a place on the radar someone has stood in ("go long", "push ct spawn")
	new word[24], prev[24], two[48]
	for (new i = 0; c[i]; )
	{
		while (c[i] && !isalnum(c[i]))
			i++
		new k
		while (c[i] && isalnum(c[i]))
		{
			if (k < 23)
				word[k++] = c[i]
			i++
		}
		word[k] = 0
		if (!k)
			break
		formatex(two, charsmax(two), "%s %s", prev, word)
		if (prev[0] && TrieKeyExists(g_PlaceAt, two))
		{
			copy(place, plen, two)
			arg = SPOT_PLACE
			return true
		}
		if (k >= 3 && TrieKeyExists(g_PlaceAt, word))
		{
			copy(place, plen, word)
			arg = SPOT_PLACE
			return true
		}
		// (a word only one place has: "long" for "long a", "tunnels" for "upper tunnels")
		new full[32]
		if (k >= 3 && TrieGetString(g_PlaceWord, word, full, charsmax(full)) && full[0])
		{
			copy(place, plen, full)
			arg = SPOT_PLACE
			return true
		}
		copy(prev, charsmax(prev), word)
	}
	return false
}

// ---- callouts from people ("two long", "awp mid", "they are b") ----

new Float:g_InfoAt[3], Float:g_InfoPos[3][3], g_InfoPlace[3][32], g_InfoN[3]   // a team's last one

// What someone saw, said to the team: enemies (a number, "they", "awp", "rushing", or a CT's
// "rush b", since CTs don't rush) and a place the map knows. Not a question. The arg has the
// spot, and how many were named times 16 (0: not said; 9: "all", "stacked")
bool:info_said(id, const c[], &arg, place[], plen)
{
	if (contain(c, "?") != -1)
		return false
	static const ASKS[][] = { "where", "wheres", "how", "who", "whos", "what", "whats", "is it", "are they", "did" }
	if (any_in(c, ASKS, sizeof ASKS))
		return false
	static const NUM[][] = { "one", "two", "three", "four", "five", "1", "2", "3", "4", "5" }
	static const MANY[][] = { "all", "all of them", "everyone", "full", "stack", "stacked", "stacking", "rush", "rushing", "lots", "a lot",
		"bunch", "many", "five", "whole team" }
	static const THEM[][] = { "they", "they are", "them", "enemy", "enemies", "awp", "awper", "sniper", "lurker", "lurking", "flank",
		"flanking", "stacked", "stacking", "coming", "pushing", "rushing", "ts", "cts", "terrorists", "spotted", "saw", "seen",
		"contact" }
	// (hurt: about them, "one b he is low"; not "im low at b", about themself)
	static const HURT[][] = { "low", "lit", "tagged", "hurt", "one shot" }
	new bool:self = word_in(c, "im") || word_in(c, "i am")
	new bool:themw = any_in(c, THEM, sizeof THEM) || (!self && any_in(c, HURT, sizeof HURT))
	// how many: the first number said, and whether a place or an enemy word comes right after
	// it ("two long", "one b", "2 awpers")
	new n, bool:placed, bool:after, word[24], probe[160], a, p[32]
	for (new i = 0; c[i]; )
	{
		while (c[i] && !isalnum(c[i]))
			i++
		new k
		while (c[i] && isalnum(c[i]))
		{
			if (k < 23)
				word[k++] = c[i]
			i++
		}
		word[k] = 0
		if (!k)
			break
		if (after)
		{
			formatex(probe, charsmax(probe), "go %s", word)
			placed = any_in(word, THEM, sizeof THEM) || (goto_target(probe, a, p, charsmax(p)) && a != SPOT_THERE && a != SPOT_HOST)
			break
		}
		for (new j = 0; j < sizeof NUM && !after; j++)
			if (equal(word, NUM[j]))
			{
				n = (j % 5) + 1
				after = true
			}
	}
	// a number in an order to teammates is how many of them go ("one of you go b", "2 rush a,
	// rest b", "one go long"), and "im one shot at b" is about themself: no enemies, unless
	// they're named too, or it's right before a place ("two long go")
	static const ORDER[][] = { "go", "push", "come", "hold", "of you", "you guys", "rest" }
	new bool:order = any_in(c, ORDER, sizeof ORDER) || (get_user_team(id) != 2 && word_in(c, "rush"))
	if (n && (order || self) && !themw && !placed)
		n = 0
	new bool:them = n > 0 || themw
	// (the bomb's down: "bomb b", "its a", "planted b" is where)
	if (!them && g_BombPlanted && (word_in(c, "bomb") || word_in(c, "planted") || word_in(c, "its") || word_in(c, "c4")))
		them = true
	// (a CT's "rush b": the Ts are rushing it; not "lets rush", "we rush")
	if (!them && get_user_team(id) == 2 && word_in(c, "rush") && !word_in(c, "lets") && !word_in(c, "we") && !word_in(c, "go"))
		them = true
	if (!them)
		return false
	// ("follow them", "kill them": orders, no place)
	formatex(probe, charsmax(probe), "go %s", c)
	if (!goto_target(probe, arg, place, plen) || arg == SPOT_THERE || arg == SPOT_HOST)
		return false
	if (!n && any_in(c, MANY, sizeof MANY))
		n = 9
	arg |= n << 4
	return true
}

// Whether a bot could see a point from where it stands (to look there, not at a wall)
bool:sees_point(bot, const Float:at[3])
{
	new Float:eye[3], Float:ofs[3], Float:to[3], Float:f
	pev(bot, pev_origin, eye)
	pev(bot, pev_view_ofs, ofs)
	for (new i = 0; i < 3; i++)
		eye[i] += ofs[i]
	to = at
	to[2] += 36.0
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, eye, to, IGNORE_MONSTERS | IGNORE_GLASS, bot, tr)
	get_tr2(tr, TR_flFraction, f)
	free_tr2(tr)
	return f >= 0.97
}

// A teammate's callout: remembered for the team; bots that can see the place glance at it;
// on CT, a site being hit (two or more, or "they are b") pulls a few from elsewhere over,
// each by its own nature (req_decide); now and then one says ok, or that it's rotating. `count`:
// how many it named (0: not said, 9: "all")
Float:info_heard(id, spot, const place[], bool:team, Float:extra, count)
{
	new t = get_user_team(id)
	if (teams_off() || (t != 1 && t != 2) || g_SteeredMode)
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "not here")
		return -1.0
	}
	new Float:at[3], label[32]
	if (!req_target(id, spot, place, at, label, charsmax(label)))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no place")
		return -1.0
	}
	new Float:now = get_gametime()
	g_InfoAt[t] = now
	g_InfoPos[t] = at
	copy(g_InfoPlace[t], charsmax(g_InfoPlace[]), label)
	g_InfoN[t] = count
	// (the team's "where are they" answers go by it too)
	copy(g_SeenPlace[t], charsmax(g_SeenPlace[]), label)
	g_SeenAt[t] = now
	trace_add(g_TrDecision, charsmax(g_TrDecision), "info")
	new bool:site = spot == SPOT_A || spot == SPOT_B || spot == SPOT_C
	new bool:many = count != 1
	new bool:moves = puppets_ok() && !g_Freeze && !g_RoundOver
	new rotated, maxrot = match_like() ? 4 : 2, first_rot, acker
	for (new b = 1; b <= MaxClients; b++)
	{
		if (b == id || !is_user_alive(b) || !is_user_bot(b) || get_user_team(b) != t || bot_afk(b) || fighting(b))
			continue
		new Float:o[3]
		pev(b, pev_origin, o)
		new Float:d = get_distance_f(o, at)
		if (t == 2 && site && many && moves && d > 1200.0 && rotated < maxrot && !g_ReqAct[b] && random_float(0.0, 1.0) < rotate_odds(b))
		{
			new sec[24]
			if (req_decide(b, id, I_GOTO, false, sec, charsmax(sec)) == REQ_YES)
			{
				spread_spot(at, g_ReqTo[b])
				copy(g_CtxSite, charsmax(g_CtxSite), label)
				req_set(b, id, I_GOTO, spot, place, team, false, false, 1)
				g_CtxSite[0] = 0
				req_attempt(b)
				rotated++
				if (!first_rot)
					first_rot = b
				trace_bot(b)
				continue
			}
		}
		if (moves && d < 2500.0 && sees_point(b, at) && random_float(0.0, 1.0) < glance_odds(b))
			server_cmd("ls_bot %d look %.0f %.0f %.0f %.1f", b, at[0], at[1], at[2] + 36.0, random_float(1.5, 3.0))
		// (the one who'd say "ok": a chatty one before a quiet one; a silent one never)
		if (!g_WasBusy[b] && can_answer(b, id) && !trait(b, "talk", "silent")
			&& (!acker || (talk_rate(b) > talk_rate(acker))))
			acker = b
	}
	if (rotated)
		trace_add(g_TrDecision, charsmax(g_TrDecision), fmt("rotate %d", rotated))
	// someone answers now and then: the one rotating says so, else an "ok"
	if (first_rot && !g_WasBusy[first_rot] && random_float(0.0, 1.0) < 0.6)
	{
		copy(g_CtxSite, charsmax(g_CtxSite), label)
		answer(first_rot, "r_info_rotate", id, true, extra + random_float(0.4, 1.2))
		g_CtxSite[0] = 0
	}
	else if (acker && random_float(0.0, 1.0) < 0.3 * talk_rate(acker) * (0.6 + 0.4 * personality()))
		answer(acker, "r_info_ok", id, true, extra + random_float(0.3, 1.0))
	return 1.5
}

// How often this bot does its own thing when the team's told to go somewhere, on a pub
Float:ignores_calls(bot)
{
	if (trait(bot, "style", "lurker") || trait(bot, "style", "roamer") || trait(bot, "style", "ninja"))
		return 0.55
	if (trait(bot, "style", "camper") || trait(bot, "style", "awper"))
		return 0.4
	if (trait(bot, "style", "rusher") || trait(bot, "style", "objective"))
		return 0.08
	return 0.25
}

// A callout: how likely it glances at the place (careful ones always look, a lurker on its
// own plan less) and, a CT elsewhere, rotates to a site being hit (objective and careful
// ones come, a lurker mostly stays)
Float:glance_odds(bot)
{
	if (trait(bot, "style", "careful") || trait(bot, "style", "awper") || trait(bot, "style", "camper"))
		return 0.9
	if (trait(bot, "style", "lurker") || trait(bot, "style", "ninja"))
		return 0.5
	if (trait(bot, "style", "rusher"))
		return 0.6
	return 0.75
}

Float:rotate_odds(bot)
{
	if (trait(bot, "style", "objective") || trait(bot, "style", "careful"))
		return 0.85
	if (trait(bot, "style", "lurker") || trait(bot, "style", "ninja") || trait(bot, "style", "camper"))
		return 0.35
	if (trait(bot, "style", "rusher"))
		return 0.75
	return 0.65
}

// How much this bot says: silent 0, quiet 0.4, normal 1, chatty 1.8, spammer 2.5
Float:talk_rate(bot)
{
	if (trait(bot, "talk", "silent"))
		return 0.0
	if (trait(bot, "talk", "quiet"))
		return 0.4
	if (trait(bot, "talk", "chatty"))
		return 1.8
	if (trait(bot, "talk", "spammer"))
		return 2.5
	return 1.0
}

// ---- buy calls, grenades, fakes, what they have ----

// "eco", "lets save", "force", "full buy": 1 save, 2 force, 0 buy; -1 not a buy call ("save
// the awp" is keeping a gun, not a save round)
buy_call(const c[])
{
	if (count_words(c) > 6 || word_in(c, "awp") || word_in(c, "gun") || word_in(c, "weapon") || word_in(c, "your"))
		return -1
	// ("save me", "save him": a player asking for help, not a save round)
	if (word_in(c, "me") || word_in(c, "us") || word_in(c, "him") || word_in(c, "her") || word_in(c, "them"))
		return -1
	static const SAVE[][] = { "eco", "lets eco", "full eco", "eco round", "we eco", "save", "lets save", "save round", "we save",
		"save this round", "full save", "everyone save", "eco this round" }
	static const FORCE[][] = { "force", "force buy", "lets force", "forcebuy", "we force", "everyone force", "force this round" }
	static const BUY[][] = { "full buy", "buy up", "everyone buy", "buy guns", "buy rifles", "lets buy", "we buy", "buy everything",
		"buy this round", "full buy this round" }
	for (new i = 0; i < sizeof FORCE; i++)
		if (phrase_is(c, FORCE[i]) || lead_word(c, "force"))
			return 2
	for (new i = 0; i < sizeof BUY; i++)
		if (phrase_is(c, BUY[i]) || (word_in(c, BUY[i]) && i < 3))
			return 0
	for (new i = 0; i < sizeof SAVE; i++)
		if (phrase_is(c, SAVE[i]) || (word_in(c, SAVE[i]) && (lead_word(c, "eco") || lead_word(c, "save") || lead_word(c, "lets"))))
			return 1
	return -1
}

// A gun's price, 0 for what this doesn't buy
gun_price(csw)
{
	switch (csw)
	{
		case CSW_AWP: return 4750
		case CSW_M4A1: return 3100
		case CSW_AK47: return 2500
		case CSW_AUG, CSW_SG552: return 3500
		case CSW_FAMAS: return 2250
		case CSW_GALIL: return 2000
		case CSW_SCOUT: return 2750
		case CSW_MP5NAVY: return 1500
		case CSW_P90: return 2350
		case CSW_M3: return 1700
		case CSW_XM1014: return 3000
		case CSW_M249: return 5750
		case CSW_DEAGLE: return 650
	}
	return 0
}

// "someone buy me an awp": in buy time, of the teammates in the buy zone with the money for
// it (and still enough for their own: a gun of their own already, or 1,500 left), each in
// turn may (who it is and how it feels about them: warm ones, friends, clanmates, the rich;
// not the cold or tilted); the first that does buys it and brings it, and that's it for the
// round. Mostly nobody: one says so now and then
Float:buyfor_heard(id, to, csw, Float:extra)
{
	new t = get_user_team(id)
	if (teams_off() || (t != 1 && t != 2) || g_SteeredMode || !puppets_ok() || !is_user_alive(id))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "not here")
		return -1.0
	}
	if (g_BuyForRound[id] == g_Round)
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "already")
		return 0.5
	}
	new price = gun_price(csw)
	new Float:bt = get_cvar_float("mp_buytime") * 60.0
	new bool:buytime = g_Freeze || get_gametime() - g_RoundLiveAt < bt
	if (!price || !buytime)
	{
		new b = to ? to : pick_mate(id, I_MONEY, 0)
		if (b && is_user_connected(b) && !g_WasBusy[b] && random_float(0.0, 1.0) < 0.5)
			answer(b, "r_buyfor_late", id, true, extra)
		trace_add(g_TrDecision, charsmax(g_TrDecision), "not buy time")
		return 1.0
	}
	new list[32], n
	for (new b = 1; b <= MaxClients; b++)
	{
		if (b == id || !is_user_alive(b) || !is_user_bot(b) || get_user_team(b) != t || bot_afk(b) || (to && b != to)
			|| !cs_get_user_buyzone(b))
			continue
		new spare = user_has_primary(b) ? 0 : 1500
		if (cs_get_user_money(b) >= price + spare)
			list[n++] = b
	}
	// (in a random order: not always the same one)
	for (new i = n - 1; i > 0; i--)
	{
		new j = random_num(0, i), x = list[i]
		list[i] = list[j]
		list[j] = x
	}
	for (new i = 0; i < n; i++)
	{
		new b = list[i]
		new Float:p = match_like() ? 0.45 : 0.22
		if (trait(b, "warmth", "warm") || clanmates(b, id))
			p *= 2.0
		if (LibraryExists("gm_people", LibType_Library) && people_match(b, id, "rel", ">=", "2") == 1)
			p *= 1.6
		if (trait(b, "warmth", "cold") || trait(b, "temper", "tilty"))
			p *= 0.4
		if (cs_get_user_money(b) >= 10000)
			p *= 1.5
		if (to)
			p *= 2.5
		if (random_float(0.0, 1.0) < floatmin(p, 0.9))
		{
			g_BuyForRound[id] = g_Round
			server_cmd("ls_bot %d buyfor %d %d", b, id, csw)
			trace_bot(b)
			if (!g_WasBusy[b] && random_float(0.0, 1.0) < 0.8 * floatmax(talk_rate(b), 0.5))
				answer(b, "r_buyfor_ok", id, true, extra + random_float(0.3, 1.0))
			trace_add(g_TrDecision, charsmax(g_TrDecision), "buying")
			return 2.0
		}
	}
	// nobody: one of them says no now and then (the one asked, mostly)
	new b = to ? to : pick_mate(id, I_MONEY, 0)
	if (b && is_user_connected(b) && !g_WasBusy[b] && random_float(0.0, 1.0) < (to ? 0.85 : 0.35) * floatmax(talk_rate(b), 0.3))
		answer(b, (cs_get_user_money(b) < price + 1500) ? "r_buyfor_broke" : "r_buyfor_no", id, true, extra + random_float(0.5, 1.5))
	trace_add(g_TrDecision, charsmax(g_TrDecision), "nobody")
	return 1.0
}

// Has a primary gun (a rifle, smg, shotgun, sniper or the para)
bool:user_has_primary(id)
{
	static const PRIMARY[] = { CSW_AK47, CSW_M4A1, CSW_AUG, CSW_SG552, CSW_FAMAS, CSW_GALIL, CSW_MP5NAVY, CSW_P90, CSW_UMP45,
		CSW_MAC10, CSW_TMP, CSW_M3, CSW_XM1014, CSW_M249, CSW_SCOUT, CSW_G3SG1, CSW_SG550, CSW_AWP }
	for (new i = 0; i < sizeof PRIMARY; i++)
		if (user_has_weapon(id, PRIMARY[i]))
			return true
	return false
}

// "flash for me", "smoke mid", "nade them": 1 flash, 2 smoke, 3 an HE; 0 none ("nice flash",
// "got flashed" and "dont flash me" aren't asking)
util_kind(const c[])
{
	if (word_in(c, "nice") || word_in(c, "flashed") || word_in(c, "smoked") || word_in(c, "naded"))
		return 0
	static const FLASH[][] = { "flash for", "flash me in", "pop a flash", "pop flash", "popflash", "throw a flash", "throw flash",
		"need a flash", "can you flash", "someone flash", "flash over", "flash it", "flash them", "flash pls", "flash please", "flash in" }
	if (lead_word(c, "flash") || any_in(c, FLASH, sizeof FLASH))
		return 1
	static const SMOKE[][] = { "throw a smoke", "smoke it", "smoke for", "need a smoke", "someone smoke", "can you smoke", "smoke off",
		"smoke pls", "smoke please", "smoke that", "smoke there" }
	if (lead_word(c, "smoke") || any_in(c, SMOKE, sizeof SMOKE))
		return 2
	static const NADE[][] = { "throw a nade", "nade them", "nade it", "nade him", "throw he", "throw an he", "he grenade", "throw nade",
		"nade pls", "nade there", "frag them", "throw a frag", "nade that", "nade please" }
	if (lead_word(c, "nade") || lead_word(c, "frag") || any_in(c, NADE, sizeof NADE))
		return 3
	return 0
}

// The extras an order was given with: walking there; a fake's second site (then over there
// after a few seconds)
req_extras(bot, bool:walk, fakeTo)
{
	g_ReqWalk[bot] = walk
	g_ReqFakeTo[bot] = fakeTo
	remove_task(TASK_FAKE + bot)
	if (fakeTo)
	{
		new data[2]
		data[0] = g_Round
		data[1] = get_user_userid(bot)
		set_task(random_float(8.0, 14.0), "task_fake_rotate", TASK_FAKE + bot, data, 2)
	}
}

public task_fake_rotate(const data[], tid)
{
	new bot = tid - TASK_FAKE
	if (data[0] != g_Round || !is_user_alive(bot) || get_user_userid(bot) != data[1] || g_ReqAct[bot] != I_GOTO || !g_ReqFakeTo[bot])
		return
	new human = g_ReqFor[bot], label[32]
	if (human < 1 || human > MaxClients || !is_user_connected(human))
		return
	new spot = g_ReqFakeTo[bot]
	g_ReqFakeTo[bot] = 0
	if (fighting(bot) || !req_target(human, spot, "", g_ReqTo[bot], label, charsmax(label)))
		return
	new Float:to[3]
	to = g_ReqTo[bot]
	spread_spot(to, g_ReqTo[bot])
	g_ReqArg[bot] = spot
	server_cmd("ls_bot %d goto %.0f %.0f %.0f %d %d bc", bot, g_ReqTo[bot][0], g_ReqTo[bot][1], g_ReqTo[bot][2], 0,
		match_like() ? random_num(20, 30) : random_num(8, 15))
	if (random_float(0.0, 1.0) < 0.35 * talk_rate(bot))
	{
		copy(g_CtxSite, charsmax(g_CtxSite), label)
		answer(bot, "r_fake_go", human, !teams_off(), random_float(0.2, 0.8))
		g_CtxSite[0] = 0
	}
}

// "eco" / "force" / "full buy" to the team (or one): each teammate bot goes along or not (in a
// match nearly all do; on a pub most; one with plenty of money ignores an eco, a careful one
// a force), and its next buy (this one, in freeze time) follows. One or two say so
Float:buy_heard(id, to, plan, Float:extra)
{
	new t = get_user_team(id)
	if (teams_off() || (t != 1 && t != 2) || g_SteeredMode || !puppets_ok())
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "not here")
		return -1.0
	}
	new took, said, refused
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b) || get_user_team(b) != t || bot_afk(b) || (to && b != to))
			continue
		new money = cs_get_user_money(b)
		new Float:p = match_like() ? 0.95 : 0.75
		if (plan == 1 && money >= 6000)
			p *= 0.4
		if (plan == 2 && trait(b, "style", "careful"))
			p *= 0.7
		if (trait(b, "temper", "tilty") || trait(b, "role", "drifter"))
			p *= 0.8
		if (to)
			p = floatmin(1.0, p + 0.15)
		if (random_float(0.0, 1.0) < p)
		{
			server_cmd("ls_bot %d buyplan %d", b, plan)
			took++
			if (!said && can_answer(b, id) && !g_WasBusy[b] && random_float(0.0, 1.0) < (to ? 0.85 : 0.45) * talk_rate(b))
			{
				answer(b, plan == 1 ? "r_buy_save" : plan == 2 ? "r_buy_force" : "r_buy_full", id, true, extra + random_float(0.3, 1.2))
				said = b
			}
		}
		else if (!refused && can_answer(b, id) && !g_WasBusy[b] && random_float(0.0, 1.0) < 0.5 * talk_rate(b))
		{
			answer(b, "r_buy_no", id, true, extra + random_float(0.8, 2.0))
			refused = b
		}
	}
	trace_add(g_TrDecision, charsmax(g_TrDecision), fmt("buy %d took %d", plan, took))
	return (said || refused) ? 1.5 : 0.5
}

// A grenade asked for: the one named, else the teammate nearest the spot with one, not in a
// fight, close enough to throw it there; it says so, and a moment later (a flash for
// someone: time for them to look away) throws it
Float:util_heard(id, to, packed, const place[], Float:extra)
{
	new t = get_user_team(id)
	if (teams_off() || (t != 1 && t != 2) || g_SteeredMode || !puppets_ok() || g_Freeze)
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "not here")
		return -1.0
	}
	new kind = packed >> 4, spot = packed & 15
	new csw = kind == 1 ? CSW_FLASHBANG : kind == 2 ? CSW_SMOKEGRENADE : CSW_HEGRENADE
	new Float:at[3], Float:normal[3], label[32]
	new bool:found = spot == SPOT_THERE ? aim_point(id, at, normal) : req_target(id, spot, place, at, label, charsmax(label))
	if (spot == SPOT_THERE && found)
		for (new i = 0; i < 3; i++)
			at[i] += normal[i] * 16.0
	new bot = 0, Float:bestd = 1600.0
	if (to && is_user_alive(to) && get_user_team(to) == t && user_has_weapon(to, csw))
		bot = to
	else if (!to)
	{
		for (new b = 1; b <= MaxClients; b++)
		{
			if (b == id || !is_user_alive(b) || !is_user_bot(b) || get_user_team(b) != t || bot_afk(b) || fighting(b) || !user_has_weapon(b, csw))
				continue
			new Float:o[3]
			pev(b, pev_origin, o)
			new Float:d = found ? get_distance_f(o, at) : 0.0
			if (d < bestd)
			{
				bestd = d
				bot = b
			}
		}
	}
	if (!bot)
	{
		// nobody with one (or the one named hasn't): it says so, now and then
		new b = to ? to : pick_mate(id, I_MONEY, 0)
		if (b && is_user_connected(b) && !g_WasBusy[b] && random_float(0.0, 1.0) < (to ? 0.8 : 0.4))
			answer(b, "r_util_none", id, true, extra)
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no grenade")
		return 1.0
	}
	trace_bot(bot)
	if (!found)
	{
		answer(bot, "r_req_where", id, true, extra)
		return 1.5
	}
	new Float:o[3]
	pev(bot, pev_origin, o)
	if (get_distance_f(o, at) > 1600.0)
	{
		answer(bot, "r_util_far", id, true, extra)
		return 1.5
	}
	new sec[24]
	if (req_decide(bot, id, I_UTIL, to != 0, sec, charsmax(sec)) != REQ_YES)
	{
		if (sec[0] && !g_WasBusy[bot])
			answer(bot, sec, id, true, extra)
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no")
		return 1.5
	}
	copy(g_CtxSite, charsmax(g_CtxSite), label)
	if (!g_WasBusy[bot])
		answer(bot, kind == 1 ? "r_util_flash" : kind == 2 ? "r_util_smoke" : "r_util_he", id, true, extra)
	g_CtxSite[0] = 0
	new data[4]
	data[0] = kind
	data[1] = floatround(at[0])
	data[2] = floatround(at[1])
	data[3] = floatround(at[2])
	remove_task(TASK_UTIL + bot)
	set_task(extra + (kind == 1 ? random_float(0.9, 1.4) : random_float(0.3, 0.8)), "task_util_throw", TASK_UTIL + bot, data, 4)
	trace_add(g_TrDecision, charsmax(g_TrDecision), "throw")
	return 2.0
}

public task_util_throw(const data[], tid)
{
	new bot = tid - TASK_UTIL
	if (!is_user_alive(bot))
		return
	static const KIND[][] = { "", "flash", "smoke", "he" }
	server_cmd("ls_bot %d throw %s %d %d %d", bot, KIND[data[0]], data[1], data[2], data[3])
}

// What one of the other side was seen carrying (by a bot of team t): an awp, a rifle or smg,
// or a pistol only
saw_guns(t, e)
{
	if (g_SawGunsRound[t] != g_Round)
	{
		g_SawGunsRound[t] = g_Round
		g_SawGuns[t] = 0
	}
	if (user_has_weapon(e, CSW_AWP))
	{
		g_SawGuns[t] |= 1
		return
	}
	static const PRIMARY[] = { CSW_AK47, CSW_M4A1, CSW_AUG, CSW_SG552, CSW_FAMAS, CSW_GALIL, CSW_MP5NAVY, CSW_P90, CSW_UMP45,
		CSW_MAC10, CSW_TMP, CSW_M3, CSW_XM1014, CSW_M249, CSW_SCOUT, CSW_G3SG1, CSW_SG550 }
	for (new i = 0; i < sizeof PRIMARY; i++)
		if (user_has_weapon(e, PRIMARY[i]))
		{
			g_SawGuns[t] |= 2
			return
		}
	g_SawGuns[t] |= 4
}

// "what do they have", "are they eco": what the team saw them with this round (an awp, rifles,
// pistols only), or that nobody's seen them yet
Float:theirs_heard(id, to, Float:extra)
{
	new t = get_user_team(id)
	if (teams_off() || (t != 1 && t != 2))
		return -1.0
	new g = g_SawGunsRound[t] == g_Round ? g_SawGuns[t] : 0
	new bot = to && is_user_connected(to) && get_user_team(to) == t && can_answer(to, id) && !g_WasBusy[to] ? to : pick_mate(id, I_SEEN, 0)
	if (!bot)
		return 0.0
	trace_bot(bot)
	if (random_float(0.0, 1.0) > (to ? 0.92 : 0.8))
		return 0.0
	answer(bot, (g & 1) ? "r_theirs_awp" : (g & 2) ? "r_theirs_rifles" : (g & 4) ? "r_theirs_eco" : "r_theirs_unknown", id, true, extra)
	trace_add(g_TrDecision, charsmax(g_TrDecision), "theirs")
	return 2.0
}

// What one part of a line asks ("money?", "cover me", "go b"), I_NONE if nothing this handles
clause_intent(id, const c[], &arg, place[], plen)
{
	arg = 0
	place[0] = 0
	new bool:q = contain(c, "?") != -1
	new words = count_words(c)
	new Float:now = get_gametime()
	// follow-ups to the bot just asked ("where?", "u sure?", "ok go")
	if (g_AskBot[id] && now - g_AskAt[id] < 20.0 && words <= 3)
	{
		if (phrase_is(c, "where") || phrase_is(c, "where at") || phrase_is(c, "where exactly") || phrase_is(c, "where tho"))
			return g_AskIntent[id] == I_WHEREYOU ? I_WHEREYOU : I_SEEN
		static const SURE[][] = { "you sure", "are you sure", "sure", "really", "rly", "for real", "srsly", "seriously", "u sure" }
		for (new i = 0; i < sizeof SURE; i++)
			if (phrase_is(c, SURE[i]))
				return I_SURE
		if ((g_AskIntent[id] == I_HOLD || g_AskIntent[id] == I_FOLLOW) && (phrase_is(c, "go") || phrase_is(c, "ok go")
			|| phrase_is(c, "go go") || phrase_is(c, "go now") || phrase_is(c, "move") || phrase_is(c, "ok move") || phrase_is(c, "lets go")))
			return I_RELEASE
	}
	// letting it go again
	static const FREE[][] = { "stop following", "dont follow", "go ahead", "you can go", "carry on", "nvm", "never mind",
		"nevermind", "forget it", "dont wait", "stop waiting", "no need to wait", "do your thing", "go do your thing" }
	if (any_in(c, FREE, sizeof FREE))
		return I_RELEASE
	static const STAYPUT[][] = { "dont move", "dont push", "dont go", "dont peek" }
	if (any_in(c, STAYPUT, sizeof STAYPUT))
		return I_HOLD
	if (word_in(c, "dont") || word_in(c, "do not") || word_in(c, "no need") || word_in(c, "never") || word_in(c, "stop following"))
		return I_NONE
	new bool:bomb = word_in(c, "bomb") || word_in(c, "c4")
	// "give me the bomb", "drop c4", "pass the bomb"
	static const GIVE[][] = { "give me", "drop", "pass", "toss", "throw me", "can i have", "i want", "i need", "let me have", "i take" }
	if (bomb && any_in(c, GIVE, sizeof GIVE) && !word_in(c, "who") && !word_in(c, "dropped"))
		return I_GIVEBOMB
	// "plant", "plant it", "plant at a", "plant near me"
	static const PLANTS[][] = { "go plant", "plant it", "plant the bomb", "plant bomb", "plant c4", "can you plant", "please plant",
		"plant at", "plant near", "plant here", "plant now", "plant already" }
	if (lead_word(c, "plant") || any_in(c, PLANTS, sizeof PLANTS))
	{
		arg = site_named(c, "b") ? SPOT_B : site_named(c, "a") ? SPOT_A : (word_in(c, "near me") || word_in(c, "here")
			|| word_in(c, "near them") || word_in(c, "near us")) ? SPOT_HUMAN : SPOT_NONE
		return I_PLANT
	}
	// "drop pls", "drop me an ak", "can u drop" (not "did they drop the awp?")
	static const DROPS[][] = { "drop me", "can you drop", "could you drop", "someone drop", "anyone drop", "pls drop", "please drop",
		"drop please", "drop pls", "need a drop", "drop plz" }
	if (word_in(c, "drop") && !bomb && !word_in(c, "dropped") && !word_in(c, "drop down") && !word_in(c, "drop to")
		&& !word_in(c, "drop off") && (lead_word(c, "drop") || any_in(c, DROPS, sizeof DROPS)))
	{
		arg = gun_asked(c)
		return I_DROP
	}
	static const KYS[][] = { "kys", "kill yourself", "kill your self", "suicide", "go die", "just die" }
	if (any_in(c, KYS, sizeof KYS) || phrase_is(c, "die"))
		return I_KYS
	if (bomb && (q || word_in(c, "where") || word_in(c, "wheres") || word_in(c, "who") || word_in(c, "has") || word_in(c, "got")))
		return I_BOMB
	// "someone buy me an awp", "can u buy me an ak"
	static const BUYME[][] = { "buy me", "buy for me", "someone buy", "anyone buy", "can you buy", "could you buy", "please buy",
		"pls buy", "buy one for me", "get me an", "get me a" }
	// (grenades aren't bought for others: "buy me a flash" falls through to asking for a flash)
	if (any_in(c, BUYME, sizeof BUYME) && (arg = gun_asked(c)) != 0 && arg != ITEM_KIT
		&& arg != CSW_FLASHBANG && arg != CSW_SMOKEGRENADE && arg != CSW_HEGRENADE)
		return I_BUYFOR
	arg = 0
	// a buy call ("eco", "lets force", "full buy"); asked ("eco?") it's about the money
	if (!q)
	{
		new b = buy_call(c)
		if (b >= 0)
		{
			arg = b
			return I_BUYCALL
		}
	}
	if (q && (phrase_is(c, "eco") || phrase_is(c, "save") || phrase_is(c, "force") || phrase_is(c, "buy")))
		return I_MONEY
	// money
	if ((word_in(c, "money") || word_in(c, "cash") || word_in(c, "$") || word_in(c, "how rich")) && !word_in(c, "save")
		&& !word_in(c, "eco") && !word_in(c, "need") && !word_in(c, "i have") && !word_in(c, "im") && !word_in(c, "no money")
		&& !word_in(c, "out of money") && !word_in(c, "broke")
		&& (q || word_in(c, "how much") || word_in(c, "your") || word_in(c, "everyone") || word_in(c, "check") || words <= 2))
		return I_MONEY
	// who has an awp
	static const WHO[][] = { "who has", "who got", "who else has", "anyone got", "anyone have", "anyone has", "someone got", "anybody got", "who has got" }
	if (any_in(c, WHO, sizeof WHO) && (arg = gun_asked(c)) != 0)
		return I_WHOHAS
	arg = 0
	// what gun
	static const GUN[][] = { "what gun", "which gun", "what weapon", "which weapon", "what you got", "what do you have",
		"whatcha got", "what are you using", "what you using", "your gun", "your weapon", "what you have", "what did you buy",
		"what you buy", "what did you get", "what you rocking" }
	if (any_in(c, GUN, sizeof GUN) || ((phrase_is(c, "weapon") || phrase_is(c, "gun") || phrase_is(c, "guns")) && q))
		return I_GUN
	// health
	if ((word_in(c, "hp") || word_in(c, "health") || word_in(c, "hurt") || word_in(c, "you low") || word_in(c, "how low")
		|| phrase_is(c, "you ok")) && !word_in(c, "im") && !word_in(c, "i am") && !word_in(c, "i have")
		&& (q || word_in(c, "how") || word_in(c, "your") || word_in(c, "you") || words <= 2))
		return I_HP
	// enemies left
	static const LEFT[][] = { "how many left", "how many alive", "how many are left", "how many enemies", "how many of them",
		"how many remain", "whos left", "who is left", "anyone left", "how many t", "how many ct", "how many ts", "how many cts",
		"how many terrorists", "how many still", "how many more" }
	if (any_in(c, LEFT, sizeof LEFT) || (q && (phrase_is(c, "how many") || phrase_is(c, "left"))))
		return I_LEFT
	// what the other side has
	static const THEIRS[][] = { "what do they have", "what guns do they", "what are they buying", "what did they buy", "what they got",
		"what have they got", "they eco", "are they eco", "is it eco", "they saving", "are they saving", "they have an awp",
		"do they have", "they got an awp", "their guns", "they buy", "are they rich", "they broke", "they poor" }
	if (any_in(c, THEIRS, sizeof THEIRS))
		return I_THEIRS
	// where
	if (word_in(c, "where") || word_in(c, "wheres"))
	{
		static const THEM[][] = { "he", "him", "they", "them", "enemy", "enemies", "awper", "camper", "guy", "last one",
			"the t", "the ct", "ts", "cts", "see", "saw", "come from", "he is", "they are", "rest", "everyone", "anyone" }
		new bool:them = any_in(c, THEM, sizeof THEM)
		if (!them && (word_in(c, "you") || word_in(c, "your")))
			return I_WHEREYOU
		if (them || phrase_is(c, "where"))
			return I_SEEN
		return I_NONE
	}
	if (phrase_is(c, "your position") || phrase_is(c, "your pos") || phrase_is(c, "position"))
		return I_WHEREYOU
	// come here
	static const COME[][] = { "come here", "come to me", "get over here", "back to me" }
	static const COME_SHORT[][] = { "over here", "come back", "regroup", "come over", "get here", "to me" }
	if (any_in(c, COME, sizeof COME) || phrase_is(c, "come") || (words <= 3 && any_in(c, COME_SHORT, sizeof COME_SHORT)))
		return I_COME
	// spray
	static const SPRAYS[][] = { "please spray", "can you spray", "go spray", "spray that", "spray here", "spray there",
		"spray your", "spray something", "spray logo" }
	if ((lead_word(c, "spray") || any_in(c, SPRAYS, sizeof SPRAYS)) && !word_in(c, "nice") && !word_in(c, "pray") && !word_in(c, "down"))
	{
		static const SPOT[][] = { "there", "here", "that", "this", "wall", "it" }
		if (any_in(c, SPOT, sizeof SPOT))
			arg = SPOT_THERE
		return I_SPRAY
	}
	// jump
	static const JUMPS[][] = { "can you jump", "please jump", "go jump", "you jump", "jump for", "jump if", "jump around", "jump updown", "jump jump" }
	if ((lead_word(c, "jump") || lead_word(c, "hop") || lead_word(c, "bhop") || any_in(c, JUMPS, sizeof JUMPS)) && !word_in(c, "nice"))
	{
		arg = (word_in(c, "updown") || word_in(c, "around") || word_in(c, "jump jump")) ? random_num(6, 10) : random_num(2, 5)
		return I_JUMP
	}
	// crouch
	static const DUCKS[][] = { "can you crouch", "please crouch", "get down", "crouch spam", "spam crouch", "you crouch", "can you duck" }
	if (lead_word(c, "crouch") || lead_word(c, "duck") || lead_word(c, "teabag") || lead_word(c, "tbag") || any_in(c, DUCKS, sizeof DUCKS))
	{
		arg = (word_in(c, "spam") || word_in(c, "teabag") || word_in(c, "tbag") || word_in(c, "updown")) ? 1 : 0
		return I_CROUCH
	}
	// a grenade asked for ("flash for me", "smoke mid", "nade them")
	new kind = util_kind(c)
	if (kind)
	{
		new probe[160], spot
		formatex(probe, charsmax(probe), "go %s", c)
		if (!goto_target(probe, spot, place, plen) || spot == SPOT_HOST)
		{
			spot = SPOT_THERE
			place[0] = 0
		}
		arg = spot | (kind << 4)
		return I_UTIL
	}
	// a fake ("fake b", "fake a then b")
	if ((lead_word(c, "fake") || word_in(c, "fake it") || word_in(c, "lets fake")) && (site_named(c, "a") || site_named(c, "b")))
	{
		new ia = contain(c, " a"), ib = contain(c, " b")
		// (the site named first is the one faked)
		arg = (ia != -1 && (ib == -1 || ia < ib)) ? SPOT_A : SPOT_B
		if (!site_named(c, "a"))
			arg = SPOT_B
		else if (!site_named(c, "b"))
			arg = SPOT_A
		return I_FAKE
	}
	// "go defuse" (a CT, the bomb down)
	static const DEFUSES[][] = { "defuse", "go defuse", "defuse it", "def it", "tap it", "stick it", "get the defuse", "go for the defuse",
		"defuse pls", "defuse please", "ill cover you", "i cover you", "go def" }
	if (g_BombPlanted && get_user_team(id) == 2 && !q && any_in(c, DEFUSES, sizeof DEFUSES) && !word_in(c, "kit") && !word_in(c, "defuser"))
		return I_DEFUSE
	// "fall back", "watch flank"
	static const BACK[][] = { "fall back", "back up", "pull back", "retreat", "go back", "get back", "back off", "everyone back" }
	if (any_in(c, BACK, sizeof BACK) && !word_in(c, "back me") && !word_in(c, "my back"))
	{
		arg = SPOT_BACK
		return I_GOTO
	}
	static const FLANK[][] = { "watch flank", "cover flank", "watch the flank", "watch our back", "watch behind", "watch our flank",
		"hold flank", "someone flank", "check flank", "watch our six", "watch the back" }
	if (any_in(c, FLANK, sizeof FLANK) || phrase_is(c, "flank"))
	{
		arg = SPOT_FLANK
		return I_GOTO
	}
	// what they saw ("two long", "they are b"), before "go" ("they are rushing b" isn't an order)
	if (info_said(id, c, arg, place, plen))
		return I_INFO
	arg = 0
	place[0] = 0
	// go somewhere (slowly, said so: walking)
	if (goto_target(c, arg, place, plen))
	{
		static const SLOW[][] = { "slow", "slowly", "walk", "walking", "quiet", "quietly", "sneak", "shift", "silent" }
		if (any_in(c, SLOW, sizeof SLOW))
			arg |= SPOT_WALK
		return I_GOTO
	}
	arg = 0
	// tag along
	static const FOLLOW[][] = { "follow me", "cover me", "come with me", "stick with me", "go with me", "back me up",
		"escort me", "stay with me", "guard me", "watch my back", "cover my back", "stick together", "bodyguard me", "tag along",
		"trade me", "trade frag me", "peek with me", "push with me", "entry with me", "swing with me", "on me", "group up", "with me" }
	// ("follow", "cover", "with me" on their own; not "thanks for the cover")
	if (any_in(c, FOLLOW, sizeof FOLLOW) || (words <= 3 && (lead_word(c, "follow") || lead_word(c, "cover") || phrase_is(c, "with me"))
		&& !word_in(c, "nice") && !word_in(c, "thanks")))
		return I_FOLLOW
	// wait, hold
	static const NOTHOLD[][] = { "wait what", "wait why", "wait how", "wait wtf", "wait lol", "stay alive", "hold fire", "hold your fire", "wait who" }
	if (any_in(c, NOTHOLD, sizeof NOTHOLD))
		return I_NONE
	static const HOLD[][] = { "wait for me", "wait here", "hold here", "stay here", "hang back", "sit tight", "camp here",
		"stay back", "slow down", "hold this", "hold that", "guard this" }
	if (any_in(c, HOLD, sizeof HOLD) || (words <= 3 && (lead_word(c, "wait") || lead_word(c, "hold") || lead_word(c, "stay")))
		|| phrase_is(c, "stop") || phrase_is(c, "stop there") || phrase_is(c, "stop moving") || phrase_is(c, "stop please") || phrase_is(c, "stop here"))
		return I_HOLD
	return I_NONE
}

// The bots a part of a line names ("mike and dave follow me": mike, dave), at most `max`
named_bots(const c[], out[], max, speaker)
{
	new n
	for (new i = 1; i <= MaxClients && n < max; i++)
		if (i != speaker && is_user_connected(i) && is_user_bot(i) && name_score(c, i) >= 1)
			out[n++] = i
	return n
}

// The things one line asks for, in order, three at most ("money? and cover me"), and who each
// is for: the bots named in that part, or named just before it ("mike and dave follow me")
parse_intents(id, const msg[], list[3], args[3], places[3][32], targets[3][4], ntargets[3])
{
	new rest[160], clause[128], n, pending[4], npending
	copy(rest, charsmax(rest), msg)
	replace_string(rest, charsmax(rest), " up and down", " updown")
	replace_string(rest, charsmax(rest), " and ", ",")
	replace_string(rest, charsmax(rest), " then ", ",")
	replace_string(rest, charsmax(rest), "&", ",")
	replace_string(rest, charsmax(rest), "+", ",")
	replace_string(rest, charsmax(rest), ". ", ",")
	replace_string(rest, charsmax(rest), "? ", "?,")
	replace_string(rest, charsmax(rest), "! ", ",")
	replace_string(rest, charsmax(rest), ";", ",")
	for (new guard = 0; rest[0] && n < 3 && guard < 8; guard++)
	{
		strtok2(rest, clause, charsmax(clause), rest, charsmax(rest), ',', TRIM_FULL)
		if (!clause[0])
			continue
		new names[4], nn = named_bots(clause, names, 4, id)
		new arg, place[32]
		new it = clause_intent(id, clause, arg, place, charsmax(place))
		if (it == I_NONE)
		{
			// just names: they're who the next part is for
			for (new k = 0; k < nn && npending < 4; k++)
				pending[npending++] = names[k]
			continue
		}
		new bool:dup
		for (new k = 0; k < n; k++)
			if (list[k] == it)
				dup = true
		if (dup)
			continue
		list[n] = it
		args[n] = arg
		copy(places[n], 31, place)
		ntargets[n] = 0
		for (new k = 0; k < npending && ntargets[n] < 4; k++)
			targets[n][ntargets[n]++] = pending[k]
		for (new k = 0; k < nn && ntargets[n] < 4; k++)
		{
			new bool:have
			for (new j = 0; j < ntargets[n]; j++)
				if (targets[n][j] == names[k])
					have = true
			if (!have)
				targets[n][ntargets[n]++] = names[k]
		}
		npending = 0
		n++
	}
	return n
}

// Questions and requests in what someone said; true when there were some (then nothing else
// answers the line). An unaddressed "kys" isn't a request: it's an insult, answered as one
bool:heard_intents(id, const msg[], to, bool:team, bool:typed)
{
	new list[3], args[3], places[3][32], targets[3][4], ntargets[3]
	new n = parse_intents(id, msg, list, args, places, targets, ntargets)
	if (!n)
		return false
	new you = to ? 0 : you_bot(id, msg, team)
	if (you && ((team && get_user_team(you) != get_user_team(id)) || you == id))
		you = 0
	// asked straight out: someone who knows nearly always answers
	static const QW[][] = { "how", "where", "wheres", "who", "whos", "what", "whats", "anyone", "any", "which", "hows" }
	new first[16]
	copyc(first, charsmax(first), msg, ' ')
	g_Direct = contain(msg, "?") != -1
	for (new i = 0; i < sizeof QW && !g_Direct; i++)
		if (equal(first, QW[i]))
			g_Direct = true
	// who's busy typing now (before this line's own answers are queued)
	new Float:now = get_gametime()
	for (new b = 1; b <= MaxClients; b++)
		g_WasBusy[b] = is_user_connected(b) && is_user_bot(b) && named_busy(b, now)
	static const INAME[][] = { "none", "money", "gun", "whohas", "hp", "left", "seen", "whereyou", "bomb", "sure", "theirs", "info",
		"drop", "release", "follow", "hold", "goto", "come", "spray", "jump", "crouch", "kys", "plant", "givebomb",
		"buycall", "util", "fake", "defuse", "buyfor" }
	new Float:extra = 0.0, handled
	for (new k = 0; k < n; k++)
	{
		new it = list[k]
		// "you" for a team question is a teammate ("u got money?" to the enemy who just talked: no)
		new def = to ? to : (you && (it >= I_DROP || get_user_team(you) == get_user_team(id))) ? you : 0
		// an unaddressed "kys" is an insult (answered as one), not a request
		if (it == I_KYS && !ntargets[k] && !def)
			continue
		trace_add(g_TrIntents, charsmax(g_TrIntents), INAME[it])
		new Float:most = -1.0
		// a callout is for the whole team, whoever's named
		if (it == I_INFO)
		{
			most = info_heard(id, args[k] & 15, places[k], team, extra, args[k] >> 4)
			if (most >= 0.0)
			{
				handled++
				extra += most
			}
			continue
		}
		if (!ntargets[k])
			most = it >= I_DROP ? request(id, def, it, args[k], places[k], team, typed, extra) : question(id, def, it, args[k], extra)
		for (new t = 0; t < ntargets[k]; t++)
		{
			new b = targets[k][t]
			if (b == id || (team && get_user_team(b) != get_user_team(id)))
			{
				// (the other side doesn't see team chat)
				if (b != id)
					trace_add(g_TrDecision, charsmax(g_TrDecision), "named enemy in team chat")
				continue
			}
			new Float:e = it >= I_DROP ? request(id, b, it, args[k], places[k], team, typed, extra + float(t) * 0.8) : question(id, b, it, args[k], extra + float(t) * 0.8)
			most = floatmax(most, e)
		}
		// (a question that doesn't apply here, as in free for all: not taken, so a named bot
		// still answers the usual way)
		if (most < 0.0)
			continue
		handled++
		extra += most
	}
	return handled > 0
}

bool:trait(bot, const key[], const value[])
{
	return LibraryExists("gm_people", LibType_Library) && people_match(bot, 0, key, "=", value) == 1
}

// Whether this bot can answer what this person said where they'd see it: with sv_alltalk 0 the
// living don't see what the dead (and spectators) type, so only the dead answer them; a dead
// bot answers someone alive only if the dead are heard (alltalk, or gm_botchat_deadchat)
bool:can_answer(bot, human)
{
	if (bot == human)
		return false
	if (test_asker(human))
		return true
	new bool:alltalk = get_cvar_num("sv_alltalk") != 0
	if (!is_user_alive(human) && !alltalk)
		return !is_user_alive(bot)
	if (!is_user_alive(bot))
		return !is_user_alive(human) || alltalk || get_pcvar_num(g_pDeadChat) != 0
	return true
}

// ---- answers ----

// A teammate bot to answer: alive (the dead know the money, the count and what they watched,
// when they can be heard), close by, the one that saw something; not one in a fight if
// there's another
pick_mate(human, it, arg, not = 0)
{
	new t = get_user_team(human), list[32], Float:w[32], n, Float:total
	new Float:ho[3], Float:o[3], bool:halive = bool:is_user_alive(human)
	if (halive)
		pev(human, pev_origin, ho)
	new Float:now = get_gametime()
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == not || i == human || !is_user_connected(i) || !is_user_bot(i) || get_user_team(i) != t || bot_afk(i) || g_AiState[i] || !can_answer(i, human))
			continue
		new bool:alive = bool:is_user_alive(i)
		if (!alive && it != I_MONEY && it != I_LEFT && it != I_SEEN && it != I_BOMB)
			continue
		if (it == I_WHOHAS && !(alive && has_item(i, arg)))
			continue
		new Float:wt = alive ? 3.0 : 1.0
		if (it == I_SEEN)
		{
			if (now - g_BotSawAt[i] < 30.0)
				wt += 5.0
			if (!alive)
				wt += 1.0
		}
		if (alive && halive)
		{
			pev(i, pev_origin, o)
			if (get_distance_f(ho, o) < 1000.0)
				wt += 2.0
		}
		if (in_combat(i))
			wt *= 0.15
		list[n] = i
		w[n++] = wt
		total += wt
	}
	if (!n)
		return 0
	new Float:r = random_float(0.0, total)
	for (new k = 0; k < n; k++)
	{
		r -= w[k]
		if (r <= 0.0)
			return list[k]
	}
	return list[n - 1]
}

// A question with a real answer, in team chat (in free for all nobody tells anyone what they
// have or where they are). How much later the next thing said on the same line comes
Float:question(id, to, it, arg, Float:extra)
{
	new t = get_user_team(id)
	if (teams_off() || (t != 1 && t != 2) || (g_SteeredMode && it != I_LEFT && it != I_SEEN))
	{
		// free for all: everyone's an enemy, so the one named laughs it off
		if (teams_off() && to && can_answer(to, id) && !g_WasBusy[to])
		{
			trace_bot(to)
			trace_add(g_TrDecision, charsmax(g_TrDecision), "ffa")
			if (random_float(0.0, 1.0) < 0.7)
				answer(to, "r_ask_enemy", id, false, extra)
			return 1.5
		}
		trace_add(g_TrDecision, charsmax(g_TrDecision), "not here")
		return -1.0
	}
	new Float:now = get_gametime()
	if (it == I_SURE)
	{
		new b = g_AskBot[id]
		if (b && is_user_connected(b) && can_answer(b, id) && random_float(0.0, 1.0) < 0.7 && answer(b, "r_sure", id, true, extra))
			return 1.5
		return 0.0
	}
	if (it == I_THEIRS)
		return theirs_heard(id, to, extra)
	static const SEC[][] = { "", "r_money", "r_gun", "r_whohas_me", "r_hp", "r_left", "r_where", "r_whereme", "r_bomb" }
	static const Float:CHANCE[] = { 0.0, 0.75, 0.7, 0.85, 0.75, 0.6, 0.7, 0.75, 0.65 }
	new Float:chance = CHANCE[it]
	new bot = to
	// one who can't be heard (dead, to someone alive), or still typing, leaves it to the others
	if (bot && (!can_answer(bot, id) || g_WasBusy[bot]))
		bot = 0
	// asked someone on the other side: a joke now and then
	if (bot && get_user_team(bot) != t)
	{
		if (random_float(0.0, 1.0) < 0.4 * (0.6 + 0.4 * personality()) && !named_busy(bot, now))
			answer(bot, "r_ask_enemy", id, false, extra)
		return 1.5
	}
	if (it == I_WHOHAS)
	{
		if (!bot || !is_user_alive(bot) || !has_item(bot, arg))
			bot = pick_mate(id, it, arg)
		if (!bot)
		{
			// nobody has one: someone says so, more often when asked straight out
			new b = pick_mate(id, I_MONEY, 0)
			trace_bot(b)
			if (b && random_float(0.0, 1.0) < (g_Direct ? 0.7 : 0.4) && answer(b, "r_whohas_none", id, true, extra))
				trace_add(g_TrDecision, charsmax(g_TrDecision), "nobody")
			else
				trace_add(g_TrDecision, charsmax(g_TrDecision), b ? "chance" : "no one to answer")
			return 1.0
		}
	}
	else if (!bot)
		bot = pick_mate(id, it, arg)
	if (!bot)
		return 0.0
	if (to)
		chance = 0.92
	else if (g_Direct)
		chance = floatmax(chance, 0.85)
	chance *= 0.75 + 0.25 * personality()
	trace_bot(bot)
	if (random_float(0.0, 1.0) > chance)
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "chance")
		return 0.0
	}
	if (!answer(bot, SEC[it], id, true, extra + random_float(0.0, 0.8)))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no line")
		return 0.0
	}
	trace_add(g_TrDecision, charsmax(g_TrDecision), "answer")
	g_AskBot[id] = bot
	g_AskIntent[id] = it
	g_AskAt[id] = now
	g_NamedNext[bot] = now + 3.0
	g_HeardRun++
	g_HeardRunAt = now
	g_NextHeard = now + random_float(2.0, 4.0)
	// asked to the team ("money?"): a couple of them answer
	if (!to && (it == I_MONEY || it == I_GUN || it == I_HP || it == I_WHEREYOU) && random_float(0.0, 1.0) < 0.45 * (0.5 + 0.5 * personality()))
	{
		new other = pick_mate(id, it, arg, bot)
		if (other)
			answer(other, SEC[it], id, true, extra + random_float(1.5, 3.5), true)
	}
	return 2.0
}

// ---- requests ----

// Loner-Strike's game DLL can be told what to do: there, with the bots playing the way it
// makes them (not the original style: bot_ls_fight 0 or gm_botstyle 0)
bool:puppets_ok()
{
	if (!g_Fork)
		return false
	new p = get_cvar_pointer("bot_ls_fight")
	return (!p || get_pcvar_num(p) != 0) && get_cvar_num("gm_botstyle") != 0
}

// The nearest living teammate bot to someone (any one if they're dead), not one of `skip`;
// one that can see what they typed
nearest_mate(human, const skip[], nskip)
{
	new t = get_user_team(human), best, Float:bestd = 999999.0
	new Float:ho[3], Float:o[3], bool:halive = bool:is_user_alive(human)
	if (halive)
		pev(human, pev_origin, ho)
	for (new i = 1; i <= MaxClients; i++)
	{
		if (i == human || !is_user_alive(i) || !is_user_bot(i) || get_user_team(i) != t || bot_afk(i) || g_AiState[i] || !can_answer(i, human))
			continue
		new bool:skipped
		for (new k = 0; k < nskip; k++)
			if (skip[k] == i)
				skipped = true
		if (skipped)
			continue
		pev(i, pev_origin, o)
		new Float:d = halive ? get_distance_f(ho, o) : random_float(0.0, 1000.0)
		if (d < bestd)
		{
			bestd = d
			best = i
		}
	}
	return best
}

// Where someone is aiming: the wall (or floor) it hits, and which way that faces
bool:aim_point(id, Float:end[3], Float:normal[3])
{
	if (!is_user_alive(id))
		return false
	new Float:eye[3], Float:ofs[3], Float:ang[3], Float:fwd[3], Float:to[3]
	pev(id, pev_origin, eye)
	pev(id, pev_view_ofs, ofs)
	pev(id, pev_v_angle, ang)
	angle_vector(ang, ANGLEVECTOR_FORWARD, fwd)
	for (new i = 0; i < 3; i++)
	{
		eye[i] += ofs[i]
		to[i] = eye[i] + fwd[i] * 3000.0
	}
	new tr = create_tr2()
	engfunc(EngFunc_TraceLine, eye, to, IGNORE_MONSTERS, id, tr)
	new Float:f
	get_tr2(tr, TR_flFraction, f)
	get_tr2(tr, TR_vecEndPos, end)
	get_tr2(tr, TR_vecPlaneNormal, normal)
	free_tr2(tr)
	return f < 1.0
}

// Somewhere a player can stand near `at`: floor under it within reach, room for a player, not
// too steep; stepping back towards `from` (up to about 300 units) until there is
bool:walkable_near(const Float:at[3], const Float:from[3], Float:out[3])
{
	new Float:dir[3], Float:len
	for (new i = 0; i < 3; i++)
		dir[i] = from[i] - at[i]
	dir[2] = 0.0
	len = floatmax(floatsqroot(dir[0] * dir[0] + dir[1] * dir[1]), 1.0)
	new tr = create_tr2()
	new bool:ok
	for (new step = 0; step <= 6 && !ok; step++)
	{
		new Float:p[3], Float:down[3], Float:f, Float:n[3]
		p[0] = at[0] + dir[0] / len * 50.0 * float(step)
		p[1] = at[1] + dir[1] / len * 50.0 * float(step)
		p[2] = at[2] + 16.0
		down = p
		down[2] -= 300.0
		engfunc(EngFunc_TraceLine, p, down, IGNORE_MONSTERS, 0, tr)
		get_tr2(tr, TR_flFraction, f)
		get_tr2(tr, TR_vecPlaneNormal, n)
		if (f >= 1.0 || n[2] < 0.7)
			continue
		get_tr2(tr, TR_vecEndPos, out)
		// room to stand (the player's box, its middle 37 units up)
		new Float:mid[3]
		mid = out
		mid[2] += 37.0
		engfunc(EngFunc_TraceHull, mid, mid, IGNORE_MONSTERS, HULL_HUMAN, 0, tr)
		if (!get_tr2(tr, TR_StartSolid) && !get_tr2(tr, TR_AllSolid))
			ok = true
	}
	free_tr2(tr)
	return ok
}

// The bomb sites' letters: a target named for its site ("bombsite_b"), else the map's order
name_sites()
{
	for (new i = 0; i < 3; i++)
		g_SiteLetter[i] = 'a' + i
	g_SiteFixed[0] = g_SiteFixed[1] = false
	new ent = -1, k, bool:named[3]
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "func_bomb_target")) > 0 && k < 3)
	{
		named[k] = site_from_name(ent, k)
		k++
	}
	ent = -1
	while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "info_bomb_target")) > 0 && k < 3)
	{
		named[k] = site_from_name(ent, k)
		k++
	}
	// two sites with the same letter: the one named keeps it
	if (g_SiteCount >= 2 && g_SiteLetter[0] == g_SiteLetter[1])
	{
		new other = g_SiteLetter[0] == 'a' ? 'b' : 'a'
		if (named[1] && !named[0])
			g_SiteLetter[0] = other
		else
			g_SiteLetter[1] = other
	}
}

bool:site_from_name(ent, k)
{
	new name[32]
	pev(ent, pev_targetname, name, charsmax(name))
	strtolower(name)
	new l = strlen(name), c = l ? name[l - 1] : 0
	if (l >= 2 && (c == 'a' || c == 'b') && (!isalpha(name[l - 2]) || (l >= 5 && (equal(name[l - 5], "site", 4) || equal(name[l - 5], "bomb", 4)))))
	{
		g_SiteLetter[k] = c
		return true
	}
	return false
}

// A place of more than one word: each telling word of it is that place, as long as no other
// place has the word too ("long a": "long"; "upper tunnel" and "lower tunnel": not "tunnel")
place_words(const place[])
{
	if (contain(place, " ") == -1)
		return
	static const PLAIN[][] = { "upper", "lower", "spawn", "side", "the", "back", "front", "top", "bottom", "middle", "inner",
		"outer", "near", "far", "left", "right", "small", "big" }
	new word[24], i, k
	while (place[i])
	{
		while (place[i] == ' ')
			i++
		k = 0
		while (place[i] && place[i] != ' ')
		{
			if (k < 23)
				word[k++] = place[i]
			i++
		}
		word[k] = 0
		if (k < 3)
			continue
		new bool:plain
		for (new p = 0; p < sizeof PLAIN; p++)
			if (equal(word, PLAIN[p]))
				plain = true
		if (plain)
			continue
		new had[32]
		if (TrieGetString(g_PlaceWord, word, had, charsmax(had)))
		{
			if (had[0] && !equal(had, place))
				TrieSetString(g_PlaceWord, word, "")
		}
		else
			TrieSetString(g_PlaceWord, word, place)
	}
}

// The nav names a place a site ("BombsiteA" -> "a"): the site nearest where someone stood in it
// is that letter (and on a two-site map, the other one the other letter)
place_learned(const place[], const Float:at[3])
{
	if (g_SiteCount < 2 || place[1] || (place[0] != 'a' && place[0] != 'b'))
		return
	new which = place[0] - 'a'
	if (g_SiteFixed[which])
		return
	new best = -1, Float:bestd = 1500.0
	for (new i = 0; i < g_SiteCount; i++)
	{
		new Float:d = get_distance_f(at, g_Sites[i])
		if (d < bestd)
		{
			bestd = d
			best = i
		}
	}
	if (best < 0)
		return
	g_SiteFixed[which] = true
	g_SiteLetter[best] = place[0]
	if (g_SiteCount == 2)
		g_SiteLetter[1 - best] = place[0] == 'a' ? 'b' : 'a'
}

site_with(letter)
{
	for (new i = 0; i < g_SiteCount; i++)
		if (g_SiteLetter[i] == letter)
			return i
	return -1
}

// Where a request is about, and how people say it ("b", "mid", "there"); false if it's
// nowhere (no b on this map, they aim at the sky, nowhere to stand there)
bool:req_target(human, spot, const place[], Float:out[3], label[], len)
{
	label[0] = 0
	static const SITE[][] = { "a", "b", "c" }
	switch (spot)
	{
		case SPOT_A, SPOT_B, SPOT_C:
		{
			new s = spot - SPOT_A
			copy(label, len, SITE[s])
			new k = site_with('a' + s)
			if (k >= 0)
			{
				out = g_Sites[k]
				return true
			}
			return bool:TrieGetArray(g_PlaceAt, SITE[s], out, 3)
		}
		case SPOT_MID:
		{
			copy(label, len, "mid")
			if (TrieGetArray(g_PlaceAt, "mid", out, 3) || TrieGetArray(g_PlaceAt, "middle", out, 3))
				return true
			if (g_HaveT && g_HaveCT)
			{
				for (new i = 0; i < 3; i++)
					out[i] = (g_SpawnT[i] + g_SpawnCT[i]) * 0.5
				return true
			}
			return false
		}
		case SPOT_THERE:
		{
			new Float:normal[3], Float:hit[3], Float:from[3]
			copy(label, len, "there")
			if (!aim_point(human, hit, normal))
				return false
			// (in front of the wall, not in it), on floor someone can stand on
			for (new i = 0; i < 3; i++)
				hit[i] += normal[i] * 24.0
			pev(human, pev_origin, from)
			return walkable_near(hit, from, out)
		}
		case SPOT_HOST:
		{
			copy(label, len, "hostages")
			new ent = engfunc(EngFunc_FindEntityByString, -1, "classname", "hostage_entity")
			if (ent <= 0)
				return false
			pev(ent, pev_origin, out)
			return true
		}
		case SPOT_PLACE:
		{
			copy(label, len, place)
			return bool:TrieGetArray(g_PlaceAt, place, out, 3)
		}
		case SPOT_HUMAN:
		{
			if (!is_user_alive(human))
				return false
			pev(human, pev_origin, out)
			return true
		}
		case SPOT_BACK:
		{
			copy(label, len, "back")
			new t = get_user_team(human)
			if (t == 1 && g_HaveT)
				out = g_SpawnT
			else if (t == 2 && g_HaveCT)
				out = g_SpawnCT
			else
				return false
			return true
		}
		case SPOT_FLANK:
		{
			copy(label, len, "flank")
			new t = get_user_team(human)
			if (!is_user_alive(human) || (t == 1 && !g_HaveT) || (t == 2 && !g_HaveCT) || (t != 1 && t != 2))
				return false
			new Float:ho[3], Float:dir[3], Float:p[3]
			pev(human, pev_origin, ho)
			for (new i = 0; i < 2; i++)
				dir[i] = (t == 1 ? g_SpawnT[i] : g_SpawnCT[i]) - ho[i]
			new Float:l = floatmax(floatsqroot(dir[0] * dir[0] + dir[1] * dir[1]), 1.0)
			// (a few distances back the way they came, a little to either side; none free: by them)
			static const Float:DIST[] = { 350.0, 250.0, 450.0, 180.0 }
			static const Float:SIDE[] = { 0.0, 0.35, -0.35 }
			for (new d = 0; d < sizeof DIST; d++)
				for (new e = 0; e < sizeof SIDE; e++)
				{
					p[0] = ho[0] + (dir[0] / l - dir[1] / l * SIDE[e]) * DIST[d]
					p[1] = ho[1] + (dir[1] / l + dir[0] / l * SIDE[e]) * DIST[d]
					p[2] = ho[2] + 32.0
					if (walkable_near(p, ho, out))
						return true
				}
			out = ho
			return true
		}
		case SPOT_BOMB:
		{
			copy(label, len, "bomb")
			new ent = -1
			while ((ent = engfunc(EngFunc_FindEntityByString, ent, "classname", "grenade")) > 0)
			{
				new model[32]
				pev(ent, pev_model, model, charsmax(model))
				if (contain(model, "c4") != -1)
				{
					pev(ent, pev_origin, out)
					return true
				}
			}
			return false
		}
	}
	return false
}

bool:fun_request(it)
{
	return it == I_SPRAY || it == I_JUMP || it == I_CROUCH || it == I_KYS
}

// How likely a bot is to do what it's asked: who it is (gm_people), how it feels about the
// person, how it's doing, the lobby
Float:req_odds(bot, human, it, bool:named)
{
	new bool:fun = fun_request(it)
	new Float:p = fun ? 0.5 : 0.72
	if (match_like() && !fun)
		p = 0.9
	if (named)
		p += 0.08
	if (trait(bot, "warmth", "warm"))
		p += 0.15
	else if (trait(bot, "warmth", "cold"))
		p -= 0.25
	if (trait(bot, "temper", "tilty"))
		p -= 0.12
	new mood[8]
	mood_of(bot, human, mood, charsmax(mood))
	if (contain(mood, "-") != -1)
		p -= 0.15
	if (fun)
	{
		if (trait(bot, "humor", "goofy"))
			p += 0.3
		else if (trait(bot, "humor", "troll"))
			p += 0.2
		else if (trait(bot, "humor", "none"))
			p -= 0.15
		if (trait(bot, "age", "kid"))
			p += 0.15
		else if (trait(bot, "age", "old"))
			p -= 0.15
		if (trait(bot, "skill", "pro") || has_persona(bot, "tE"))
			p -= 0.2
		if (has_persona(bot, "jmk"))
			p += 0.15
	}
	else
	{
		if (trait(bot, "humor", "troll"))
			p -= 0.1
		if (it == I_HOLD)
			p += trait(bot, "style", "rusher") ? -0.25 : trait(bot, "style", "camper") ? 0.15 : 0.0
		if (it == I_GOTO)
			p += trait(bot, "style", "rusher") ? 0.1 : trait(bot, "style", "camper") ? -0.1 : 0.0
		if (it == I_FOLLOW || it == I_COME)
			p -= (trait(bot, "style", "lurker") || trait(bot, "style", "camper")) ? 0.18 : trait(bot, "style", "awper") ? 0.08 : 0.0
		if (trait(bot, "skill", "newbie"))
			p += 0.1
		if (has_persona(bot, "i") && it == I_FOLLOW)
			p -= 0.15
	}
	if (has_persona(bot, "fw"))
		p += 0.1
	else if (has_persona(bot, "sO"))
		p -= 0.12
	if (LibraryExists("gm_people", LibType_Library))
	{
		if (people_match(bot, human, "rel", ">=", "2") == 1)
			p += 0.15
		else if (people_match(bot, human, "rel", "<=", "-2") == 1)
			p -= 0.3
		if (people_match(bot, human, "rival", "=", "1") == 1)
			p -= 0.25
	}
	if (clanmates(bot, human))
		p += 0.2
	new m = lobby_mood()
	p += m == 2 ? -0.1 : m == 0 ? 0.05 : 0.0
	return floatclamp(p, 0.05, 0.97)
}

// Yes or no, and the section for what it says (YES: "" for the action's own). Whether the
// game's bot then takes it is checked when it's done (req_attempt)
req_decide(bot, human, it, bool:named, section[], len)
{
	section[0] = 0
	new bool:fun = fun_request(it)
	if (!is_user_alive(bot))
	{
		copy(section, len, "r_req_dead")
		return REQ_NOPE
	}
	if (g_SteeredMode)
	{
		copy(section, len, "r_req_mode")
		return REQ_NOPE
	}
	if (match_like() && fun)
	{
		copy(section, len, "r_req_match")
		return REQ_NO
	}
	if ((it == I_FOLLOW || it == I_COME) && !alive_asker(human))
	{
		copy(section, len, "r_req_you_dead")
		return REQ_NOPE
	}
	// the game's own bots (no Loner-Strike game DLL, or its original style) can't be told
	// where to go or what to do: just an answer, as before
	if (!puppets_ok())
	{
		copy(section, len, it == I_HOLD ? "r_wait" : fun ? "r_req_no_fun" : "r_plan")
		return REQ_NOPE
	}
	if (it == I_SPRAY && (g_NoSprayNative || !g_HumanBots))
	{
		copy(section, len, "r_req_nospray")
		return REQ_NOPE
	}
	if (bot_afk(bot))
		return REQ_NOPE
	if (in_combat(bot) || bot_target(bot))
	{
		copy(section, len, "r_req_busy")
		return REQ_BUSY
	}
	// nothing with the bomb down (but going to defuse it), or in its hands (a hold in freeze
	// time too: it'd stand there with the bomb once the round starts)
	if (g_BombPlanted && it != I_SPRAY && !(it == I_GOTO && g_ReqBomb && get_user_team(bot) == 2))
	{
		copy(section, len, "r_req_planted")
		return REQ_NOPE
	}
	if (user_has_weapon(bot, CSW_C4) && it != I_SPRAY && (!g_Freeze || it == I_HOLD || it == I_GOTO || it == I_FOLLOW || it == I_COME))
	{
		copy(section, len, "r_req_bomb")
		return REQ_NOPE
	}
	if (random_float(0.0, 1.0) >= req_odds(bot, human, it, named))
	{
		copy(section, len, it == I_HOLD ? "r_req_no_hold" : (it == I_FOLLOW || it == I_COME) ? "r_req_no_follow" : fun ? "r_req_no_fun" : "r_req_no")
		return REQ_NO
	}
	return REQ_YES
}

// What it says when it does it ("omw", "k holding")
req_section(it, out[], len)
{
	switch (it)
	{
		case I_FOLLOW: copy(out, len, "r_req_follow")
		case I_HOLD: copy(out, len, "r_req_hold")
		case I_GOTO: copy(out, len, "r_req_goto")
		case I_COME: copy(out, len, "r_req_come")
		case I_SPRAY: copy(out, len, "r_req_spray")
		case I_JUMP: copy(out, len, "r_req_jump")
		case I_CROUCH: copy(out, len, "r_req_crouch")
		case I_PLANT: copy(out, len, "r_plant_ok")
		case I_GIVEBOMB: copy(out, len, "r_givebomb_ok")
		default: copy(out, len, "r_req_ok")
	}
}

// Would the game's bot take it right now? What it turns down: an enemy in view, attacking or
// just hit, the bomb (down, unless it's going to defuse it, or in its hands), a ladder; a jump
// or crouch also wants it calm (no enemy just seen, no danger around)
bool:req_ready(bot, it)
{
	if (!is_user_alive(bot) || bot_target(bot) || in_combat(bot) || (g_BombPlanted && !(it == I_GOTO && g_ReqArg[bot] == SPOT_BOMB)))
		return false
	if (user_has_weapon(bot, CSW_C4) && !g_Freeze && it != I_PLANT && it != I_GIVEBOMB)
		return false
	if ((it == I_PLANT || it == I_GIVEBOMB) && !user_has_weapon(bot, CSW_C4))
		return false
	if (pev(bot, pev_movetype) == MOVETYPE_FLY)
		return false
	if ((it == I_JUMP || it == I_CROUCH) && (bot_danger(bot) || get_gametime() - g_BotSawAt[bot] < 4.0))
		return false
	return true
}

// Remembers what it was asked (for doing it later, saying how it went, letting it go)
req_set(bot, human, it, arg, const place[], bool:team, bool:sayyes, bool:sayfail, tries)
{
	// someone else's order replaced: remembered, so their "nvm" can be answered
	if (g_ReqAct[bot] && g_ReqFor[bot] && g_ReqFor[bot] != human)
	{
		g_ReqPrevFor[bot] = g_ReqFor[bot]
		g_ReqPrevUser[bot] = g_ReqForUser[bot]
	}
	copy(g_ReqSite[bot], charsmax(g_ReqSite[]), it == I_GOTO ? g_CtxSite : "")
	g_ReqAct[bot] = it
	g_ReqArg[bot] = arg
	g_ReqFor[bot] = human
	g_ReqForUser[bot] = get_user_userid(human)
	g_ReqTeam[bot] = team
	g_ReqTries[bot] = tries
	g_ReqSayYes[bot] = sayyes
	g_ReqSayFail[bot] = sayfail
	g_ReqFirst[bot] = true
	g_ReqWaitLive[bot] = false
	g_ReqRound[bot] = g_Round
	copy(g_ReqPlace[bot], charsmax(g_ReqPlace[]), place)
}

// Forgets it: the bot died, the round's over, it gave up, they left
req_clear(bot)
{
	g_ReqAct[bot] = g_ReqFor[bot] = g_ReqPrevFor[bot] = 0
	g_ReqWaitLive[bot] = false
	g_ReqWalk[bot] = false
	g_ReqFakeTo[bot] = 0
	remove_task(TASK_FAKE + bot)
	remove_task(TASK_UTIL + bot)
	remove_task(TASK_REQ + bot)
	remove_task(TASK_REQCHECK + bot)
	remove_task(TASK_REQLIVE + bot)
	remove_task(TASK_SPRAYGO + bot)
}

// Does it (the target, if any, is in g_ReqTo[bot] already)
req_do(bot, human, it, arg)
{
	// gm_humanbots' own habits (a hold, a crouch, afk) give way to a person's order
	if (g_HumanBots && !g_NoReleaseNative && it != I_SPRAY)
		hb_release(bot)
	switch (it)
	{
		case I_FOLLOW: server_cmd("ls_bot %d follow %d %d", bot, human, match_like() ? 45 : random_num(20, 40))
		// plant: at the site nearest a point (a site asked for, or the person), else the one
		// nearest the bot
		case I_PLANT:
		{
			if (arg)
				server_cmd("ls_bot %d plant -1 %.0f %.0f %.0f", bot, g_ReqTo[bot][0], g_ReqTo[bot][1], g_ReqTo[bot][2])
			else
				server_cmd("ls_bot %d plant -1", bot)
		}
		case I_GIVEBOMB: server_cmd("ls_bot %d dropbomb %d", bot, human)
		case I_HOLD: server_cmd("ls_bot %d hold %d bc", bot, match_like() ? 15 : random_num(7, 13))
		// "go b": it holds the spot a while once there (longer in a match); "come here" just comes
		// (bc: a person's order, which the game's bot keeps through a fight; an unreachable spot it
		// turns down, and the check below says "cant rn")
		// (at the bomb it doesn't hold: its own brain defuses; walking when asked to)
		case I_GOTO: server_cmd("ls_bot %d goto %.0f %.0f %.0f %d %d bc", bot, g_ReqTo[bot][0], g_ReqTo[bot][1], g_ReqTo[bot][2],
			g_ReqWalk[bot] ? 1 : 0, g_ReqArg[bot] == SPOT_BOMB ? 0 : match_like() ? random_num(20, 30) : random_num(8, 15))
		case I_COME: server_cmd("ls_bot %d goto %.0f %.0f %.0f 0 0 bc", bot, g_ReqTo[bot][0], g_ReqTo[bot][1], g_ReqTo[bot][2])
		case I_JUMP: server_cmd("ls_bot %d jump %d %.2f", bot, arg, random_float(0.65, 0.95))
		case I_CROUCH:
		{
			if (arg)
				server_cmd("ls_bot %d crouch %d 0.3", bot, random_num(3, 5))
			else
				server_cmd("ls_bot %d crouch %d", bot, random_num(3, 6))
		}
		case I_SPRAY: spray_go(bot, arg)
	}
}

// How far it still is from where it was asked to go (or from the person it's to follow)
Float:req_dist(bot)
{
	new Float:me[3], Float:to[3]
	pev(bot, pev_origin, me)
	if (g_ReqAct[bot] == I_FOLLOW)
	{
		new h = g_ReqFor[bot]
		if (h < 1 || h > MaxClients || !is_user_alive(h))
			return 0.0
		pev(h, pev_origin, to)
		return get_distance_f(me, to)
	}
	return get_distance_f(me, g_ReqTo[bot])
}

// One go at it: if the game's bot would take it now, it's told, and watched to see it did
req_attempt(bot)
{
	new it = g_ReqAct[bot], human = g_ReqFor[bot]
	if (!it || human < 1 || human > MaxClients || !is_user_connected(human) || get_user_userid(human) != g_ReqForUser[bot] || g_ReqRound[bot] != g_Round)
	{
		req_clear(bot)
		return
	}
	if (it == I_PLANT && g_ReqArg[bot] == SPOT_HUMAN && is_user_alive(human))
		pev(human, pev_origin, g_ReqTo[bot])
	// coming to them: where they are now
	if (it == I_COME && !req_target(human, SPOT_HUMAN, "", g_ReqTo[bot], g_CtxSite, charsmax(g_CtxSite)))
	{
		req_clear(bot)
		return
	}
	if (!req_ready(bot, it))
	{
		req_took(bot, false)
		return
	}
	req_do(bot, human, it, g_ReqArg[bot])
	// a spray says so itself; the rest are watched for a moment
	if (it == I_SPRAY)
	{
		req_took(bot, true)
		return
	}
	g_CheckStart[bot] = get_gametime()
	g_CheckDist[bot] = req_dist(bot)
	remove_task(TASK_REQCHECK + bot)
	set_task(0.2, "task_req_check", TASK_REQCHECK + bot, _, _, "b")
}

// Did it take? A jump leaves the ground, a crouch ducks, a hold stands still, going somewhere
// gets closer (or is there already)
public task_req_check(tid)
{
	new bot = tid - TASK_REQCHECK
	new it = g_ReqAct[bot]
	if (!it || !is_user_alive(bot))
	{
		remove_task(tid)
		return
	}
	new flags = pev(bot, pev_flags)
	new Float:v[3]
	pev(bot, pev_velocity, v)
	new bool:seen
	switch (it)
	{
		case I_JUMP: seen = !(flags & FL_ONGROUND)
		case I_CROUCH: seen = (flags & FL_DUCKING) != 0
		// the game's bot marks an order it took (fuser4: when)
		case I_HOLD, I_GOTO, I_COME, I_FOLLOW, I_PLANT, I_GIVEBOMB:
		{
			new Float:took
			pev(bot, pev_fuser4, took)
			seen = took >= g_CheckStart[bot] - 0.05
		}
		default: seen = true
	}
	if (seen)
	{
		remove_task(tid)
		req_took(bot, true)
	}
	else if (get_gametime() - g_CheckStart[bot] >= 1.6)
	{
		remove_task(tid)
		req_took(bot, false)
	}
}

// Every living bot on the team has under $2000
bool:team_broke(t)
{
	new n
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && is_user_bot(i) && get_user_team(i) == t)
		{
			if (cs_get_user_money(i) >= 2000)
				return false
			n++
		}
	return n > 0
}

// A spot 150 to 250 units off `at` that a player can stand on (else `at` itself)
spread_spot(const Float:at[3], Float:out[3])
{
	new Float:a = random_float(0.0, 6.283), Float:r = random_float(150.0, 250.0), Float:p[3]
	p[0] = at[0] + floatcos(a) * r
	p[1] = at[1] + floatsin(a) * r
	p[2] = at[2] + 32.0
	if (!walkable_near(p, at, out))
		out = at
}

// A quick look at whoever asked (close by, or in view), as it says ok
look_at_asker(bot, human)
{
	if (!is_user_alive(bot) || !is_user_alive(human) || !g_Fork)
		return
	new Float:me[3], Float:them[3], Float:ofs[3]
	pev(bot, pev_origin, me)
	pev(bot, pev_view_ofs, ofs)
	for (new i = 0; i < 3; i++)
		me[i] += ofs[i]
	pev(human, pev_origin, them)
	pev(human, pev_view_ofs, ofs)
	for (new i = 0; i < 3; i++)
		them[i] += ofs[i]
	new bool:near = get_distance_f(me, them) < 600.0
	if (!near)
	{
		new tr = create_tr2(), Float:f
		engfunc(EngFunc_TraceLine, me, them, IGNORE_MONSTERS, bot, tr)
		get_tr2(tr, TR_flFraction, f)
		free_tr2(tr)
		if (f < 1.0)
			return
	}
	server_cmd("ls_bot %d look %.0f %.0f %.0f %.1f", bot, them[0], them[1], them[2], random_float(0.4, 0.8))
}

// It took or it didn't: "omw" (the first time; "ok now" after a "1 sec"), or "1 sec" and
// another go in a moment, or after that "cant rn" and it's forgotten
req_took(bot, bool:ok)
{
	new human = g_ReqFor[bot], it = g_ReqAct[bot]
	new bool:team = !teams_off()
	if (ok)
	{
		if (g_ReqSayYes[bot] && is_user_connected(human))
		{
			look_at_asker(bot, human)
			new sec[24]
			if (!g_ReqFirst[bot])
				copy(sec, charsmax(sec), "r_req_ok")
			// "go hostages": a CT goes to get them, a T guards them
			else if (it == I_GOTO && g_ReqArg[bot] == SPOT_HOST)
				copy(sec, charsmax(sec), get_user_team(bot) == 2 ? "r_req_host_ct" : "r_req_host_t")
			else if (it == I_GOTO && g_ReqFakeTo[bot])
				copy(sec, charsmax(sec), "r_req_fake")
			else if (it == I_GOTO && g_ReqArg[bot] == SPOT_FLANK)
				copy(sec, charsmax(sec), "r_req_flank")
			else if (it == I_GOTO && g_ReqArg[bot] == SPOT_BACK)
				copy(sec, charsmax(sec), "r_req_back")
			else if (it == I_GOTO && g_ReqArg[bot] == SPOT_BOMB)
				copy(sec, charsmax(sec), "r_req_defuse")
			else
				req_section(it, sec, charsmax(sec))
			answer(bot, sec, human, team, 0.0)
		}
		g_ReqSayYes[bot] = g_ReqSayFail[bot] = false
		g_CtxSite[0] = 0
		if (get_pcvar_num(g_pAskTest) >= 2)
			server_print("[bc] check bot=%d act=%d took=yes", bot, it)
		return
	}
	if (g_ReqTries[bot] > 0)
	{
		g_ReqTries[bot]--
		if (g_ReqFirst[bot] && g_ReqSayFail[bot] && is_user_connected(human))
			answer(bot, "r_req_later", human, team, 0.0)
		g_ReqFirst[bot] = false
		remove_task(TASK_REQ + bot)
		set_task(random_float(2.5, 4.0), "task_req_retry", TASK_REQ + bot)
		return
	}
	if (g_ReqSayFail[bot] && is_user_connected(human))
		answer(bot, "r_req_cant", human, team, 0.0)
	// (and so it really doesn't: nothing half-taken stays)
	if (is_user_alive(bot) && g_Fork)
		server_cmd("ls_bot %d release bc", bot)
	if (get_pcvar_num(g_pAskTest) >= 2)
		server_print("[bc] check bot=%d act=%d took=no", bot, it)
	req_clear(bot)
}

public task_req_retry(tid)
{
	req_attempt(tid - TASK_REQ)
}

// Asked in freeze time to go somewhere: once the round starts (it buys first)
public task_req_live(tid)
{
	new bot = tid - TASK_REQLIVE
	// a map with guns on the floor: it picks one up first
	if (armoury_wait(bot, g_ReqAct[bot]))
	{
		set_task(1.0, "task_req_live", tid)
		return
	}
	g_ReqWaitLive[bot] = false
	req_attempt(bot)
}

// On maps with guns lying around (armoury_entity), going somewhere waits until it has picked
// one up, or 8 seconds into the round
bool:armoury_wait(bot, it)
{
	static have = -1
	if (have < 0)
		have = engfunc(EngFunc_FindEntityByString, -1, "classname", "armoury_entity") > 0 ? 1 : 0
	if (!have || (it != I_GOTO && it != I_FOLLOW) || !is_user_alive(bot) || g_Freeze)
		return false
	new weapons[32], num
	get_user_weapons(bot, weapons, num)
	for (new i = 0; i < num; i++)
		if ((1 << weapons[i]) & PRIMARY_BITS)
			return false
	return get_gametime() - g_RoundLiveAt < 8.0
}

// The round's started: the ones asked in freeze time go now
start_waiting_requests()
{
	// going somewhere: a few seconds in, once it's done buying; the rest straight away
	for (new b = 1; b <= MaxClients; b++)
		if (g_ReqWaitLive[b] && is_user_connected(b))
		{
			new it = g_ReqAct[b]
			new bool:moving = it == I_GOTO || it == I_FOLLOW || it == I_COME || it == I_PLANT || it == I_GIVEBOMB
			set_task(moving ? random_float(3.0, 4.5) : random_float(0.3, 1.5), "task_req_live", TASK_REQLIVE + b)
		}
}

// The bot on the team with the bomb (0: none, or a person has it)
bomb_carrier(t)
{
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == t && user_has_weapon(i, CSW_C4))
			return i
	return 0
}

// "plant", "plant at b", "give me the bomb": the one with the bomb does it, or says why not
Float:bomb_request(id, to, it, arg, bool:team, Float:extra)
{
	new t = get_user_team(id)
	new bool:tt = !teams_off()
	new carrier = bomb_carrier(t)
	new speaker = to ? to : carrier
	// no bomb here, the wrong side, someone else has it, nobody has it
	if (!g_SiteCount || t != 1 || !carrier || !is_user_bot(carrier) || (to && to != carrier) || g_BombPlanted)
	{
		if (!speaker || !is_user_bot(speaker) || !can_answer(speaker, id))
			speaker = pick_mate(id, I_MONEY, 0)
		if (!speaker)
			return 0.0
		new sec[24]
		if (!g_SiteCount || t != 1)
			copy(sec, charsmax(sec), "r_bomb_none")
		else if (g_BombPlanted)
			copy(sec, charsmax(sec), "r_req_planted")
		else if (carrier == id)
			copy(sec, charsmax(sec), "r_bomb_you")
		else if (carrier)
		{
			short_name(carrier, g_CtxCarrier, charsmax(g_CtxCarrier))
			copy(sec, charsmax(sec), "r_bomb_notme")
		}
		else
			copy(sec, charsmax(sec), "r_bomb_none")
		trace_bot(speaker)
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no carrier")
		answer(speaker, sec, id, tt, extra)
		g_CtxCarrier[0] = 0
		return 1.5
	}
	new bot = carrier
	trace_bot(bot)
	if (!can_answer(bot, id) || g_WasBusy[bot])
		return 0.0
	new Float:me[3], Float:them[3]
	pev(bot, pev_origin, me)
	pev(id, pev_origin, them)
	new Float:apart = get_distance_f(me, them)
	// the site it's nearest, and how far
	new near = -1, Float:neard = 999999.0
	for (new k = 0; k < g_SiteCount; k++)
	{
		new Float:d = get_distance_f(me, g_Sites[k])
		if (d < neard)
		{
			neard = d
			near = k
		}
	}
	if (near >= 0)
		formatex(g_CtxSite, charsmax(g_CtxSite), "%c", g_SiteLetter[near])
	new Float:p = req_odds(bot, id, I_GOTO, to != 0)
	if (match_like())
		p = floatmax(p, 0.9)
	new sec[24]
	if (in_combat(bot) || bot_target(bot))
		copy(sec, charsmax(sec), "r_req_busy")
	else if (it == I_GIVEBOMB && (apart > 1200.0 || !alive_asker(id)))
		copy(sec, charsmax(sec), "r_bomb_come")
	else if (it == I_GIVEBOMB && neard < apart && neard < 900.0)
		copy(sec, charsmax(sec), "r_bomb_almost")
	else if (random_float(0.0, 1.0) >= p)
		copy(sec, charsmax(sec), it == I_PLANT ? "r_plant_no" : "r_givebomb_no")
	if (sec[0])
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no")
		answer(bot, sec, id, tt, extra)
		g_CtxSite[0] = 0
		return 1.5
	}
	// where to plant: the site asked for, or the one nearest the person
	new spot = SPOT_NONE
	if (it == I_PLANT && arg)
	{
		if (arg == SPOT_HUMAN)
		{
			if (alive_asker(id))
			{
				g_ReqTo[bot] = them
				spot = SPOT_HUMAN
			}
		}
		else
		{
			new k = site_with(arg == SPOT_A ? 'a' : 'b')
			if (k >= 0)
			{
				g_ReqTo[bot] = g_Sites[k]
				formatex(g_CtxSite, charsmax(g_CtxSite), "%c", g_SiteLetter[k])
				spot = arg
			}
		}
	}
	trace_add(g_TrDecision, charsmax(g_TrDecision), "yes")
	new bool:say = random_float(0.0, 1.0) < (to ? 0.9 : 0.75)
	req_set(bot, id, it, spot, "", team, say, true, 1)
	if (g_Freeze)
	{
		g_ReqWaitLive[bot] = true
		g_ReqSayYes[bot] = false
		if (say)
			answer(bot, "r_req_after", id, tt, extra)
	}
	else
		req_attempt(bot)
	return 2.0
}

// Asked to do something; how much later the next thing on the same line comes
Float:request(id, to, it, arg, const place[], bool:team, bool:typed, Float:extra)
{
	new Float:now = get_gametime()
	new t = get_user_team(id)
	if (it == I_DROP)
	{
		// asked by name of one that's broke itself: says so
		if (to && is_user_alive(to) && get_user_team(to) == t && g_BuyMode && cs_get_user_money(to) < 2000 && !g_WasBusy[to])
		{
			if (random_float(0.0, 1.0) < 0.7)
				answer(to, random_num(0, 2) ? "drop_broke" : "r_money_broke", id, true, extra)
			return 1.5
		}
		// the whole team's broke: someone says so now and then
		if (!to && g_BuyMode && team_broke(t) && random_float(0.0, 1.0) < 0.35)
		{
			new b = pick_mate(id, I_MONEY, 0)
			if (b)
				answer(b, "drop_broke_team", id, true, extra)
		}
		// typed: gm_botfun read it too and does it; said another way (voice), it's asked here
		if (!typed && !g_NoDropNative && g_BotFun && !bf_ask_drop(id, arg, to) && to && random_num(0, 1))
			answer(to, "drop_no", id, !teams_off(), extra)
		return 0.0
	}
	if (it == I_RELEASE)
		return release_for(id, to, extra)
	if (it == I_BUYCALL)
		return buy_heard(id, to, arg, extra)
	if (it == I_BUYFOR)
		return buyfor_heard(id, to, arg, extra)
	if (it == I_UTIL)
		return util_heard(id, to, arg, place, extra)
	// a fake: going to the site faked, then over to the other one; "go defuse": to the bomb
	new fakeTo = 0
	if (it == I_FAKE)
	{
		fakeTo = arg == SPOT_A ? SPOT_B : SPOT_A
		it = I_GOTO
	}
	else if (it == I_DEFUSE)
	{
		it = I_GOTO
		arg = SPOT_BOMB
	}
	new bool:walk = it == I_GOTO && (arg & SPOT_WALK) != 0
	arg &= ~SPOT_WALK
	new bool:fun = fun_request(it)
	// free for all: no teammates to ask, only the silly things, of someone named
	if (teams_off() && !fun)
	{
		if (to && is_user_alive(to) && random_float(0.0, 1.0) < 0.6)
			answer(to, "r_req_mode", id, false, extra)
		return to ? 1.5 : 0.0
	}
	new bot = to, done[33], nd
	// one who can't see it (alive, and they're dead with alltalk off) never heard it
	if (bot && !can_answer(bot, id))
		return 0.0
	if (!bot && !teams_off() && (t == 1 || t == 2))
		bot = nearest_mate(id, done, 0)
	if (!bot)
		return 0.0
	if (!teams_off() && get_user_team(bot) != t)
	{
		// someone on the other side: a joke, mostly
		if (random_float(0.0, 1.0) < 0.4 && !named_busy(bot, now))
			answer(bot, "r_req_enemy", id, false, extra)
		return 1.5
	}
	if (it == I_KYS)
		return kys(id, bot, team, extra)
	g_AskBot[id] = bot
	if (it == I_PLANT || it == I_GIVEBOMB)
		return bomb_request(id, to, it, arg, team, extra)
	g_AskIntent[id] = it
	g_AskAt[id] = now
	new section[24]
	// where to, first: nowhere ("go there" aiming at the sky, no c on this map) is a "where?"
	if (it == I_GOTO && !req_target(id, arg, place, g_ReqTo[bot], g_CtxSite, charsmax(g_CtxSite)))
	{
		g_CtxSite[0] = 0
		answer(bot, "r_req_where", id, !teams_off(), extra)
		return 1.5
	}
	// ("go defuse": the one thing asked with the bomb down)
	g_ReqBomb = it == I_GOTO && arg == SPOT_BOMB
	new res = req_decide(bot, id, it, to != 0, section, charsmax(section))
	new bool:say = random_float(0.0, 1.0) < (to ? 0.9 : 0.65 * (0.6 + 0.4 * personality()))
	// still typing something: it does it, but doesn't queue another line
	if (g_WasBusy[bot])
		say = false
	static const RES[][] = { "yes", "no", "busy", "later", "cant" }
	trace_bot(bot)
	trace_add(g_TrDecision, charsmax(g_TrDecision), RES[res])
	if (res == REQ_YES)
	{
		req_set(bot, id, it, arg, place, team, say, !g_WasBusy[bot], 1)
		req_extras(bot, walk, fakeTo)
		// in freeze time it all waits for the round (it buys first): "k" now; on a map with guns
		// on the floor, going somewhere waits until it has one
		if (armoury_wait(bot, it))
		{
			g_ReqWaitLive[bot] = true
			set_task(1.0, "task_req_live", TASK_REQLIVE + bot)
		}
		else if (g_Freeze)
		{
			g_ReqWaitLive[bot] = true
			g_ReqSayYes[bot] = false
			if (say)
				answer(bot, "r_req_after", id, !teams_off(), extra)
		}
		else
			req_attempt(bot)
	}
	// in a fight: half the time it doesn't even answer
	else if (section[0] && !g_WasBusy[bot] && (res == REQ_BUSY ? say && random_num(0, 1) == 1 : say || to))
		answer(bot, section, id, !teams_off(), extra)
	g_CtxSite[0] = 0
	// said to the team, not to one: "go b" is a call the whole team hears, and each decides for
	// itself (in a match nearly all go; on a pub some do, a lurker may not); "wait" is for the
	// ones close by; "follow me", "come here" maybe one more. The first one being busy doesn't
	// stop the rest, and if it didn't take it, the first that does may say so
	if (!to && (it == I_GOTO || it == I_HOLD || it == I_FOLLOW || it == I_COME))
	{
		// (a call to the team: every teammate hears it and decides for itself)
		new more = (it == I_GOTO || it == I_HOLD) ? 31 : (random_float(0.0, 1.0) < 0.35 ? 1 : 0)
		// (one watches the flank, one goes for the defuse; "fall back" is for the ones close by)
		if (it == I_GOTO && (arg == SPOT_FLANK || arg == SPOT_BOMB))
			more = 0
		new bool:near = it == I_HOLD || (it == I_GOTO && arg == SPOT_BACK)
		new bool:acked = res == REQ_YES && say
		new Float:ho[3], bool:halive = bool:is_user_alive(id)
		if (halive)
			pev(id, pev_origin, ho)
		done[nd++] = bot
		for (new k = 0; k < more; k++)
		{
			new other = nearest_mate(id, done, nd)
			if (!other)
				break
			done[nd++] = other
			if (near && halive)
			{
				new Float:oo[3]
				pev(other, pev_origin, oo)
				if (get_distance_f(ho, oo) > 1200.0)
					break
			}
			// (on a pub not everyone listens: a lurker or roamer often does its own thing, a
			// rusher or objective player is happy to go)
			if (it == I_GOTO && !match_like() && random_float(0.0, 1.0) < ignores_calls(other))
				continue
			new sec2[24]
			// (not all on the same spot: each a little to the side)
			if (it == I_GOTO)
				spread_spot(g_ReqTo[bot], g_ReqTo[other])
			if (req_decide(other, id, it, false, sec2, charsmax(sec2)) != REQ_YES)
				continue
			new bool:sayit = !acked && !g_WasBusy[other] && random_float(0.0, 1.0) < 0.6
			if (sayit)
				acked = true
			req_set(other, id, it, arg, place, team, sayit, false, 1)
			req_extras(other, walk, fakeTo)
			if (armoury_wait(other, it))
			{
				g_ReqWaitLive[other] = true
				set_task(1.0, "task_req_live", TASK_REQLIVE + other)
			}
			else if (g_Freeze)
				g_ReqWaitLive[other] = true
			else
				req_attempt(other)
		}
	}
	g_ReqBomb = false
	return 2.0
}

// "ok go", "nvm": the bots doing something for this person stop (only what this plugin asked:
// another plugin's hold on the same bot stays)
Float:release_for(id, to, Float:extra)
{
	new first
	for (new b = 1; b <= MaxClients; b++)
	{
		if (!is_user_connected(b) || !is_user_bot(b) || (to && b != to) || g_ReqFor[b] != id || !g_ReqAct[b])
			continue
		if (is_user_alive(b) && !g_ReqWaitLive[b])
			server_cmd("ls_bot %d release bc", b)
		req_clear(b)
		if (!first)
			first = b
	}
	if (first && random_float(0.0, 1.0) < 0.6)
	{
		answer(first, "r_req_release", id, !teams_off(), extra)
		return 1.5
	}
	// their order was replaced by someone else's: say so ("bob told me to go b")
	if (!first)
		for (new b = 1; b <= MaxClients; b++)
		{
			if (!is_user_connected(b) || !is_user_bot(b) || (to && b != to) || g_ReqPrevFor[b] != id || !g_ReqAct[b])
				continue
			new other = g_ReqFor[b]
			g_ReqPrevFor[b] = 0
			if (other < 1 || other > MaxClients || !is_user_connected(other))
				continue
			short_name(other, g_CtxCarrier, charsmax(g_CtxCarrier))
			copy(g_CtxSite, charsmax(g_CtxSite), g_ReqSite[b])
			answer(b, "r_req_taken", id, !teams_off(), extra)
			g_CtxCarrier[0] = g_CtxSite[0] = 0
			return 1.5
		}
	return 0.0
}

// Spraying where the person aims: over to it if it's further than a step, then the logo
// (SPOT_THERE); anywhere else, the first wall by it
spray_go(bot, arg)
{
	new human = g_ReqFor[bot]
	new Float:at[3], Float:normal[3], Float:zero[3]
	if (arg == SPOT_THERE && human >= 1 && human <= MaxClients && aim_point(human, at, normal) && floatabs(normal[2]) < 0.7)
	{
		new Float:me[3], Float:stand[3], Float:in_front[3]
		pev(bot, pev_origin, me)
		g_SprayAt[bot] = at
		g_SprayNormal[bot] = normal
		if (get_distance_f(me, at) > 170.0)
		{
			for (new i = 0; i < 3; i++)
				in_front[i] = at[i] + normal[i] * 50.0
			if (!walkable_near(in_front, me, stand))
			{
				hb_spray(bot, zero, zero)
				return
			}
			server_cmd("ls_bot %d goto %.0f %.0f %.0f", bot, stand[0], stand[1], stand[2])
			g_SprayUntil[bot] = get_gametime() + 15.0
			remove_task(TASK_SPRAYGO + bot)
			set_task(0.5, "task_spray_go", TASK_SPRAYGO + bot, _, _, "b")
			return
		}
		hb_spray(bot, at, normal)
		return
	}
	hb_spray(bot, zero, zero)
}

public task_spray_go(tid)
{
	new bot = tid - TASK_SPRAYGO
	if (!is_user_alive(bot) || get_gametime() > g_SprayUntil[bot] || fighting(bot))
	{
		remove_task(tid)
		return
	}
	new Float:me[3]
	pev(bot, pev_origin, me)
	if (get_distance_f(me, g_SprayAt[bot]) <= 170.0)
	{
		remove_task(tid)
		hb_spray(bot, g_SprayAt[bot], g_SprayNormal[bot])
	}
}

// "kys" said to a bot: a toxic or goofy one, with rude bots on or a toxic lobby, out of a
// fight, on a pub round where it doesn't matter, not often: does it ("fine. bye"). Everyone
// else answers
Float:kys(id, bot, bool:team, Float:extra)
{
	new Float:now = get_gametime()
	new mates
	new t = get_user_team(bot)
	for (new i = 1; i <= MaxClients; i++)
		if (is_user_alive(i) && get_user_team(i) == t)
			mates++
	new bool:allowed = (rude_on() || lobby_mood() == 2) && !match_like() && !g_SteeredMode && is_user_alive(bot) && !g_Freeze
		&& !fighting(bot) && !g_KysDone[bot] && now - g_KysAt > 300.0 && !g_BombPlanted && !user_has_weapon(bot, CSW_C4)
		&& (teams_off() || g_LastRound <= 0.0 || mates >= 3)
	new bool:goofy = trait(bot, "humor", "goofy")
	new bool:kind = goofy || has_persona(bot, "sjmzk") || trait(bot, "humor", "troll") || trait(bot, "temper", "tilty")
	if (allowed && kind && random_float(0.0, 1.0) < (goofy ? 0.25 : 0.15))
	{
		g_KysDone[bot] = true
		g_KysAt = now
		answer(bot, "r_kys_yes", id, false, extra)
		// (the round and who it is go with it: a new round or someone else in the slot, no)
		new data[2]
		data[0] = g_Round
		data[1] = get_user_userid(bot)
		remove_task(TASK_KYS + bot)
		set_task(extra + random_float(3.5, 5.0), "task_kys", TASK_KYS + bot, data, 2)
		return 2.0
	}
	if (random_float(0.0, 1.0) < 0.6 * (0.6 + 0.4 * personality()))
		answer(bot, "r_kys_no", id, team, extra)
	return 1.5
}

public task_kys(const data[], tid)
{
	new bot = tid - TASK_KYS
	if (data[0] != g_Round || !is_user_connected(bot) || get_user_userid(bot) != data[1] || !is_user_alive(bot)
		|| fighting(bot) || g_Freeze || g_BombPlanted || user_has_weapon(bot, CSW_C4))
		return
	user_kill(bot)
}

// ---- what the model is told about the bot, and what it may have it do ----

// gm_people's traits, in a few words ("warm, short-tempered, goofy, a teen, good, a rusher,
// into wc3 and cars")
traits_text(bot, out[], len)
{
	out[0] = 0
	if (!LibraryExists("gm_people", LibType_Library))
		return
	static const KEYS[][] = { "warmth", "temper", "humor", "age", "skill", "style", "region" }
	static const VALUES[][] = { "cold neutral warm", "chill normal tilty", "none dry goofy troll", "kid teen adult old",
		"newbie average good pro", "rusher roamer lurker careful camper awper knifer nader ninja objective weirdgun",
		"us uk de se pl ru br fr nl es au fi dk" }
	new val[16], list[96]
	for (new k = 0; k < sizeof KEYS; k++)
	{
		copy(list, charsmax(list), VALUES[k])
		while (list[0])
		{
			strtok2(list, val, charsmax(val), list, charsmax(list), ' ', TRIM_FULL)
			if (val[0] && people_match(bot, 0, KEYS[k], "=", val) == 1)
			{
				add(out, len, out[0] ? ", " : "")
				add(out, len, fmt("%s %s", KEYS[k], val))
				break
			}
		}
	}
	new h1[16], h2[16]
	people_hobby(bot, 0, h1, charsmax(h1))
	people_hobby(bot, 1, h2, charsmax(h2))
	if (h1[0])
		add(out, len, fmt(", into %s%s%s", h1, h2[0] ? " and " : "", h2))
}

// How the bot knows the person: "clanmate", "rival", "friend", "regular", "dislikes them", "stranger"
relation_text(bot, human, out[], len)
{
	new bool:people = bool:LibraryExists("gm_people", LibType_Library)
	if (clanmates(bot, human))
		copy(out, len, "clanmate")
	else if (people && people_match(bot, human, "rival", "=", "1") == 1)
		copy(out, len, "rival")
	else if (people && people_match(bot, human, "rel", "<=", "-2") == 1)
		copy(out, len, "dislikes them")
	else if (people && people_match(bot, human, "rel", ">=", "2") == 1 && people_match(bot, human, "fam", ">=", "3") == 1)
		copy(out, len, "friend")
	else if (people && people_match(bot, human, "fam", ">=", "3") == 1)
		copy(out, len, "regular")
	else
		copy(out, len, "stranger")
}

// What the model may have the bot do here: none where requests don't fit
bool:ai_offers_actions(bot, human)
{
	return puppets_ok() && is_user_alive(bot) && !teams_off() && !g_SteeredMode && get_user_team(bot) == get_user_team(human)
}

// The model said to do something: the same yes or no as asking in chat. False with the
// section for what it says instead when it won't (or can't); true when it's done
bool:ai_action(bot, human, const act[], section[], len)
{
	section[0] = 0
	new it, arg
	if (equal(act, "follow"))
		it = I_FOLLOW
	else if (equal(act, "hold"))
		it = I_HOLD
	else if (equal(act, "goto_a") || equal(act, "goto_b"))
	{
		it = I_GOTO
		arg = act[5] == 'a' ? SPOT_A : SPOT_B
	}
	else if (equal(act, "come"))
		it = I_COME
	else if (equal(act, "spray"))
		it = I_SPRAY
	else if (equal(act, "jump"))
	{
		it = I_JUMP
		arg = random_num(2, 5)
	}
	else if (equal(act, "crouch"))
		it = I_CROUCH
	else if (equal(act, "drop"))
	{
		// a drop on its way, or a "no" (never a "here u go" with nothing coming)
		if (!g_NoDropNative && g_BotFun && bf_ask_drop(human, 0, bot))
			return true
		copy(section, len, "drop_no")
		return false
	}
	if (!it || !ai_offers_actions(bot, human))
		return true
	if (it == I_GOTO)
	{
		new label[16]
		if (!req_target(human, arg, "", g_ReqTo[bot], label, charsmax(label)))
		{
			copy(section, len, "r_req_where")
			return false
		}
	}
	if (req_decide(bot, human, it, true, section, len) != REQ_YES)
		return false
	// the model's line says yes; "1 sec" or "cant rn" come after if the game's bot doesn't take it
	req_set(bot, human, it, arg, "", g_AiTeam[bot], false, true, 1)
	if (g_Freeze)
	{
		g_ReqWaitLive[bot] = true
		return true
	}
	if (!req_ready(bot, it))
	{
		// not now: "1 sec" instead of the model's yes, and another go in a moment
		copy(section, len, "r_req_later")
		g_ReqFirst[bot] = false
		set_task(random_float(2.5, 4.0), "task_req_retry", TASK_REQ + bot)
		return false
	}
	req_attempt(bot)
	return true
}

// The money each one starts the round with (a moment after it starts: the round's pay is in
// by then), for pistol / eco / force / full; the per-round things noticed about people
public task_start_money()
{
	for (new i = 1; i <= MaxClients; i++)
	{
		g_SaidAbout[i] &= ~(SAID_CAMP | SAID_RUSH)
		if (is_user_connected(i))
			g_StartMoney[i] = cs_get_user_money(i)
	}
}

heard(id, bool:team)
{
	new msg[128]
	read_args(msg, charsmax(msg))
	remove_quotes(msg)
	heard_text(id, msg, team, true)
}

// gm_botchat_ask <userid> "<text>" [team]: as if that person said it (in team chat with
// "team" or 1): voice, said and turned into text
public cmd_ask()
{
	if (read_argc() < 3)
	{
		server_print("gm_botchat_ask <userid> ^"<text>^" [team]")
		return PLUGIN_HANDLED
	}
	new arg[16], text[128]
	read_argv(1, arg, charsmax(arg))
	new id = find_player("k", str_to_num(arg[arg[0] == '#' ? 1 : 0]))
	read_argv(2, text, charsmax(text))
	read_argv(3, arg, charsmax(arg))
	if (id && (!is_user_bot(id) || get_pcvar_num(g_pAskTest)))
	{
		g_TestAsker = is_user_bot(id) ? id : 0
		heard_text(id, text, equali(arg, "team") || equal(arg, "1"), false)
		g_TestAsker = 0
	}
	return PLUGIN_HANDLED
}

// Someone typing line after line (more than six in ten seconds): bots stop answering each
// one, and one of them may tell them to calm down
bool:spamming(id, Float:now)
{
	g_SpamAt[id][g_SpamN[id] % 8] = now
	g_SpamN[id]++
	new recent
	for (new i = 0; i < 8; i++)
		if (now - g_SpamAt[id][i] < 10.0)
			recent++
	if (recent > 6 && now >= g_SpamUntil[id])
	{
		g_SpamUntil[id] = now + 20.0
		new b = any_bot(0)
		if (b)
			want(b, "r_spam", id, 0.6, random_float(1.0, 2.5), 1)
	}
	return now < g_SpamUntil[id]
}

// A bot answering by name: not while it's still typing (or waiting on the model), and not
// again straight away
bool:named_busy(bot, Float:now)
{
	if (g_AiState[bot] || now < g_NamedNext[bot])
		return true
	for (new i = 0; i < MAX_PENDING; i++)
		if (g_PendBot[i] == bot)
			return true
	return false
}

// The bot standing in for a person in the simulation (gm_botchat_test 2): alive, on its team
bool:test_asker(id)
{
	return id && id == g_TestAsker && get_pcvar_num(g_pAskTest) >= 2
}

bool:alive_asker(id)
{
	return test_asker(id) || bool:is_user_alive(id)
}

// Adds to one part of the trace line ("a,b")
trace_add(field[], len, const what[])
{
	if (!g_Tracing || !what[0])
		return
	if (field[0])
		add(field, len, ",")
	add(field, len, what)
}

trace_bot(bot)
{
	if (!g_Tracing || !bot)
		return
	new name[32]
	get_user_name(bot, name, charsmax(name))
	trace_add(g_TrTarget, charsmax(g_TrTarget), name)
}

// What a person said (typed: through say / say_team, so gm_botfun has heard it too); with
// gm_botchat_test 2, how it was taken goes to the console
heard_text(id, const said[], bool:team, bool:typed)
{
	g_Tracing = get_pcvar_num(g_pAskTest) >= 2
	g_TrIntents[0] = g_TrTarget[0] = g_TrSection[0] = g_TrDecision[0] = g_TrReply[0] = 0
	heard_line(id, said, team, typed)
	if (g_Tracing)
		server_print("[bc] ^"%s^" -> intents=%s target=%s section=%s decision=%s reply=^"%s^"", said,
			g_TrIntents[0] ? g_TrIntents : "none", g_TrTarget[0] ? g_TrTarget : "none", g_TrSection[0] ? g_TrSection : "none",
			g_TrDecision[0] ? g_TrDecision : "none", g_TrReply)
	g_Tracing = false
}

heard_line(id, const said[], bool:team, bool:typed)
{
	if (!is_user_connected(id) || (is_user_bot(id) && id != g_TestAsker) || !get_pcvar_num(g_pHumans) || g_Career || !get_pcvar_num(g_pLevel))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "off")
		return
	}
	new msg[128]
	copy(msg, charsmax(msg), said)
	trim(msg)
	if (!msg[0] || msg[0] == '/' || msg[0] == '!')
		return
	// nobody else saw it (muted or filtered), so no bot answers it
	if (LibraryExists("gm_community", LibType_Library) && (typed ? gm_chat_blocked(id, msg) != 0 : (!g_NoVoiceBlocked && gm_voice_blocked(id, msg) != 0)))
		return
	copy(g_HumanMsg, charsmax(g_HumanMsg), msg)
	hist_add(id, msg, team ? get_user_team(id) : 0)
	normalize(msg)
	// "timeleft", "nextmap": the server answers those itself
	new first[16]
	copyc(first, charsmax(first), msg, ' ')
	if (equal(first, "timeleft") || equal(first, "nextmap") || equal(first, "currentmap") || equal(first, "thetime"))
		return
	new Float:now = get_gametime()
	if (spamming(id, now))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "spam")
		return
	}

	new to = addressed(msg)
	// a bot on the other side doesn't see team chat (and the stand-in isn't talking to itself)
	if (to && ((team && get_user_team(to) != get_user_team(id)) || to == id))
		to = 0
	// questions with real answers and things to do ("money?", "cover me", "go b")
	if (heard_intents(id, msg, to, team, typed))
		return
	new section[24]
	classify(msg, section, charsmax(section))
	// answering the bot that was just talking to them
	if (continue_thread(id, msg, section, to, team))
	{
		trace_add(g_TrSection, charsmax(g_TrSection), section)
		trace_add(g_TrDecision, charsmax(g_TrDecision), "thread")
		return
	}
	// someone typing a lot doesn't get an answer to every line; said to a bot by name, that
	// bot doesn't answer twice at once either
	// "great team" after three lost rounds in a row: mostly sarcasm
	new st = get_user_team(id)
	if (equal(section, "r_team_good") && (st == 1 || st == 2) && g_LoseRun[st] >= 3 && random_num(1, 100) <= 60)
		copy(section, charsmax(section), "r_team_bad")
	// "beast tariq", "insane dave": praise for the one named
	static const WOW[][] = { "sick", "insane", "clutch", "beast" }
	if (!section[0] && to && any_in(msg, WOW, sizeof WOW))
		copy(section, charsmax(section), "r_nice_from_human")
	// "nice" right after a teammate bot killed them: sarcasm, and that bot says sorry
	new tker = g_KillerOf[id]
	if (equal(section, "r_nice_from_human") && tker && now - g_KillerOfAt[id] < 10.0 && is_user_connected(tker) && is_user_bot(tker)
		&& get_user_team(tker) == get_user_team(id) && !teams_off())
	{
		copy(section, charsmax(section), "r_sorry_tk")
		if (!to)
			to = tker
	}
	// the one named is still typing: it doesn't queue a second line, and nobody answers for it
	if (to && named_busy(to, now))
	{
		trace_bot(to)
		trace_add(g_TrDecision, charsmax(g_TrDecision), "named busy")
		return
	}
	if (rate_capped(now) || (!to && now < g_NextHeard))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "rate")
		return
	}
	if (!section[0])
		copy(section, charsmax(section), to ? "r_named" : "r_ack")

	new Float:chance, who, Float:second
	reply_style(section, chance, who, second)
	// who it's about: the bot that killed them or that they just killed, the one that got the kill
	new prefer = to
	if (!prefer && who == WHO_INVOLVED)
	{
		if (g_KillerOf[id] && now - g_KillerOfAt[id] < 15.0 && is_user_bot(g_KillerOf[id]))
			prefer = g_KillerOf[id]
		else if (g_VictimOf[id] && now - g_VictimOfAt[id] < 15.0 && is_user_bot(g_VictimOf[id]))
			prefer = g_VictimOf[id]
	}
	else if (!prefer && who == WHO_KILLER)
	{
		if (g_KillerOf[id] && now - g_KillerOfAt[id] < 10.0 && is_user_bot(g_KillerOf[id]))
			prefer = g_KillerOf[id]
		else if (g_LastBotKiller && now - g_LastBotKillAt < 6.0)
			prefer = g_LastBotKiller
		// "nice round", "good round" with nobody's kill to thank: the team answers ("gj all")
		else if (word_in(msg, "round") || word_in(msg, "all") || word_in(msg, "guys") || word_in(msg, "team"))
		{
			copy(section, charsmax(section), "r_round_gj")
			reply_style(section, chance, who, second)
		}
		else
			chance *= 0.3          // "nice" about nothing in particular
	}
	// how it lands (gm_people remembers): praise, thanks, a greeting, a laugh, an insult, an
	// accusation; said to everyone, a greeting or a gg warms everyone a little
	note_feeling(id, prefer, section)
	// busy fighting: someone else answers
	if (prefer && prefer != to && in_combat(prefer))
		prefer = 0
	if (to)
		chance = floatmax(chance, 0.85)

	// the more they've answered lately, the less they answer now
	if (now - g_HeardRunAt > 45.0)
		g_HeardRun = 0
	chance /= 1.0 + (to ? 0.2 : 0.6) * float(g_HeardRun)
	// at personality 0 they still answer when spoken to, at 200 a bit more
	chance *= 0.6 + 0.4 * personality()
	// trash talk back is capped like any other (rude bots off), and the lobby's mood counts
	chance *= floatmin(mood_factor(section), 1.3)
	trace_add(g_TrSection, charsmax(g_TrSection), section)
	if (random_float(0.0, 1.0) > floatmin(chance, 0.95))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "chance")
		return
	}

	new bot = responder(id, prefer, who, team)
	trace_bot(bot)
	if (!answer(bot, section, id, team, 0.0))
	{
		trace_add(g_TrDecision, charsmax(g_TrDecision), "no line")
		return
	}
	trace_add(g_TrDecision, charsmax(g_TrDecision), "reply")
	g_HeardRun++
	g_HeardRunAt = now
	g_NextHeard = now + random_float(2.0, 4.0)
	g_NamedNext[bot] = now + random_float(3.0, 6.0)
	// now and then a second one ("gg", "hi"), a little later; it doesn't start a thread
	if (second > 0.0 && random_float(0.0, 1.0) <= second * (0.5 + 0.5 * personality()))
	{
		new other = responder(id, 0, who, team)
		if (other && other != bot)
			answer(other, section, id, team, random_float(1.5, 4.0), true)
	}
}

// ---- smarter chat: a small local model writes answers to people -------------------------
// The app runs the model and sets gm_ai_url to "127.0.0.1:port" while it's ready ("" when
// not). Answers to people and thread lines go to it (POST /api/botchat/reply); a 200 with
// {"text": "..."} is what the bot types. Anything else, no answer within AI_TIMEOUT, or no
// sockets module: the written line picked as usual. Whether a bot answers at all is still
// decided here, so the model doesn't make bots chattier. Never blocks: a nonblocking socket
// polled every 0.1 s, one request per bot.

// What each personality is, for the model (same order as PERSONAS)
new const PERSONA_DESC[][] = { "friendly", "tryhard", "salty", "joker", "quiet", "new player", "chill", "old-timer",
	"kid", "english is not their first language", "hype", "zoomer", "streamer wannabe", "backseat gamer", "the team's caller",
	"half asleep", "blames themselves", "wholesome", "military roleplay", "meme lord", "anime fan", "brit",
	"conspiracy theorist", "parent who games after the kids are in bed", "esports wannabe", "foodie", "grammar nerd",
	"sees hackers everywhere", "lurker", "music fan", "stats nerd", "overconfident", "pacifist", "movement nerd", "map tourist",
	"yapper", "works in IT", "gym regular", "wants the admin", "trivia nerd", "patient coach", "pirate", "villain roleplay", "philosopher", "extreme 2000s gamer",
	"clutch hero, plays bold when last alive", "jokes about being a bot", "cowboy", "detective", "leetspeak",
	"awper, always wants the awp", "rusher, first out of spawn", "nade spammer", "lagger, blames the ping",
	"map voter, always wants rtv", "plays weird guns like shotguns and the tmp", "camper, holds one spot",
	"knifer, loves knife kills", "ninja, sneaky and quiet", "clan recruiter", "objective player, wants the bomb planted",
	"spray artist" }

ai_init()
{
	g_Sockets = bool:LibraryExists("sockets", LibType_Library)
}

// Something said in chat, for the history the model sees (team: 0 everyone, 1 or 2 that team)
hist_add(id, const text[], team)
{
	new name[32]
	get_user_name(id, name, charsmax(name))
	formatex(g_Hist[g_HistPos], charsmax(g_Hist[]), "%s: %s", name, text)
	g_HistTeam[g_HistPos] = team
	g_HistPos = (g_HistPos + 1) % HIST
	if (g_HistCount < HIST)
		g_HistCount++
}

// gm_ai_url split into host and port; false when the model isn't there
bool:ai_target(host[], len, &port)
{
	if (!g_Sockets || !g_pAiUrl)
		return false
	new url[64]
	get_pcvar_string(g_pAiUrl, url, charsmax(url))
	trim(url)
	new colon = contain(url, ":")
	if (colon <= 0)
		return false
	port = str_to_num(url[colon + 1])
	url[colon] = 0
	copy(host, len, url)
	return port > 0 && port < 65536
}

// `src` as the inside of a JSON string: quotes, backslashes and control characters escaped;
// bytes past ascii as \u00XX, so the request is always valid JSON
json_add(out[], len, const src[])
{
	new k = strlen(out)
	for (new i = 0; src[i] && k < len - 7; i++)
	{
		new c = src[i] & 0xFF
		if (c == '"' || c == '\')
		{
			out[k++] = '\'
			out[k++] = c
		}
		else if (c == 10)
		{
			out[k++] = '\'
			out[k++] = 'n'
		}
		else if (c >= 0xC2 && c <= 0xF4 && utf8_run(src[i]) > 0)
		{
			// valid UTF-8 (a name with accents): as it is
			new n = utf8_run(src[i])
			// (never half a character at the end)
			if (k + n + 1 >= len - 1)
				break
			for (new j = 0; j <= n; j++)
				out[k++] = src[i + j] & 0xFF
			i += n
		}
		else if (c < 32 || c >= 127)
		{
			static const HEX[] = "0123456789abcdef"
			out[k++] = '\'
			out[k++] = 'u'
			out[k++] = '0'
			out[k++] = '0'
			out[k++] = HEX[c >> 4]
			out[k++] = HEX[c & 15]
		}
		else
			out[k++] = c
	}
	out[k] = 0
}

// A UTF-8 lead byte's continuation bytes, if they're all there (0: not valid UTF-8)
utf8_run(const s[])
{
	new c = s[0] & 0xFF
	new n = c >= 0xF0 ? 3 : c >= 0xE0 ? 2 : 1
	for (new j = 1; j <= n; j++)
		if ((s[j] & 0xC0) != 0x80)
			return 0
	return n
}

// "key": "value",
json_field(out[], len, const key[], const value[], bool:comma = true)
{
	// (no fmt here: callers may pass a fmt() result as the value)
	add(out, len, "^"")
	add(out, len, key)
	add(out, len, "^": ^"")
	json_add(out, len, value)
	add(out, len, comma ? "^", " : "^"")
}

// Ask the model for `bot`'s answer to `human`. `written` is the line it says otherwise.
// True when the request is on its way.
bool:ai_ask(bot, human, const written[], const section[], bool:team, Float:extra, bool:thread)
{
	new host[48], port
	if (!ai_target(host, charsmax(host), port) || g_AiState[bot] || !is_user_connected(human))
		return false
	new err
	new sock = socket_open(host, port, SOCKET_TCP, err, SOCK_NON_BLOCKING)
	if (sock <= 0)
		return false

	static body[2800]
	new name[32]
	body = "{"
	g_AiAction[bot][0] = 0
	get_user_name(bot, name, charsmax(name))
	json_field(body, charsmax(body), "bot", name)
	// the letter for the ones the app knows, else a few words
	new p = g_Persona[bot]
	if (contain("ftsjqnockxbiduBDHLOS", fmt("%c", PERSONAS[p])) != -1)
		json_field(body, charsmax(body), "persona", fmt("%c", PERSONAS[p]))
	else
		json_field(body, charsmax(body), "persona", PERSONA_DESC[p])
	new mood[8]
	mood_of(bot, human, mood, charsmax(mood))
	json_field(body, charsmax(body), "mood", contain(mood, "-") != -1 ? "tilted" : contain(mood, "+") != -1 ? "cocky"
		: contain(mood, "^^") != -1 ? "polite" : "normal")
	json_field(body, charsmax(body), "kind", thread ? "thread" : "reply")
	json_field(body, charsmax(body), "message", g_HumanMsg)
	get_user_name(human, name, charsmax(name))
	json_field(body, charsmax(body), "human", name)
	// who the bot is, how it knows them, what just happened to either of them, and what it
	// may be asked to do
	new info[160]
	traits_text(bot, info, charsmax(info))
	if (info[0])
		json_field(body, charsmax(body), "traits", info)
	relation_text(bot, human, info, charsmax(info))
	json_field(body, charsmax(body), "relation", info)
	// "Bots can be rude" and the lobby's mood: with rude off the model teases at most
	add(body, charsmax(body), fmt("^"rude^": %s, ", rude_on() ? "true" : "false"))
	new lm = lobby_mood()
	json_field(body, charsmax(body), "lobby", lm == 2 ? "toxic" : lm == 0 ? "friendly" : "normal")
	add(body, charsmax(body), "^"events^": [")
	new Float:now = get_gametime(), ev[3], shown_ev
	for (new back = 1; back <= EVENTS && shown_ev < 3; back++)
	{
		new e = (g_EvPos - back + EVENTS * 2) % EVENTS
		if (!g_EvText[e][0] || now - g_EvAt[e] > 90.0)
			continue
		if (g_EvKiller[e] != bot && g_EvVictim[e] != bot && g_EvKiller[e] != human && g_EvVictim[e] != human)
			continue
		ev[shown_ev++] = e
	}
	// (oldest first)
	for (new k = shown_ev - 1; k >= 0; k--)
	{
		add(body, charsmax(body), k == shown_ev - 1 ? "^"" : ", ^"")
		json_add(body, charsmax(body), g_EvText[ev[k]])
		add(body, charsmax(body), "^"")
	}
	add(body, charsmax(body), "], ")
	if (ai_offers_actions(bot, human))
	{
		add(body, charsmax(body), "^"actions^": [^"follow^", ^"hold^", ^"come^"")
		if (g_SiteCount >= 2 || TrieKeyExists(g_PlaceAt, "b"))
			add(body, charsmax(body), ", ^"goto_a^", ^"goto_b^"")
		if (!match_like())
		{
			add(body, charsmax(body), ", ^"jump^", ^"crouch^"")
			if (!g_NoSprayNative && g_HumanBots)
				add(body, charsmax(body), ", ^"spray^"")
		}
		if (g_BuyMode && !g_NoDropNative && g_BotFun)
			add(body, charsmax(body), ", ^"drop^"")
		add(body, charsmax(body), "], ")
	}
	// the last chat lines the person could see, oldest first
	add(body, charsmax(body), "^"history^": [")
	new t = get_user_team(human), first = 1, shown
	// count back up to 8 visible lines
	new idx[8]
	for (new back = 0; back < g_HistCount && shown < 8; back++)
	{
		new i = (g_HistPos - 1 - back + HIST * 2) % HIST
		if (!g_HistTeam[i] || g_HistTeam[i] == t)
			idx[shown++] = i
	}
	for (new k = shown - 1; k >= 0; k--)
	{
		add(body, charsmax(body), first ? "^"" : ", ^"")
		json_add(body, charsmax(body), g_Hist[idx[k]])
		add(body, charsmax(body), "^"")
		first = 0
	}
	add(body, charsmax(body), "], ^"context^": {")
	// map, score and alive, then only what this answer is about (the app shows a few
	// extra keys, so nothing that doesn't matter here)
	new map[32], cmap[32]
	get_mapname(map, charsmax(map))
	casual_map(map, cmap, charsmax(cmap))
	json_field(body, charsmax(body), "map", cmap)
	new bt = get_user_team(bot)
	if (bt == 1 || bt == 2)
	{
		new us = g_Score[bt], them = g_Score[3 - bt]
		json_field(body, charsmax(body), "score", us > them ? fmt("%d %d us", us, them) : us < them ? fmt("%d %d them", them, us) : fmt("%d %d tied", us, them))
	}
	if (g_CtxPlace[0])
		json_field(body, charsmax(body), equal(section, "r_how") ? "killer_place" : equal(section, "r_bomb", 6) ? "bomb_place" : "enemy_place", g_CtxPlace)
	if (g_CtxWeapon[0])
		json_field(body, charsmax(body), "killer_weapon", g_CtxWeapon)
	if (g_CtxHs)
		add(body, charsmax(body), "^"headshot^": true, ")
	if (g_CtxCarrier[0])
		json_field(body, charsmax(body), "bomb_carrier", g_CtxCarrier)
	if (equal(section, "r_bomb_you"))
		add(body, charsmax(body), "^"human_has_bomb^": true, ")
	else if (equal(section, "r_bomb_me"))
		add(body, charsmax(body), "^"bot_has_bomb^": true, ")
	if (equal(section, "r_timeleft", 10) && get_timeleft() > 0)
		add(body, charsmax(body), fmt("^"timeleft_min^": %d, ", (get_timeleft() + 30) / 60))
	if (equal(section, "r_nextmap", 9))
	{
		new next[32]
		next_map(next, charsmax(next))
		if (next[0])
			json_field(body, charsmax(body), "nextmap", next)
	}
	if (contain(mood, "=") != -1)
		add(body, charsmax(body), "^"knows_human^": true, ")
	if (team)
		add(body, charsmax(body), "^"team_chat^": true, ")
	if (thread)
		json_field(body, charsmax(body), "topic", g_ThreadTopic[human])
	// the mode, the round, how many are left each side, and the bot's own health, gun, money
	if (g_ModeId[0])
		json_field(body, charsmax(body), "mode", g_ModeId)
	json_field(body, charsmax(body), "phase", !is_user_alive(bot) ? "dead" : g_RoundOver ? "round over" : g_Freeze ? "freeze time" : "live")
	if (!teams_off() && (bt == 1 || bt == 2))
	{
		new us, them
		for (new i = 1; i <= MaxClients; i++)
			if (is_user_alive(i))
			{
				if (get_user_team(i) == bt)
					us++
				else if (get_user_team(i) == 3 - bt)
					them++
			}
		add(body, charsmax(body), fmt("^"alive_us^": %d, ^"alive_them^": %d, ", us, them))
	}
	if (is_user_alive(bot))
	{
		new gun[24]
		gun_text(bot, gun, charsmax(gun))
		add(body, charsmax(body), fmt("^"hp^": %d, ", get_user_health(bot)))
		if (gun[0])
			json_field(body, charsmax(body), "gun", gun)
	}
	if (g_BuyMode)
		add(body, charsmax(body), fmt("^"money^": %d, ", cs_get_user_money(bot)))
	add(body, charsmax(body), fmt("^"alive^": %s", is_user_alive(bot) ? "true" : "false"))
	add(body, charsmax(body), "}}")

	formatex(g_AiReq[bot], charsmax(g_AiReq[]), "POST /api/botchat/reply HTTP/1.1^r^nHost: %s:%d^r^nContent-Type: application/json^r^nContent-Length: %d^r^nConnection: close^r^n^r^n%s",
		host, port, strlen(body), body)
	g_AiSock[bot] = sock
	g_AiState[bot] = 1
	g_AiSent[bot] = 0
	g_AiGot[bot] = 0
	g_AiResp[bot][0] = 0
	g_AiStart[bot] = get_gametime()
	g_AiUser[bot] = get_user_userid(bot)
	g_AiHuman[bot] = human
	g_AiHumanUser[bot] = get_user_userid(human)
	g_AiTeam[bot] = team
	g_AiExtra[bot] = extra
	g_AiRound[bot] = g_Round
	copy(g_AiFallback[bot], charsmax(g_AiFallback[]), written)
	copy(g_AiSection[bot], charsmax(g_AiSection[]), section)
	set_task(0.1, "task_ai_poll", TASK_AI + bot, _, _, "b")
	return true
}

// The answer no longer fits: the bot or the person left, or the round moved on
bool:ai_stale(bot)
{
	new human = g_AiHuman[bot]
	return !is_user_connected(bot) || get_user_userid(bot) != g_AiUser[bot] || !is_user_connected(human)
		|| get_user_userid(human) != g_AiHumanUser[bot] || g_Round != g_AiRound[bot]
}

ai_close(bot)
{
	remove_task(TASK_AI + bot)
	if (g_AiState[bot] && g_AiSock[bot] > 0)
		socket_close(g_AiSock[bot])
	g_AiSock[bot] = 0
	g_AiState[bot] = 0
}

// Done with a request: the model's text, or the written line, typed in the time that's left
ai_finish(bot, const said[])
{
	new bool:stale = ai_stale(bot)
	ai_close(bot)
	if (stale)
		return
	new text[128], bool:raw = said[0] != 0
	copy(text, charsmax(text), raw ? said : g_AiFallback[bot])
	// the model said to do something too: the same yes or no as asking in chat; if it won't
	// or can't after all, it says that instead of what the model wrote
	if (raw && g_AiAction[bot][0])
	{
		new sec[24]
		if (!ai_action(bot, g_AiHuman[bot], g_AiAction[bot], sec, charsmax(sec)))
		{
			text[0] = 0
			raw = false
			g_CtxOther = g_AiHuman[bot]
			if (sec[0])
				pick_line(bot, sec, text, charsmax(text), true)
			g_CtxOther = 0
			copy(g_AiSection[bot], charsmax(g_AiSection[]), sec)
		}
	}
	g_AiAction[bot][0] = 0
	if (!text[0])
		return
	// the time already spent waiting counts as reading it
	new Float:delay = g_AiExtra[bot] + typing_time(text, 1.2, 3.0) - (get_gametime() - g_AiStart[bot])
	queue(bot, g_AiHuman[bot], text, 1, g_AiTeam[bot], floatmax(delay, 0.3), g_AiSection[bot], false, raw)
}

public task_ai_poll(task)
{
	new bot = task - TASK_AI
	if (!g_AiState[bot])
	{
		remove_task(task)
		return
	}
	if (ai_stale(bot))
	{
		ai_finish(bot, "")
		return
	}
	new sock = g_AiSock[bot]
	if (get_gametime() - g_AiStart[bot] > AI_TIMEOUT)
	{
		ai_finish(bot, "")
		return
	}
	// connecting: send the request once the socket is ready
	if (g_AiState[bot] == 1)
	{
		if (!socket_is_writable(sock, 0))
			return
		new total = strlen(g_AiReq[bot])
		new n = socket_send(sock, g_AiReq[bot][g_AiSent[bot]], total - g_AiSent[bot])
		if (n < 0)
		{
			ai_finish(bot, "")
			return
		}
		g_AiSent[bot] += n
		if (g_AiSent[bot] >= total)
			g_AiState[bot] = 2
		return
	}
	// waiting: read what's there; the server closes the connection when it's done
	static chunk[512]
	while (socket_is_readable(sock, 0))
	{
		new n = socket_recv(sock, chunk, charsmax(chunk))
		if (n <= 0)
		{
			ai_parse(bot)
			return
		}
		chunk[min(n, charsmax(chunk))] = 0
		new room = charsmax(g_AiResp[]) - g_AiGot[bot]
		if (room <= 0)
		{
			ai_parse(bot)
			return
		}
		copy(g_AiResp[bot][g_AiGot[bot]], room, chunk)
		g_AiGot[bot] = strlen(g_AiResp[bot])
	}
	// everything the headers said is here (some servers keep the connection a moment)
	new body = contain(g_AiResp[bot], "^r^n^r^n")
	new cl = containi(g_AiResp[bot], "content-length:")
	if (body > 0 && cl > 0 && cl < body)
	{
		new want_len = str_to_num(g_AiResp[bot][cl + 15])
		if (g_AiGot[bot] - (body + 4) >= want_len)
			ai_parse(bot)
	}
}

// 200 with {"text": "..."}: the bot says it; anything else: the written line
ai_parse(bot)
{
	new resp[1024]
	copy(resp, charsmax(resp), g_AiResp[bot])
	new said[128]
	if (equal(resp, "HTTP/1.", 7) && str_to_num(resp[9]) == 200)
	{
		new body = contain(resp, "^r^n^r^n")
		new at = body > 0 ? contain(resp[body], "^"text^"") : -1
		if (at >= 0)
		{
			new i = body + at + 6
			while (resp[i] && resp[i] != '"')
				i++
			if (resp[i] == '"')
				json_string(resp[i + 1], said, charsmax(said))
		}
		// what to do too, if anything (a name from the list it was given, nothing else)
		new act = body > 0 ? contain(resp[body], "^"action^"") : -1
		if (act >= 0)
		{
			new i = body + act + 8
			while (resp[i] && resp[i] != '"')
				i++
			if (resp[i] == '"')
				json_string(resp[i + 1], g_AiAction[bot], charsmax(g_AiAction[]))
			for (new k = 0; g_AiAction[bot][k]; k++)
				if (!isalpha(g_AiAction[bot][k]) && g_AiAction[bot][k] != '_')
					g_AiAction[bot][0] = 0
		}
	}
	clean_ai(said)
	ai_finish(bot, said)
}

// A JSON string's text (after the opening quote), escapes undone; non-ascii dropped
json_string(const src[], out[], len)
{
	new k
	for (new i = 0; src[i] && src[i] != '"' && k < len; i++)
	{
		new c = src[i]
		if (c == '\' && src[i + 1])
		{
			new e = src[++i]
			switch (e)
			{
				case 'n', 'r', 't': c = ' '
				case 'u':
				{
					new v
					for (new h = 1; h <= 4 && src[i + h]; h++)
					{
						new d = src[i + h]
						v = v * 16 + (d >= '0' && d <= '9' ? d - '0' : d >= 'a' && d <= 'f' ? d - 'a' + 10 : d >= 'A' && d <= 'F' ? d - 'A' + 10 : 0)
					}
					i += 4
					c = v < 128 ? v : 0
				}
				default: c = e
			}
		}
		if (c >= 32 && c < 127)
			out[k++] = c
	}
	out[k] = 0
}

// What the model wrote, made safe to say: one line, no quotes or commands, not too long
clean_ai(text[])
{
	for (new i = 0; text[i]; i++)
		if (text[i] == '"')
			text[i] = 39
	trim(text)
	while (text[0] == '@' || text[0] == '/' || text[0] == '!' || text[0] == '.')
	{
		copy(text, 127, text[1])
		trim(text)
	}
	if (strlen(text) > 100)
	{
		text[100] = 0
		new sp = 100
		while (sp > 60 && text[sp] != ' ')
			sp--
		if (sp > 60)
			text[sp] = 0
	}
	trim(text)
}

// `word` as a whole word ("hi" in "hi all", not in "this"), or anywhere for "?"
bool:word_in(const msg[], const word[])
{
	if (word[0] == '?')
		return contain(msg, "?") != -1
	new len = strlen(word), from = 0, at
	while ((at = contain(msg[from], word)) != -1)
	{
		new s = from + at
		new before = s > 0 ? msg[s - 1] : ' ', after = msg[s + len]
		if (!isalnum(before) && !isalnum(after))
			return true
		from = s + 1
	}
	return false
}
