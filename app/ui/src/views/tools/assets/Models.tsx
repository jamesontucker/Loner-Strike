// The model browser: the game's models in a list (player models, weapons in hand, carried
// and on the ground, hostages, props) with search, and the picked one in 3D with its skins,
// body parts and animations.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNav } from "../../../store";
import { BusyButton, LoadError, Skeleton, Switch, useConfirm, useToast } from "../../../ui";
import { exportPng, getAnimation, getModel, modelTextureUrl, removeModel, useModelInfo, useModels, type ModelItem, type ModelKind } from "./assetsApi";
import { Row, Rows } from "../common";
import { ModelView } from "./ModelView";
import { matches, searchWords } from "../common/search";

const KINDS: [ModelKind | "all", string][] = [
  ["all", "All"],
  ["player", "Player models"],
  ["view", "Weapons in hand"],
  ["carried", "Weapons carried"],
  ["world", "Weapons on the ground"],
  ["hostage", "Hostages"],
  ["prop", "Props"],
];
const PAGE = 200;

function canDraw3d(): boolean {
  try {
    const c = document.createElement("canvas");
    const gl = c.getContext("webgl2") ?? c.getContext("webgl");
    (gl as WebGL2RenderingContext | null)?.getExtension("WEBGL_lose_context")?.loseContext();
    return !!gl;
  } catch {
    return false;
  }
}

export function Models() {
  const models = useModels();
  const [kind, setKind] = useState<ModelKind | "all">("all");
  const [q, setQ] = useState("");
  const [shown, setShown] = useState(PAGE);
  const [picked, setPicked] = useState<ModelItem | null>(null);

  const needle = q.trim().toLowerCase();
  const list = useMemo(
    () => (models.data ?? []).filter((m) => (kind === "all" || m.kind === kind) && matches(searchWords(needle), m.path, m.name)),
    [models.data, kind, needle],
  );
  useEffect(() => setShown(PAGE), [kind, needle]);
  // start on a player model, like a model viewer would
  useEffect(() => {
    if (!picked && models.data?.length) setPicked(models.data.find((m) => m.kind === "player") ?? models.data[0]);
  }, [models.data, picked]);

  if (models.isLoading) return <Skeleton rows={6} />;
  if (models.error) return <LoadError text={(models.error as Error).message} onRetry={() => models.refetch()} />;

  return (
    <div className="assets-models">
      <div className="panel assets-model-list">
        <input className="search assets-search" type="search" placeholder="Search models" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search models" />
        <select value={kind} onChange={(e) => setKind(e.target.value as ModelKind | "all")} aria-label="Kind of model">
          {KINDS.map(([k, label]) => <option key={k} value={k}>{label}</option>)}
        </select>
        <p className="muted assets-count">{list.length} model{list.length === 1 ? "" : "s"}</p>
        <ul className="assets-list" aria-label="Models">
          {list.slice(0, shown).map((m) => (
            <li key={`${m.folder}/${m.path}`}>
              <button className="assets-list-item" aria-pressed={picked?.folder === m.folder && picked.path === m.path} onClick={() => setPicked(m)} title={`${m.folder}/${m.path}`}>
                <span className="assets-list-name">{m.name}</span>
                <span className="muted assets-list-where">{m.folder}{m.imported ? " · yours" : ""}</span>
              </button>
            </li>
          ))}
          {list.length > shown && (
            <li><button className="btn assets-more" onClick={() => setShown((n) => n + PAGE)}>Show more ({list.length - shown} left)</button></li>
          )}
        </ul>
      </div>
      {picked ? <ModelPanel key={`${picked.folder}/${picked.path}`} item={picked} onRemoved={() => setPicked(null)} /> : (
        <div className="panel"><p className="muted">No models found.</p></div>
      )}
    </div>
  );
}

