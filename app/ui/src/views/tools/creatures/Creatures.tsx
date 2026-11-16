// Creature Studio (Beta): the creature types the bot brain's Creature Framework plays (zombies,
// aliens, wildlife, turrets, bosses), edited with real controls instead of in creatures.ini;
// the wave mixes modes use; and "apply to game" / "spawn one" on the player's running map.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useEffect, useMemo, useState, type KeyboardEvent } from "react";
import { useDirty, useNav } from "../../../store";
import { remixCreature } from "../community/communityApi"; // CP-kind
import { DOCS_URL } from "../../DocsLink";
import { BusyButton, Info, LoadError, Segmented, useAskText, useConfirm, useToast } from "../../../ui";
import { Empty, Loading } from "../common";
import { ActionBar } from "../common/ActionBar";
import { errorText, useBusy } from "../common/api";
import { matches, searchWords } from "../common/search";
import { useToolCrumbs } from "../common/shell";
import { applyLive, checkTypes, creatureStatus, LOOKS, liveStatus, lookModel, saveTypes, spawnOne, usedBy, type TypeUser, type Checked, type Creature, type Placed, type Problem, type Status, type TypeRow } from "./creaturesApi";
import { SendToServers } from "./SendToServers";
import { TypeForm } from "./TypeForm";
import { WavesTab } from "./WavesTab";
import { CATEGORY_WORDS, KIND_WORDS, TEMPLATE_WORDS } from "./words";
import "./creatures.css";

const CreaturePreview = lazy(() => import("./CreaturePreview"));

