// The radar (overview) maker: a top-down picture of a map in the format CS 1.6 and Condition
// Zero use for the spectator overview, drawn from the map itself. Pick a map, choose a look,
// install. The game shows it to spectators (overview mode and the small map in the corner)
// and to anyone watching an HLTV demo; the round radar players see while alive is a plain
// circle and doesn't use it.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useToolArg } from "../../../stores/tools";
import { useReportAddress } from "../common/address";
import { Banner, BusyButton, LoadError, Segmented, Switch, useConfirm, useToast } from "../../../ui";
import { isAbort } from "../../../api";
import { errorText } from "../common/api";
import { Check, Loading, MapList, Row, Rows } from "../common";
import type { ToolProps } from "../registry";
import { currentUrl, DEFAULT_CHOICE, installOverview, overviewInfo, previewUrl, restoreOverview, type Choice, type Format, type MarkKind, type OverviewInfo } from "./overviewApi";
import "./overview.css";

export default function OverviewMaker({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useReportAddress("radar", map ? `radar:${map}` : null);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  if (!map)
    return (
      <div className="tl-page stack-tight">
        <p className="muted measure">
          Pick a map to make its overview: the top-down picture spectators see in overview mode, and anyone watching an HLTV demo of it. It's drawn from the map itself, lit the way the game lights it.
        </p>
        <MapList maps={maps} onPick={setMap} placeholder="Pick a map" />
      </div>
    );
  return <Maker key={map} map={map} onBack={() => setMap(null)} />;
}

const MARKS: [MarkKind, string][] = [
  ["bomb", "Bomb sites"],
  ["rescue", "Hostage rescue"],
  ["vip", "VIP escape"],
  ["buy", "Buy zones"],
  ["spawns", "Spawns"],
  ["hostages", "Hostages"],
];

/** The preview picture for the current choices (redrawn a moment after they stop changing). */
function usePicture(map: string, choice: Choice, which: "ours" | "installed") {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [drawing, setDrawing] = useState(true);
  const key = which === "ours" ? JSON.stringify(choice) : "installed";
  useEffect(() => {
    const ctl = new AbortController();
    setDrawing(true);
    const t = setTimeout(async () => {
      try {
        const u = which === "ours" ? await previewUrl(map, choice, ctl.signal) : await currentUrl(map, ctl.signal);
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
    }, 180);
    return () => {
      clearTimeout(t);
      ctl.abort();
    };
  }, [map, key]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => setUrl((old) => (old && URL.revokeObjectURL(old), null)), []);
  return { url, error, drawing };
}

function Maker({ map, onBack }: { map: string; onBack: () => void }) {
  const toast = useToast();
  const confirm = useConfirm();
  const qc = useQueryClient();
  const info = useQuery({ queryKey: ["tools", "overview", map], queryFn: ({ signal }) => overviewInfo(map, signal), retry: false });
  const [choice, setChoice] = useState<Choice>(DEFAULT_CHOICE);
  const [format, setFormat] = useState<Format>("bmp");
  const [res, setRes] = useState(true);
  const [show, setShow] = useState<"ours" | "installed">("ours");
  const [busy, setBusy] = useState<"install" | "restore" | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const set = (c: Partial<Choice>) => {
    setChoice((old) => ({ ...old, ...c }));
    setDone(null);
  };

  // without a nav, roofs can only be left out by height
  const i: OverviewInfo | undefined = info.data;
  useEffect(() => {
    if (i && !i.hasNav && choice.roofs === "nav") set({ roofs: "cut" });
  }, [i?.hasNav]); // eslint-disable-line react-hooks/exhaustive-deps

  const pic = usePicture(map, choice, show);

  const head = (
    <div className="tl-head">
      <div className="tl-head-title">
        <h3>{map}</h3>
        {i?.current && <span className="muted">{i.current.ours ? "Has an overview made here" : "Has an overview"}</span>}
      </div>
      <div className="tl-head-actions">
        <button className="btn" onClick={onBack}>Pick another map</button>
      </div>
    </div>
  );

  if (info.isPending)
    return (
      <div className="tl-page stack-tight">
        {head}
        <div className="panel"><Loading text={`Reading ${map}…`} /></div>
      </div>
    );
  if (info.isError || !i)
    return (
      <div className="tl-page stack-tight">
        {head}
        <LoadError text={errorText(info.error, "Couldn't read that map.")} onRetry={() => info.refetch()} />
      </div>
    );

  const install = async () => {
    setBusy("install");
    const r = await installOverview(map, choice, format, res && !i.stock);
    setBusy(null);
    if (r.error || !r.ok) return toast(r.error || "Couldn't install the overview.", true);
    setDone(r.message || "Installed.");
    toast(r.message || "Installed.");
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
    setShow("installed");
    qc.invalidateQueries({ queryKey: ["tools", "overview", map] });
  };

  const present = MARKS.filter(([k]) => (i.marks[k] ?? 0) > 0);
  const hasCurrent = !!i.current?.imageFound;
  const cut = choice.cut ?? i.heights.cut;

  return (
    <div className="tl-page stack-tight">
      {head}
      {i.inUse && <Banner>The game has {map} loaded right now. It reads the overview when a map loads, so a new one shows after the next map change.</Banner>}
      <div className="ov">
        <div className="panel ov-stage">
          {hasCurrent && (
            <div className="row between ov-compare">
              <Segmented label="Show" values="ours installed" labels="New|Installed now" current={show} onPick={(v) => setShow(v as "ours" | "installed")} />
              {show === "installed" && <span className="muted small-note">{i.current?.file}</span>}
            </div>
          )}
          <div className="ov-picture" aria-busy={pic.drawing}>
            {pic.url && !pic.error && <img src={pic.url} width={1024} height={768} alt={show === "ours" ? `The new overview of ${map}` : `The overview ${map} has now`} />}
            {pic.error && <p className="ov-msg">{pic.error}</p>}
            {!pic.url && !pic.error && <p className="ov-msg"><span className="spinner" aria-hidden="true" /> Drawing {map}…</p>}
            {pic.drawing && pic.url && <span className="spinner ov-busy" aria-label="Drawing" />}
          </div>
          <p className="muted small-note ov-note">
            1024×768, the size the game reads. {show === "ours" ? `Zoom ${i.frame.zoom.toFixed(2)} for this framing.` : ""}
          </p>
        </div>

        <div className="panel ov-options">
          <h3>Look</h3>
          <Rows>
            <Row name="Style" info="Like the game: the map's own textures, lit as the game lights them (what the game's own overviews look like). Clean: colours by height with the map's light, easy to read. Simple: colours by height only.">
              <Segmented label="Style" values="game clean simple" labels="Like the game|Clean|Simple" current={choice.style} onPick={(v) => set({ style: v as Choice["style"] })} />
            </Row>
            <Row
              name="Leave out roofs"
              note={choice.roofs === "nav" ? "Shows the floors the bots walk on and what's right next to them" : `Nothing above ${Math.round(cut)} units is drawn`}
              info="A roof seen from above hides the rooms under it. With the bots' nav, roofs over places players walk are left out. Without one, pick a height: anything higher isn't drawn."
            >
              <Segmented
                label="Leave out roofs"
                values={i.hasNav ? "nav cut" : "cut"}
                labels={i.hasNav ? "Using the bots' nav|Above a height" : "Above a height"}
                current={choice.roofs}
                onPick={(v) => set({ roofs: v as Choice["roofs"] })}
              />
            </Row>
            {choice.roofs === "cut" && (
              <Row name="Height" note={`${Math.round(i.heights.low)} to ${Math.round(i.heights.high + 512)}`}>
                <input
                  className="ov-range"
                  type="range"
                  aria-label="Height above which nothing is drawn"
                  min={Math.floor(i.heights.low)}
                  max={Math.ceil(i.heights.high + 512)}
                  step={8}
                  value={cut}
                  onChange={(e) => set({ cut: Number(e.target.value) })}
                />
                <span className="ov-num">{Math.round(cut)}</span>
              </Row>
            )}
            {present.length > 0 && (
              <Row name="Mark" stacked>
                <div className="tl-checks">
                  {present.map(([k, label]) => (
                    <Check key={k} label={label} checked={choice.marks.includes(k)} onChange={(on) => set({ marks: on ? [...choice.marks, k] : choice.marks.filter((x) => x !== k) })} />
                  ))}
                </div>
              </Row>
            )}
            <Row name="Outline walls" note="A dark line where the floor drops away">
              <Switch on={choice.outline} onChange={(v) => set({ outline: v })} label="Outline walls" />
            </Row>
            <Row name="Around the map" info="The game's own overviews are bright green around the map. Black is easier on the eyes.">
              <Segmented label="Around the map" values="green black" labels="Green, like the game's|Black" current={choice.background} onPick={(v) => set({ background: v as Choice["background"] })} />
            </Row>
            <Row name="Turn" info="Auto turns the picture so the map fills it best. The game shows it the same way round either way.">
              <Segmented label="Turn" values="auto north turned" labels="Auto|North up|Sideways" current={choice.orient} onPick={(v) => set({ orient: v as Choice["orient"] })} />
            </Row>
          </Rows>

          <h3 className="ov-sub">Install</h3>
          <Rows>
            <Row name="File" info="256 colours (.bmp) is what the game's classic overviews use and is a small download for players. Full colour (.tga) looks better; the game reads both.">
              <Segmented label="File" values="bmp tga" labels="256 colours (.bmp)|Full colour (.tga)" current={format} onPick={(v) => setFormat(v as Format)} />
            </Row>
            {!i.stock && (
              <Row name="Send it to players" note={`Adds it to ${map}.res, so players joining your server download it (if they don't have one for this map already)`}>
                <Switch on={res} onChange={setRes} label="Send it to players" />
              </Row>
            )}
          </Rows>
          <div className="row ov-actions">
            <BusyButton className="btn primary" busy={busy === "install"} disabled={busy !== null || !!pic.error} label={i.current ? "Install, replacing the old one" : "Install overview"} busyLabel="Installing…" onClick={install} />
            {i.backups && <BusyButton className="btn" busy={busy === "restore"} disabled={busy !== null} label="Put the old one back" busyLabel="Putting it back…" onClick={restore} />}
          </div>
          {i.current && !i.current.ours && <p className="muted small-note">The old files are kept next to the new ones (.bak), so you can put them back.</p>}
          {done && <p className="ov-done" role="status">{done}</p>}
        </div>
      </div>

      <div className="panel ov-who">
        <h3>Who sees it</h3>
        <ul>
          <li>Spectators, in overview mode and the small map in the corner (the spectator menu, or the overview key).</li>
          <li>Anyone watching an HLTV demo of the map: HLTV demos play back as a spectator.</li>
          <li>Not the radar players see while they're alive: in CS 1.6 and Condition Zero that's a plain circle with dots.</li>
          <li>Players joining your server get it with the map only if they don't have an overview for it yet.</li>
        </ul>
      </div>
    </div>
  );
}
