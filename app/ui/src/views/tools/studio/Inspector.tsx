// The side panel: the selected node's settings and help (or the mode's name and the problem
// list when nothing is selected).

import { ColorField } from "./ColorField";
import { useStudio } from "./store";
import { ENT_NAME_CHARS, KEYS_CHARS, KVPATH_CHARS, NAME_CHARS, PATH_CHARS, nodeDef, type ParamDef } from "./registry";
import { freshMember, mapName, removeGraphPart, updateCustomEvent, removeMarker, updateMarker, removeType, setParam, clearParam, toggleBreakpoint, updateEnum, updateFunction, updateMacro, updateStruct, updateVariable } from "./graphOps";
import { allEnums, isAssetType, LABEL_CHARS, type GMarker, type GMarkerKey, type GMember, type GNode, type GVariable, type Graph } from "./schema";
import { allVariables, itemTypeOk, nodeTitle, orphanValues, typeLabel } from "./pins";
import { TypePicker } from "./TypePicker";
import { Requirements } from "./Requirements";
import { insertItem, newerInLibrary } from "./library";
import { useConfirm } from "../../../ui";
import { askDelete } from "./DeleteDialog";
import { useFreshFocus } from "./freshFocus";
import { CreaturePicker } from "../common/CreaturePicker"; // shared creatures (CP-ui)
import { AssetField, assetKindOf } from "../common/AssetField"; // T1fix: asset pickers
import { AssetDefaultField } from "./AssetDefaultField"; // assets (X-fx)
import { ModelPointPicker } from "./ModelPointPicker"; // Y: Get Bone / Attachment Position

/** Node params that name a creature type (tools/creatures.rs NODE_TYPE_PARAMS). */
const CREATURE_PARAMS = ["creature.spawn:kind", "turret.setup:type", "creature.for_each:type", "creature.of_type:type", "creature.nearest:type"];

type PV = string | number | boolean;

function NumberField({ value, min, max, onChange }: { value: number; min?: number; max?: number; onChange: (v: number) => void }) {
  return (
    <input
      className="st-field st-field-num"
      type="number"
      min={min}
      max={max}
      step="any"
      value={Number(value)}
      onChange={(e) => {
        const n = Number(e.target.value);
        if (Number.isFinite(n)) onChange(Math.min(max ?? 100000, Math.max(min ?? -100000, n)));
      }}
    />
  );
}

/** A text box that only takes the characters `allowed` lets through (the app checks the same). */
function TextField({ value, max, allowed, onChange, fresh }: { value: string; max: number; allowed: RegExp; onChange: (v: string) => void; fresh?: string }) {
  // (fresh: the id of what it names; just added, it takes the focus)
  const ref = useFreshFocus<HTMLInputElement>(fresh);
  return (
    <input
      ref={ref}
      className="st-field"
      value={value}
      maxLength={max}
      onChange={(e) => {
        const v = e.target.value;
        if (allowed.test(v) && v.length <= max) onChange(v);
      }}
    />
  );
}

/** A value for a variable of this type. */
function ValueField({ type, value, onChange, graph }: { type: GVariable["type"]; value: PV | undefined; onChange: (v: PV) => void; graph?: Graph }) {
  if (type === "number") return <NumberField value={typeof value === "number" ? value : 0} onChange={onChange} />;
  if (type.startsWith("enum:")) {
    const e = allEnums(graph).find((x) => `enum:${x.id}` === type);
    return (
      <select className="st-field" value={String(typeof value === "number" ? value : 0)} onChange={(ev) => onChange(Number(ev.target.value))}>
        {(e?.values ?? []).map((v, i) => (
          <option key={i} value={i}>
            {v}
          </option>
        ))}
      </select>
    );
  }
  // assets (X-fx): a file variable starts as the file picked (or none)
  if (isAssetType(type)) return <AssetDefaultField kind={type} value={typeof value === "string" ? value : ""} onChange={onChange} />;
  if (type === "color") return <ColorField value={value} onChange={onChange} label="Start value" />;
  if (type !== "bool" && type !== "text") return <em className="st-muted">{/^(array|map)</.test(type) ? "Starts empty" : type === "vector" ? "Starts at 0 0 0" : type.startsWith("struct:") ? "Starts with every field empty" : "Starts as nothing"}</em>;
  if (type === "bool")
    return (
      <select className="st-field" value={value === true ? "1" : "0"} onChange={(e) => onChange(e.target.value === "1")}>
        <option value="1">Yes</option>
        <option value="0">No</option>
      </select>
    );
  return <TextField value={typeof value === "string" ? value : ""} max={60} allowed={LABEL_CHARS} onChange={onChange} />;
}

