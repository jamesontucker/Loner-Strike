// Plain words for Creature Studio's controls (docs/CREATURES.md says what each does in the game).

export const CATEGORY_WORDS: Record<string, string> = {
  infected: "Infected",
  alien: "Alien",
  wildlife: "Wildlife",
  mechanical: "Machine",
  boss: "Boss",
  special: "Special",
};

export const BEHAVIOUR_WORDS: Record<string, { label: string; help: string }> = {
  idle: { label: "Stand still", help: "Stays where it was put until it notices someone." },
  wander: { label: "Wander", help: "Walks around its home spot (within its home range) until it notices someone." },
  patrol: { label: "Patrol", help: "Walks a route of path_corner points placed in the map; without one it wanders its home range." },
  guard: { label: "Guard", help: "Holds its spot and only fights near it. Good for turrets and territorial animals." },
  chase: { label: "Hunt (horde)", help: "Always knows where the nearest enemy is and goes for them. What wave modes use." },
  flee: { label: "Keep away", help: "Runs from people. Comes back after a while." },
};

export const TARGET_WORDS: Record<string, { label: string; help: string }> = {
  nearest: { label: "Nearest", help: "Goes for whoever is closest." },
  weakest: { label: "Weakest", help: "Goes for whoever has the least health." },
  attacker: { label: "Last attacker", help: "Goes for whoever hurt it last." },
  random: { label: "Random", help: "Picks someone it can see at random." },
};

export const HATES_WORDS: Record<string, string> = { all: "Everyone", t: "Terrorists", ct: "Counter-Terrorists" };
export const BLOOD_WORDS: Record<string, string> = { red: "Red", yellow: "Yellow", none: "None" };
export const ATTACK_WORDS: Record<string, string> = { none: "None", melee: "Melee", ranged: "Ranged" };

export const SOUND_WORDS = [
  { label: "Idle", help: "Now and then while it has nothing to do." },
  { label: "Notices someone", help: "When it spots an enemy." },
  { label: "Attacks", help: "As it starts an attack." },
  { label: "Hits", help: "When an attack lands." },
  { label: "Misses", help: "When an attack misses." },
  { label: "Hurt", help: "When it's hurt." },
  { label: "Dies", help: "When it dies." },
];

export const ANIM_WORDS = ["Idle", "Walk", "Run", "Attack", "Flinch", "Die"];

export function aggressionWords(a: number): string {
  if (a <= 0) return "Never starts a fight (fights back, or runs if it can't)";
  if (a < 0.5) return "Territorial: only attacks who comes close";
  if (a < 0.9) return "Attacks most people it notices";
  return "Attacks anyone it notices";
}

/** Where a model or sound comes from, in words for a badge. */
export const KIND_WORDS: Record<string, string> = {
  game: "Everyone has it",
  hl: "Half-Life's own",
  custom: "Players download it",
  missing: "Not in your game",
  bad: "Not a model path",
};

export const TEMPLATE_WORDS: Record<string, string> = {
  zombie: "Zombie: average, slow, wanders",
  runner: "Runner: fast, fragile, hunts",
  brute: "Brute: tough, slow, hits hard",
  spitter: "Spitter: shoots from afar, runs when hurt",
  hound: "Hound: guards its patch",
  turret: "Turret: doesn't move, shoots",
  boss: "Boss: huge health, hits very hard",
};
