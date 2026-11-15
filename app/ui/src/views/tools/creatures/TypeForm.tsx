// One creature type's form: a control for every key the bot brain reads from creatures.ini,
// with the game's limits, plain help and the server's problems under the control they're about.
// Sections fold (remembered); every row is the same grid: name and help on the left, the
// control on the right, so controls line up down the page.

import { useState, type ReactNode } from "react";
import { Segmented, Switch } from "../../../ui";
import type { Checked, Creature, Limits, ModelLook, Problem, Vec3 } from "./creaturesApi";
import { ModelPicker, SoundTable } from "./Pickers";
import { ANIM_WORDS, ATTACK_WORDS, BEHAVIOUR_WORDS, BLOOD_WORDS, CATEGORY_WORDS, HATES_WORDS, TARGET_WORDS, aggressionWords } from "./words";

export const hex = (c: Vec3) => "#" + c.map((v) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, "0")).join("");
const rgb = (h: string): Vec3 => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) || 0) as Vec3;
export const isOff = (c: Vec3) => c.every((v) => v === 0);

/** A slider and a number box for one value, within the game's range. */
function Num({ value, onChange, min, max, step = 1, unit, label, sliderMax }: { value: number; onChange: (v: number) => void; min: number; max: number; step?: number; unit?: string; label: string; sliderMax?: number }) {
  const top = sliderMax ?? max;
  return (
    <span className="cs-num">
      <input type="range" min={min} max={top} step={step} value={Number.isFinite(value) ? Math.min(Math.max(value, min), top) : min} aria-label={label} onChange={(e) => onChange(Number(e.target.value))} />
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

/** One row of the grid. */
function Field({ name, note, problems, children }: { name: ReactNode; note?: ReactNode; problems: Problem[]; children: ReactNode }) {
  return (
    <div className="cs-field">
      <div className="cs-label">
        <span className="cs-label-name">{name}</span>
        {note && <span className="cs-label-note">{note}</span>}
      </div>
      <div className="cs-ctl">
        {children}
        <Problems list={problems} />
      </div>
    </div>
  );
}

const OPEN_KEY = "ls.creatures.sections";
function useOpen() {
  const [closed, setClosed] = useState<string[]>(() => {
    try {
      return JSON.parse(localStorage.getItem(OPEN_KEY) || '["Finer points"]');
    } catch {
      return ["Finer points"];
    }
  });
  const toggle = (k: string, open: boolean) => {
    const next = open ? closed.filter((x) => x !== k) : [...closed, k];
    setClosed(next);
    try {
      localStorage.setItem(OPEN_KEY, JSON.stringify(next));
    } catch {
      /* not kept */
    }
  };
  return { isOpen: (k: string) => !closed.includes(k), toggle };
}

function Section({ title, hint, open, onToggle, problems, children }: { title: string; hint?: string; open: boolean; onToggle: (o: boolean) => void; problems: number; children: ReactNode }) {
  return (
    <details className="cs-section" open={open} onToggle={(e) => onToggle((e.currentTarget as HTMLDetailsElement).open)}>
      <summary>
        <span className="cs-section-title">{title}</span>
        {hint && !open && <span className="muted cs-section-hint">{hint}</span>}
        {problems > 0 && <span className="cs-dot err" title="Something to fix in here" />}
      </summary>
      <div className="cs-fields">{children}</div>
    </details>
  );
}

const SND_KEYS = ["snd_idle", "snd_alert", "snd_attack", "snd_hit", "snd_miss", "snd_pain", "snd_die"];
const ANIM_KEYS = ["anim_idle", "anim_walk", "anim_run", "anim_attack", "anim_flinch", "anim_die"];

export function TypeForm({ t, isNew, builtin, limits, problems, checked, look, onChange }: {
  t: Creature;
  isNew: boolean;
  builtin: boolean;
  limits: Limits;
  problems: Problem[];
  checked: Checked | null;
  look: ModelLook | undefined;
  onChange: (t: Creature) => void;
}) {
  const { isOpen, toggle } = useOpen();
  const r = (k: string) => limits.ranges.find((x) => x.key === k) ?? { min: 0, max: 1000 };
  const set = <K extends keyof Creature>(k: K, v: Creature[K]) => onChange({ ...t, [k]: v });
  const p = (...fields: string[]) => problems.filter((x) => fields.includes(x.field));
  const count = (...fields: string[]) => p(...fields).filter((x) => x.error).length;
  const n = (k: keyof Creature, label: string, opts: { step?: number; unit?: string; sliderMax?: number } = {}) => (
    <Num value={t[k] as number} onChange={(v) => set(k, v as never)} min={r(k).min} max={r(k).max} label={label} {...opts} />
  );
  const sec = (title: string, hint: string, fields: string[], body: ReactNode) => (
    <Section title={title} hint={hint} open={isOpen(title)} onToggle={(o) => toggle(title, o)} problems={count(...fields)}>
      {body}
    </Section>
  );
  const width = Math.round(t.hull_max[0] - t.hull_min[0]);
  const height = Math.round(t.hull_max[2] - t.hull_min[2]);
  const seqNames = look?.sequences?.map((s) => s.name) ?? [];
  const ranged = t.attack === "ranged";

  return (
    <div className="cs-form">
      <Problems list={p("", "base")} />
      {sec("Name and kind", `${t.name} · ${CATEGORY_WORDS[t.category] ?? t.category}`, ["name", "display", "category"], (
        <>
          <Field name="Type name" note={builtin ? "Built in: the name stays" : isNew ? "Small letters, digits and _: modes spawn it by this" : "Duplicate to rename a saved type"} problems={p("name")}>
            <input className="search cs-name" value={t.name} maxLength={limits.maxName} disabled={!isNew} aria-label="Type name" spellCheck={false} onChange={(e) => set("name", e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, "_"))} />
          </Field>
          <Field name="Shown name" note="In the Studio and for modes (the game itself doesn't print it yet)" problems={p("display")}>
            <input className="search cs-name" value={t.display} maxLength={limits.maxDisplay} aria-label="Shown name" onChange={(e) => set("display", e.target.value)} />
          </Field>
          <Field name="Kind" note="Bots weigh a boss or ranged alien above a zombie" problems={p("category")}>
            <Segmented label="Kind" values={limits.categories.join(" ")} labels={limits.categories.map((c) => CATEGORY_WORDS[c] ?? c).join("|")} current={t.category} onPick={(v) => set("category", v)} />
          </Field>
          {t.base && (
            <Field name="Starts from" note="What you don't set comes from this type" problems={[]}>
              <code>{t.base}</code>
            </Field>
          )}
        </>
      ))}

      {sec("Look", t.model.split(/\s+/)[0]?.replace(/^models\//, "") ?? "", ["model", "glow", "scale", "blood"], (
        <>
          <Field name="Model" note="Half-Life-style models only: CS player models would stand in the floor. Use an NPC for human characters." problems={p("model")}>
            <ModelPicker value={t.model} onChange={(v) => set("model", v)} report={checked?.models ?? look?.report} />
          </Field>
          <Field name="Tint" note="A glow shell over the model, as the game draws it" problems={p("glow")}>
            <span className="cs-inline">
              <Switch on={!isOff(t.glow)} onChange={(on) => set("glow", on ? [160, 40, 40] : [0, 0, 0])} label="Tinted" />
              {!isOff(t.glow) && <input type="color" value={hex(t.glow)} aria-label="Tint color" onChange={(e) => set("glow", rgb(e.target.value))} />}
            </span>
          </Field>
          <Field name="Size" note="Only how it looks: it still fits where a player fits" problems={p("scale")}>
            {n("scale", "Size", { step: 0.05, unit: "×" })}
          </Field>
          <Field name="Blood" problems={p("blood")}>
            <Segmented label="Blood" values="red yellow none" labels={Object.values(BLOOD_WORDS).join("|")} current={t.blood} onPick={(v) => set("blood", v)} />
          </Field>
        </>
      ))}

      {sec("Health and speed", `${t.hp} health${t.moves ? `, runs ${t.speed}` : ", doesn't move"}`, ["hp", "moves", "speed", "walk_speed"], (
        <>
          <Field name="Health" note="A player has 100" problems={p("hp")}>
            {n("hp", "Health", { sliderMax: 5000, step: 5 })}
          </Field>
          <Field name="Moves" note={t.moves ? "Walks and runs" : "Stays put: a turret"} problems={p("moves")}>
            <Switch on={t.moves} onChange={(v) => set("moves", v)} label="Moves" />
          </Field>
          {t.moves && (
            <>
              <Field name="Running speed" note={look?.noRun ? "This model has no run of its own: the game plays its walk faster" : "A player with a knife runs 250"} problems={p("speed")}>
                {n("speed", "Running speed", { sliderMax: 600, unit: "u/s" })}
              </Field>
              <Field name="Walking speed" note="When it has nobody to go for" problems={p("walk_speed")}>
                {n("walk_speed", "Walking speed", { sliderMax: 400, unit: "u/s" })}
              </Field>
            </>
          )}
        </>
      ))}

      {sec("Senses", `sees ${t.detect_radius} units`, ["aggression", "detect_radius", "detect_fov", "hearing"], (
        <>
          <Field name="Aggression" note={aggressionWords(t.aggression)} problems={p("aggression")}>
            {n("aggression", "Aggression", { step: 0.05 })}
          </Field>
          <Field name="Sees up to" note="1.5× as far once it's after someone" problems={p("detect_radius")}>
            {n("detect_radius", "Sees up to", { sliderMax: 4000, step: 50, unit: "u" })}
          </Field>
          <Field name="View cone" note={t.detect_fov >= 360 ? "Sees all round" : "How wide it looks ahead"} problems={p("detect_fov")}>
            {n("detect_fov", "View cone", { step: 5, unit: "°" })}
          </Field>
          <Field name="Hearing" note={t.hearing === 0 ? "Deaf" : "1 hears what a bot hears"} problems={p("hearing")}>
            {n("hearing", "Hearing", { step: 0.1, sliderMax: 3, unit: "×" })}
          </Field>
        </>
      ))}

      {sec("Attack", t.attack === "none" ? "never attacks" : `${ATTACK_WORDS[t.attack]?.toLowerCase()}, ${t.damage} damage`, ["attack", "damage", "attack_range", "attack_rate", "attack_delay", "spread"], (
        <>
          <Field name="Attack" note={ranged ? "Shoots: a trace with a tracer" : t.attack === "melee" ? "Hits up close, after a wind-up" : "Never attacks"} problems={p("attack")}>
            <Segmented label="Attack" values={limits.attacks.join(" ")} labels={limits.attacks.map((a) => ATTACK_WORDS[a] ?? a).join("|")} current={t.attack} onPick={(v) => set("attack", v)} />
          </Field>
          {t.attack !== "none" && (
            <>
              <Field name="Damage" note="Per hit or shot" problems={p("damage")}>
                {n("damage", "Damage", { sliderMax: 150 })}
              </Field>
              <Field name="Reach" note={ranged ? "How far it shoots" : "How close it must be"} problems={p("attack_range")}>
                {n("attack_range", "Reach", { sliderMax: ranged ? 3000 : 200, unit: "u" })}
              </Field>
              <Field name="Attacks a second" problems={p("attack_rate")}>
                {n("attack_rate", "Attacks a second", { step: 0.1, sliderMax: 6 })}
              </Field>
              <Field name="Wind-up" note="Time to dodge before it lands" problems={p("attack_delay")}>
                {n("attack_delay", "Wind-up", { step: 0.05, unit: "s" })}
              </Field>
              {ranged && (
                <Field name="Spread" note="0 never misses" problems={p("spread")}>
                  {n("spread", "Spread", { step: 0.5, unit: "°" })}
                </Field>
              )}
            </>
          )}
        </>
      ))}

      {sec("Behavior", BEHAVIOUR_WORDS[t.behaviour]?.label ?? t.behaviour, ["behaviour", "radius", "target", "hates", "friendly", "flee_hp", "flee_home", "respawn", "corpse_time"], (
        <>
          <Field name="Does" note={BEHAVIOUR_WORDS[t.behaviour]?.help} problems={p("behaviour")}>
            <select value={t.behaviour} aria-label="Behavior" onChange={(e) => set("behaviour", e.target.value)}>
              {limits.behaviours.map((b) => (
                <option key={b} value={b}>
                  {BEHAVIOUR_WORDS[b]?.label ?? b}
                </option>
              ))}
            </select>
          </Field>
          {["wander", "patrol", "guard"].includes(t.behaviour) && (
            <Field name="Home range" note="How far from its spot it goes" problems={p("radius")}>
              {n("radius", "Home range", { sliderMax: 2000, step: 25, unit: "u" })}
            </Field>
          )}
          <Field name="Goes for" note={TARGET_WORDS[t.target]?.help} problems={p("target")}>
            <select value={t.target} aria-label="Goes for" onChange={(e) => set("target", e.target.value)}>
              {limits.targets.map((x) => (
                <option key={x} value={x}>
                  {TARGET_WORDS[x]?.label ?? x}
                </option>
              ))}
            </select>
          </Field>
          <Field name="On the players' side" note={t.friendly ? "Shoots creatures only; creatures attack it, players and bots never do (a bought turret). Modes can also spawn any type with friendly=1" : "Off: an enemy, like the rest"} problems={p("friendly")}>
            <Switch on={t.friendly} onChange={(v) => set("friendly", v)} label="Friendly" />
          </Field>
          {!t.friendly && (
            <Field name="Enemies" note="Modes can set this per creature" problems={p("hates")}>
              <Segmented label="Enemies" values="all t ct" labels={Object.values(HATES_WORDS).join("|")} current={t.hates} onPick={(v) => set("hates", v)} />
            </Field>
          )}
          <Field name="Runs when hurt" note={t.flee_hp > 0 ? `Below ${Math.round(t.flee_hp * 100)}% health, then comes back` : "Never runs"} problems={p("flee_hp")}>
            <Num value={Math.round(t.flee_hp * 100)} onChange={(v) => set("flee_hp", v / 100)} min={0} max={100} unit="%" label="Runs when hurt" />
          </Field>
          {t.flee_hp > 0 && (
            <Field name="Runs home" note={t.flee_home ? "Back toward where it came in, then into the fight again" : "Anywhere away from the threat (it may range far off)"} problems={p("flee_home")}>
              <Switch on={t.flee_home} onChange={(v) => set("flee_home", v)} label="Runs home" />
            </Field>
          )}
          <Field name="Comes back" note={t.respawn > 0 ? "At its home, for creatures placed in the map" : "Never (map-placed ones come back next round)"} problems={p("respawn")}>
            {n("respawn", "Comes back", { sliderMax: 300, unit: "s" })}
          </Field>
          <Field name="Body stays" problems={p("corpse_time")}>
            {n("corpse_time", "Body stays", { unit: "s" })}
          </Field>
        </>
      ))}

      {sec("Tiers and enrage", t.enrage_below > 0 ? `enraged below ${Math.round(t.enrage_below * 100)}%` : "never enraged", ["enrage_below", "enrage_speed", "enrage_damage", "enrage_anim", "tier2_skin", "tier2_body", "tier2_hp", "tier3_skin", "tier3_body", "tier3_hp"], (
        <>
          <Field name="Enraged when hurt" note={t.enrage_below > 0 ? `Below ${Math.round(t.enrage_below * 100)}% health: a deep roar, a pulsing red glow, faster and harder` : "Never (heavies are, at 40%)"} problems={p("enrage_below")}>
            <Num value={Math.round(t.enrage_below * 100)} onChange={(v) => set("enrage_below", v / 100)} min={0} max={95} unit="%" label="Enraged when hurt" />
          </Field>
          {t.enrage_below > 0 && (
            <>
              <Field name="Enraged speed" note="Times its speeds" problems={p("enrage_speed")}>
                {n("enrage_speed", "Enraged speed", { step: 0.05, unit: "×" })}
              </Field>
              <Field name="Enraged damage" note="Times its damage" problems={p("enrage_damage")}>
                {n("enrage_damage", "Enraged damage", { step: 0.05, unit: "×" })}
              </Field>
              <Field name="Enraged quickness" note="Times its attack rate and how fast it animates" problems={p("enrage_anim")}>
                {n("enrage_anim", "Enraged quickness", { step: 0.05, unit: "×" })}
              </Field>
            </>
          )}
          <p className="cs-note">
            Modes spawn tougher tiers (tier 2: a yellow glow; tier 3: a red-orange glow, a little bigger), the same look on every type. Pick a tier above the preview to see it. A skin or body here only matters where the model has more than one.
          </p>
          {([2, 3] as const).map((tier) => (
            <Field key={tier} name={`Tier ${tier}`} note={`Skin and body (-1: its own), health times (1: the mode decides)`} problems={p(`tier${tier}_skin`, `tier${tier}_body`, `tier${tier}_hp`)}>
              <span className="cs-inline">
                <label className="cs-inline">
                  Skin
                  <input className="search cs-numbox" type="number" min={-1} max={255} value={t[`tier${tier}_skin`]} aria-label={`Tier ${tier} skin`} onChange={(e) => set(`tier${tier}_skin`, Number(e.target.value))} />
                </label>
                <label className="cs-inline">
                  Body
                  <input className="search cs-numbox" type="number" min={-1} max={255} value={t[`tier${tier}_body`]} aria-label={`Tier ${tier} body`} onChange={(e) => set(`tier${tier}_body`, Number(e.target.value))} />
                </label>
                <label className="cs-inline">
                  Health ×
                  <input className="search cs-numbox" type="number" min={0.1} max={20} step={0.1} value={t[`tier${tier}_hp`]} aria-label={`Tier ${tier} health times`} onChange={(e) => set(`tier${tier}_hp`, Number(e.target.value))} />
                </label>
              </span>
            </Field>
          ))}
        </>
      ))}

      {sec("Sounds", `${t.snd.filter((s) => s.trim()).length} of 7 events have sounds`, SND_KEYS, <SoundTable snd={t.snd} reports={checked?.sounds} onChange={(snd) => onChange({ ...t, snd })} />)}

      {sec("Rewards", `$${t.money}, ${t.frags} score`, ["money", "frags"], (
        <>
          <Field name="Money" note="For whoever kills it (Last Stand pays its own too)" problems={p("money")}>
            {n("money", "Money", { sliderMax: 3000, step: 50, unit: "$" })}
          </Field>
          <Field name="Score" note="Only with ls_creature_frags 1 (off, so creature kills don't end a match)" problems={p("frags")}>
            {n("frags", "Score", { sliderMax: 20 })}
          </Field>
        </>
      ))}

      {sec("Finer points", "animations, body size, skin, stand-in tint", [...ANIM_KEYS, "hull_min", "hull_max", "skin", "body", "fallback_glow"], (
        <>
          {ANIM_WORDS.map((a, i) => (
            <Field key={a} name={`${a} animation`} note={look?.picks?.[i]?.seq != null && !t.anim[i] ? `The game plays “${look.sequences?.[look.picks[i].seq!]?.name}”` : undefined} problems={p(ANIM_KEYS[i])}>
              <select value={t.anim[i]} aria-label={`${a} animation`} onChange={(e) => onChange({ ...t, anim: t.anim.map((x, j) => (j === i ? e.target.value : x)) })}>
                <option value="">Picked by the game</option>
                {t.anim[i] && !seqNames.includes(t.anim[i]) && <option value={t.anim[i]}>{t.anim[i]} (not in this model)</option>}
                {seqNames.map((s, k) => (
                  <option key={s + k} value={s}>
                    {s}
                  </option>
                ))}
              </select>
            </Field>
          ))}
          <Field name="Body size" note="What walls and bullets meet. At most a player's 32 wide; 72 tall stands, 36 crouches" problems={p("hull_min", "hull_max")}>
            <span className="cs-inline">
              <label className="cs-inline">
                Width
                <input className="search cs-numbox" type="number" min={8} max={32} value={width} aria-label="Width" onChange={(e) => {
                  const w = Number(e.target.value) / 2;
                  onChange({ ...t, hull_min: [-w, -w, t.hull_min[2]], hull_max: [w, w, t.hull_max[2]] });
                }} />
              </label>
              <label className="cs-inline">
                Height
                <input className="search cs-numbox" type="number" min={30} max={256} value={height} aria-label="Height" onChange={(e) => onChange({ ...t, hull_min: [t.hull_min[0], t.hull_min[1], 0], hull_max: [t.hull_max[0], t.hull_max[1], Number(e.target.value)] })} />
              </label>
            </span>
          </Field>
          <Field name="Skin" note="For models with more than one" problems={p("skin")}>
            {n("skin", "Skin", { sliderMax: 8 })}
          </Field>
          <Field name="Body" note="For models with parts to choose" problems={p("body")}>
            {n("body", "Body", { sliderMax: 8 })}
          </Field>
          <Field name="Stand-in tint" note="Only when the stand-in plays it, to tell types apart" problems={p("fallback_glow")}>
            <span className="cs-inline">
              <Switch on={!isOff(t.fallback_glow)} onChange={(on) => set("fallback_glow", on ? [40, 140, 20] : [0, 0, 0])} label="Tinted" />
              {!isOff(t.fallback_glow) && <input type="color" value={hex(t.fallback_glow)} aria-label="Stand-in tint color" onChange={(e) => set("fallback_glow", rgb(e.target.value))} />}
            </span>
          </Field>
          <Field name="All model candidates" note="Space separated: the first one the game has wins" problems={[]}>
            <input className="search cs-wide" value={t.model} aria-label="All model candidates" spellCheck={false} onChange={(e) => set("model", e.target.value)} />
          </Field>
        </>
      ))}
    </div>
  );
}
