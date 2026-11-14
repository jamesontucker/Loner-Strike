// The example modes: small, commented graphs that teach the Studio and ship as the
// community's seed modes. Made by app/src-tauri/src/tools/studio/make_examples.py.

import infectionRaw from "./templates/infection.graph.json?raw";
import gunGameRaw from "./templates/gun-game-lite.graph.json?raw";
import hillRaw from "./templates/king-of-the-hill.graph.json?raw";
import chamberRaw from "./templates/one-in-the-chamber.graph.json?raw";
import juggernautRaw from "./templates/juggernaut.graph.json?raw";
import questRaw from "./templates/quest-giver.graph.json?raw";

export type Example = { id: string; label: string; blurb: string; raw: string };

export const EXAMPLES: Example[] = [
  { id: "infection", label: "Infection", blurb: "One zombie; whoever it kills turns. Teams, a round clock and a Function.", raw: infectionRaw },
  { id: "gungame", label: "Gun Game Lite", blurb: "Each kill gives the next gun; a knife kill wins. Per-player variables and a data table.", raw: gunGameRaw },
  { id: "hill", label: "King of the Hill", blurb: "Hold the hill to score. A capture point, a map marker, a timer and team scores.", raw: hillRaw },
  { id: "chamber", label: "One in the Chamber", blurb: "One bullet, a kill gives one more, three lives. Ammo, damage and lives.", raw: chamberRaw },
  { id: "juggernaut", label: "Juggernaut", blurb: "Kill the big one to become it. Player looks, a bonus, and bots told to hunt.", raw: juggernautRaw },
  { id: "quest", label: "The Quest Giver", blurb: "An NPC asks you to find his note; bring it and he pays. NPCs, a conversation and an interactable.", raw: questRaw },
];
