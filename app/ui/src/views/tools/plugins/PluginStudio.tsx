// The Plugin Studio (Beta): write AMX Mod X plugins in the app. Files on the left (My
// plugins, the game's plugins, the include files), Monaco in the middle with Pawn colours,
// completions and docs from the game's include files, and compile / try it in my game with
// the problems in plain words and the plugin's live log underneath.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { usePoll } from "../../../api";
import { useDirty, useNav } from "../../../store";
import { useToolArg } from "../../../stores/tools";
import { useReportAddress } from "../common/address";
import { useToolCrumbs } from "../common/shell";
import { BusyButton, LoadError, Sheet, Skeleton, useAskText, useConfirm, useToast } from "../../../ui";
import { ActionBar, type BarAction } from "../common/ActionBar";
import { HistoryPanel } from "../common/History";
import type { ToolProps } from "../registry";
import { HelpPanel } from "./Help";
import type { EditorHandle } from "./monaco";
import { RESERVED_NAME, windowsReserved } from "../common/fileNames";
import { compile, deleteFile, exportFile, importFiles, importPick, overview, revealFile, pluginLog, readFile, renameFile, resetFile, saveFile, stopTrying, tryIt, words, type CompileResult, type Diag, type FolderId, type LogLine, type OpenedFile, type SourceFile } from "./pluginsApi";
import { TEMPLATES } from "./templates";
import "./plugins.css";

type Open = Pick<OpenedFile, "folder" | "name" | "encoding" | "readOnly" | "reset" | "resetHint" | "history">;
type Busy = "open" | "save" | "compile" | "try" | "stop" | "reset" | "rename" | null;
type Tab = "problems" | "log" | "output" | "help";
/** what the action bar says while each runs */
const WORKING: Record<Exclude<Busy, null>, string> = { open: "Opening…", save: "Saving…", compile: "Compiling…", try: "Compiling and loading…", stop: "Taking it out…", reset: "Resetting…", rename: "Renaming…" };

const keyOf = (folder: FolderId, name: string) => `${folder}/${name}`;
const stemOf = (name: string) => name.replace(/\.[^.]+$/, "");
const SAFE_STEM = /^[A-Za-z0-9_][A-Za-z0-9_-]{0,59}$/;
/** Why a plugin name (without .sma) can't be used, or null. */
const stemProblem = (stem: string) => (!stem ? "Type a name." : !SAFE_STEM.test(stem) ? "Use only letters, numbers, - and _ in the name (no spaces)." : windowsReserved(stem) ? RESERVED_NAME : null);

// what was last saved, per file (kept while the app runs, like the editor's models)
const savedText = new Map<string, string>();
const lastResult = new Map<string, CompileResult>();

