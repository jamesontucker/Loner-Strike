import { useEffect, useMemo, useState } from "react";
import { getJson, postJson } from "../api";
import { SectionHead, useToast } from "../ui";

type Data = { style: "builtin" | "custom" | "url"; note: string; html: string | null; url: string | null; max: number; error?: string };

const STYLES = [
  { id: "builtin", name: "Built-in page", blurb: "The server, mode, map and commands, plus your own message" },
  { id: "custom", name: "Your own page", blurb: "Write it in HTML, with the live details filled in" },
  { id: "url", name: "A web page", blurb: "The game's browser opens a page you host (rules, Discord…)" },
] as const;

const PLACEHOLDERS: [string, string][] = [
  ["{server}", "the server's name"],
  ["{player}", "the player's name"],
  ["{map}", "the map"],
  ["{mode}", "the game mode"],
  ["{players}", "how many are playing"],
];

const STARTER = `<html><head><style>
body{background:#3e4637;color:#dedfd6;font:15px Verdana,sans-serif;padding:20px}
h1{color:#c4b550;margin:0 0 6px}
</style></head><body>
<h1>{server}</h1>
<p>Welcome, {player}! Now playing {mode} on {map}.</p>
<ul>
<li>No cheating, no spawn camping</li>
<li>Say <b>rtv</b> to vote for a new map</li>
</ul>
</body></html>`;

const SAMPLE: Record<string, string> = { "{server}": "Friday night frags", "{player}": "Loner", "{map}": "de_dust2", "{mode}": "Classic", "{players}": "9" };

/** The page players see when they join (the game's MOTD window), for the game or server being managed. */
export function Welcome({ target }: { target: string }) {
  const toast = useToast();
  const [d, setD] = useState<Data | null>(null);
  const [style, setStyle] = useState<Data["style"]>("builtin");
  const [note, setNote] = useState("");
  const [html, setHtml] = useState("");
  const [url, setUrl] = useState("");

  useEffect(() => {
    getJson<Data>("/api/welcome")
      .then((r) => {
        setD(r);
        setStyle(r.style);
        setNote(r.note);
        setHtml(r.html ?? "");
        setUrl(r.url ?? "");
      })
      .catch(() => null);
  }, [target]);

  const preview = useMemo(() => Object.entries(SAMPLE).reduce((h, [k, v]) => h.split(k).join(v), html), [html]);
  if (!d) return <p className="muted">Loading…</p>;
  if (d.error) return <p className="muted">{d.error}</p>;
  const tooLong = html.length > d.max;

  const save = async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/welcome", { style, note, html, url }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    toast("Saved. Players see it next time they join.");
  };

  return (
    <div className="stack">
      <SectionHead title="Welcome page" note="What players see when they join. Turn it on or off, or show it every map, under Match → Welcome screen. Players can say /welcome to see it again." />
      <div className="methods" role="radiogroup" aria-label="Welcome page">
        {STYLES.map((s) => (
          <button key={s.id} className="method" role="radio" aria-checked={style === s.id} onClick={() => setStyle(s.id)}>
            <span className="method-name">{s.name}</span>
            <span className="method-blurb">{s.blurb}</span>
          </button>
        ))}
      </div>

      {style === "builtin" && (
        <div className="panel">
          <label className="field">
            <span>Your message</span>
            <textarea className="invite short" maxLength={300} placeholder="Play fair, have fun. gl hf!" value={note} onChange={(e) => setNote(e.target.value)} />
            <small>Shown under the commands: your rules, a Discord invite, a thank-you to supporters. {300 - note.length} characters left.</small>
          </label>
        </div>
      )}

      {style === "custom" && (
        <div className="welcome-edit">
          <div className="panel">
            <div className="row between">
              <h3>Page</h3>
              {!html && <button className="btn small" onClick={() => setHtml(STARTER)}>Start from an example</button>}
            </div>
            <textarea className="invite code" spellCheck={false} value={html} onChange={(e) => setHtml(e.target.value)} placeholder="<h1>{server}</h1>" />
            <p className={tooLong ? "err-text" : "muted"}>
              {html.length} of {d.max} characters{tooLong ? ". The game cuts off longer pages, so trim it, or host it as a web page instead." : "."}
            </p>
            <div className="chips">
              {PLACEHOLDERS.map(([k, label]) => (
                <button key={k} className="chip" title={`Fills in ${label}`} onClick={() => setHtml((h) => h + k)}>{k}</button>
              ))}
            </div>
          </div>
          <div className="panel">
            <h3>Preview</h3>
            <iframe className="motd-preview" title="Welcome page preview" sandbox="" srcDoc={preview} />
            <p className="muted">With example details. The game's window is about 600 by 400.</p>
          </div>
        </div>
      )}

      {style === "url" && (
        <div className="panel">
          <label className="field">
            <span>Web address</span>
            <input className="search" type="url" placeholder="https://example.com/welcome" value={url} onChange={(e) => setUrl(e.target.value)} spellCheck={false} />
            <small>The game's browser is old, so keep the page simple. Pages on GitHub Pages, Carrd or your own site work well.</small>
          </label>
        </div>
      )}

      <div className="row">
        <button className="btn primary" disabled={style === "custom" && tooLong} onClick={save}>Save</button>
      </div>
    </div>
  );
}