function ParamField({ p, value, vars, nodeVar, nodes, onChange, graph, node }: { p: ParamDef; value: PV; vars: GVariable[]; nodeVar?: GVariable; nodes: GNode[]; onChange: (v: PV) => void; graph: Graph; node: GNode }) {
  const pick = (list: [string, string][], empty: string) => (
    <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
      {!list.some(([k]) => k === String(value)) && <option value={String(value)}>{list.length ? "Pick one…" : empty}</option>}
      {list.map(([k, l]) => (
        <option key={k} value={k}>
          {l}
        </option>
      ))}
    </select>
  );
  if (p.kind === "fn") return pick((graph.functions ?? []).filter((f) => !!f.pure === (node.type === "fn.call_pure") && f.id !== node.graph).map((f) => [f.id, f.name]), "Add a Function under My mode first");
  if (p.kind === "table") return pick((graph.dataTables ?? []).map((t) => [t.id, t.name]), "Add a data table under My mode first");
  if (p.kind === "column") {
    const t = graph.dataTables?.find((x) => x.id === node.params.table);
    return t ? pick(t.columns.map((c) => [c.id, c.name]), "This table has no columns") : <em className="st-muted">Pick a table first</em>;
  }
  if (p.kind === "tline") return pick((graph.timelines ?? []).map((t) => [t.id, t.name]), "Add a Timeline under My mode first");
  if (p.kind === "cevent") return pick((graph.customEvents ?? []).filter((c) => node.type !== "timer.by_event" || c.inputs.length === 0).map((c) => [c.id, c.name]), node.type === "timer.by_event" ? "Add a Custom Event with no inputs under My mode first" : "Add a Custom Event under My mode first");
  if (p.kind === "macro") return pick((graph.macros ?? []).filter((m) => m.id !== node.graph).map((m) => [m.id, m.name]), "Add a Macro under My mode first");
  if (p.kind === "struct") return pick((graph.structs ?? []).map((x) => [x.id, x.name]), "Add a struct under My mode first");
  if (p.kind === "enum") return pick(allEnums(graph).map((x) => [x.id, x.name]), "Add an enum under My mode first");
  if (p.kind === "marker") return pick((graph.markers ?? []).map((x) => [x.id, x.label]), "Add a map marker under My mode first");
  if (p.kind === "type") return <TypePicker graph={graph} value={itemTypeOk(graph, String(value), true) ? String(value) : "number"} onChange={onChange} containers={false} structs={!node.type.startsWith("map.")} label={p.label} />;
  if (p.kind === "field") {
    const st = graph.structs?.find((x) => `struct:${x.id}` === nodeVar?.type);
    return st ? pick(st.fields.map((f) => [f.id, `${f.name} (${typeLabel(graph, f.type)})`]), "") : <em className="st-muted">Pick a struct variable first</em>;
  }
  if (p.kind === "enumvalue") {
    const en = allEnums(graph).find((x) => x.id === node.params.enum);
    return en ? (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(Number(e.target.value))}>
        {en.values.map((v, i) => (
          <option key={i} value={i}>
            {v}
          </option>
        ))}
      </select>
    ) : (
      <em className="st-muted">Pick an enum first</em>
    );
  }
  if (p.kind === "func" || p.kind === "timer") {
    const list = nodes.filter((n) => (p.kind === "func" ? n.type === "func.define" : ["timer.delay", "timer.repeat", "timer.countdown"].includes(n.type)));
    return (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        <option value="">{list.length ? "Pick one…" : p.kind === "func" ? "Add a Function node first" : "Add a timer first"}</option>
        {list.map((n) => (
          <option key={n.id} value={n.id}>
            {p.kind === "func" ? String(n.params.name ?? n.id) : `${nodeDef(n.type)?.title ?? n.type} (${n.id})`}
          </option>
        ))}
      </select>
    );
  }
  if (p.kind === "bone" || p.kind === "attachment") return <ModelPointPicker kind={p.kind} model={String(node.params.model ?? "models/player.mdl")} value={value as string | number} onChange={onChange} />;
  if (p.kind === "choice")
    return (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        {p.choices?.map(([k, l]) => (
          <option key={k} value={k}>
            {l}
          </option>
        ))}
      </select>
    );
  if (p.kind === "number") return <NumberField value={Number(value)} min={p.min} max={p.max} onChange={onChange} />;
  if (p.kind === "color") return <ColorField value={value} onChange={onChange} label={p.label} />;
  if (p.kind === "bool")
    return (
      <select className="st-field" value={value ? "1" : "0"} onChange={(e) => onChange(e.target.value === "1")}>
        <option value="1">Yes</option>
        <option value="0">No</option>
      </select>
    );
  // assets (X-fx): the typed file settings (Model / Sprite / Sound / Model or sprite)
  if (p.kind === "model" || p.kind === "sprite" || p.kind === "sound" || p.kind === "visual") return <AssetDefaultField kind={p.kind} value={String(value ?? "")} label={p.label} onChange={onChange} />;
  // T1fix (asset pickers): a model, sprite or sound path gets Choose… / Import… / your map projects
  if (p.kind === "path" && assetKindOf(String(p.default ?? value)))
    return <AssetField kind={assetKindOf(String(p.default ?? value))!} value={String(value)} max={64} allowed={PATH_CHARS} label={p.label} onChange={onChange} />;
  if (p.kind === "path") return <TextField value={String(value)} max={64} allowed={PATH_CHARS} onChange={onChange} />;
  // shared creatures (CP-ui, for X): a creature type param picks from built-in, your own and installed types
  if (p.kind === "name" && CREATURE_PARAMS.includes(`${node.type}:${p.id}`))
    return <CreaturePicker className="st-field" value={String(value)} label={p.label} onChange={(v) => NAME_CHARS.test(v) && onChange(v)} />;
  if (p.kind === "name" || p.kind === "entname") return <TextField value={String(value)} max={32} allowed={p.kind === "name" ? NAME_CHARS : ENT_NAME_CHARS} onChange={onChange} />;
  if (p.kind === "keys") return <TextField value={String(value)} max={p.max ?? 120} allowed={KEYS_CHARS} onChange={onChange} />;
  if (p.kind === "kvpath") return <TextField value={String(value)} max={p.max ?? 400} allowed={KVPATH_CHARS} onChange={onChange} />;
  if (p.kind === "var") {
    const list = vars.filter((v) => !p.varType || v.type === p.varType || (p.varType === "struct" && v.type.startsWith("struct:")) || (p.varType === "array" && v.type.startsWith("array<")));
    return (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        <option value="">{list.length ? "Pick one…" : "Add a variable under This mode first"}</option>
        {list.map((v) => (
          <option key={v.id} value={v.id}>
            {v.name} ({typeLabel(graph, v.type)}
            {v.scope === "player" ? ", per player" : v.scope === "team" ? ", per team" : ""})
          </option>
        ))}
      </select>
    );
  }
  if (p.kind === "varvalue") return nodeVar ? <ValueField type={nodeVar.type} value={value === "" ? undefined : value} onChange={onChange} graph={graph} /> : <em className="st-muted">Pick a variable first</em>;
  return <TextField value={String(value)} max={p.max ?? 100} allowed={LABEL_CHARS} onChange={onChange} />;
}

