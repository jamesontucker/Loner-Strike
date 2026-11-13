// Crosshair & config builder: the player's own settings for Counter-Strike 1.6 and Condition
// Zero (crosshair, keys, mouse, connection, HUD), with the crosshair drawn live as the game
// draws it. Saved in userconfig.cfg, which the game runs after its own config.cfg every time it
// starts; the player's own lines there stay as they are, and the old file is kept first.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { Banner, BusyButton, Info, LoadError, Segmented, Switch, useConfirm, useToast } from "../../../ui";
import { Empty, Loading, Row, Rows } from "../common";
import { errorText, useBusy } from "../common/api";
import { ACTIONS, ALL_ACTIONS, COLOURS, commandWords, keyFromCode, keyLabel, MOUSE, NET, PRESETS, RESERVED } from "./catalog";
import { CrosshairPreview, type Motion } from "./Crosshair";
import { cfgStatus, previewCfg, removeCfg, restoreCfg, saveCfg, type Bind, type Choice, type GameCfg } from "./playercfgApi";
import "./playercfg.css";

const toHex = (c: string) =>
  "#" +
  c
    .split(/\s+/)
    .slice(0, 3)
    .map((n) => Math.max(0, Math.min(255, Number(n) || 0)).toString(16).padStart(2, "0"))
    .join("");
const fromHex = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) || 0).join(" ");

/** A number with a slider beside it. */
function NumberField({ value, onChange, min, max, step, label, unit }: { value: string; onChange: (v: string) => void; min: number; max: number; step: number; label: string; unit?: string }) {
  const n = Number(value);
  return (
    <span className="pc-num">
      <input type="range" min={min} max={max} step={step} value={Number.isFinite(n) ? Math.min(max, Math.max(min, n)) : min} aria-label={label} onChange={(e) => onChange(e.target.value)} />
      <input type="number" className="search pc-num-box" min={min} max={max} step={step} value={value} aria-label={`${label} (number)`} onChange={(e) => onChange(e.target.value)} />
      {unit && <span className="muted">{unit}</span>}
    </span>
  );
}

/** A key button: shows the key, or waits for one to be pressed (keyboard, mouse, wheel). */
function KeyPick({ value, onPick, onClear, label }: { value: string | undefined; onPick: (k: string) => void; onClear: () => void; label: string }) {
  const [waiting, setWaiting] = useState(false);
  const btn = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!waiting) return;
    const done = (k: string | null) => {
      setWaiting(false);
      if (k) onPick(k);
    };
    const key = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape") return done(null);
      done(keyFromCode(e.code));
    };
    const mouse = (e: MouseEvent) => {
      // the click that started waiting isn't a pick
      if (e.target === btn.current && e.button === 0) return;
      e.preventDefault();
      done(MOUSE[e.button] ?? null);
    };
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      done(e.deltaY < 0 ? "mwheelup" : "mwheeldown");
    };
    const menu = (e: MouseEvent) => e.preventDefault();
    window.addEventListener("keydown", key, true);
    window.addEventListener("mousedown", mouse, true);
    window.addEventListener("wheel", wheel, { capture: true, passive: false });
    window.addEventListener("contextmenu", menu, true);
    return () => {
      window.removeEventListener("keydown", key, true);
      window.removeEventListener("mousedown", mouse, true);
      window.removeEventListener("wheel", wheel, true);
      window.removeEventListener("contextmenu", menu, true);
    };
  }, [waiting, onPick]);
  return (
    <span className="pc-key">
      <button ref={btn} type="button" className={`btn small pc-key-btn${waiting ? " waiting" : ""}`} aria-label={`${label}: ${value ? keyLabel(value) : "no key"}. Press to choose a key`} onClick={() => setWaiting((w) => !w)}>
        {waiting ? "Press a key…" : value ? keyLabel(value) : "Choose a key"}
      </button>
      {value && !waiting && (
        <button type="button" className="btn small ghost" aria-label={`Take the key off ${label}`} title="Take this key off" onClick={onClear}>
          ×
        </button>
      )}
    </span>
  );
}

function same(a: Choice, b: Choice) {
  return JSON.stringify(a.settings) === JSON.stringify(b.settings) && JSON.stringify([...a.binds].sort((x, y) => x.action.localeCompare(y.action))) === JSON.stringify([...b.binds].sort((x, y) => x.action.localeCompare(y.action)));
}

