/*
	Round End Music (folder based, T win / CT win)

	Based on "REMusic" 2.3.1 by OciXCrom (https://github.com/OciXCrom/REMusic).
	Changes for this server:
	  - no config list needed: at every map load the plugin scans
	        sound/roundsound/t_win/    -> played when Terrorists win
	        sound/roundsound/ct_win/   -> played when Counter-Terrorists win
	        sound/roundsound/          -> (files directly in it) played for either team
	    and uses every .mp3 / .wav it finds (czero/ and cstrike/ are both searched).
	  - per-team tracks, chance per round, optional draw music, optional radio blocking
	  - say /music (or /ers) toggles music for yourself (saved)

	mp3 files are played with "mp3 play sound/...", wav files with "spk".
	wav must be 8/16-bit PCM mono, 11025 or 22050 Hz (GoldSrc limit). mp3: any normal CBR mp3.

	Cvars:
	  roundmusic_enabled      1    0 = off
	  roundmusic_chance       100  % of rounds that get music
	  roundmusic_draw         0    1 = also play (shared folder) on a round draw
	  roundmusic_block_radio  0    1 = mute the "Terrorists Win" / "Counter-Terrorists Win" radio line
	  roundmusic_stop_newround 0   1 = cut the track when the next round starts
*/

#include <amxmodx>
#include <amxmisc>
#include <nvault>

#define PLUGIN_VERSION "2.3.1-cz"

#define MAX_PER_FOLDER 16     // precache cap per folder (sound precache limit is 512 per map)
#define PATH_LEN 96

new const g_szPrefix[] = "^4[Music]^1"
new const g_szBase[] = "roundsound"

enum { POOL_T = 0, POOL_CT, POOL_ANY, POOL_COUNT }
new const g_szSub[POOL_COUNT][] = { "t_win", "ct_win", "" }

new Array:g_aPool[POOL_COUNT]
new g_iPoolSize[POOL_COUNT]
new g_iLast[POOL_COUNT] = { -1, ... }

new g_pEnabled, g_pChance, g_pDraw, g_pBlockRadio, g_pStopNew
new g_iVault
new bool:g_bBlocked[33]

public plugin_init()
{
	register_plugin("Round End Music", PLUGIN_VERSION, "OciXCrom / folder patch")
	register_cvar("CRXReMusic", PLUGIN_VERSION, FCVAR_SERVER|FCVAR_SPONLY|FCVAR_UNLOGGED)

	register_event("SendAudio", "OnTWin", "a", "2=%!MRAD_terwin")
	register_event("SendAudio", "OnCTWin", "a", "2=%!MRAD_ctwin")
	register_event("SendAudio", "OnDraw", "a", "2=%!MRAD_rounddraw")
	register_message(get_user_msgid("SendAudio"), "msgSendAudio")
	register_logevent("OnRoundStart", 2, "1=Round_Start")

	register_clcmd("say /music", "cmdMute")
	register_clcmd("say_team /music", "cmdMute")
	register_clcmd("say /ers", "cmdMute")
	register_clcmd("say_team /ers", "cmdMute")

	g_pEnabled    = register_cvar("roundmusic_enabled", "1")
	g_pChance     = register_cvar("roundmusic_chance", "100")
	g_pDraw       = register_cvar("roundmusic_draw", "0")
	g_pBlockRadio = register_cvar("roundmusic_block_radio", "0")
	g_pStopNew    = register_cvar("roundmusic_stop_newround", "0")

	g_iVault = nvault_open("roundmusic")
}

public plugin_precache()
{
	new Trie:tSeen = TrieCreate()
	new szDir[PATH_LEN]

	for (new p = 0; p < POOL_COUNT; p++)
	{
		g_aPool[p] = ArrayCreate(PATH_LEN, 4)

		if (g_szSub[p][0])
			formatex(szDir, charsmax(szDir), "%s/%s", g_szBase, g_szSub[p])
		else
			copy(szDir, charsmax(szDir), g_szBase)

		ScanFolder(p, szDir, tSeen)
	}

	TrieDestroy(tSeen)
	log_amx("[roundmusic] tracks: T win %d, CT win %d, shared %d", g_iPoolSize[POOL_T], g_iPoolSize[POOL_CT], g_iPoolSize[POOL_ANY])
}

ScanFolder(p, const szDir[], Trie:tSeen)
{
	// collect candidates first so a random subset can be picked when there are too many
	new Array:aFound = ArrayCreate(PATH_LEN, 8)
	new szSearch[PATH_LEN + 8], szFile[64], szRel[PATH_LEN]
	new FileType:iType

	formatex(szSearch, charsmax(szSearch), "sound/%s", szDir)

	// Valve FS sees czero/ and its fallback cstrike/; fall back to the plain game dir
	new hDir = open_dir(szSearch, szFile, charsmax(szFile), iType, true, NULL_STRING)
	if (!hDir)
		hDir = open_dir(szSearch, szFile, charsmax(szFile), iType)

	if (hDir)
	{
		do
		{
			if (iType == FileType_Directory || szFile[0] == '.')
				continue
			if (!get_sound_type(szFile) || contain(szFile, " ") != -1)
				continue

			formatex(szRel, charsmax(szRel), "%s/%s", szDir, szFile)
			strtolower(szRel)

			if (TrieKeyExists(tSeen, szRel))
				continue
			TrieSetCell(tSeen, szRel, 1)
			ArrayPushString(aFound, szRel)
		}
		while (next_file(hDir, szFile, charsmax(szFile), iType))

		close_dir(hDir)
	}

	new iCount = ArraySize(aFound)
	while (iCount > MAX_PER_FOLDER)
	{
		ArrayDeleteItem(aFound, random(iCount))
		iCount--
	}

	new szPrecache[PATH_LEN + 8]
	for (new i = 0; i < iCount; i++)
	{
		ArrayGetString(aFound, i, szRel, charsmax(szRel))

		if (get_sound_type(szRel) == 1)
			precache_sound(szRel)
		else
		{
			formatex(szPrecache, charsmax(szPrecache), "sound/%s", szRel)
			precache_generic(szPrecache)
		}

		ArrayPushString(g_aPool[p], szRel)
		g_iPoolSize[p]++
	}

	ArrayDestroy(aFound)
}

