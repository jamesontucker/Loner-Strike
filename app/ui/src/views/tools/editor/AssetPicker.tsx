// P5: the entity form's asset buttons (Unreal's asset picker): beside a model, sprite or sound
// key, "Choose…" opens a searchable list of that kind with thumbnails, ↗ shows the file in the
// Content Browser, and "Use selected" takes what's picked there.

import { useEffect, useMemo, useRef, useState } from "react";
import { assetList, keyValue, revealAsset, sameAsset, spriteUrl, useBrowser, used, type AssetItem, type AssetKind } from "./contentApi";
import { cachedThumb, modelThumb } from "./modelThumbs";
import { showPanel } from "./dockState";
import "./p5.css";

function SmallThumb({ a }: { a: AssetItem }) {
  const [url, setUrl] = useState(() => (a.kind === "model" ? cachedThumb(a) : undefined));
  useEffect(() => {
    if (a.kind !== "model" || url !== undefined) return;
    let live = true;
    void modelThumb(a).then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, [a, url]);
  if (a.kind === "sprite") return <img className="p5-ap-thumb" src={spriteUrl(a)} alt="" loading="lazy" />;
  if (a.kind === "sound") return <span className="p5-ap-thumb p5-cb-sound" aria-hidden="true">♪</span>;
  return url ? <img className="p5-ap-thumb" src={url} alt="" /> : <span className="p5-ap-thumb">{url === null ? "" : <span className="spinner" aria-hidden="true" />}</span>;
}

/** The buttons for one key: `value` now, `set` writes the key. */
export function AssetButtons({ kind, value, set, label }: { kind: AssetKind; value: string; set: (v: string) => void; label: string }) {
  const [open, setOpen] = useState(false);
  const selected = useBrowser((s) => s.selected);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  const fits = selected?.kind === kind;
  return (
    <span className="p5-ap" ref={box}>
      <button type="button" className="btn small" aria-haspopup="listbox" aria-expanded={open} title={`Pick a ${kind} from the game's files`} onClick={() => setOpen(!open)}>
        Choose…
      </button>
      <button
        type="button"
        className="btn small icon"
        aria-label={`Show ${label} in the Content Browser`}
        title="Show it in the Content Browser"
        disabled={!value.trim()}
        onClick={() => {
          showPanel("content");
          revealAsset(kind, kind === "sound" && !/^sound\//i.test(value.trim()) ? `sound/${value.trim()}` : value.trim());
        }}
      >
        ↗
      </button>
      <button
        type="button"
        className="btn small"
        disabled={!fits}
        title={fits ? `Use ${selected!.path} (picked in the Content Browser)` : `Pick a ${kind} in the Content Browser first`}
        onClick={() => {
          if (!selected) return;
          set(keyValue(selected));
          used(selected);
        }}
      >
        Use selected
      </button>
      {open && (
        <Chooser
          kind={kind}
          value={value}
          onPick={(a) => {
            set(keyValue(a));
            used(a);
            setOpen(false);
          }}
        />
      )}
    </span>
  );
}

function Chooser({ kind, value, onPick }: { kind: AssetKind; value: string; onPick: (a: AssetItem) => void }) {
  const [items, setItems] = useState<AssetItem[] | null>(null);
  const [q, setQ] = useState("");
  useEffect(() => {
    let live = true;
    void assetList(kind).then((r) => live && setItems(r.items));
    return () => {
      live = false;
    };
  }, [kind]);
  const shown = useMemo(() => {
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    return (items ?? []).filter((a) => words.every((w) => a.path.toLowerCase().includes(w))).slice(0, 150);
  }, [items, q]);
  return (
    <div className="p5-ap-pop" role="dialog" aria-label={`Choose a ${kind}`}>
      <input autoFocus type="search" placeholder={`Search ${kind}s`} value={q} onChange={(e) => setQ(e.target.value)} aria-label={`Search ${kind}s`} />
      {!items ? (
        <p className="small-note muted" role="status">
          <span className="spinner" aria-hidden="true" /> Reading the game's folders…
        </p>
      ) : !shown.length ? (
        <p className="small-note muted">Nothing matches.</p>
      ) : (
        <ul role="listbox" aria-label={`${kind}s`}>
          {shown.map((a) => (
            <li key={`${a.folder}/${a.path}`} role="option" aria-selected={sameAsset(value, a)}>
              <button type="button" onClick={() => onPick(a)} title={a.path}>
                <SmallThumb a={a} />
                <span className="p5-ap-name">{a.path.replace(/^(models|sprites|sound)\//, "")}</span>
                <span className="muted small-note">{a.source === "game" ? "game" : a.source === "downloaded" ? a.page?.name ?? "downloaded" : a.source}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
