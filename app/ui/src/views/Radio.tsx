import { useCallback, useEffect, useRef, useState } from "react";
import { create } from "zustand";

/** Internet radio, the site's little Winamp in the corner. Never autoplays: the stream is only
 *  requested on play, and stop drops the connection. Station, volume and whether it's open are
 *  remembered in localStorage when it's there. */
const STATIONS = [
  { name: "Bassdrive", desc: "Worldwide drum and bass", stream: "https://ice.bassdrive.net/stream", site: "https://www.bassdrive.com/", credit: "Bassdrive", now: "https://ice.bassdrive.net/status-json.xsl" },
  { name: "SomaFM The Trip", desc: "Progressive house and trance", stream: "https://ice1.somafm.com/thetrip-128-mp3", site: "https://somafm.com/thetrip/", credit: "SomaFM: The Trip", soma: true, now: "https://somafm.com/songs/thetrip.json" },
  { name: "SomaFM Groove Salad", desc: "Ambient and downtempo", stream: "https://ice1.somafm.com/groovesalad-128-mp3", site: "https://somafm.com/groovesalad/", credit: "SomaFM: Groove Salad", soma: true, now: "https://somafm.com/songs/groovesalad.json" },
];

/** What's on now: SomaFM lists the track; Bassdrive (mostly live DJ sets) names the show. */
async function nowPlaying(s: (typeof STATIONS)[number], signal: AbortSignal): Promise<string> {
  const r = await fetch(s.now, { signal, cache: "no-store" });
  const j = await r.json();
  if (s.soma) {
    const song = j?.songs?.[0];
    return song ? [song.artist, song.title].filter(Boolean).join(" - ") : "";
  }
  const sources = [j?.icestats?.source].flat().filter(Boolean);
  const main = sources.find((x: { listenurl?: string }) => x.listenurl?.endsWith("/stream")) ?? sources[0];
  return String(main?.title ?? "").trim();
}
const FAILED = "This station isn't answering. Try another.";

const keep = {
  get(k: string) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k: string, v: string) { try { localStorage.setItem(k, v); } catch { /* private window */ } },
};
const reduce = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

type State = "stopped" | "connecting" | "playing" | "error";

/** Whether the radio is open, and what's on, for the button in the top bar. The player itself
 *  stays mounted in App so music keeps going whatever page or game is showing. */
const useRadioStore = create<{ open: boolean; on: boolean; label: string; toggle: () => void; close: () => void }>((set) => ({
  open: false,
  on: false,
  label: "",
  toggle: () => set((s) => ({ open: !s.open })),
  close: () => set({ open: false }),
}));

/** The top bar's Radio button: opens the player under it; the light is on while it plays. */
export function RadioButton() {
  const { open, on, label, toggle } = useRadioStore();
  return (
    <button type="button" id="radio-button" className={`radio-toggle${on ? " on" : ""}`} aria-expanded={open} aria-controls="radio" onClick={toggle} title={on ? label : "Radio"}>
      <i aria-hidden="true" />Radio
    </button>
  );
}

