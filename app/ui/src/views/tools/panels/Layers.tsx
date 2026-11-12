import { useTools, type Layers as LayerSet } from "../../../stores/tools";
import { Info, Switch } from "../../../ui";

const ROWS: [keyof LayerSet, string, string][] = [
  ["textures", "Textures", "The map's own pictures on every surface. Off: a plain colour per texture, easier on the eyes when you're looking at shapes."],
  ["lighting", "Lighting", "The light and shadow baked into the map, as in the game. Off: everything evenly lit."],
  ["entities", "Spawns & items", "Spawn points (T orange, CT blue), lights, items, sounds and other markers."],
  ["moving", "Doors & breakables", "Doors, breakable glass and boxes, ladders, lifts and other moving parts."],
  ["triggers", "Triggers & zones", "Invisible areas: buy zones, bomb sites, hostage rescue, and triggers that hurt, teleport or start something."],
];

export function Layers() {
  const layers = useTools((s) => s.layers);
  const setLayer = useTools((s) => s.setLayer);
  return (
    <section className="tools-panel" aria-label="Layers">
      <h3>Show</h3>
      {ROWS.map(([k, label, info]) => (
        <div className="setting" key={k}>
          <span className="name">
            {label}
            <Info text={info} />
          </span>
          <Switch on={layers[k]} label={label} onChange={(v) => setLayer(k, v)} />
        </div>
      ))}
    </section>
  );
}
