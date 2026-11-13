// P5: keys of several entities at once. Every key any of them has; a value they share is
// shown, different values show blank ("different"). Changing a value sets it on all of them
// in one undo step; × removes the key from all; a new key goes on all.

import { useState } from "react";
import { Info } from "../../../ui";
import { useFgd } from "./fgd";
import { cmd2 } from "./p5store";
import { useEditor } from "./store";

const HIDE = new Set(["origin"]);

export function MultiEntityKeys() {
  const sel = useEditor((s) => s.sel.entities);
  const doc = useEditor((s) => s.doc);
  const fgd = useFgd();
  const [newKey, setNewKey] = useState("");
  const [newVal, setNewVal] = useState("");
  const ents = (doc?.entities ?? []).filter((e) => sel.includes(e.id));
  if (ents.length < 2) return null;
  const keys: string[] = [];
  for (const e of ents) for (const [k] of e.keys) if (!keys.includes(k) && !HIDE.has(k)) keys.push(k);
  const valueOf = (k: string): { same: boolean; value: string; have: number } => {
    const vals = ents.map((e) => e.keys.find(([a]) => a === k)?.[1]);
    const have = vals.filter((v) => v != null).length;
    const first = vals[0];
    const same = vals.every((v) => v === first);
    return { same, value: same ? first ?? "" : "", have };
  };
  const classes = [...new Set(ents.map((e) => e.classname))];
  const oneClass = classes.length === 1 ? fgd?.get(classes[0]) : undefined;
  const set = (key: string, value: string) => void cmd2({ op: "set_keys", entities: ents.map((e) => e.id), key, value });

  return (
    <div className="ed-multikeys">
      <h4>
        Keys of {ents.length} entities{" "}
        <Info text="A value they all share is shown. Blank means they differ: typing a value sets it on all of them. × takes the key off all of them. One undo step each time." />
      </h4>
      <p className="small-note muted">{classes.length === 1 ? classes[0] : `${classes.length} kinds: ${classes.slice(0, 4).join(", ")}${classes.length > 4 ? "…" : ""}`}</p>
      <div className="ed-multikeys-grid">
        {keys.map((k) => {
          const v = valueOf(k);
          const help = oneClass?.keys.find((x) => x.name === k);
          return (
            <div key={k} className="ed-multikeys-row">
              <span className="ed-multikeys-key" title={help?.help ?? k}>
                {help?.label && help.label !== k ? help.label : k}
                {v.have < ents.length && <span className="muted small-note"> ({v.have}/{ents.length})</span>}
              </span>
              <input
                key={`${k}=${v.value}=${v.same}`}
                defaultValue={v.value}
                placeholder={v.same ? "" : "different"}
                aria-label={k}
                spellCheck={false}
                onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                onBlur={(e) => {
                  const nv = e.target.value;
                  if (v.same ? nv !== v.value : nv !== "") set(k, nv);
                }}
              />
              <button type="button" className="btn small icon" aria-label={`Remove ${k} from all`} title="Remove from all" disabled={k === "classname"} onClick={() => set(k, "")}>
                ×
              </button>
            </div>
          );
        })}
      </div>
      <div className="ed-multikeys-row">
        <input value={newKey} onChange={(e) => setNewKey(e.target.value)} placeholder="New key" aria-label="New key" spellCheck={false} list="ed-multikeys-keys" />
        <datalist id="ed-multikeys-keys">{oneClass?.keys.map((x) => <option key={x.name} value={x.name}>{x.label}</option>)}</datalist>
        <input value={newVal} onChange={(e) => setNewVal(e.target.value)} placeholder="Value" aria-label="Value" spellCheck={false} />
        <button
          type="button"
          className="btn small"
          disabled={!newKey.trim() || !newVal}
          onClick={() => {
            set(newKey.trim(), newVal);
            setNewKey("");
            setNewVal("");
          }}
        >
          Add to all
        </button>
      </div>
    </div>
  );
}
