// "Start from": the composer's templates and your saved designs, in the tools' standard dialog
// (EditorDialog: a header with the title and ×, a body that scrolls, a footer that stays).
// Cards are one size in a grid of four (two on narrow windows); arrow keys move between them,
// focus starts on the design you have open or the first template, and Esc closes.

import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { Skeleton } from "../../../ui";
import { EditorDialog } from "../editor/EditorDialog";
import { toPng } from "./doc";
import type { DesignInfo } from "./sprayApi";
import { TEMPLATES, type Template } from "./templates";

/** Template pictures, drawn once the fonts are in (the game's font draws late the first time). */
function useTemplatePictures() {
  const draw = () => Object.fromEntries(TEMPLATES.map((t) => [t.id, toPng(t.make(), 0.375)]));
  const [pics, setPics] = useState<Record<string, string>>(draw);
  useEffect(() => {
    let live = true;
    void document.fonts?.load('48px "Counter-Strike"').then(
      () => live && setPics(draw()),
      () => {},
    );
    return () => {
      live = false;
    };
  }, []);
  return pics;
}

/** Arrow keys move between the cards of a grid (by its real column count). */
function moveInGrid(e: KeyboardEvent<HTMLElement>) {
  const keys: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 0, ArrowUp: 0, Home: 0, End: 0 };
  if (!(e.key in keys)) return;
  const grid = e.currentTarget;
  const cards = [...grid.querySelectorAll<HTMLElement>("[data-card]")];
  const at = cards.indexOf(document.activeElement as HTMLElement);
  if (at < 0) return;
  const cols = getComputedStyle(grid).gridTemplateColumns.split(" ").length || 1;
  let to = at;
  if (e.key === "ArrowRight") to = at + 1;
  else if (e.key === "ArrowLeft") to = at - 1;
  else if (e.key === "ArrowDown") to = at + cols;
  else if (e.key === "ArrowUp") to = at - cols;
  else if (e.key === "Home") to = 0;
  else if (e.key === "End") to = cards.length - 1;
  if (to < 0 || to >= cards.length) return;
  e.preventDefault();
  cards[to].focus();
}

export function StartFrom({
  designs,
  loading,
  openId,
  onTemplate,
  onOpen,
  onDelete,
  onClose,
}: {
  designs: DesignInfo[];
  loading: boolean;
  /** the saved design that's open now (marked) */
  openId: string | null;
  onTemplate: (t: Template) => void;
  onOpen: (d: DesignInfo) => void;
  onDelete: (d: DesignInfo) => void;
  onClose: () => void;
}) {
  const pics = useTemplatePictures();
  const first = useRef<HTMLButtonElement>(null);
  const current = useRef<HTMLButtonElement>(null);
  // focus the open design if there is one, else the first template (not the × in the corner)
  useEffect(() => {
    (current.current ?? first.current)?.focus();
  }, []);
  const sorted = useMemo(() => [...designs].sort((a, b) => b.updated - a.updated), [designs]);

  return (
    <EditorDialog title="Start a spray" subtitle="Pick a template to make your own, or open a design you saved." size="lg" onClose={onClose} bodyClass="sc-start" footerNote="Opening one replaces the canvas. You'll be asked first if it has changes you haven't saved.">
      <section className="sc-start-section" aria-labelledby="sc-start-yours">
        <div className="sc-start-head">
          <h3 id="sc-start-yours">Your designs</h3>
          {sorted.length > 0 && <span className="muted">{sorted.length}</span>}
        </div>
        {loading ? (
          <Skeleton rows={1} />
        ) : sorted.length === 0 ? (
          <div className="sc-start-empty">
            <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M9 9h6v12H9zM10 9V6h4v3M11 6V4h2M17 5h.01M19 3h.01M19 7h.01" />
            </svg>
            <div>
              <strong>No saved designs yet</strong>
              <p className="muted">Save design keeps what you make here, so you can come back and change it after you've seen it in game.</p>
            </div>
          </div>
        ) : (
          <div className="sc-start-grid" role="list" onKeyDown={moveInGrid}>
            {sorted.map((d) => (
              <div key={d.id} role="listitem" className={`sc-start-card saved${openId === d.id ? " on" : ""}`}>
                <button type="button" data-card ref={openId === d.id ? current : undefined} className="sc-start-open" onClick={() => onOpen(d)} aria-label={`Open ${d.name}`} aria-current={openId === d.id ? "true" : undefined}>
                  <span className="sc-start-pic">{d.thumb ? <img src={d.thumb} alt="" /> : null}</span>
                  <span className="sc-start-text">
                    <strong>{d.name}</strong>
                    <small className="muted">{openId === d.id ? "Open now" : d.updated ? `Saved ${new Date(d.updated * 1000).toLocaleDateString(undefined, { dateStyle: "medium" })}` : "Saved"}</small>
                  </span>
                </button>
                <button type="button" className="btn small icon sc-start-del" aria-label={`Delete ${d.name}`} title="Delete" onClick={() => onDelete(d)}>
                  ×
                </button>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="sc-start-section" aria-labelledby="sc-start-templates">
        <div className="sc-start-head">
          <h3 id="sc-start-templates">Templates</h3>
          <span className="muted">Made to read well at the game's small spray size</span>
        </div>
        <div className="sc-start-grid" role="list" onKeyDown={moveInGrid}>
          {TEMPLATES.map((t, i) => (
            <div key={t.id} role="listitem" className="sc-start-card">
              <button type="button" data-card ref={i === 0 ? first : undefined} className="sc-start-open" onClick={() => onTemplate(t)}>
                <span className={`sc-start-pic${t.id === "blank" ? " blank" : ""}`}>{t.id === "blank" ? <span aria-hidden="true">+</span> : <img src={pics[t.id]} alt="" />}</span>
                <span className="sc-start-text">
                  <strong>{t.label}</strong>
                  <small className="muted">{t.note}</small>
                </span>
              </button>
            </div>
          ))}
        </div>
      </section>
    </EditorDialog>
  );
}
