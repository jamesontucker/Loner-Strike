// A model, sprite or sound field with a picker (Game Mode Studio's Model, Sprite and Sound
// inputs, e.g. Objective Marker and Spawn NPC): type a path, or Choose… from the game's files,
// Import… one (or drop it on the field), or take one from your map projects. Sounds are named
// from inside sound/, the way the game reads them.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { assetList, keyValue, type AssetItem } from "../editor/contentApi";
import { matches, searchWords } from "./search";
import { CreditLine, EmptyImport, ImportBar, ImportZone, ProjectFilesGroup, useAssetImport, useFocusRefresh, type AssetCredit, type ImportKind } from "./AssetImport";

/** The kind a path names, by its extension ("" when it isn't one of ours). */
export function assetKindOf(path: string): ImportKind | null {
  const l = path.toLowerCase();
  return l.endsWith(".mdl") ? "model" : l.endsWith(".spr") ? "sprite" : l.endsWith(".wav") ? "sound" : null;
}

const WORD: Record<ImportKind, string> = { model: "model", sprite: "sprite", sound: "sound" };

export function AssetField({ kind, value, onChange, allowed, max = 64, className = "st-field", label }: { kind: ImportKind; value: string; onChange: (v: string) => void; allowed?: RegExp; max?: number; className?: string; label?: string }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [items, setItems] = useState<AssetItem[] | null>(null);
  const [fetching, setFetching] = useState(false);
  const [credit, setCredit] = useState<AssetCredit | undefined>(undefined);
  const box = useRef<HTMLDivElement>(null);
  const load = useCallback(
    (fresh: boolean) => {
      setFetching(true);
      void assetList(kind, fresh).then((r) => {
        setItems(r.items);
        setFetching(false);
      });
    },
    [kind],
  );
  useEffect(() => {
    if (open && !items) load(false);
  }, [open, items, load]);
  const rescan = useCallback(() => load(true), [load]);
  useFocusRefresh(useCallback(() => open && load(true), [open, load]));
  const set = (v: string) => (!allowed || allowed.test(v)) && v.length <= max && onChange(v);
  const imp = useAssetImport(
    kind,
    (path, cr) => {
      set(keyValue({ kind, path }));
      setCredit(cr ?? undefined);
      setOpen(false);
      setQ("");
    },
    rescan,
  );
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && !(e.target as HTMLElement).closest?.('[role="alertdialog"]') && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  const shown = useMemo(() => {
    const w = searchWords(q);
    return (items ?? []).filter((a) => matches(w, a.path)).slice(0, 150);
  }, [items, q]);
  return (
    <div className="ai-field" ref={box} {...imp.dropProps}>
      <input className={className} value={value} maxLength={max} aria-label={label} onChange={(e) => set(e.target.value)} />
      <button type="button" className="btn small" aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen(!open)} title={`Pick a ${WORD[kind]}, or import one`}>
        Choose…
      </button>
      {open && (
        <div className="ai-pop panel" role="dialog" aria-label={`Choose a ${WORD[kind]}`}>
          <ImportZone imp={imp}>
            <input className="search" type="search" autoFocus placeholder={`Search ${WORD[kind]}s`} value={q} aria-label={`Search ${WORD[kind]}s`} onChange={(e) => setQ(e.target.value)} />
            <ImportBar kind={kind} imp={imp} onRefresh={rescan} refreshing={fetching} />
            {!items ? (
              <p className="muted" role="status">
                <span className="spinner" aria-hidden="true" /> Reading the game's folders…
              </p>
            ) : (
              <ul className="cs-pop-list" role="listbox" aria-label={`${WORD[kind]}s`}>
                {shown.map((a) => (
                  <li key={`${a.folder}/${a.path}`} role="option" aria-selected={keyValue(a) === value}>
                    <button type="button" className={`cs-pick${keyValue(a) === value ? " on" : ""}`} onClick={() => (set(keyValue(a)), setCredit(undefined), setOpen(false), setQ(""))} title={a.path}>
                      <span>{a.path.replace(/^(models|sprites|sound)\//, "")}</span>
                      <span className="muted">{a.source === "game" ? "game" : a.page?.name ?? a.source}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            {items && shown.length === 0 && <EmptyImport kind={kind} imp={imp} what={WORD[kind]} />}
            <ProjectFilesGroup kind={kind} q={q} imp={imp} />
          </ImportZone>
        </div>
      )}
      <CreditLine credit={credit} />
    </div>
  );
}
