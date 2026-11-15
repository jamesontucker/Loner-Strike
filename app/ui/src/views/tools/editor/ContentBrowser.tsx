// P5: the Content Browser (docs/notes/Assets.md 3.2, after Unreal's): a dock panel with Models,
// Sprites, Sounds, Textures, Prefabs and GameBanana; a source tree (Game, Downloaded per page,
// Community, Mine, Favorites, Recent), search, thumbnail size; a details pane with where the
// file came from, its credits and a permission badge (T's credits data); drag a tile onto the
// map to place it (model → cycler_sprite, sprite → env_sprite, sound → ambient_generic, prefab
// → the prefab); right-click for Find uses. Also the content drawer (Ctrl+Space; ⌘⇧Space on a Mac), which shows
// the same browser over the bottom panels and closes on a click away.

import { SoundOptions } from "../common/SoundOptions";
import { memo, lazy, Suspense, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Skeleton, useAskText, useConfirm } from "../../../ui";
import {
  assetCredits,
  assetKey,
  assetList,
  copyToMine,
  deleteFile,
  exportAsset,
  getFolders,
  MINE_MIME,
  moveFile,
  movedAsset,
  newFolder,
  fileData,
  importFiles,
  IMPORT_TYPES,
  keyValue,
  showInFolder,
  sameAsset,
  soundInfo,
  spriteInfo,
  spriteUrl,
  toggleFavorite,
  useBrowser,
  verdictBadge,
  type AssetItem,
  type AssetKind,
  type AssetList,
  type Credits,
  type Folders,
  type ImportSummary,
  type SoundInfo,
  type SpriteInfo,
} from "./contentApi";
import { dragging } from "./extras";
import { SoundButton } from "./AssetPreview";
import { playSound, toggleSound } from "./soundPlayer";
import { cachedThumb, dropQueued, modelThumb } from "./modelThumbs";
import { COMPONENT_MIME } from "./P4Scene";
import { PrefabPanel } from "./PrefabPanel";
import { TextureBrowser } from "./Panels";
import { keyOf, originOf, useEditor } from "./store";
import { useTools } from "../../../stores/tools";
import { useNav } from "../../../store";
import type { Vec3 } from "../viewer/coords";
import { MakeSpriteButton } from "../common/SpriteMaker"; // T1fix
import "./p5.css";

const GetMore = lazy(() => import("../../GetMore").then((m) => ({ default: m.GetMore })));
const ModelPreview = lazy(() => import("./ModelPreview"));

type Section = AssetKind | "texture" | "prefab" | "gamebanana";
const SECTIONS: { id: Section; label: string }[] = [
  { id: "model", label: "Models" },
  { id: "sprite", label: "Sprites" },
  { id: "sound", label: "Sounds" },
  { id: "texture", label: "Textures" },
  { id: "prefab", label: "Prefabs" },
  { id: "gamebanana", label: "GameBanana" },
];
const SOURCE_LABEL: Record<string, string> = { game: "The game's own", downloaded: "Downloaded", community: "Community", mine: "Yours (next to the project)", other: "Other files in the game" };

const pref = (k: string, f: string) => {
  try {
    return localStorage.getItem(k) ?? f;
  } catch {
    return f;
  }
};
const keep = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private window */
  }
};
const kb = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

/** What a tile drags onto the map ("asset:<kind>:<folder>:<path>", extras.ts places it). */
export const assetDragId = (a: AssetItem) => `asset:${a.kind}:${a.folder}:${a.path}`;

/** The entities whose keys name this asset. */
export function usesOf(a: { kind: string; path: string }) {
  const ents = useEditor.getState().doc?.entities ?? [];
  return ents.filter((e) => e.keys.some(([k, v]) => k !== "classname" && k !== "targetname" && sameAsset(v, a)));
}

// ---- tiles -------------------------------------------------------------------------------------

function Thumb({ a, size }: { a: AssetItem; size: number }) {
  const box = useRef<HTMLDivElement>(null);
  const [url, setUrl] = useState<string | null | undefined>(() => (a.kind === "model" ? cachedThumb(a) : undefined));
  useEffect(() => {
    if (a.kind !== "model" || url !== undefined) return;
    const el = box.current;
    if (!el) return;
    let live = true;
    const io = new IntersectionObserver((ents) => {
      if (!ents.some((x) => x.isIntersecting)) return;
      io.disconnect();
      void modelThumb(a).then((u) => live && setUrl(u));
    });
    io.observe(el);
    return () => {
      live = false;
      io.disconnect();
    };
  }, [a, url]);
  return (
    <div ref={box} className="p5-cb-thumb" style={{ width: size, height: size }}>
      {a.kind === "sprite" ? (
        <img src={spriteUrl(a)} alt="" loading="lazy" draggable={false} />
      ) : a.kind === "sound" ? (
        <span className="p5-cb-sound" aria-hidden="true">
          ♪
        </span>
      ) : url ? (
        <img src={url} alt="" draggable={false} />
      ) : url === null ? (
        <span className="muted small-note">No picture</span>
      ) : (
        <span className="spinner" aria-label="Drawing the model" />
      )}
    </div>
  );
}

