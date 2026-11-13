// One creature type's form: real controls for every key the bot brain reads from creatures.ini,
// with the game's limits on each, plain help, the problems the server found under the control
// they're about, the model in 3D, and the sounds to pick and play.

import { useQuery } from "@tanstack/react-query";
import { lazy, Suspense, useMemo, useRef, useState, type ReactNode } from "react";
import { Info, Segmented, Switch } from "../../../ui";
import { Row, Rows } from "../common";
import { matches, searchWords } from "../common/search";
import { listModels, listSounds, lookModel, soundUrl, type Checked, type Creature, type Limits, type ModelItem, type Problem, type Vec3 } from "./creaturesApi";
import { ANIM_WORDS, ATTACK_WORDS, BEHAVIOUR_WORDS, BLOOD_WORDS, CATEGORY_WORDS, HATES_WORDS, KIND_WORDS, SOUND_WORDS, TARGET_WORDS, aggressionWords } from "./words";

const CreaturePreview = lazy(() => import("./CreaturePreview"));
const STAND_IN = "models/hostage.mdl";

const hex = (c: Vec3) => "#" + c.map((v) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, "0")).join("");
const rgb = (h: string): Vec3 => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) || 0) as Vec3;
const isOff = (c: Vec3) => c.every((v) => v === 0);

/** A slider and a number box for one value, within the game's range. */
function Num({ value, onChange, min, max, step = 1, unit, label, sliderMax }: { value: number; onChange: (v: number) => void; min: number; max: number; step?: number; unit?: string; label: string; sliderMax?: number }) {
  const top = sliderMax ?? max;
  return (
    <span className="cs-num">
      <input type="range" min={min} max={top} step={step} value={Math.min(Math.max(value, min), top)} aria-label={label} onChange={(e) => onChange(Number(e.target.value))} />
      <input className="search cs-numbox" type="number" min={min} max={max} step={step} value={Number.isFinite(value) ? value : ""} aria-label={`${label} (number)`} onChange={(e) => onChange(e.target.value === "" ? NaN : Number(e.target.value))} />
      {unit && <span className="muted cs-unit">{unit}</span>}
    </span>
  );
}

function Problems({ list }: { list: Problem[] }) {
  if (!list.length) return null;
  return (
    <ul className="cs-problems">
      {list.map((p, i) => (
        <li key={i} className={p.error ? "cs-error" : "cs-warning"}>
          {p.text}
        </li>
      ))}
    </ul>
  );
}

