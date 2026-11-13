// Creature Studio (Beta): the creature types the bot brain's Creature Framework plays (zombies,
// aliens, wildlife, turrets, bosses), edited with real controls instead of in creatures.ini;
// the wave mixes modes use; and "apply to game" / "spawn one" on the player's running map.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useEffect, useMemo, useState } from "react";
import { useDirty } from "../../../store";
import { BusyButton, Info, LoadError, Segmented, useAskText, useConfirm, useToast } from "../../../ui";
import { Empty, Loading } from "../common";
import { ActionBar } from "../common/ActionBar";
import { errorText, useBusy } from "../common/api";
import { matches, searchWords } from "../common/search";
import { useToolCrumbs } from "../common/shell";
import { applyLive, checkTypes, creatureStatus, liveStatus, lookModel, saveTypes, spawnOne, type Checked, type Creature, type Status, type TypeRow } from "./creaturesApi";
import { TypeForm } from "./TypeForm";
import { WavesTab } from "./WavesTab";
import { CATEGORY_WORDS, KIND_WORDS, TEMPLATE_WORDS } from "./words";
import "./creatures.css";

const CreaturePreview = lazy(() => import("./CreaturePreview"));

type Tab = "types" | "waves";
const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));
/** the fields a Creature has (what's saved), from a row that also has builtin/inFile/... */
const plain = (r: TypeRow | Creature): Creature => {
  const { builtin: _b, inFile: _i, set: _s, skipped: _k, summary: _m, ...c } = r as TypeRow;
  return c;
};