export default function PluginStudio(_: ToolProps) {
  const qc = useQueryClient();
  const toast = useToast();
  const confirm = useConfirm();
  const askText = useAskText();
  const go = useNav((s) => s.go);
  const [io, setIo] = useState<"export" | "import" | null>(null);
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
  // the open file is in the address ("plugins:mine/foo.sma") and the crumbs; unsaved files are
  // asked about before leaving
  useReportAddress("plugins", open ? `plugins:${open.folder}/${open.name}` : null);
  useToolCrumbs("plugins", open ? [{ label: open.name }] : null);
  useDirty("plugins", dirty.size > 0);
  const [busy, setBusy] = useState<Busy>(null);
  const [result, setResult] = useState<CompileResult | null>(null);
  const [tab, setTab] = useState<Tab>("problems");
  const [creating, setCreating] = useState(false);
  // narrow windows: the file list is a drawer over the editor, so the editor comes first
  const [filesOpen, setFilesOpen] = useState(false);
  const [helpQuery, setHelpQuery] = useState("");
  const [history, setHistory] = useState<string | null>(null);
  const openRef = useRef<Open | null>(null);
  openRef.current = open;

  const key = open ? keyOf(open.folder, open.name) : null;
  const trying = ov.data?.trying ?? [];
  const isTrying = !!open && trying.includes(stemOf(open.name));
  const isSource = !!open && open.name.toLowerCase().endsWith(".sma");

  // Monaco loads on first open, in its own chunk
  const actions = useRef<{ save: () => void; compile: () => void; open: (f: FolderId, n: string, line?: number) => Promise<void> }>({ save: () => {}, compile: () => {}, open: async () => {} });
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
          hooks: {
            openInclude: (file, line) => void actions.current.open("include", file, line),
            readInclude: async (file) => {
              try {
                return (await readFile("include", file)).text;
              } catch {
                return null;
              }
            },
            newPlugin: () => setCreating(true),
            docs: (q) => {
              setHelpQuery(q);
              setTab("help");
            },
          },
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
    async (folder: FolderId, name: string, line?: number) => {
      if (!ed.current) return;
      const k = keyOf(folder, name);
      setBusy("open");
      try {
        const f = await readFile(folder, name);
        // a file with unsaved changes keeps them
        const keep = dirty.has(k);
        if (!keep) savedText.set(k, f.text);
        ed.current.show(k, keep ? null : f.text, !!f.readOnly);
        setOpen({ folder, name, encoding: f.encoding, readOnly: f.readOnly, reset: f.reset, resetHint: f.resetHint, history: f.history });
        const r = lastResult.get(k) ?? null;
        setResult(r);
        markers(r);
        if (line) ed.current.reveal(line);
        else ed.current.focus();
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
  // Export / Import: Community doesn't take plugins, so they're shared by hand
  const exportNow = async () => {
    if (!open) return;
    setIo("export");
    try {
      const r = await exportFile(open.folder, open.name);
      if (r.error) toast(r.error, true);
      else if (!r.cancelled) toast(r.message || "Exported.");
    } finally {
      setIo(null);
    }
  };
  const importNow = async () => {
    setIo("import");
    try {
      const p = await importPick();
      if (p.error) return toast(p.error, true);
      if (p.cancelled || !p.id || !p.files) return;
      const ok = p.files.filter((f) => !f.problem);
      const refused = p.files.filter((f) => f.problem).map((f) => f.problem);
      if (!ok.length) return toast(refused.join(" ") || "Nothing to import.", true);
      const taken = new Set(ok.map((f) => f.name.toLowerCase()));
      const have = new Set((ov.data?.folders.find((f) => f.id === "mine")?.files ?? []).map((f) => f.name.toLowerCase()));
      const renames: Record<string, string> = {};
      for (const f of ok.filter((x) => x.taken)) {
        const ext = f.name.slice(-4);
        const typed = await askText(
          `There's already a ${f.name} in My plugins. Import it under another name (it's never overwritten).`,
          { label: "New name", value: f.name.slice(0, -4) + "_2" + ext, max: 64 },
          {
            ok: "Import as this",
            check: (v) => {
              const n = v.trim().toLowerCase().endsWith(ext.toLowerCase()) ? v.trim() : v.trim() + ext;
              if (windowsReserved(n)) return RESERVED_NAME;
              if (!/^[A-Za-z0-9_][A-Za-z0-9_.-]*$/.test(n) || n.includes("..") || n.length > 64) return "Use letters, numbers, - and _.";
              if (have.has(n.toLowerCase()) || (taken.has(n.toLowerCase()) && n.toLowerCase() !== f.name.toLowerCase())) return `There's already a file called ${n}.`;
              return null;
            },
          },
        );
        if (typed === null) return;
        const n = typed.trim().toLowerCase().endsWith(ext.toLowerCase()) ? typed.trim() : typed.trim() + ext;
        renames[f.name] = n;
        have.add(n.toLowerCase());
      }
      const r = await importFiles(p.id, renames);
      if (r.error) return toast(r.error, true);
      toast(r.message || "Imported.", !r.ok);
      await refresh();
      if (r.first) await openFile("mine", r.first);
    } finally {
      setIo(null);
    }
  };

  // History restored a file (or Undo put one back): read the open one again, unless it has
  // unsaved changes
  useEffect(() => {
    const again = () => {
      const o = openRef.current;
      refresh();
      if (o && !dirty.has(keyOf(o.folder, o.name))) void openFile(o.folder, o.name);
    };
    window.addEventListener("ls-files-restored", again);
    return () => window.removeEventListener("ls-files-restored", again);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dirty, openFile]);

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
    const ext = o.name.split(".").pop()?.toLowerCase() ?? "sma";
    const text = ed.current.value();
    let file = "";
    const name = await askText(
      "Save a copy in My plugins as:",
      { label: "File name", placeholder: stemOf(o.name) + "_copy", max: 60 },
      {
        ok: "Save copy",
        busy: "Saving…",
        check: async (typed) => {
          const stem = stemOf(typed);
          const why = stemProblem(stem);
          if (why) return why;
          file = `${stem}.${ext}`;
          const r = await saveFile("mine", file, text, "utf8", true);
          return r.error ?? null;
        },
      },
    );
    if (name === null || !file) return;
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

  actions.current = { save: () => void saveNow(), compile: () => void run("compile"), open: openFile };

  const reset = async () => {
    const o = openRef.current;
    if (!o?.reset) return;
    const plugin = o.folder === "game";
    const ok = await confirm(
      `Put ${o.name} back to Loner-Strike's version?${plugin ? ` ${stemOf(o.name)}.amxx in your game is rebuilt from it too.` : ""} The file as it is now is kept first, so you can get it back from History.`,
      { ok: "Reset", danger: true },
    );
    if (!ok) return;
    setBusy("reset");
    const r = await resetFile(o.folder, o.name);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    toast(r.message ?? "Reset.");
    const k = keyOf(o.folder, o.name);
    setDirty((d) => {
      const n = new Set(d);
      n.delete(k);
      return n;
    });
    refresh();
    await openFile(o.folder, o.name);
  };

  const renameNow = async () => {
    const o = openRef.current;
    if (!o || o.folder !== "mine") return;
    if (!(await saveNow(true))) return;
    const ext = o.name.split(".").pop()?.toLowerCase() ?? "sma";
    let stem = "";
    const to = await askText(
      `Rename ${o.name} to:`,
      { label: "New name", placeholder: stemOf(o.name), max: 60, value: stemOf(o.name) },
      {
        ok: "Rename",
        busy: "Renaming…",
        check: async (typed) => {
          const s = stemOf(typed);
          const why = stemProblem(s);
          if (why) return why;
          if (`${s}.${ext}` === o.name) return null;
          setBusy("rename");
          try {
            const r = await renameFile("mine", o.name, `${s}.${ext}`);
            if (r.error) return r.error;
          } finally {
            setBusy(null);
          }
          stem = s;
          return null;
        },
      },
    );
    if (to === null || !stem) return;
    mod.current?.dropModel(keyOf("mine", o.name));
    savedText.delete(keyOf("mine", o.name));
    await refresh();
    await openFile("mine", `${stem}.${ext}`);
  };

  const stop = async () => {
    if (!open) return;
    setBusy("stop");
    const r = await stopTrying(stemOf(open.name), true);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    toast(r.message ?? "Taken out.");
    refresh();
  };

  /** null when made; else why not (shown under the name) */
  const create = async (stem: string, template: string): Promise<string | null> => {
    const why = stemProblem(stem);
    if (why) return why;
    const t = TEMPLATES.find((x) => x.id === template) ?? TEMPLATES[0];
    const file = `${stem}.sma`;
    const r = await saveFile("mine", file, t.text(stem), "utf8", true);
    if (r.error) return r.error;
    setCreating(false);
    setFilesOpen(false);
    await refresh();
    openFile("mine", file);
    return null;
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
        <aside id="ps-files" className={`ps-files panel${filesOpen ? " open" : ""}`} aria-label="Plugin files">
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
              files={f.files}
              hidden={f.hidden ?? 0}
              current={open?.folder === f.id ? open.name : null}
              dirty={(n) => dirty.has(keyOf(f.id, n))}
              tried={(n) => trying.includes(stemOf(n)) && f.id !== "include"}
              onOpen={(n) => {
                setFilesOpen(false);
                void openFile(f.id, n);
              }}
              onDelete={f.id === "mine" ? remove : undefined}
              empty={f.id === "mine" ? "Nothing here yet. Make one with New plugin." : "No files."}
            />
          ))}
        </aside>

        <section className="ps-editor-col">
          <div className="ps-bar">
            <button type="button" className="btn small ps-files-toggle" aria-controls="ps-files" aria-expanded={filesOpen} onClick={() => setFilesOpen((o) => !o)}>
              Files
            </button>
            <span className="ps-title" title={open ? `${open.folder === "mine" ? "My plugins" : open.folder === "game" ? "Game's plugins" : "Include files"} / ${open.name}` : undefined}>
              {open ? open.name : "No file open"}
              {key && dirty.has(key) && <span className="ps-dirty" role="img" aria-label="Not saved" title="Not saved" />}
              {open?.readOnly && <span className="tag">Read only</span>}
              {isTrying && <span className="tag ps-trying">In your game</span>}
            </span>
            <span className="ps-actions">
              <button className="btn small" disabled={!open || !!open.readOnly || !editorReady} onClick={() => ed.current?.palette()} title="Snippets, templates and the docs (or type / at the start of a line)">
                Insert… <kbd>/</kbd>
              </button>
              <button className="btn small" aria-pressed={tab === "help"} onClick={() => setTab(tab === "help" ? "problems" : "help")} title="Search every function, and guides on writing plugins">
                Help & docs
              </button>
            </span>
          </div>
          {open?.readOnly && (
            <div className="ps-readonly" role="note">
              <span className="ps-lock" aria-hidden="true">🔒</span>
              <span className="ps-readonly-text">
                {open.readOnly}
                {open.reset?.differs && " The copy in your game has been changed from Loner-Strike's version."}
                {open.resetHint && ` ${open.resetHint}`}
              </span>
              <span className="ps-readonly-actions">
                {open.folder !== "include" || open.name.endsWith(".inc") ? (
                  <button className="btn small" onClick={saveCopy} disabled={!!busy}>
                    Save a copy in My plugins…
                  </button>
                ) : null}
                {open.reset?.differs && <BusyButton className="btn small" busy={busy === "reset"} label="Reset to Loner-Strike's version…" busyLabel="Resetting…" disabled={!!busy} onClick={reset} />}
              </span>
            </div>
          )}
          <div className="ps-editor-wrap">
            <div ref={host} className="ps-editor" />
            {!open && editorReady && (
              <div className="ps-empty">
                <p>Open a file from the list, or make a new plugin from a template.</p>
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
              <button role="tab" className="chip" aria-selected={tab === "help"} onClick={() => setTab("help")}>
                Help & docs
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
              {tab === "help" && (
                <HelpPanel
                  words={wq.data}
                  query={helpQuery}
                  setQuery={setHelpQuery}
                  onOpen={(w) => void openFile("include", w.file, w.line)}
                  onInsert={(w) => {
                    if (!open || open.readOnly) return toast("Open one of your own plugins to put it in.", true);
                    ed.current?.insertWord(w);
                  }}
                />
              )}
            </div>
          </div>
        </section>
      </div>
      {!wq.isLoading && wq.error && <p className="ps-note">Suggestions and docs aren't available: {(wq.error as Error).message}</p>}
      <ActionBar
        dirty={!!key && dirty.has(key)}
        working={busy ? WORKING[busy] : null}
        where={open ? `${open.folder === "mine" ? "My plugins" : open.folder === "game" ? "game's plugins" : "include files"} / ${open.name}` : undefined}
        info="Saving keeps the version before (History). Compile checks it; Try it in my game puts it in your game and restarts the map. Ctrl/⌘+S saves, Ctrl/⌘+Enter compiles."
        status={open?.readOnly ? <span className="tl-ab-state">Read only</span> : !open ? <span className="tl-ab-state">No file open</span> : undefined}
        save={open && !open.readOnly ? { onClick: () => saveNow(), busyLabel: WORKING.save } : null}
        actions={[
          { label: "Compile", busyLabel: WORKING.compile, onClick: () => run("compile"), disabled: !isSource || !data.compiler, title: data.compiler ? "Check it with the compiler (Ctrl/⌘+Enter)" : data.compilerProblem ?? undefined },
          { label: "Try it in my game", busyLabel: WORKING.try, onClick: () => run("try"), disabled: !isSource || !data.compiler || !!open?.readOnly, title: open?.readOnly ? "This file is read only: Save a copy to My plugins first, then try that" : !data.compiler ? data.compilerProblem ?? undefined : "Compile it, put it in your game and restart the map" },
          { label: "Take it out", busyLabel: WORKING.stop, onClick: stop, hidden: !isTrying },
        ]}
        more={moreActions({
          open,
          busy: !!busy,
          saveCopy,
          rename: renameNow,
          remove: () => open && remove(open.name),
          history: () => open && setHistory(open.history),
          reset,
          reveal: () => {
            if (!open) return;
            void revealFile(open.folder, open.name).then((r) => r.error && toast(r.error, true));
          },
        })}
        reset={false}
      >
        <div className="ps-share row">
          <BusyButton className="btn small" busy={io === "export"} disabled={!open || !!io} label="Export…" busyLabel="Exporting…" onClick={() => void exportNow()} />
          <BusyButton className="btn small" busy={io === "import"} disabled={!!io} label="Import…" busyLabel="Importing…" onClick={() => void importNow()} />
        <p className="ps-share-note muted">
          Community doesn't take plugins. To share one, post its .sma on GameBanana or AlliedModders yourself ({SHOW_IN} under More). To share a mode here, make it in{" "}
          <button type="button" className="link inline" onClick={() => go("tools", "studio")}>
            Game Mode Studio
          </button>
          .
        </p>
        </div>
      </ActionBar>
      {history && (
        <Sheet className="sheet wide hist-sheet" label="History & restore" onClose={() => setHistory(null)}>
          <div className="sheet-body">
            <h2>History & restore</h2>
            <p className="muted">Every time {open?.name ?? "the file"} is saved, the version before is kept (the last 20, and always the first one). Restoring keeps the current file too, so you can always go back.</p>
            <HistoryPanel match={[history]} none="No earlier versions yet: they're kept from the first time the file is saved here." />
            <div className="row">
              <button className="btn" onClick={() => setHistory(null)}>Close</button>
            </div>
          </div>
        </Sheet>
      )}
    </div>
  );
}

/** The action bar's More menu for the open file. */
const SHOW_IN = navigator.userAgent.includes("Windows") ? "Show in Explorer" : navigator.userAgent.includes("Mac") ? "Show in Finder" : "Show in folder";

function moreActions(p: { open: Open | null; busy: boolean; saveCopy: () => void; rename: () => void; remove: () => void; history: () => void; reset: () => void; reveal: () => void }): BarAction[] {
  const o = p.open;
  if (!o) return [];
  const mine = o.folder === "mine";
  return [
    { label: "Save a copy in My plugins…", onClick: p.saveCopy, note: "Your own version to change and try" },
    { label: SHOW_IN, onClick: p.reveal, note: "The .sma file, to post it yourself" },
    { label: "Rename…", onClick: p.rename, hidden: !mine },
    { label: "Restore a previous version…", onClick: p.history, note: "Earlier saves of this file: compare and restore", hidden: o.folder === "include" && !o.reset },
    { label: "Reset to Loner-Strike's version…", onClick: p.reset, danger: true, hidden: !o.reset?.differs, note: o.folder === "game" ? "The source and the plugin in your game" : "The copy in your game's include folder" },
    { label: "Delete…", onClick: p.remove, danger: true, hidden: !mine },
  ];
}

function FolderList(p: {
  label: string;
  path: string;
  start: boolean;
  files: SourceFile[];
  hidden: number;
  current: string | null;
  dirty: (n: string) => boolean;
  tried: (n: string) => boolean;
  onOpen: (n: string) => void;
  onDelete?: (n: string) => void;
  empty: string;
}) {
  const [shown, setShown] = useState(p.start);
  const [filter, setFilter] = useState("");
  const list = useMemo(() => (filter ? p.files.filter((f) => f.name.toLowerCase().includes(filter.toLowerCase())) : p.files), [p.files, filter]);
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
          {p.files.length > 12 && <input className="search ps-filter" placeholder="Filter" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label={`Filter ${p.label}`} />}
          {p.files.length === 0 && <p className="muted ps-folder-empty">{p.empty}</p>}
          {p.hidden > 0 && (
            <p className="muted ps-folder-empty" title={p.path}>
              {p.hidden === 1 ? "1 file hidden" : `${p.hidden} files hidden`}: rename {p.hidden === 1 ? "it" : "them"} to use letters, numbers, - and _.
            </p>
          )}
          <ul className="ps-file-list">
            {list.map(({ name: n, readOnly, changed }) => (
              <li key={n} className={n === p.current ? "on" : undefined}>
                <button className="ps-file" onClick={() => p.onOpen(n)} aria-current={n === p.current ? "true" : undefined} title={readOnly ? `${n}: read only${changed ? ", changed from Loner-Strike's version" : ""}` : n}>
                  <span className="ps-file-name">{n}</span>
                  {p.dirty(n) && <span className="ps-dirty" role="img" aria-label="Not saved" />}
                  {changed && <span className="ps-changed" role="img" aria-label="Changed from Loner-Strike's version" title="Changed from Loner-Strike's version">●</span>}
                  {readOnly && <span className="ps-ro" role="img" aria-label="Read only">🔒</span>}
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

function NewPlugin({ onCreate, onCancel }: { onCreate: (stem: string, template: string) => Promise<string | null>; onCancel: () => void }) {
  const [name, setName] = useState("my_plugin");
  const [template, setTemplate] = useState("mode");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = TEMPLATES.find((x) => x.id === template);
  return (
    <form
      className="ps-new"
      onSubmit={async (e) => {
        e.preventDefault();
        if (busy) return;
        setBusy(true);
        setError(await onCreate(name.trim(), template));
        setBusy(false);
      }}
    >
      <label className="field">
        <span>Name</span>
        <input
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setError(null);
          }}
          maxLength={60}
          spellCheck={false}
          autoFocus
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? "ps-new-error" : undefined}
        />
        {error ? (
          <small id="ps-new-error" className="ps-new-error" role="alert">
            {error}
          </small>
        ) : (
          <small>Letters, numbers, - and _. It becomes {name.trim() || "name"}.sma.</small>
        )}
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
  const slow = lines.filter((l) => l.slow).length;
  const shown = lines.filter((l) => !l.slow);
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
        {slow > 0 && (
          <p className="muted ps-log-slow">
            {slow === 1 ? "1 note" : `${slow} notes`} that a function took over 1 ms {slow === 1 ? "is" : "are"} hidden. Try it runs plugins in debug mode, so errors name their line, and debug mode writes these. They're harmless.
          </p>
        )}
        {shown.length === 0 ? (
          <p className="muted">Nothing yet.</p>
        ) : (
          shown.map((l, i) => (
            <div key={i} className={l.error ? "bad" : undefined}>
              <span className="ps-log-at">{l.at}</span> {l.text}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