/** The game settings the mode's cfg sets (each map, after base.cfg): Set Setting's choices. */
function ModeSettings() {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const choices = nodeDef("game.set_cvar")?.params.find((p) => p.id === "cvar")?.choices ?? [];
  const list = graph.mode.settings ?? [];
  const put = (next: { cvar: string; value: number }[]) => edit((g) => ({ ...g, mode: { ...g.mode, settings: next } }));
  return (
    <>
      <h4 className="st-side-sub">Game settings</h4>
      <p className="st-side-help">Set every map, after the defaults (the mode's own cfg).</p>
      <ul className="st-vars-list">
        {list.map((st, k) => (
          <li key={k} className="st-var">
            {(() => {
              // a yes/no setting is a switch, a number one shows its unit; the whole name shows
              // (UX review M9)
              const label = choices.find(([c]) => c === st.cvar)?.[1] ?? st.cvar;
              const yesNo = /\(1 on\b/.test(label);
              const unit = /\(([^)]*)\)/.exec(label)?.[1] ?? "";
              const set = (value: number) => put(list.map((x, j) => (j === k ? { ...x, value } : x)));
              return (
                <>
                  <select className="st-field st-setting-pick" value={st.cvar} title={label} onChange={(e) => put(list.map((x, j) => (j === k ? { ...x, cvar: e.target.value } : x)))} aria-label="Setting">
                    {choices.map(([c, l]) => (
                      <option key={c} value={c}>
                        {l.replace(/ \(1 on\)$/, "")}
                      </option>
                    ))}
                  </select>
                  <div className="st-var-row">
                    {yesNo ? (
                      <label className="st-side-check">
                        <input type="checkbox" role="switch" checked={st.value !== 0} onChange={(e) => set(e.target.checked ? 1 : 0)} aria-label={label.replace(/ \(.*\)$/, "")} /> {st.value !== 0 ? "On" : "Off"}
                      </label>
                    ) : (
                      <>
                        <NumberField value={st.value} min={0} max={100000} onChange={set} />
                        {unit && <span className="st-muted">{unit}</span>}
                      </>
                    )}
                  </div>
                </>
              );
            })()}
            <button type="button" className="btn small" onClick={() => put(list.filter((_, j) => j !== k))}>
              Remove
            </button>
          </li>
        ))}
      </ul>
      <button type="button" className="btn small" disabled={list.length >= 32 || !choices.length} onClick={() => put([...list, { cvar: choices[0][0], value: 1 }])}>
        Add a setting
      </button>
    </>
  );
}

/** One variable's details (picked in the left panel). */
function VariableDetails({ id }: { id: string }) {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const selectVar = useStudio((s) => s.selectVar);
  const v = allVariables(graph).find((x) => x.id === id);
  if (!v) return null;
  const owner = graph.functions?.find((f) => f.locals?.some((x) => x.id === id));
  const complex = /^(array|map)<|^struct:/.test(v.type);
  const used = graph.nodes.filter((n) => n.params.var === id).length;
  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · {owner ? `Local variable of ${owner.name}` : "Variable"}</p>
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Name</span>
          <TextField value={v.name} max={30} allowed={LABEL_CHARS} fresh={v.id} onChange={(name) => edit((g) => updateVariable(g, v.id, { name }))} />
        </label>
        <label className="st-side-field">
          <span>Type</span>
          <TypePicker graph={graph} value={v.type} onChange={(type) => edit((g) => updateVariable(g, v.id, { type }))} />
        </label>
        {!owner && (
          <>
            <label className="st-side-field">
              <span>One for</span>
              <select className="st-field" value={v.scope} disabled={complex} title={complex ? "Arrays, maps and structs are kept for the whole mode" : undefined} onChange={(e) => edit((g) => updateVariable(g, v.id, { scope: e.target.value as GVariable["scope"] }))}>
                <option value="mode">The whole mode</option>
                <option value="player">Each player (reset when they join)</option>
                <option value="team">Each team</option>
              </select>
            </label>
            <label className="st-side-field">
              <span>Kept</span>
              <select className="st-field" value={v.reset ?? "never"} onChange={(e) => edit((g) => updateVariable(g, v.id, { reset: e.target.value as "never" | "round" }))}>
                <option value="never">For the map</option>
                <option value="round">Starts again each round</option>
              </select>
            </label>
          </>
        )}
        <label className="st-side-field">
          <span>Starts at</span>
          <ValueField type={v.type} value={v.default} graph={graph} onChange={(d) => edit((g) => updateVariable(g, v.id, { default: d }))} />
        </label>
      </div>
      {!owner && v.scope === "mode" && (v.type === "number" || v.type === "bool") && <SettingFields v={v} />}
      <p className="st-side-help">
        Used by {used} node{used === 1 ? "" : "s"}. Drag it from the left onto the canvas for Get or Set.
      </p>
      <button
        type="button"
        className="btn small"
        onClick={async () => {
          if (await askDelete({ kind: "var", id: v.id })) selectVar(null);
        }}
      >
        Remove this variable
      </button>
    </aside>
  );
}

/** Inputs, outputs or fields: a name and a type each, added and removed here. */
function Members({ graph, list, prefix, exec, containers = true, structs = true, max = 12, onChange, noun }: { graph: Graph; list: GMember[]; prefix: string; exec?: boolean; containers?: boolean; structs?: boolean; max?: number; onChange: (l: GMember[]) => void; noun: string }) {
  return (
    <>
      <ul className="st-vars-list">
        {list.map((m, k) => (
          <li key={m.id} className="st-var">
            <div className="st-var-row">
              <TextField value={m.name} max={30} allowed={LABEL_CHARS} onChange={(name) => onChange(list.map((x, j) => (j === k ? { ...x, name } : x)))} />
              <button type="button" className="btn small" aria-label={`Move ${m.name} up`} disabled={k === 0} onClick={() => onChange(list.map((x, j) => (j === k - 1 ? list[k] : j === k ? list[k - 1] : x)))}>
                ↑
              </button>
              <button type="button" className="btn small" aria-label={`Remove ${m.name}`} onClick={() => onChange(list.filter((_, j) => j !== k))}>
                ×
              </button>
            </div>
            <TypePicker graph={graph} value={m.type} exec={exec} containers={containers} structs={structs} onChange={(type) => onChange(list.map((x, j) => (j === k ? { ...x, type } : x)))} />
          </li>
        ))}
      </ul>
      <button type="button" className="btn small" disabled={list.length >= max} onClick={() => onChange([...list, { id: freshMember(list, prefix), name: `${noun} ${list.length + 1}`, type: "number" }])}>
        Add {noun.toLowerCase()}
      </button>
    </>
  );
}