export function Radio() {
  const [at, setAt] = useState(() => Math.min(STATIONS.length - 1, Math.max(0, Number(keep.get("ls-radio-station")) || 0)));
  const [state, setState] = useState<State>("stopped");
  const [seconds, setSeconds] = useState(0);
  const [vol, setVol] = useState(() => {
    const v = Number(keep.get("ls-radio-volume") ?? "70");
    return v >= 0 && v <= 100 ? v : 70;
  });
  const open = useRadioStore((s) => s.open);
  const [announce, setAnnounce] = useState("");
  const [track, setTrack] = useState("");
  const audio = useRef<HTMLAudioElement | null>(null);
  const tick = useRef<number | null>(null);
  const stateRef = useRef<State>("stopped");
  const canvas = useRef<HTMLCanvasElement>(null);
  const titleRef = useRef<HTMLDivElement>(null);
  const playRef = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLElement>(null);
  const station = STATIONS[at];

  const set = (s: State) => {
    stateRef.current = s;
    setState(s);
  };
  if (!audio.current && typeof Audio !== "undefined") {
    audio.current = new Audio();
    audio.current.preload = "none";
  }

  // Drop the stream completely, so nothing keeps downloading in the background
  const disconnect = useCallback(() => {
    if (tick.current) clearInterval(tick.current);
    tick.current = null;
    const a = audio.current;
    if (!a) return;
    a.pause();
    a.removeAttribute("src");
    a.load();
  }, []);
  const fail = useCallback(() => {
    disconnect();
    set("error");
  }, [disconnect]);

  const play = useCallback((index = at) => {
    const a = audio.current;
    if (!a || stateRef.current === "playing" || stateRef.current === "connecting") return;
    set("connecting");
    setSeconds(0);
    let last = 0;
    let stuck = 0;
    a.src = STATIONS[index].stream;
    a.play().catch((e: DOMException) => { if (e.name !== "AbortError" && stateRef.current === "connecting") fail(); });
    // once a second: the clock, and give up if the stream hasn't moved for 15 s
    tick.current = window.setInterval(() => {
      if (a.currentTime > last) { last = a.currentTime; stuck = 0; } else stuck++;
      if (stateRef.current === "playing" && !stuck) setSeconds((s) => s + 1);
      if (stuck >= 15) fail();
    }, 1000);
  }, [at, fail]);

  const stop = () => {
    disconnect();
    set("stopped");
    setSeconds(0);
  };

  const tune = (step: number) => {
    const wasOn = stateRef.current === "playing" || stateRef.current === "connecting";
    if (wasOn) disconnect();
    const next = (at + step + STATIONS.length) % STATIONS.length;
    setAt(next);
    keep.set("ls-radio-station", String(next));
    set("stopped");
    setSeconds(0);
    setAnnounce(`${STATIONS[next].name}, ${STATIONS[next].desc.toLowerCase()}`);
    if (wasOn) play(next);
  };

  useEffect(() => {
    const a = audio.current;
    if (!a) return;
    const onPlaying = () => { if (stateRef.current === "connecting") set("playing"); };
    const onWaiting = () => { if (stateRef.current === "playing") set("connecting"); };
    const onError = () => { if (stateRef.current === "playing" || stateRef.current === "connecting") fail(); };
    a.addEventListener("playing", onPlaying);
    a.addEventListener("waiting", onWaiting);
    a.addEventListener("error", onError);
    return () => {
      a.removeEventListener("playing", onPlaying);
      a.removeEventListener("waiting", onWaiting);
      a.removeEventListener("error", onError);
    };
  }, [fail]);
  // stop for good when the app closes this view
  useEffect(() => disconnect, [disconnect]);

  useEffect(() => {
    if (audio.current) audio.current.volume = vol / 100;
    keep.set("ls-radio-volume", String(vol));
  }, [vol]);

  // Fake spectrum: bars that bounce while playing, a still picture with reduced motion
  useEffect(() => {
    const ctx = canvas.current?.getContext("2d");
    if (!ctx || !open) return;
    const BARS = 18;
    const levels = Array(BARS).fill(0);
    const peaks = Array(BARS).fill(0);
    const signal = getComputedStyle(document.documentElement).getPropertyValue("--signal").trim() || "#c4b550";
    const draw = () => {
      ctx.clearRect(0, 0, 74, 22);
      ctx.fillStyle = signal;
      for (let i = 0; i < BARS; i++) {
        const h = Math.max(1, Math.round(levels[i] * 20));
        ctx.globalAlpha = 0.85;
        ctx.fillRect(i * 4 + 1, 22 - h, 3, h);
        ctx.globalAlpha = 1;
        if (peaks[i] > 0.05) ctx.fillRect(i * 4 + 1, 21 - Math.round(peaks[i] * 20), 3, 1);
      }
    };
    const on = state === "playing";
    if (!on || reduce) {
      for (let i = 0; i < BARS; i++) levels[i] = on ? 0.25 + 0.55 * Math.abs(Math.sin(i * 0.9)) * (1 - i / (BARS * 1.6)) : 0;
      draw();
      return;
    }
    let alive = true;
    let timer = 0;
    const step = () => {
      if (!alive) return;
      for (let i = 0; i < BARS; i++) {
        // lows a bit louder than highs, like most music
        const target = Math.random() * (1 - i / (BARS * 1.6)) * (Math.random() < 0.2 ? 1 : 0.7);
        levels[i] = target > levels[i] ? target : levels[i] * 0.82;
        peaks[i] = Math.max(levels[i], peaks[i] - 0.03);
      }
      draw();
      timer = window.setTimeout(() => requestAnimationFrame(step), 60);
    };
    step();
    return () => { alive = false; clearTimeout(timer); };
  }, [state, open]);

  // What's on now, asked for only while a station is playing
  useEffect(() => {
    setTrack("");
    if (state !== "playing") return;
    const ctl = new AbortController();
    const ask = () => nowPlaying(station, ctl.signal).then((t) => setTrack(t.slice(0, 120))).catch(() => { /* keep the station name */ });
    ask();
    const id = window.setInterval(ask, 20000);
    return () => { ctl.abort(); clearInterval(id); };
  }, [state, station]);

  // Title marquee: scrolls only when it doesn't fit, and never with reduced motion
  const title = state === "error" ? FAILED : track ? `${at + 1}. ${station.name} · ${track}` : `${at + 1}. ${station.name} · ${station.desc}`;
  const [scroll, setScroll] = useState(false);
  useEffect(() => {
    const box = titleRef.current;
    const span = box?.firstElementChild as HTMLElement | null;
    if (!box || !span) return;
    setScroll(false);
    requestAnimationFrame(() => setScroll(!reduce && span.offsetWidth > box.clientWidth));
  }, [title, open]);

  const close = () => {
    useRadioStore.getState().close();
    document.getElementById("radio-button")?.focus();
  };
  // the button's light and tooltip follow what's playing
  useEffect(() => {
    useRadioStore.setState({ on: state === "playing" || state === "connecting", label: state === "playing" ? `Playing ${station.name}` : "" });
  }, [state, station]);
  // opening moves focus to Play; Escape or a click elsewhere closes it again
  useEffect(() => {
    if (!open) return;
    requestAnimationFrame(() => playRef.current?.focus());
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    const onDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!panel.current?.contains(t) && !document.getElementById("radio-button")?.contains(t)) useRadioStore.getState().close();
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    return () => { document.removeEventListener("keydown", onKey); document.removeEventListener("pointerdown", onDown); };
  }, [open]);

  const clock = `${String(Math.floor(seconds / 60) % 100).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;

  if (!open) return <p className="sr-only" aria-live="polite">{announce}</p>;
  return (
    <section ref={panel} id="radio" className="radio" aria-label="Radio">
      <div className="radio-bar">
        <span>Loner-Strike radio</span>
        <button type="button" onClick={close} aria-label="Close radio">
          <svg width="8" height="8" viewBox="0 0 8 8" aria-hidden="true"><path d="M0 6h8v2H0z" /></svg>
        </button>
      </div>
      <div className="radio-lcd">
        <span className={`radio-time${state === "playing" || state === "connecting" ? "" : " off"}`} aria-hidden="true">{clock}</span>
        <div ref={titleRef} className={`radio-title${scroll ? " scroll" : ""}`}>
          <span data-t={scroll ? title : undefined} style={scroll ? ({ "--marquee-s": `${Math.round(title.length / 4)}s` } as React.CSSProperties) : undefined}>{title}</span>
        </div>
        <canvas ref={canvas} className="radio-vis" width={74} height={22} aria-hidden="true" />
        <div className="radio-info">
          <span>MP3</span>
          <span>{{ stopped: "Stopped", connecting: "Connecting…", playing: "Playing", error: "Stopped" }[state]}</span>
          <b className={state === "playing" ? "" : "off"}>Live</b>
        </div>
      </div>
      <div className="radio-controls">
        <button type="button" onClick={() => tune(-1)} aria-label="Previous station">
          <svg width="12" height="10" viewBox="0 0 12 10" aria-hidden="true"><path d="M0 0h2v10H0zM12 0v10L3 5z" /></svg>
        </button>
        <button ref={playRef} type="button" onClick={() => play()} aria-label="Play">
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l9 5-9 5z" /></svg>
        </button>
        <button type="button" onClick={stop} aria-label="Stop">
          <svg width="9" height="9" viewBox="0 0 9 9" aria-hidden="true"><path d="M0 0h9v9H0z" /></svg>
        </button>
        <button type="button" onClick={() => tune(1)} aria-label="Next station">
          <svg width="12" height="10" viewBox="0 0 12 10" aria-hidden="true"><path d="M10 0h2v10h-2zM0 0v10l9-5z" /></svg>
        </button>
        <input type="range" min={0} max={100} value={vol} onChange={(e) => setVol(Number(e.target.value))} aria-label="Volume" />
      </div>
      <p className="radio-credit">
        <a href={station.site} target="_blank" rel="noreferrer">{station.credit}</a>
        {station.soma && <a href="https://somafm.com/support/" target="_blank" rel="noreferrer">Support SomaFM</a>}
      </p>
      <p className="sr-only" aria-live="polite">{announce}</p>
    </section>
  );
}