const play = (path: string) => toggleSound(path);

// ---- the browser -------------------------------------------------------------------------------

function ContentBrowserView({ drawer }: { drawer?: boolean }) {
  const [section, setSection] = useState<Section>(() => pref("ls.editor.content.section", "model") as Section);
  const [source, setSource] = useState("all");
  const [q, setQ] = useState("");
  const [size, setSize] = useState(() => Number(pref("ls.editor.content.size", "96")) || 96);
  const [hoverPlay, setHoverPlay] = useState(() => pref("ls.editor.content.hoverPlay", "0") === "1");
  const [list, setList] = useState<AssetList | null>(null);
  const [rev, setRev] = useState(0);
  const [sel, setSel] = useState<AssetItem | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; a: AssetItem } | null>(null);
  const [uses, setUses] = useState<AssetItem | null>(null);
  // Parity A: Import (a button, or files dropped from Finder or Explorer) and Export
  const [importing, setImporting] = useState(false);
  const [summary, setSummary] = useState<ImportSummary | null>(null);
  const [dropOver, setDropOver] = useState(false);
  const [exporting, setExporting] = useState<AssetItem | null>(null);
  const picker = useRef<HTMLInputElement>(null);
  const ask = useConfirm();
  const askText = useAskText();
  // Parity A: your own files: the project's subfolders (Mine), the one shown, a delete that asks
  const [folders, setFolders] = useState<Folders>({});
  const [mineDir, setMineDir] = useState<string | null>(null);
  const [dropDir, setDropDir] = useState<string | null>(null);
  const [delAsk, setDelAsk] = useState<{ a: AssetItem; text: string } | null>(null);
  // (the right-click menu kept on screen: near the bottom or right edge it opens up or left)
  const menuEl = useRef<HTMLUListElement>(null);
  useLayoutEffect(() => {
    const el = menuEl.current;
    if (!el || !menu) return;
    const r = el.getBoundingClientRect();
    const dx = Math.min(0, window.innerWidth - 8 - r.right);
    const dy = Math.min(0, window.innerHeight - 8 - r.bottom);
    if (dx || dy) {
      el.style.left = `${Math.max(8 - (r.left - menu.x), menu.x + dx)}px`;
      el.style.top = `${Math.max(8 - (r.top - menu.y), menu.y + dy)}px`;
    }
  }, [menu]);
  const favorites = useBrowser((s) => s.favorites);
  const recent = useBrowser((s) => s.recent);
  const reveal = useBrowser((s) => s.reveal);
  const goto = useBrowser((s) => s.goto);
  useEffect(() => {
    if (goto && SECTIONS.some((x) => x.id === goto.section)) setSection(goto.section as Section);
  }, [goto]);
  const grid = useRef<HTMLDivElement>(null);
  const kind = section === "model" || section === "sprite" || section === "sound" ? section : null;

  useEffect(() => keep("ls.editor.content.section", section), [section]);
  useEffect(() => {
    let live = true;
    void getFolders().then((r) => live && setFolders(r.folders ?? {}));
    return () => {
      live = false;
    };
  }, [rev]);
  useEffect(() => {
    if (!kind) return;
    let live = true;
    setList(null);
    void assetList(kind, rev > 0).then((r) => live && setList(r));
    return () => {
      live = false;
    };
  }, [kind, rev]);
  // "Show in Content Browser" from the entity form
  useEffect(() => {
    if (!reveal) return;
    setSection(reveal.kind);
    setSource("all");
    setQ("");
    void assetList(reveal.kind).then((r) => {
      const a = r.items.find((x) => sameAsset(reveal.path, x));
      if (!a) return;
      setSel(a);
      useBrowser.getState().set({ selected: a });
      requestAnimationFrame(() => grid.current?.querySelector(`[data-asset="${CSS.escape(assetKey(a))}"]`)?.scrollIntoView({ block: "nearest" }));
    });
  }, [reveal]);
  useEffect(() => {
    if (!menu) return;
    const away = () => setMenu(null);
    window.addEventListener("pointerdown", away);
    window.addEventListener("keydown", away);
    return () => {
      window.removeEventListener("pointerdown", away);
      window.removeEventListener("keydown", away);
    };
  }, [menu]);

  const shown = useMemo(() => {
    const items = list?.items ?? [];
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    const out = items.filter((a) => {
      if (source === "favorites" && !favorites.includes(assetKey(a))) return false;
      if (source === "recent" && !recent.includes(assetKey(a))) return false;
      if (source.startsWith("page:") && a.page?.key !== source.slice(5)) return false;
      if (["game", "downloaded", "community", "mine", "other"].includes(source) && a.source !== source) return false;
      if (source === "mine" && mineDir && !a.path.toLowerCase().startsWith(`${mineDir.toLowerCase()}/`)) return false;
      const hay = `${a.path} ${a.page?.name ?? ""}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    });
    if (source === "recent") out.sort((x, y) => recent.indexOf(assetKey(x)) - recent.indexOf(assetKey(y)));
    return out.slice(0, 600);
  }, [list, q, source, favorites, recent, mineDir]);
  // thumbnails no longer shown aren't drawn
  useEffect(() => {
    const want = new Set(shown.filter((a) => a.kind === "model").map((a) => `${a.folder}/${a.path}`));
    dropQueued((k) => want.has(k));
  }, [shown]);

  const pick = (a: AssetItem) => {
    setSel(a);
    useBrowser.getState().set({ selected: a });
  };
  const counts = (s: string) => (list?.items ?? []).filter((a) => a.source === s).length;

  // Parity A: files into the project's folder; a name that's taken asks before taking a new one
  const runImport = async (picked: File[]) => {
    const files = picked.filter((f) => /\.(mdl|spr|wav|mp3|ogg|oga|flac|wad)$/i.test(f.name));
    const skipped = picked.filter((f) => !files.includes(f)).map((f) => ({ name: f.name, why: "only models (.mdl), sprites (.spr), sounds (.wav .mp3 .ogg .flac) and texture files (.wad) can be imported" }));
    if (!files.length) return setSummary({ refused: skipped, message: "Nothing imported." });
    setImporting(true);
    setSummary(null);
    try {
      const data = await Promise.all(files.map(async (f) => ({ name: f.name, data: await fileData(f) })));
      let r = await importFiles(data);
      if (r.taken?.length && !r.error && !r.needProject) {
        const names = r.taken.map((t) => `${t.path} (as ${t.suggest})`).join(", ");
        if (await askRename(`${r.taken.length === 1 ? "This file is" : "These files are"} in the project already: ${names}. Import under the new name${r.taken.length === 1 ? "" : "s"}? Nothing there is replaced.`)) {
          const again = await importFiles(data.filter((d) => r.taken!.some((t) => t.name === d.name)).map((d) => ({ ...d, rename: true })));
          r = { ...r, imported: [...(r.imported ?? []), ...(again.imported ?? [])], renamed: [...(r.renamed ?? []), ...(again.renamed ?? [])], refused: [...(r.refused ?? []), ...(again.refused ?? [])], taken: again.taken ?? [] };
        }
      }
      r.refused = [...(r.refused ?? []), ...skipped];
      setSummary(r);
      if (r.imported?.length) setRev((n) => n + 1);
    } finally {
      setImporting(false);
    }
  };
  const askRename = (text: string) => ask(text, { ok: "Import under new names" });

  // Parity A: move, rename, delete your own files; the map's references follow in one undo step
  const fileName = (p: string) => p.slice(p.lastIndexOf("/") + 1);
  const dirOf = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "");
  const afterFileOp = (msg: string, extra?: string) => {
    setSummary({ message: extra ? `${msg} ${extra}` : msg });
    setRev((n) => n + 1);
    void useEditor.getState().load();
  };
  const doMove = async (a: AssetItem, to: string) => {
    if (to.toLowerCase() === a.path.toLowerCase()) return;
    const r = await moveFile(a.path, to);
    if (r.error) return setSummary({ error: r.error });
    movedAsset(a.kind, a.path, r.path ?? to);
    const refs = r.references ? ` ${r.references} place${r.references === 1 ? "" : "s"} in the map updated (Undo puts it all back).` : "";
    afterFileOp(`Moved to ${r.path}.${refs}`, r.others);
  };
  const rename = async (a: AssetItem) => {
    const ext = a.path.slice(a.path.lastIndexOf("."));
    const v = await askText(`Rename ${fileName(a.path)} to:`, { label: "New name", placeholder: fileName(a.path).slice(0, -ext.length), max: 60 }, { ok: "Rename" });
    if (!v?.trim()) return;
    const name = v.trim().toLowerCase().endsWith(ext.toLowerCase()) ? v.trim() : `${v.trim()}${ext}`;
    await doMove(a, dirOf(a.path) ? `${dirOf(a.path)}/${name}` : name);
  };
  const moveTo = async (a: AssetItem) => {
    const root = a.path.split("/")[0];
    const opts = [root, ...((folders[a.kind] as string[] | undefined) ?? [])];
    const v = await askText(`Move ${fileName(a.path)} to which folder? (${opts.join(", ")})`, { label: "Folder", placeholder: root, max: 120 }, { ok: "Move" });
    if (!v?.trim()) return;
    const dir = v.trim().replace(/\/+$/, "");
    await doMove(a, `${dir.toLowerCase().startsWith(`${root}/`) || dir === root ? dir : `${root}/${dir}`}/${fileName(a.path)}`);
  };
  const remove = async (a: AssetItem, confirm = false) => {
    const r = await deleteFile(a.path, confirm);
    if (r.error) return setSummary({ error: r.error });
    // (one floating box at a time: the question replaces Find uses)
    if (r.ask) return (setUses(null), setExporting(null), setDelAsk({ a, text: r.ask }));
    setDelAsk(null);
    movedAsset(a.kind, a.path, null);
    afterFileOp(`Deleted ${r.deleted?.join(", ")}. Undo brings ${r.deleted && r.deleted.length > 1 ? "them" : "it"} back.`);
  };
  const addFolder = async () => {
    if (!kind) return;
    const root = kind === "model" ? "models" : kind === "sprite" ? "sprites" : "sound";
    const v = await askText(`New folder in ${mineDir ?? root}:`, { label: "Folder name", placeholder: "props", max: 40 }, { ok: "Make it" });
    if (!v?.trim()) return;
    const r = await newFolder(kind, `${mineDir ?? root}/${v.trim()}`);
    if (r.error) return setSummary({ error: r.error });
    setFolders(r.folders ?? {});
    setMineDir(r.path ?? null);
  };
  const copyMine = async (a: AssetItem) => {
    setImporting(true);
    const r = await copyToMine(a);
    setImporting(false);
    setSummary(r);
    if (r.imported?.length) setRev((n) => n + 1);
  };
  const dropOn = (dir: string) => ({
    onDragOver: (e: React.DragEvent) => {
      if (!e.dataTransfer.types.includes(MINE_MIME)) return;
      e.preventDefault();
      e.stopPropagation();
      setDropDir(dir);
    },
    onDragLeave: () => setDropDir((d) => (d === dir ? null : d)),
    onDrop: (e: React.DragEvent) => {
      const from = e.dataTransfer.getData(MINE_MIME);
      if (!from) return;
      e.preventDefault();
      e.stopPropagation();
      setDropDir(null);
      const a = (list?.items ?? []).find((x) => x.source === "mine" && x.path === from);
      if (a) void doMove(a, `${dir}/${fileName(a.path)}`);
    },
  });

  return (
    <section
      className={`tools-panel p5-cb${drawer ? " in-drawer" : ""}${dropOver ? " drop-over" : ""}`}
      aria-label="Content Browser"
      onDragOver={(e) => {
        if (!e.dataTransfer.types.includes("Files")) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
        setDropOver(true);
      }}
      onDragLeave={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropOver(false);
      }}
      onDrop={(e) => {
        if (!e.dataTransfer.files?.length) return;
        e.preventDefault();
        setDropOver(false);
        void runImport([...e.dataTransfer.files]);
      }}
    >
      <div className="p5-cb-top">
        <input ref={picker} type="file" accept={IMPORT_TYPES} multiple hidden data-testid="content-import-input" onChange={(e) => { const f = [...(e.target.files ?? [])]; e.target.value = ""; void runImport(f); }} />
        <div className="seg" role="tablist" aria-label="Content">
          {SECTIONS.map((s) => (
            <button key={s.id} type="button" role="tab" aria-selected={section === s.id} aria-pressed={section === s.id} onClick={() => (setSection(s.id), setQ(""), setSource("all"))}>
              {s.label}
            </button>
          ))}
        </div>
        {kind && (
          <>
            <input type="search" className="p5-cb-search" placeholder={`Search ${SECTIONS.find((s) => s.id === section)!.label.toLowerCase()}`} value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search" />
            <label className="p5-cb-size" title="Thumbnail size">
              <span aria-hidden="true">▫</span>
              <input
                type="range"
                min={56}
                max={192}
                step={8}
                value={size}
                aria-label="Thumbnail size"
                onChange={(e) => {
                  setSize(Number(e.target.value));
                  keep("ls.editor.content.size", e.target.value);
                }}
              />
              <span aria-hidden="true">◻</span>
            </label>
            {kind === "sound" && (
              <label className="editor-check small-note" title="Play a sound when the pointer rests on it">
                <input
                  type="checkbox"
                  checked={hoverPlay}
                  onChange={(e) => {
                    setHoverPlay(e.target.checked);
                    keep("ls.editor.content.hoverPlay", e.target.checked ? "1" : "0");
                  }}
                />{" "}
                Play on hover
              </label>
            )}
            <button type="button" className="btn small" title="Read the folders again" onClick={() => setRev((n) => n + 1)}>
              Refresh
            </button>
            <button type="button" className="btn small" disabled={importing} title="Copy your own models, sprites, sounds or texture files (.mdl .spr .wav .mp3 .ogg .flac .wad) into the project's folder. Sounds become WAVs the game plays well. You can also drop them here." onClick={() => picker.current?.click()}>
              {importing ? <><span className="spinner" aria-hidden="true" /> Importing…</> : "Import…"}
            </button>
            <SoundOptions />
            {/* T1fix: a sprite made from pictures goes in like an imported one */}
            <MakeSpriteButton onMade={async (f) => { await runImport([f]); return true; }} />
          </>
        )}
      </div>
      {summary && (
        <div className="p5-cb-import" role="status" aria-live="polite">
          <div className="row">
            <strong className="small-note">{summary.error ?? summary.message}</strong>
            <span className="push" />
            <button type="button" className="btn small icon" aria-label="Close" onClick={() => setSummary(null)}>×</button>
          </div>
          {summary.needProject && (
            <div className="row wrap">
              <button type="button" className="btn small primary" onClick={() => void useEditor.getState().file("save")}>Save the project…</button>
              <button type="button" className="btn small" onClick={() => useNav.getState().go("tools", "assets")}>Tools › Add your own</button>
            </div>
          )}
          <ul className="small-note">
            {(summary.imported ?? []).map((x) => <li key={`i${x.path}`}>Imported {x.path}{x.converted ? ` (${x.converted})` : ""}</li>)}
            {(summary.renamed ?? []).map((x) => <li key={`r${x.to}`}>{x.from} was taken: saved as {x.to}</li>)}
            {(summary.taken ?? []).map((x) => <li key={`t${x.path}`}>Not imported: {x.path} is there already</li>)}
            {(summary.refused ?? []).map((x) => <li key={`x${x.name}`} className="compile-bad">Refused {x.name}: {x.why}</li>)}
          </ul>
          {!!summary.imported?.length && <p className="small-note muted">They're under Mine, and go with Playtest and Publish. Publish asks who made each one unless you mark it as yours.</p>}
        </div>
      )}
      {section === "texture" && <TextureBrowser />}
      {section === "prefab" && <PrefabPanel />}
      {section === "gamebanana" && (
        <Suspense fallback={<Skeleton rows={4} />}>
          <p className="small-note muted">Props and models (Weapon skins › Map props), sprites and sounds from GameBanana. Each page's credits and terms show before Download; what you download lands in Downloaded, with its page.</p>
          <GetMore only={["skins", "sprites", "sounds"]} title="From GameBanana" onInstalled={() => setRev((n) => n + 1)} />
        </Suspense>
      )}
      {kind && (
        <div className="p5-cb-body">
          <nav className="p5-cb-tree" aria-label="Where from">
            {[
              ["all", "Everything", list?.items.length],
              ["game", "Game", counts("game")],
              ["downloaded", "Downloaded", counts("downloaded")],
              ["community", "Community", counts("community")],
              ["mine", "Mine", counts("mine")],
              ["other", "Other", counts("other")],
            ].map(([id, label, n]) => (
              <span key={id as string} className="p5-cb-tree-item">
                <button
                  type="button"
                  aria-pressed={source === id && (id !== "mine" || !mineDir)}
                  onClick={() => (setSource(id as string), setMineDir(null))}
                  title={SOURCE_LABEL[id as string] ?? ""}
                  className={id === "mine" && dropDir === (kind === "model" ? "models" : kind === "sprite" ? "sprites" : "sound") ? "drop-over" : undefined}
                  {...(id === "mine" && kind ? dropOn(kind === "model" ? "models" : kind === "sprite" ? "sprites" : "sound") : {})}
                >
                  {label} {n != null && <span className="muted">{n as number}</span>}
                </button>
                {id === "mine" && source === "mine" && kind && (
                  <span className="p5-cb-folders" role="group" aria-label="Folders">
                    {((folders[kind] as string[] | undefined) ?? []).map((f) => (
                      <button
                        key={f}
                        type="button"
                        className={`p5-cb-folder${dropDir === f ? " drop-over" : ""}`}
                        style={{ paddingLeft: 10 + 10 * (f.split("/").length - 1) }}
                        aria-pressed={mineDir === f}
                        title={`${f}: drop a file here to move it`}
                        onClick={() => setMineDir(f)}
                        {...dropOn(f)}
                      >
                        {f.slice(f.lastIndexOf("/") + 1)}/
                      </button>
                    ))}
                    <button type="button" className="p5-cb-folder linklike" onClick={() => void addFolder()}>
                      + New folder
                    </button>
                  </span>
                )}
              </span>
            ))}
            {(list?.downloads ?? []).map((d) => (
              <button key={d.key} type="button" className="p5-cb-page" aria-pressed={source === `page:${d.key}`} title={d.url} onClick={() => setSource(`page:${d.key}`)}>
                {d.name} <span className="muted">{d.count}</span>
              </button>
            ))}
            <hr />
            <button type="button" aria-pressed={source === "favorites"} onClick={() => setSource("favorites")}>
              ★ Favorites <span className="muted">{favorites.filter((k) => k.startsWith(`${kind}:`)).length}</span>
            </button>
            <button type="button" aria-pressed={source === "recent"} onClick={() => setSource("recent")}>
              Recent <span className="muted">{recent.filter((k) => k.startsWith(`${kind}:`)).length}</span>
            </button>
          </nav>
          <div className="p5-cb-main">
            {!list ? (
              <p className="small-note muted" role="status">
                <span className="spinner" aria-hidden="true" /> Reading the game's folders…
              </p>
            ) : !list.game ? (
              <p className="small-note muted">Set up the game on this computer to browse its {section}s.</p>
            ) : !shown.length ? (
              <p className="small-note muted">{source === "favorites" ? "No favorites yet: right-click a file, Add to favorites." : source === "recent" ? "Nothing used yet: what you place or choose shows here." : q ? "Nothing matches." : "Nothing here."}</p>
            ) : (
              <div ref={grid} className="p5-cb-grid" role="listbox" aria-label={`${section}s`} style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${size + 8}px, 1fr))` }}>
                {shown.map((a) => (
                  <div
                    key={assetKey(a)}
                    data-asset={assetKey(a)}
                    role="option"
                    aria-selected={sel ? assetKey(sel) === assetKey(a) : false}
                    className="p5-cb-tile"
                    tabIndex={0}
                    draggable
                    title={`${a.path}\nDrag onto the map to place it. Right-click for more.`}
                    onClick={() => pick(a)}
                    onKeyDown={(e) => e.key === "Enter" && pick(a)}
                    onDoubleClick={() => a.kind === "sound" && play(a.path)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      pick(a);
                      setMenu({ x: e.clientX, y: e.clientY, a });
                    }}
                    onPointerEnter={() => hoverPlay && a.kind === "sound" && playSound(a.path)}
                    onDragStart={(e) => {
                      e.dataTransfer.setData(COMPONENT_MIME, assetDragId(a));
                      e.dataTransfer.setData("text/plain", keyValue(a));
                      if (a.source === "mine") e.dataTransfer.setData(MINE_MIME, a.path);
                      e.dataTransfer.effectAllowed = "copy";
                      dragging.current = { id: assetDragId(a), size: [32, 32, 32], component: true };
                    }}
                    onDragEnd={() => (dragging.current = null)}
                  >
                    <Thumb a={a} size={size} />
                    <span className="p5-cb-name">
                      {favorites.includes(assetKey(a)) && <span aria-label="Favorite">★ </span>}
                      {a.name}
                    </span>
                    {a.kind === "sound" && (
                      <span className="p5-cb-play">
                        <SoundButton path={a.path} label={a.name} />
                      </span>
                    )}
                  </div>
                ))}
              </div>
            )}
            {list?.capped && <p className="small-note muted">Only the first 6000 are listed: search to find the rest.</p>}
          </div>
          <Details a={sel} onUses={setUses} />
        </div>
      )}
      {menu && (
        <ul ref={menuEl} className="p5-cb-menu" role="menu" style={{ left: menu.x, top: menu.y }} onPointerDown={(e) => e.stopPropagation()}>
          <li role="none">
            <button type="button" role="menuitem" onClick={() => (setUses(menu.a), setMenu(null))}>
              Find uses in the map
            </button>
          </li>
          <li role="none">
            <button type="button" role="menuitem" onClick={() => (toggleFavorite(menu.a), setMenu(null))}>
              {favorites.includes(assetKey(menu.a)) ? "Remove from favorites" : "Add to favorites"}
            </button>
          </li>
          <li role="none">
            <button type="button" role="menuitem" onClick={() => (void navigator.clipboard?.writeText(keyValue(menu.a)), setMenu(null))}>
              Copy its path
            </button>
          </li>
          {menu.a.kind === "sound" && (
            <li role="none">
              <button type="button" role="menuitem" onClick={() => (play(menu.a.path), setMenu(null))}>
                Play
              </button>
            </li>
          )}
          {menu.a.source === "mine" ? (
            <>
              <li role="none">
                <button type="button" role="menuitem" onClick={() => { const a = menu.a; setMenu(null); void rename(a); }}>
                  Rename…
                </button>
              </li>
              <li role="none">
                <button type="button" role="menuitem" onClick={() => { const a = menu.a; setMenu(null); void moveTo(a); }}>
                  Move to…
                </button>
              </li>
              <li role="none">
                <button type="button" role="menuitem" onClick={() => { const a = menu.a; setMenu(null); void remove(a); }}>
                  Delete…
                </button>
              </li>
            </>
          ) : (
            <li role="none">
              <button type="button" role="menuitem" title="A copy of yours in the project's folder, to change (its credits stay with it)" onClick={() => { const a = menu.a; setMenu(null); void copyMine(a); }}>
                Copy to Mine
              </button>
            </li>
          )}
          <li role="none">
            <button type="button" role="menuitem" onClick={() => { const a = menu.a; setMenu(null); void showInFolder(a).then((r) => r.error && setSummary({ error: r.error })); }}>
              {/Win/i.test(navigator.platform) ? "Show in Explorer" : "Show in Finder"}
            </button>
          </li>
          <li role="none">
            <button type="button" role="menuitem" onClick={() => (setUses(null), setDelAsk(null), setExporting(menu.a), setMenu(null))}>
              Export…
            </button>
          </li>
        </ul>
      )}
      {exporting && <ExportBox a={exporting} onClose={() => setExporting(null)} onDone={(m) => setSummary({ message: m })} />}
      {delAsk && (
        <div className="p5-cb-uses" role="alertdialog" aria-label={`Delete ${delAsk.a.name}`}>
          <p className="small-note">{delAsk.text}</p>
          <div className="row">
            <button type="button" className="btn small danger" onClick={() => void remove(delAsk.a, true)}>Delete anyway</button>
            <button type="button" className="btn small" onClick={() => (setUses(delAsk.a), setDelAsk(null))}>Show uses</button>
            <button type="button" className="btn small" onClick={() => setDelAsk(null)}>Cancel</button>
          </div>
        </div>
      )}
      {uses && <Uses a={uses} onClose={() => setUses(null)} />}
    </section>
  );
}

