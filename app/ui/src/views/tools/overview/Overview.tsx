// The radar (overview) maker: a top-down picture of a map in the format CS 1.6 and Condition
// Zero use for the spectator overview, drawn from the map itself. Pick a map, choose a look,
// install. The game shows it to spectators (overview mode and the small map in the corner)
// and to anyone watching an HLTV demo; the round radar players see while alive is a plain
// circle and doesn't use it.
//
// Layout: the picture large on the left (new, side by side with the installed one, or the
// installed one alone; scroll zooms, drag moves), the steps on the right (look, what's drawn,
// install), Install in the bar at the top.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useToolArg } from "../../../stores/tools";
import { useReportAddress } from "../common/address";
import { Banner, BusyButton, Info, LoadError, Segmented, Switch, useConfirm, useToast } from "../../../ui";
import { isAbort } from "../../../api";
import { errorText } from "../common/api";
import { Loading } from "../common";
import { MapBrowser, rememberMap } from "../common/MapBrowser";
import type { ToolProps } from "../registry";
import { currentUrl, DEFAULT_CHOICE, installOverview, overviewInfo, previewUrl, restoreOverview, type Choice, type Format, type MarkKind, type OverviewInfo, type Style } from "./overviewApi";
import { FIT, PanZoom, zoomBy, type View } from "./PanZoom";
import "./overview.css";

export default function OverviewMaker({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useReportAddress("radar", map ? `radar:${map}` : null);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  useEffect(() => {
    if (map) rememberMap("radar", map);
  }, [map]);
  if (!map)
    return (
      <MapBrowser
        tool="radar"
        maps={maps}
        onPick={setMap}
        title="Pick a map to make its overview"
        note={<>The top-down picture spectators and HLTV viewers see, drawn from the map. <Info text={WHO} /></>}
        action={(m) => (m.thumb === "radar" ? "Remake overview" : "Make overview")}
        badges={(m) => (m.thumb === "radar" ? [{ text: "Has an overview", tone: "ok" }] : [])}
      />
    );
  return <Maker key={map} map={map} onBack={() => setMap(null)} />;
}

const WHO = (
  <>
    <strong>Who sees it:</strong> spectators (overview mode and the small map in the corner) and anyone watching an HLTV demo of the map. Not the round radar players see while alive: in CS 1.6 and Condition Zero that's a plain circle. Players joining your server get it with the map only if they have no overview for it yet.
  </>
);

const MARKS: [MarkKind, string][] = [
  ["bomb", "Bomb sites"],
  ["rescue", "Hostage rescue"],
  ["vip", "VIP escape"],
  ["buy", "Buy zones"],
  ["spawns", "Spawns"],
  ["hostages", "Hostages"],
];

const STYLES: [Style, string, string][] = [
  ["game", "Game", "Like the game: the map's own textures, lit as the game lights them"],
  ["clean", "Clean", "Clean: colours by height, with the map's light: easy to read"],
  ["simple", "Simple", "Simple: colours by height only"],
];

type Show = "ours" | "split" | "installed";

/** A picture from the app as an object URL, redrawn a moment after `key` stops changing. */
function usePicture(key: string | null, load: (signal: AbortSignal) => Promise<string>, wait = 180) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [drawing, setDrawing] = useState(true);
  useEffect(() => {
    if (key === null) return;
    const ctl = new AbortController();
    setDrawing(true);
    const t = setTimeout(async () => {
      try {
        const u = await load(ctl.signal);
        setUrl((old) => {
          if (old) URL.revokeObjectURL(old);
          return u;
        });
        setError(null);
      } catch (e) {
        if (isAbort(e)) return;
        setError(errorText(e, "Couldn't draw the map."));
      }
      setDrawing(false);
    }, wait);
    return () => {
      clearTimeout(t);
      ctl.abort();
    };
  }, [key]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => setUrl((old) => (old && URL.revokeObjectURL(old), null)), []);
  return { url, error, drawing };
}