function startChoice(g: GameCfg): Choice {
  return { settings: { ...g.current }, binds: g.ours.binds.map((b) => ({ ...b })) };
}

export default function PlayerConfig() {
  const toast = useToast();
  const ask = useConfirm();
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["tools", "playercfg"], queryFn: ({ signal }) => cfgStatus(signal) });
  const games = status.data?.games ?? [];
  const [gameId, setGameId] = useState<string | null>(null);
  const game = games.find((g) => g.game === gameId) ?? games[0];
  const [choice, setChoice] = useState<Choice | null>(null);
  const [start, setStart] = useState<Choice | null>(null);
  const [motion, setMotion] = useState<Motion>("still");
  const [res, setRes] = useState("1024");
  const [zoom, setZoom] = useState(2);
  const [lines, setLines] = useState<string[] | null>(null);
  const [lineError, setLineError] = useState<string | null>(null);
  const { working, run } = useBusy();

  // a fresh start whenever the game (or what's on disk) changes
  useEffect(() => {
    if (!game) return;
    const c = startChoice(game);
    setChoice(c);
    setStart(c);
  }, [game?.game, status.dataUpdatedAt]); // eslint-disable-line react-hooks/exhaustive-deps

  // the lines a save would write, a moment after a change
  useEffect(() => {
    if (!game || !choice) return;
    let live = true;
    const t = setTimeout(async () => {
      const r = await previewCfg(game.game, choice);
      if (!live) return;
      setLineError(r.error ?? null);
      setLines(r.error ? null : (r.lines ?? []));
    }, 250);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [game, choice]);

  const dirty = !!choice && !!start && !same(choice, start);

  // shooting in the preview: the crosshair opens, then settles
  useEffect(() => {
    if (motion !== "shoot") return;
    const t = setTimeout(() => setMotion("still"), 900);
    return () => clearTimeout(t);
  }, [motion]);

  if (status.isPending) return <Loading text="Reading your game's settings…" />;
  if (status.isError) return <LoadError text={errorText(status.error, "Couldn't read your settings.")} onRetry={() => status.refetch()} />;
  if (!game || !choice)
    return <Empty title="No game found" text="Couldn't find Counter-Strike 1.6 or Condition Zero on this computer. Set up the game first, then come back to make your config." />;

  const s = choice.settings;
  const set = (k: string, v: string) => setChoice((c) => (c ? { ...c, settings: { ...c.settings, [k]: v } } : c));
  const setMany = (vals: Record<string, string>) => setChoice((c) => (c ? { ...c, settings: { ...c.settings, ...vals } } : c));
  const on = (k: string) => s[k] === "1";
  const toggle = (k: string) => (v: boolean) => set(k, v ? "1" : "0");
  const keyOf = (action: string) => choice.binds.find((b) => b.action === action)?.key;
  const setKey = (action: string, key: string | null) =>
    setChoice((c) => {
      if (!c) return c;
      // one job per key: the key leaves whatever else had it
      const binds: Bind[] = c.binds.filter((b) => b.action !== action && b.key !== key);
      if (key) binds.push({ key, action });
      return { ...c, binds };
    });
  const presetOn = PRESETS.find((p) => Object.entries(p.values).every(([k, v]) => s[k] === v))?.id ?? "";
  const netOn = NET.find((p) => Object.entries(p.values).every(([k, v]) => Number(s[k]) === Number(v)))?.id ?? "";

  const after = (r: { error?: string; message?: string; status?: GameCfg }, fallback: string) => {
    if (r.error) return toast(r.error, true);
    toast(r.message || fallback);
    qc.invalidateQueries({ queryKey: ["tools", "playercfg"] });
  };
  const save = () => run("Saving…", async () => after(await saveCfg(game.game, choice), "Saved."));
  const takeOut = async () => {
    if (!(await ask(`Take the builder's settings out of ${game.label}? Your own lines in userconfig.cfg stay, and a copy of the file is kept first.`, { ok: "Take out" }))) return;
    await run("Taking out…", async () => after(await removeCfg(game.game), "Taken out."));
  };
  const putBack = async () => {
    if (!(await ask(`Put back the userconfig.cfg you had before the last save in ${game.label}?`, { ok: "Put back" }))) return;
    await run("Putting back…", async () => after(await restoreCfg(game.game), "Put back."));
  };

  const keyRow = (a: (typeof ALL_ACTIONS)[number]) => {
    const k = keyOf(a.id);
    const was = k ? game.binds[k] : undefined;
    const reserved = k ? RESERVED[k] : undefined;
    const note = reserved ? `${keyLabel(k!)} opens ${reserved}: pick another key.` : was && !game.ours.binds.some((b) => b.key === k) ? `Replaces ${keyLabel(k!)}: ${commandWords(was)}.` : a.note;
    return (
      <Row key={a.id} name={a.label} note={<span className={reserved ? "pc-warn" : undefined}>{note}</span>}>
        <KeyPick value={k} label={a.label} onPick={(key) => setKey(a.id, key)} onClear={() => setKey(a.id, null)} />
      </Row>
    );
  };

  return (
    <div className="pc tl-page">
      {status.data?.running && <Banner>The game is open. It reads these settings when it starts, so restart it after saving (or type exec userconfig.cfg in its console).</Banner>}
      {game.overridden.length > 0 && (
        <Banner>
          A line of your own after the builder's in userconfig.cfg sets {game.overridden.join(", ")} again, so yours wins there.
        </Banner>
      )}

      <div className="panel pc-top">
        <div className="tl-head">
          <div className="tl-head-title">
            <h3>Start from</h3>
          </div>
          {games.length > 1 && (
            <div className="tl-head-actions">
              <Segmented label="Game" values={games.map((g) => g.game).join(" ")} labels={games.map((g) => g.label).join("|")} current={game.game} onPick={(v) => (dirty ? ask("Switch game? Changes you haven't saved are dropped.", { ok: "Switch" }).then((ok) => ok && setGameId(v)) : setGameId(v))} />
            </div>
          )}
        </div>
        <div className="pc-presets">
          {PRESETS.map((p) => (
            <button key={p.id} type="button" className={`pc-preset${presetOn === p.id ? " on" : ""}`} aria-pressed={presetOn === p.id} onClick={() => setMany(p.values)}>
              <strong>{p.label}</strong>
              <span className="muted">{p.note}</span>
            </button>
          ))}
        </div>
        <p className="muted pc-hint">A preset sets the crosshair, connection and weapon switch. Your keys, sensitivity and sound stay as they are.</p>
      </div>

      <div className="pc-cross">
        <div className="panel pc-stage">
          <div className="pc-canvas-wrap">
            <CrosshairPreview colour={s.cl_crosshair_color} size={s.cl_crosshair_size} translucent={on("cl_crosshair_translucent")} dynamic={on("cl_dynamiccrosshair")} show={on("crosshair")} motion={motion} width={Number(res)} zoom={zoom} />
          </div>
          <div className="pc-stage-bar">
            <Segmented label="What you're doing" values="still run jump shoot" labels="Standing|Running|Jumping|Shooting" current={motion} onPick={(v) => setMotion(v as Motion)} />
            <Segmented label="Screen size" values="800 1024 1920" labels="800×600|1024×768|1920×1080" current={res} onPick={setRes} />
            <Segmented label="Zoom" values="1 2" labels="1×|2×" current={String(zoom)} onPick={(v) => setZoom(Number(v))} />
          </div>
          <p className="muted pc-hint">Drawn pixel for pixel at the screen size picked, as the game draws it with a rifle. The bright patch shows how a see-through crosshair fades on light walls.</p>
        </div>

        <div className="panel">
          <h3>Crosshair</h3>
          <Rows bare>
            <Row name="Show the crosshair">
              <Switch on={on("crosshair")} onChange={toggle("crosshair")} label="Show the crosshair" />
            </Row>
            <Row name="Colour">
              <span className="pc-swatches">
                {COLOURS.map(([name, c]) => (
                  <button key={c} type="button" className="pc-swatch" style={{ background: toHex(c) }} aria-pressed={s.cl_crosshair_color === c} aria-label={name} title={name} onClick={() => set("cl_crosshair_color", c)} />
                ))}
                <label className="pc-custom" title="Any colour">
                  <input type="color" value={toHex(s.cl_crosshair_color)} aria-label="Any colour" onChange={(e) => set("cl_crosshair_color", fromHex(e.target.value))} />
                </label>
              </span>
            </Row>
            <Row name="Size" info="Auto picks by your screen size: the bigger the screen, the bigger the crosshair.">
              <Segmented label="Size" values="auto small medium large" labels="Auto|Small|Medium|Large" current={s.cl_crosshair_size} onPick={(v) => set("cl_crosshair_size", v)} />
            </Row>
            <Row name="See-through" note="Blends into the picture. Off: solid, easier to see on bright walls">
              <Switch on={on("cl_crosshair_translucent")} onChange={toggle("cl_crosshair_translucent")} label="See-through" />
            </Row>
            <Row name="Moves when you do" note="Opens up while you run, jump and shoot, showing how accurate you are">
              <Switch on={on("cl_dynamiccrosshair")} onChange={toggle("cl_dynamiccrosshair")} label="Moves when you do" />
            </Row>
            <Row name="While watching others" note="Shows a crosshair when you spectate">
              <Switch on={on("cl_observercrosshair")} onChange={toggle("cl_observercrosshair")} label="While watching others" />
            </Row>
          </Rows>
        </div>
      </div>

      <div className="pc-grid">
        <div className="panel">
          <h3>Mouse</h3>
          <Rows bare>
            <Row name="Sensitivity" info="How far the view turns for a move of the mouse. Most players sit between 1.5 and 4.">
              <NumberField label="Sensitivity" value={s.sensitivity} min={0.1} max={10} step={0.05} onChange={(v) => set("sensitivity", v)} />
            </Row>
            <Row name="Zoomed sensitivity" info="With a scope, the turn is this much of your usual sensitivity scaled to the zoom. 1 keeps it the same feel; the game's own is 1.2.">
              <NumberField label="Zoomed sensitivity" value={s.zoom_sensitivity_ratio} min={0.1} max={3} step={0.05} onChange={(v) => set("zoom_sensitivity_ratio", v)} />
            </Row>
            <Row name="Raw mouse input" note="Reads the mouse directly, without the system's acceleration">
              <Switch on={on("m_rawinput")} onChange={toggle("m_rawinput")} label="Raw mouse input" />
            </Row>
            <Row name="Smooth the mouse" note="Averages movement over two frames. Feels floaty; most leave it off">
              <Switch on={on("m_filter")} onChange={toggle("m_filter")} label="Smooth the mouse" />
            </Row>
          </Rows>
        </div>

        <div className="panel">
          <h3>Connection</h3>
          <Rows bare>
            <Row name="Your connection" info="How much the game sends and asks for each second. On your own game or a LAN it hardly matters; on internet servers, faster settings feel sharper if your connection keeps up.">
              <Segmented label="Your connection" values={NET.map((n) => n.id).join(" ")} labels={NET.map((n) => n.label).join("|")} current={netOn} onPick={(v) => setMany(NET.find((n) => n.id === v)!.values)} />
            </Row>
          </Rows>
          <details className="pc-more">
            <summary>Each setting</summary>
            <Rows bare>
              <Row name="Rate" note="Bytes a second the server may send you">
                <NumberField label="Rate" value={s.rate} min={2500} max={100000} step={500} onChange={(v) => set("rate", v)} />
              </Row>
              <Row name="Updates a second" note="cl_updaterate: how often the server sends the world">
                <NumberField label="Updates a second" value={s.cl_updaterate} min={10} max={128} step={1} onChange={(v) => set("cl_updaterate", v)} />
              </Row>
              <Row name="Commands a second" note="cl_cmdrate: how often you send your moves">
                <NumberField label="Commands a second" value={s.cl_cmdrate} min={10} max={128} step={1} onChange={(v) => set("cl_cmdrate", v)} />
              </Row>
              <Row name="Smoothing" info="ex_interp: how far behind the game draws others, to smooth their movement. The game never uses less than one update (0.01 at 100 updates a second).">
                <NumberField label="Smoothing" value={s.ex_interp} min={0} max={0.1} step={0.01} unit="seconds" onChange={(v) => set("ex_interp", v)} />
              </Row>
            </Rows>
          </details>
        </div>

        <div className="panel">
          <h3>Screen and HUD</h3>
          <Rows bare>
            <Row name="Frame rate limit" info="The most frames a second the game draws (fps_max). 100 is the classic; 101 lets the game hold 100.">
              <NumberField label="Frame rate limit" value={s.fps_max} min={30} max={1000} step={1} unit="fps" onChange={(v) => set("fps_max", v)} />
            </Row>
            <Row name="Fast weapon switch" note="Number keys and the wheel switch at once, without the menu">
              <Switch on={on("hud_fastswitch")} onChange={toggle("hud_fastswitch")} label="Fast weapon switch" />
            </Row>
            <Row name="Names in the middle" note="Shows who you aim at in the middle of the screen instead of the corner">
              <Switch on={on("hud_centerid")} onChange={toggle("hud_centerid")} label="Names in the middle" />
            </Row>
            <Row name="Gun in the right hand">
              <Switch on={on("cl_righthand")} onChange={toggle("cl_righthand")} label="Gun in the right hand" />
            </Row>
            <Row name="Switch to better weapons you pick up">
              <Switch on={on("_cl_autowepswitch")} onChange={toggle("_cl_autowepswitch")} label="Switch to better weapons you pick up" />
            </Row>
            <Row name="Solid radar" note="A solid radar instead of a see-through one">
              <Switch on={on("cl_radartype")} onChange={toggle("cl_radartype")} label="Solid radar" />
            </Row>
            <Row name="Same model for everyone" note="One player model per team: easier to tell sides apart, a little faster">
              <Switch on={on("cl_minmodels")} onChange={toggle("cl_minmodels")} label="Same model for everyone" />
            </Row>
            <Row name="Show frames a second">
              <Switch on={on("cl_showfps")} onChange={toggle("cl_showfps")} label="Show frames a second" />
            </Row>
            <Row name="Net graph" info="Ping, frame rate and loss at the bottom of the screen. 1 is the short one, 3 the most detail.">
              <Segmented label="Net graph" values="0 1 2 3" labels="Off|1|2|3" current={s.net_graph} onPick={(v) => set("net_graph", v)} />
            </Row>
          </Rows>
        </div>

        <div className="panel">
          <h3>Sound and voice</h3>
          <Rows bare>
            <Row name="Game volume">
              <NumberField label="Game volume" value={s.volume} min={0} max={1} step={0.05} onChange={(v) => set("volume", v)} />
            </Row>
            <Row name="Voice chat" note="Hear other players' microphones">
              <Switch on={on("voice_enable")} onChange={toggle("voice_enable")} label="Voice chat" />
            </Row>
            <Row name="Voice volume">
              <NumberField label="Voice volume" value={s.voice_scale} min={0} max={2} step={0.05} onChange={(v) => set("voice_scale", v)} />
            </Row>
          </Rows>
        </div>
      </div>

      <div className="panel">
        <h3>Keys</h3>
        <p className="muted pc-hint">Press "Choose a key", then the key, mouse button or wheel. Esc stops. F3 and F4 stay Loner-Strike's menus.</p>
        <div className="pc-keys">
          {ACTIONS.map((g) => (
            <section key={g.title}>
              <h4>{g.title}</h4>
              <Rows bare>{g.actions.map(keyRow)}</Rows>
            </section>
          ))}
        </div>
      </div>

      <div className="panel pc-save">
        <div className="row between pc-save-row">
          <div>
            <strong>{dirty ? "You have changes to save" : "Nothing new to save"}</strong>{" "}
            <Info text="Saved in the game's userconfig.cfg, which it runs every time it starts, after its own config.cfg. Only settings that differ from the game's go in, so the rest still follow the game's Options. Your own lines in that file stay, and a copy of the old file is kept first." />
            <p className="muted">In {game.label}'s userconfig.cfg</p>
          </div>
          <div className="row">
            {game.canRestore && <BusyButton className="btn" busy={working === "Putting back…"} disabled={!!working} label="Put back the previous file" busyLabel="Putting back…" onClick={putBack} />}
            {game.hasBlock && <BusyButton className="btn" busy={working === "Taking out…"} disabled={!!working} label="Take out" busyLabel="Taking out…" onClick={takeOut} />}
            {dirty && (
              <button type="button" className="btn" disabled={!!working} onClick={() => setChoice(start)}>
                Undo changes
              </button>
            )}
            <BusyButton className="btn primary" busy={working === "Saving…"} disabled={!dirty || !!working || !!lineError} label="Save" busyLabel="Saving…" onClick={save} />
          </div>
        </div>
        {lineError && <p className="pc-warn" role="alert">{lineError}</p>}
        <details className="pc-more">
          <summary>What gets saved</summary>
          {lines === null ? <Loading text="Working it out…" /> : lines.length === 0 ? <p className="muted">Nothing: everything is as the game has it.</p> : <pre className="pc-lines">{lines.join("\n")}</pre>}
        </details>
      </div>
    </div>
  );
}

