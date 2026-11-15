// The editor's Compile panel: SDHLT downloaded on first use (asked first), a preset, an optional
// cordon around what's picked, the compile streamed with plain-language problems that point
// back into the map, the leak line in 3D, and Playtest: compile, put the map in the game, write
// its .res, optionally teach bots the map, and start the game on it with bots.
// Mounted by the editor as a side tab (`<CompilePanel />`); the leak line is drawn by LeakLine.tsx.

import { mapNameProblem } from "./mapName"; // P5
import { goToLeak } from "./LeakTools"; // P5
import { useCallback, useEffect, useRef, useState } from "react";
import { BusyButton, SteamProgress, useConfirm, useToast } from "../../../ui";
import { waitForSetupJob } from "../../../queries";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import * as api from "./compileApi";
import { remixMap } from "../community/communityApi"; // Parity A: Remix it, from a Community map's clash
import type { Job, Preset, Problem } from "./compileApi";
import { originOf, useEditor } from "./store";
import "./compile.css";
import { useNav } from "../../../store";
import { CordonPanel } from "./CordonPanel";
import { cameraPos, cordonOf } from "./extras";
import { madeForOf } from "./kits"; // P5: the modes the map is made for come first
import { useP5 } from "./p5store"; // P5: Playtest from anywhere
import { budgetWarnings } from "./BudgetPanel"; // P5

const PRESETS: { id: Preset; label: string; hint: string }[] = [
  { id: "fast", label: "Fast", hint: "Quick lighting and no visibility pass: for trying things out." },
  { id: "normal", label: "Normal", hint: "Visibility and normal lighting: how most maps are tested." },
  { id: "final", label: "Final", hint: "Full visibility and extra-quality lighting: for release. Slow on big maps." },
];