function Picture({ pic, alt, view, onView, tag }: { pic: ReturnType<typeof usePicture>; alt: string; view: View; onView: (v: View) => void; tag?: string }) {
  return (
    <div className="ov-pane" aria-busy={pic.drawing && !pic.url}>
      {pic.url && !pic.error && (
        <PanZoom view={view} onView={onView} label={alt}>
          <img src={pic.url} width={1024} height={768} alt="" draggable={false} />
        </PanZoom>
      )}
      {pic.error && <p className="ov-msg">{pic.error}</p>}
      {!pic.url && !pic.error && <Loading className="ov-msg" text="Drawing…" />}
      {tag && <span className="ov-tag">{tag}</span>}
      {pic.drawing && pic.url && <span className="spinner ov-busy" aria-label="Drawing" />}
    </div>
  );
}

function Maker({ map, onBack }: { map: string; onBack: () => void }) {
  const toast = useToast();
  const confirm = useConfirm();
  const qc = useQueryClient();
  const info = useQuery({ queryKey: ["tools", "overview", map], queryFn: ({ signal }) => overviewInfo(map, signal), retry: false });
  const [choice, setChoice] = useState<Choice>(DEFAULT_CHOICE);
  const [format, setFormat] = useState<Format>("bmp");
  const [res, setRes] = useState(true);
  const [show, setShow] = useState<Show>("ours");
  const [view, setView] = useState<View>(FIT);
  const [busy, setBusy] = useState<"install" | "restore" | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [n, setN] = useState(0);
  const set = (c: Partial<Choice>) => {
    setChoice((old) => ({ ...old, ...c }));
    setDone(null);
  };

  // without a nav, roofs can only be left out by height
  const i: OverviewInfo | undefined = info.data;
  useEffect(() => {
    if (i && !i.hasNav && choice.roofs === "nav") set({ roofs: "cut" });
  }, [i?.hasNav]); // eslint-disable-line react-hooks/exhaustive-deps

  const hasCurrent = !!i?.current?.imageFound;
  const ours = usePicture(i ? JSON.stringify(choice) : null, (s) => previewUrl(map, choice, s));
  const installed = usePicture(hasCurrent && show !== "ours" ? `installed${n}` : null, (s) => currentUrl(map, s), 0);
  // the three looks, drawn once with the map's first choices (after the big picture)
  const thumbs = {
    game: usePicture(i && ours.url ? `game-${choice.background}` : null, (s) => previewUrl(map, { ...DEFAULT_CHOICE, roofs: choice.roofs, background: choice.background, style: "game" }, s), 300),
    clean: usePicture(i && ours.url ? `clean-${choice.background}` : null, (s) => previewUrl(map, { ...DEFAULT_CHOICE, roofs: choice.roofs, background: choice.background, style: "clean" }, s), 900),
    simple: usePicture(i && ours.url ? `simple-${choice.background}` : null, (s) => previewUrl(map, { ...DEFAULT_CHOICE, roofs: choice.roofs, background: choice.background, style: "simple" }, s), 1500),
  };

  // keys: 1/2/3 new, side by side, installed; + - 0 zoom
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (e.metaKey || e.ctrlKey || e.altKey || ["INPUT", "SELECT", "TEXTAREA"].includes(t.tagName) || document.querySelector(".sheet-backdrop")) return;
      if (e.key === "1") setShow("ours");
      else if (e.key === "2" && hasCurrent) setShow("split");
      else if (e.key === "3" && hasCurrent) setShow("installed");
      else if (e.key === "+" || e.key === "=") setView((v) => zoomCentre(v, 1.4));
      else if (e.key === "-") setView((v) => zoomCentre(v, 1 / 1.4));
      else if (e.key === "0") setView(FIT);
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, [hasCurrent]);

  const title = (
    <>
      <span className="map-title">{map}</span>
      {i && <span className={`tag ${hasCurrent ? "" : "ov-none"}`}>{!i.current ? "No overview yet" : i.current.ours ? "Overview made here" : hasCurrent ? "Has an overview" : "Overview file missing"}</span>}
      <button className="btn small" onClick={onBack}>Other map</button>
    </>
  );

  if (info.isPending)
    return (
      <div className="tools-page">
        <div className="tools-bar">{title}</div>
        <div className="tools-stage ov-empty"><Loading text={`Reading ${map}…`} /></div>
      </div>
    );
  if (info.isError || !i)
    return (
      <div className="tools-page">
        <div className="tools-bar">{title}</div>
        <LoadError title={`Couldn't read ${map}`} text={errorText(info.error, "Couldn't read that map.")} onRetry={() => info.refetch()} />
      </div>
    );

  const install = async () => {
    setBusy("install");
    const r = await installOverview(map, choice, format, res && !i.stock);
    setBusy(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't install the overview.", true);
    setDone(r.message || "Installed.");
    setN((x) => x + 1);
    qc.invalidateQueries({ queryKey: ["tools", "overview", map] });
  };

  const restore = async () => {
    if (!(await confirm(`Put back the overview ${map} had before? The one made here is replaced.`, { ok: "Put it back" }))) return;
    setBusy("restore");
    const r = await restoreOverview(map);
    setBusy(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't put the old overview back.", true);
    toast(r.message || "Put the old overview back.");
    setDone(null);
    setN((x) => x + 1);
    setShow("installed");
    qc.invalidateQueries({ queryKey: ["tools", "overview", map] });
  };

  const present = MARKS.filter(([k]) => (i.marks[k] ?? 0) > 0);
  const cut = choice.cut ?? i.heights.cut;
  const mode: Show = hasCurrent ? show : "ours";

  return (
    <div className="tools-page ov-page">
      <div className="tools-bar">
        {title}
        <span className="push" />
        {hasCurrent && <Segmented label="Show" values="ours split installed" labels="New|Side by side|Installed now" current={mode} onPick={(v) => setShow(v as Show)} />}
        <BusyButton className="btn primary" busy={busy === "install"} disabled={busy !== null || !!ours.error} label={i.current ? "Install, replacing the old one" : "Install overview"} busyLabel="Installing…" onClick={install} />
      </div>
      {i.inUse && <Banner>The game has {map} loaded. It reads the overview when a map loads, so a new one shows after the next map change.</Banner>}

      <div className="tools-viewer">
        <div className="tools-stage">
          <div className={`tools-canvas ov-stage ov-${mode}`} style={{ background: choice.background === "green" ? "#0c120c" : "#0b0d0b" }}>
            <div className="ov-panes">
            {mode !== "installed" && <Picture pic={ours} alt={`The new overview of ${map}`} view={view} onView={setView} tag={mode === "split" ? "New" : undefined} />}
            {mode !== "ours" && <Picture pic={installed} alt={`The overview ${map} has now`} view={view} onView={setView} tag={mode === "split" ? "Installed now" : undefined} />}
            </div>
            <div className="ov-zoom" role="group" aria-label="Zoom">
              <button className="btn small" aria-label="Zoom in" title="Zoom in (+)" onClick={() => setView((v) => zoomCentre(v, 1.4))}>+</button>
              <button className="btn small" aria-label="Zoom out" title="Zoom out (−)" disabled={view.s <= 1} onClick={() => setView((v) => zoomCentre(v, 1 / 1.4))}>−</button>
              <button className="btn small" title="Fit (0, or double-click)" disabled={view.s === 1} onClick={() => setView(FIT)}>Fit</button>
            </div>
          </div>
          <div className="tools-status">
            <span className="mono">1024×768</span>
            <span className="tools-hover">{mode === "installed" ? i.current?.file : `zoom ${i.frame.zoom.toFixed(2)} · ${i.frame.rotated ? "north up" : "sideways"}`}{view.s > 1 ? ` · ${Math.round(view.s * 100)}%` : ""}</span>
            <span className="tools-hint">Scroll to zoom · drag to move · double-click fits</span>
          </div>
        </div>

        <aside className="tools-side">
          <section className="tools-panel">
            <h3 className="ov-step"><span>1</span> Look <Info text={WHO} /></h3>
            <div className="ov-styles" role="group" aria-label="Style">
              {STYLES.map(([s, label, help]) => (
                <button key={s} className="ov-style" aria-pressed={choice.style === s} title={help} onClick={() => set({ style: s })}>
                  <span className="ov-thumb">{thumbs[s].url ? <img src={thumbs[s].url!} alt="" /> : <span className="spinner" aria-hidden="true" />}</span>
                  <span className="ov-style-name">{label}</span>
                </button>
              ))}
            </div>
            <div className="tools-field">
              <span>Around the map <Info text="The game's own overviews are bright green around the map. Black is easier on the eyes." /></span>
              <div className="ov-swatches" role="group" aria-label="Around the map">
                <button className={`ov-swatch green`} aria-pressed={choice.background === "green"} title="Green, like the game's" aria-label="Green, like the game's" onClick={() => set({ background: "green" })} />
                <button className={`ov-swatch black`} aria-pressed={choice.background === "black"} title="Black" aria-label="Black" onClick={() => set({ background: "black" })} />
              </div>
            </div>
          </section>

          <section className="tools-panel">
            <h3 className="ov-step"><span>2</span> What's drawn</h3>
            {present.length > 0 && (
              <div className="chips" role="group" aria-label="Marks">
                {present.map(([k, label]) => (
                  <button key={k} className="chip" aria-pressed={choice.marks.includes(k)} onClick={() => set({ marks: choice.marks.includes(k) ? choice.marks.filter((x) => x !== k) : [...choice.marks, k] })}>
                    {label}
                  </button>
                ))}
              </div>
            )}
            <div className="tools-field">
              <span>Roofs <Info text="A roof seen from above hides the rooms under it. With the bots' nav, roofs over places players walk are left out. Without one, pick a height: anything higher isn't drawn." /></span>
              <Segmented label="Leave out roofs" values={i.hasNav ? "nav cut" : "cut"} labels={i.hasNav ? "By the nav|By height" : "By height"} current={choice.roofs} onPick={(v) => set({ roofs: v as Choice["roofs"] })} />
            </div>
            {choice.roofs === "cut" && (
              <label className="ov-height">
                <input type="range" aria-label="Height above which nothing is drawn" min={Math.floor(i.heights.low)} max={Math.ceil(i.heights.high + 512)} step={8} value={cut} onChange={(e) => set({ cut: Number(e.target.value) })} />
                <span className="ov-num">{Math.round(cut)}</span>
              </label>
            )}
            <div className="tools-field">
              <span>Outline walls</span>
              <Switch on={choice.outline} onChange={(v) => set({ outline: v })} label="Outline walls" />
            </div>
            <div className="tools-field">
              <span>Turn <Info text="Auto turns the picture so the map fills it best. The game shows it the right way round either way." /></span>
              <Segmented label="Turn" values="auto north turned" labels="Auto|North up|Sideways" current={choice.orient} onPick={(v) => set({ orient: v as Choice["orient"] })} />
            </div>
          </section>

          <section className="tools-panel">
            <h3 className="ov-step"><span>3</span> Install</h3>
            <div className="tools-field">
              <span>File <Info text="256 colours (.bmp) is what the game's own overviews use and is a small download for players. Full colour (.tga) looks better; the game reads both." /></span>
              <Segmented label="File" values="bmp tga" labels=".bmp|.tga" current={format} onPick={(v) => setFormat(v as Format)} />
            </div>
            {!i.stock && (
              <div className="tools-field">
                <span>Send to players <Info text={`Adds it to ${map}.res, so players joining your server download it (if they have no overview for this map yet).`} /></span>
                <Switch on={res} onChange={setRes} label="Send it to players" />
              </div>
            )}
            {done ? (
              <div className="ov-done" role="status">
                <strong>Installed</strong>
                <span>{done}</span>
                <span className="small-note muted">Spectate on {map} and press the overview key to see it.</span>
                <div className="row">
                  <button className="btn small" onClick={() => setShow("split")}>Compare with the old one</button>
                </div>
              </div>
            ) : (
              i.current && !i.current.ours && <p className="small-note muted">The old files are kept (.bak), so you can put them back.</p>
            )}
            {i.backups && <BusyButton className="btn small" busy={busy === "restore"} disabled={busy !== null} label="Put the old one back" busyLabel="Putting it back…" onClick={restore} />}
          </section>
        </aside>
      </div>
    </div>
  );
}

/** zoom about the middle of the picture's box (the keys and buttons) */
function zoomCentre(v: View, f: number): View {
  const el = document.querySelector(".ov-pz");
  const r = el?.getBoundingClientRect();
  return zoomBy(v, f, r ? r.width / 2 : 0, r ? r.height / 2 : 0);
}