/** A Function, Macro, struct or enum (picked in the left panel). */
function PartDetails({ id, embedded = false }: { id: string; embedded?: boolean }) {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const { selectPart, openTab } = useStudio.getState();
  const ask = useConfirm();
  const f = graph.functions?.find((x) => x.id === id);
  const m = graph.macros?.find((x) => x.id === id);
  const st = graph.structs?.find((x) => x.id === id);
  const en = allEnums(graph).find((x) => x.id === id);
  const calls = graph.nodes.filter((n) => n.params.func === id || n.params.macro === id).length;
  const library = useStudio((s) => s.library);
  const from = (f ?? m ?? st ?? en)?.from;
  const newer = newerInLibrary(library, from);
  const update = async () => {
    if (!newer || !from) return;
    if (!(await ask(`Update “${name}” to version ${newer.itemVersion} from your library? Its graph and pins are replaced by the library's (it keeps its name); calls to it stay.`, { ok: "Update" }))) return;
    edit((g) => insertItem(g, newer, id).graph);
  };
  const name = f?.name ?? m?.name ?? st?.name ?? en?.name ?? "";
  const rename = (v: string) => edit((g) => (f ? updateFunction(g, id, { name: v }) : m ? updateMacro(g, id, { name: v }) : st ? updateStruct(g, id, { name: v }) : updateEnum(g, id, { name: v })));
  const drop = async () => {
    if (f) {
      if (await askDelete({ kind: "fn", id })) selectPart(null);
      return;
    }
    if (!(await ask(`Remove “${name}”${m ? " and its graph" : ""}? Nodes using it show a problem until changed.`, { ok: "Remove", danger: true }))) return;
    edit((g) => (m ? removeGraphPart(g, id) : removeType(g, id)));
    selectPart(null);
  };
  const Box = embedded ? "div" : "aside";
  return (
    <Box className={embedded ? "st-side-embed" : "st-side"} aria-label={embedded ? undefined : "Details"}>
      <p className="st-side-kicker">{embedded ? "" : "Details · "}{f ? (f.pure ? "Pure Function" : "Function") : m ? "Macro" : st ? "Struct" : "Enum"}</p>
      {from && (
        <div className={newer ? "st-update" : "st-side-help"}>
          {newer ? (
            <>
              <p className="st-side-help">
                Your library has a newer “{newer.name}” (version {newer.itemVersion}; this copy is version {from.version}).
              </p>
              <button type="button" className="btn small" onClick={() => void update()}>
                Update from library
              </button>
            </>
          ) : (
            <span className="st-muted">From your library{library.some((x) => x.id === from.lib) ? `, version ${from.version} (up to date)` : " (no longer there)"}.</span>
          )}
        </div>
      )}
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Name</span>
          <TextField value={name} max={30} allowed={LABEL_CHARS} fresh={id} onChange={rename} />
        </label>
        {f && (
          <label className="st-side-check">
            <input type="checkbox" checked={!!f.pure} onChange={() => edit((g) => ({ ...updateFunction(g, id, { pure: !f.pure || undefined }), nodes: g.nodes.map((n) => (n.params.func === id && (n.type === "fn.call" || n.type === "fn.call_pure") ? { ...n, type: f.pure ? "fn.call" : "fn.call_pure" } : n)) }))} />
            <span>Pure: no flow pins where it's called; worked out where its outputs are used (no Delays)</span>
          </label>
        )}
      </div>
      {(f || m) && (
        <>
          <h4 className="st-side-sub">Inputs</h4>
          <Members graph={graph} list={(f ?? m)!.inputs} prefix="a" exec={!!m} noun="Input" onChange={(inputs) => edit((g) => (f ? updateFunction(g, id, { inputs }) : updateMacro(g, id, { inputs })))} />
          <h4 className="st-side-sub">Outputs</h4>
          <Members graph={graph} list={(f ?? m)!.outputs} prefix="r" exec={!!m} noun="Output" onChange={(outputs) => edit((g) => (f ? updateFunction(g, id, { outputs }) : updateMacro(g, id, { outputs })))} />
          <p className="st-side-help">
            Called from {calls} place{calls === 1 ? "" : "s"}.{" "}
            <button type="button" className="btn small" onClick={() => openTab(id)}>
              Open its graph
            </button>
          </p>
        </>
      )}
      {st && (
        <>
          <h4 className="st-side-sub">Fields</h4>
          <Members graph={graph} list={st.fields} prefix="f" containers={false} structs={false} noun="Field" onChange={(fields) => edit((g) => updateStruct(g, id, { fields }))} />
          <p className="st-side-help">Drag it from the left onto the canvas for Make; Break, Set Field and arrays of it are in the node search.</p>
        </>
      )}
      {en && (
        <>
          <h4 className="st-side-sub">Values</h4>
          <ul className="st-vars-list">
            {en.values.map((v, k) => (
              <li key={k} className="st-var">
                <div className="st-var-row">
                  <TextField value={v} max={30} allowed={LABEL_CHARS} onChange={(x) => edit((g) => updateEnum(g, id, { values: en.values.map((y, j) => (j === k ? x : y)) }))} />
                  <button type="button" className="btn small" aria-label={`Remove ${v}`} disabled={en.values.length <= 1} onClick={() => edit((g) => updateEnum(g, id, { values: en.values.filter((_, j) => j !== k) }))}>
                    ×
                  </button>
                </div>
              </li>
            ))}
          </ul>
          <button type="button" className="btn small" disabled={en.values.length >= 32} onClick={() => edit((g) => updateEnum(g, id, { values: [...en.values, `Value ${en.values.length + 1}`] }))}>
            Add value
          </button>
        </>
      )}
      <p className="st-side-help">
        <button type="button" className="btn small" onClick={() => void drop()}>
          Remove “{name}”
        </button>
      </p>
    </Box>
  );
}

const COMMENT_COLOURS = ["#5b6a4b", "#4a5a6a", "#6a4a8a", "#2f6a4a", "#7a6a2b", "#7a2b2b", "#2f4f7a", "#555555"];

const cleanLabel = (v: string, max: number) => [...v].filter((c) => LABEL_CHARS.test(c)).join("").slice(0, max);