/** Parity A: Export… asks first, with whose work it is (credits and the permission badge). */
function ExportBox({ a, onClose, onDone }: { a: AssetItem; onClose: () => void; onDone: (message: string) => void }) {
  const [c, setC] = useState<Credits | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    void assetCredits(a).then((x) => live && setC(x));
    return () => {
      live = false;
    };
  }, [a]);
  const badge = verdictBadge(c?.verdict);
  const who = c?.authors?.map((x) => x.name).join(", ") || c?.submitter?.name || (c?.source === "mine" ? "you" : c?.source === "stock" ? "Valve (the game's own)" : "not known");
  const go = async () => {
    setBusy(true);
    setErr(null);
    const r = await exportAsset(a);
    setBusy(false);
    if (r.error) return setErr(r.error);
    if (r.cancelled) return;
    onDone(`Exported ${r.files?.join(", ")} to ${r.folder}.`);
    onClose();
  };
  return (
    <div className="p5-cb-uses p5-cb-export" role="dialog" aria-label={`Export ${a.name}`}>
      <strong>Export {a.path}</strong>
      {a.kind === "model" && <p className="small-note muted">A model's texture (T) and sequence files beside it go too.</p>}
      {c === undefined ? (
        <p className="small-note muted"><span className="spinner" aria-hidden="true" /> Checking whose work it is…</p>
      ) : (
        <p className="small-note" data-testid="export-credits">
          Made by: <b>{who}</b>
          {c?.page?.url ? <> · <a href={c.page.url} target="_blank" rel="noreferrer">{c.page.name || "its page"}</a></> : null}{" "}
          <span className={`p5-cb-badge ${badge.tone}`} title={c?.why ?? badge.tip}>{badge.label}</span>
        </p>
      )}
      <p className="small-note muted">Credit its makers if you share it, and follow what its page allows.</p>
      {err && <p className="small-note compile-bad">{err}</p>}
      <div className="row">
        <button type="button" className="btn small primary" disabled={busy} onClick={() => void go()}>
          {busy ? <><span className="spinner" aria-hidden="true" /> Exporting…</> : "Choose a folder and export"}
        </button>
        <button type="button" className="btn small" onClick={onClose}>Cancel</button>
      </div>
    </div>
  );
}