export default function Creatures() {
  const toast = useToast();
  const ask = useConfirm();
  const askText = useAskText();
  const qc = useQueryClient();
  const { working, run } = useBusy();
  const status = useQuery({ queryKey: ["tools", "creatures"], queryFn: ({ signal }) => creatureStatus(signal) });
  const live = useQuery({ queryKey: ["tools", "creatures", "live"], queryFn: ({ signal }) => liveStatus(signal), refetchOnWindowFocus: true, staleTime: 5_000 });
  const [tab, setTab] = useState<Tab>("types");
  const [types, setTypes] = useState<Creature[] | null>(null);
  const [saved, setSaved] = useState<Creature[] | null>(null);
  const [sel, setSel] = useState(0);
  const [q, setQ] = useState("");
  const [checked, setChecked] = useState<Checked | null>(null);
  const [calm, setCalm] = useState(true);

  const reset = (st: Status) => {
    const list = st.types.filter((t) => !t.skipped).map(plain);
    setTypes(clone(list));
    setSaved(clone(list));
  };
  useEffect(() => {
    if (status.data && !types) reset(status.data);
  }, [status.data]); // eslint-disable-line react-hooks/exhaustive-deps

  const t = types?.[Math.min(sel, (types?.length ?? 1) - 1)] ?? null;
  const look = useQuery({
    queryKey: ["tools", "creatures", "look", t?.model, t?.anim.join(","), t?.attack === "ranged"],
    queryFn: ({ signal }) => lookModel(t!.model, t!.anim, t!.attack === "ranged", signal),
    enabled: !!t?.model.trim(),
    staleTime: 30_000,
  });
  // problems, a summary and the model/sound report, from the server (its rules are the game's)
  useEffect(() => {
    if (!types) return;
    let live = true;
    const id = setTimeout(async () => {
      const r = await checkTypes(types, t?.name);
      if (live && !r.error) setChecked(r);
    }, 300);
    return () => {
      live = false;
      clearTimeout(id);
    };
  }, [types, t?.name]);

  const dirty = !!types && !!saved && JSON.stringify(types) !== JSON.stringify(saved);
  useDirty("creatures", dirty);
  useToolCrumbs("creatures", tab === "waves" ? [{ label: "Waves" }] : t ? [{ label: t.display || t.name }] : null);

  const filtered = useMemo(() => {
    const w = searchWords(q);
    return (types ?? []).map((x, i) => ({ x, i })).filter(({ x }) => matches(w, x.name, x.display, x.category, CATEGORY_WORDS[x.category] ?? ""));
  }, [types, q]);

  if (status.isPending) return <Loading text="Reading the creature types…" />;
  if (status.isError) return <LoadError text={errorText(status.error, "Couldn't read the creature types.")} onRetry={() => status.refetch()} />;
  const st = status.data;
  if (!types) return null;
  const builtinNames = st.templates.map((x) => x.name);
  const savedNames = (saved ?? []).map((x) => x.name);
  const problems = checked?.problems ?? st.problems;
  const errors = problems.filter((p) => p.error);
  const rowOf = (name: string) => st.types.find((x) => x.name === name);
  const summary = (name: string) => checked?.summaries.find((s) => s.name === name)?.summary ?? rowOf(name)?.summary ?? "";
  const isBuiltin = (name: string) => builtinNames.includes(name);
  const changedFromShipped = (c: Creature) => {
    const b = st.templates.find((x) => x.name === c.name);
    return !!b && JSON.stringify(plain(b)) !== JSON.stringify({ ...c, base: b.base });
  };

  const uniqueName = (base: string) => {
    const stem = base.replace(/_\d+$/, "").slice(0, 27);
    let n = 2;
    while (types.some((x) => x.name === `${stem}_${n}`)) n++;
    return `${stem}_${n}`;
  };
  const addFrom = async (template: string) => {
    const from = types.find((x) => x.name === template) ?? st.templates.find((x) => x.name === template);
    if (!from) return;
    const suggested = uniqueName(`my_${template}`);
    const typed = await askText(`A name for the new type: small letters, digits and _ (it starts as a copy of ${from.display || from.name}). Leave it empty for ${suggested}.`, { label: "Type name", placeholder: suggested, max: 31 }, { ok: "Make it" });
    if (typed === null) return;
    const name = typed.trim() || suggested;
    const clean = name.toLowerCase().replace(/[^a-z0-9_]/g, "_").slice(0, 31);
    if (types.some((x) => x.name === clean)) return toast(`There's already a type called ${clean}.`, true);
    const c: Creature = { ...clone(plain(from)), name: clean, display: (from.display || from.name) + " (copy)", base: isBuiltin(from.name) ? from.name : from.base };
    setTypes([...types, c]);
    setSel(types.length);
  };
  const remove = async () => {
    if (!t) return;
    if (isBuiltin(t.name)) {
      if (!(await ask(`Put ${t.display || t.name} back to how the game has it? Your changes to it go once you save.`, { ok: "Put back" }))) return;
      const b = st.templates.find((x) => x.name === t.name)!;
      setTypes(types.map((x) => (x.name === t.name ? clone(plain(b)) : x)));
      return;
    }
    if (!(await ask(`Remove ${t.display || t.name}? Modes or maps that spawn "${t.name}" won't find it once you save.`, { ok: "Remove", danger: true }))) return;
    setTypes(types.filter((x) => x.name !== t.name));
    setSel(Math.max(0, sel - 1));
  };
  const save = () =>
    run("Saving…", async () => {
      const r = await saveTypes(types);
      if (r.error) return toast(r.error, true);
      toast(r.message || "Saved.");
      if (r.status) {
        qc.setQueryData(["tools", "creatures"], r.status);
        reset(r.status);
      }
    });
  const apply = () =>
    run("Applying…", async () => {
      if (dirty) {
        const r = await saveTypes(types);
        if (r.error) return toast(r.error, true);
        if (r.status) {
          qc.setQueryData(["tools", "creatures"], r.status);
          reset(r.status);
        }
      }
      const r = await applyLive();
      if (r.error) toast(r.error, true);
      else toast(r.message || "Applied.");
      void live.refetch();
    });
  const spawn = () =>
    run("Spawning…", async () => {
      if (!t) return;
      const r = await spawnOne(t.name, calm);
      if (r.error) toast(r.error, true);
      else toast(r.message || "Spawned.");
      void live.refetch();
    });

  const liveText = live.data
    ? !live.data.running
      ? "Your game isn't running a map: start one to try your creatures."
      : !live.data.creatures
        ? "The running game has no creatures (they come with Loner-Strike's bot brain)."
        : `Your game is running${live.data.alive ? ` with ${live.data.alive} creature${live.data.alive === 1 ? "" : "s"} about` : ""}.`
    : "Checking your game…";
  const liveType = t && live.data?.types?.find((x) => x.name === t.name);
  const canSpawn = !!t && !!live.data?.creatures && !dirty && savedNames.includes(t.name);
  const spawnWhy = !live.data?.creatures ? liveText : dirty ? "Save and apply first." : !t || !savedNames.includes(t.name) ? "Save it first." : liveType && !liveType.precached ? "This map didn't load this type: it may refuse." : "";

  const report = checked?.models ?? look.data?.report;
  const first = report?.candidates[report.picked ?? 0];

  return (
    <div className="cs tl-page">
      <div className="tl-head">
        <Segmented label="Part" values="types waves" labels="Creature types|Waves & spawn groups" current={tab} onPick={(v) => setTab(v as Tab)} />
        <span className="muted">For {st.game}</span>
      </div>

      {tab === "waves" && <WavesTab />}

      {tab === "types" && (
        <div className="cs-grid">
          <aside className="panel cs-list" aria-label="Creature types">
            <input className="search" type="search" placeholder="Search types" aria-label="Search creature types" value={q} onChange={(e) => setQ(e.target.value)} />
            <ul role="listbox" aria-label="Creature types">
              {filtered.map(({ x, i }) => {
                const err = problems.some((p) => p.error && p.at === x.name);
                const row = rowOf(x.name);
                return (
                  <li key={x.name + i}>
                    <button type="button" role="option" aria-selected={i === sel} className={`cs-item${i === sel ? " on" : ""}`} onClick={() => setSel(i)}>
                      <span className="cs-item-name">
                        <strong>{x.display || x.name}</strong>
                        <span className="muted">
                          {x.name} · {CATEGORY_WORDS[x.category] ?? x.category}
                        </span>
                      </span>
                      {err ? <span className="cs-dot err" title="Something to fix" /> : isBuiltin(x.name) ? <span className="cs-tag">{row?.inFile || changedFromShipped(x) ? "changed" : "built in"}</span> : <span className="cs-tag own">yours</span>}
                    </button>
                  </li>
                );
              })}
              {filtered.length === 0 && <li className="muted cs-none">No type matches.</li>}
            </ul>
            <select className="cs-new" value="" aria-label="New from a template" onChange={(e) => void addFrom(e.target.value)}>
              <option value="">New from a template…</option>
              {st.templates.map((x) => (
                <option key={x.name} value={x.name}>
                  {TEMPLATE_WORDS[x.name] ?? x.display}
                </option>
              ))}
            </select>
            <div className="cs-list-actions">
              <button type="button" className="btn small" disabled={!t} onClick={() => t && void addFrom(t.name)}>
                Duplicate
              </button>
              <button type="button" className={`btn small${t && !isBuiltin(t.name) ? " danger" : ""}`} disabled={!t || (isBuiltin(t.name) && !changedFromShipped(t) && !rowOf(t.name)?.inFile)} onClick={remove}>
                {t && isBuiltin(t.name) ? "Put back" : "Remove"}
              </button>
            </div>
            <p className="cs-note">
              {types.length} of {st.limits.maxTypes} types <Info text={`The game always has the ${builtinNames.length} built-in types; change them or make your own from them, ${st.limits.maxTypes} in all. A mode spawns a type by its name.`} />
            </p>
          </aside>

          <section className="panel cs-main" aria-label="Edit the type">
            {t ? (
              <>
                <h3>{t.display || t.name}</h3>
                <p className="cs-summary">{summary(t.name)}</p>
                <TypeForm
                  key={t.name + sel}
                  t={t}
                  isNew={!savedNames.includes(t.name)}
                  builtin={isBuiltin(t.name)}
                  limits={st.limits}
                  problems={problems.filter((p) => p.at === t.name)}
                  checked={checked}
                  look={look.data}
                  onChange={(c) => setTypes(types.map((x, j) => (j === sel ? c : x)))}
                />
              </>
            ) : (
              <Empty plain title="No creature types" text="Make one from a template." />
            )}
          </section>

          <aside className="panel cs-side" aria-label="Preview">
            {t && look.data?.model ? (
              <Suspense fallback={<p className="muted"><span className="spinner" aria-hidden="true" /> Loading the 3D view…</p>}>
                <CreaturePreview look={look.data} skin={t.skin} glow={t.glow} fallbackGlow={t.fallback_glow} scale={t.scale} />
              </Suspense>
            ) : look.isFetching ? (
              <p className="muted"><span className="spinner" aria-hidden="true" /> Looking for the model…</p>
            ) : (
              <p className="muted cs-nomodel">{look.data?.modelError ?? "None of its models are in your game, so there's nothing to show."}</p>
            )}
            {first && (
              <p className="cs-note">
                <span className={`cs-kind ${first.kind}`}>{KIND_WORDS[first.kind]}</span> {first.path.replace(/^models\//, "")}
              </p>
            )}
            <div className="cs-try">
              <h4>Try it in your game</h4>
              <p className="cs-note">{liveText}</p>
              <BusyButton className="btn" busy={working === "Spawning…"} disabled={!!working || !canSpawn} label="Spawn one in front of me" busyLabel="Spawning…" onClick={spawn} />
              <label className="tl-check" title="It stands still so you can look at it (it still fights back)">
                <input type="checkbox" checked={calm} onChange={(e) => setCalm(e.target.checked)} /> Standing still
              </label>
              {spawnWhy && spawnWhy !== liveText && <p className="cs-note">{spawnWhy}</p>}
            </div>
          </aside>
        </div>
      )}

      {tab === "types" && (
        <ActionBar
          dirty={dirty}
          errors={errors.length}
          working={working}
          where={errors.length > 0 && errors[0] ? `${errors[0].at ? errors[0].at + ": " : ""}${errors[0].text}` : `creatures.ini for ${st.game}`}
          save={{ onClick: save }}
          undo={() => reset(st)}
          actions={[{ label: dirty ? "Save and apply to game" : "Apply to game", busyLabel: "Applying…", onClick: apply, disabled: errors.length > 0 || !live.data?.creatures, title: live.data?.creatures ? undefined : liveText }]}
          reset={{ tool: "creatures", match: ["addons/amxmodx/configs/gamemodes/creatures.ini"] }}
        >
          {errors.length > 1 && (
            <ul className="cs-problems">
              {errors.slice(1, 4).map((p, i) => (
                <li key={i} className="cs-error">
                  {p.at && <strong>{p.at}: </strong>}
                  {p.text}
                </li>
              ))}
            </ul>
          )}
        </ActionBar>
      )}
    </div>
  );
}
