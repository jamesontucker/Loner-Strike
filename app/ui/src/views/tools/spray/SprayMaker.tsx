// The spray composer: make your spray for Counter-Strike 1.6 and Condition Zero from layers
// (your pictures, words, stickers), each moved, turned, scaled, flipped and coloured, then see
// it as the game will draw it (its real size and colours, on a wall) and install it.
//
// The page draws the design itself and hands the drawing to the server (tools/spray.rs
// /use), which turns it into the game's tempdecal.wad exactly as it does a picture: the same
// preview, the same install, backups and "put back". Designs are kept as JSON in the app's
// folder (sprays/designs) and open again for changing.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDirty } from "../../../store";
import { BusyButton, Info, LoadError, Segmented, Skeleton, Switch, useConfirm, useToast } from "../../../ui";
import { Check, Empty } from "../common";
import { ActionBar } from "../common/ActionBar";
import { errorText, useBusy } from "../common/api";
import { CommunitySprays } from "../community/Hooks";
import { blankDoc, cleanDoc, fitSize, imageLayer, imagesReady, layerSize, newId, readPicture, shapeLayer, SHAPES, SIZE_PIXELS, textLayer, toPng, type Doc, type ImageLayer, type Layer, type Shape } from "./doc";
import { useHistory } from "./history";
import { AddPanel, Inspector, LayerList } from "./Panels";
import { deleteDesign, DEFAULT_CHOICE, installSpray, listDesigns, openDesign, previewSpray, restoreSpray, saveDesign, sendDrawn, sprayStatus, type Choice, type DesignInfo, type GameSpray, type SprayImage } from "./sprayApi";
import { Stage } from "./Stage";
import { StartFrom } from "./StartFrom";
import { FALLBACK, fontsReady } from "./sysfonts";
import { TEMPLATES } from "./templates";
import "./spray.css";

const SIZE_NOTE: Record<Choice["size"], string> = {
  small: "About the size of the game's own sprays",
  medium: "A bit bigger",
  large: "The biggest the game allows",
};

/** Walls to see the spray on: one drawn here, two from the game's own textures (when there). */
const WALLS: { id: string; label: string; tex?: string }[] = [
  { id: "plaster", label: "Plaster" },
  { id: "sand", label: "Sandstone", tex: "/api/tools/assets/wad/cstrike/cs_dust.wad/tex/csSandWall2.png" },
  { id: "crate", label: "Crate", tex: "/api/tools/assets/wad/cstrike/cs_dust.wad/tex/SandCrtLrgSd.png" },
];

const same = (a: Doc, b: Doc) => JSON.stringify(strip(a)) === JSON.stringify(strip(b));
/** the doc without layer ids (they're new on every open) */
const strip = (d: Doc) => ({ ...d, layers: d.layers.map(({ id: _id, ...l }) => l) });

