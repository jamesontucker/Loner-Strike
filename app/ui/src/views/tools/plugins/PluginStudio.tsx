// The Plugin Studio (Beta): write AMX Mod X plugins in the app. Files on the left (My
// plugins, the game's plugins, the include files), Monaco in the middle with Pawn colours,
// completions and docs from the game's include files, and compile / try it in my game with
// the problems in plain words and the plugin's live log underneath.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { usePoll } from "../../../api";
import { useToolArg } from "../../../stores/tools";
import { BusyButton, LoadError, Skeleton, useAskText, useConfirm, useToast } from "../../../ui";
import type { ToolProps } from "../registry";
import type { EditorHandle } from "./monaco";
import { compile, deleteFile, overview, pluginLog, readFile, saveFile, stopTrying, tryIt, words, type CompileResult, type Diag, type FolderId, type LogLine } from "./pluginsApi";
import { TEMPLATES } from "./templates";
import "./plugins.css";

type Open = { folder: FolderId; name: string; encoding: string; readOnly: string | null };
type Busy = "open" | "save" | "compile" | "try" | "stop" | null;
type Tab = "problems" | "log" | "output";

const keyOf = (folder: FolderId, name: string) => `${folder}/${name}`;
const stemOf = (name: string) => name.replace(/\.[^.]+$/, "");
const SAFE_STEM = /^[A-Za-z0-9_][A-Za-z0-9_-]{0,59}$/;

// what was last saved, per file (kept while the app runs, like the editor's models)
const savedText = new Map<string, string>();
const lastResult = new Map<string, CompileResult>();

