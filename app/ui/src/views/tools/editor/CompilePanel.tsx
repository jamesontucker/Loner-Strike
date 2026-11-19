// The editor's Compile panel: SDHLT downloaded on first use (asked first), a preset, an optional
// cordon around what's picked, the compile streamed with plain-language problems that point
// back into the map, the leak line in 3D, and Playtest: compile, put the map in the game, write
// its .res, optionally teach bots the map, and start the game on it with bots.
// Mounted by the editor as a side tab (`<CompilePanel />`); the leak line is drawn by LeakLine.tsx.

import { mapNameProblem } from "./mapName"; // P5
import { goToLeak } from "./LeakTools"; // P5
import { useCallback, useEffect, useRef, useState } from "react";
import { BusyButton, SteamProgress, useConfirm, useToast } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import * as api from "./compileApi";
import { remixMap } from "../community/communityApi"; // Parity A: Remix it, from a Community map's clash
import { saveSettings } from "./editorApi";
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
const secs = (s: number) => (s < 60 ? `${s} s` : s < 3600 ? `${Math.floor(s / 60)} min ${s % 60} s` : `${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
/** 1.0.1: "just now", "5 min ago", "3 h ago", else the date. */
function ago(when: number) {
  const s = Math.max(0, Math.round(Date.now() / 1000 - when));
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return new Date(when * 1000).toLocaleDateString();
}

// 1.0.1: a playtest's steps, each timed, with what happened
type StepId = "compile" | "res" | "teach" | "game";
type StepState = "waiting" | "running" | "done" | "skipped" | "failed" | "stopped";
type PtStep = { id: StepId; label: string; state: StepState; started?: number; seconds?: number; note?: string };
const STEP_WORDS: Record<StepState, string> = { waiting: "Waiting", running: "Running", done: "Done", skipped: "Skipped", failed: "Didn't work", stopped: "Stopped" };
const STAGE_WORDS: Record<api.StageRow["state"], string> = { waiting: "Waiting", running: "Running", done: "Done", failed: "Failed", stopped: "Stopped" };
/** Maps this many brushes or more count as big when bots learn them (br_valley, 5,426 brushes, took 25 minutes). */
const BIG_MAP = 2000;

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
  // 1.0.1: the playtest's steps (kept on screen after it ends, until the next one), Stop and Skip
  const [steps, setSteps] = useState<PtStep[] | null>(null);
  const stepsRef = useRef<PtStep[]>([]);
  const ptStop = useRef<null | "stop" | "skip">(null);
  const [ptAsked, setPtAsked] = useState<null | "stop" | "skip">(null);
  const [teachDetail, setTeachDetail] = useState("");
  const [choice, setChoice] = useState<{ text: string; yes: string; no: string; done: (yes: boolean) => void } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  // 1.0.1: the map's last compile and playtest, from their logs on disk
  const [last, setLast] = useState<{ map: string; compile?: api.LastCompile | null; playtest?: api.LastPlaytest | null } | null>(null);
  const following = useRef(false);
  // (the last compile asked for got going: not refused before it started)
  const compileStarted = useRef(false);
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
  // 1.0.1: how big the map is, for how long teaching bots may take
  const brushCount = useEditor((s) => s.brushes.size);
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

  // 1.0.1: the running step or stage kept in sight (under the sticky row) as the playtest goes on
  useEffect(() => {
    document.querySelector(".compile-panel .compile-step.is-running, .compile-panel .compile-stage.is-running")?.scrollIntoView({ block: "nearest" });
  }, [steps, job?.stage]);

  const refreshLast = useCallback(async (map: string) => {
    if (!map || mapNameProblem(map)) return setLast(null);
    const r = await api.lastLogs(map);
    setLast(r.error ? null : { map, compile: r.compile, playtest: r.playtest });
  }, []);
  // (the name being typed: asked once it settles)
  const logName = (name.trim() || status?.suggestName || "").trim();
  useEffect(() => {
    const t = setTimeout(() => void refreshLast(logName), 400);
    return () => clearTimeout(t);
  }, [logName, refreshLast]);

  // the steps' times tick while something runs
  const ticking = playing || !!job?.running;
  useEffect(() => {
    if (!ticking) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [ticking]);

  const setStep = (id: StepId, patch: Partial<PtStep>) => {
    stepsRef.current = stepsRef.current.map((x) => (x.id === id ? { ...x, ...patch } : x));
    setSteps(stepsRef.current);
  };
  const startStep = (id: StepId) => setStep(id, { state: "running", started: Date.now() });
  const endStep = (id: StepId, state: StepState, note?: string) => {
    const x = stepsRef.current.find((y) => y.id === id);
    setStep(id, { state, note, seconds: x?.started ? Math.round((Date.now() - x.started) / 1000) : undefined });
  };

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
    compileStarted.current = false;
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
    compileStarted.current = true;
    if (r.map && r.map !== name) setName(r.map);
    const j = await follow(true);
    void refreshLast(r.map || n);
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

  /** 1.0.1: bots taught the map, following the app's job; Stop and Skip end it early. */
  const teachBots = async (map: string, hadNav: boolean): Promise<{ state: StepState; note: string }> => {
    const lr = await api.learnNav(map);
    if (lr.error) return { state: "failed", note: `Bots didn't learn ${map}: ${lr.error} They'll find their way as they play.` };
    let seen = false;
    let asked = -1;
    let idle = 0;
    let misses = 0;
    const early = (want: "stop" | "skip") =>
      want === "skip"
        ? { state: "skipped" as const, note: hadNav ? "Skipped: bots use what they knew of the map before, so they may get lost where you changed it." : "Skipped: bots will wander until they learn the map." }
        : { state: "stopped" as const, note: "Stopped while bots were learning the map." };
    for (let i = 0; ; i++) {
      await sleep(800);
      const want = ptStop.current;
      if (want && asked < 0) {
        asked = i;
        void api.learnStop();
      }
      // (stopped: a few seconds for the hidden server to close, then on)
      if (want && i - asked > 15) return early(want);
      const j = await api.setupJob();
      if (!j.state) {
        if (++misses >= 10) return { state: "failed", note: `Bots didn't learn ${map} before the playtest: the app stopped answering while they learned. They'll find their way as they play.` };
        continue;
      }
      misses = 0;
      if (j.state === "running") {
        seen = true;
        const d = j.steps?.find((x) => x.state === "running")?.detail;
        // (its own "(3 min so far)": the step shows the time)
        if (d) setTeachDetail(d.replace(/\s*\(\d+ min so far\)$/, ""));
      }
      if (j.state === "done" || j.state === "error") {
        if (want) return early(want);
        if (j.state === "done") return { state: "done", note: "Bots learned the map." };
        const said = await api.learnResult();
        // P5: say why (the run's own summary, else the job's error), never a bare "couldn't"
        return { state: "failed", note: `Bots didn't learn ${map}. ${said.result?.message || j.error || "The hidden server stopped without saying why."} They'll find their way as they play.` };
      }
      if (j.state === "idle" && (seen || ++idle > 10)) {
        if (want) return early(want);
        return { state: "failed", note: `Bots didn't learn ${map} before the playtest: the app stopped answering while they learned. They'll find their way as they play.` };
      }
    }
  };

  const onPlaytest = async () => {
    // P5: a limit over 90% (Budget panel) is said first, never a silent block
    const over = await budgetWarnings();
    if (over.length && !(await ask(`Near the engine's limits: ${over.join("; ")}. The map may not load or may crash. Playtest anyway? (The Budget panel shows what uses most.)`, { ok: "Playtest anyway" }))) return;
    ptStop.current = null;
    setPtAsked(null);
    setTeachDetail("");
    stepsRef.current = [
      { id: "compile", label: "Compile", state: "waiting" },
      { id: "res", label: "Write the map's .res file", state: "waiting" },
      { id: "teach", label: "Teach bots the map", state: "waiting" },
      { id: "game", label: "Start the game", state: "waiting" },
    ];
    setSteps(stepsRef.current);
    setPlaying(true);
    const began = Date.now();
    let map = name.trim();
    let outcome: "ok" | "failed" | "stopped" = "failed";
    let learner = false;
    let keepLog = true;
    const stopNow = () => ptStop.current === "stop";
    try {
      startStep("compile");
      const j = await compile(true);
      if (!j?.result?.map) {
        // (refused before it started: a Community map's name, a bad name; its box says why)
        if (!compileStarted.current) {
          keepLog = false;
          setSteps(null);
          return;
        }
        if (stopNow()) {
          outcome = "stopped";
          endStep("compile", "stopped");
        } else endStep("compile", "failed", "The compile didn't finish. Its problems are listed below.");
        return;
      }
      map = j.result.map;
      endStep("compile", "done");
      if (stopNow()) return void (outcome = "stopped");

      startStep("res");
      setPhase("Writing the map's .res file…");
      const rr = await api.writeRes(map);
      if (rr.error) {
        endStep("res", "failed", `The .res file wasn't written: ${rr.error}`);
        toast(`The .res file wasn't written: ${rr.error}`, true);
      } else endStep("res", "done");

      // bots' knowledge: taught when asked for, unless they know this version already; offered when
      // the layout changed since they learned it and teaching is off
      const nav = j.result.nav?.state;
      const hadNav = !!j.result.hasNav;
      let doTeach = teach;
      let skipNote = "";
      if (teach && nav === "current") {
        doTeach = false;
        skipNote = "Bots already know this version of the map.";
      } else if (!teach && nav === "changed") {
        setPhase("Waiting for your answer…");
        doTeach = await new Promise<boolean>((done) =>
          setChoice({
            text: "You changed the map's layout since bots last learned it, so they may get lost. Teach them again first? On big maps it can take 20 minutes or more.",
            yes: "Teach bots",
            no: "Play now",
            done,
          }),
        );
        setChoice(null);
        if (stopNow()) return void (outcome = "stopped");
        if (!doTeach) skipNote = "Skipped: bots use what they knew of the map before, so they may get lost where you changed it.";
      } else if (!teach) {
        skipNote = nav === "none" || !hadNav ? "Skipped (Teach bots is off): bots will wander until they learn the map." : "Skipped (Teach bots is off).";
      }
      if (doTeach) {
        learner = true;
        startStep("teach");
        setPhase("Teaching bots the map…");
        const t = await teachBots(map, hadNav);
        endStep("teach", t.state, t.note);
        setTeachDetail("");
        if (t.state === "stopped") return void (outcome = "stopped");
        if (t.state === "failed") toast(t.note, true);
      } else setStep("teach", { state: "skipped", note: skipNote });
      if (stopNow()) return void (outcome = "stopped");

      startStep("game");
      setPhase("Starting the game…");
      const st = await api.gameState();
      const open = !!st.online || !!st.game_open;
      if (open && !(await ask(`The game is open. Close it and start a playtest on ${map}?`, { ok: "Restart and playtest" }))) {
        endStep("game", "stopped", "The game was left open.");
        return void (outcome = "stopped");
      }
      if (stopNow()) {
        endStep("game", "stopped");
        return void (outcome = "stopped");
      }
      const lr = await api.launchGame({ map, bots, mode: modes.length ? mode : undefined, restart: open || undefined });
      if (lr.error) {
        endStep("game", "failed", lr.error);
        return toast(lr.error, true);
      }
      setPhase("Joining…");
      endStep("game", "done", `With ${bots} bot${bots === 1 ? "" : "s"}. It joins by itself once the map loads.`);
      outcome = "ok";
      toast(`Starting the game on ${map} with ${bots} bot${bots === 1 ? "" : "s"}. It joins by itself once the map loads.`);
      await sleep(2500);
    } finally {
      setPhase(null);
      setPlaying(false);
      setChoice(null);
      setPtAsked(null);
      ptStop.current = null;
      if (keepLog && map && !mapNameProblem(map)) {
        const lines = stepsRef.current.map((x) => `${x.label}: ${x.state === "waiting" ? "not run" : STEP_WORDS[x.state].toLowerCase()}${x.seconds != null ? ` (${secs(x.seconds)})` : ""}${x.note ? `. ${x.note}` : ""}`);
        void api.keepPlaytestLog({ map, outcome, lines, learner, seconds: Math.round((Date.now() - began) / 1000) }).then(() => refreshLast(map));
      }
    }
  };

  const openLog = async (map: string, which: "compile" | "playtest") => {
    const r = await api.openLog(map, which);
    if (r.error) toast(r.error, true);
  };

  /** 1.0.1: Stop, through every step of a playtest (the compile, teaching bots, before the game starts). */
  const stopPlaytest = async () => {
    ptStop.current = "stop";
    setPtAsked("stop");
    if (choice) choice.done(false);
    if (job?.running) {
      const r = await api.cancelCompile();
      if (r.error) toast(r.error, true);
    }
  };
  const skipTeaching = () => {
    ptStop.current = "skip";
    setPtAsked("skip");
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
    // kept in the project, so opening it again compiles to the new name (saved now when it has a file)
    const sr = await saveSettings({ mapName: c.editName });
    if (sr.error) toast(sr.error, true);
    else if (useEditor.getState().doc?.saved) await useEditor.getState().file("save");
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
  const stages = job?.kind === "compile" ? job.stages ?? [] : [];
  const teaching = !!steps?.some((x) => x.id === "teach" && x.state === "running");
  // 1.0.1: the sticky row's words: the running step (a playtest) or stage (a compile), and its time
  const runStep = playing ? steps?.find((x) => x.state === "running") : undefined;
  const runStage = stages.find((r) => r.state === "running");
  const liveName = ptAsked === "stop" || stopping ? "Stopping…" : ptAsked === "skip" ? "Skipping…" : runStep && runStep.id !== "compile" ? runStep.label : runStage ? (playing ? `Compile: ${runStage.name}` : runStage.name) : playing ? phase ?? "Compile" : job?.label ?? "Compiling";
  const liveSecs = runStep && runStep.id !== "compile" ? (runStep.started ? Math.max(0, Math.round((now - runStep.started) / 1000)) : null) : runStage?.seconds ?? (compiling ? job?.seconds ?? null : null);
  const nameWhy = mapNameProblem(name.trim());
  const nameBad = !!nameWhy;
  const problems = job?.kind === "compile" ? job.problems : [];
  const errors = problems.filter((p) => p.level === "error");
  const warnings = problems.filter((p) => p.level === "warning");
  const done = job?.kind === "compile" && !job.running && job.result?.map ? job.result : null;

  return (
    <section className="tools-panel compile-panel">
      {(compiling || playing) && (
        // 1.0.1: what's running and its Stop (and Skip teaching) always in sight, however short the dock
        <div className="compile-live" role="status" aria-label="Running now">
          <span className="compile-live-name">
            <span className="spinner" aria-hidden="true" />
            {liveName}
          </span>
          {liveSecs != null && <span className="compile-live-time">{secs(liveSecs)}</span>}
          <span className="compile-live-actions">
            {playing ? (
              <>
                {teaching && <BusyButton className="btn small" busy={ptAsked === "skip"} label="Skip teaching, play now" busyLabel="Skipping…" disabled={!!ptAsked} title="Starts the game now. Bots will wander until they learn the map." onClick={skipTeaching} />}
                <BusyButton className="btn small" busy={ptAsked === "stop"} label="Stop" busyLabel="Stopping…" disabled={!!ptAsked} onClick={() => void stopPlaytest()} />
              </>
            ) : (
              <BusyButton className="btn small" busy={stopping} label="Stop" busyLabel="Stopping…" onClick={() => void stop()} />
            )}
          </span>
        </div>
      )}
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

      {compiling && (
        <SteamProgress
          value={job?.fraction ?? 0}
          title={playing ? `Playtest: ${name}` : `Compiling ${name}`}
          text={stopping || ptAsked === "stop" ? "Stopping…" : job?.label}
          foot={job ? `${secs(job.seconds)} so far` : ""}
        />
      )}
      {stages.length > 0 && (
        <div className="compile-stages-box">
          <ol className="compile-stages" aria-label="Compile stages">
            {stages.map((r) => (
              <li key={r.id} className={`compile-stage is-${r.state}`}>
                <span className="compile-stage-name">
                  {r.state === "running" && <span className="spinner" aria-hidden="true" />}
                  {r.name}
                </span>
                <span className="compile-stage-state">{STAGE_WORDS[r.state]}</span>
                <span className="compile-stage-time">{r.seconds != null ? secs(r.seconds) : ""}</span>
              </li>
            ))}
          </ol>
          {compiling && <p className="small-note muted">Each stage can take from a few seconds to several minutes, depending on the map's size and detail. Visibility (VIS) is usually the slowest on big maps.</p>}
        </div>
      )}

      {steps && (
        <div className="compile-steps-box" aria-live="polite">
          <strong className="small-note">{playing ? `Playtest: ${name}` : "Playtest steps"}</strong>
          <ol className="compile-steps" aria-label="Playtest steps">
            {steps.map((x) => {
              const t = x.state === "running" && x.started ? Math.max(0, Math.round((now - x.started) / 1000)) : x.seconds;
              return (
                <li key={x.id} className={`compile-step is-${x.state}`}>
                  <div className="compile-step-line">
                    <span className="compile-step-name">
                      {x.state === "running" && <span className="spinner" aria-hidden="true" />}
                      {x.label}
                    </span>
                    <span className="compile-step-state">{x.state === "waiting" && !playing ? "Not run" : STEP_WORDS[x.state]}</span>
                    <span className="compile-step-time">{t != null ? secs(t) : ""}</span>
                  </div>
                  {x.id === "teach" && x.state === "running" && (
                    <div className="small-note muted compile-step-note">
                      {ptAsked === "skip" ? "Skipping…" : ptAsked === "stop" ? "Stopping…" : teachDetail || "Getting a hidden server ready"}
                      {". "}
                      {brushCount >= BIG_MAP ? `Big maps like this one (${brushCount.toLocaleString("en-US")} brushes) can take 20 minutes or more.` : "Usually a few minutes; bigger maps take longer."}
                    </div>
                  )}
                  {x.note && x.state !== "running" && <div className="small-note muted compile-step-note">{x.note}</div>}
                </li>
              );
            })}
          </ol>
          {choice && (
            <div className="compile-clash" role="alertdialog" aria-labelledby="compile-choice-text">
              <p id="compile-choice-text">{choice.text}</p>
              <div className="editor-row wrap">
                <button type="button" className="btn small primary" onClick={() => choice.done(true)}>
                  {choice.yes}
                </button>
                <button type="button" className="btn small" onClick={() => choice.done(false)}>
                  {choice.no}
                </button>
              </div>
            </div>
          )}
          {playing && teaching && !ptAsked && <p className="small-note muted">Skip starts the game now. Bots will wander until they learn the map.</p>}
        </div>
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

      {last && (last.compile || last.playtest) && !running && (
        <div className="compile-last" data-testid="compile-last">
          {last.compile && (
            <div className="compile-last-line">
              <span className="small-note">
                <strong>Last compile</strong> of {last.compile.map}: {last.compile.outcome === "ok" ? "compiled" : last.compile.outcome === "stopped" ? "stopped" : "didn't finish"} {ago(last.compile.when)}
                {last.compile.seconds ? `, took ${secs(last.compile.seconds)}` : ""}
                {last.compile.errors ? `, ${last.compile.errors} problem${last.compile.errors === 1 ? "" : "s"}` : ""}
                {last.compile.warnings ? `, ${last.compile.warnings} warning${last.compile.warnings === 1 ? "" : "s"}` : ""}.
              </span>
              <button type="button" className="linklike small-note" onClick={() => void openLog(last.map, "compile")}>
                Open the full log
              </button>
            </div>
          )}
          {last.compile && !(job?.kind === "compile") && (last.compile.error || (last.compile.problems ?? []).some((p) => p.level === "error")) && (
            <ul className="compile-last-errors small-note">
              {last.compile.error && !(last.compile.problems ?? []).some((p) => p.message === last.compile!.error) && <li className="compile-bad">{last.compile.error}</li>}
              {(last.compile.problems ?? [])
                .filter((p) => p.level === "error")
                .slice(0, 5)
                .map((p, i) => (
                  <li key={i} className="compile-bad">
                    {p.message}
                  </li>
                ))}
            </ul>
          )}
          {last.playtest && (
            <div className="compile-last-line">
              <span className="small-note">
                <strong>Last playtest</strong>: {last.playtest.outcome === "ok" ? "started the game" : last.playtest.outcome === "stopped" ? "stopped" : "didn't start the game"} {ago(last.playtest.when)}.
              </span>
              <button type="button" className="linklike small-note" onClick={() => void openLog(last.map, "playtest")}>
                Open the playtest log
              </button>
            </div>
          )}
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