function ModelPanel({ item, onRemoved }: { item: ModelItem; onRemoved: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const qc = useQueryClient();
  const webgl = useMemo(canDraw3d, []);
  const [lost, setLost] = useState(0);
  const bundle = useQuery({ queryKey: ["tools", "assets", "model", item.folder, item.path, item.size], queryFn: ({ signal }) => getModel(item, signal), staleTime: Infinity, gcTime: 60_000 });
  const info = useModelInfo(item);
  const [skin, setSkin] = useState(0);
  const [body, setBody] = useState<number[]>([]);
  const [seq, setSeq] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [wire, setWire] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const b = bundle.data;
  // the animation is fetched once it's wanted (another one picked, or Play)
  const wantAnim = !!b && b.sequences.length > 0 && (seq !== 0 || playing);
  const anim = useQuery({ queryKey: ["tools", "assets", "anim", item.folder, item.path, item.size, seq], queryFn: ({ signal }) => getAnimation(item, seq, signal), enabled: wantAnim, staleTime: Infinity, gcTime: 30_000 });
  useEffect(() => {
    if (b) setBody(b.bodyparts.map(() => 0));
  }, [b]);

  const skins = Math.max(1, b?.skins.length ?? 1);
  const triangles = b ? b.draws.filter((d) => (body[d.part] ?? 0) === d.model).reduce((n, d) => n + d.count / 3, 0) : 0;

  const saveTexture = async (i: number) => {
    setBusy(`tex${i}`);
    const r = await exportPng({ kind: "model", folder: item.folder, path: item.path, i });
    setBusy(null);
    if (r.cancelled) return;
    if (r.error || !r.ok) return toast(r.error || "Couldn't save the picture.", true);
    toast(r.message || "Saved.");
  };
  const remove = async () => {
    if (!(await ask(`Remove ${item.name}? It was added with this browser, so its file is deleted from the game folder.`, { ok: "Remove", danger: true }))) return;
    setBusy("remove");
    const r = await removeModel(item);
    setBusy(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't remove it.", true);
    toast(`Removed ${item.name}.`);
    await qc.invalidateQueries({ queryKey: ["tools", "assets", "models"] });
    onRemoved();
  };

  return (
    <div className="assets-model">
      <div className="panel assets-stage">
        {!webgl ? (
          <p className="muted">Your system can't show 3D here, so models can't be shown.</p>
        ) : bundle.isLoading ? (
          <div className="assets-stage-wait"><span className="spinner" aria-hidden /> Reading the model…</div>
        ) : bundle.error ? (
          <LoadError text={(bundle.error as Error).message} onRetry={() => bundle.refetch()} />
        ) : b ? (
          <ModelView key={lost} item={item} bundle={b} skin={skin} body={body} anim={anim.data ?? null} playing={playing && !!anim.data} wire={wire} onLost={() => setTimeout(() => setLost((n) => n + 1), 500)} />
        ) : null}
        {anim.isFetching && <span className="spinner assets-stage-busy" aria-label="Reading the animation" />}
      </div>
      <div className="panel assets-model-side">
        <h3 className="assets-detail-name">{item.name}</h3>
        <p className="muted assets-path">{item.folder}/{item.path}</p>
        {b && (
          <Rows bare>
            {b.sequences.length > 0 && (
              <Row name="Animation" note={anim.error ? (anim.error as Error).message : `${b.sequences.length} in this model`} stacked>
                  <select value={seq} onChange={(e) => setSeq(Number(e.target.value))} aria-label="Animation">
                    {b.sequences.map((s, i) => <option key={i} value={i}>{s.name || `#${i}`}{s.frames > 1 ? ` (${s.frames})` : ""}</option>)}
                  </select>
                  <button className="btn" onClick={() => setPlaying((v) => !v)} aria-pressed={playing}>{playing ? "Pause" : "Play"}</button>
              </Row>
            )}
            {skins > 1 && (
              <Row name="Skin" note={`${skins} looks`}>
                <select value={skin} onChange={(e) => setSkin(Number(e.target.value))} aria-label="Skin">
                  {Array.from({ length: skins }, (_, i) => <option key={i} value={i}>Skin {i + 1}</option>)}
                </select>
              </Row>
            )}
            {b.bodyparts.map((p, pi) => p.models.length > 1 && (
              <Row key={pi} name={p.name || `Part ${pi + 1}`}>
                <select value={body[pi] ?? 0} onChange={(e) => setBody((old) => old.map((v, k) => (k === pi ? Number(e.target.value) : v)))} aria-label={p.name || `Part ${pi + 1}`}>
                  {p.models.map((m, mi) => <option key={mi} value={mi}>{m.triangles ? m.name || `Choice ${mi + 1}` : `${m.name || "Nothing"} (empty)`}</option>)}
                </select>
              </Row>
            ))}
            <Row name="Wireframe" note="The model's triangles">
              <Switch on={wire} onChange={setWire} label="Wireframe" />
            </Row>
          </Rows>
        )}
        <dl className="assets-facts">
          <dt>Triangles</dt><dd>{b ? Math.round(triangles).toLocaleString() : "…"}</dd>
          <dt>Bones</dt><dd>{b ? b.bones.length : "…"}</dd>
          <dt>File</dt><dd>{(item.size / 1024).toFixed(0)} KB</dd>
          {info.data && info.data.maps.length > 0 && (<>
            <dt>Placed in</dt>
            <dd>
              {info.data.maps.slice(0, 12).map((m, i) => (
                <span key={m}>{i > 0 && ", "}<button className="link" onClick={() => useNav.getState().go("tools", `map:${m}`)}>{m}</button></span>
              ))}
              {info.data.maps.length > 12 && ` and ${info.data.maps.length - 12} more`}
            </dd>
          </>)}
        </dl>
        {b && b.textures.length > 0 && (
          <>
            <h4>Textures</h4>
            <ul className="assets-model-tex">
              {b.textures.map((t, i) => (
                <li key={i}>
                  <img src={modelTextureUrl(item, i)} alt="" loading="lazy" />
                  <span className="assets-tile-name" title={t.name}>{t.name}</span>
                  <span className="muted">{t.width} × {t.height}</span>
                  <BusyButton className="btn small" busy={busy === `tex${i}`} label="Save…" busyLabel="Saving…" onClick={() => saveTexture(i)} />
                </li>
              ))}
            </ul>
          </>
        )}
        {item.imported && (
          <div className="row">
            <BusyButton className="btn danger" busy={busy === "remove"} label="Remove this model" busyLabel="Removing…" onClick={remove} />
          </div>
        )}
      </div>
    </div>
  );
}
