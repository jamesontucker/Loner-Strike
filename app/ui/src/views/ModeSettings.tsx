// A Studio mode's settings (variables its maker marked "hosts can change it"): shown with the
// mode on the Play page and on a server's Modes tab (the request goes to the open server, on
// this computer or on its own machine). Each server keeps its own values; the mode reads them as
// a map starts and at each new round. Hosts are trusted: a value outside the author's range is
// kept, with a warning. Nothing here edits or compiles the mode.

import { useEffect, useState } from "react";
import { getJson, postJson } from "../api";
import { Info, useToast } from "../ui";

type Setting = {
  cvar: string;
  label: string;
  help?: string;
  group?: string;
  type: "number" | "bool" | "choice";
  min: number;
  max: number;
  step?: number;
  choices?: string[];
  default: number;
  value: number;
  changed?: boolean;
  warning?: string;
};
type Reply = { settings?: Setting[]; warnings?: string[]; error?: string };

export function ModeSettings({ mode, live }: { mode: string; live?: boolean }) {
  const toast = useToast();
  const [list, setList] = useState<Setting[] | null>(null);
  const [saving, setSaving] = useState<string | null>(null);
  useEffect(() => {
    let gone = false;
    setList(null);
    getJson<Reply>(`/api/tools/studio/mode-settings?mode=${encodeURIComponent(mode)}`)
      .then((r) => !gone && setList(r.settings ?? []))
      .catch(() => !gone && setList([]));
    return () => {
      gone = true;
    };
  }, [mode]);
  if (!list || list.length === 0) return null;
  const send = async (key: string, body: Record<string, unknown>) => {
    setSaving(key);
    try {
      const r = await postJson<Reply>("/api/tools/studio/mode-settings", { mode, ...body });
      if (r.error) toast(r.error, true);
      else {
        if (r.settings) setList(r.settings);
        for (const w of r.warnings ?? []) toast(w);
      }
    } catch {
      toast("Couldn't save that setting.", true);
    } finally {
      setSaving(null);
    }
  };
  const save = (s: Setting, value: number) => send(s.cvar, { values: { [s.cvar]: value } });
  const changed = list.filter((s) => s.changed).length;
  const groups = [...new Set(list.map((s) => s.group ?? ""))];
  return (
    <section className="mode-settings" aria-label="Mode settings" data-testid="mode-settings">
      <h3>
        Mode settings
        <Info text={`Set by the mode's maker for hosts to change. This ${live ? "server" : "game"} keeps its own values; the mode uses them from the next map${live ? " or the next round" : ""}. Admins also find them in the game's mode menu (options).`} />
        {changed > 0 && (
          <button type="button" className="link mode-settings-reset" disabled={!!saving} onClick={() => void send("all", { reset_all: true })}>
            {saving === "all" ? (
              <>
                <span className="spinner" aria-hidden="true" /> Saving…
              </>
            ) : (
              `Reset all to the mode's defaults (${changed} changed)`
            )}
          </button>
        )}
      </h3>
      {groups.map((g) => (
        <div key={g} className="mode-settings-group">
          {g && <h4>{g}</h4>}
          {list
            .filter((s) => (s.group ?? "") === g)
            .map((s) => (
              <div key={s.cvar} className={`mode-setting${s.changed ? " changed" : ""}`} title={s.help || undefined}>
                <label className="mode-setting-label" htmlFor={`ms-${s.cvar}`}>
                  {s.label}
                  {s.changed && <span className="mode-setting-mark" title={`Not the mode's default (${defaultText(s)})`}> •</span>}
                  {saving === s.cvar && (
                    <span className="muted small-note">
                      {" "}
                      <span className="spinner" aria-hidden="true" /> Saving…
                    </span>
                  )}
                </label>
                <span className="mode-setting-input">
                  {s.type === "bool" ? (
                    <input id={`ms-${s.cvar}`} type="checkbox" checked={s.value !== 0} disabled={!!saving} onChange={(e) => void save(s, e.target.checked ? 1 : 0)} />
                  ) : s.type === "choice" ? (
                    <select id={`ms-${s.cvar}`} value={s.value} disabled={!!saving} onChange={(e) => void save(s, Number(e.target.value))}>
                      {(s.choices ?? []).map((c, i) => (
                        <option key={c} value={i}>
                          {c}
                        </option>
                      ))}
                      {(s.value < 0 || s.value >= (s.choices ?? []).length) && <option value={s.value}>{s.value}</option>}
                    </select>
                  ) : (
                    <NumberBox id={`ms-${s.cvar}`} s={s} disabled={!!saving} onSave={(v) => void save(s, v)} />
                  )}
                  {s.changed && (
                    <button type="button" className="link small" style={{ whiteSpace: "nowrap" }} disabled={!!saving} onClick={() => void send(s.cvar, { reset: [s.cvar] })} title={`Back to ${defaultText(s)}`}>
                      Reset
                    </button>
                  )}
                </span>
                {s.warning && <span className="small-note mode-setting-warn">{s.warning}</span>}
                {s.help && <span className="muted small-note">{s.help}</span>}
              </div>
            ))}
        </div>
      ))}
    </section>
  );
}

function defaultText(s: Setting) {
  if (s.type === "bool") return s.default ? "yes" : "no";
  if (s.type === "choice") return s.choices?.[s.default] ?? String(s.default);
  return String(s.default);
}

/** A number saved when it's left (or Enter); any number (outside the author's range warns). */
function NumberBox({ id, s, disabled, onSave }: { id: string; s: Setting; disabled: boolean; onSave: (v: number) => void }) {
  const [text, setText] = useState(String(s.value));
  useEffect(() => setText(String(s.value)), [s.value]);
  const commit = () => {
    const v = Number(text);
    if (text.trim() === "" || !Number.isFinite(v)) return setText(String(s.value));
    if (v !== s.value) onSave(v);
  };
  const outside = Number(text) < s.min || Number(text) > s.max;
  return (
    <input
      id={id}
      type="number"
      step={s.step && s.step > 0 ? s.step : "any"}
      value={text}
      disabled={disabled}
      aria-invalid={outside || undefined}
      title={`The mode's author suggests ${s.min}-${s.max}`}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && commit()}
    />
  );
}
