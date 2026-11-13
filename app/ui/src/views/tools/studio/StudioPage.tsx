// Game Mode Studio (Beta): a mode's rules as nodes and wires, compiled into an AMX Mod X plugin
// and installed as a mode of the Game Mode Menu; Playtest starts the game in it with bots and
// shows the nodes running. The graph is our own JSON (schema.ts); the app does the checks,
// the Pawn and the compiling (app/src-tauri/src/tools/studio/).

import { useCallback, useEffect, useRef, useState } from "react";
import { ReactFlowProvider, useReactFlow } from "@xyflow/react";
import type { ToolProps } from "../registry";
import { Sheet, useAskText, useConfirm, useToast } from "../../../ui";
import { Empty, Loading } from "../common";
import { busy, errorText } from "../common/api";
import { useToolCrumbs } from "../common/shell";
import { ActionBar } from "../common/ActionBar";
import { useReportAddress } from "../common/address";
import { useToolArg } from "../../../stores/tools";
import { useDirty, useNav } from "../../../store";
import { useStudio, activeBreakpoints } from "./store";
import { Canvas, type CanvasApi } from "./Canvas";
import { builtInSnippets, mySnippets } from "./snippets";
import type { Snippet } from "./graphOps";
import { Inspector } from "./Inspector";
import { MyBlueprint } from "./MyBlueprint";
import { TabBar } from "./TabBar";
import { tabView } from "./graphOps";
import { Results } from "./Results";
import { DebugPanel } from "./DebugPanel";
import { applyLayout, autoLayout } from "./layout";
import { emptyGraph, parseGraph, type Graph } from "./schema";
import rifleRaw from "./templates/team-dm.graph.json?raw";
import lastStandRaw from "./templates/last-stand.graph.json?raw";
import royaleRaw from "./templates/battle-royale.graph.json?raw";
import * as api from "./studioApi";
import { nodeDef } from "./registry";
import "./studio.css";

const PREFS = "ls.studio.playtest";

/** Ready-made modes to start from (shipped laid out). */
const TEMPLATES = [
  { id: "rifle", label: "Rifle Reward", blurb: "Every kill earns the killer a rifle.", raw: rifleRaw },
  { id: "laststand", label: "Last Stand", blurb: "Everyone defends against creature waves; between waves, money and a shop.", raw: lastStandRaw },
  { id: "royale", label: "Battle Royale", blurb: "Everyone for themselves, a shrinking zone, loot; last one standing wins.", raw: royaleRaw },
];

function loadPrefs(): { map: string; bots: number } {
  try {
    const v = JSON.parse(localStorage.getItem(PREFS) ?? "{}");
    return { map: typeof v.map === "string" ? v.map : "de_dust2", bots: Number.isInteger(v.bots) ? v.bots : 9 };
  } catch {
    return { map: "de_dust2", bots: 9 };
  }
}

function savePrefs(p: { map: string; bots: number }) {
  try {
    localStorage.setItem(PREFS, JSON.stringify(p));
  } catch {
    /* private window */
  }
}

/** A mode id from a name: "Rifle Reward" -> "rifle_reward", unique among `taken`. */
function idFrom(name: string, taken: Set<string>): string {
  let base = name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "").slice(0, 20);
  if (!/^[a-z]/.test(base)) base = `mode_${base}`.slice(0, 20);
  if (base.length < 2) base = "my_mode";
  let id = base;
  for (let i = 2; taken.has(id); i++) id = `${base}_${i}`;
  return id;
}

