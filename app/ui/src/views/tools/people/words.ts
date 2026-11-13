// Plain words for gm_people's traits, values and play styles (plugins/scripting/gm_people.sma,
// docs/bot-people.md).

export const TRAIT_WORDS: Record<string, { label: string; info: string }> = {
  warmth: { label: "Warmth", info: "How friendly they are: a warm one says gg and nice shot, a cold one doesn't bother." },
  ego: { label: "Ego", info: "A show-off brags about kills and their aim; a humble one plays it down." },
  temper: { label: "Temper", info: "How fast they get salty when things go wrong: a tilty one rages, a chill one shrugs it off." },
  talk: { label: "Talk", info: "How much they chat. Most real players say little, so most bots do too." },
  humor: { label: "Humor", info: "What kind of jokes they make, if any: dry, goofy, or trolling people." },
  age: { label: "Age", info: "Kids and teens type and joke like it; old-timers remember 1.3 and talk about the old days." },
  skill: { label: "Skill", info: "How good they are and talk like they are. With bot aim styles on, the skill follows the bot difficulty." },
  role: { label: "Role", info: "Their place on the server: a regular, someone passing through, an admin wannabe, a clan member, or someone often away from keyboard." },
  region: { label: "From", info: "Where they're from: their English shows it, and now and then a word of their own." },
  style: { label: "Plays like", info: "How the bot plays: where it goes, what it buys, how it fights." },
};

export const VALUE_WORDS: Record<string, string> = {
  cold: "Cold", neutral: "Neutral", warm: "Warm",
  humble: "Humble", normal: "Normal", showoff: "Show-off",
  chill: "Chill", tilty: "Tilty",
  silent: "Silent", quiet: "Quiet", chatty: "Chatty", spammer: "Spammer",
  none: "None", dry: "Dry", goofy: "Goofy", troll: "Troll",
  kid: "Kid", teen: "Teen", adult: "Adult", old: "Old-timer",
  newbie: "Newbie", average: "Average", good: "Good", pro: "Ex-clan pro",
  regular: "Regular", drifter: "Passing through", admin: "Admin wannabe", clan: "Clan member", afk: "Often away",
  us: "USA", uk: "UK", de: "Germany", se: "Sweden", pl: "Poland", ru: "Russia", br: "Brazil", fr: "France", nl: "Netherlands", es: "Spain", au: "Australia", fi: "Finland", dk: "Denmark",
  rusher: "Rusher", roamer: "Roamer", lurker: "Lurker", careful: "Careful", camper: "Camper", awper: "AWPer", knifer: "Knifer", nader: "Nade thrower", ninja: "Ninja", objective: "Objective player", weirdgun: "Odd guns",
  wc3: "Warcraft III", wow: "World of Warcraft", halo: "Halo", school: "school", work: "work", cars: "cars", music: "music", lan: "LAN parties", cz: "Condition Zero", source: "CS: Source", awp: "the AWP", sleep: "sleep", food: "food", girlfriend: "his girlfriend", footy: "football", metal: "metal",
};

export const STYLE_NOTES: Record<string, string> = {
  rusher: "First out of spawn, straight at the enemy",
  roamer: "Moves around the map, never in one place long",
  lurker: "Sneaks around alone, catches people from behind",
  careful: "Slow and steady, checks corners",
  camper: "Holds one spot and waits",
  awper: "Always wants the AWP",
  knifer: "Loves a knife kill",
  nader: "Throws lots of grenades",
  ninja: "Sneaky and quiet, goes for the bomb",
  objective: "Plays the bomb and the hostages",
  weirdgun: "Plays odd guns: shotguns, the TMP",
};

export const word = (v: string) => VALUE_WORDS[v] ?? v;

/** "just_plays" -> "Just plays" */
export const archLabel = (n: string) => {
  const s = n.replace(/_/g, " ").trim();
  return s ? s[0].toUpperCase() + s.slice(1) : "(no name)";
};

/** "; ---- jokers and trolls" -> "Jokers and trolls" */
export const groupLabel = (g: string) => {
  const s = g.replace(/^;\s*/, "").replace(/^-+\s*/, "").replace(/\s*-+$/, "").trim();
  return s ? s[0].toUpperCase() + s.slice(1) : "No heading";
};