const MARKER_ID = /^[a-z0-9_]{0,24}$/;
const ICONS = ["flag", "skull", "star", "crate", "door", "person", "arrow", "circle", "bolt", "heart"];
const RESERVED = ["classname", "origin", "angles", "angle", "model", "targetname", "target", "mode", "kind", "order", "path", "size", "radius", "spawnflags"];

/** One key a marker carries in the map. */
function MarkerKeyRow({ k, onChange, onRemove }: { k: GMarkerKey; onChange: (k: GMarkerKey) => void; onRemove: () => void }) {
  const choicesText = (k.choices ?? []).map(([v, l]) => (v === l ? v : `${v}=${l}`)).join(", ");
  return (
    <li className="st-var">
      <div className="st-mk-key-row">
        <TextField value={k.name} max={24} allowed={MARKER_ID} onChange={(name) => onChange({ ...k, name })} />
        <select className="st-field" value={k.type} aria-label="Key type" onChange={(e) => {
          const type = e.target.value as GMarkerKey["type"];
          onChange({ ...k, type, default: type === "number" ? 0 : type === "bool" ? false : type === "choice" ? k.choices?.[0]?.[0] ?? "" : "", choices: type === "choice" ? k.choices ?? [] : undefined });
        }}>
          <option value="number">Number</option>
          <option value="text">Text</option>
          <option value="bool">Yes/no</option>
          <option value="choice">Choice</option>
        </select>
        <button type="button" className="btn small" aria-label={`Remove the key ${k.name}`} onClick={onRemove}>×</button>
      </div>
      {RESERVED.includes(k.name) && <p className="st-side-help bad">Maps use “{k.name}” themselves: pick another name.</p>}
      <label className="st-side-field">
        <span>Default</span>
        {k.type === "number" ? (
          <NumberField value={Number(k.default ?? 0)} onChange={(v) => onChange({ ...k, default: v })} />
        ) : k.type === "bool" ? (
          <select className="st-field" value={k.default ? "1" : "0"} onChange={(e) => onChange({ ...k, default: e.target.value === "1" })}>
            <option value="1">Yes</option>
            <option value="0">No</option>
          </select>
        ) : k.type === "choice" ? (
          <select className="st-field" value={String(k.default ?? "")} onChange={(e) => onChange({ ...k, default: e.target.value })}>
            {(k.choices ?? []).map(([v, l]) => (
              <option key={v} value={v}>{l}</option>
            ))}
          </select>
        ) : (
          <TextField value={String(k.default ?? "")} max={60} allowed={LABEL_CHARS} onChange={(v) => onChange({ ...k, default: v })} />
        )}
      </label>
      {k.type === "choice" && (
        <label className="st-side-field">
          <span>Choices (value=Label, …)</span>
          <input className="st-field" defaultValue={choicesText} placeholder="north=North, south=South" onBlur={(e) => {
            const choices = e.target.value.split(",").map((x) => x.trim()).filter(Boolean).slice(0, 16).map((x) => {
              const [v, l] = x.split("=").map((y) => y.trim());
              return [v.toLowerCase().replace(/[^a-z0-9_]/g, "_").slice(0, 24), (l || v).slice(0, 30)] as [string, string];
            });
            onChange({ ...k, choices, default: choices.some(([v]) => v === k.default) ? k.default : choices[0]?.[0] ?? "" });
          }} />
        </label>
      )}
      <label className="st-side-field">
        <span>Help</span>
        <TextField value={k.help ?? ""} max={120} allowed={LABEL_CHARS} onChange={(help) => onChange({ ...k, help })} />
      </label>
    </li>
  );
}

/** A map marker (picked under Map markers): what a map for this mode places. */
function MarkerDetails({ id }: { id: string }) {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const { selectPart } = useStudio.getState();
  const ask = useConfirm();
  const m = graph.markers!.find((x) => x.id === id)!;
  const set = (patch: Partial<GMarker>) => {
    edit((g) => updateMarker(g, id, patch));
    if (patch.id && patch.id !== id) selectPart(`mk:${patch.id}`);
  };
  const used = graph.nodes.filter((n) => n.params.marker === id).length;
  const keys = m.keys ?? [];
  const setKey = (i: number, k: GMarkerKey) => set({ keys: keys.map((x, j) => (j === i ? k : x)) });
  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Map marker</p>
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Name</span>
          <TextField value={m.label} max={30} allowed={LABEL_CHARS} onChange={(label) => set({ label })} />
        </label>
        <label className="st-side-field">
          <span>Id in the map (kind)</span>
          <TextField value={m.id} max={24} allowed={MARKER_ID} onChange={(v) => v && !(graph.markers ?? []).some((x) => x.id === v && x.id !== id) && set({ id: v })} />
        </label>
        <label className="st-side-field">
          <span>What it's for (map makers read this)</span>
          <TextField value={m.description} max={200} allowed={LABEL_CHARS} onChange={(description) => set({ description })} />
        </label>
        <label className="st-side-field">
          <span>Shape</span>
          <select className="st-field" value={m.type} onChange={(e) => {
            const type = e.target.value as GMarker["type"];
            set({ type, size: type === "area" ? m.size ?? [256, 256, 128] : undefined, radius: type === "area" ? m.radius : undefined });
          }}>
            <option value="point">A point (one spot)</option>
            <option value="area">An area (a box or circle round a spot)</option>
            <option value="path">A path (spots in order)</option>
          </select>
        </label>
        {m.type === "area" && (
          <label className="st-side-field">
            <span>{m.radius != null ? "Radius" : "Box (x y z)"}</span>
            {m.radius != null ? (
              <NumberField value={m.radius} min={1} max={16384} onChange={(radius) => set({ radius })} />
            ) : (
              <span className="st-var-row st-num-row">
                {[0, 1, 2].map((i) => (
                  <NumberField key={i} value={(m.size ?? [256, 256, 128])[i]} min={1} max={16384} onChange={(v) => set({ size: (m.size ?? [256, 256, 128]).map((x, j) => (j === i ? v : x)) as [number, number, number] })} />
                ))}
              </span>
            )}
            <button type="button" className="btn small" onClick={() => set(m.radius != null ? { radius: undefined, size: [256, 256, 128] } : { radius: 256, size: undefined })}>
              {m.radius != null ? "Use a box" : "Use a circle"}
            </button>
          </label>
        )}
        <label className="st-side-field">
          <span>{m.type === "path" ? "Paths: at least" : "At least"}</span>
          <NumberField value={m.min} min={0} max={64} onChange={(min) => set({ min: Math.round(min), max: m.max != null && m.max < min ? Math.round(min) || 1 : m.max })} />
        </label>
        <label className="st-side-field">
          <span>At most (empty: no limit)</span>
          <input className="st-field st-field-num" type="number" min={1} max={64} value={m.max ?? ""} onChange={(e) => {
            const v = e.target.value === "" ? undefined : Math.max(1, Math.min(64, Math.round(Number(e.target.value))));
            if (v === undefined || Number.isFinite(v)) set({ max: v });
          }} />
        </label>
        <label className="st-side-field">
          <span>Color in the editor</span>
          <input className="st-field" type="color" value={m.colour || "#d9c27a"} onChange={(e) => set({ colour: e.target.value })} />
        </label>
        <label className="st-side-field">
          <span>Icon</span>
          <select className="st-field" value={m.icon || "flag"} onChange={(e) => set({ icon: e.target.value })}>
            {ICONS.map((i) => (
              <option key={i} value={i}>{i}</option>
            ))}
          </select>
        </label>
      </div>
      <h4 className="st-side-sub">Keys map makers set</h4>
      <ul className="st-vars-list">
        {keys.map((k, i) => (
          <MarkerKeyRow key={i} k={k} onChange={(x) => setKey(i, x)} onRemove={() => set({ keys: keys.filter((_, j) => j !== i) })} />
        ))}
      </ul>
      <button type="button" className="btn small" disabled={keys.length >= 8} onClick={() => {
        let n = keys.length + 1;
        while (keys.some((k) => k.name === `key_${n}`)) n++;
        set({ keys: [...keys, { name: `key_${n}`, type: "number", default: 0, help: "" }] });
      }}>
        Add key
      </button>
      <p className="st-side-help">
        In the map: an <code>ls_marker</code> with <code>kind</code> “{m.id}”{m.type === "path" ? ", one per spot with order 1, 2, 3…" : ""}. Used by {used} node{used === 1 ? "" : "s"}; drag it onto the canvas for Find Marker.
      </p>
      <p className="st-side-help">
        <button type="button" className="btn small" onClick={async () => {
          if (!(await ask(`Remove the map marker “${m.label}”? Nodes using it show a problem until changed.`, { ok: "Remove", danger: true }))) return;
          edit((g) => removeMarker(g, id));
          selectPart(null);
        }}>
          Remove “{m.label}”
        </button>
      </p>
    </aside>
  );
}

