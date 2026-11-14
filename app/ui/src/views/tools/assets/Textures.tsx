// The texture browser: pick a WAD (or "Inside maps"), search by name, see a texture large
// with its size and the maps that use it, save it as a picture, or make your own.

import { useQueries, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { getJson } from "../../../api";
import { useNav } from "../../../store";
import { BusyButton, Info, LoadError, Segmented, Skeleton, useToast } from "../../../ui";
import { Row, Rows } from "../common";
import { exportPng, mapTextureUrl, pickPicture, previewTexture, saveTexture, useUsage, useWads, useWadTextures, wadTextureUrl, type ExportSource, type WadInfo, type WadTexture } from "./assetsApi";
import { matches, searchWords } from "../common/search";

/** "all", "maps" (textures inside maps) or "folder/name.wad" */
type Source = string;
type Item = { name: string; width: number; height: number; url: string; where: string; source: ExportSource; kind?: string };

const PAGE = 240;
const wadKey = (w: WadInfo) => `${w.folder}/${w.name}`;

export function Textures() {
  const wads = useWads();
  const usage = useUsage();
  const [source, setSource] = useState<Source>("");
  // the texture file was picked by hand (not the one the page started on)
  const [chosen, setChosen] = useState(false);
  const [q, setQ] = useState("");
  const [shown, setShown] = useState(PAGE);
  const [picked, setPicked] = useState<Item | null>(null);
  const [making, setMaking] = useState(false);

  const list = wads.data?.wads ?? [];
  // start on the game's main WAD (cstrike.wad / czero.wad), else the first
  useEffect(() => {
    if (source || !list.length) return;
    const main = list.find((w) => w.name.toLowerCase() === `${wads.data?.game}.wad`) ?? list.find((w) => !w.error);
    setSource(main ? wadKey(main) : "maps");
  }, [list, source, wads.data?.game]);

  const needle = q.trim().toLowerCase();
  // a search covers every texture file, unless one was picked by hand (QA4-T2 UX-M3)
  const scope: Source = needle && !chosen && source !== "maps" ? "all" : source;
  const one = scope !== "all" && scope !== "maps" ? list.find((w) => wadKey(w) === scope) ?? null : null;
  const single = useWadTextures(one);
  // "every WAD": each listing on its own (cached by the app after the first time)
  const readable = list.filter((w) => !w.error);
  const all = useQueries({
    queries: scope === "all" ? readable.map((w) => ({
      queryKey: ["tools", "assets", "wad", w.folder, w.name, w.size],
      queryFn: () => getJson<{ textures: WadTexture[] }>(`/api/tools/assets/wad/${encodeURIComponent(w.folder)}/${encodeURIComponent(w.name)}`).then((r) => r.textures ?? []),
    })) : [],
  });

  const items: Item[] = useMemo(() => {
    if (scope === "maps") {
      const u = usage.data;
      if (!u) return [];
      return u.embedded.map(([m, name, w, h]) => ({ name, width: w, height: h, url: mapTextureUrl(u.maps[m], name), where: `Inside ${u.maps[m]}`, source: { kind: "map", map: u.maps[m], tex: name } as ExportSource }));
    }
    const from = (w: WadInfo, ts: WadTexture[]) =>
      ts.map((t) => ({ name: t.name, width: t.width, height: t.height, url: wadTextureUrl(w.folder, w.name, t.name), where: `${w.folder}/${w.name}`, kind: t.kind, source: { kind: "wad", folder: w.folder, wad: w.name, tex: t.name } as ExportSource }));
    if (scope === "all") return readable.flatMap((w, i) => from(w, all[i]?.data ?? []));
    return one && single.data ? from(one, single.data) : [];
  }, [scope, usage.data, single.data, one, all, readable]);

  const found = needle ? items.filter((t) => matches(searchWords(needle), t.name)) : items;
  useEffect(() => setShown(PAGE), [source, needle]);

  const loading = scope === "maps" ? usage.isLoading : scope === "all" ? all.some((x) => x.isLoading) : single.isLoading;
  const error = (scope === "maps" ? usage.error : single.error) as Error | null;
  const unreadable = list.filter((w) => w.error);
  const inWords = scope === "all" ? "every texture file" : scope === "maps" ? "maps" : one?.name ?? "";

  if (wads.isLoading) return <Skeleton rows={6} />;
  if (wads.error) return <LoadError text={(wads.error as Error).message} onRetry={() => wads.refetch()} />;

  return (
    <div className="assets-split">
      <div className="panel assets-browse">
        <div className="row assets-filters">
          <input className="search assets-search" type="search" placeholder="Search textures by name" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search textures" />
          <select value={source} onChange={(e) => (setSource(e.target.value), setChosen(true))} aria-label="Where from">
            <option value="all">Every WAD ({readable.length})</option>
            <option value="maps">Inside maps</option>
            {list.map((w) => (
              <option key={wadKey(w)} value={wadKey(w)} disabled={!!w.error} title={w.error || undefined}>
                {w.name} ({w.folder}){w.custom ? " · yours" : ""}{w.error ? " · can't be read" : w.textures != null ? ` · ${w.textures}` : ""}
              </option>
            ))}
          </select>
          <button className="btn" onClick={() => setMaking((v) => !v)} aria-expanded={making}>Make your own…</button>
        </div>
        {one?.error && <p className="muted">{one.error}</p>}
        {unreadable.length > 0 && (
          <details className="assets-unreadable">
            <summary className="muted">
              {unreadable.length} texture file{unreadable.length === 1 ? "" : "s"} can't be read
            </summary>
            <ul>
              {unreadable.map((w) => (
                <li key={wadKey(w)}>
                  <strong>{w.name}</strong> <span className="muted">({w.folder}): {w.error}</span>
                </li>
              ))}
            </ul>
          </details>
        )}
        {making && <MakeTexture onDone={(wad) => { setMaking(false); if (wad) setSource(`cstrike/${wad}`); }} />}
        <p className="muted assets-count">
          {loading ? (
            <>
              <span className="spinner" aria-hidden="true" /> Reading textures…
            </>
          ) : (
            `${found.length} texture${found.length === 1 ? "" : "s"}${needle ? ` matching "${q.trim()}" in ${inWords}` : ""}`
          )}
          {scope === "maps" && " · textures that maps carry inside them"}
          {needle && chosen && scope !== "all" && scope !== "maps" && (
            <>
              {" · "}
              <button type="button" className="link" onClick={() => setChosen(false)}>
                Search every texture file
              </button>
            </>
          )}
        </p>
        {error ? <LoadError text={error.message} /> : loading && !items.length ? <Skeleton rows={4} /> : (
          <>
            <ul className="assets-grid" aria-label="Textures">
              {found.slice(0, shown).map((t) => (
                <li key={`${t.where}/${t.name}`}>
                  <button className="assets-tile" aria-pressed={picked?.url === t.url} onClick={() => setPicked(t)} title={`${t.name} · ${t.width}×${t.height} · ${t.where}`}>
                    <img src={t.url} alt="" loading="lazy" decoding="async" />
                    <span className="assets-tile-name">{t.name}</span>
                    {scope === "all" && needle && <span className="assets-tile-where muted">{t.where.split("/").pop()}</span>}
                  </button>
                </li>
              ))}
            </ul>
            {found.length > shown && (
              <div className="row assets-more">
                <button className="btn" onClick={() => setShown((n) => n + PAGE)}>Show more ({found.length - shown} left)</button>
              </div>
            )}
          </>
        )}
      </div>
      <TextureDetail item={picked} />
    </div>
  );
}

function TextureDetail({ item }: { item: Item | null }) {
  const usage = useUsage();
  const toast = useToast();
  const [saving, setSaving] = useState(false);
  const [allMaps, setAllMaps] = useState(false);
  useEffect(() => setAllMaps(false), [item?.url]);
  if (!item) {
    return (
      <div className="panel assets-detail">
        <h3>Pick a texture</h3>
        <p className="muted measure">Click any texture to see it large, how big it is, and which of your maps use it.</p>
      </div>
    );
  }
  const u = usage.data;
  const maps = u ? (u.textures[item.name.toLowerCase()] ?? []).map((i) => u.maps[i]).filter(Boolean) : [];
  const zoom = Math.max(1, Math.min(4, Math.floor(320 / Math.max(item.width, item.height, 1))));
  const flags = describe(item.name);
  const save = async () => {
    setSaving(true);
    const r = await exportPng(item.source);
    setSaving(false);
    if (r.cancelled) return;
    if (r.error || !r.ok) return toast(r.error || "Couldn't save the picture.", true);
    toast(r.message || "Saved.");
  };
  return (
    <div className="panel assets-detail">
      <h3 className="assets-detail-name">{item.name}</h3>
      <div className="assets-big">
        <img src={item.url} alt={`The texture ${item.name}`} width={item.width * zoom} height={item.height * zoom} />
      </div>
      <dl className="assets-facts">
        <dt>Size</dt>
        <dd>{item.width} × {item.height}{zoom > 1 ? <span className="muted"> (shown {zoom}× bigger)</span> : null}</dd>
        <dt>From</dt>
        <dd>{item.where}{item.kind === "decal" ? <span className="muted"> · a decal</span> : null}</dd>
        {flags && (<><dt>Special</dt><dd>{flags}</dd></>)}
      </dl>
      <div className="row">
        <BusyButton className="btn" busy={saving} label="Save as picture…" busyLabel="Saving…" onClick={save} />
      </div>
      <h4>
        Used by {usage.isLoading ? "…" : `${maps.length} map${maps.length === 1 ? "" : "s"}`}{" "}
        <Info text="Maps whose textures include one with this name. A map takes each texture from the first of its WADs (or itself) that has it." />
      </h4>
      {usage.isLoading ? <p className="muted">Looking through your maps… <span className="spinner" aria-hidden /></p> : maps.length === 0 ? (
        <p className="muted">None of your maps use it.</p>
      ) : (
        <ul className="assets-maps">
          {(allMaps ? maps : maps.slice(0, 24)).map((m) => (
            <li key={m}>
              <button className="link" onClick={() => useNav.getState().go("tools", `map:${m}`)} title="Open in the map viewer">{m}</button>
            </li>
          ))}
          {!allMaps && maps.length > 24 && <li><button className="link" onClick={() => setAllMaps(true)}>and {maps.length - 24} more</button></li>}
        </ul>
      )}
    </div>
  );
}

/** What a texture name's first character means to the game. */
function describe(name: string): string | null {
  if (name.startsWith("{")) return "See-through where it's blue";
  if (name.startsWith("!") || name.startsWith("*")) return "Water";
  if (/^\+[0-9a-j]/i.test(name)) return "Animated (one frame of a set)";
  if (/^-[0-9]/.test(name)) return "Random tiling";
  if (/^sky$/i.test(name)) return "Sky";
  return null;
}

const SIZES = ["64", "128", "256", "512"];

/** Your own picture as a texture in a WAD of yours. */
function MakeTexture({ onDone }: { onDone: (wad: string | null) => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [picture, setPicture] = useState<{ name: string; width: number; height: number } | null>(null);
  const [name, setName] = useState("");
  const [size, setSize] = useState("256");
  const [wad, setWad] = useState("my_textures");
  const [preview, setPreview] = useState<{ png: string; width: number; height: number } | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState<"pick" | "save" | null>(null);
  const [previewing, setPreviewing] = useState(false);

  useEffect(() => {
    if (!picture || !name) return;
    let live = true;
    setPreviewing(true);
    const t = setTimeout(async () => {
      const r = await previewTexture({ name, size: Number(size), wad });
      if (!live) return;
      setPreviewing(false);
      if (r.error || !r.png) {
        setProblem(r.error || "Couldn't make the texture.");
        setPreview(null);
      } else {
        setProblem(null);
        setPreview({ png: r.png, width: r.width ?? 0, height: r.height ?? 0 });
      }
    }, 200);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [picture, name, size]); // eslint-disable-line react-hooks/exhaustive-deps

  const pick = async () => {
    setBusy("pick");
    const r = await pickPicture();
    setBusy(null);
    if (r.cancelled) return;
    if (r.error || !r.name) return toast(r.error || "Couldn't open that picture.", true);
    setPicture({ name: r.name, width: r.width ?? 0, height: r.height ?? 0 });
    if (r.suggested) setName(r.suggested);
  };
  const save = async () => {
    setBusy("save");
    const r = await saveTexture({ name, size: Number(size), wad });
    setBusy(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't save the texture.", true);
    toast(r.message || "Texture saved.");
    await qc.invalidateQueries({ queryKey: ["tools", "assets", "wads"] });
    await qc.invalidateQueries({ queryKey: ["tools", "assets", "wad"] });
    onDone(r.wad ?? null);
  };

  return (
    <div className="assets-make">
      <h4>Make your own texture</h4>
      <p className="muted measure">
        A picture becomes a texture in a WAD of yours in the cstrike folder, ready for maps you make. Start the name with {"{"} to make its clear parts see-through.
      </p>
      <div className="row assets-make-body">
        <div className="assets-make-preview">
          {preview ? <img src={preview.png} alt="Your texture as the game draws it" /> : <span className="muted">{picture ? (problem ?? "Making a preview…") : "No picture yet"}</span>}
          {previewing && preview && <span className="spinner assets-make-busy" aria-label="Updating the preview" />}
        </div>
        <div className="assets-make-form">
          <Rows>
            <Row name="Picture" note={picture ? `${picture.name} · ${picture.width} × ${picture.height}` : undefined}>
              <BusyButton className="btn" busy={busy === "pick"} label={picture ? "Choose another…" : "Choose a picture…"} busyLabel="Opening…" onClick={pick} />
            </Row>
            <Row name="Texture name" note="Up to 15 letters, digits, _ or -">
              <input className="search" aria-label="Texture name" value={name} maxLength={15} onChange={(e) => setName(e.target.value.toLowerCase())} spellCheck={false} />
            </Row>
            <Row name="Biggest side" note={preview ? `Comes out ${preview.width} × ${preview.height}` : undefined}>
              <Segmented values={SIZES.join(" ")} labels={SIZES.join("|")} current={size} onPick={setSize} label="Biggest side" />
            </Row>
            <Row name="Save in" note="A WAD you made here, or a new one">
              <span className="row nowrap"><input className="search" aria-label="WAD to save in" value={wad} maxLength={40} onChange={(e) => setWad(e.target.value)} spellCheck={false} /><span className="muted">.wad</span></span>
            </Row>
          </Rows>
          <div className="row assets-make-actions">
            <BusyButton className="btn primary" busy={busy === "save"} disabled={!preview || !wad.trim()} label="Save texture" busyLabel="Saving…" onClick={save} />
            <button className="btn" onClick={() => onDone(null)}>Close</button>
          </div>
        </div>
      </div>
    </div>
  );
}
