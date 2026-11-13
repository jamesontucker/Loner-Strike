// A creature type's model in the asset browser's 3D viewer (its own chunk, with three.js),
// playing the sequence the game would play for walking, running, attacking...: the type's own
// anim_* names, else by activity, else the player-model names (tools/creatures.rs `sequences`).

import { useEffect, useState } from "react";
import { getAnimation, getModel, type Animation, type ModelBundle } from "../assets/assetsApi";
import { ModelView } from "../assets/ModelView";
import type { ModelLook } from "./creaturesApi";

const MOVES = ["Idle", "Walk", "Run", "Attack", "Flinch", "Die"];

export default function CreaturePreview({ look, skin, tint, scale }: { look: ModelLook; skin: number; tint: string | null; scale: number }) {
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
  const seqName = seq != null ? look.sequences?.[seq]?.name : null;
  const once = move >= 3;
  return (
    <div className="cs-preview">
      <div className="cs-preview-view" style={tint ? { boxShadow: `inset 0 0 0 3px ${tint}, inset 0 0 40px ${tint}` } : undefined}>
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
          return (
            <button key={m} type="button" className={`btn small${move === i ? " primary" : ""}`} disabled={p?.seq == null} title={p?.seq == null ? "This model has nothing the game would play for this" : `Plays ${look.sequences?.[p.seq]?.name}`} onClick={() => setMove(i)}>
              {m}
            </button>
          );
        })}
      </div>
      <p className="small-note muted">
        {seqName ? <>Playing “{seqName}”{look.picks?.[move]?.how === "activity" ? " (picked by what it's for)" : look.picks?.[move]?.how === "player" ? " (a player-model name)" : ""}.</> : "Nothing to play for this."}
        {scale !== 1 && ` In the game it's ${scale}× this size.`}
        {tint && " The frame shows its tint."}
      </p>
    </div>
  );
}

