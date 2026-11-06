import { useState } from "react";
import { postJson } from "../api";
import { Icon } from "../icons";
import type { AppInfo, MapInfo, Online } from "../types";
import { SectionHead, useCommand, useToast } from "../ui";
import { MapCard } from "./Maps";

// How well bots handle each mode (shown on the cards)
const BOT_RATING: Record<string, "great" | "ok" | "weak" | "none"> = {
  hns: "ok", deathrun: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};
const BOT_NOTE = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };

const GAME_NAME = (id?: string) => (id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero");

export function PlayIdle({ app, reason, maps }: { app: AppInfo | null; reason: string; maps: MapInfo[] }) {
  const toast = useToast();
  const [starting, setStarting] = useState(false);
  const game = GAME_NAME(app?.game?.id);
  const launch = async (map?: string) => {
    setStarting(true);
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", map ? { map } : {}).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) {
      toast(r.error, true);
      setStarting(false);
    } else {
      toast(`Starting ${game}…`);
      setTimeout(() => setStarting(false), 20000);
    }
  };
  const bots = app?.bots ?? null;
  const recent = maps.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent).slice(0, 6);

  return (
    <div className="idle">
      <div className="idle-hero">
        <h1>
          0 friends online.
          <br />
          {bots === null ? "Bots" : bots} bot{bots === 1 ? "" : "s"} ready.
        </h1>
        <p>
          {app?.game ? `Start ${game} and Loner-Strike connects by itself.` : reason}
        </p>
        {app?.game && (
          <button className="btn launch" disabled={starting} onClick={() => launch()}>
            <Icon name="play" size={22} />
            {starting ? "Starting…" : app.lastmap ? `Play ${app.lastmap}` : `Play ${game}`}
          </button>
        )}
        {app?.game && (
          <div className="last">
            {app.lastmap ? "Starts a game on the last map you played. " : ""}
            Or pick a map below.
          </div>
        )}
      </div>
      {recent.length > 0 && (
        <section>
          <h2>Recently played</h2>
          <div className="maps">
            {recent.map((m) => (
              <MapCard key={m.name} map={m} onClick={() => launch(m.name)} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

export function PlayLive({ data, maps, refresh, openMaps }: { data: Online; maps: MapInfo[]; refresh: () => void; openMaps: () => void }) {
  const run = useCommand(refresh);
  const st = data.state;
  const recent = maps.filter((m) => m.recent >= 0 && m.name.toLowerCase() !== st.map.toLowerCase()).sort((a, b) => a.recent - b.recent).slice(0, 6);

  return (
    <div className="stack">
      {data.presets.length > 0 && (
        <section>
          <SectionHead title="Presets" note="One click sets the mode, its settings and the bots." />
          <div className="presets">
            {data.presets.map((p) => (
              <button
                key={p.id}
                className="preset"
                aria-pressed={st.preset === p.id}
                onClick={() => confirm(`Play "${p.name}"? This resets that mode's settings to the preset.`) && run("gm_preset", [p.id])}
              >
                <span className="preset-name">{p.name}</span>
                <span className="preset-desc">{p.desc}</span>
              </button>
            ))}
          </div>
        </section>
      )}
      <section>
        <SectionHead title="Game mode" note="Switching reloads the map. Each mode remembers its own settings." />
        <div className="modes">
          {data.modes.map((m) => {
            const rating = BOT_RATING[m.id] ?? "great";
            return (
              <button
                key={m.id}
                className="mode"
                aria-pressed={st.mode === m.id}
                onClick={() => st.mode !== m.id && run("gm_mode", [m.id])}
              >
                <span className="mode-icon">
                  <Icon name={m.id} size={26} />
                </span>
                <span className="mode-name">{m.name}</span>
                <span className="mode-desc">{m.desc}</span>
                <span className={`bots-note ${rating}`}>{BOT_NOTE[rating]}</span>
              </button>
            );
          })}
        </div>
      </section>
      {recent.length > 0 && (
        <section>
          <SectionHead title="Recently played">
            <button className="link" onClick={openMaps}>All maps</button>
          </SectionHead>
          <div className="maps">
            {recent.map((m) => (
              <MapCard key={m.name} map={m} onClick={() => run("gm_map", [m.name])} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