/** The kit's advice for map makers: size, players, notes. */
function MapAdvice() {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const k = graph.mapKit ?? { size: "" as const, notes: "" };
  const set = (patch: Partial<NonNullable<Graph["mapKit"]>>) => edit((g) => ({ ...g, mapKit: { ...(g.mapKit ?? { size: "", notes: "" }), ...patch } }));
  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Map advice</p>
      <p className="st-side-help">Goes in the mode's map kit with the markers, the components its nodes use and the creatures it spawns.</p>
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Map size</span>
          <select className="st-field" value={k.size} onChange={(e) => set({ size: e.target.value as "" })}>
            <option value="">Any</option>
            <option value="small">Small (up to 2048 across)</option>
            <option value="medium">Medium (up to 4096)</option>
            <option value="large">Large</option>
          </select>
        </label>
        <label className="st-side-field">
          <span>Players, from</span>
          <NumberField value={k.playersMin ?? 1} min={1} max={32} onChange={(v) => set({ playersMin: Math.round(v) })} />
        </label>
        <label className="st-side-field">
          <span>to</span>
          <NumberField value={k.playersMax ?? 32} min={1} max={32} onChange={(v) => set({ playersMax: Math.round(v) })} />
        </label>
        <label className="st-side-field">
          <span>Notes for map makers</span>
          <textarea className="st-field" maxLength={600} rows={5} value={k.notes} onChange={(e) => set({ notes: [...e.target.value].filter((c) => LABEL_CHARS.test(c) || c === "\n").join("").replace(/\n/g, " ") })} />
        </label>
      </div>
    </aside>
  );
}

/** A Timeline's tab is open and nothing else is picked: what it is (UX review M8). */
function TimelineDetails({ id }: { id: string }) {
  const graph = useStudio((s) => s.graph)!;
  const tl = graph.timelines?.find((t) => t.id === id);
  if (!tl) return null;
  const plays = graph.nodes.filter((n) => n.type === "timeline.play" && n.params.timeline === id).length;
  const kinds: Record<string, string> = { float: "number", vector: "position", color: "color", event: "events" };
  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Timeline</p>
      <h3 className="st-side-title">{tl.name}</h3>
      <p className="st-side-help">
        {tl.length} s{tl.loop ? ", looping" : ""}
        {tl.autoplay ? ", plays at each round's start" : ""}. {plays ? `${plays} Timeline node${plays === 1 ? "" : "s"} play${plays === 1 ? "s" : ""} it.` : "No Timeline node plays it yet: find Timeline: " + tl.name + " in the node search."}
      </p>
      <h4 className="st-side-sub">Tracks</h4>
      {tl.tracks.length ? (
        <ul className="st-side-list">
          {tl.tracks.map((t) => (
            <li key={t.id}>
              {t.name} <span className="st-muted">({kinds[t.kind] ?? t.kind}, {t.keys.length} key{t.keys.length === 1 ? "" : "s"})</span>
            </li>
          ))}
        </ul>
      ) : (
        <p className="st-muted">None yet: add one in the editor.</p>
      )}
      <p className="st-side-help">Change its name, length, tracks and keys in its tab.</p>
    </aside>
  );
}