/** Find uses: the entities whose keys name it (click one to pick it and look at it). */
function Uses({ a, onClose }: { a: AssetItem; onClose: () => void }) {
  useEditor((s) => s.doc?.revision);
  const ents = usesOf(a);
  const s = useEditor.getState();
  return (
    <div className="p5-cb-uses" role="dialog" aria-label={`Uses of ${a.name}`}>
      <div className="row">
        <strong>
          {ents.length ? `${ents.length} entit${ents.length === 1 ? "y uses" : "ies use"}` : "Nothing in the map uses"} {a.name}
        </strong>
        <span className="push" />
        {ents.length > 1 && (
          <button type="button" className="btn small" onClick={() => s.select({ brushes: [], entities: ents.map((e) => e.id), face: null })}>
            Pick them all
          </button>
        )}
        <button type="button" className="btn small icon" aria-label="Close" onClick={onClose}>
          ×
        </button>
      </div>
      <ul>
        {ents.slice(0, 200).map((e) => (
          <li key={e.id}>
            <button
              type="button"
              className="linklike"
              onClick={() => {
                s.select({ brushes: [], entities: [e.id], face: null });
                const o = originOf(e.keys);
                if (o) useTools.getState().look(o as Vec3, 96);
              }}
            >
              {e.classname} #{e.id}
              {keyOf(e.keys, "targetname") ? ` “${keyOf(e.keys, "targetname")}”` : ""}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

// ---- details -----------------------------------------------------------------------------------

function Badge({ c, loading }: { c: Credits | null; loading: boolean }) {
  if (loading)
    return (
      <span className="p5-cb-badge muted">
        <span className="spinner" aria-hidden="true" /> Checking…
      </span>
    );
  const b = verdictBadge(c?.verdict);
  return (
    <span className={`p5-cb-badge ${b.tone}`} title={c?.why ?? b.tip}>
      {b.label}
    </span>
  );
}

function Details({ a, onUses }: { a: AssetItem | null; onUses: (a: AssetItem) => void }) {
  const [credits, setCredits] = useState<Credits | null>(null);
  const [checking, setChecking] = useState(false);
  const [spr, setSpr] = useState<SpriteInfo | null>(null);
  const [snd, setSnd] = useState<SoundInfo | null>(null);
  const [frame, setFrame] = useState(0);
  const [look, setLook] = useState(false);
  useEditor((s) => s.doc?.revision);
  useEffect(() => {
    setCredits(null);
    setSpr(null);
    setSnd(null);
    setFrame(0);
    setLook(false);
    if (!a) return;
    let live = true;
    setChecking(true);
    void assetCredits(a).then((c) => live && (setCredits(c), setChecking(false)));
    if (a.kind === "sprite") void spriteInfo(a).then((i) => live && setSpr(i));
    if (a.kind === "sound") void soundInfo(a).then((i) => live && setSnd(i));
    return () => {
      live = false;
    };
  }, [a]);
  // the sprite's frames, played
  useEffect(() => {
    if (!spr || !(spr.frames > 1)) return;
    const id = setInterval(() => setFrame((f) => (f + 1) % spr.frames), 100);
    return () => clearInterval(id);
  }, [spr]);
  if (!a) return <aside className="p5-cb-details muted small-note">Click a file to see where it's from, its credits and what it allows. Drag it onto the map to place it.</aside>;
  const n = usesOf(a).length;
  return (
    <aside className="p5-cb-details" aria-label="Details">
      <strong className="p5-cb-dname">{a.name}</strong>
      <span className="mono small-note p5-cb-path">{a.path}</span>
      {a.kind === "sprite" && (
        <div className="p5-cb-preview">
          <img src={spriteUrl(a, frame)} alt={`${a.name}, frame ${frame + 1}`} />
        </div>
      )}
      {a.kind === "model" &&
        (look ? (
          <div className="p5-cb-preview model">
            <Suspense fallback={<span className="spinner" aria-hidden="true" />}>
              <ModelPreview path={a.path} />
            </Suspense>
          </div>
        ) : (
          <button type="button" className="btn small" onClick={() => setLook(true)}>
            Look in 3D
          </button>
        ))}
      {a.kind === "sound" && (
        <SoundButton path={a.path} label={a.name} />
      )}
      <dl className="p5-cb-facts">
        <dt>Size</dt>
        <dd>{kb(a.size)}</dd>
        <dt>In</dt>
        <dd>{a.folder === "project" ? "the project's folder" : a.folder}</dd>
        <dt>From</dt>
        <dd>
          {a.page ? (
            <>
              <a href={a.page.url} target="_blank" rel="noreferrer">
                {a.page.name}
              </a>
              {a.page.author ? ` by ${a.page.author}` : ""}
            </>
          ) : (
            SOURCE_LABEL[a.source]
          )}
        </dd>
        {spr && !spr.error && (
          <>
            <dt>Sprite</dt>
            <dd>
              {spr.width}×{spr.height}, {spr.frames} frame{spr.frames === 1 ? "" : "s"}, {spr.format}, {spr.orientation}
            </dd>
          </>
        )}
        {snd && !snd.error && snd.format === "wav" && (
          <>
            <dt>Sound</dt>
            <dd>
              {snd.seconds}s, {(snd.rate ?? 0) / 1000} kHz, {snd.bits}-bit, {snd.channels === 1 ? "mono" : "stereo"}
              {snd.loops ? ", loops" : ""}
            </dd>
          </>
        )}
        <dt>Used</dt>
        <dd>
          {n ? (
            <button type="button" className="linklike" onClick={() => onUses(a)}>
              by {n} entit{n === 1 ? "y" : "ies"}
            </button>
          ) : (
            "not in this map"
          )}
        </dd>
      </dl>
      {snd?.warn && <p className="small-note p5-cb-warn">{snd.warn}</p>}
      {snd?.note && <p className="small-note muted">{snd.note}</p>}
      <div className="p5-cb-rights">
        <span className="small-note muted">Sharing it in a map:</span> <Badge c={credits} loading={checking} />
        {credits?.authors?.length ? (
          <ul className="small-note">
            {credits.authors.slice(0, 12).map((x, i) => (
              <li key={i}>
                {x.group ? `${x.group}: ` : ""}
                {x.url ? (
                  <a href={x.url} target="_blank" rel="noreferrer">
                    {x.name}
                  </a>
                ) : (
                  x.name
                )}
                {x.role ? ` (${x.role})` : ""}
              </li>
            ))}
          </ul>
        ) : null}
        {credits?.ported && <p className="small-note">Ported from another game: credit its makers.</p>}
        {credits?.why && <p className="small-note muted">{credits.why}</p>}
      </div>
    </aside>
  );
}

// ---- the content drawer ------------------------------------------------------------------------

/** Parity A (PerfAudit #1): the dock re-renders its panels on every edit; the browser (and its
 *  grid of thumbnails) only needs to when its own props or state change. */
export const ContentBrowser = memo(ContentBrowserView);

/** Ctrl+Space (⌘⇧Space on a Mac): the browser slides up over the bottom panels; a click away closes it. */
export function ContentDrawer() {
  const open = useBrowser((s) => s.drawer);
  const box = useRef<HTMLDivElement>(null);
  const [rect, setRect] = useState<{ left: number; width: number; bottom: number; height: number } | null>(null);
  useEffect(() => {
    if (!open) return;
    const place = () => {
      const m = document.querySelector(".editor-page .editor-main")?.getBoundingClientRect();
      if (m) setRect({ left: m.left, width: m.width, bottom: window.innerHeight - m.bottom, height: Math.max(260, Math.round(m.height * 0.5)) });
    };
    place();
    window.addEventListener("resize", place);
    const away = (e: PointerEvent) => {
      const t = e.target as HTMLElement;
      // (a drag from the drawer onto the map is not a click away; nor are its menus and dialogs)
      if (box.current?.contains(t) || t.closest(".p5-cb-menu, .sheet, [role=dialog]")) return;
      useBrowser.getState().set({ drawer: false });
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && useBrowser.getState().set({ drawer: false });
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  if (!open || !rect) return null;
  return (
    <div ref={box} className="p5-cb-drawer" role="dialog" aria-label="Content drawer" style={{ left: rect.left, width: rect.width, bottom: rect.bottom, height: rect.height }}>
      <ContentBrowser drawer />
    </div>
  );
}
