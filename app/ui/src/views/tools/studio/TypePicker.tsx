// Picks a type, as Blueprints' variable type menu does: one value, an array of them, or a map
// from text, numbers or players to them; the item a plain type, an enum or a struct of the mode.

import { container } from "./registry";
import { BASE_TYPES, KEY_TYPES, typeLabel } from "./pins";
import type { Graph } from "./schema";

const BASE_NAMES: Record<string, string> = { number: "Number", bool: "Yes/No", text: "Text", player: "Player", entity: "Entity", team: "Team", vector: "Vector", timer: "Timer" };

export function TypePicker({
  graph,
  value,
  onChange,
  exec = false,
  containers = true,
  structs = true,
  label = "Type",
}: {
  graph: Graph;
  value: string;
  onChange: (t: string) => void;
  /** a Macro's pin can be a flow pin */
  exec?: boolean;
  /** arrays and maps (not for a struct's fields) */
  containers?: boolean;
  /** structs as items (not for a struct's fields) */
  structs?: boolean;
  label?: string;
}) {
  const c = container(value);
  const shape = value === "exec" ? "exec" : c?.kind ?? "single";
  const item = value === "exec" ? "number" : c?.item ?? value;
  const key = c?.key ?? "text";
  const make = (s: string, it: string, k: string) => (s === "exec" ? "exec" : s === "array" ? `array<${it}>` : s === "map" ? `map<${k},${it.startsWith("struct:") ? "number" : it}>` : it);
  const items: [string, string][] = [
    ...BASE_TYPES.map((t): [string, string] => [t, BASE_NAMES[t]]),
    ...(graph.enums ?? []).map((e): [string, string] => [`enum:${e.id}`, `${e.name} (enum)`]),
    ...(structs && shape !== "map" ? (graph.structs ?? []).map((s): [string, string] => [`struct:${s.id}`, `${s.name} (struct)`]) : []),
  ];
  return (
    <span className="st-typepick">
      <select className="st-field" aria-label={`${label}: one or many`} value={shape} onChange={(e) => onChange(make(e.target.value, item, key))}>
        {exec && <option value="exec">Flow</option>}
        <option value="single">One</option>
        {containers && <option value="array">Array of</option>}
        {containers && <option value="map">Map from</option>}
      </select>
      {shape === "map" && (
        <>
          <select className="st-field" aria-label={`${label}: map keys`} value={key} onChange={(e) => onChange(make("map", item, e.target.value))}>
            {KEY_TYPES.map((k) => (
              <option key={k} value={k}>
                {BASE_NAMES[k]}
              </option>
            ))}
          </select>
          <span className="st-muted">to</span>
        </>
      )}
      {shape !== "exec" && (
        <select className="st-field" aria-label={label} value={item} onChange={(e) => onChange(make(shape, e.target.value, key))}>
          {items.map(([k, l]) => (
            <option key={k} value={k}>
              {l}
            </option>
          ))}
          {!items.some(([k]) => k === item) && <option value={item}>{typeLabel(graph, item)} (gone)</option>}
        </select>
      )}
    </span>
  );
}