export function Inspector() {
  const graph = useStudio((s) => s.graph);
  const selected = useStudio((s) => s.selected);
  const selectedVar = useStudio((s) => s.selectedVar);
  const part = useStudio((s) => s.part);
  const problems = useStudio((s) => s.problems);
  const runs = useStudio((s) => s.debug.runs);
  const edit = useStudio((s) => s.edit);
  const tab = useStudio((s) => s.tab);
  const modeShown = useStudio((s) => s.modeShown);
  if (!graph) return null;
  const tlTab = (graph.timelines ?? []).find((t) => t.id === tab);
  if (tlTab && !modeShown && !selected.length && !part && !selectedVar) return <TimelineDetails id={tlTab.id} />;
  if (selectedVar && allVariables(graph).some((x) => x.id === selectedVar)) return <VariableDetails id={selectedVar} />;
  if (part?.startsWith("mk:") && !selected.length) {
    const id = part.slice(3);
    if (!id) return <MapAdvice />;
    if ((graph.markers ?? []).some((m) => m.id === id)) return <MarkerDetails id={id} />;
  }
  if (part && !selected.length && (graph.customEvents ?? []).some((c) => c.id === part)) return <CustomEventDetails id={part} />;
  if (part && !selected.length) {
    const d = <PartDetails id={part} />;
    if ([...(graph.functions ?? []), ...(graph.macros ?? []), ...(graph.structs ?? []), ...(graph.enums ?? [])].some((x) => x.id === part)) return d;
  }
  const node = selected.length === 1 ? graph.nodes.find((n) => n.id === selected[0]) : undefined;
  const def = node && nodeDef(node.type);

  if (node && def) {
    const mine = problems.filter((p) => p.node === node.id);
    const run = runs[node.id];
    return (
      <aside className="st-side" aria-label="Details">
        <p className="st-side-kicker">Details · {def.category}</p>
        <h3 className="st-side-title">{nodeTitle(graph, node)}</h3>
        <p className="st-side-help">{def.help}</p>
        {def.needs && <p className="st-side-help st-needs">{def.needs === "components" ? "Needs the game mode components (gm_components) on the server; without them this does nothing and Playtest says so." : "Needs Loner-Strike's bot brain with creatures; without it this does nothing and Playtest says so."}</p>}
        {orphanValues(node).length > 0 && (
          <div className="st-side-fields" aria-label="Removed settings">
            {orphanValues(node).map((k) => (
              <div key={k} className="st-orphan-value" title="This setting was removed in an update. Clear it or delete the node.">
                <span>Orphaned: {k} = {String(node.params?.[k])}</span>
                <button type="button" className="btn small" onClick={() => edit((g) => clearParam(g, node.id, k))}>Clear</button>
              </div>
            ))}
          </div>
        )}
        {def.params.length > 0 && (
          <div className="st-side-fields">
            {def.params.map((p) => {
              const wiredInstead = def.inputs.some((i) => i.param === p.id && graph.edges.some((e) => e.to.node === node.id && e.to.pin === i.id));
              return (
                <label key={p.id} className="st-side-field">
                  <span>{p.label}</span>
                  {wiredInstead ? (
                    <em className="st-muted">From its wire</em>
                  ) : (
                    <ParamField
                      p={p}
                      value={node.params[p.id] ?? p.default}
                      vars={[...graph.variables, ...(graph.functions?.find((f) => f.id === node.graph)?.locals ?? [])]}
                      nodeVar={allVariables(graph).find((x) => x.id === node.params.var)}
                      nodes={graph.nodes}
                      graph={graph}
                      node={node}
                      onChange={(v) => edit((g) => setParam(g, node.id, p.id, v))}
                    />
                  )}
                </label>
              );
            })}
          </div>
        )}
        {["fn.entry", "fn.return", "macro.entry", "macro.exit"].includes(node.type) && node.graph && <PartDetails id={node.graph} embedded />}
        {!def.reroute && (
          <label className="st-side-check">
            <input type="checkbox" checked={!!node.breakpoint} onChange={() => edit((g) => toggleBreakpoint(g, node.id))} />
            <span>Breakpoint: pause the world when this runs (F9)</span>
          </label>
        )}
        {mine.length > 0 && (
          <ul className="st-problems">
            {mine.map((p, i) => (
              <li key={i} className={p.level}>
                {p.message}
              </li>
            ))}
          </ul>
        )}
        {run && (
          <div className="st-side-run">
            <h4>Last time it ran</h4>
            <div>
              {run.count}× · {run.error ? <span className="st-bad-text">{run.error}</span> : run.result ?? "ran"}
            </div>
            {run.inputs && Object.keys(run.inputs).length > 0 && <Values values={run.inputs} />}
          </div>
        )}
      </aside>
    );
  }

  const comment = selected.length === 1 ? graph.comments.find((c) => c.id === selected[0]) : undefined;
  if (comment)
    return (
      <aside className="st-side" aria-label="Details">
        <p className="st-side-kicker">Details · Comment</p>
        <p className="st-side-help">Double-click its title bar to change the text. Dragging the title bar moves the box and the nodes inside; inside it, the canvas works as anywhere.</p>
        <div className="st-side-fields">
          <div className="st-side-field">
            <span id="st-comment-colour">Color</span>
            <div className="st-swatches" role="radiogroup" aria-labelledby="st-comment-colour">
              {COMMENT_COLOURS.map((c) => (
                <button key={c} type="button" role="radio" aria-checked={(comment.color ?? "#5b6a4b") === c} className={`st-swatch${(comment.color ?? "#5b6a4b") === c ? " on" : ""}`} style={{ background: c }} aria-label={`Color ${c}`} onClick={() => edit((g) => ({ ...g, comments: g.comments.map((x) => (x.id === comment.id ? { ...x, color: c } : x)) }))} />
              ))}
            </div>
          </div>
        </div>
      </aside>
    );

  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Mode</p>
      <h3 className="st-side-title">This mode</h3>
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Name</span>
          <input className="st-field" value={graph.mode.name} maxLength={40} onChange={(e) => edit((g) => ({ ...g, mode: { ...g.mode, name: cleanLabel(e.target.value, 40) } }))} />
        </label>
        <label className="st-side-field">
          <span>Description</span>
          <input className="st-field" value={graph.mode.description} maxLength={60} onChange={(e) => edit((g) => ({ ...g, mode: { ...g.mode, description: cleanLabel(e.target.value, 60) } }))} />
        </label>
        <div className="st-side-field">
          <span>Id</span>
          <code className="st-muted">{graph.mode.id}</code>
        </div>
      </div>
      <ModeSettings />
      <h4 className="st-side-sub">Requirements</h4>
      <Requirements id={graph.mode.id} />
      <p className="st-side-help st-keys">
        Right-click or Space: add a node · drag from a pin to empty space: nodes for that wire · double-click a wire: reroute point · C: comment · F9:
        breakpoint · ⌘Z / ⇧⌘Z: undo / redo · ⌘C / ⌘V: copy / paste · ?: every shortcut
      </p>
    </aside>
  );
}