type Tab = "types" | "waves";
const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));
/** Number fields left empty or not a number (they'd be sent as null): "Type a number." under each. */
export function numberProblems(list: Creature[]): Problem[] {
  const out: Problem[] = [];
  for (const c of list)
    for (const [k, v] of Object.entries(c)) if (typeof v === "number" && !Number.isFinite(v)) out.push({ at: c.name, field: k, text: "Type a number.", error: true });
  return out;
}
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
  /** a "Placed by mappers" entry shown instead of a type (its id) */
  const [placedId, setPlacedId] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  /** an installed (community) type shown instead, read only (its name) */
  const [instName, setInstName] = useState<string | null>(null);
  /** the preview as tier 1, 2, 3 or enraged */
  const [as, setAs] = useState<"1" | "2" | "3" | "rage">("1");

  const reset = (st: Status) => {
    const list = st.types.filter((t) => !t.skipped).map(plain);
    setTypes(clone(list));
    setSaved(clone(list));
  };
  useEffect(() => {
    if (status.data && !types) reset(status.data);
  }, [status.data]); // eslint-disable-line react-hooks/exhaustive-deps

  const t = types?.[Math.min(sel, (types?.length ?? 1) - 1)] ?? null;
  const placed = status.data?.placed?.find((x) => x.id === placedId) ?? null;
  const inst = status.data?.installed?.find((x) => x.name === instName) ?? null;
  // a placed turret plays as its type (the user's edits included)
  const placedType = placed?.type ? (types?.find((x) => x.name === placed.type) ?? null) : null;
  const shown = inst ? { model: inst.model, anim: [] as string[], ranged: false } : placed ? { model: placedType?.model ?? placed.model, anim: placedType?.anim ?? [], ranged: placedType?.attack === "ranged" } : t ? { model: t.model, anim: t.anim, ranged: t.attack === "ranged" } : null;
  const look = useQuery({
    queryKey: ["tools", "creatures", "look", shown?.model, shown?.anim.join(","), shown?.ranged],
    queryFn: ({ signal }) => lookModel(shown!.model, shown!.anim, shown!.ranged, signal),
    enabled: !!shown?.model.trim(),
    staleTime: 30_000,
  });
  // problems, a summary and the model/sound report, from the server (its rules are the game's)
  useEffect(() => {
    if (!types || numberProblems(types).length) return;
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
  useToolCrumbs("creatures", tab === "waves" ? [{ label: "Waves" }] : inst ? [{ label: inst.label ?? inst.name }] : placed ? [{ label: placed.title }] : t ? [{ label: t.display || t.name }] : null);

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
  // your own saved types (not the built-ins you changed): what "Send to servers…" can send
  const ownNames = savedNames.filter((n) => !builtinNames.includes(n));
  const blanks = numberProblems(types);
  const problems = [...blanks, ...(checked?.problems ?? st.problems).filter((p) => !blanks.some((b) => b.at === p.at && b.field === p.field))];
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
    const cleanName = (typed: string) =>
      (typed.trim() || suggested)
        .toLowerCase()
        .replace(/[^a-z0-9_]+/g, "_")
        .replace(/_+/g, "_")
        .replace(/^_+|_+$/g, "")
        .slice(0, 31);
    // (the dialog stays open, with what was typed, while the name can't be used)
    const typed = await askText(
      `A name for the new type: small letters, digits and _ (it starts as a copy of ${from.display || from.name}). Leave it empty for ${suggested}.`,
      { label: "Type name", placeholder: suggested, max: 31 },
      { ok: "Make it", check: (v) => (!cleanName(v) ? "Use at least one letter or digit." : types.some((x) => x.name === cleanName(v)) ? `There's already a type called ${cleanName(v)}.` : null) },
    );
    if (typed === null) return;
    const clean = cleanName(typed);
    if (!clean || types.some((x) => x.name === clean)) return;
    if (typed.trim() && clean !== typed.trim()) toast(`Named it ${clean}: small letters, digits and _ only.`);
    // copies are numbered, so the list doesn't hold look-alikes
    const shownBase = (from.display || from.name).replace(/ \(copy( \d+)?\)$/, "");
    let display = `${shownBase} (copy)`;
    for (let n = 2; types.some((x) => x.display === display); n++) display = `${shownBase} (copy ${n})`;
    const c: Creature = { ...clone(plain(from)), name: clean, display, base: isBuiltin(from.name) ? from.name : from.base };
    setTypes([...types, c]);
    setSel(types.length);
  };
  // CP-kind: an installed type copied into your own (credited), then opened
  const remixInstalled = (name: string) =>
    run("Remixing…", async () => {
      const r = await remixCreature(name);
      if (r.error) return toast(r.error, true);
      toast(r.message || "Remixed.");
      const fresh = await qc.fetchQuery({ queryKey: ["tools", "creatures"], queryFn: ({ signal }) => creatureStatus(signal) });
      reset(fresh);
      setInstName(null);
      const i = fresh.types.filter((x) => !x.skipped).findIndex((x) => x.name === r.name);
      if (i >= 0) setSel(i);
    });
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
  const spawnWhy = !live.data?.creatures ? liveText : dirty ? "Save and apply first." : !t || !savedNames.includes(t.name) ? "Save it first." : liveType && !liveType.precached ? "This map didn't load this type. In Last Stand, add it to the waves, then change map." : "";

  const report = placed || inst ? look.data?.report : (checked?.models ?? look.data?.report);
  const pick = (i: number) => {
    setPlacedId(null);
    setInstName(null);
    setSel(i);
  };
  const first = report?.candidates[report.picked ?? 0];
  // the list is one Tab stop; Up/Down/Home/End move through it (like the Demos table)
  const current = filtered.some(({ i }) => i === sel) ? sel : (filtered[0]?.i ?? -1);
  const listKeys = (e: KeyboardEvent<HTMLUListElement>) => {
    const at = filtered.findIndex(({ i }) => i === current);
    const to = e.key === "ArrowDown" ? at + 1 : e.key === "ArrowUp" ? at - 1 : e.key === "Home" ? 0 : e.key === "End" ? filtered.length - 1 : null;
    if (to === null || !filtered.length) return;
    e.preventDefault();
    const next = filtered[Math.max(0, Math.min(filtered.length - 1, to))].i;
    pick(next);
    const ul = e.currentTarget;
    requestAnimationFrame(() => ul.querySelector<HTMLButtonElement>(`[data-i="${next}"]`)?.focus());
  };

  return (
    <div className="cs tl-page">
      <div className="tl-head">
        <Segmented label="Part" values="types waves" labels="Creature types|Waves & spawn groups" current={tab} onPick={(v) => setTab(v as Tab)} />
      </div>
      <p className="cs-target" role="note">
        {st.server ? (
          <>
            Editing creatures for <strong>{st.game}</strong> on this computer. Only that server uses them.
          </>
        ) : (
          <>
            Editing creatures for <strong>{st.game}</strong> on this computer. Only games you host from here use them.
          </>
        )}{" "}
        Changes count after Apply to game or a map change. Nothing is sent to other people's games or servers.{" "}
        <a href={`${DOCS_URL}#creatures-how`} target="_blank" rel="noreferrer">
          How it works ↗
        </a>
      </p>

      <p className="cs-npc-hint muted">
        Making a talking character or something to use? Place an NPC or interactable in the{" "}
        <button type="button" className="link inline" onClick={() => useNav.getState().go("tools", "editor")}>
          map editor
        </button>{" "}
        or{" "}
        <button type="button" className="link inline" onClick={() => useNav.getState().go("tools", "studio")}>
          Game Mode Studio
        </button>
        .
      </p>

      {tab === "waves" && <WavesTab />}

      {tab === "types" && (
        <div className="cs-grid">
          <aside className="panel cs-list" aria-label="Creature types">
            <input className="search" type="search" placeholder="Search types" aria-label="Search creature types" value={q} onChange={(e) => setQ(e.target.value)} />
            <ul role="listbox" aria-label="Creature types" onKeyDown={listKeys}>
              {filtered.map(({ x, i }) => {
                const err = problems.some((p) => p.error && p.at === x.name);
                const row = rowOf(x.name);
                return (
                  <li key={x.name + i}>
                    <button type="button" role="option" data-i={i} tabIndex={i === current ? 0 : -1} aria-selected={!placed && i === sel} className={`cs-item${!placed && i === sel ? " on" : ""}`} onClick={() => pick(i)}>
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
              <button type="button" className={`btn small${t && !isBuiltin(t.name) ? " danger" : ""}`} disabled={!t || (isBuiltin(t.name) && !changedFromShipped(t) && !rowOf(t.name)?.inFile)} title={t && isBuiltin(t.name) && !changedFromShipped(t) && !rowOf(t.name)?.inFile ? "It's as it came with the app: nothing to put back" : undefined} onClick={remove}>
                {t && isBuiltin(t.name) ? "Put back" : "Remove"}
              </button>
              {t && !isBuiltin(t.name) && !st.server && (
                <button
                  type="button"
                  className="btn small"
                  disabled={dirty || !savedNames.includes(t.name)}
                  title={dirty || !savedNames.includes(t.name) ? "Save it first" : "Share it in Community: packed under your name with its model and sounds"}
                  onClick={() => useNav.getState().go("tools", `community:publish:creature:${t.name}`)}
                >
                  Share…
                </button>
              )}
              {!st.server && (
                <button
                  type="button"
                  className="btn small"
                  disabled={dirty || !ownNames.length}
                  title={dirty ? "Save first" : !ownNames.length ? "Make a type of your own first" : "Put your types on your servers, with their models and sounds"}
                  onClick={() => setSending(true)}
                >
                  Send to servers…
                </button>
              )}
            </div>
            {sending && <SendToServers own={ownNames} start={t?.name} onClose={() => setSending(false)} />}
            <p className="cs-note">
              {types.length} of {st.limits.maxTypes} types <Info text={`The game always has the ${builtinNames.length} built-in types; change them or make your own from them, ${st.limits.maxTypes} in all. A mode spawns a type by its name.`} />
            </p>
            {(st.installed?.length ?? 0) > 0 && (
              <div className="cs-placed cs-installed" role="group" aria-label="From Community">
                <h4>
                  From Community <Info text="Types that came with maps, modes or creatures you installed. They're read only here: remix one to make your own copy." />
                </h4>
                <ul role="listbox" aria-label="From Community">
                  {st.installed!.map((x) => (
                    <li key={x.name}>
                      <button type="button" role="option" aria-selected={instName === x.name} className={`cs-item${instName === x.name ? " on" : ""}`} onClick={() => (setPlacedId(null), setInstName(x.name))}>
                        <span className="cs-item-name">
                          <strong>{x.label ?? x.name}</strong>
                          {x.display && <span className="muted">{x.display}</span>}
                        </span>
                        <span className="cs-tag">from Community</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {(st.placed?.length ?? 0) > 0 && (
              <div className="cs-placed" role="group" aria-label="Placed by mappers">
                <h4>
                  Placed by mappers <Info text="Hazards and defenses a map puts in place, added in the map editor (Gameplay or an entity) rather than spawned in waves. Flyers join here when the game has them." />
                </h4>
                <ul role="listbox" aria-label="Placed by mappers">
                  {st.placed!.map((x) => (
                    <li key={x.id}>
                      <button type="button" role="option" aria-selected={placedId === x.id} className={`cs-item${placedId === x.id ? " on" : ""}`} onClick={() => (setInstName(null), setPlacedId(x.id))}>
                        <span className="cs-item-name">
                          <strong>{x.title}</strong>
                          <span className="muted">{x.type ? `${x.entity} · ${x.type}` : x.entity}</span>
                        </span>
                        <span className="cs-tag">map</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            )}
          </aside>

          <section className="panel cs-main" aria-label={inst ? "From Community" : placed ? "Placed by mappers" : "Edit the type"}>
            {inst ? (
              <InstalledView t={inst} onRemix={remixInstalled} />
            ) : placed ? (
              <PlacedView p={placed} onEditType={placedType ? () => pick(types.indexOf(placedType)) : undefined} />
            ) : t ? (
              <>
                <h3>{t.display || t.name}</h3>
                <p className="cs-summary">{summary(t.name)}</p>
                <UsedBy name={t.name} />
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
            {(placed || inst || t) && look.data?.model ? (
              <Suspense fallback={<p className="muted"><span className="spinner" aria-hidden="true" /> Loading the 3D view…</p>}>
                {inst ? (
                  <CreaturePreview key={`inst:${inst.name}`} look={look.data} skin={0} glow={[0, 0, 0]} fallbackGlow={[0, 0, 0]} scale={1} />
                ) : placed ? (
                  <CreaturePreview key={placed.id} look={look.data} skin={placedType?.skin ?? 0} glow={placedType?.glow ?? [0, 0, 0]} fallbackGlow={placedType?.fallback_glow ?? [0, 0, 0]} scale={placedType?.scale ?? 1} />
                ) : (
                  <>
                    <CreaturePreview
                      look={look.data}
                      skin={as === "2" && t!.tier2_skin >= 0 ? t!.tier2_skin : as === "3" && t!.tier3_skin >= 0 ? t!.tier3_skin : t!.skin}
                      glow={t!.glow}
                      fallbackGlow={t!.fallback_glow}
                      scale={t!.scale * (as === "3" ? LOOKS.tier3.scale : 1)}
                      shellOver={as === "2" ? LOOKS.tier2 : as === "3" ? LOOKS.tier3 : as === "rage" ? LOOKS.enraged : null}
                      animScale={as === "rage" ? t!.enrage_anim : 1}
                    />
                    <Segmented label="Show it as" values="1 2 3 rage" labels={`Tier 1|Tier 2|Tier 3|${t!.enrage_below > 0 ? "Enraged" : "Enraged (off)"}`} current={as} onPick={(v) => setAs(v as typeof as)} />
                  </>
                )}
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
            {!placed && !inst && <div className="cs-try">
              <h4>Try it in your game</h4>
              <p className="cs-note">{liveText}</p>
              <BusyButton className="btn" busy={working === "Spawning…"} disabled={!!working || !canSpawn} label="Spawn one in front of me" busyLabel="Spawning…" onClick={spawn} />
              <label className="tl-check" title="It stands still so you can look at it (it still fights back)">
                <input type="checkbox" checked={calm} onChange={(e) => setCalm(e.target.checked)} /> Standing still
              </label>
              {spawnWhy && spawnWhy !== liveText && <p className="cs-note">{spawnWhy}</p>}
            </div>}
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

/** A placed creature: what it does, where to add it, and its map settings (from the FGD). */
function PlacedView({ p, onEditType }: { p: Placed; onEditType?: () => void }) {
  return (
    <>
      <h3>{p.title}</h3>
      <p className="cs-summary">{p.about}</p>
      <p className="banner cs-placed-how">
        <strong>Added in the map editor, not in waves.</strong> {p.how}
      </p>
      {onEditType && (
        <p className="cs-note">
          It plays as the <strong>{p.type}</strong> type: its health, attack, model and sounds are the type's.{" "}
          <button type="button" className="btn small" onClick={onEditType}>
            Edit the type
          </button>
        </p>
      )}
      <h4 className="cs-placed-keys-title">Settings on the entity ({p.entity})</h4>
      <div className="cs-fields">
        {p.keys.map((k) => {
          const def = k.choices?.find((c) => c.value === (k.default ?? ""))?.label ?? (k.default || "the type's own");
          return (
            <div key={k.name} className="cs-field">
              <span className="cs-label">
                <span className="cs-label-name">{k.label || k.name}</span>
                <span className="cs-label-note mono">{k.name}</span>
              </span>
              <span className="cs-ctl">
                <span>
                  Default: <strong>{def}</strong>
                  {k.choices && k.choices.length > 0 && <span className="muted"> ({k.choices.map((c) => c.label).join(", ")})</span>}
                </span>
                {k.help && <span className="cs-label-note">{k.help}</span>}
              </span>
            </div>
          );
        })}
      </div>
    </>
  );
}

const USER_KIND: Record<TypeUser["kind"], string> = { map: "Map", mode: "Mode", waves: "Waves" };

/** Which of your maps, modes and waves use this type (also through another type's base). */
function UsedBy({ name }: { name: string }) {
  const q = useQuery({ queryKey: ["tools", "creatures", "used-by"], queryFn: ({ signal }) => usedBy(signal), staleTime: 30_000 });
  const list = q.data?.usedBy?.[name] ?? [];
  return (
    <p className="cs-note cs-usedby" aria-label="Used by">
      <strong>Used by:</strong>{" "}
      {q.isPending ? (
        <span className="muted">
          <span className="spinner" aria-hidden="true" /> Looking…
        </span>
      ) : q.isError ? (
        <span className="muted">Couldn't look.</span>
      ) : list.length === 0 ? (
        <span className="muted">none of your maps, modes or waves yet.</span>
      ) : (
        list.map((u, i) => (
          <span key={`${u.kind}:${u.id}`} className="cs-usedby-item">
            {i > 0 && " · "}
            <span className="muted">{USER_KIND[u.kind]}</span> {u.name}
          </span>
        ))
      )}
    </p>
  );
}

/** A type a community item installed: what it is and where from. Read only; "Remix…" (a copy
 *  under your name) goes here once the creature remix lands (CP fork, `onRemix`). */
export function InstalledView({ t, onRemix }: { t: { name: string; label?: string; display: string; base: string; summary: string; from: string[] }; onRemix?: (name: string) => void }) {
  return (
    <>
      <h3>{t.label ?? t.name}</h3>
      <p className="cs-summary">{t.summary}</p>
      <p className="banner cs-placed-how">
        <strong>From Community, read only.</strong> It came with {t.from.length === 1 ? "an item" : `${t.from.length} items`} you installed and goes when {t.from.length === 1 ? "it's" : "they're all"} removed.
        {t.base ? ` It starts from ${t.base}.` : ""}
      </p>
      <UsedBy name={t.name} />
      {onRemix && (
        <button type="button" className="btn" onClick={() => onRemix(t.name)}>
          Remix…
        </button>
      )}
    </>
  );
}