public plugin_end()
{
	for (new p = 0; p < POOL_COUNT; p++)
		if (g_aPool[p] != Invalid_Array)
			ArrayDestroy(g_aPool[p])

	if (g_iVault != INVALID_HANDLE)
		nvault_close(g_iVault)
}

public client_putinserver(id)
{
	g_bBlocked[id] = false
	UseVault(id, false)
}

#if AMXX_VERSION_NUM >= 190
public client_disconnected(id)
#else
public client_disconnect(id)
#endif
{
	if (!is_user_bot(id))
		UseVault(id, true)
}

UseVault(id, bool:bWrite)
{
	if (g_iVault == INVALID_HANDLE || is_user_bot(id))
		return

	new szKey[40], szData[4]
	get_user_authid(id, szKey, charsmax(szKey))
	if (!szKey[0] || equal(szKey, "STEAM_ID_PENDING") || equal(szKey, "BOT"))
		return

	if (bWrite)
	{
		num_to_str(_:g_bBlocked[id], szData, charsmax(szData))
		nvault_set(g_iVault, szKey, szData)
	}
	else if (nvault_get(g_iVault, szKey, szData, charsmax(szData)))
		g_bBlocked[id] = str_to_num(szData) == 1
}

public msgSendAudio(msgid, dest, id)
{
	if (!get_pcvar_num(g_pEnabled) || !get_pcvar_num(g_pBlockRadio))
		return PLUGIN_CONTINUE

	new szAudio[24]
	get_msg_arg_string(2, szAudio, charsmax(szAudio))

	if (equal(szAudio, "%!MRAD_terwin") || equal(szAudio, "%!MRAD_ctwin"))
		return PLUGIN_HANDLED

	return PLUGIN_CONTINUE
}

public OnTWin()  PlayFor(POOL_T)
public OnCTWin() PlayFor(POOL_CT)

public OnDraw()
{
	if (get_pcvar_num(g_pDraw))
		PlayFor(POOL_ANY)
}

public OnRoundStart()
{
	if (!get_pcvar_num(g_pEnabled) || !get_pcvar_num(g_pStopNew))
		return

	new iPlayers[32], iNum
	get_players(iPlayers, iNum, "ch")
	for (new i = 0; i < iNum; i++)
		if (!g_bBlocked[iPlayers[i]])
			client_cmd(iPlayers[i], "mp3 stop")
}

PlayFor(iPool)
{
	if (!get_pcvar_num(g_pEnabled))
		return

	// SendAudio is sent once per receiving player; play only once per round end
	static Float:flLast
	new Float:flNow = get_gametime()
	if (flNow - flLast < 2.0 && flLast > 0.0)
		return
	flLast = flNow

	if (random_num(1, 100) > clamp(get_pcvar_num(g_pChance), 0, 100))
		return

	// team pool + shared pool form one list
	new iTeamSize = (iPool == POOL_ANY) ? 0 : g_iPoolSize[iPool]
	new iTotal = iTeamSize + g_iPoolSize[POOL_ANY]
	if (!iTotal)
		return

	new iPick = random(iTotal)
	if (iTotal > 1 && iPick == g_iLast[iPool])   // avoid the same track twice in a row
		iPick = (iPick + 1) % iTotal
	g_iLast[iPool] = iPick

	new szSound[PATH_LEN]
	if (iPick < iTeamSize)
		ArrayGetString(g_aPool[iPool], iPick, szSound, charsmax(szSound))
	else
		ArrayGetString(Array:g_aPool[POOL_ANY], iPick - iTeamSize, szSound, charsmax(szSound))

	new bool:bWav = (get_sound_type(szSound) == 1) ? true : false
	new iPlayers[32], iNum
	get_players(iPlayers, iNum, "ch")

	for (new i = 0; i < iNum; i++)
	{
		if (g_bBlocked[iPlayers[i]])
			continue

		if (bWav)
			client_cmd(iPlayers[i], "spk ^"%s^"", szSound)
		else
			client_cmd(iPlayers[i], "mp3 play ^"sound/%s^"", szSound)
	}
}

public cmdMute(id)
{
	g_bBlocked[id] = !g_bBlocked[id]
	if (g_bBlocked[id])
		client_cmd(id, "mp3 stop")
	client_print_color(id, print_team_default, "%s Round end music is now %s^1. Type ^4/music^1 to toggle.", g_szPrefix, g_bBlocked[id] ? "^3OFF" : "^3ON")
	return PLUGIN_HANDLED
}

// 1 = wav, 2 = mp3, 0 = other
get_sound_type(const szSound[])
{
	new iLen = strlen(szSound)
	if (iLen < 5)
		return 0
	if (equali(szSound[iLen - 4], ".wav"))
		return 1
	if (equali(szSound[iLen - 4], ".mp3"))
		return 2
	return 0
}