/** A node's input values as the game saw them (players with team, health, weapon). */
export function Values({ values }: { values: Record<string, unknown> }) {
  return (
    <dl className="st-values">
      {Object.entries(values).map(([k, v]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd>{showValue(v)}</dd>
        </div>
      ))}
    </dl>
  );
}

export function showValue(v: unknown): string {
  if (v == null) return "nobody";
  if (typeof v === "object") {
    const p = v as { name?: string; team?: string; hp?: number; alive?: boolean; weapon?: string };
    if (typeof p.name === "string") return `${p.name} (${p.team ?? "?"}, ${p.alive ? `${p.hp} hp${p.weapon ? `, ${p.weapon}` : ""}` : "dead"})`;
    return JSON.stringify(v);
  }
  return String(v);
}

/** A Custom Event: its name, inputs, whether the map can fire it, and where it's used. */
function CustomEventDetails({ id }: { id: string }) {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const { selectPart } = useStudio.getState();
  const c = graph.customEvents?.find((x) => x.id === id);
  if (!c) return null;
  const uses = graph.nodes.filter((n) => ["event.custom", "event.call", "timer.by_event"].includes(n.type) && n.params.event === id);
  const drop = async () => {
    if (await askDelete({ kind: "cevent", id })) selectPart(null);
  };
  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Custom Event</p>
      <label className="st-side-label">
        Name
        <TextField value={c.name} max={26} allowed={LABEL_CHARS} fresh={id} onChange={(name) => edit((g) => updateCustomEvent(g, id, { name }))} />
      </label>
      <p className="st-side-help">Add a Custom Event node for it on any event graph (what runs), and Call {c.name} or Set Timer by {c.name} where it should run. Find them in the node search.</p>
      <h4 className="st-side-sub">Inputs</h4>
      <Members graph={graph} list={c.inputs} prefix="i" max={8} noun="Input" onChange={(inputs) => edit((g) => updateCustomEvent(g, id, { inputs }))} />
      <label className="st-check-auto">
        <input type="checkbox" checked={!!c.map} onChange={(e) => edit((g) => updateCustomEvent(g, id, { map: e.target.checked }))} /> The map can fire it
      </label>
      {c.map && (
        <p className="st-side-help">
          In the map: an <code>ls_event</code> entity with event <code>{mapName(c.name)}</code>, or an <code>info_target</code> named <code>lsev_{mapName(c.name)}</code>; trigger it (a trigger_multiple or button targeting it). Player is who set it off; numbers and texts come from the entity's keys a and b.
        </p>
      )}
      <p className="st-muted">{uses.length ? `Used by ${uses.length} node${uses.length === 1 ? "" : "s"}.` : "Not used yet."}</p>
      <button type="button" className="btn danger small" onClick={() => void drop()}>
        Remove this Custom Event
      </button>
    </aside>
  );
}

/** "Setting (hosts can change it)": a whole-mode number or yes/no a host sets on the Play page,
 *  the server page, the admin menu or through MCP, without editing the graph. */
function SettingFields({ v }: { v: GVariable }) {
  const edit = useStudio((s) => s.edit);
  const graph = useStudio((s) => s.graph)!;
  const st = v.setting;
  const set = (patch: Partial<NonNullable<GVariable["setting"]>> | null) =>
    edit((g) => updateVariable(g, v.id, { setting: patch === null ? undefined : { label: v.name, help: "", min: 0, max: 100, step: 0, group: "", ...st, ...patch } }));
  const num = (label: string, key: "min" | "max" | "step", help?: string) => (
    <label className="st-side-field" title={help}>
      <span>{label}</span>
      <input className="st-field" type="number" value={st?.[key] ?? 0} onChange={(e) => set({ [key]: Number(e.target.value) || 0 })} aria-label={`Setting ${label}`} />
    </label>
  );
  return (
    <div className="st-side-fields">
      <label className="st-check-auto" title="Hosts set its start value on the Play page, the server page and the in-game admin menu, without editing or compiling the mode">
        <input type="checkbox" checked={!!st} onChange={(e) => set(e.target.checked ? {} : null)} /> Setting (hosts can change it)
      </label>
      {st && (
        <>
          <label className="st-side-field">
            <span>Label</span>
            <TextField value={st.label} max={30} allowed={LABEL_CHARS} onChange={(label) => set({ label })} />
          </label>
          <label className="st-side-field">
            <span>Help</span>
            <TextField value={st.help ?? ""} max={120} allowed={LABEL_CHARS} onChange={(help) => set({ help })} />
          </label>
          <label className="st-side-field">
            <span>Group</span>
            <TextField value={st.group ?? ""} max={20} allowed={LABEL_CHARS} onChange={(group) => set({ group })} />
          </label>
          {v.type === "number" && !st.choices?.length && (
            <>
              {num("Smallest", "min")}
              {num("Largest", "max")}
              {num("Step", "step", "How much the admin menu's Less/More changes it (0: a tenth of the range)")}
            </>
          )}
          {v.type === "number" && (
            <label className="st-side-field" title="Named choices instead of a number: the value is the choice's place (0, 1, 2...)">
              <span>Choices</span>
              <TextField
                value={(st.choices ?? []).join(", ")}
                max={300}
                allowed={LABEL_CHARS}
                onChange={(t) => {
                  const choices = t.split(",").map((c) => c.trim()).filter(Boolean).slice(0, 12);
                  set({ choices: choices.length ? choices : undefined });
                }}
              />
            </label>
          )}
          <p className="st-side-help">
            Its name on servers: <code>{`lsm_${graph.mode.id}_${v.id}`.toLowerCase()}</code>. Each server keeps its own value; the mode reads it as a map starts and at each new round.
          </p>
        </>
      )}
    </div>
  );
}
