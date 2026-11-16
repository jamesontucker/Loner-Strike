// P5: the project viewer: the map's files (project, files beside it, backups, compile output,
// what's installed in the game, texture files, sounds/models/sprites and sky it uses) with
// open, show in folder, rename and delete (asked first), missing-file warnings, and recent
// projects.

import { useEffect, useState } from "react";
import { useAskText, useConfirm } from "../../../ui";
import * as api from "./editorApi";
import type { RecentFile } from "./editorApi";
import { fileHooks } from "./commands";
import { afterJump, copyAssets, findAssets, getProject, projectAction, type ProjectFile, type ProjectInfo } from "./p5store";
import { useEditor } from "./store";
import { remixUpdates, type RemixUpdate } from "./compileApi"; // Parity A

function size(n: number | null) {
  if (n == null) return "";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 102.4) / 10} kB`;
  return `${Math.round(n / 104857.6) / 10} MB`;
}
function when(t: number | null) {
  if (!t) return "";
  const d = new Date(t * 1000);
  const today = new Date().toDateString() === d.toDateString();
  return today ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : d.toLocaleDateString();
}

const GROUP_NOTE: Record<string, string> = {
  Project: "The project and files beside it",
  Backups: "Kept each time you save over the project (last 10)",
  Compiled: "What the compiler made last time",
  "In the game": "What Playtest and the other tools put in the game folder",
  "Texture files": "The WADs this map names",
  "Sounds, models and sky": "Files the map's entities and sky use",
};

export function ProjectPanel() {
  const docId = useEditor((s) => s.doc?.docId);
  const saved = useEditor((s) => s.doc?.saved);
  const name = useEditor((s) => s.doc?.name);
  const [files, setFiles] = useState<ProjectFile[] | null>(null);
  const [info, setInfo] = useState<ProjectInfo | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [recent, setRecent] = useState<RecentFile[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [onlyMissing, setOnlyMissing] = useState(false);
  const [n, setN] = useState(0);
  // Parity A: this remix's original was updated in Community since (from the list the app has)
  const [upd, setUpd] = useState<RemixUpdate[]>([]);
  const ask = useConfirm();
  const askText = useAskText();
  const notify = useEditor((s) => s.notify);

  useEffect(() => {
    let live = true;
    void getProject().then((r) => {
      if (!live) return;
      if (r.error) setErr(r.error);
      else {
        setErr(null);
        setFiles(r.files);
        setInfo(r.project);
      }
    });
    void api.getRecent().then((r) => live && !r.error && setRecent(r.files ?? []));
    void remixUpdates().then((r) => live && setUpd(r.updates ?? []));
    return () => {
      live = false;
    };
  }, [docId, saved, name, n]);

  const act = async (f: ProjectFile, action: "open" | "reveal" | "rename" | "delete") => {
    if (!f.path) return;
    let newName = "";
    if (action === "rename") {
      const v = await askText(`Rename ${f.label} to:`, { label: "New name", placeholder: f.label.replace(/\.[^.]+$/, ""), max: 64 }, { ok: "Rename" });
      if (!v?.trim()) return;
      newName = v.trim();
    }
    if (action === "delete" && !(await ask(`Delete ${f.label}? It's moved to the editor's deleted files folder, not gone for good.`, { ok: "Delete", danger: true }))) return;
    if (action === "open" && f.openHere) {
      const d = useEditor.getState().doc;
      if (d?.dirty && !(await ask(`“${d.name}” has unsaved changes. Leave them and open ${f.label}?`, { ok: "Leave them", danger: true }))) return;
    }
    setBusy(f.path + action);
    try {
      const r = await projectAction(action, f.path, newName);
      if (r.error) return notify(r.error, true);
      if (r.opened) {
        await useEditor.getState().load();
        return;
      }
      if (r.renamed) {
        notify(`Renamed to ${r.renamed}.`);
        if (r.state) await afterJump(r.state);
      }
      if (r.deleted) notify(`Deleted ${f.label}.`);
      setN((x) => x + 1);
    } finally {
      setBusy(null);
    }
  };

  // the models, sprites and sounds: into the game for Playtest, or next to the project to keep together
  const copy = async (to: "game" | "project") => {
    setBusy(`copy-${to}`);
    try {
      const r = await copyAssets(to);
      notify(r.error ?? r.message, !!r.error);
      setN((x) => x + 1);
    } finally {
      setBusy(null);
    }
  };

  if (err) return <section className="tools-panel"><p className="small-note">{err}</p></section>;
  if (!files || !info) {
    return (
      <section className="tools-panel">
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Looking at the map's files…
        </p>
      </section>
    );
  }
  const missing = files.filter((f) => f.missing);
  const shown = onlyMissing ? missing : files;
  const groups = [...new Set(shown.map((f) => f.group))];
  return (
    <section className="tools-panel ed-project">
      <div className="ed-project-head">
        <strong>{info.name}</strong>
        <span className="muted small-note">
          {info.folder ? info.folder : "Not saved yet"} · compiles as <span className="mono">{info.map}.bsp</span>
        </span>
      </div>
      {(() => {
        const u = info.project ? upd.find((x) => x.project === info.project) : undefined;
        return u ? (
          <p className="small-note ed-remix-note" data-testid="remix-updated">
            The original was updated.{" "}
            <a href={u.url} target="_blank" rel="noreferrer" title={`${u.original} by ${u.author} on GameBanana. Your remix stays as it is.`}>
              See what changed.
            </a>
          </p>
        ) : null;
      })()}
      {!info.project && (
        <div className="row wrap">
          <button type="button" className="btn small primary" onClick={() => void useEditor.getState().file("saveAs")}>
            Save as a project…
          </button>
        </div>
      )}
      {missing.length > 0 && (
        <p className="banner ed-project-missing" role="status">
          {missing.length} file{missing.length === 1 ? " this map uses is" : "s this map uses are"} missing on this computer.{" "}
          <button type="button" className="linklike" onClick={() => setOnlyMissing(!onlyMissing)}>
            {onlyMissing ? "Show all files" : "Show which"}
          </button>
        </p>
      )}
      {info.assets && info.assets.assets.some((a) => a.status !== "game" || !a.stock) && (
        <div className="ed-project-assets" role="group" aria-label="The map's models, sprites and sounds">
          <p className="small-note">
            {info.assets.missing > 0
              ? `${info.assets.missing} model, sprite, sound or sky file${info.assets.missing === 1 ? " isn't" : "s aren't"} on this computer. A missing model or sprite stops the server loading the map, so Playtest waits for those; a missing sound just doesn't play. Find them in a folder you pick, put them beside the project, or change the entities.`
              : info.assets.toGame > 0
                ? `${info.assets.toGame} file${info.assets.toGame === 1 ? " the map uses is" : "s the map uses are"} beside the map but not in the game yet. Playtest copies ${info.assets.toGame === 1 ? "it" : "them"} into the game's download folder.`
                : "Every model, sprite and sound the map uses is in the game."}
          </p>
          <div className="row wrap">
            {info.assets.missing > 0 && (
              <button
                type="button"
                className="btn small primary"
                disabled={!!busy}
                title="Pick a folder (the map's download, another game's cstrike folder…): the missing files found there are copied into the game and next to the project"
                onClick={async () => {
                  setBusy("find");
                  try {
                    const r = await findAssets();
                    if (r.error) notify(r.error, true);
                    else if (!r.cancelled) notify(r.message, r.missing.length > 0);
                    setN((x) => x + 1);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                {busy === "find" ? <><span className="spinner" aria-hidden="true" /> Looking…</> : "Find missing files…"}
              </button>
            )}
            {info.assets.toGame > 0 && (
              <button type="button" className="btn small" disabled={!!busy} onClick={() => void copy("game")} title="Into the game's download folder, where the game looks and servers send them from">
                {busy === "copy-game" ? <><span className="spinner" aria-hidden="true" /> Copying…</> : "Copy into the game"}
              </button>
            )}
            {info.project && info.assets.assets.some((a) => a.status !== "missing" && !a.stock && !a.inProject) && (
              <button type="button" className="btn small" disabled={!!busy} onClick={() => void copy("project")} title="So the project folder holds everything the map needs (to move or share it)">
                {busy === "copy-project" ? <><span className="spinner" aria-hidden="true" /> Copying…</> : "Copy next to the project"}
              </button>
            )}
          </div>
        </div>
      )}
      {!info.game && <p className="small-note muted">Set up the game on this computer to check texture files, sounds and models.</p>}
      <div className="row wrap">
        <button type="button" className="btn small" onClick={() => setN((x) => x + 1)}>
          Look again
        </button>
      </div>
      {groups.map((g) => (
        <div key={g} className="ed-project-group">
          <h4 title={GROUP_NOTE[g]}>
            {g} <span className="muted small-note">{shown.filter((f) => f.group === g).length}</span>
          </h4>
          <ul>
            {shown
              .filter((f) => f.group === g)
              .map((f, i) => (
                <li key={(f.path ?? f.label) + i} className={f.missing ? "missing" : f.exists ? undefined : "absent"}>
                  <div className="ed-project-file">
                    <span className="ed-project-name" title={f.path ?? f.note}>
                      {f.missing && <span className="ed-warn" aria-label="Missing" title="Missing on this computer">!</span>}
                      {f.label}
                    </span>
                    <span className="muted small-note">
                      {f.missing ? "Missing" : f.note}
                      {f.exists && (f.note ? " · " : "")}
                      {f.exists && `${size(f.size)} · ${when(f.modified)}`}
                    </span>
                  </div>
                  {f.exists && (
                    <span className="ed-project-actions">
                      {f.openHere ? (
                        <button type="button" className="btn small" disabled={!!busy} title="Open it in the editor" onClick={() => void act(f, "open")}>
                          Open
                        </button>
                      ) : (
                        !/Texture files|Sounds/.test(g) && (
                          <button type="button" className="btn small" disabled={!!busy} title="Open with this computer's app for it" onClick={() => void act(f, "open")}>
                            Open
                          </button>
                        )
                      )}
                      <button type="button" className="btn small" disabled={!!busy} title="Show it in its folder" onClick={() => void act(f, "reveal")}>
                        Show
                      </button>
                      {f.canRename && (
                        <button type="button" className="btn small" disabled={!!busy} onClick={() => void act(f, "rename")}>
                          Rename
                        </button>
                      )}
                      {f.canDelete && (
                        <button type="button" className="btn small danger" disabled={!!busy} onClick={() => void act(f, "delete")}>
                          Delete
                        </button>
                      )}
                    </span>
                  )}
                </li>
              ))}
          </ul>
        </div>
      ))}
      {recent.length > 0 && (
        <div className="ed-project-group">
          <h4>Recent projects</h4>
          <ul>
            {recent.map((f) => (
              <li key={f.index} className={f.found ? undefined : "absent"}>
                <div className="ed-project-file">
                  <button
                    type="button"
                    className="linklike"
                    disabled={!f.found}
                    title={f.found ? undefined : "This file isn't there any more (moved or deleted)"}
                    onClick={async () => {
                      const d = useEditor.getState().doc;
                      if (d?.dirty && !(await ask(`“${d.name}” has unsaved changes. Leave them and open ${f.name}?`, { ok: "Leave them", danger: true }))) return;
                      await useEditor.getState().file(f.index);
                    }}
                  >
                    {f.name}
                  </button>
                  <span className="muted small-note">
                    {f.folder}
                    {f.project ? "" : " · .map"}
                    {f.found ? "" : " · missing"}
                  </span>
                </div>
              </li>
            ))}
          </ul>
          <button type="button" className="btn small" onClick={() => fileHooks.replace("open")}>
            Open another…
          </button>
        </div>
      )}
    </section>
  );
}
