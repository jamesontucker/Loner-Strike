// A Studio mode's settings (variables its maker marked "hosts can change it"): shown with the
// mode on the Play page and on a server's Modes tab. Each server keeps its own values; the mode
// reads them as a map starts and at each new round. Nothing here edits or compiles the mode.

import { useEffect, useState } from "react";
import { getJsonAt, postJsonAt } from "../api";
import { Info, useToast } from "../ui";

type Setting = { cvar: string; label: string; help?: string; group?: string; type: "number" | "bool" | "choice"; min: number; max: number; step?: number; choices?: string[]; default: number; value: number };
type Reply = { settings?: Setting[]; error?: string };

export function ModeSettings({ mode, server, live }: { mode: string; server?: string | null; live?: boolean }) {
  const toast = useToast();
  const [list, setList] = useState<Setting[] | null>(null);
  const [saving, setSaving] = useState<string | null>(null);
  const q = `?mode=${encodeURIComponent(mode)}${server ? `&server=${encodeURIComponent(server)}` : ""}`;
  useEffect(() => {
    let gone = false;
    setList(null);
    getJsonAt<Reply>(`/api/tools/studio/mode-settings${q}`, null)
      .then((r) => !gone && setList(r.settings ?? []))
      .catch(() => !gone && setList([]));
    return () => {
      gone = true;
    };
  }, [q]);
  if (!list || list.length === 0) return null;
  const save = async (s: Setting, value: number) => {
    setSaving(s.cvar);
    try {
      const r = await postJsonAt<Reply>("/api/tools/studio/mode-settings", { mode, server: server ?? undefined, values: { [s.cvar]: value } }, null);
      if (r.error) toast(r.error, true);
      else if (r.settings) setList(r.settings);
    } catch {
      toast("Couldn't save that setting.", true);
    } finally {
      setSaving(null);
    }
  };
  const groups = [...new Set(list.map((s) => s.group ?? ""))];
  return (
    <section className="mode-settings" aria-label="Mode settings" data-testid="mode-settings">
      <h3>
        Mode settings
        <Info text={`Set by the mode's maker for hosts to change. This ${server ? "server" : "computer"} keeps its own values; the mode uses them from the next map${live ? " or the next round" : ""}. Admins also find them in the game's mode menu (options).`} />
      </h3>
      {groups.map((g) => (
        <div key={g} className="mode-settings-group">
          {g && <h4>{g}</h4>}
          {list
            .filter((s) => (s.group ?? "") === g)
            .map((s) => (
              <label key={s.cvar} className="mode-setting" title={s.help || undefined}>
                <span className="mode-setting-label">
                  {s.label}
                  {saving === s.cvar && (
                    <span className="muted small-note">
                      {" "}
                      <span className="spinner" aria-hidden="true" /> Saving…
                    </span>
                  )}
                </span>
                {s.type === "bool" ? (
                  <input type="checkbox" checked={s.value !== 0} disabled={saving === s.cvar} onChange={(e) => void save(s, e.target.checked ? 1 : 0)} aria-label={s.label} />
                ) : s.type === "choice" ? (
                  <select value={s.value} disabled={saving === s.cvar} onChange={(e) => void save(s, Number(e.target.value))} aria-label={s.label}>
                    {(s.choices ?? []).map((c, i) => (
                      <option key={c} value={i}>
                        {c}
                      </option>
                    ))}
                  </select>
                ) : (
                  <NumberBox s={s} disabled={saving === s.cvar} onSave={(v) => void save(s, v)} />
                )}
                {s.help && <span className="muted small-note">{s.help}</span>}
              </label>
            ))}
        </div>
      ))}
    </section>
  );
}

/** A number saved when it's left (or Enter), kept in its range. */
function NumberBox({ s, disabled, onSave }: { s: Setting; disabled: boolean; onSave: (v: number) => void }) {
  const [text, setText] = useState(String(s.value));
  useEffect(() => setText(String(s.value)), [s.value]);
  const commit = () => {
    const v = Number(text);
    if (!Number.isFinite(v)) return setText(String(s.value));
    const c = Math.min(s.max, Math.max(s.min, v));
    setText(String(c));
    if (c !== s.value) onSave(c);
  };
  return (
    <input
      type="number"
      min={s.min}
      max={s.max}
      step={s.step && s.step > 0 ? s.step : "any"}
      value={text}
      disabled={disabled}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && commit()}
      aria-label={s.label}
    />
  );
}
