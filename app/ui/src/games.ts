// The two games, and how well the bots handle each mode: one place for every page.

export type GameId = "cstrike" | "czero";

/** "Counter-Strike 1.6" or "Condition Zero" */
export const gameTitle = (id?: string | null) => (id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero");

/** "CS 1.6" or "CZ", where space is tight */
export const gameShort = (id?: string | null) => (id === "cstrike" ? "CS 1.6" : "CZ");

export type BotRating = "great" | "ok" | "weak" | "none";

/** How well bots handle each mode (by mode id, without "mode-"); unlisted modes are "great". */
export const BOT_RATING: Record<string, BotRating> = {
  hns: "ok", deathrun: "ok", match: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};

export const BOT_NOTE: Record<BotRating, string> = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };
