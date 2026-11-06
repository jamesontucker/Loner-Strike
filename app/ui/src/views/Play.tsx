import { useState } from "react";
import { postJson } from "../api";
import { Icon } from "../icons";
import type { AppInfo, MapInfo, Online } from "../types";
import { SectionHead, useCommand, useConfirm, useToast } from "../ui";
import { MapCard } from "./Maps";

// How well bots handle each mode (shown on the cards)
const BOT_RATING: Record<string, "great" | "ok" | "weak" | "none"> = {
  hns: "ok", deathrun: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};
const BOT_NOTE = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };

const GAME_NAME = (id?: string) => (id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero");

export function PlayIdle({ app, reason, maps, openSetup }: { app: AppInfo | null; reason: string; maps: MapInfo[]; openSetup: () => void }) {
  const toast = useToast();
  const [starting, setStarting] = useState(false);
  const game = GAME_NAME(app?.game?.id);
  const [pick, setPick] = useState("");
  const [botsPick, setBotsPick] = useState<number | null>(null);
  const launchMenu = async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", { menu: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) toast(r.error, true);
    else toast(`Opening ${game}…`);
  };
  const launch = async (map?: string, bots?: number) => {
    setStarting(true);
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", { ...(map ? { map } : {}), ...(bots !== undefined ? { bots } : {}) }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) {
      toast(r.error, true);
      setStarting(false);
    } else {
      toast(`Starting ${game}…`);
      setTimeout(() => setStarting(false), 20000);
    }
  };
  const bots = app?.bots ?? null;
  if (app?.game && !app.game.installed?.includes("core")) {
    return (
      <div className="idle">
        <div className="idle-hero">
          <h1>Not set up yet.</h1>
          <p>Loner-Strike adds the bots, the modes and the in-game menu to {game}. Setup takes a couple of minutes, and everything it changes is backed up.</p>
          <button className="btn launch" onClick={openSetup}>
            <Icon name="play" size={22} />
            Set it up
          </button>
          <div className="last">
            Or <button className="link inline" onClick={() => launch()}>start {game} without it</button>.
          </div>
        </div>
      </div>
    );
  }
  const recent = maps.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent).slice(0, 10);
  const favs = maps.filter((m) => m.fav && !m.hidden).sort((a, b) => a.name.localeCompare(b.name));
  const others = maps.filter((m) => !m.hidden && m.recent < 0 && !m.fav).sort((a, b) => a.name.localeCompare(b.name));
  const stock = app?.game?.id === "czero" ? "de_dust2_cz" : "de_dust2";
  const map = pick || app?.lastmap || recent[0]?.name || favs[0]?.name || (maps.some((m) => m.name === stock) ? stock : maps[0]?.name) || stock;
  const botCount = botsPick ?? bots ?? 9;

  if (!app?.game) {
    return (
      <div className="idle">
        <div className="idle-hero">
          <h1>No game yet.</h1>
          <p>{reason || "Loner-Strike couldn't find Counter-Strike 1.6 or Condition Zero on this computer."}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="idle">
      <div className="idle-hero">
        <h1>Start a game</h1>
        <p>{game} isn't running. Pick a map and how many bots, and Loner-Strike starts it and connects by itself.</p>
        <div className="start-box">
          <label className="field">
            <span>Map</span>
            <select value={map} onChange={(e) => setPick(e.target.value)}>
              {recent.length > 0 && <optgroup label="Recently played">{recent.map((m) => <option key={"r" + m.name} value={m.name}>{m.name}</option>)}</optgroup>}
              {favs.length > 0 && <optgroup label="Favourites">{favs.map((m) => <option key={"f" + m.name} value={m.name}>{m.name}</option>)}</optgroup>}
              <optgroup label="All maps">
                {(others.length ? others : [{ name: stock }]).map((m) => <option key={m.name} value={m.name}>{m.name}</option>)}
              </optgroup>
            </select>
          </label>
          <div className="field">
            <span>Bots</span>
            <div className="seg" role="group" aria-label="Bots">
              {[0, 5, 9, 15].map((n) => <button key={n} aria-pressed={botCount === n} onClick={() => setBotsPick(n)}>{n === 0 ? "None" : n}</button>)}
            </div>
          </div>
        </div>
        <button className="btn launch" disabled={starting} onClick={() => launch(map, botCount)}>
          <Icon name="play" size={22} />
          {starting ? "Starting…" : `Play ${map}`}
        </button>
        <div className="last">
          Change the mode once you're in, here or with F3 in the game.{" "}
          <button className="link inline" onClick={() => launchMenu()}>Just open {game}</button>
        </div>
      </div>
      {(recent.length > 0 || favs.length > 0) && (
        <section>
          <h2>{recent.length > 0 ? "Recently played" : "Favourites"}</h2>
          <div className="maps">
            {(recent.length > 0 ? recent.slice(0, 6) : favs.slice(0, 6)).map((m) => (
              <MapCard key={m.name} map={m} onClick={() => launch(m.name, botCount)} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

export function PlayLive({ data, maps, refresh, openMaps }: { data: Online; maps: MapInfo[]; refresh: () => void; openMaps: () => void }) {
  const ask = useConfirm();
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
                onClick={async () => (await ask(`Play "${p.name}"? This resets that mode's settings to the preset.`, { ok: "Play it" })) && run("gm_preset", [p.id])}
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
