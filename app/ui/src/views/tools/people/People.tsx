// Bot people: who the bots are, edited with real controls instead of in people.ini.
// Every bot is someone, worked out from its name: a kind of person (picked by how common each
// kind is) fixes a few traits, the rest are rolled. Here: the kinds, how common each trait
// is, what never goes together, and the regulars' names (with who each name turns out to be).

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { Banner, BusyButton, Info, LoadError, Segmented, useConfirm, useToast } from "../../../ui";
import { ActionBar } from "../common/ActionBar";
import { Empty, Loading, Row, Rows } from "../common";
import { errorText, useBusy } from "../common/api";
import { checkPeople, exportPeople, importPeople, peopleStatus, restorePeople, savePeople, shippedPeople, type Arch, type Catalog, type NameEdits, type People as PeopleModel, type Problem, type Status, type Who } from "./peopleApi";
import { archLabel, groupLabel, STYLE_NOTES, TRAIT_WORDS, word } from "./words";
import "./people.css";

type Tab = "kinds" | "weights" | "never" | "names";
const NO_EDITS: NameEdits = { renamed: [], removed: [], added: [] };
const PAGE = 200;

const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));

/** A name problem the game would trip on (gm_pubserver's rules), or null. */
function nameProblem(n: string, names: string[]): string | null {
  if (!n.trim()) return "Type a name.";
  if (n.trim() !== n) return "No spaces at the start or end.";
  if (n.includes('"')) return "No quotes: the game couldn't kick that bot.";
  if (/^[;#]/.test(n)) return "Can't start with ; or #.";
  if (new TextEncoder().encode(n).length >= 32) return "Keep it under 32 characters.";
  if (/(^|\D)(19[5-9]\d|20[0-3]\d)(\D|$)/.test(n)) return "No years (1950 to 2039): the game skips those names.";
  if (names.includes(n)) return "That name is already in the list.";
  return null;
}

function ProblemList({ list }: { list: Problem[] }) {
  if (!list.length) return null;
  return (
    <ul className="pp-problems">
      {list.map((p, i) => (
        <li key={i} className={p.error ? "pp-error" : "pp-warning"}>
          {p.error ? "Fix: " : "Note: "}
          {p.text}
        </li>
      ))}
    </ul>
  );
}

/** One kind of person's form. */
function KindForm({ arch, index, people, catalog, groups, problems, who, onChange }: {
  arch: Arch;
  index: number;
  people: PeopleModel;
  catalog: Catalog;
  groups: string[];
  problems: Problem[];
  who: Who[];
  onChange: (a: Arch) => void;
}) {
  const total = people.archetypes.reduce((n, a) => n + a.weight, 0) || 1;
  const share = (arch.weight / total) * 100;
  const fixed = (k: string) => arch.fixed.find((f) => f.key === k)?.value ?? "";
  const setFixed = (k: string, v: string) => {
    const has = arch.fixed.some((f) => f.key === k);
    const fixed = !v ? arch.fixed.filter((f) => f.key !== k) : has ? arch.fixed.map((f) => (f.key === k ? { key: k, value: v } : f)) : [...arch.fixed, { key: k, value: v }];
    onChange({ ...arch, fixed });
  };
  const [newGroup, setNewGroup] = useState(false);
  const examples = who.filter((w) => w.archIndex === index).slice(0, 8);
  return (
    <div className="pp-form">
      <ProblemList list={problems} />
      <Rows bare>
        <Row name="Chats like" info="The chat personality the bot talks with: which lines in botchat.ini it says. Each kind acts as one.">
          <select value={arch.letter} aria-label="Chats like" onChange={(e) => onChange({ ...arch, letter: e.target.value })}>
            {!catalog.letters.some((l) => l.letter === arch.letter) && <option value={arch.letter}>Pick one</option>}
            {catalog.letters.map((l) => (
              <option key={l.letter} value={l.letter}>
                {l.desc[0].toUpperCase() + l.desc.slice(1)}
              </option>
            ))}
          </select>
        </Row>
        <Row name={TRAIT_WORDS.style.label} note={arch.style === "-" ? "Random: any style, by how common each is" : STYLE_NOTES[arch.style]} info={TRAIT_WORDS.style.info}>
          <select value={arch.style} aria-label="Plays like" onChange={(e) => onChange({ ...arch, style: e.target.value })}>
            <option value="-">Any (random)</option>
            {catalog.traits.find((t) => t.key === "style")?.values.map((v) => (
              <option key={v} value={v}>
                {word(v)}
              </option>
            ))}
          </select>
        </Row>
        <Row name="Weight" info="Bigger is more common: each kind's weight against the others' total gives its share of new bots." note={arch.weight === 0 ? "Never picked for a new bot" : `About ${share < 1 ? share.toFixed(1) : Math.round(share)} in every 100 new bots`}>
          <span className="pp-num">
            <input type="range" min={0} max={120} value={Math.min(arch.weight, 120)} aria-label="Weight" onChange={(e) => onChange({ ...arch, weight: Number(e.target.value) })} />
            <input className="search pp-numbox" type="number" min={0} max={1000} value={arch.weight} aria-label="Weight (number)" onChange={(e) => onChange({ ...arch, weight: Math.max(0, Math.min(1000, Math.round(Number(e.target.value) || 0))) })} />
          </span>
        </Row>
        <Row name="Name" note="Letters, digits and _ only. Bots remember their kind by this name">
          <input className="search pp-name" value={arch.name} maxLength={31} aria-label="Name" onChange={(e) => onChange({ ...arch, name: e.target.value.replace(/\s/g, "_") })} />
        </Row>
        <Row name="Group" note="Which group it's listed under, here and in the file">
          {newGroup ? (
            <input className="search" autoFocus placeholder="A new group" aria-label="New group" onBlur={(e) => {
              setNewGroup(false);
              if (e.target.value.trim()) onChange({ ...arch, group: `; ---- ${e.target.value.trim()}` });
            }} onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
          ) : (
            <select value={arch.group} aria-label="Group" onChange={(e) => (e.target.value === "\u0000new" ? setNewGroup(true) : onChange({ ...arch, group: e.target.value }))}>
              {groups.map((g) => (
                <option key={g} value={g}>
                  {groupLabel(g)}
                </option>
              ))}
              <option value={"\u0000new"}>New group…</option>
            </select>
          )}
        </Row>
      </Rows>
      <h4 className="pp-sub">
        Fixed traits <Info text="A fixed trait is the same for every bot of this kind. Left on Random, each bot gets one at random, by how common each value is (the next tab). Fix only what makes the kind what it is: two or three at most." />
      </h4>
      <Rows bare>
        {catalog.traits
          .filter((t) => t.key !== "style")
          .map((t) => (
            <Row key={t.key} name={TRAIT_WORDS[t.key]?.label ?? t.key} info={TRAIT_WORDS[t.key]?.info}>
              {t.values.length > 5 ? (
                <select value={fixed(t.key)} aria-label={TRAIT_WORDS[t.key]?.label ?? t.key} onChange={(e) => setFixed(t.key, e.target.value)}>
                  <option value="">Random</option>
                  {t.values.map((v) => (
                    <option key={v} value={v}>
                      {word(v)}
                    </option>
                  ))}
                </select>
              ) : (
                <Segmented label={TRAIT_WORDS[t.key]?.label ?? t.key} values={["-", ...t.values].join(" ")} labels={["Random", ...t.values.map(word)].join("|")} current={fixed(t.key) || "-"} onPick={(v) => setFixed(t.key, v === "-" ? "" : v)} />
              )}
            </Row>
          ))}
        <Row name="Hobby" info="Something they bring up now and then. Everyone has two; this fixes one of them.">
          <select value={fixed("hobby")} aria-label="Hobby" onChange={(e) => setFixed("hobby", e.target.value)}>
            <option value="">Random</option>
            {catalog.hobbies.map((h) => (
              <option key={h} value={h}>
                {word(h)}
              </option>
            ))}
          </select>
        </Row>
      </Rows>
      {examples.length > 0 && (
        <p className="muted pp-examples">
          Regulars of this kind: {examples.map((w) => w.name).join(", ")}
          {examples.length === 8 ? "…" : ""}
        </p>
      )}
    </div>
  );
}

/** How common each value of a trait is when it's rolled. */
function WeightsTab({ people, catalog, problems, onChange }: { people: PeopleModel; catalog: Catalog; problems: Problem[]; onChange: (p: PeopleModel) => void }) {
  return (
    <div className="pp-weights">
      <p className="muted pp-hint">When a kind of person doesn't fix a trait, each bot gets one at random: the bigger the number, the more bots get that value. Most people sit in the middle; only the extremes stand out.</p>
      {catalog.traits.map((t) => {
        const w = people.weights.find((x) => x.trait === t.key);
        const vals = t.values.map((_, i) => w?.weights[i] ?? 0);
        const sum = vals.reduce((a, b) => a + b, 0) || 1;
        const set = (i: number, n: number) => {
          const next = vals.slice();
          next[i] = Math.max(0, Math.min(1000, Math.round(n) || 0));
          const weights = people.weights.some((x) => x.trait === t.key) ? people.weights.map((x) => (x.trait === t.key ? { ...x, weights: next } : x)) : [...people.weights, { trait: t.key, weights: next }];
          onChange({ ...people, weights });
        };
        return (
          <div className="panel pp-weight" key={t.key}>
            <h4>
              {TRAIT_WORDS[t.key]?.label ?? t.key} <Info text={TRAIT_WORDS[t.key]?.info ?? ""} />
            </h4>
            <ProblemList list={problems.filter((p) => p.where === `weight:${t.key}`)} />
            <div className="pp-bar" aria-hidden="true">
              {vals.map((v, i) => (
                <span key={i} style={{ flexGrow: v }} title={`${word(t.values[i])}: ${Math.round((v / sum) * 100)}%`} />
              ))}
            </div>
            <div className="pp-weight-rows">
              {t.values.map((v, i) => (
                <label key={v} className="pp-weight-row">
                  <span className="pp-weight-name">{word(v)}</span>
                  <input type="range" min={0} max={100} value={Math.min(100, vals[i])} aria-label={`${word(v)}, how common`} onChange={(e) => set(i, Number(e.target.value))} />
                  <span className="muted pp-pct">{Math.round((vals[i] / sum) * 100)}%</span>
                </label>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

/** Pairs that never go together. */
function NeverTab({ people, catalog, problems, onChange }: { people: PeopleModel; catalog: Catalog; problems: Problem[]; onChange: (p: PeopleModel) => void }) {
  const keys = [...catalog.traits.map((t) => t.key), "hobby"];
  const values = (k: string) => (k === "hobby" ? catalog.hobbies : (catalog.traits.find((t) => t.key === k)?.values ?? []));
  const setAt = (i: number, side: "a" | "b", key: string, value: string) => onChange({ ...people, conflicts: people.conflicts.map((c, j) => (j === i ? { ...c, [side]: { key, value } } : c)) });
  const pick = (i: number, side: "a" | "b") => {
    const p = people.conflicts[i][side];
    return (
      <span className="pp-pair">
        <select value={p.key} aria-label="Trait" onChange={(e) => setAt(i, side, e.target.value, values(e.target.value)[0] ?? "")}>
          {keys.map((k) => (
            <option key={k} value={k}>
              {k === "hobby" ? "Hobby" : (TRAIT_WORDS[k]?.label ?? k)}
            </option>
          ))}
        </select>
        <select value={p.value} aria-label="Value" onChange={(e) => setAt(i, side, p.key, e.target.value)}>
          {values(p.key).map((v) => (
            <option key={v} value={v}>
              {word(v)}
            </option>
          ))}
        </select>
      </span>
    );
  };
  return (
    <div className="panel">
      <p className="muted pp-hint">A bot never has both of a pair: a random trait that would clash is picked again. A kid who's an ex-clan pro, a silent troll.</p>
      <ProblemList list={problems.filter((p) => p.where.startsWith("conflict:"))} />
      <div className="pp-never">
        {people.conflicts.map((_, i) => (
          <div className="pp-never-row" key={i}>
            {pick(i, "a")}
            <span className="muted">never with</span>
            {pick(i, "b")}
            <button type="button" className="btn small" aria-label="Remove this pair" onClick={() => onChange({ ...people, conflicts: people.conflicts.filter((__, j) => j !== i) })}>
              Remove
            </button>
          </div>
        ))}
      </div>
      <button type="button" className="btn" onClick={() => onChange({ ...people, conflicts: [...people.conflicts, { a: { key: "age", value: "kid" }, b: { key: "skill", value: "pro" } }] })}>
        Add a pair
      </button>
    </div>
  );
}

export default function People() {
  const toast = useToast();
  const ask = useConfirm();
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["tools", "people"], queryFn: ({ signal }) => peopleStatus(signal) });
  const [people, setPeople] = useState<PeopleModel | null>(null);
  const [loaded, setLoaded] = useState<PeopleModel | null>(null);
  const [names, setNames] = useState<string[]>([]);
  const [edits, setEdits] = useState<NameEdits>(NO_EDITS);
  const [forget, setForget] = useState<string[]>([]);
  const [tab, setTab] = useState<Tab>("kinds");
  const [sel, setSel] = useState(0);
  const [q, setQ] = useState("");
  const [nameQ, setNameQ] = useState("");
  const [newName, setNewName] = useState("");
  const [renaming, setRenaming] = useState<{ from: string; to: string } | null>(null);
  const [problems, setProblems] = useState<Problem[]>([]);
  const [who, setWho] = useState<Who[]>([]);
  const { working, run } = useBusy();

  const reset = (s: Status) => {
    setPeople(clone(s.people));
    setLoaded(clone(s.people));
    setNames(s.names);
    setEdits(NO_EDITS);
    setForget([]);
    setProblems(s.problems);
  };
  useEffect(() => {
    if (status.data) reset(status.data);
  }, [status.dataUpdatedAt]); // eslint-disable-line react-hooks/exhaustive-deps

  const nameMatches = useMemo(() => {
    const n = nameQ.trim().toLowerCase();
    return (n ? names.filter((x) => x.toLowerCase().includes(n)) : names).slice(0, PAGE);
  }, [names, nameQ]);

  // checks and who-is-who, a moment after a change (the names on show, plus a sample for examples)
  useEffect(() => {
    if (!people) return;
    let live = true;
    const t = setTimeout(async () => {
      const sample = Array.from(new Set([...nameMatches, ...names.slice(0, 200)]));
      const r = await checkPeople(people, sample);
      if (!live || r.error) return;
      setProblems(r.problems ?? []);
      setWho(r.who ?? []);
    }, 300);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [people, nameMatches, names]);

  const peopleDirty = !!people && !!loaded && JSON.stringify(people) !== JSON.stringify(loaded);
  const namesDirty = edits.added.length + edits.removed.length + edits.renamed.length > 0;
  const dirty = peopleDirty || namesDirty || forget.length > 0;
  const errors = problems.filter((p) => p.error);

  if (status.isPending) return <Loading text="Reading who the bots are…" />;
  if (status.isError) return <LoadError text={errorText(status.error, "Couldn't read the bots' people.")} onRetry={() => status.refetch()} />;
  const st = status.data;
  if (!st || !people) return <Empty title="No bot people" text="Couldn't read people.ini." />;
  const catalog = st.catalog;

  const groups = Array.from(new Set(people.archetypes.map((a) => a.group)));
  const arch = people.archetypes[Math.min(sel, people.archetypes.length - 1)];
  const filtered = people.archetypes.map((a, i) => ({ a, i })).filter(({ a }) => !q.trim() || (a.name + " " + groupLabel(a.group)).toLowerCase().includes(q.trim().toLowerCase().replace(/ /g, "_")) || archLabel(a.name).toLowerCase().includes(q.trim().toLowerCase()));
  const setArch = (i: number, a: Arch) => setPeople({ ...people, archetypes: people.archetypes.map((x, j) => (j === i ? a : x)) });
  const uniqueName = (base: string) => {
    let n = 2;
    while (people.archetypes.some((a) => a.name === `${base}_${n}`)) n++;
    return `${base}_${n}`.slice(0, 31);
  };
  const add = () => {
    const a: Arch = { name: uniqueName("new_kind"), weight: 10, letter: "c", style: "-", fixed: [], group: arch?.group ?? "" };
    const at = sel + 1;
    setPeople({ ...people, archetypes: [...people.archetypes.slice(0, at), a, ...people.archetypes.slice(at)] });
    setSel(at);
  };
  const duplicate = () => {
    if (!arch) return;
    const a = { ...clone(arch), name: uniqueName(arch.name.replace(/_\d+$/, "")) };
    const at = sel + 1;
    setPeople({ ...people, archetypes: [...people.archetypes.slice(0, at), a, ...people.archetypes.slice(at)] });
    setSel(at);
  };
  const remove = async () => {
    if (!arch || !(await ask(`Remove "${archLabel(arch.name)}"? Regulars of this kind become someone else (from their name) once you save.`, { ok: "Remove", danger: true }))) return;
    setPeople({ ...people, archetypes: people.archetypes.filter((_, j) => j !== sel) });
    setSel(Math.max(0, sel - 1));
  };

  // names: edits are kept as a list of changes, so the file's notes and order stay
  const addName = () => {
    const n = newName;
    if (nameProblem(n, names)) return;
    setNames([...names, n]);
    setEdits((e) => (e.removed.includes(n) ? { ...e, removed: e.removed.filter((x) => x !== n) } : { ...e, added: [...e.added, n] }));
    setNewName("");
  };
  const removeName = (n: string) => {
    setNames(names.filter((x) => x !== n));
    setEdits((e) => (e.added.includes(n) ? { ...e, added: e.added.filter((x) => x !== n) } : { ...e, removed: [...e.removed, n] }));
  };
  const renameName = (from: string, to: string) => {
    setNames(names.map((x) => (x === from ? to : x)));
    setEdits((e) => {
      if (e.added.includes(from)) return { ...e, added: e.added.map((x) => (x === from ? to : x)) };
      // renamed again: one rename from the name on disk
      const orig = e.renamed.find(([, b]) => b === from)?.[0] ?? from;
      return { ...e, renamed: [...e.renamed.filter(([, b]) => b !== from), [orig, to] as [string, string]].filter(([a, b]) => a !== b) };
    });
    setRenaming(null);
  };

  const after = (r: { error?: string; message?: string; status?: Status }, fallback: string) => {
    if (r.error) return toast(r.error, true);
    toast(r.message || fallback);
    if (r.status) qc.setQueryData(["tools", "people"], r.status);
    else qc.invalidateQueries({ queryKey: ["tools", "people"] });
  };
  const save = () => run("Saving…", async () => after(await savePeople(peopleDirty ? people : null, edits, forget), "Saved."));
  const loadInto = (r: { error?: string; people?: PeopleModel; cancelled?: boolean }, words: string) => {
    if (r.cancelled) return;
    if (r.error || !r.people) return toast(r.error || "Couldn't read that.", true);
    setPeople(r.people);
    setSel(0);
    toast(words);
  };
  const fromShipped = async () => {
    if (peopleDirty && !(await ask("Start over from Loner-Strike's own kinds of people? Changes you haven't saved are dropped.", { ok: "Start over" }))) return;
    await run("Loading…", async () => loadInto(await shippedPeople(), "Loner-Strike's own kinds of people are in. Save to use them."));
  };
  const doImport = () => run("Opening…", async () => loadInto(await importPeople(), "Read the file. Look it over, then save to use it."));
  const doExport = () =>
    run("Exporting…", async () => {
      const r = await exportPeople(people);
      if (r.cancelled) return;
      if (r.error) toast(r.error, true);
      else toast(r.message || "Saved.");
    });
  const putBack = async () => {
    if (!(await ask("Put back the people.ini you had before the last save?", { ok: "Put back" }))) return;
    await run("Putting back…", async () => after(await restorePeople(), "Put back."));
  };

  const whoOf = (n: string) => who.find((w) => w.name === n);
  const archProblems = (i: number) => problems.filter((p) => p.where === `arch:${i}`);

  return (
    <div className="pp tl-page">
      {st.newBeside && (
        <Banner action={<BusyButton className="btn" busy={working === "Loading…"} disabled={!!working} label="Start from Loner-Strike's" busyLabel="Loading…" onClick={fromShipped} />}>
          An update put a newer people.ini beside yours (people.ini.new). Yours is kept; start from Loner-Strike's to take the new one.
        </Banner>
      )}
      <div className="tl-head">
        <Segmented label="Part" values="kinds weights never names" labels={`Kinds of people|How common traits are|Never together|Regulars' names`} current={tab} onPick={(v) => setTab(v as Tab)} />
        <span className="muted">For {st.game}</span>
      </div>

      {tab === "kinds" && (
        <div className="pp-kinds">
          <div className="panel pp-list">
            <input className="search" type="search" placeholder="Search kinds" aria-label="Search kinds of people" value={q} onChange={(e) => setQ(e.target.value)} />
            <ul role="listbox" aria-label="Kinds of people">
              {filtered.map(({ a, i }, k) => (
                <li key={i}>
                  {(k === 0 || filtered[k - 1].a.group !== a.group) && <span className="pp-group">{groupLabel(a.group)}</span>}
                  <button type="button" role="option" aria-selected={i === sel} className={`pp-item${i === sel ? " on" : ""}`} onClick={() => setSel(i)}>
                    <span>{archLabel(a.name)}</span>
                    {archProblems(i).some((p) => p.error) ? <span className="pp-dot err" title="Something to fix" /> : <span className="muted pp-w">{a.weight}</span>}
                  </button>
                </li>
              ))}
              {filtered.length === 0 && <li className="muted pp-none">No kind matches.</li>}
            </ul>
            <div className="row pp-list-actions">
              <button type="button" className="btn small" onClick={add}>
                Add
              </button>
              <button type="button" className="btn small" disabled={!arch} onClick={duplicate}>
                Duplicate
              </button>
              <button type="button" className="btn small danger" disabled={!arch || people.archetypes.length <= 1} onClick={remove}>
                Remove
              </button>
            </div>
          </div>
          <div className="panel">
            {arch ? (
              <>
                <h3>{archLabel(arch.name)}</h3>
                <KindForm arch={arch} index={sel} people={people} catalog={catalog} groups={groups} problems={archProblems(sel)} who={who} onChange={(a) => setArch(sel, a)} />
              </>
            ) : (
              <Empty plain title="No kinds of people" text="Add one, or start from Loner-Strike's." />
            )}
          </div>
        </div>
      )}

      {tab === "weights" && <WeightsTab people={people} catalog={catalog} problems={problems} onChange={setPeople} />}
      {tab === "never" && <NeverTab people={people} catalog={catalog} problems={problems} onChange={setPeople} />}

      {tab === "names" && (
        <div className="pp-names-grid">
          <div className="panel">
            <h3>
              Names <span className="muted pp-count">{names.length}</span>
            </h3>
            <p className="muted pp-hint">The names bot regulars get. Each name is always the same person: who they are shows beside it. A name the game has met before keeps the kind it first got.</p>
            <div className="row pp-add">
              <input className="search pp-newname" value={newName} placeholder="Add a name" aria-label="Add a name" maxLength={40} onChange={(e) => setNewName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addName()} />
              <button type="button" className="btn" disabled={!!nameProblem(newName, names)} onClick={addName}>
                Add
              </button>
              {newName && nameProblem(newName, names) && <span className="pp-warning">{nameProblem(newName, names)}</span>}
            </div>
            <input className="search" type="search" placeholder="Search names" aria-label="Search names" value={nameQ} onChange={(e) => setNameQ(e.target.value)} />
            <ul className="pp-names">
              {nameMatches.map((n) => {
                const w = whoOf(n);
                return (
                  <li key={n} className="pp-name-row">
                    {renaming?.from === n ? (
                      <span className="row">
                        <input className="search pp-newname" autoFocus value={renaming.to} aria-label={`New name for ${n}`} onChange={(e) => setRenaming({ from: n, to: e.target.value })} onKeyDown={(e) => {
                          if (e.key === "Escape") setRenaming(null);
                          if (e.key === "Enter" && !nameProblem(renaming.to, names.filter((x) => x !== n))) renameName(n, renaming.to);
                        }} />
                        <button type="button" className="btn small" disabled={!!nameProblem(renaming.to, names.filter((x) => x !== n))} onClick={() => renameName(n, renaming.to)}>
                          Rename
                        </button>
                        {nameProblem(renaming.to, names.filter((x) => x !== n)) && <span className="pp-warning">{nameProblem(renaming.to, names.filter((x) => x !== n))}</span>}
                      </span>
                    ) : (
                      <>
                        <strong className="pp-name-text">{n}</strong>
                        <span className="muted pp-who">
                          {w?.arch ? (
                            <button type="button" className="pp-link" title="Open this kind" onClick={() => {
                              setSel(w.archIndex ?? 0);
                              setTab("kinds");
                            }}>
                              {archLabel(w.arch)}
                            </button>
                          ) : null}
                          {w?.traits && ` · ${word(w.traits.skill)}, ${word(w.traits.style).toLowerCase()}, ${word(w.traits.talk).toLowerCase()}, from ${word(w.traits.region)}`}
                        </span>
                        <span className="pp-name-actions">
                          <button type="button" className="btn small" onClick={() => setRenaming({ from: n, to: n })}>
                            Rename
                          </button>
                          <button type="button" className="btn small" aria-label={`Remove ${n}`} onClick={() => removeName(n)}>
                            Remove
                          </button>
                        </span>
                      </>
                    )}
                  </li>
                );
              })}
            </ul>
            {names.length > 0 && nameMatches.length === PAGE && <p className="muted">Showing the first {PAGE}. Search to find others.</p>}
          </div>
          <div className="panel">
            <h3>Who has which name</h3>
            <p className="muted pp-hint">Each bot profile keeps the name it first got, for good. Hand one a fresh name and it gets a new one from the list next map.</p>
            {st.regulars.length === 0 ? (
              <p className="muted">No bot has a regular name yet. They get one the first time they play.</p>
            ) : (
              <>
                <ul className="pp-regs">
                  {st.regulars.map((r) => {
                    const gone = forget.includes(r.profile);
                    return (
                      <li key={r.profile} className={gone ? "gone" : undefined}>
                        <span className="muted">{r.profile}</span>
                        <strong>{r.name}</strong>
                        <button type="button" className="btn small" onClick={() => setForget(gone ? forget.filter((x) => x !== r.profile) : [...forget, r.profile])}>
                          {gone ? "Keep" : "Fresh name"}
                        </button>
                      </li>
                    );
                  })}
                </ul>
                <button type="button" className="btn" onClick={() => setForget(forget.length === st.regulars.length ? [] : st.regulars.map((r) => r.profile))}>
                  {forget.length === st.regulars.length ? "Keep everyone's" : "Fresh names for everyone"}
                </button>
              </>
            )}
          </div>
        </div>
      )}

      <ActionBar
        dirty={dirty}
        errors={errors.length}
        working={working}
        where={`people.ini for ${st.game}`}
        info="Saved into the game's people.ini, and a copy of the old one is kept first. A running game picks it up straight away; name changes show from the next map. Loner-Strike's updates keep your changes and put their own version beside yours."
        save={{ onClick: save }}
        undo={() => reset(st)}
        more={[
          { label: "Import…", busyLabel: "Opening…", onClick: doImport, note: "Read a people.ini from a file" },
          { label: "Export…", busyLabel: "Exporting…", onClick: doExport, note: "Save these people to a file" },
          { label: "Start from Loner-Strike's", busyLabel: "Loading…", onClick: fromShipped, hidden: st.newBeside, note: "Load the shipped kinds of people to edit (not saved yet)" },
          { label: "Put back the previous file", busyLabel: "Putting back…", onClick: putBack, hidden: !st.canRestore },
        ]}
        reset={{ tool: "people", match: ["addons/amxmodx/configs/gamemodes/people.ini", "addons/amxmodx/configs/gamemodes/regular_names.ini"] }}
      >
        {errors.length > 0 && <ProblemList list={errors.slice(0, 5)} />}
      </ActionBar>
    </div>
  );
}