const KEEP_LINES = 3000;
const mb = (b: number) => `${(b / 1_048_576).toFixed(1)} MB`;
const secs = (s: number) => (s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`);

function pref<T extends string | number | boolean>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    if (v == null) return fallback;
    return (typeof fallback === "number" ? Number(v) : typeof fallback === "boolean" ? v === "1" : v) as T;
  } catch {
    return fallback;
  }
}
function keep(key: string, v: string | number | boolean) {
  try {
    localStorage.setItem(key, typeof v === "boolean" ? (v ? "1" : "0") : String(v));
  } catch {
    /* private window */
  }
}

/** Selects what a problem is about and points the camera at it. */
function goTo(p: Problem) {
  const s = useEditor.getState();
  const look = useTools.getState().look;
  if (p.brush != null && s.brushes.get(p.brush)) {
    const b = s.brushes.get(p.brush)!;
    s.select({ brushes: [p.brush], entities: [], face: null });
    look(b.min.map((x, k) => (x + b.max[k]) / 2) as Vec3, Math.max(64, ...b.max.map((x, k) => x - b.min[k])));
  } else if (p.entity != null) {
    s.select({ brushes: [], entities: [p.entity], face: null });
    const e = s.doc?.entities.find((x) => x.id === p.entity);
    const o = (e && originOf(e.keys)) ?? p.at ?? null;
    if (o) look(o, 64);
  } else if (p.at) {
    look(p.at, 96);
  }
}

export function CompilePanel() {
  const toast = useToast();
  const ask = useConfirm();
  const [status, setStatus] = useState<api.CompileStatus | null>(null);
  const [preset, setPreset] = useState<Preset>(() => pref<string>("ls.compile.preset", "normal") as Preset);
  const [name, setName] = useState("");
  const [job, setJob] = useState<Job | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const [partial, setPartial] = useState("");
  // the playtest's own steps after the compile ("Teaching bots the map…"), null when idle
  const [phase, setPhase] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [modes, setModes] = useState<api.Mode[]>([]);
  const [mode, setMode] = useState<string>(() => pref<string>("ls.compile.mode", "classic"));
  const [bots, setBots] = useState<number>(() => pref<number>("ls.compile.bots", 8));
  const [teach, setTeach] = useState<boolean>(() => pref<boolean>("ls.compile.teach", true));
  // the compiler leaves two cores free (and runs below normal priority) unless asked
  const [allCores, setAllCores] = useState<boolean>(() => pref<boolean>("ls.compile.allCores", false));
  // Parity A: a Community map's name refused, with the two ways on
  const [clash, setClash] = useState<api.CommunityClash | null>(null);
  // Parity A: "Teach bots and send to my servers": the step running, and the summary at the end
  const [outStep, setOutStep] = useState<string | null>(null);
  const [outSummary, setOutSummary] = useState<string[] | null>(null);
  const outStop = useRef(false);
  const following = useRef(false);
  const logBox = useRef<HTMLPreElement>(null);
  const leak = api.useLeak();

  const loadStatus = useCallback(async () => {
    const s = await api.getStatus();
    setStatus(s);
    return s;
  }, []);

  /** Follows the job to its end, streaming its output; the finished job (null: lost the app). */
  const follow = useCallback(async (fresh: boolean): Promise<Job | null> => {
    if (following.current) return null;
    following.current = true;
    let next = 0;
    if (fresh) {
      setLines([]);
      setPartial("");
    }
    let misses = 0;
    try {
      for (;;) {
        const p = await api.getProgress(next);
        if (p.error || !p.job) {
          if (++misses >= 10) return null;
        } else {
          misses = 0;
          const add = p.lines ?? [];
          if (add.length) setLines((old) => [...old, ...add].slice(-KEEP_LINES));
          next = p.next ?? next;
          setPartial(p.partial ?? "");
          setJob(p.job);
          if (p.job.leak) {
            api.useLeak.getState().set(p.job.leak.points);
            void api.useLeak.getState().refresh(); // P5: the hole and the brushes round it
          }
          if (!p.job.running) return p.job;
        }
        await new Promise((r) => setTimeout(r, 500));
      }
    } finally {
      following.current = false;
    }
  }, []);

  // on opening: the status, the modes, and a job already running (from before the panel opened)
  useEffect(() => {
    void loadStatus().then((s) => {
      if (s.busy) void follow(true);
    });
    void api.getModes().then((r) => {
      if (r.modes?.length) setModes([...r.modes].sort((a, b) => a.name.localeCompare(b.name)));
    });
  }, [loadStatus, follow]);

  // P5: the modes the map is made for (Gameplay > Mode kits) first, and picked when the last
  // used mode isn't one of them
  // (the game's mode list has short ids)
  const madeKey = useEditor((s) => madeForOf(s.doc?.world).map((m) => m.id).join(" "));
  const made = madeKey ? madeKey.split(" ") : [];
  const rank = (id: string) => (made.includes(id) ? made.indexOf(id) : made.length);
  const shownModes = [...modes].sort((a, b) => rank(a.id) - rank(b.id));
  useEffect(() => {
    const first = made.find((m) => modes.some((x) => x.id === m));
    if (first && !made.includes(mode)) setMode(first);
  }, [madeKey, modes]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!name && status?.suggestName) setName(status.suggestName);
  }, [status?.suggestName]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    const el = logBox.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 80) el.scrollTop = el.scrollHeight;
  }, [lines, partial]);

  const running = !!job?.running || starting || playing;

  const download = async () => {
    setStarting(true);
    const r = await api.setUpCompiler();
    setStarting(false);
    if (r.error) return toast(r.error, true);
    if (r.installed) return void loadStatus();
    const j = await follow(true);
    await loadStatus();
    if (j?.error) toast(j.error, true);
    else if (j && !j.stopped) toast("SDHLT is ready. You can compile now.");
  };

  /** Starts a compile and follows it; the finished job when it compiled. */
  const compile = async (install: boolean, as?: string): Promise<Job | null> => {
    const n = (as ?? name).trim();
    const why = mapNameProblem(n);
    if (why) {
      toast(why, true);
      return null;
    }
    api.useLeak.getState().set(null);
    // P4: the cordon (boxes kept in the project); a playtest spawns inside it
    const cd = cordonOf(useEditor.getState().doc);
    const cordoned = cd.on && cd.boxes.length > 0;
    const cam = install ? cameraPos() : null;
    setStarting(true);
    const r = await api.startCompile({
      preset,
      name: n || undefined,
      install,
      allCores,
      ...(cordoned ? { cordons: cd.boxes, cordonOptions: { seal: cd.seal, keepLights: cd.keepLights, spawn: install, ...(cam ? { spawnAt: cam } : {}) } } : {}),
    });
    setStarting(false);
    if (r.community) {
      setClash(r.community);
      return null;
    }
    if (r.error) {
      toast(r.error, true);
      return null;
    }
    setClash(null);
    if (r.map && r.map !== name) setName(r.map);
    const j = await follow(true);
    if (!j) {
      toast("Lost touch with the compiler. Try again.", true);
      return null;
    }
    if (j.stopped) {
      toast("Compile stopped.");
      return null;
    }
    if (j.error) {
      toast(j.error, true);
      return null;
    }
    return j;
  };

  const onCompile = async () => {
    const j = await compile(false);
    if (j?.result?.map) toast(`Compiled ${j.result.map} in ${secs(j.seconds)}.`);
  };

  const onPlaytest = async () => {
    // P5: a limit over 90% (Budget panel) is said first, never a silent block
    const over = await budgetWarnings();
    if (over.length && !(await ask(`Near the engine's limits: ${over.join("; ")}. The map may not load or may crash. Playtest anyway? (The Budget panel shows what uses most.)`, { ok: "Playtest anyway" }))) return;
    setPlaying(true);
    try {
      const j = await compile(true);
      const map = j?.result?.map;
      if (!map) return;
      setPhase("Writing the map's .res file…");
      const rr = await api.writeRes(map);
      if (rr.error) toast(`The .res file wasn't written: ${rr.error}`, true);
      if (teach) {
        setPhase("Teaching bots the map (about a minute)…");
        const lr = await api.learnNav(map);
        if (lr.error) {
          toast(`Bots didn't learn ${map} before the playtest: ${lr.error} They'll find their way as they play.`, true);
        } else {
          const done = await waitForSetupJob();
          const said = await api.learnResult();
          // P5: say why (the run's own summary, else the job's error), never a bare "couldn't"
          if (!done) toast(`Bots didn't learn ${map} before the playtest: the app stopped answering while they learned. They'll find their way as they play.`, true);
          else if (done.state === "error") toast(`Bots didn't learn ${map} before the playtest. ${said.result?.message || done.error || "The hidden server stopped without saying why."} They'll find their way as they play.`, true);
        }
      }
      setPhase("Starting the game…");
      const st = await api.gameState();
      const open = !!st.online || !!st.game_open;
      if (open && !(await ask(`The game is open. Close it and start a playtest on ${map}?`, { ok: "Restart and playtest" }))) return;
      const lr = await api.launchGame({ map, bots, mode: modes.length ? mode : undefined, restart: open || undefined });
      if (lr.error) return toast(lr.error, true);
      setPhase("Joining…");
      toast(`Starting the game on ${map} with ${bots} bot${bots === 1 ? "" : "s"}. It joins by itself once the map loads.`);
      await new Promise((r) => setTimeout(r, 2500));
    } finally {
      setPhase(null);
      setPlaying(false);
    }
  };

  const stop = async () => {
    setStopping(true);
    const r = await api.cancelCompile();
    setStopping(false);
    if (r.error) toast(r.error, true);
  };

  // Parity A: your own copy under a free name: the name set, then the same compile and playtest
  const saveAsEdit = async () => {
    const c = clash;
    if (!c) return;
    setClash(null);
    setName(c.editName);
    toast(`Your version is ${c.editName}. ${c.map} stays as Community has it.`);
    setPlaying(true);
    try {
      const j = await compile(true, c.editName);
      if (j?.result?.map) toast(`Compiled ${j.result.map} into the game. Playtest it, or compile again any time.`);
    } finally {
      setPlaying(false);
    }
  };
  const remixIt = async () => {
    const c = clash;
    if (!c) return;
    const r = await remixMap(c.model, c.gb);
    if (r.error) return toast(r.error, true);
    setClash(null);
    toast(`${r.name} is yours to change: a remix of ${c.name}. Publishing it credits the original.`);
  };

  // Parity A: after a Final compile: the map in the game, bots taught, then onto every server
  const sendOut = async () => {
    const map = job?.result?.map;
    if (!map) return;
    outStop.current = false;
    setOutSummary(null);
    const said: string[] = [];
    const stopped = () => outStop.current;
    try {
      if (!job?.result?.installed) {
        setOutStep("Putting the map in the game…");
        const ir = await api.installLast();
        if (ir.error) {
          if (/came from Community/.test(ir.error)) return void (await compile(true));
          said.push(`Stopped: ${ir.error}`);
          return;
        }
        await api.writeRes(map);
      }
      if (stopped()) return void said.push("Stopped before teaching bots.");
      setOutStep("Teaching bots…");
      const lr = await api.learnNav(map);
      if (lr.error) said.push(`Bots weren't taught ${map}: ${lr.error}`);
      else {
        const end = await followJob();
        const r = await api.learnResult();
        if (stopped()) said.push("Teaching bots was stopped.");
        else if (!end || end.state === "error") said.push(`Bots weren't taught ${map}. ${r.result?.message || end?.error || "The hidden server stopped without saying why."}`);
        else said.push(`Bots learned ${map}.`);
      }
      said.push("Map knowledge: the bots build it on each server the first time the map runs (a minute or two). No simulator run was started.");
      if (stopped()) return;
      const list = await api.copyServers(map);
      const to = (list.servers ?? []).filter((x) => !x.missing);
      if (list.error) return void said.push(`Not sent to servers: ${list.error}`);
      if (!to.length) return void said.push("You have no servers set up, so nothing was sent.");
      setOutStep(`Sending to ${to.length} server${to.length === 1 ? "" : "s"}…`);
      const cr = await api.copyToServers(map, to.map((x) => x.id));
      if (cr.error) return void said.push(`Not sent to servers: ${cr.error}`);
      await followJob();
      const out = (await api.copyResult()).result;
      if (out?.message) said.push(out.message);
      for (const f of out?.failed ?? []) said.push(`${f.server}: ${f.why}.`);
    } finally {
      setOutStep(null);
      setOutSummary(said);
    }
  };
  /** The app's setup job (nav learning, a copy) to its end; stops it when asked. */
  const followJob = async (): Promise<api.SetupJob | null> => {
    let seen = false;
    let asked = -1;
    for (let i = 0; i < 3600; i++) {
      await new Promise((r) => setTimeout(r, 800));
      if (outStop.current && asked < 0) {
        asked = i;
        void api.learnStop();
        void api.copyStop();
      }
      // (stopped: a few seconds for the job to wind down, then on to the summary)
      if (asked >= 0 && i - asked > 12) return null;
      const j = await api.setupJob();
      if (j.state === "running") seen = true;
      if (j.state === "done" || j.state === "error") return j;
      if (j.state === "idle" && (seen || i > 10)) return null;
    }
    return null;
  };

  // P5: Playtest asked for from anywhere (F5, Map > Playtest, the editor bar's button)
  const asked = useP5((x) => x.playtestAsked);
  useEffect(() => {
    const p = useP5.getState();
    if (!status || status.error || asked <= p.playtestDone) return;
    p.set({ playtestDone: asked });
    if (running) return toast("A compile is running already.", true);
    void onPlaytest();
    // (its progress shows in the Playtest box: bring it into sight)
    requestAnimationFrame(() => document.querySelector(".compile-playtest")?.scrollIntoView({ block: "nearest" }));
  }, [asked, status]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!status) {
    return (
      <section className="tools-panel">
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Checking the map compiler…
        </p>
      </section>
    );
  }
  if (status.error) {
    return (
      <section className="tools-panel">
        <p className="small-note">{status.error}</p>
        <button type="button" className="btn small" onClick={() => void loadStatus()}>Try again</button>
        <CordonPanel />
      </section>
    );
  }

  const settingUp = job?.kind === "setup" && job.running;
  if (!status.installed) {
    return (
      <section className="tools-panel compile-panel">
        <p>
          Compiling maps needs <strong>SDHLT</strong>, a free GoldSrc map compiler (about {mb(status.bytes)}). Download it?
        </p>
        <p className="small-note muted">{status.licence}</p>
        {settingUp ? (
          <SteamProgress value={job!.fraction} title="Getting the map compiler" text={job!.label} foot="Only this once." />
        ) : (
          <div className="editor-row wrap">
            <BusyButton className="btn" busy={starting} label={`Download SDHLT (${mb(status.bytes)})`} busyLabel="Starting…" disabled={!status.canRun} onClick={() => void download()} />
            <a className="small-note" href="https://github.com/seedee/SDHLT" target="_blank" rel="noreferrer">About SDHLT</a>
          </div>
        )}
        {!status.canRun && status.cantRun && <p className="small-note">{status.cantRun}</p>}
        {job?.kind === "setup" && !job.running && job.error && <p className="small-note compile-bad">{job.error}</p>}
        <CordonPanel />
      </section>
    );
  }

  const compiling = job?.kind === "compile" && job.running;
  const nameWhy = mapNameProblem(name.trim());
  const nameBad = !!nameWhy;
  const problems = job?.kind === "compile" ? job.problems : [];
  const errors = problems.filter((p) => p.level === "error");
  const warnings = problems.filter((p) => p.level === "warning");
  const done = job?.kind === "compile" && !job.running && job.result?.map ? job.result : null;

  return (
    <section className="tools-panel compile-panel">
      {!status.canRun && status.cantRun && <p className="small-note compile-bad">{status.cantRun}</p>}

      <h4 className="compile-head">Compile</h4>
      <label className={`editor-field${nameBad ? " compile-field-bad" : ""}`}>
        Map name
        <input value={name} maxLength={31} spellCheck={false} placeholder={status.suggestName ?? "my_map"} disabled={running} aria-invalid={nameBad} onChange={(e) => setName(e.target.value.replace(/\s/g, "_"))} />
        {nameBad && <span className="compile-bad small-note" role="alert">{nameWhy}</span>}
      </label>

      <div className="seg compile-presets" role="group" aria-label="Quality">
        {PRESETS.map((p) => (
          <button
            key={p.id}
            type="button"
            aria-pressed={preset === p.id}
            title={p.hint}
            disabled={running}
            onClick={() => {
              setPreset(p.id);
              keep("ls.compile.preset", p.id);
            }}
          >
            {p.label}
          </button>
        ))}
      </div>
      <p className="small-note muted">{PRESETS.find((p) => p.id === preset)?.hint}</p>

      <CordonPanel disabled={running} />

      <label className="editor-check compile-check" title="Off: the compiler leaves two cores free and runs at low priority, so the computer (and a running game) stays smooth.">
        <input
          type="checkbox"
          checked={allCores}
          disabled={running}
          onChange={(e) => {
            setAllCores(e.target.checked);
            keep("ls.compile.allCores", e.target.checked);
          }}
        />
        Use all cores (faster, the computer may lag)
      </label>
      {status.cores != null && (
        <p className="small-note muted">
          {allCores ? `Uses all ${status.cores} cores` : `Uses ${status.threads ?? Math.max(1, status.cores - 2)} of ${status.cores} cores`}, at low priority.
        </p>
      )}

      <div className="editor-row wrap">
        <BusyButton className="btn primary" busy={starting && !playing} label="Compile" busyLabel="Starting…" disabled={running || !status.canRun || nameBad} onClick={() => void onCompile()} />
        {compiling && <BusyButton className="btn small" busy={stopping} label="Stop" busyLabel="Stopping…" onClick={() => void stop()} />}
      </div>

      <fieldset className="compile-playtest" disabled={running || !status.canRun}>
        <legend>Playtest</legend>
        <div className="editor-row wrap">
          {modes.length > 0 && (
            <label className="editor-field compile-inline">
              Mode
              <select
                value={modes.some((m) => m.id === mode) ? mode : shownModes[0].id}
                onChange={(e) => {
                  setMode(e.target.value);
                  keep("ls.compile.mode", e.target.value);
                }}
              >
                {shownModes.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                    {made.includes(m.id) ? " (made for)" : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="editor-field compile-inline">
            Bots
            <input
              type="number"
              min={0}
              max={31}
              value={bots}
              onChange={(e) => {
                const n = Math.max(0, Math.min(31, Math.round(Number(e.target.value) || 0)));
                setBots(n);
                keep("ls.compile.bots", n);
              }}
            />
          </label>
        </div>
        <label className="editor-check compile-check" title="A hidden server loads the map and a bot walks it, so bots know their way from the first round.">
          <input
            type="checkbox"
            checked={teach}
            onChange={(e) => {
              setTeach(e.target.checked);
              keep("ls.compile.teach", e.target.checked);
            }}
          />
          Teach bots the map first
        </label>
        <BusyButton className="btn launch" busy={playing} label="Playtest" busyLabel={phase ?? (compiling ? job!.label : "Starting…")} onClick={() => void onPlaytest()} />
        <p className="small-note muted">Compiles, puts the map in your game (backing up one with the same name), and starts it with bots.</p>
      </fieldset>

      {(compiling || phase) && (
        <SteamProgress
          value={phase ? 1 : job?.fraction ?? 0}
          title={phase ? `Playtest: ${name}` : `Compiling ${name}`}
          text={phase ?? (stopping ? "Stopping…" : job?.label)}
          foot={job ? `${secs(job.seconds)} so far` : ""}
        />
      )}

      {done && (
        <p className="small-note">
          Compiled <strong>{done.map}</strong> ({mb(done.bytes ?? 0)}) in {secs(job!.seconds)}.{done.installed ? " It's in the game's maps folder." : ""}
          {done.backup ? " The map that was there is backed up." : ""}
          {done.installed && (
            <>
              {" "}
              <button type="button" className="linklike" title="Make a package to share it in Tools > Community" onClick={() => useNav.getState().go("tools", `community:publish:${done.map}`)}>
                Publish it…
              </button>
            </>
          )}
        </p>
      )}

      {clash && (
        <div className="compile-clash" role="alertdialog" aria-labelledby="compile-clash-text">
          <p id="compile-clash-text">
            <strong>{clash.map} came from Community.</strong> Its updates would replace your changes.
          </p>
          <div className="editor-row wrap">
            <button type="button" className="btn small primary" onClick={() => void saveAsEdit()}>
              Save as {clash.editName}
            </button>
            <button type="button" className="btn small" onClick={() => void remixIt()} title={clash.remixable ? `A remix of ${clash.name}, credited to ${clash.author}` : `${clash.name} was shared without its editable source; Community says so if it can't be remixed`}>
              Remix it
            </button>
            <button type="button" className="btn small" onClick={() => setClash(null)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {done && done.preset === "final" && !running && (
        <div className="compile-sendout">
          {outStep ? (
            <div className="editor-row wrap" aria-live="polite">
              <span className="small-note">
                <span className="spinner" aria-hidden="true" /> {outStep}
              </span>
              <button type="button" className="btn small" onClick={() => (outStop.current = true)}>
                Cancel
              </button>
            </div>
          ) : (
            <button type="button" className="btn small" title="Teaches bots the map, then copies it with its bot files to every server you have (yours only; never the game's own maps)" onClick={() => void sendOut()}>
              Teach bots and send to my servers
            </button>
          )}
          {outSummary && !outStep && (
            <ul className="small-note compile-sendout-summary" aria-live="polite">
              {outSummary.map((l, i) => (
                <li key={i}>{l}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      {problems.length > 0 && (
        <div className="editor-checks compile-problems">
          <strong className="small-note">
            {errors.length ? `${errors.length} problem${errors.length === 1 ? "" : "s"}` : ""}
            {errors.length && warnings.length ? ", " : ""}
            {warnings.length ? `${warnings.length} warning${warnings.length === 1 ? "" : "s"}` : ""}
          </strong>
          <ul>
            {[...errors, ...warnings].map((p, i) => (
              <li key={i} className={`editor-issue ${p.level}`}>
                <button type="button" className="linklike" disabled={p.brush == null && p.entity == null && !p.at} title={p.detail} onClick={() => goTo(p)}>
                  {p.message}
                </button>
                {p.help && <div className="small-note muted">{p.help}</div>}
                {p.kind === "leak" && leak.points && (
                  <div className="editor-row wrap compile-leak">
                    <button type="button" className="btn small primary" title="Fly along the line to the hole and pick the brush beside it" onClick={() => goToLeak()}>
                      Go to leak
                    </button>
                    <button type="button" className="btn small" onClick={() => leak.toggle()}>
                      {leak.visible ? "Hide the leak line" : "Show the leak line"}
                    </button>
                    <button type="button" className="btn small" onClick={() => useTools.getState().look(leak.points![0], 128)}>
                      Go to its start
                    </button>
                    <button type="button" className="btn small" onClick={() => useTools.getState().look(leak.points![leak.points!.length - 1], 128)}>
                      Go to its end
                    </button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {(lines.length > 0 || partial) && (
        <details className="compile-log" open={compiling || undefined}>
          <summary>Compiler output</summary>
          <pre ref={logBox}>
            {lines.join("\n")}
            {partial ? `\n${partial}` : ""}
          </pre>
        </details>
      )}

      <p className="small-note muted compile-foot">
        SDHLT {status.version} ·{" "}
        <a href="https://github.com/seedee/SDHLT" target="_blank" rel="noreferrer">
          its own license
        </a>
      </p>
    </section>
  );
}

export default CompilePanel;

