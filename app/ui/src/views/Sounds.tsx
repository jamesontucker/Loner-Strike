import { useCallback, useEffect, useRef, useState } from "react";
import { getJson, ON_THIS_COMPUTER, postJson, SETUP_ON_COMPUTER, useActions, useActivity, usePoll, isAbort } from "../api";
import { reloadServers, useGameState, useServers, useSetupJob, waitForSetupJob } from "../queries";
import { restartPending } from "../types";
import { useNav } from "../store";
import { Banner, BusyButton, LoadError, SectionHead, Segmented, Skeleton, SteamProgress, Switch, useCommand, useConfirm, useToast } from "../ui";

type Track = { name: string; size: number; on: boolean; kind: "mp3" | "wav"; pack?: string | null; problem?: string | null };
type Pool = { id: "t" | "ct" | "any"; label: string; tracks: Track[]; on: number; over: boolean };
type Music = { installed: boolean; on: boolean; pools: Pool[]; total: number; limit: number; good: number; warn_total: number; max: number; fallback?: number };
type Data = { game: string; hl: string; dedicated: boolean; setup: boolean; music: Music; quake: { installed: boolean; on: boolean }; announcer?: string; error?: string };
type Pack = { id: number; name: string; author: string; kind: "music" | "replace"; about: string; installed: boolean; url: string; note?: string };
type PackJob = { id: string; mod: number; state: string; message: string; got: number; size: number };
type Packs = { packs: Pack[]; jobs: PackJob[]; error?: string };
type Reply = { ok?: boolean; error?: string; how?: string; job?: string; cancelled?: boolean; added?: string[]; notes?: string[] };

const POOL_NOTE: Record<string, string> = {
  t: "Plays when the Terrorists win the round.",
  ct: "Plays when the Counter-Terrorists win the round.",
  any: "Plays for whichever team wins, mixed in with that team's own tracks.",
};
const size = (n: number) => (n >= 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
const failed = (): Reply => ({ error: "Couldn't reach the app." });
const busyState = (s: string) => !["done", "error"].includes(s);

/** Round-end music, Quake sounds and sound packs, for the player's game or a dedicated server. */
type SoundTab = "music" | "quake" | "packs";
const SOUND_TABS: [SoundTab, string][] = [["music", "Round-end music"], ["quake", "Quake sounds"], ["packs", "Sound packs"]];

export function Sounds({ target }: { target: string }) {
  const toast = useToast();
  const ask = useConfirm();
  const run = useCommand();
  const [d, setD] = useState<Data | null>(null);
  const [tab, setTabRaw] = useState<SoundTab>(() => {
    try {
      const t = localStorage.getItem("ls.soundTab") as SoundTab;
      return SOUND_TABS.some(([id]) => id === t) ? t : "music";
    } catch {
      return "music";
    }
  });
  const setTab = (t: SoundTab) => {
    setTabRaw(t);
    try {
      localStorage.setItem("ls.soundTab", t);
    } catch {
      /* private window */
    }
  };
  const [packs, setPacks] = useState<Packs | null>(null);
  const [packsFailed, setPacksFailed] = useState(false);
  const go = useNav((s) => s.go);
  // the restart button only works while the game (or server) is running
  const live = !!useGameState().state?.online;
  const [changedHere, setChanged] = useState(false);
  // on a server, the app also keeps what waits for the next map (so it survives a reload)
  const server = useServers(target !== "local").data?.servers.find((x) => x.id === target);
  const serverWaits = restartPending(server).sounds;
  // the map was just restarted here: the app's record catches up on its next answer
  const [restarted, setRestarted] = useState(false);
  useEffect(() => {
    if (!serverWaits) setRestarted(false);
  }, [serverWaits]);
  const changed = changedHere || (serverWaits && !restarted);
  const [playing, setPlaying] = useState<string | null>(null);
  const jobQ = useSetupJob();
  const setupJob = jobQ.data?.state === "running" ? jobQ.data : null;
  const { run: once, busy: isBusy } = useActions();
  const key = (what: string) => `sounds:${target}:${what}`;
  const installing = isBusy(key("install-music"));
  // one track change at a time
  const busy = useActivity().some(([k]) => k.startsWith(key("track:")));
  const audio = useRef<HTMLAudioElement | null>(null);
  const seen = useRef(new Set<string>());

  const load = useCallback(async () => {
    let cancelled = false;
    const [s, p] = await Promise.all([
      getJson<Data>("/api/sounds").catch((e) => {
        cancelled ||= isAbort(e);
        return { error: "Couldn't reach the app." } as Data;
      }),
      getJson<Packs>("/api/sounds/packs").catch(() => null),
    ]);
    // another server was opened meanwhile: this answer isn't for this page any more
    if (cancelled) return;
    setD(s);
    setPacksFailed(!p || !!p.error);
    if (p && !p.error) {
      // finished jobs from before this page opened aren't news
      setPacks((old) => {
        if (!old) p.jobs.forEach((j) => !busyState(j.state) && seen.current.add(j.id));
        return p;
      });
    }
  }, []);

  // a different game or server: start over
  useEffect(() => {
    setD(null);
    setPacks(null);
    setPacksFailed(false);
    setChanged(false);
    setRestarted(false);
    stop();
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target, load]);

  useEffect(() => () => audio.current?.pause(), []);

  // follow pack downloads while any run
  const packBusy = !!packs?.jobs.some((j) => busyState(j.state));
  usePoll(async () => {
    const p = await getJson<Packs>("/api/sounds/packs").catch(() => null);
    if (!p || p.error) return;
    setPacks(p);
    let music = false;
    for (const j of p.jobs) {
      if (!busyState(j.state) && !seen.current.has(j.id)) {
        seen.current.add(j.id);
        toast(j.message || (j.state === "done" ? "Installed" : "Couldn't install that pack."), j.state === "error");
        if (j.id.startsWith("roundsound-") && j.state === "done") music = true;
      }
    }
    if (music) {
      setChanged(true);
      load();
    }
  }, 1200, [], packBusy);

  function stop() {
    audio.current?.pause();
    setPlaying(null);
  }

  const fileUrl = (pool: string, t: Track) =>
    `/api/sounds/file?pool=${pool}&name=${encodeURIComponent(t.name)}&off=${t.on ? 0 : 1}${target !== "local" ? `&target=${encodeURIComponent(target)}` : ""}`;

  const play = (pool: string, t: Track) => {
    const key = `${pool}/${t.on ? "" : "off/"}${t.name}`;
    if (playing === key) return stop();
    if (!audio.current) {
      audio.current = new Audio();
      audio.current.onended = () => setPlaying(null);
    }
    const a = audio.current;
    a.onerror = () => {
      setPlaying(null);
      toast("Couldn't play that one here. The game may still play it.", true);
    };
    a.src = fileUrl(pool, t);
    a.play().then(() => setPlaying(key)).catch(() => setPlaying(null));
  };

  const track = async (body: Record<string, unknown>, label: string, done?: string) => {
    if (busy) return;
    await once(key(`track:${body.pool}/${body.off ? "off/" : ""}${body.name}`), async () => {
      const r = await postJson<Reply>("/api/sounds/track", body).catch(failed);
      if (r.error) return toast(r.error, true);
      if (done) toast(done);
      setChanged(true);
      load();
    }, label);
  };

  const remove = async (pool: string, t: Track) => {
    if (!(await ask(`Remove ${t.name}? The file is deleted.${t.pack ? ` It came with ${t.pack}.` : ""}`, { ok: "Remove", danger: true }))) return;
    stop();
    track({ action: "remove", pool, name: t.name, off: !t.on }, `Removing ${t.name}…`, "Removed");
  };

  const add = (pool: string) => once(key(`add:${pool}`), async () => {
    const r = await postJson<Reply>("/api/sounds/add", { pool }).catch(failed);
    if (r.cancelled) return;
    if (r.error) return toast(r.error, true);
    const n = r.added?.length ?? 0;
    if (n) {
      toast(`Added ${n} track${n === 1 ? "" : "s"}. They play from the next map.`);
      setChanged(true);
    }
    for (const note of r.notes ?? []) toast(note, true);
    load();
  }, "Adding tracks…");

  const setAddon = (id: "roundmusic" | "quakesounds", on: boolean, name: string) => once(key(`addon:${id}`), async () => {
    const r = await postJson<Reply>("/api/sounds/addon", { id, on }).catch(failed);
    if (r.error) return toast(r.error, true);
    toast(r.how === "live" ? `${name} ${on ? "on" : "off"}. It changes after a map restart.` : `${name} ${on ? "on" : "off"}. It changes the next time a map starts.`);
    if (r.how === "live") setChanged(true);
    load();
  }, `Turning ${name.toLowerCase()} ${on ? "on" : "off"}…`);

  const setAnnouncer = (which: string) => once(key("announcer"), async () => {
    const r = await postJson<Reply>("/api/sounds/announcer", { which }).catch(failed);
    if (r.error) return toast(r.error, true);
    toast("Announcer changed. It's heard from the next map.");
    load();
  }, "Changing the announcer…");

  const installMusic = () => once(key("install-music"), async () => {
    if (!d) return;
    const r = await postJson<Reply>("/api/setup/install", { hl: d.hl, games: [d.game], components: ["addon-roundmusic"] }).catch(failed);
    if (r.error) return toast(r.error, true);
    const j = await waitForSetupJob();
    if (!j) toast("Lost track of the install. Check Mods to see if it finished.", true);
    else toast(j.state === "done" ? "Round-end music is installed. It starts on the next map." : j.error || "The install stopped.", j.state === "error");
    load();
  }, "Installing round-end music…");

  const packAction = async (p: Pack, action: "install" | "remove") => {
    if (isBusy(key(`pack:${p.id}`))) return;
    if (action === "remove") {
      const what = p.kind === "music" ? "Its tracks are deleted." : "The game's original sounds are put back.";
      if (!(await ask(`Remove ${p.name}? ${what}`, { ok: "Remove", danger: true }))) return;
    }
    await once(key(`pack:${p.id}`), () => packStep(p, action), `${action === "remove" ? "Removing" : "Installing"} ${p.name}…`);
  };
  const packStep = async (p: Pack, action: "install" | "remove") => {
    const r = await postJson<Reply>("/api/sounds/pack", { id: p.id, action }).catch(failed);
    if (r.error) return toast(r.error, true);
    if (action === "remove") {
      toast(`Removed ${p.name}`);
      if (p.kind === "music") setChanged(true);
    }
    load();
  };

  if (!d || d.error)
    return (
      <div className="stack sounds">
        <SectionHead title="Sounds" />
        {d?.error ? <LoadError text={d.error} onRetry={() => { setD(null); load(); }} /> : <Skeleton />}
      </div>
    );
  const m = d.music;
  const where = d.dedicated ? "this server" : "your game";
  const now = setupJob?.steps.find((s) => s.state === "running");
  const count = m.pools.reduce((n, p) => n + p.on, 0);
  // setting up happens on Mods: a button for your own game, words for a server (its own Mods tab)
  const toMods = !ON_THIS_COMPUTER
    ? <p className="muted">{SETUP_ON_COMPUTER}</p>
    : target === "local"
      ? <div className="row"><button className="btn" onClick={() => go("mods")}>Run full setup in Mods</button></div>
      : null;

  return (
    <div className="stack sounds">
      <SectionHead title="Sounds" note={`Round-end music, Quake sounds and sound packs on ${where}. Players download custom sounds the first time they join.`} />

      {changed && (live ? (
        <Banner action={<BusyButton className="btn primary" busy={isBusy(key("restartmap"))} label="Restart map" busyLabel="Restarting…" onClick={() => once(key("restartmap"), () => run("gm_restartmap").then((ok) => {
          if (!ok) return;
          setChanged(false);
          setRestarted(true);
          if (target !== "local") reloadServers();
        }), "Restarting the map…")} />}>
          Sound changes start on the next map.
        </Banner>
      ) : (
        <Banner>Changes apply when the game next starts.</Banner>
      ))}

      <div className="chips admin-tabs" role="tablist" aria-label="Sounds sections">
        {SOUND_TABS.map(([id, label]) => (
          <button key={id} role="tab" className="chip" aria-selected={tab === id} onClick={() => setTab(id)}>{label}</button>
        ))}
      </div>

      {/* ---- round-end music ---- */}
      {tab === "music" && (
      <section className="stack-tight">
        <div className="row between sounds-head">
          <div>
            <h3>Round-end music</h3>
            <p className="muted measure">A short track for the winning team at the end of each round. Anyone can type /music in chat to turn it off for themselves.</p>
          </div>
          {m.installed && (
            <label className="sounds-switch">
              <span>{m.on ? "On" : "Off"}</span>
              <Switch on={m.on} label="Round-end music" disabled={isBusy(key("addon:roundmusic"))} onChange={(v) => setAddon("roundmusic", v, "Round-end music")} />
            </label>
          )}
        </div>

        {installing && <SteamProgress value={setupJob?.progress ?? 0} title="Installing round-end music…" text={now ? `${now.label}${now.detail ? ` · ${now.detail}` : ""}` : "Getting ready…"} />}

        {!m.installed && !installing && (
          <div className="panel">
            <h3>Round-end music isn't installed on {where}</h3>
            {d.setup ? (
              <>
                <p className="muted measure">It comes with eight free tracks to start with, and you can add your own.</p>
                {ON_THIS_COMPUTER ? (
                  <div className="row"><BusyButton className="btn primary" busy={installing} label="Install round-end music" busyLabel="Installing…" onClick={installMusic} /></div>
                ) : (
                  <p className="muted">{SETUP_ON_COMPUTER}</p>
                )}
              </>
            ) : (
              <>
                <p className="muted measure">Set up Loner-Strike on {where} first{target === "local" ? "" : ", from this server's Mods tab"}. Then round-end music can be added.</p>
                {toMods}
              </>
            )}
          </div>
        )}

        {m.installed && !m.on && <p className="muted">The add-on is off, so none of these play. Turn it on above.</p>}

        {(m.installed || count > 0) && (
          <>
            <p className={m.total > m.warn_total ? "err-text" : "muted"}>
              {count} track{count === 1 ? "" : "s"} on, {size(m.total)} in all. {m.total > m.warn_total
                ? "That's a lot to download: friends wait longer when they join. Turn some off or use smaller files."
                : `Friends download them when they join, so mp3s under ${size(m.good)} each keep that quick.`}
              {m.fallback ? ` Condition Zero also plays ${m.fallback} track${m.fallback === 1 ? "" : "s"} from Counter-Strike 1.6's folder.` : ""}
            </p>
            {!ON_THIS_COMPUTER && <p className="muted">Adding and removing tracks works on the computer running Loner-Strike. Here you can turn them on or off and move them.</p>}
            <div className="sound-pools">
              {m.pools.map((p) => (
                <div className="panel sound-pool" key={p.id}>
                  <div className="row between">
                    <div>
                      <h3 className={`pool-${p.id}`}>{p.label}</h3>
                      <p className="muted small-note">{POOL_NOTE[p.id]}</p>
                    </div>
                    {ON_THIS_COMPUTER && <BusyButton busy={isBusy(key(`add:${p.id}`))} label="Add tracks" busyLabel="Adding…" onClick={() => add(p.id)} />}
                  </div>
                  {p.over && <p className="err-text small-note">{p.on} tracks are on; the game uses a random {m.limit} of them each map.</p>}
                  {p.tracks.length === 0 ? (
                    <p className="muted small-note">No tracks yet.</p>
                  ) : (
                    <ul className="track-list">
                      {p.tracks.map((t) => {
                        const key = `${p.id}/${t.on ? "" : "off/"}${t.name}`;
                        return (
                          <li key={key} className={t.on ? "" : "off"}>
                            <button className={`track-play${playing === key ? " on" : ""}`} aria-label={playing === key ? `Stop ${t.name}` : `Play ${t.name}`} onClick={() => play(p.id, t)}>
                              {playing === key ? (
                                <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><rect x="3" y="3" width="10" height="10" fill="currentColor" /></svg>
                              ) : (
                                <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><path d="M4 2.5v11l9-5.5z" fill="currentColor" /></svg>
                              )}
                            </button>
                            <div className="track-info">
                              <span className="track-name">{t.name}</span>
                              <small className={t.size > m.good ? "warn" : ""}>
                                {size(t.size)} · {t.kind}{t.pack ? ` · ${t.pack}` : ""}{t.on ? "" : " · off"}
                              </small>
                              {t.problem && <small className="err-text">{t.problem}</small>}
                            </div>
                            <div className="track-actions">
                              <select
                                aria-label={`Move ${t.name}`}
                                value=""
                                disabled={busy}
                                onChange={(e) => e.target.value && track({ action: "move", pool: p.id, name: t.name, off: !t.on, to: e.target.value }, `Moving ${t.name}…`, "Moved")}
                              >
                                <option value="">Move to</option>
                                {m.pools.filter((o) => o.id !== p.id).map((o) => <option key={o.id} value={o.id}>{o.label}</option>)}
                              </select>
                              <Switch on={t.on} label={`${t.name} ${t.on ? "on" : "off"}`} disabled={busy} onChange={(v) => track({ action: v ? "on" : "off", pool: p.id, name: t.name, off: !t.on }, `Turning ${t.name} ${v ? "on" : "off"}…`)} />
                              {ON_THIS_COMPUTER && <BusyButton className="btn danger" busy={isBusy(`sounds:${target}:track:${key}`)} disabled={busy} label="Remove" busyLabel="Removing…" onClick={() => remove(p.id, t)} />}
                            </div>
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </div>
              ))}
            </div>
            <p className="muted small-note measure">mp3 works best. A wav has to be mono, 8 or 16-bit, at 11025 or 22050 Hz. Tracks up to {size(m.max)} are accepted.</p>
          </>
        )}
      </section>
      )}

      {/* ---- quake sounds ---- */}
      {tab === "quake" && (
      <section className="panel">
        <div className="row between sounds-head">
          <div>
            <h3>Quake sounds</h3>
            <p className="muted measure">
              The announcer from the old pub servers: First blood, Headshot, Double kill, Killing spree, Monster kill, Godlike and more.
              {d.quake.installed ? " Changing it applies from the next map." : ` They come with Loner-Strike's basics; set it up${target === "local" ? "" : " from this server's Mods tab"} to get them.`}
            </p>
            {!d.quake.installed && toMods}
          </div>
          {d.quake.installed && (
            <label className="sounds-switch">
              <span>{d.quake.on ? "On" : "Off"}</span>
              <Switch on={d.quake.on} label="Quake sounds" disabled={isBusy(key("addon:quakesounds"))} onChange={(v) => setAddon("quakesounds", v, "Quake sounds")} />
            </label>
          )}
        </div>
        {d.quake.installed && (
          <div className="setting">
            <span className="name">Announcer<span className="note">The classic takes, or another recording of each line (both from Advanced Quake Sounds)</span></span>
            <Segmented values="classic alt" labels="Classic|Alternate takes" current={d.announcer ?? "classic"} label="Announcer" disabled={isBusy(key("announcer"))} onPick={setAnnouncer} />
          </div>
        )}
      </section>
      )}

      {/* ---- sound packs ---- */}
      {tab === "packs" && (
      <section>
        <SectionHead
          title="Classic sound packs"
          note={`A few picks from GameBanana's Counter-Strike 1.6 sounds.${d.dedicated ? " Packs that replace game sounds change what you hear, so they go into your own game; music packs go onto this server." : ""} Anything replaced is backed up, and removing a pack puts it back.`}
        />
        {!ON_THIS_COMPUTER && <p className="muted">Install and remove packs on the computer running Loner-Strike.</p>}
        {!packs && packsFailed ? (
          <LoadError text="Couldn't load the sound packs." onRetry={load} />
        ) : !packs ? (
          <Skeleton rows={2} />
        ) : (
          <div className="sound-packs">
            {packs.packs.map((p) => {
              const job = packs.jobs.find((j) => j.mod === p.id && busyState(j.state));
              return (
                <div className="panel sound-pack" key={p.id}>
                  <div className="row">
                    <span className="tag">{p.kind === "music" ? "Round-end music" : "Replaces game sounds"}</span>
                    {p.installed && <span className="tag ok">Installed</span>}
                  </div>
                  <h3>{p.name}</h3>
                  <p className="muted small-note">by {p.author}</p>
                  <p className="pack-about">{p.about}</p>
                  {p.note && !p.installed && <p className="err-text small-note">{p.note}</p>}
                  {p.kind === "music" && !m.installed && !p.installed && <p className="muted small-note">Needs the round-end music add-on to play.</p>}
                  <div className="row pack-actions">
                    {job ? (
                      <span className="muted">{job.message}{job.size ? ` ${Math.round((job.got / job.size) * 100)}%` : ""}</span>
                    ) : p.installed ? (
                      ON_THIS_COMPUTER && <BusyButton className="btn danger" busy={isBusy(key(`pack:${p.id}`))} label="Remove" busyLabel="Removing…" onClick={() => packAction(p, "remove")} />
                    ) : (
                      ON_THIS_COMPUTER && <BusyButton className="btn primary" busy={isBusy(key(`pack:${p.id}`))} label="Install" busyLabel="Installing…" onClick={() => packAction(p, "install")} />
                    )}
                    <a className="btn" href={p.url} target="_blank" rel="noreferrer">Open on GameBanana</a>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>
      )}
    </div>
  );
}