/** Nodes whose title, kind, id or settings have the words. */
function findNodes(g: Graph, q: string) {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  return g.nodes
    .filter((n) => {
      const hay = `${nodeDef(n.type)?.title ?? ""} ${n.type} ${n.id} ${Object.values(n.params).join(" ")}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    })
    .slice(0, 30);
}

function Studio({ maps }: ToolProps) {
  const graph = useStudio((s) => s.graph);
  const saved = useStudio((s) => s.saved);
  const playing = useStudio((s) => s.playing);
  const problems = useStudio((s) => s.problems);
  const history = useStudio((s) => s.history);
  const st = useStudio.getState();
  const toast = useToast();
  const ask = useConfirm();
  const askText = useAskText();
  const rf = useReactFlow();
  const go = useNav((s) => s.go);
  const [list, setList] = useState<api.GraphInfo[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [working, setWorking] = useState<string | null>(null);
  const [code, setCode] = useState<string | null>(null);
  const [prefs, setPrefs] = useState(loadPrefs);
  const focusRef = useRef<(id: string) => void>(() => {});
  const [canvas, setCanvas] = useState<CanvasApi | null>(null);
  const onApi = useCallback((a: CanvasApi) => {
    focusRef.current = a.focus;
    setCanvas(a);
  }, []);
  const [mine, setMine] = useState<Snippet[]>([]);
  const reloadSnippets = useCallback(() => {
    mySnippets().then(setMine, () => setMine([]));
  }, []);
  useEffect(() => reloadSnippets(), [reloadSnippets]);
  const snippets = [...builtInSnippets, ...mine];
  const dirty = !!graph && graph !== saved;
  const funcFocus = useStudio((s) => s.funcFocus);
  const tab = useStudio((s) => s.tab);
  // (the open tab joins the crumbs: a Function's or Macro's graph, or an old Function node looked at)
  const tabName = tab ? graph?.functions?.find((f) => f.id === tab)?.name ?? graph?.macros?.find((m) => m.id === tab)?.name ?? "" : "";
  const tabKind = tab && graph?.macros?.some((m) => m.id === tab) ? "Macro" : "Function";
  const funcName = tabName || (funcFocus ? String(graph?.nodes.find((n) => n.id === funcFocus)?.params.name ?? "") : "");
  useToolCrumbs("studio", graph ? [{ label: graph.mode.name || graph.mode.id, address: funcName ? `studio:${graph.mode.id}` : undefined }, ...(funcName ? [{ label: `${tabName ? tabKind : "Function"}: ${funcName}` }] : [])] : null);

  const [find, setFind] = useState("");
  useDirty("studio", dirty);
  // the open graph is the address ("studio:<id>"), so Back and the crumbs step in and out of it
  // (an open Function's or Macro's tab too: "studio:<id>/<tab>", so Back steps out of it and
  // the mode's crumb goes back to the Event Graph)
  useReportAddress("studio", graph ? `studio:${graph.mode.id}${tab ? `/${tab}` : ""}` : null);
  const arg = useToolArg();

  const refreshList = useCallback(async () => {
    try {
      setList((await api.listGraphs()).graphs);
      setListError(null);
    } catch (e) {
      setListError(errorText(e));
    }
  }, []);
  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  // the app's checks, shortly after each change
  useEffect(() => {
    if (!graph) return;
    const t = setTimeout(async () => {
      const r = await api.validate(graph);
      if (!r.error && r.problems) useStudio.getState().setProblems(r.problems);
    }, 300);
    return () => clearTimeout(t);
  }, [graph]);

  // leaving with unsaved changes
  useEffect(() => {
    if (!dirty) return;
    const warn = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty]);

  // the Playtest's events, read every 150 ms while debugging
  useEffect(() => {
    if (!playing) return;
    let from = 0;
    let stop = false;
    let timer: ReturnType<typeof setTimeout>;
    const tick = async () => {
      const r = await api.debugEvents(playing, from).catch(() => ({ error: "x" }) as { error: string; events?: undefined; next?: undefined; reset?: undefined });
      if (stop) return;
      if (!r.error && r.events) {
        if (r.events.length) useStudio.getState().events(r.events);
        from = r.next ?? from;
      }
      timer = setTimeout(tick, r.events?.length ? 60 : 150);
    };
    void tick();
    return () => {
      stop = true;
      clearTimeout(timer);
    };
  }, [playing]);

  // breakpoints changed while playing: tell the game
  const bpKey = graph ? activeBreakpoints(graph, useStudio.getState().bpsOff).join(" ") : "";
  const bpsOff = useStudio((s) => s.bpsOff);
  const firstBp = useRef(true);
  useEffect(() => {
    if (!playing) {
      firstBp.current = true;
      return;
    }
    if (firstBp.current) {
      firstBp.current = false;
      return;
    }
    void api.debugCommand({ mode: playing, breakpoints: bpKey ? bpKey.split(" ") : [] });
  }, [bpKey, bpsOff, playing]);

  const openId = async (id: string, fromAddress = false) => {
    if (!fromAddress && dirty && !(await ask("This graph has changes that aren't saved. Open another one anyway?", { ok: "Open without saving", danger: true }))) return;
    try {
      const r = await api.openGraph(id);
      const p = parseGraph(r.graph);
      if ("error" in p) toast(p.error, true);
      else st.open(p.graph, true);
    } catch (e) {
      toast(errorText(e), true);
    }
  };

  const newGraph = async (template?: (typeof TEMPLATES)[number]) => {
    if (dirty && !(await ask("This graph has changes that aren't saved. Start a new one anyway?", { ok: "Start a new one", danger: true }))) return;
    const name = await askText("What's the mode called?", { label: "Name", placeholder: template?.label ?? "My mode", max: 40 }, { ok: "Create" });
    if (name == null) return;
    const clean = [...name.trim()].filter((c) => /[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/u.test(c)).join("").slice(0, 40) || template?.label || "My mode";
    const id = idFrom(clean, new Set((list ?? []).map((g) => g.id)));
    let g: Graph;
    if (template) {
      const p = parseGraph(JSON.parse(template.raw));
      if ("error" in p) return toast(p.error, true);
      g = { ...p.graph, mode: { ...p.graph.mode, id, name: clean } };
    } else g = emptyGraph(id, clean);
    // saved straight away: a new mode isn't "unsaved changes"
    const r = await busy("studio-save", "Saving the graph…", () => api.saveGraph(g));
    if (r.error) return toast(r.error, true);
    st.open(g, true);
    void refreshList();
  };

  // the address says which graph is open: open it (Back, a crumb, a link), or the list
  useEffect(() => {
    const cur = useStudio.getState().graph;
    if (!arg) {
      if (cur) useStudio.getState().close();
      return;
    }
    const [gid, tabId = ""] = arg.split("/");
    if (cur?.mode.id === gid) {
      const st = useStudio.getState();
      if (st.tab !== tabId && (!tabId || [...(cur.functions ?? []), ...(cur.macros ?? [])].some((x) => x.id === tabId))) st.openTab(tabId);
      if (!tabId) st.setFuncFocus(null);
      return;
    }
    if (/^[a-z][a-z0-9_]{1,23}$/.test(gid)) void openId(gid, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [arg]);

  const save = async (): Promise<boolean> => {
    const g = useStudio.getState().graph;
    if (!g) return false;
    const r = await busy("studio-save", "Saving the graph…", () => api.saveGraph(g));
    if (r.error) {
      toast(r.error, true);
      return false;
    }
    useStudio.getState().markSaved();
    void refreshList();
    return true;
  };

  const run = async <T,>(label: string, f: () => Promise<T>) => {
    if (working) return undefined;
    setWorking(label);
    try {
      return await busy("studio-work", label, f);
    } finally {
      setWorking(null);
    }
  };

  const showCompile = (r: api.CompileResult): boolean => {
    if (r.error) {
      toast(r.error, true);
      return false;
    }
    if (r.problems) st.setProblems(r.problems);
    if (!r.ok) {
      const first = r.problems?.find((p) => p.level === "error");
      toast(first ? `Not compiled: ${first.message}` : "Not compiled: the compiler said no.", true);
      if (first?.node) focusRef.current(first.node);
      return false;
    }
    return true;
  };

  const install = () =>
    run("Compiling and installing…", async () => {
      const g = useStudio.getState().graph;
      if (!g || !(await save())) return;
      const r = await api.compile(g, false);
      if (r.output) st.setOutput(r.output);
      if (showCompile(r)) toast(`Installed “${g.mode.name}”: pick it on the Play page or in the game's mode menu.`);
      void refreshList();
    });

  // Playtest, step by step: each step says what it's doing and for how long (the first compile
  // and starting the game under CrossOver can take a while); Cancel stops before the next step
  const cancelled = useRef(false);
  const playtest = () =>
    run("Starting the Playtest…", async () => {
      cancelled.current = false;
      const g = useStudio.getState().graph;
      if (!g) return;
      setStep("Saving");
      if (!(await save()) || cancelled.current) return;
      setStep("Compiling (the first time under CrossOver takes longer)");
      const r = await api.playtest(g, activeBreakpoints(g, useStudio.getState().bpsOff));
      if (r.output) st.setOutput(r.output);
      if (!showCompile(r) || cancelled.current) return;
      setStep("Checking the game");
      const state = await api.gameState();
      if (cancelled.current) return;
      if (state.game_open) {
        // the open game switches to the mode with a map change: much quicker than a restart
        if (await ask(`The game is open. Switch it to \u201c${g.mode.name}\u201d on ${prefs.map} now? (A map change; the bots stay as they are.)`, { ok: "Switch the open game" })) {
          setStep("Switching the open game");
          const sw = await api.switchOpenGame(g.mode.id, prefs.map);
          if (!sw.error) {
            st.startPlaying(g.mode.id);
            toast(`Switching to ${prefs.map}. Nodes light up as they run.`);
            return;
          }
          if (!(await ask(`${sw.error} Close the game and start it again in this mode?`, { ok: "Restart and playtest" }))) return;
        } else return;
        setStep("Restarting the game");
        const l = await api.launchGame({ map: prefs.map, bots: prefs.bots, mode: g.mode.id, restart: true });
        if (l.error) return void toast(l.error, true);
      } else {
        setStep("Starting the game");
        const l = await api.launchGame({ map: prefs.map, bots: prefs.bots, mode: g.mode.id });
        if (l.error) return void toast(l.error, true);
      }
      st.startPlaying(g.mode.id);
      toast(`Starting the game on ${prefs.map}. Nodes light up as they run.`);
    }).finally(() => setStep(null));

  const [step, setStepState] = useState<{ label: string; at: number } | null>(null);
  const setStep = (label: string | null) => setStepState(label ? { label, at: Date.now() } : null);
  const [, tick] = useState(0);
  useEffect(() => {
    if (!step) return;
    const t = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, [step]);

  const showCode = () =>
    run("Making the code…", async () => {
      const g = useStudio.getState().graph;
      if (!g) return;
      const r = await api.generate(g, false);
      if (r.error) toast(r.error, true);
      else if (!r.source) toast("Fix the problems on the nodes first.", true);
      else setCode(r.source);
    });

  const openCode = () =>
    run("Opening the code…", async () => {
      const g = useStudio.getState().graph;
      if (!g) return;
      const r = await api.openAsCode(g);
      if (r.error || !r.name) return toast(r.error ?? "Fix the problems on the nodes first.", true);
      setCode(null);
      go("tools", `plugins:${r.folder}/${r.name}`);
    });

  const removeInstalled = async () => {
    const g = useStudio.getState().graph;
    if (!g) return;
    if (!(await ask(`Remove the mode \u201c${g.mode.name}\u201d from the game? The graph is kept.`, { ok: "Remove", danger: true }))) return;
    const r = await api.uninstall(g.mode.id);
    if (r.error) toast(r.error, true);
    else toast("Removed from the game.");
    void refreshList();
  };

  const layout = () => {
    const g = useStudio.getState().graph;
    if (!g) return;
    const sizes = new Map(rf.getNodes().map((n) => [n.id, { w: n.measured?.width ?? 220, h: n.measured?.height ?? 100 }]));
    st.edit((x) => applyLayout(x, autoLayout(tabView(x, useStudio.getState().tab), sizes)));
    setTimeout(() => void rf.fitView({ padding: 0.2, duration: 300 }), 50);
  };

  const removeGraph = async (g: api.GraphInfo) => {
    if (!(await ask(`Delete the graph “${g.name}”?${g.installed ? " The installed mode stays in the game until you remove it." : ""}`, { ok: "Delete", danger: true }))) return;
    const r = await api.deleteGraph(g.id);
    if (r.error) toast(r.error, true);
    void refreshList();
  };

  const uninstall = async (g: api.GraphInfo) => {
    if (!(await ask(`Remove the mode “${g.name}” from the game? The graph is kept.`, { ok: "Remove", danger: true }))) return;
    const r = await api.uninstall(g.id);
    if (r.error) toast(r.error, true);
    else toast("Removed from the game.");
    void refreshList();
  };

  if (!graph) {
    return (
      <div className="st-start tl-page">
        <Empty title="Game Mode Studio (Beta)" text="Make a game mode out of nodes: when something happens (a round starts, a player dies), check things and do things. The Studio turns it into a plugin and Playtest starts the game in your mode with bots.">
          {TEMPLATES.map((t, k) => (
            <button key={t.id} type="button" className={k === 0 ? "btn primary" : "btn"} title={t.blurb} onClick={() => void newGraph(t)}>
              {t.label}
            </button>
          ))}
          <button type="button" className="btn" onClick={() => void newGraph()}>
            Blank mode
          </button>
        </Empty>
        <h3 className="st-start-head">Your modes</h3>
        {listError ? (
          <p className="st-bad-text">{listError}</p>
        ) : !list ? (
          <Loading text="Reading your modes…" />
        ) : list.length === 0 ? (
          <p className="st-muted">None yet.</p>
        ) : (
          <ul className="st-graph-list">
            {list.map((g) => (
              <li key={g.id}>
                <button type="button" className="st-graph-open" disabled={g.damaged} onClick={() => void openId(g.id)}>
                  <strong>{g.name}</strong>
                  <span className="st-muted">{g.damaged ? "This file is damaged." : g.description || `${g.nodes ?? 0} nodes`}</span>
                </button>
                {g.installed && <span className="st-tag">Installed</span>}
                {g.installed && (
                  <button type="button" className="btn small" onClick={() => void uninstall(g)}>
                    Remove from game
                  </button>
                )}
                <button type="button" className="btn small" onClick={() => void removeGraph(g)}>
                  Delete
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    );
  }

  const errors = problems.filter((p) => p.level === "error").length;
  const mapNames = maps.map((m) => m.name).sort();
  return (
    <div className={`st-page${playing ? " playing" : ""}`}>
      <div className="st-toolbar tools-bar">
        <button
          type="button"
          className="btn"
          onClick={async () => {
            if (dirty && !(await ask("This graph has changes that aren't saved. Close it anyway?", { ok: "Close without saving", danger: true }))) return;
            st.close();
            void refreshList();
          }}
        >
          ‹ Modes
        </button>
        <strong className="st-name">
          {graph.mode.name}
          {dirty ? " •" : ""}
        </strong>
        <span className="st-sep" />
        <button type="button" className="btn" onClick={st.undo} disabled={!history.past.length} title="Undo (⌘Z)">
          Undo
        </button>
        <button type="button" className="btn" onClick={st.redo} disabled={!history.future.length} title="Redo (⇧⌘Z)">
          Redo
        </button>
        <button type="button" className="btn" onClick={layout} title="Lay the nodes out left to right">
          Tidy up
        </button>
        <div className="st-find">
          <input className="st-field" placeholder="Find in this mode" value={find} onChange={(e) => setFind(e.target.value)} onKeyDown={(e) => e.key === "Escape" && setFind("")} aria-label="Find in this mode" />
          {find.trim() && (
            <div className="st-find-list" role="listbox">
              {findNodes(graph, find).map((n) => (
                <button
                  key={n.id}
                  type="button"
                  className="st-palette-item"
                  onClick={() => {
                    focusRef.current(n.id);
                    setFind("");
                  }}
                >
                  <span className="st-palette-title">{nodeDef(n.type)?.title ?? n.type}</span>
                  <span className="st-palette-help">{Object.values(n.params).filter((x) => typeof x === "string" && x).join(" · ") || n.id}</span>
                </button>
              ))}
              {findNodes(graph, find).length === 0 && <div className="st-palette-none">Nothing matches.</div>}
            </div>
          )}
        </div>
        <span className="st-grow" />
        <select className="st-field" value={prefs.map} onChange={(e) => (setPrefs({ ...prefs, map: e.target.value }), savePrefs({ ...prefs, map: e.target.value }))} aria-label="Map">
          {(mapNames.includes(prefs.map) ? mapNames : [prefs.map, ...mapNames]).map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
        <input
          className="st-field st-field-num"
          type="number"
          min={0}
          max={31}
          value={prefs.bots}
          aria-label="Bots"
          title="Bots"
          onChange={(e) => {
            const n = Math.max(0, Math.min(31, Math.round(Number(e.target.value) || 0)));
            setPrefs({ ...prefs, bots: n });
            savePrefs({ ...prefs, bots: n });
          }}
        />
        {step ? (
          <>
            <span className="st-step" role="status" aria-live="polite">
              <span className="spinner" /> {step.label}… {Math.floor((Date.now() - step.at) / 1000)} s
            </span>
            <button
              type="button"
              className="btn"
              onClick={() => {
                cancelled.current = true;
                setStep(null);
              }}
              title="Stop after this step"
            >
              Cancel
            </button>
          </>
        ) : (
          <button type="button" className="btn primary st-playtest" data-testid="studio-playtest" onClick={() => void playtest()} disabled={!!working || errors > 0} title={errors > 0 ? "Fix the problems first (Results, below)" : "Compile with the debugger, then start the game in this mode with bots"}>
            Playtest
          </button>
        )}
      </div>
      <div className="st-main">
        <MyBlueprint api={canvas} snippets={snippets} reloadSnippets={reloadSnippets} toast={toast} />
        <div className="st-center">
          <TabBar />
          <Canvas toast={toast} snippets={snippets} onApi={onApi} />
        </div>
        <Inspector />
      </div>
      <Results focus={(id) => focusRef.current(id)} />
      <ActionBar
        dirty={dirty}
        errors={errors}
        working={working}
        where="in your Game Mode Studio folder"
        info="Saving keeps the graph. Install compiles it into a plugin and adds it to the game's modes (the Play page and the in-game menu list it); Playtest does that with the debugger and starts the game."
        save={{ onClick: () => save() }}
        undo={saved ? () => st.edit(() => saved) : null}
        actions={[{ label: "Install", busyLabel: "Compiling and installing…", onClick: () => install(), disabled: errors > 0, title: "Compile and add it to the game's modes" }]}
        more={[
          { label: "Show code", onClick: () => showCode(), note: "The plugin this graph becomes" },
          { label: "Open in Plugin Studio", onClick: () => openCode(), note: "Read only; save a copy there to make it your own" },
          { label: "Remove from the game", danger: true, onClick: () => removeInstalled(), note: "Takes the mode out of the game; the graph stays" },
        ]}
        reset={false}
      />
      {playing && (
        <DebugPanel
          focus={(id) => focusRef.current(id)}
          onResume={() => void api.debugCommand({ mode: playing, resume: true, breakpoints: activeBreakpoints(useStudio.getState().graph, useStudio.getState().bpsOff) })}
          onBpsOff={(off) => st.setBpsOff(off)}
          onRefresh={() => void api.debugCommand({ mode: playing, vars: true })}
          onStop={() => st.stopPlaying()}
        />
      )}
      {code != null && (
        <Sheet label="Generated code" onClose={() => setCode(null)}>
          <div className="st-code">
            <h3>The plugin this graph becomes</h3>
            <p className="st-muted">Made from the graph each time it's compiled: change the graph rather than this.</p>
            <pre>{code}</pre>
            <div className="row">
              <button type="button" className="btn" onClick={() => void navigator.clipboard.writeText(code).then(() => toast("Copied."), () => toast("Couldn't copy.", true))}>
                Copy
              </button>
              <button
                type="button"
                className="btn"
                disabled={!!working}
                title="Opens it read only in the Plugin Studio (Save a copy there to make it your own plugin)"
                onClick={() => void openCode()}
              >
                Open in Plugin Studio
              </button>
              <button type="button" className="btn primary" onClick={() => setCode(null)}>
                Close
              </button>
            </div>
          </div>
        </Sheet>
      )}
    </div>
  );
}

export default function StudioPage(props: ToolProps) {
  return (
    <ReactFlowProvider>
      <Studio {...props} />
    </ReactFlowProvider>
  );
}
