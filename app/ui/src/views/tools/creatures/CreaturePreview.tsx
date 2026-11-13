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

export default function CreaturePreview({ look, skin, glow, fallbackGlow, scale }: { look: ModelLook; skin: number; glow: Vec3; fallbackGlow: Vec3; scale: number }) {
  const item = look.model!;
  const [bundle, setBundle] = useState<ModelBundle | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [move, setMove] = useState(1);
  const [anim, setAnim] = useState<Animation | null>(null);
  const [lost, setLost] = useState(0);
  const key = `${item.folder}/${item.path}`;

  useEffect(() => {
    const ac = new AbortController();
    setBundle(null);
    setError(null);
    getModel(item, ac.signal).then(
      (b) => !ac.signal.aborted && setBundle(b),
      () => !ac.signal.aborted && setError("Couldn't read this model."),
    );
    return () => ac.abort();
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps

  const seq = look.picks?.[move]?.seq ?? null;
  useEffect(() => {
    setAnim(null);
    if (seq == null || !bundle) return;
    const ac = new AbortController();
    getAnimation(item, seq, ac.signal).then(
      (a) => !ac.signal.aborted && setAnim(a),
      () => undefined,
    );
    return () => ac.abort();
  }, [key, seq, bundle]); // eslint-disable-line react-hooks/exhaustive-deps

  if (error) return <p className="muted">{error}</p>;
  const on = (c: Vec3) => c.some((v) => v > 0);
  const color = on(glow) ? glow : look.report.standIn && on(fallbackGlow) ? fallbackGlow : null;
  const shell = color ? { color, amount: SHELL_AMOUNT } : null;
  const seqName = seq != null ? look.sequences?.[seq]?.name : null;
  const once = move >= 3;
  // no run of its own: the game plays the walk faster (ResolveAnims)
  const runIsWalk = move === 2 && look.noRun;
  return (
    <div className="cs-preview">
      <div className="cs-preview-view">
        {bundle ? (
          <ModelView
            key={lost}
            item={item}
            bundle={bundle}
            skin={skin}
            body={[]}
            anim={anim}
            playing={!!anim}
            wire={false}
            loop={!once}
            speed={runIsWalk ? 2 : 1}
            scale={scale}
            shell={shell}
            onEnded={() => once && move !== 5 && setMove(1)}
            onLost={() => setTimeout(() => setLost((n) => n + 1), 500)}
          />
        ) : (
          <p className="muted cs-preview-wait">
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