function BarIcon({ d }: { d: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

export function SprayMaker() {
  const toast = useToast();
  const ask = useConfirm();
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["tools", "spray"], queryFn: sprayStatus });
  const designs = useQuery({ queryKey: ["tools", "spray", "designs"], queryFn: listDesigns });
  const first = useMemo(() => (TEMPLATES.find((t) => t.id === "graffiti") ?? TEMPLATES[0]).make(), []);
  const h = useHistory(first);
  const doc = h.doc;
  const [selected, setSelected] = useState<string | null>(null);
  const [snap, setSnap] = useState(true);
  const [design, setDesign] = useState<{ id: string | null; name: string; saved: Doc | null }>({ id: null, name: "My spray", saved: first });
  const [choice, setChoice] = useState<Choice>({ ...DEFAULT_CHOICE, clear: "alpha" });
  const [preview, setPreview] = useState<SprayImage | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [wall, setWall] = useState("sand");
  const [badWalls, setBadWalls] = useState<string[]>([]);
  const [wallW, setWallW] = useState<Record<string, number>>({});
  const [zoom, setZoom] = useState(3);
  const [lock, setLock] = useState(true);
  const [skip, setSkip] = useState<string[]>([]);
  const [done, setDone] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);
  const [restoring, setRestoring] = useState<string | null>(null);
  const { working, run } = useBusy();
  const file = useRef<HTMLInputElement>(null);
  const replacing = useRef<string | null>(null);
  const stageCanvas = useRef<HTMLCanvasElement>(null);

  const games = status.data?.games ?? [];
  const targets = games.filter((g) => !skip.includes(g.game)).map((g) => g.game);
  const dirty = design.saved ? !same(doc, design.saved) : doc.layers.length > 0;
  useDirty("tools:spray", dirty);
  const sel = doc.layers.find((l) => l.id === selected) ?? null;

  // ---- changes -------------------------------------------------------------------------
  const change = useCallback((id: string, v: Partial<Layer>, key?: string) => h.commit((d) => ({ ...d, layers: d.layers.map((l) => (l.id === id ? ({ ...l, ...v } as Layer) : l)) }), key ? `${id}:${key}` : undefined), [h]);
  const add = (l: Layer) => {
    h.commit((d) => ({ ...d, layers: [...d.layers, l] }));
    setSelected(l.id);
  };
  const remove = useCallback((id: string) => {
    h.commit((d) => ({ ...d, layers: d.layers.filter((l) => l.id !== id) }));
    setSelected(null);
  }, [h]);
  const duplicate = useCallback((id: string) => {
    const nid = newId();
    h.commit((d) => {
      const i = d.layers.findIndex((l) => l.id === id);
      if (i < 0) return d;
      const copy = { ...d.layers[i], id: nid, x: d.layers[i].x + 16, y: d.layers[i].y + 16, locked: false } as Layer;
      return { ...d, layers: [...d.layers.slice(0, i + 1), copy, ...d.layers.slice(i + 1)] };
    });
    setSelected(nid);
  }, [h]);
  const moveLayer = useCallback((id: string, by: number) => h.commit((d) => {
    const i = d.layers.findIndex((l) => l.id === id);
    const j = i + by;
    if (i < 0 || j < 0 || j >= d.layers.length) return d;
    const layers = [...d.layers];
    [layers[i], layers[j]] = [layers[j], layers[i]];
    return { ...d, layers };
  }), [h]);
  const setDoc = (v: Partial<Doc>, key?: string) =>
    h.commit((d) => {
      if (v.shape && v.shape !== d.shape) {
        // keep each layer in the same place relative to the canvas
        const a = SHAPES[d.shape];
        const b = SHAPES[v.shape];
        const k = Math.min(b.w / a.w, b.h / a.h);
        return { ...d, ...v, layers: d.layers.map((l) => ({ ...l, x: (l.x / a.w) * b.w, y: (l.y / a.h) * b.h, scale: l.scale * k })) };
      }
      return { ...d, ...v };
    }, key);
  const centre = () => sel && change(sel.id, { x: SHAPES[doc.shape].w / 2, y: SHAPES[doc.shape].h / 2 });
  const fill = () => {
    if (!sel) return;
    const { w, h: lh } = layerSize(sel);
    const s = SHAPES[doc.shape];
    change(sel.id, { x: s.w / 2, y: s.h / 2, rot: 0, scale: Math.min(s.w / w, s.h / lh) });
  };

  const addPictures = async (files: FileList | null) => {
    if (!files?.length) return;
    const into = replacing.current;
    replacing.current = null;
    for (const f of [...files].slice(0, into ? 1 : 8)) {
      try {
        const p = await readPicture(f);
        if (into) {
          const old = doc.layers.find((l) => l.id === into) as ImageLayer | undefined;
          if (old) {
            const k = Math.sqrt((old.w * old.h) / (p.w * p.h));
            change(into, { src: p.src, w: p.w * k, h: p.h * k, name: p.name.slice(0, 40) } as Partial<ImageLayer>);
          }
        } else add(imageLayer(doc, p.src, p.w, p.h, p.name));
      } catch (e) {
        toast(errorText(e, "Couldn't open that picture."), true);
      }
    }
  };
  const pickPictures = (replace: string | null = null) => {
    replacing.current = replace;
    if (file.current) {
      file.current.value = "";
      file.current.multiple = !replace;
      file.current.click();
    }
  };

  // ---- keys ----------------------------------------------------------------------------
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.closest("input, textarea, select, [contenteditable=true]") || t.closest(".sheet, [role=dialog]"))) return;
      if (!document.querySelector(".sc-page")) return;
      const mod = e.metaKey || e.ctrlKey;
      const k = e.key.toLowerCase();
      if (mod && k === "z") {
        e.preventDefault();
        if (e.shiftKey) h.redo();
        else h.undo();
      } else if (mod && k === "y") {
        e.preventDefault();
        h.redo();
      } else if (mod && k === "d" && sel) {
        e.preventDefault();
        duplicate(sel.id);
      } else if (!mod && (e.key === "Delete" || e.key === "Backspace") && sel) {
        e.preventDefault();
        remove(sel.id);
      } else if (!mod && e.key === "Escape" && sel) {
        setSelected(null);
      } else if (!mod && (e.key === "[" || e.key === "]") && sel) {
        moveLayer(sel.id, e.key === "]" ? 1 : -1);
      } else if (!mod && sel && !sel.locked && e.key.startsWith("Arrow")) {
        e.preventDefault();
        const step = e.shiftKey ? 10 : 1;
        const dx = e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
        const dy = e.key === "ArrowUp" ? -step : e.key === "ArrowDown" ? step : 0;
        change(sel.id, { x: sel.x + dx, y: sel.y + dy }, "nudge");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [h, sel, duplicate, remove, moveLayer, change]);

  // ---- the game's look: the drawing handed to the server, its preview back ---------------
  useEffect(() => {
    let live = true;
    setPreviewing(true);
    const t = setTimeout(async () => {
      await imagesReady(doc);
      await fontsReady(doc);
      await document.fonts?.ready;
      if (!live) return;
      if (!doc.layers.some((l) => !l.hidden) && !doc.background) {
        setPreviewing(false);
        setPreview(null);
        setPreviewError(null);
        return;
      }
      const u = await sendDrawn(design.name, toPng(doc));
      if (!live) return;
      const r = u.error ? u : await previewSpray(choice);
      if (!live) return;
      setPreviewing(false);
      if (r.error || !("png" in r) || !r.png) {
        setPreviewError(r.error || "Couldn't make the spray.");
        setPreview(null);
      } else {
        setPreviewError(null);
        setPreview(r as SprayImage);
      }
    }, 300);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [doc, choice, design.name]);

  const setC = (c: Partial<Choice>) => {
    setChoice((old) => ({ ...old, ...c }));
    setDone(null);
  };

  // ---- saving, opening, installing -----------------------------------------------------
  const thumb = () => toPng(doc, 128 / Math.max(SHAPES[doc.shape].w, SHAPES[doc.shape].h));
  const save = (asNew = false) =>
    run("Saving…", async () => {
      await imagesReady(doc);
      const r = await saveDesign(asNew ? null : design.id, design.name, strip(doc), thumb());
      if (r.error || !r.id) return toast(r.error || "Couldn't save the design.", true);
      setDesign({ id: r.id, name: r.name ?? design.name, saved: doc });
      qc.invalidateQueries({ queryKey: ["tools", "spray", "designs"] });
      toast(`Saved “${r.name ?? design.name}”.`);
    });
  const leaveOk = async () => !dirty || (await ask("Drop the changes to this design you haven't saved?", { ok: "Drop them" }));
  const startFrom = async (d: Doc, name: string) => {
    if (!(await leaveOk())) return;
    h.reset(d);
    setSelected(null);
    setDesign({ id: null, name, saved: d });
    setDone(null);
  };
  const open = async (d: DesignInfo) => {
    if (!(await leaveOk())) return;
    setOpening(false);
    await run("Opening…", async () => {
      const r = await openDesign(d.id).catch((e) => ({ error: errorText(e) }) as { error: string });
      if ("error" in r && r.error) return toast(r.error, true);
      const doc = cleanDoc((r as { doc: unknown }).doc);
      h.reset(doc);
      // fonts it was made with that this computer doesn't have: say so once
      const missing = await fontsReady(doc);
      if (missing.length) toast(`${missing.map((f) => `“${f}”`).join(", ")} ${missing.length > 1 ? "aren't" : "isn't"} installed on this computer, so ${missing.length > 1 ? "those words show" : "its words show"} in ${FALLBACK}. Pick another font in Lettering, or install it.`);
      setSelected(null);
      setDesign({ id: d.id, name: (r as { name: string }).name || d.name, saved: doc });
    });
  };
  const removeDesign = async (d: DesignInfo) => {
    if (!(await ask(`Delete the design “${d.name}”? It can't be brought back. A spray you installed from it stays.`, { ok: "Delete", danger: true }))) return;
    const r = await deleteDesign(d.id);
    if (r.error) return toast(r.error, true);
    if (design.id === d.id) setDesign((x) => ({ ...x, id: null, saved: null }));
    qc.invalidateQueries({ queryKey: ["tools", "spray", "designs"] });
  };
  const install = () =>
    run("Installing…", async () => {
      // the drawing the preview showed is the one installed
      await imagesReady(doc);
      await fontsReady(doc);
      const u = await sendDrawn(design.name, toPng(doc));
      if (u.error) return toast(u.error, true);
      const r = await installSpray(choice, targets, lock);
      if (r.error || !r.ok) return toast(r.error || "Couldn't install the spray.", true);
      const msg = r.message || "Spray installed. Press T in game to spray it.";
      setDone(msg);
      toast(msg);
      qc.invalidateQueries({ queryKey: ["tools", "spray"] });
    });
  const restore = async (g: GameSpray) => {
    setRestoring(g.game);
    const r = await restoreSpray(g.game);
    setRestoring(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't put your old spray back.", true);
    toast(r.message || "Your previous spray is back.");
    setDone(null);
    qc.invalidateQueries({ queryKey: ["tools", "spray"] });
  };

  if (status.isPending) return <Skeleton rows={4} />;
  if (status.isError) return <LoadError text="Couldn't load the spray maker." onRetry={() => status.refetch()} />;
  if (status.data?.error) return <LoadError text={status.data.error} onRetry={() => status.refetch()} />;
  if (games.length === 0)
    return <Empty title="No game found" text="Couldn't find Counter-Strike 1.6 or Condition Zero on this computer. Set up the game first, then come back to make a spray." />;

  const [gw, gh] = fitSize(SHAPES[doc.shape].w, SHAPES[doc.shape].h, SIZE_PIXELS[choice.size]);
  const wallTex = WALLS.find((w) => w.id === wall && !badWalls.includes(w.id))?.tex;
  const savedList = designs.data?.designs ?? [];

  return (
    <div className="tl-page sc-page">
      <input ref={file} type="file" accept="image/png,image/jpeg,image/gif,image/bmp,image/webp" hidden onChange={(e) => void addPictures(e.target.files)} />

      <div className="toolbar sc-bar">
        <label className="sc-name">
          <span className="sr-only">Design name</span>
          <input className="search" value={design.name} maxLength={60} aria-label="Design name" onChange={(e) => setDesign((d) => ({ ...d, name: e.target.value }))} />
        </label>
        <span className="muted sc-bar-note">{design.id ? (dirty ? "Changed since saved" : "Saved") : "Not saved yet"}</span>
        <span className="push" />
        {/* focused first: WebKit doesn't focus a clicked button, and the sheet gives focus back to it */}
        <button type="button" className="btn" onClick={(e) => (e.currentTarget.focus(), setOpening(true))}>
          Designs & templates…
        </button>
      </div>

      <div className="sc-grid">
        <div className="sc-left">
          <AddPanel onPicture={() => pickPictures()} onWords={() => add(textLayer(doc))} onSticker={(id) => add(shapeLayer(doc, id))} />
          <LayerList doc={doc} selected={selected} onSelect={setSelected} onChange={(id, v) => change(id, v)} onMove={moveLayer} onDuplicate={duplicate} onDelete={remove} />
        </div>

        <div className="sc-centre">
          <div className="panel sc-stage-panel">
            <div className="sc-canvas-bar" role="toolbar" aria-label="Canvas">
              <span className="sc-bar-group">
                <button type="button" className="btn small sc-ibtn" disabled={!h.canUndo} onClick={h.undo} title="Undo (Mod+Z)" aria-label="Undo">
                  <BarIcon d="M9 14L4 9l5-5M4 9h10a6 6 0 0 1 0 12h-3" />
                </button>
                <button type="button" className="btn small sc-ibtn" disabled={!h.canRedo} onClick={h.redo} title="Redo (Shift+Mod+Z)" aria-label="Redo">
                  <BarIcon d="M15 14l5-5-5-5M20 9H10a6 6 0 0 0 0 12h3" />
                </button>
              </span>
              <Segmented label="Shape" options={(Object.keys(SHAPES) as Shape[]).map((k) => [k, SHAPES[k].label] as const)} current={doc.shape} onPick={(v) => setDoc({ shape: v as Shape })} />
              <span className="sc-bar-group">
                <Segmented label="Background" values=",colour" labels="No background|Color" current={doc.background ? "colour" : ""} onPick={(v) => setDoc({ background: v ? "#202020" : "" })} />
                {doc.background && <input type="color" className="sc-colour" value={doc.background} aria-label="Background color" title="Background color" onChange={(e) => setDoc({ background: e.target.value }, "bg")} />}
              </span>
              <button type="button" className="btn small sc-snap" aria-pressed={snap} onClick={() => setSnap(!snap)} title="Snap to the middle, the edges and other layers while dragging (hold Alt to ignore)">
                <BarIcon d="M4 4v6a8 8 0 0 0 16 0V4h-5v6a3 3 0 0 1-6 0V4zM4 7h5M15 7h5" />
                Snap
              </button>
            </div>
            <Stage doc={doc} selected={selected} onSelect={setSelected} onBegin={h.begin} onLive={h.live} snap={snap} canvasRef={stageCanvas} />
            <p className="muted sc-hint">
              Drag to move, a corner to resize, the dot to turn. Shift: a straight line. Alt: no snapping. The game gets {gw}×{gh} pixels.
            </p>
          </div>
        </div>

        <div className="sc-right">
          <Inspector layer={sel} look={choice.look} onChange={(v, key) => sel && change(sel.id, v, key)} onReplacePicture={() => sel && pickPictures(sel.id)} onCentre={centre} onFit={fill} />
        </div>
      </div>

      <section className="panel sc-game" aria-label="In game">
        <div className="sc-game-head">
          <h3>In game</h3>
          <Info text="The spray as the game will draw it: shrunk to the size the game takes, its colors cut to the game's 255 (or one color), on a wall at the wall's own scale. Where your design has nothing, the wall shows." />
          {preview && (
            <span className="sc-facts">
              <span className="tag">{preview.width}×{preview.height} in game</span>
              <span className="tag">{preview.colour ? `${preview.colours} colors` : "one color"}</span>
            </span>
          )}
        </div>
        <div className="sc-game-body">
          <div className="sc-preview">
            <div className="sc-preview-bar">
              <Segmented label="Wall" options={WALLS.filter((w) => !badWalls.includes(w.id)).map((w) => [w.id, w.label] as const)} current={wallTex || wall === "plaster" ? wall : "plaster"} onPick={setWall} />
              <Segmented label="Zoom" values="1 2 3" labels="1×|2×|3×" current={String(zoom)} onPick={(v) => setZoom(Number(v))} />
            </div>
            <div className="sc-views">
              <div className={`spray-wall sc-wall${wallTex ? " tex" : ""}`} style={wallTex ? { backgroundImage: `url(${wallTex})`, backgroundSize: `${(wallW[wall] ?? 128) * zoom}px auto` } : undefined}>
                {wallTex && <img src={wallTex} alt="" hidden onLoad={(e) => { const w = e.currentTarget.naturalWidth; setWallW((m) => ({ ...m, [wall]: w })); }} onError={() => setBadWalls((b) => [...b, wall])} />}
                {preview ? (
                  <img className="spray-img sc-pixel" src={preview.png} width={preview.width * zoom} height={preview.height * zoom} alt="Your spray on a wall, as it looks in game" />
                ) : (
                  <p className="spray-empty">{previewError ?? (previewing ? "Making your spray…" : "Add something to see it here.")}</p>
                )}
                {previewing && preview && <span className="spinner spray-busy" aria-label="Updating the preview" />}
              </div>
              <figure className="sc-view">
                <div className="sc-actual">{preview && <img className="sc-pixel" src={preview.png} width={preview.width} height={preview.height} alt="Your spray at its real size" />}</div>
                <figcaption className="muted">Real size</figcaption>
              </figure>
            </div>
          </div>

          <div className="sc-options" role="group" aria-label="Spray settings">
            <h4>Spray settings</h4>
            <div className="sc-field">
              <span className="sc-field-name">Size</span>
              <div className="sc-field-control">
                <Segmented label="Size" values="small medium large" labels="Small|Medium|Large" current={choice.size} onPick={(v) => setC({ size: v as Choice["size"] })} />
                <small className="muted">{SIZE_NOTE[choice.size]}</small>
              </div>
            </div>
            <div className="sc-field">
              <span className="sc-field-name">
                Colors
                <Info text="Full color keeps your design's colors (up to 255); in game each pixel is solid or clear, never see-through. One color paints it in a single color, like the game's own sprays, and here see-through is real: how dark a part is sets how thick the paint is (black is solid, gray lets the wall show through, white is clear)." />
              </span>
              <div className="sc-field-control">
                <Segmented label="Colors" values="colour mono" labels="Full color|One color" current={choice.look} onPick={(v) => setC({ look: v as Choice["look"] })} />
              </div>
            </div>
            {choice.look === "mono" ? (
              <div className="sc-field">
                <span className="sc-field-name">Paint</span>
                <div className="sc-field-control sc-inline">
                  <input className="sc-colour" type="color" value={choice.paint} aria-label="Paint color" onChange={(e) => setC({ paint: e.target.value })} />
                  <label className="sc-inline">
                    <Switch on={choice.invert} onChange={(v) => setC({ invert: v })} label="Paint the light parts" /> Paint the light parts
                  </label>
                </div>
              </div>
            ) : (
              <div className="sc-field">
                <span className="sc-field-name">Smooth colors</span>
                <div className="sc-field-control sc-inline">
                  <Switch on={choice.dither} onChange={(v) => setC({ dither: v })} label="Smooth photo colors" />
                  <small className="muted">Fine dots; good for photos, not for logos</small>
                </div>
              </div>
            )}
            {games.length > 1 && (
              <div className="sc-field">
                <span className="sc-field-name">Install for</span>
                <div className="sc-field-control tl-checks">
                  {games.map((g) => (
                    <Check key={g.game} label={g.label} checked={!skip.includes(g.game)} onChange={(on) => (setSkip((s) => (on ? s.filter((x) => x !== g.game) : [...s, g.game])), setDone(null))} />
                  ))}
                </div>
              </div>
            )}
            <div className="sc-field">
              <span className="sc-field-name">
                Keep it
                <Info text="Picking a spray in the game's Options would replace yours. This makes the file read-only, like players usually do." />
              </span>
              <div className="sc-field-control sc-inline">
                <Switch on={lock} onChange={setLock} label="Keep it from being replaced" />
                <small className="muted">From being replaced by the game's Options</small>
              </div>
            </div>
            {done && (
              <p className="spray-done" role="status">
                {done} <span className="muted">If the game is open, it shows from the next server you join.</span>
              </p>
            )}
          </div>
        </div>
      </section>

      <section className="panel sc-now" aria-label="Your spray now">
        <h3>Your spray now</h3>
        <div className="spray-now">
          {games.map((g) => (
            <div className="spray-now-game" key={g.game}>
              <div className="spray-wall small">
                {g.current && !g.current.error ? <img className="spray-img" src={(g.current as SprayImage).png} alt={`Your spray in ${g.label}`} /> : <span className="muted">{g.current?.error ? "Can't read it" : "None yet"}</span>}
              </div>
              <div>
                <strong>{g.label}</strong>
                <p className="muted">{g.current && !g.current.error ? (g.readOnly ? "Read-only, so the game keeps it" : "The game may replace it from Options") : g.current?.error ? g.current.error : "The game makes one when you pick a spray in Options"}</p>
                {g.canRestore && <BusyButton className="btn small" busy={restoring === g.game} label="Put back the previous one" busyLabel="Putting back…" onClick={() => restore(g)} />}
              </div>
            </div>
          ))}
        </div>
      </section>
      <CommunitySprays />

      {opening && (
        <StartFrom
          designs={savedList}
          loading={designs.isPending}
          openId={design.id}
          onClose={() => setOpening(false)}
          onTemplate={(t) => {
            setOpening(false);
            void startFrom(t.make(), t.id === "blank" ? "My spray" : t.label);
          }}
          onOpen={(d) => void open(d)}
          onDelete={(d) => void removeDesign(d)}
        />
      )}

      <ActionBar
        dirty={dirty}
        working={working}
        where={design.id ? `Design “${design.name}”` : "A new design"}
        info="Save keeps the design in Loner-Strike's folder, to open and change later (Designs & templates). Install makes the spray from it and puts it in the game (tempdecal.wad); the spray you had is kept first, to put back."
        status={done && !dirty && !working ? <span className="tl-ab-state saved">✓ Installed</span> : undefined}
        actions={[{ label: "Save design", busyLabel: "Saving…", onClick: () => save(), disabled: !dirty && !!design.id }]}
        more={[
          { label: "Save as a new design", busyLabel: "Saving…", onClick: () => save(true), hidden: !design.id },
          { label: "Add a picture…", onClick: () => pickPictures() },
          { label: "Start a blank design", onClick: () => void startFrom(blankDoc(doc.shape as Shape), "My spray") },
        ]}
        save={{ label: "Install spray", busyLabel: "Installing…", onClick: install, disabled: !preview || targets.length === 0, title: !preview ? "Add something first" : targets.length === 0 ? "Tick a game to install it for" : undefined }}
      />
    </div>
  );
}

export default SprayMaker;
