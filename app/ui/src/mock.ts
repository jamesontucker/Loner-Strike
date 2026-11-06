// Sample data for the layout mockup. The real app reads the same shapes from /api/state.

export type Mode = { id: string; name: string; desc: string; bots: "great" | "ok" | "weak" | "none" };

export const MODES: Mode[] = [
  { id: "classic", name: "Classic", desc: "Bomb defusal and hostage rescue", bots: "great" },
  { id: "csdm", name: "Deathmatch", desc: "Teams, instant respawn, pick your guns", bots: "great" },
  { id: "csdm_ffa", name: "Free for all", desc: "Everyone for themselves", bots: "great" },
  { id: "gungame", name: "Gun Game", desc: "Kill to level through guns", bots: "great" },
  { id: "zp5", name: "Zombie Plague Advanced", desc: "Sniper, Assassin, Nemesis, Survivor", bots: "great" },
  { id: "war3", name: "Warcraft 3", desc: "Races, skills, items and ultimates", bots: "great" },
  { id: "shero", name: "SuperHero", desc: "Level up and pick superheroes", bots: "great" },
  { id: "paintball", name: "Paintball", desc: "One hit and you're out", bots: "great" },
  { id: "hns", name: "Hide and Seek", desc: "Ts hide, CTs seek with knives", bots: "ok" },
  { id: "deathrun", name: "Deathrun", desc: "Survive the trap course", bots: "ok" },
  { id: "surf", name: "Surf", desc: "Air control, long rounds", bots: "ok" },
  { id: "soccerjam", name: "SoccerJam", desc: "Knife football", bots: "weak" },
  { id: "basebuilder", name: "Base Builder", desc: "Build a base, survive the zombies", bots: "weak" },
  { id: "kreedz", name: "Kreedz", desc: "Solo climbing with a timer", bots: "none" },
];

export const BOT_NOTE: Record<Mode["bots"], string> = {
  great: "Bots play this well",
  ok: "Bots are so-so",
  weak: "Best with a friend",
  none: "Solo, no bots",
};

export const LIVE = {
  map: "de_dust2",
  mode: "csdm",
  bots: 14,
  humans: 1,
  maxplayers: 32,
  timeleft: 742,
  scoreT: 31,
  scoreCT: 27,
};

export const RECENT_MAPS = ["de_dust2", "cs_italy", "de_rats_2002", "aim_map", "de_shortdust", "cs_mini_militia"];

export const PLAYERS = [
  { name: "You", team: "CT", frags: 22, deaths: 9, bot: false },
  { name: "[NoX] Viper", team: "T", frags: 19, deaths: 12, bot: true },
  { name: "kevin from work", team: "CT", frags: 15, deaths: 14, bot: true },
  { name: "Lumen | aXe", team: "T", frags: 14, deaths: 16, bot: true },
  { name: "Dr. Pepperoni", team: "T", frags: 11, deaths: 17, bot: true },
  { name: "<<OGs>> Kilo", team: "CT", frags: 9, deaths: 18, bot: true },
];
