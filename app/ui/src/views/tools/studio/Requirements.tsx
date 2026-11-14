// MCP: a mode's requirements (My mode › Details › Requirements): the default and recommended maps,
// the models, sounds and sprites the graph uses with the community or GameBanana item that
// provides each, and what's known already (components, creature types, the bot brain).
// Server: app/src-tauri/src/tools/requirements.rs (kept beside the graph, not in it). X mounts
// <Requirements id={...} /> in the Details panel (docs/notes/X.md "Requirements").

import { useEffect, useState } from "react";
import { get, post } from "../common/api";
import { BusyButton } from "../../../ui";

type Link = { model: "Mod" | "Spray"; id: number; name: string };
type MapReq = { name: string; source: "base" | "community" | "local"; key?: string; have: boolean; warning?: string };
type ContentReq = { path: string; kind: "model" | "sound" | "sprite"; have: boolean; link: Link | null; suggest: Link | null };
export type Report = {
  id: string;
  defaultMap: MapReq | null;
  maps: MapReq[];
  content: ContentReq[];
  known: { components: { classname: string }[] | null; creatures: string[]; brain: boolean };
  warnings: string[];
  missing: string[];
  installedMaps: MapReq[];
  error?: string;
};

const SOURCE: Record<MapReq["source"], string> = { base: "Base game", community: "Community", local: "Only on this computer" };

/** "https://gamebanana.com/mods/12345" or "12345" as a page link. */
export function parseLink(text: string): Link | null {
  const t = text.trim();
  const m = /gamebanana\.com\/(mods|sprays)\/(\d+)/i.exec(t) ?? (/^(\d+)$/.exec(t) ? [t, "mods", t] : null);
  if (!m) return null;
  const id = Number(m[2]);
  return id > 0 && id < 1e8 ? { model: m[1].toLowerCase() === "sprays" ? "Spray" : "Mod", id, name: "" } : null;
}

/** Base-game maps first, then community ones, then the rest; by name inside each. */
const sortMaps = (l: MapReq[]) => [...l].sort((a, b) => ["base", "community", "local"].indexOf(a.source) - ["base", "community", "local"].indexOf(b.source) || a.name.localeCompare(b.name));

/** Recommended maps: the picked ones as chips, and a search to add more (UX review H3). */
export function MapPicker({ maps, picked, disabled, onChange }: { maps: MapReq[]; picked: MapReq[]; disabled: boolean; onChange: (names: string[]) => void }) {
  const [q, setQ] = useState("");
  const names = picked.map((m) => m.name);
  const found = q.trim() ? sortMaps(maps).filter((m) => !names.includes(m.name) && m.name.toLowerCase().includes(q.trim().toLowerCase())).slice(0, 8) : [];
  return (
    <div className="st-req-picker">
      {picked.length > 0 ? (
        <ul className="st-req-chips" aria-label="Recommended maps">
          {picked.map((m) => (
            <li key={m.name} className="st-req-chip" title={m.warning ?? SOURCE[m.source]}>
              {m.name}
              {m.warning && <span className="warn"> !</span>}
              <button type="button" aria-label={`Remove ${m.name}`} disabled={disabled} onClick={() => onChange(names.filter((x) => x !== m.name))}>
                ×
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted small-note">None yet.</p>
      )}
      <input className="st-field" type="search" placeholder={`Add a map (${maps.length} on this computer)`} value={q} onChange={(e) => setQ(e.target.value)} aria-label="Add a recommended map" disabled={disabled} />
      {found.length > 0 && (
        <ul className="st-req-found" role="listbox" aria-label="Maps found">
          {found.map((m) => (
            <li key={m.name}>
              <button
                type="button"
                role="option"
                aria-selected={false}
                onClick={() => {
                  onChange([...names, m.name]);
                  setQ("");
                }}
              >
                {m.name} <span className="muted">· {SOURCE[m.source]}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {q.trim() && found.length === 0 && <p className="muted small-note">No map matches that.</p>}
    </div>
  );
}

export function Requirements({ id }: { id: string }) {
  const [r, setR] = useState<Report | null>(null);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [typed, setTyped] = useState<Record<string, string>>({});
  useEffect(() => {
    let gone = false;
    get<Report>(`/api/tools/requirements/${encodeURIComponent(id)}`)
      .then((v) => !gone && (v.error ? setError(v.error) : setR(v)))
      .catch(() => !gone && setError("Couldn't read the requirements. Save the mode first."));
    return () => {
      gone = true;
    };
  }, [id]);
  const save = async (change: { defaultMap?: string; maps?: string[]; links?: Record<string, Link | null> }) => {
    if (!r) return;
    setSaving(true);
    const links: Record<string, Link | null> = {};
    for (const c of r.content) if (c.link) links[c.path] = c.link;
    Object.assign(links, change.links ?? {});
    const body = { defaultMap: change.defaultMap ?? r.defaultMap?.name ?? "", maps: change.maps ?? r.maps.map((m) => m.name), links };
    try {
      const v = await post<Report>(`/api/tools/requirements/${encodeURIComponent(id)}`, body);
      if (v.error) setError(v.error);
      else {
        setR(v);
        setError("");
      }
    } finally {
      setSaving(false);
    }
  };
  if (error && !r) return <p className="err-text small-note">{error}</p>;
  if (!r)
    return (
      <p className="muted small-note">
        <span className="spinner" aria-hidden="true" />
        Reading the mode's requirements…
      </p>
    );
  return (
    <section className="st-requirements" aria-label="Requirements">
      {saving && (
        <p className="muted small-note" aria-live="polite">
          <span className="spinner" aria-hidden="true" />
          Saving…
        </p>
      )}
      {error && <p className="err-text small-note">{error}</p>}
      <h4>Default map</h4>
      <p className="st-side-help">The map a Playtest and the mode's first game start on.</p>
      <select className="st-field" value={r.defaultMap?.name ?? ""} onChange={(e) => void save({ defaultMap: e.target.value })} disabled={saving} aria-label="Default map">
        <option value="">(none)</option>
        {sortMaps(r.installedMaps).map((m) => (
          <option key={m.name} value={m.name}>
            {m.name} · {SOURCE[m.source]}
          </option>
        ))}
      </select>
      {r.defaultMap?.warning && <p className="small-note warn">{r.defaultMap.warning}</p>}
      <h4>Recommended maps</h4>
      <p className="st-side-help">Maps that suit the mode. Players who install it see them, and the app offers to get the ones they don't have.</p>
      <MapPicker maps={r.installedMaps} picked={r.maps} disabled={saving} onChange={(maps) => void save({ maps })} />
      <h4>Content</h4>
      {r.content.length === 0 ? (
        <p className="muted small-note">The mode uses only the game's own models, sounds and sprites.</p>
      ) : (
        <ul className="st-req-content">
          {r.content.map((c) => (
            <li key={c.path}>
              <code>{c.path}</code> <span className="muted">({c.kind}{c.have ? "" : ", not on this computer"})</span>
              <div className="row">
                {c.link ? (
                  <>
                    <span>
                      From {c.link.name || `${c.link.model === "Spray" ? "spray" : "page"} ${c.link.id}`}
                    </span>
                    <button type="button" className="btn small" disabled={saving} onClick={() => void save({ links: { [c.path]: null } })}>
                      Remove link
                    </button>
                  </>
                ) : (
                  <>
                    <input
                      placeholder="GameBanana link or page number"
                      value={typed[c.path] ?? ""}
                      onChange={(e) => setTyped({ ...typed, [c.path]: e.target.value })}
                      aria-label={`The item that provides ${c.path}`}
                    />
                    <BusyButton
                      className="btn small"
                      busy={saving}
                      busyLabel="Saving…"
                      label="Link"
                      disabled={!parseLink(typed[c.path] ?? "")}
                      onClick={() => void save({ links: { [c.path]: parseLink(typed[c.path] ?? "") } })}
                    />
                    {c.suggest && (
                      <button type="button" className="btn small" disabled={saving} onClick={() => void save({ links: { [c.path]: c.suggest } })}>
                        Use {c.suggest.name || "the installed item"}
                      </button>
                    )}
                  </>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
      <h4>Also needs</h4>
      <p className="muted small-note">
        {[
          (r.known.components ?? []).length ? `Components: ${(r.known.components ?? []).map((c) => c.classname).join(", ")}` : "",
          r.known.creatures.length ? `Creatures: ${r.known.creatures.join(", ")}` : "",
          r.known.brain ? "Loner-Strike's bot brain (for its creatures)" : "",
        ]
          .filter(Boolean)
          .join(" · ") || "Nothing else."}
      </p>
      {r.warnings.map((w) => (
        <p key={w} className="small-note warn">
          {w}
        </p>
      ))}
      {r.missing.length > 0 && <p className="err-text small-note">Publish stops until these are linked or on this computer: {r.missing.join(", ")}.</p>}
    </section>
  );
}
