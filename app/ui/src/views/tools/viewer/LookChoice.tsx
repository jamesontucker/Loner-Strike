// The Layers panel's "Look" row (under Lighting): as the game draws the map, or brighter.

import { Info, Segmented } from "../../../ui";
import { useLookMode } from "./look";

export function LookChoice() {
  const mode = useLookMode((s) => s.mode);
  const setMode = useLookMode((s) => s.setMode);
  return (
    <div className="setting">
      <span className="name">
        Look
        <Info text="Like the game: the light, colours, sky, water and see-through parts as Counter-Strike draws them, with your game's own brightness and gamma settings. Bright: the same, lifted, for looking into dark corners." />
      </span>
      <Segmented values="game bright" labels="Like the game|Bright" current={mode} onPick={(v) => setMode(v === "bright" ? "bright" : "game")} label="Look" />
    </div>
  );
}