/** The model: search the game's models, see who has the one picked. */
function ModelPicker({ value, onChange, report }: { value: string; onChange: (v: string) => void; report?: Checked["models"] }) {
  const models = useQuery({ queryKey: ["tools", "creatures", "models"], queryFn: ({ signal }) => listModels(signal).then((r) => r.models), staleTime: 60_000 });
  const [q, setQ] = useState("");
  const [open, setOpen] = useState(false);
  const parts = value.split(/\s+/).filter(Boolean);
  const first = parts[0] ?? "";
  const standIn = parts.includes(STAND_IN) && first !== STAND_IN;
  const shown = useMemo(() => {
    const w = searchWords(q);
    return (models.data ?? []).filter((m) => matches(w, m.path, m.group)).slice(0, 120);
  }, [models.data, q]);
  const pick = (m: ModelItem) => {
    const rest = parts.slice(1).filter((p) => p !== m.path);
    onChange([m.path, ...rest].join(" "));
    setOpen(false);
    setQ("");
  };
  const setStandIn = (on: boolean) => onChange((on ? [first, ...parts.slice(1).filter((p) => p !== STAND_IN), STAND_IN] : parts.filter((p, i) => i === 0 || p !== STAND_IN)).join(" "));
  const c = report?.candidates;
  return (
    <div className="cs-model">
      <div className="row cs-model-row">
        <code className="cs-model-path" title={value}>{first || "No model"}</code>
        <button type="button" className="btn small" aria-expanded={open} onClick={() => setOpen(!open)}>
          {open ? "Close" : "Change…"}
        </button>
      </div>
      {open && (
        <div className="cs-model-list panel">
          <input className="search" type="search" autoFocus placeholder="Search models (zombie, player/gign…)" value={q} aria-label="Search models" onChange={(e) => setQ(e.target.value)} />
          {models.isPending && <p className="muted"><span className="spinner" aria-hidden="true" /> Finding models…</p>}
          <ul>
            {shown.map((m) => (
              <li key={m.path}>
                <button type="button" className={`cs-pick${m.path === first ? " on" : ""}`} onClick={() => pick(m)} title={m.note}>
                  <span>{m.path.replace(/^models\//, "")}</span>
                  <span className={`cs-kind ${m.kind}`}>{KIND_WORDS[m.kind]}</span>
                </button>
              </li>
            ))}
            {models.data && shown.length === 0 && <li className="muted">No model matches.</li>}
          </ul>
        </div>
      )}
      {c?.[0] && (
        <p className={`small-note cs-avail ${c[0].kind}`}>
          <span className={`cs-kind ${c[0].kind}`}>{KIND_WORDS[c[0].kind]}</span> {c[0].note}
          {report?.standIn && ` Your game doesn't have it, so ${c[report.picked ?? 0]?.path} stands in.`}
        </p>
      )}
      <label className="tl-check cs-standin">
        <input type="checkbox" checked={standIn} disabled={!first || first === STAND_IN} onChange={(e) => setStandIn(e.target.checked)} />
        Use the hostage as a stand-in where this model is missing (tinted, so it still plays)
      </label>
    </div>
  );
}

/** One sound slot: the files (play, remove), "|" groups are fallbacks, and a search to add. */
function SoundField({ value, onChange, report, label }: { value: string; onChange: (v: string) => void; report?: Checked["sounds"] extends (infer R)[] | undefined ? R : never; label: string }) {
  const sounds = useQuery({ queryKey: ["tools", "creatures", "sounds"], queryFn: ({ signal }) => listSounds(signal).then((r) => r.sounds), staleTime: 60_000 });
  const [q, setQ] = useState("");
  const audio = useRef<HTMLAudioElement | null>(null);
  const groups = value.split("|").map((g) => g.split(/\s+/).filter(Boolean));
  const play = (f: string) => {
    audio.current?.pause();
    audio.current = new Audio(soundUrl(f));
    void audio.current.play().catch(() => undefined);
  };
  const write = (gs: string[][]) => onChange(gs.filter((g, i) => g.length || i === 0).map((g) => g.join(" ")).join(" | ").trim());
  const remove = (gi: number, fi: number) => write(groups.map((g, i) => (i === gi ? g.filter((_, j) => j !== fi) : g)));
  const add = (f: string) => {
    if (!f) return;
    const gs = groups.map((g) => [...g]);
    if (!gs[0].includes(f)) gs[0].push(f);
    write(gs);
    setQ("");
  };
  const hits = useMemo(() => {
    const w = searchWords(q);
    return q.trim().length < 2 ? [] : (sounds.data ?? []).filter((s) => matches(w, s.file)).slice(0, 40);
  }, [sounds.data, q]);
  const id = `cs-snd-${label.replace(/\W/g, "")}`;
  return (
    <div className="cs-sound">
      {groups.map((g, gi) => (
        <div key={gi} className="cs-sound-group">
          {gi > 0 && <span className="muted cs-or" title="Used only when none of the files before it are in the game">or, if none of those:</span>}
          {g.map((f, fi) => {
            const found = report?.groups[gi]?.files.find((x) => x.file === f)?.found;
            return (
              <span key={f + fi} className={`cs-chip${found === false ? " missing" : ""}`} title={found === false ? "Not in your game's folders" : f}>
                <button type="button" className="cs-chip-play" aria-label={`Play ${f}`} onClick={() => play(f)}>
                  ▶
                </button>
                {f}
                <button type="button" className="cs-chip-x" aria-label={`Remove ${f}`} onClick={() => remove(gi, fi)}>
                  ×
                </button>
              </span>
            );
          })}
        </div>
      ))}
      <input className="search cs-sound-add" list={id} placeholder="Add a sound (type to search)" aria-label={`Add a ${label} sound`} value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => e.key === "Enter" && add(q.trim())} />
      <datalist id={id}>
        {hits.map((s) => (
          <option key={s.file} value={s.file} />
        ))}
      </datalist>
      {q && hits.some((h) => h.file === q) && (
        <button type="button" className="btn small" onClick={() => add(q)}>
          Add
        </button>
      )}
    </div>
  );
}

export function TypeForm({ t, isNew, builtin, limits, problems, checked, onChange }: {
  t: Creature;
  isNew: boolean;
  builtin: boolean;
  limits: Limits;
  problems: Problem[];
  checked: Checked | null;
  onChange: (t: Creature) => void;
}) {
  const [more, setMore] = useState(false);
  const r = (k: string) => limits.ranges.find((x) => x.key === k) ?? { min: 0, max: 1000 };
  const set = <K extends keyof Creature>(k: K, v: Creature[K]) => onChange({ ...t, [k]: v });
  const p = (...fields: string[]) => problems.filter((x) => fields.includes(x.field));
  const look = useQuery({
    queryKey: ["tools", "creatures", "look", t.model, t.anim.join(",")],
    queryFn: ({ signal }) => lookModel(t.model, t.anim, signal),
    enabled: !!t.model.trim(),
    staleTime: 30_000,
  });
  const n = (k: keyof Creature, label: string, opts: { step?: number; unit?: string; sliderMax?: number } = {}) => (
    <Num value={t[k] as number} onChange={(v) => set(k, v as never)} min={r(k).min} max={r(k).max} label={label} {...opts} />
  );
  const row = (name: ReactNode, fields: string[], control: ReactNode, note?: ReactNode, info?: ReactNode) => (
    <Row key={typeof name === "string" ? name : fields.join(",")} name={name} note={note} info={info}>
      <span className="cs-control">
        {control}
        <Problems list={p(...fields)} />
      </span>
    </Row>
  );
  const width = Math.round(t.hull_max[0] - t.hull_min[0]);
  const height = Math.round(t.hull_max[2] - t.hull_min[2]);
  const seqNames = look.data?.sequences?.map((s) => s.name) ?? [];
  const ranged = t.attack === "ranged";

  return (
    <div className="cs-form">
      <Problems list={p("", "base")} />
      <section>
        <h4 className="cs-sub">Who it is</h4>
        <Rows bare>
          {row(
            "Type name",
            ["name"],
            <input className="search cs-name" value={t.name} maxLength={limits.maxName} disabled={!isNew} aria-label="Type name" spellCheck={false} onChange={(e) => set("name", e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, "_"))} />,
            builtin ? "Built in: the name stays. Your changes go in creatures.ini" : isNew ? "Small letters, digits and _: commands and modes name it by this" : "Saved types keep their name (duplicate to rename)",
          )}
          {row("Shown name", ["name"], <input className="search cs-name" value={t.display} maxLength={limits.maxDisplay} aria-label="Shown name" onChange={(e) => set("display", e.target.value)} />)}
          {row(
            "Kind",
            ["category"],
            <Segmented label="Kind" values={limits.categories.join(" ")} labels={limits.categories.map((c) => CATEGORY_WORDS[c] ?? c).join("|")} current={t.category} onPick={(v) => set("category", v)} />,
            undefined,
            "What it is, for modes and bots: a boss is announced and bots weigh a ranged alien above a zombie.",
          )}
          {t.base && <Row name="Starts from" note="Anything you don't set comes from this type"><code>{t.base}</code></Row>}
        </Rows>
      </section>

      <section>
        <h4 className="cs-sub">Looks</h4>
        <div className="cs-look">
          <div className="cs-look-preview">
            {look.data?.model ? (
              <Suspense fallback={<p className="muted"><span className="spinner" aria-hidden="true" /> Loading the 3D view…</p>}>
                <CreaturePreview look={look.data} skin={t.skin} tint={isOff(t.glow) ? null : hex(t.glow)} scale={t.scale} />
              </Suspense>
            ) : look.isPending && t.model.trim() ? (
              <p className="muted"><span className="spinner" aria-hidden="true" /> Looking for the model…</p>
            ) : (
              <p className="muted cs-nomodel">{look.data?.modelError ?? "None of its models are in your game, so there's nothing to show."}</p>
            )}
          </div>
          <Rows bare>
            {row("Model", ["model"], <ModelPicker value={t.model} onChange={(v) => set("model", v)} report={checked?.models ?? look.data?.report} />)}
            {row(
              "Tint",
              ["glow"],
              <span className="row cs-tint">
                <Switch on={!isOff(t.glow)} onChange={(on) => set("glow", on ? [160, 40, 40] : [0, 0, 0])} label="Tinted" />
                {!isOff(t.glow) && <input type="color" value={hex(t.glow)} aria-label="Tint colour" onChange={(e) => set("glow", rgb(e.target.value))} />}
              </span>,
              "A glow over the whole model",
            )}
            {row("Size", ["scale"], n("scale", "Size", { step: 0.05, unit: "×" }), "1 is the model's own size")}
            {row("Blood", ["blood"], <Segmented label="Blood" values="red yellow none" labels={Object.values(BLOOD_WORDS).join("|")} current={t.blood} onPick={(v) => set("blood", v)} />)}
          </Rows>
        </div>
      </section>

      <section>
        <h4 className="cs-sub">Toughness and moving</h4>
        <Rows bare>
          {row("Health", ["hp"], n("hp", "Health", { sliderMax: 5000, step: 5 }), "A player has 100")}
          {row("Moves", ["moves"], <Switch on={t.moves} onChange={(v) => set("moves", v)} label="Moves" />, t.moves ? "Walks and runs" : "Stays put: a turret")}
          {t.moves && row("Running speed", ["speed"], n("speed", "Running speed", { sliderMax: 600, unit: "u/s" }), "A player with a knife runs 250")}
          {t.moves && row("Walking speed", ["walk_speed"], n("walk_speed", "Walking speed", { sliderMax: 400, unit: "u/s" }), "When it has nobody to go for")}
        </Rows>
      </section>

      <section>
        <h4 className="cs-sub">Senses and temper</h4>
        <Rows bare>
          {row("Aggression", ["aggression"], n("aggression", "Aggression", { step: 0.05 }), aggressionWords(t.aggression))}
          {row("Sees up to", ["detect_radius"], n("detect_radius", "Sees up to", { sliderMax: 4000, step: 50, unit: "units" }), "Further when it's already after someone")}
          {row("View cone", ["detect_fov"], n("detect_fov", "View cone", { step: 5, unit: "°" }), t.detect_fov >= 360 ? "Sees all round" : "How wide it looks ahead (360 = all round)")}
          {row("Hearing", ["hearing"], n("hearing", "Hearing", { step: 0.1, sliderMax: 3, unit: "×" }), t.hearing === 0 ? "Deaf" : "Times a bot's hearing: 1 hears what a bot hears")}
        </Rows>
      </section>

      <section>
        <h4 className="cs-sub">Attack</h4>
        <Rows bare>
          {row("Attack", ["attack"], <Segmented label="Attack" values={limits.attacks.join(" ")} labels={limits.attacks.map((a) => ATTACK_WORDS[a] ?? a).join("|")} current={t.attack} onPick={(v) => set("attack", v)} />, ranged ? "Shoots: a trace with a tracer" : t.attack === "melee" ? "Hits up close, with a wind-up" : "Never attacks")}
          {t.attack !== "none" && (
            <>
              {row("Damage", ["damage"], n("damage", "Damage", { sliderMax: 150 }), "Per hit or shot")}
              {row("Reach", ["attack_range"], n("attack_range", "Reach", { sliderMax: ranged ? 3000 : 200, unit: "units" }), ranged ? "How far it shoots" : "How close it must be")}
              {row("Attacks a second", ["attack_rate"], n("attack_rate", "Attacks a second", { step: 0.1, sliderMax: 6 }))}
              {row("Wind-up", ["attack_delay"], n("attack_delay", "Wind-up", { step: 0.05, unit: "s" }), "Time to dodge before it lands")}
              {ranged && row("Spread", ["spread"], n("spread", "Spread", { step: 0.5, unit: "°" }), "0 never misses")}
            </>
          )}
        </Rows>
      </section>

      <section>
        <h4 className="cs-sub">Behaviour</h4>
        <Rows bare>
          {row(
            "Does",
            ["behaviour"],
            <select value={t.behaviour} aria-label="Behaviour" onChange={(e) => set("behaviour", e.target.value)}>
              {limits.behaviours.map((b) => (
                <option key={b} value={b}>
                  {BEHAVIOUR_WORDS[b]?.label ?? b}
                </option>
              ))}
            </select>,
            BEHAVIOUR_WORDS[t.behaviour]?.help,
          )}
          {["wander", "patrol", "guard"].includes(t.behaviour) && row("Home range", ["radius"], n("radius", "Home range", { sliderMax: 2000, step: 25, unit: "units" }), "How far from its spot it goes")}
          {row(
            "Goes for",
            ["target"],
            <select value={t.target} aria-label="Goes for" onChange={(e) => set("target", e.target.value)}>
              {limits.targets.map((x) => (
                <option key={x} value={x}>
                  {TARGET_WORDS[x]?.label ?? x}
                </option>
              ))}
            </select>,
            TARGET_WORDS[t.target]?.help,
          )}
          {row("Enemies", ["hates"], <Segmented label="Enemies" values="all t ct" labels={Object.values(HATES_WORDS).join("|")} current={t.hates} onPick={(v) => set("hates", v)} />, "Who it attacks (modes can set this per creature)")}
          {row("Runs when hurt", ["flee_hp"], <Num value={Math.round(t.flee_hp * 100)} onChange={(v) => set("flee_hp", v / 100)} min={0} max={100} unit="%" label="Runs when hurt" />, t.flee_hp > 0 ? `Runs away below ${Math.round(t.flee_hp * 100)}% health, then comes back` : "Never runs")}
          {row("Comes back", ["respawn"], n("respawn", "Comes back", { sliderMax: 300, unit: "s" }), t.respawn > 0 ? `${t.respawn} s after dying, at its home (map-placed ones)` : "Never (until the next round, if placed in the map)")}
          {row("Body stays", ["corpse_time"], n("corpse_time", "Body stays", { unit: "s" }))}
        </Rows>
      </section>

      <section>
        <h4 className="cs-sub">Rewards</h4>
        <Rows bare>
          {row("Money", ["money"], n("money", "Money", { sliderMax: 3000, step: 50, unit: "$" }), "For whoever kills it (Last Stand pays its own as well)")}
          {row("Score", ["frags"], n("frags", "Score", { sliderMax: 20 }), "Frags for the kill, only when the server has ls_creature_frags 1 (off by default)", "Off by default so creature kills don't end a match through mp_fraglimit.")}
        </Rows>
      </section>

      <section>
        <h4 className="cs-sub">Sounds</h4>
        <Rows bare>
          {SOUND_WORDS.map((s, i) =>
            row(
              s.label,
              [["snd_idle", "snd_alert", "snd_attack", "snd_hit", "snd_miss", "snd_pain", "snd_die"][i]],
              <SoundField label={s.label} value={t.snd[i]} report={checked?.sounds?.[i]} onChange={(v) => onChange({ ...t, snd: t.snd.map((x, j) => (j === i ? v : x)) })} />,
              s.help,
            ),
          )}
        </Rows>
      </section>

      <button type="button" className="btn small cs-more" aria-expanded={more} onClick={() => setMore(!more)}>
        {more ? "Hide the finer points" : "Finer points: animations, body size, skin, stand-in tint"}
      </button>
      {more && (
        <section>
          <Rows bare>
            {ANIM_WORDS.map((a, i) =>
              row(
                `${a} animation`,
                [["anim_idle", "anim_walk", "anim_run", "anim_attack", "anim_flinch", "anim_die"][i]],
                <span className="row">
                  <select value={t.anim[i]} aria-label={`${a} animation`} onChange={(e) => onChange({ ...t, anim: t.anim.map((x, j) => (j === i ? e.target.value : x)) })}>
                    <option value="">Picked by the game</option>
                    {t.anim[i] && !seqNames.includes(t.anim[i]) && <option value={t.anim[i]}>{t.anim[i]} (not in this model)</option>}
                    {seqNames.map((s) => (
                      <option key={s} value={s}>
                        {s}
                      </option>
                    ))}
                  </select>
                </span>,
                look.data?.picks?.[i]?.seq != null && !t.anim[i] ? `The game plays “${look.data.sequences?.[look.data.picks[i].seq!]?.name}”` : undefined,
              ),
            )}
            {row(
              "Body size",
              ["hull_min", "hull_max"],
              <span className="row cs-hull">
                <label>
                  Width <input className="search cs-numbox" type="number" min={8} max={256} value={width} aria-label="Width" onChange={(e) => {
                    const w = Number(e.target.value) / 2;
                    onChange({ ...t, hull_min: [-w, -w, t.hull_min[2]], hull_max: [w, w, t.hull_max[2]] });
                  }} />
                </label>
                <label>
                  Height <input className="search cs-numbox" type="number" min={30} max={256} value={height} aria-label="Height" onChange={(e) => onChange({ ...t, hull_min: [t.hull_min[0], t.hull_min[1], 0], hull_max: [t.hull_max[0], t.hull_max[1], Number(e.target.value)] })} />
                </label>
              </span>,
              "What bullets and walls meet, in units (a player is 32 wide, 72 tall)",
            )}
            {row("Skin", ["skin"], n("skin", "Skin", { sliderMax: 8 }), "For models with more than one")}
            {row("Body", ["body"], n("body", "Body", { sliderMax: 8 }), "For models with parts to choose")}
            {row(
              "Stand-in tint",
              ["fallback_glow"],
              <span className="row cs-tint">
                <Switch on={!isOff(t.fallback_glow)} onChange={(on) => set("fallback_glow", on ? [40, 140, 20] : [0, 0, 0])} label="Tinted" />
                {!isOff(t.fallback_glow) && <input type="color" value={hex(t.fallback_glow)} aria-label="Stand-in tint colour" onChange={(e) => set("fallback_glow", rgb(e.target.value))} />}
              </span>,
              "Only when a stand-in model plays it, so players can tell types apart",
            )}
            {row("All model candidates", ["model"], <input className="search cs-wide" value={t.model} aria-label="All model candidates" spellCheck={false} onChange={(e) => set("model", e.target.value)} />, "Space separated: the first one the game has wins")}
          </Rows>
        </section>
      )}
      <p className="small-note muted">
        <Info text="Changes are saved into creatures.ini in your game (the old file is kept in History). Apply to game makes a running map read it again; new models and sounds wait for the next map." /> What each setting does in the game: docs/CREATURES.md.
      </p>
    </div>
  );
}
