// The two games, and how well the bots handle each mode: one place for every page.

export type GameId = "cstrike" | "czero";

/** "Counter-Strike 1.6" or "Condition Zero" */
export const gameTitle = (id?: string | null) => (id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero");

/** "CS 1.6" or "CZ", where space is tight */
export const gameShort = (id?: string | null) => (id === "cstrike" ? "CS 1.6" : "CZ");

export type BotRating = "great" | "ok" | "weak" | "none";

/** How well bots handle each mode (by mode id, without "mode-"); unlisted modes are "great". */
export const BOT_RATING: Record<string, BotRating> = {
  hns: "ok", deathrun: "ok", match: "great", surf: "ok", soccerjam: "great", basebuilder: "weak", kreedz: "none",
};

export const BOT_NOTE: Record<BotRating, string> = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };

/** What the bot rating badges mean (their "?" or hover text). */
export const BOT_RATING_INFO = "How well bots play each mode. Some modes are made for people, so bots are weak or missing there.";

/** Bot styles (gm_botstyle), in the order the in-game menu shows them. */
// Original first, then easiest to hardest (Mixed and LAN party are mixes of levels, about Pub to
// Matchmaking on average; Old-timers mostly Veteran level with a few a notch below)
export const BOT_STYLES: [string, string][] = [["0", "Original"], ["4", "Newbie lobby"], ["1", "Pub server"], ["8", "Mixed lobby"], ["9", "LAN party"], ["2", "Matchmaking"], ["10", "Old-timers"], ["3", "Veterans"], ["5", "Locked in"], ["6", "Hackers"], ["7", "Gods"]];

/** What each bot style is like, by gm_botstyle value. */
export const BOT_STYLE_NOTE: Record<string, string> = {
  "0": "The game's stock bots, unchanged: robotic and predictable",
  "4": "New players: slow to react, they spray and miss a lot",
  "1": "A typical public server: a few sharp players, most average, some bad",
  "8": "Everyone different, from new to very good. A regular keeps their level",
  "9": "Friends at a LAN: every level, lots of banter, and they play together",
  "2": "Solid players who work as a team",
  "3": "Experienced: quick, accurate, short bursts",
  "10": "Good aim, slow and methodical. They camp a lot and rarely talk",
  "5": "Very good and focused. They bunnyhop between fights",
  "6": "Suspiciously good: most bullets find you and they notice you from behind",
  "7": "Everything maxed. Every bullet near you is a headshot",
};
