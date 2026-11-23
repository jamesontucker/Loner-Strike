// A creature type's model in the asset browser's 3D viewer (its own chunk, with three.js),
// drawn as the game draws it: pev->scale, and kRenderFxGlowShell in the tint colour with
// renderamt 12 (the stand-in tint only when a stand-in model plays it). It plays the sequence
// the game would play for idle, walk, run, attack, flinch and death (tools/creatures.rs
// `sequences`: the type's anim_* names, else by activity, else player-model names).

import { useEffect, useState } from "react";
import { getAnimation, getModel, type Animation, type ModelBundle } from "../assets/assetsApi";
import { ModelView } from "../assets/ModelView";
import type { ModelLook, Vec3 } from "./creaturesApi";

const MOVES = ["Idle", "Walk", "Run", "Attack", "Flinch", "Die"];
/** the brain's renderamt for a glow shell (creature.cpp Setup) */
const SHELL_AMOUNT = 12;

export default function CreaturePreview({ look, skin, glow, fallbackGlow, scale, shellOver, animScale = 1, waiting = false }: { look: ModelLook; skin: number; glow: Vec3; fallbackGlow: Vec3; scale: number; shellOver?: { color: Vec3; amount: number } | null; animScale?: number; /** the next type's model is still being looked up */ waiting?: boolean }) {
  const item = look.model!;
  const key = `${item.folder}/${item.path}`;
  // the model on show, with the item it came from: while the next one loads (switching types
  // quickly) the last stays up, dimmed, in the same 3D view. A new view per model went blank
  // between switches, and could lose its drawing context; a fresh item with the last model
  // for one render drew one creature's mesh with another's textures.
  const [shown, setShown] = useState<{ key: string; item: typeof item; bundle: ModelBundle } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [move, setMove] = useState(1);
  const [anim, setAnim] = useState<{ key: string; seq: number; anim: Animation } | null>(null);
  const [lost, setLost] = useState(0);

  useEffect(() => {
    const ac = new AbortController();
    setError(null);
    getModel(item, ac.signal).then(
      (b) => !ac.signal.aborted && setShown({ key, item, bundle: b }),
      () => !ac.signal.aborted && setError("Couldn't read this model."),
    );
    return () => ac.abort();
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps

  const seq = look.picks?.[move]?.seq ?? null;
  const current = shown?.key === key ? shown : null;
  useEffect(() => {
    if (seq == null || !current) return;
    const ac = new AbortController();
    getAnimation(current.item, seq, ac.signal).then(
      (a) => !ac.signal.aborted && setAnim({ key: current.key, seq, anim: a }),
      () => undefined,
    );
    return () => ac.abort();
  }, [current, seq]);
  // only the shown model's own animation (another model's would pull its vertices apart)
  const playAnim = current && anim && anim.key === current.key && anim.seq === seq ? anim.anim : null;

  if (error) return <p className="muted">{error}</p>;
  const on = (c: Vec3) => c.some((v) => v > 0);
  const color = on(glow) ? glow : look.report.standIn && on(fallbackGlow) ? fallbackGlow : null;
  // (a tier's or enraged look over the type's own tint)
  const shell = shellOver ?? (color ? { color, amount: SHELL_AMOUNT } : null);
  const seqName = seq != null ? look.sequences?.[seq]?.name : null;
  const once = move >= 3;
  // no run of its own: the game plays the walk faster (ResolveAnims)
  const runIsWalk = move === 2 && look.noRun;
  return (
    <div className="cs-preview">
      <div className="cs-preview-view">
        {shown ? (
          <ModelView
            key={lost}
            item={shown.item}
            bundle={shown.bundle}
            skin={skin}
            body={[]}
            anim={playAnim}
            playing={!!playAnim}
            wire={false}
            loop={!once}
            speed={(runIsWalk ? 2 : 1) * animScale}
            scale={scale}
            shell={shell}
            onEnded={() => once && move !== 5 && setMove(1)}
            onLost={() => setTimeout(() => setLost((n) => n + 1), 500)}
          />
        ) : null}
        {(!current || waiting) && (
          <p className={`muted cs-preview-wait${shown ? " over" : ""}`}>
            <span className="spinner" aria-hidden="true" /> Opening the model…
          </p>
        )}
      </div>
      <div className="cs-moves" role="group" aria-label="Play">
        {MOVES.map((m, i) => {
          const p = look.picks?.[i];
          const none = p?.seq == null;
          return (
            <button key={m} type="button" className={`btn small${move === i ? " primary" : ""}`} aria-pressed={move === i} disabled={none} title={none ? "This model has nothing the game would play for this" : i === 2 && look.noRun ? "No run of its own: plays its walk faster" : `Plays ${look.sequences?.[p!.seq!]?.name}`} onClick={() => setMove(i)}>
              {m}
            </button>
          );
        })}
      </div>
      <p className="cs-note">
        {seqName ? <>Playing “{seqName}”{runIsWalk ? ", sped up: this model has no run of its own" : look.picks?.[move]?.how === "activity" ? " (picked by what it's for)" : look.picks?.[move]?.how === "player" ? " (a player-model name)" : ""}.</> : "Nothing to play for this."}
        {look.noRun && move !== 2 && " No run animation: it runs by playing its walk faster."}
        {color && (on(glow) ? " Tinted as in the game." : " Tinted: the stand-in model plays it.")}
        {scale !== 1 && ` Drawn at ${scale}×.`}
      </p>
    </div>
  );
}