export default function PluginStudio(_: ToolProps) {
  const qc = useQueryClient();
  const toast = useToast();
  const confirm = useConfirm();
  const askText = useAskText();
  const arg = useToolArg();
  const ov = useQuery({ queryKey: ["tools", "plugins"], queryFn: overview, retry: false });
  const wq = useQuery({ queryKey: ["tools", "plugins", "words"], queryFn: words, retry: false, staleTime: Infinity });

  const host = useRef<HTMLDivElement>(null);
  const ed = useRef<EditorHandle | null>(null);
  const mod = useRef<typeof import("./monaco") | null>(null);
  const [editorReady, setEditorReady] = useState(false);
  const [editorFailed, setEditorFailed] = useState(false);
  const [open, setOpen] = useState<Open | null>(null);
  const [dirty, setDirty] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState<Busy>(null);
  const [result, setResult] = useState<CompileResult | null>(null);
  const [tab, setTab] = useState<Tab>("problems");
  const [creating, setCreating] = useState(false);
  const openRef = useRef<Open | null>(null);
  openRef.current = open;

  const key = open ? keyOf(open.folder, open.name) : null;
  const trying = ov.data?.trying ?? [];
  const isTrying = !!open && trying.includes(stemOf(open.name));
  const isSource = !!open && open.name.toLowerCase().endsWith(".sma");

  // Monaco loads on first open, in its own chunk
  const actions = useRef({ save: () => {}, compile: () => {} });
  const haveData = !!ov.data;
  useEffect(() => {
    if (!haveData) return;
    let gone = false;
    import("./monaco")
      .then((m) => {
        if (gone || !host.current) return;
        mod.current = m;
        ed.current = m.createEditor(host.current, {
          change: () => {
            const o = openRef.current;
            if (!o || !ed.current) return;
            const k = keyOf(o.folder, o.name);
            const changed = ed.current.value() !== savedText.get(k);
            setDirty((d) => {
              if (changed === d.has(k)) return d;
              const n = new Set(d);
              if (changed) n.add(k);
              else n.delete(k);
              return n;
            });
          },
          save: () => actions.current.save(),
          compile: () => actions.current.compile(),
        });
        setEditorReady(true);
      })
      .catch(() => !gone && setEditorFailed(true));
    return () => {
      gone = true;
      ed.current?.dispose();
      ed.current = null;
      setEditorReady(false);
    };
  }, [haveData]);

  useEffect(() => {
    if (editorReady && wq.data) mod.current?.setWords(wq.data);
  }, [editorReady, wq.data]);

  const markers = useCallback((r: CompileResult | null) => {
    ed.current?.markers((r?.diagnostics ?? []).filter((d) => d.inSource));
  }, []);

  const openFile = useCallback(
    async (folder: FolderId, name: string) => {
      if (!ed.current) return;
      const k = keyOf(folder, name);
      setBusy("open");
      try {
        const f = await readFile(folder, name);
        // a file with unsaved changes keeps them
        const keep = dirty.has(k);
        if (!keep) savedText.set(k, f.text);
        ed.current.show(k, keep ? null : f.text, !!f.readOnly);
        setOpen({ folder, name, encoding: f.encoding, readOnly: f.readOnly });
        const r = lastResult.get(k) ?? null;
        setResult(r);
        markers(r);
        ed.current.focus();
      } catch (e) {
        toast((e as Error).message, true);
      } finally {
        setBusy(null);
      }
    },
    [dirty, markers, toast],
  );

  // "plugins:mine/foo.sma" opens that file
  useEffect(() => {
    if (!editorReady || !arg) return;
    const m = arg.match(/^(mine|game|include)\/(.+)$/);
    if (m) openFile(m[1] as FolderId, m[2]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editorReady, arg]);

  const refresh = () => qc.invalidateQueries({ queryKey: ["tools", "plugins"], exact: true });

  /** Saves the open file if it has changes. False if it couldn't. */
  const saveNow = async (quiet = false): Promise<boolean> => {
    const o = openRef.current;
    if (!o || !ed.current) return false;
    const k = keyOf(o.folder, o.name);
    if (o.readOnly) {
      if (!quiet) await saveCopy();
      return false;
    }
    if (!dirty.has(k)) return true;
    const text = ed.current.value();
    const r = await saveFile(o.folder, o.name, text, o.encoding);
    if (r.error) {
      toast(r.error, true);
      return false;
    }
    savedText.set(k, text);
    setDirty((d) => {
      const n = new Set(d);
      n.delete(k);
      return n;
    });
    if (!quiet) toast("Saved");
    refresh();
    return true;
  };

  const saveCopy = async () => {
    const o = openRef.current;
    if (!o || !ed.current) return;
    const name = await askText("Save a copy in My plugins as:", { label: "File name", placeholder: stemOf(o.name) + "_copy", max: 60 }, { ok: "Save copy" });
    if (name === null) return;
    const stem = stemOf(name.trim());
    if (!SAFE_STEM.test(stem)) return toast("Use only letters, numbers, - and _ in the name.", true);
    const file = `${stem}.${o.name.split(".").pop()?.toLowerCase() ?? "sma"}`;
    const r = await saveFile("mine", file, ed.current.value(), "utf8", true);
    if (r.error) return toast(r.error, true);
    toast(`Saved as ${file} in My plugins`);
    await refresh();
    openFile("mine", file);
  };

  const run = async (kind: "compile" | "try") => {
    const o = openRef.current;
    if (!o || busy) return;
    if (!o.name.toLowerCase().endsWith(".sma")) return toast("Open a plugin (.sma) to compile it.", true);
    setBusy(kind);
    try {
      if (!o.readOnly && !(await saveNow(true))) return;
      const r = kind === "try" ? await tryIt(o.folder, o.name) : await compile(o.folder, o.name);
      if (r.error) return toast(r.error, true);
      const k = keyOf(o.folder, o.name);
      lastResult.set(k, r);
      if (openRef.current && keyOf(openRef.current.folder, openRef.current.name) === k) {
        setResult(r);
        markers(r);
      }
      if (r.diagnostics.length) setTab("problems");
      if (r.tried) {
        toast(r.message);
        setTab("log");
        refresh();
      } else if (r.ok) toast(r.message);
      else toast(r.message, true);
    } finally {
      setBusy(null);
    }
  };

  actions.current = { save: () => void saveNow(), compile: () => void run("compile") };

  const stop = async () => {
    if (!open) return;
    setBusy("stop");
    const r = await stopTrying(stemOf(open.name), !!ov.data?.running);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    toast(r.message ?? "Taken out.");
    refresh();
  };

  const create = async (stem: string, template: string) => {
    if (!SAFE_STEM.test(stem)) return toast("Use only letters, numbers, - and _ in the name (no spaces).", true);
    const t = TEMPLATES.find((x) => x.id === template) ?? TEMPLATES[0];
    const file = `${stem}.sma`;
    const r = await saveFile("mine", file, t.text(stem), "utf8", true);
    if (r.error) return toast(r.error, true);
    setCreating(false);
    await refresh();
    openFile("mine", file);
  };

  const remove = async (name: string) => {
    if (!(await confirm(`Delete ${name} from My plugins? This can't be undone.`, { ok: "Delete", danger: true }))) return;
    const r = await deleteFile("mine", name);
    if (r.error) return toast(r.error, true);
    const k = keyOf("mine", name);
    mod.current?.dropModel(k);
    savedText.delete(k);
    lastResult.delete(k);
    if (open?.folder === "mine" && open.name === name) setOpen(null);
    refresh();
  };

  if (ov.isLoading) return <Skeleton rows={4} />;
  if (ov.error) return <LoadError text={(ov.error as Error).message} onRetry={() => ov.refetch()} />;
  const data = ov.data!;
  const diags = result?.diagnostics ?? [];
  const errors = diags.filter((d) => d.severity !== "warning").length;

  return (
    <div className="ps">
      {!data.compiler && data.compilerProblem && <p className="ps-note warn">{data.compilerProblem} You can still write and save plugins.</p>}
      <div className="ps-main">
        <aside className="ps-files panel" aria-label="Plugin files">
          <div className="ps-files-head">
            <button className="btn small primary" onClick={() => setCreating((c) => !c)} aria-expanded={creating}>
              New plugin
            </button>
            <button className="btn small" onClick={() => refresh()} title="Look for new files">
              Refresh
            </button>
          </div>
          {creating && <NewPlugin onCreate={create} onCancel={() => setCreating(false)} />}
          {data.folders.map((f) => (
            <FolderList
              key={f.id}
              label={f.label}
              path={f.path}
              start={f.id !== "include"}
              files={f.files.map((x) => x.name)}
              current={open?.folder === f.id ? open.name : null}
              dirty={(n) => dirty.has(keyOf(f.id, n))}
              tried={(n) => trying.includes(stemOf(n)) && f.id !== "include"}
              onOpen={(n) => openFile(f.id, n)}
              onDelete={f.id === "mine" ? remove : undefined}
              empty={f.id === "mine" ? "Nothing here yet. Make one with New plugin." : "No files."}
            />
          ))}
        </aside>

        <section className="ps-editor-col">
          <div className="ps-bar">
            <span className="ps-title" title={open ? `${open.folder === "mine" ? "My plugins" : open.folder === "game" ? "Game's plugins" : "Include files"} / ${open.name}` : undefined}>
              {open ? open.name : "No file open"}
              {key && dirty.has(key) && <span className="ps-dirty" role="img" aria-label="Not saved" title="Not saved" />}
              {open?.readOnly && <span className="tag">Read only</span>}
              {isTrying && <span className="tag ps-trying">In your game</span>}
            </span>
            <span className="ps-actions">
              {open?.readOnly ? (
                <button className="btn small" onClick={saveCopy} disabled={!!busy}>
                  Save a copy…
                </button>
              ) : (
                <BusyButton className="btn small" busy={busy === "save"} label="Save" busyLabel="Saving…" disabled={!open || !!busy || !dirty.has(key ?? "")} onClick={() => void saveNow()} />
              )}
              <BusyButton className="btn small" busy={busy === "compile"} label="Compile" busyLabel="Compiling…" disabled={!isSource || !!busy || !data.compiler} onClick={() => run("compile")} />
              <BusyButton className="btn small primary" busy={busy === "try"} label="Try it in my game" busyLabel="Compiling and loading…" disabled={!isSource || !!busy || !data.compiler || !!open?.readOnly} onClick={() => run("try")} />
              {isTrying && <BusyButton className="btn small" busy={busy === "stop"} label="Take it out" busyLabel="Taking it out…" disabled={!!busy} onClick={stop} />}
            </span>
          </div>
          <div className="ps-editor-wrap">
            <div ref={host} className="ps-editor" />
            {!open && editorReady && (
              <div className="ps-empty">
                <p>Open a file on the left, or make a new plugin from a template.</p>
                <p className="muted">Ctrl/⌘+S saves, Ctrl/⌘+Enter compiles. Hover a function to see what it does; type to get suggestions.</p>
              </div>
            )}
            {!editorReady && !editorFailed && (
              <div className="ps-empty">
                <p className="muted">
                  <span className="spinner" aria-hidden="true" /> Opening the editor…
                </p>
              </div>
            )}
            {editorFailed && (
              <div className="ps-empty">
                <LoadError text="The editor couldn't load. Reload the app and try again." />
              </div>
            )}
            {busy === "open" && <span className="ps-opening spinner" aria-label="Opening the file" />}
          </div>
          <div className="ps-bottom panel">
            <div className="ps-tabs" role="tablist">
              <button role="tab" className="chip" aria-selected={tab === "problems"} onClick={() => setTab("problems")}>
                Problems{diags.length ? ` (${diags.length})` : ""}
              </button>
              <button role="tab" className="chip" aria-selected={tab === "log"} onClick={() => setTab("log")}>
                Game log
              </button>
              <button role="tab" className="chip" aria-selected={tab === "output"} onClick={() => setTab("output")}>
                Compiler output
              </button>
              {result && (
                <span className={`ps-result ${result.ok ? (result.warnings ? "warn" : "ok") : "bad"}`}>
                  {result.message}
                  {result.ok && result.size ? ` ${(result.size / 1024).toFixed(1)} KB.` : ""}
                </span>
              )}
            </div>
            <div className="ps-tab-body">
              {tab === "problems" && <Problems diags={diags} errors={errors} compiled={!!result} onGo={(d) => ed.current?.reveal(d.line)} />}
              {tab === "output" && (result ? <pre className="ps-output">{result.output.trim()}</pre> : <p className="muted">Compile to see what the compiler said.</p>)}
              {tab === "log" && <GameLog name={open && isSource ? stemOf(open.name) : null} trying={isTrying} running={data.running} />}
            </div>
          </div>
        </section>
      </div>
      {!wq.isLoading && wq.error && <p className="ps-note">Suggestions and docs aren't available: {(wq.error as Error).message}</p>}
    </div>
  );
}

function FolderList(p: {
  label: string;
  path: string;
  start: boolean;
  files: string[];
  current: string | null;
  dirty: (n: string) => boolean;
  tried: (n: string) => boolean;
  onOpen: (n: string) => void;
  onDelete?: (n: string) => void;
  empty: string;
}) {
  const [shown, setShown] = useState(p.start);
  const [filter, setFilter] = useState("");
  const list = useMemo(() => (filter ? p.files.filter((f) => f.toLowerCase().includes(filter.toLowerCase())) : p.files), [p.files, filter]);
  return (
    <div className="ps-folder">
      <button className="ps-folder-head" aria-expanded={shown} onClick={() => setShown((s) => !s)} title={p.path}>
        <span className="ps-caret" aria-hidden="true">
          {shown ? "▾" : "▸"}
        </span>
        {p.label}
        <span className="muted"> {p.files.length}</span>
      </button>
      {shown && (
        <>
          {p.files.length > 12 && <input className="ps-filter" placeholder="Filter" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label={`Filter ${p.label}`} />}
          {p.files.length === 0 && <p className="muted ps-folder-empty">{p.empty}</p>}
          <ul className="ps-file-list">
            {list.map((n) => (
              <li key={n} className={n === p.current ? "on" : undefined}>
                <button className="ps-file" onClick={() => p.onOpen(n)} aria-current={n === p.current ? "true" : undefined}>
                  <span className="ps-file-name">{n}</span>
                  {p.dirty(n) && <span className="ps-dirty" role="img" aria-label="Not saved" />}
                  {p.tried(n) && <span className="ps-live" title="In your game" aria-label="In your game" />}
                </button>
                {p.onDelete && (
                  <button className="ps-file-del" title={`Delete ${n}`} aria-label={`Delete ${n}`} onClick={() => p.onDelete!(n)}>
                    ✕
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

function NewPlugin({ onCreate, onCancel }: { onCreate: (stem: string, template: string) => Promise<void>; onCancel: () => void }) {
  const [name, setName] = useState("my_plugin");
  const [template, setTemplate] = useState("mode");
  const [busy, setBusy] = useState(false);
  const t = TEMPLATES.find((x) => x.id === template);
  return (
    <form
      className="ps-new"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        await onCreate(name.trim(), template);
        setBusy(false);
      }}
    >
      <label className="field">
        <span>Name</span>
        <input value={name} onChange={(e) => setName(e.target.value)} maxLength={60} spellCheck={false} autoFocus />
        <small>Letters, numbers, - and _. It becomes {name.trim() || "name"}.sma.</small>
      </label>
      <label className="field">
        <span>Start from</span>
        <select value={template} onChange={(e) => setTemplate(e.target.value)}>
          {TEMPLATES.map((x) => (
            <option key={x.id} value={x.id}>
              {x.label}
            </option>
          ))}
        </select>
        {t && <small>{t.blurb}</small>}
      </label>
      <div className="row">
        <BusyButton type="submit" className="btn small primary" busy={busy} label="Make it" busyLabel="Making…" />
        <button type="button" className="btn small" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </form>
  );
}

function Problems({ diags, errors, compiled, onGo }: { diags: Diag[]; errors: number; compiled: boolean; onGo: (d: Diag) => void }) {
  if (!compiled) return <p className="muted">Compile to check the plugin. Problems show here and on their lines.</p>;
  if (!diags.length) return <p className="ps-good">No problems.</p>;
  return (
    <ul className="ps-problems">
      {diags.map((d, i) => (
        <li key={i} className={d.severity === "warning" ? "warn" : "bad"}>
          <button className="ps-problem" onClick={() => d.inSource && onGo(d)} disabled={!d.inSource}>
            <span className="ps-problem-where">{d.inSource ? `Line ${d.line}` : `${d.file}, line ${d.line}`}</span>
            <span className="ps-problem-kind">{d.severity === "warning" ? "Warning" : "Error"}</span>
            <span className="ps-problem-text">
              {d.plain || d.message}
              <span className="muted ps-problem-raw">
                {" "}
                ({d.severity} {d.code}: {d.message})
              </span>
            </span>
          </button>
        </li>
      ))}
      {errors === 0 && <li className="muted ps-problem-foot">Only warnings: the plugin works, but they're worth a look.</li>}
    </ul>
  );
}

function GameLog({ name, trying, running }: { name: string | null; trying: boolean; running: boolean }) {
  const [lines, setLines] = useState<LogLine[]>([]);
  const cursor = useRef<string | null>(null);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    setLines([]);
    cursor.current = null;
  }, [name]);
  usePoll(
    async () => {
      if (!name) return;
      const r = await pluginLog(name, cursor.current);
      if ("error" in r && r.error) return;
      cursor.current = r.cursor;
      if (r.lines.length) setLines((l) => [...l, ...r.lines].slice(-500));
    },
    1500,
    [name],
    !!name,
  );
  useEffect(() => {
    const b = box.current;
    if (b) b.scrollTop = b.scrollHeight;
  }, [lines]);
  if (!name) return <p className="muted">Open a plugin to see what it writes to the game's log.</p>;
  return (
    <div className="ps-log-wrap">
      <p className="muted ps-log-note">
        {trying ? (running ? `Showing ${name}'s lines from AMX Mod X's logs as they come.` : `${name} is in your game. Start a map from Play to see it run.`) : `Try ${name} in your game to see it run here. Older lines about it show too.`}
        {lines.length > 0 && (
          <button className="btn small" onClick={() => setLines([])}>
            Clear
          </button>
        )}
      </p>
      <div className="ps-log" ref={box} role="log" aria-live="polite">
        {lines.length === 0 ? (
          <p className="muted">Nothing yet.</p>
        ) : (
          lines.map((l, i) => (
            <div key={i} className={l.error ? "bad" : undefined}>
              <span className="ps-log-at">{l.at}</span> {l.text}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
