import { useEffect, useId, useMemo, useRef, useState } from "react";
import { postJson } from "../api";

/** One of the server's commands or settings (consolehints.rs). */
type Hint = { n: string; k: "cmd" | "cvar"; a?: string; d?: string; s?: string };
type HintsReply = { ok?: boolean; error?: string; items?: Hint[]; value?: string | null };

/** How many suggestions are shown at once (the rest by typing more) */
const MAX_SHOWN = 60;
const HISTORY_MAX = 50;

const historyKey = (id: string) => `ls-console-history:${id}`;
function loadHistory(id: string): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(historyKey(id)) ?? "[]");
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string").slice(-HISTORY_MAX) : [];
  } catch {
    return [];
  }
}
function saveHistory(id: string, h: string[]) {
  try {
    localStorage.setItem(historyKey(id), JSON.stringify(h.slice(-HISTORY_MAX)));
  } catch {
    // private window or storage off: history just isn't kept
  }
}

/** What's offered for the text typed so far: names that start with it first, then ones
 *  that contain it. After a space only subcommands (meta list, amxx plugins) are offered. */
export function suggest(items: Hint[], typed: string): Hint[] {
  const q = typed.replace(/^\s+/, "").toLowerCase();
  if (!q) return [];
  if (q.includes(" ")) return items.filter((i) => i.n.includes(" ") && i.n.toLowerCase().startsWith(q) && i.n.length > q.length);
  const starts: Hint[] = [];
  const has: Hint[] = [];
  for (const i of items) {
    const n = i.n.toLowerCase();
    if (n === q && !items.some((o) => o !== i && o.n.toLowerCase().startsWith(q + " "))) continue; // typed in full already
    if (n.startsWith(q)) starts.push(i);
    else if (q.length >= 2 && n.includes(q)) has.push(i);
  }
  return [...starts, ...has];
}

/** The command or setting the typed text is using, to show what it takes. */
export function typedItem(items: Hint[], typed: string): Hint | undefined {
  const words = typed.trim().toLowerCase().split(/\s+/);
  if (!words[0]) return undefined;
  const two = words.length > 1 ? `${words[0]} ${words[1]}` : "";
  return items.find((i) => two && i.n.toLowerCase() === two) ?? items.find((i) => i.n.toLowerCase() === words[0]);
}

/**
 * The console's command box: suggestions from the running server's own commands and
 * settings as you type (Tab or Enter on a highlighted one fills it in), what the command
 * takes and a setting's current value, and Up/Down through what was sent before.
 */
export function ConsoleInput({ serverId, running, value, onChange }: { serverId: string; running: boolean; value: string; onChange: (v: string) => void }) {
  const [items, setItems] = useState<Hint[]>([]);
  const [active, setActive] = useState(-1);
  const [dismissed, setDismissed] = useState(false);
  const [focused, setFocused] = useState(false);
  const [history, setHistory] = useState<string[]>(() => loadHistory(serverId));
  // where Up/Down is in the history (null: not browsing it), and what was typed before
  const [histPos, setHistPos] = useState<number | null>(null);
  const draft = useRef("");
  const [values, setValues] = useState<Record<string, string | null>>({});
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const uid = useId();
  const listId = `console-suggest-${uid}`;
  const optId = (i: number) => `${listId}-${i}`;

  useEffect(() => {
    setHistory(loadHistory(serverId));
    setHistPos(null);
    setValues({});
  }, [serverId]);

  // the server's list, each time it's running. A server that just started doesn't answer
  // until its map is up (and it has tried Steam), so it's asked again every few seconds for
  // a while, and again whenever the box is clicked into without a list
  const [attempt, setAttempt] = useState(0);
  const loaded = items.length > 0;
  useEffect(() => {
    if (!running) return setItems([]);
    if (loaded) return;
    let stop = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = (tries: number) =>
      postJson<HintsReply>(`/api/servers/${serverId}/console/hints`, {})
        .then((r) => {
          if (stop) return;
          if (r.items?.length) setItems(r.items);
          else if (tries > 0) timer = setTimeout(() => load(tries - 1), 4000);
        })
        .catch(() => {
          if (!stop && tries > 0) timer = setTimeout(() => load(tries - 1), 4000);
        });
    load(45);
    return () => {
      stop = true;
      clearTimeout(timer);
    };
  }, [serverId, running, loaded, attempt]);

  const shown = useMemo(() => suggest(items, value).slice(0, MAX_SHOWN), [items, value]);
  const open = focused && !dismissed && histPos === null && shown.length > 0;
  const focusItem = open && active >= 0 ? shown[active] : typedItem(items, value);

  // a setting's current value, asked for once it's been on screen a moment
  const wanted = focusItem?.k === "cvar" ? focusItem.n : null;
  useEffect(() => {
    if (!wanted || !running || wanted in values) return;
    const t = setTimeout(() => {
      postJson<HintsReply>(`/api/servers/${serverId}/console/hints`, { value: wanted })
        .then((r) => setValues((v) => ({ ...v, [wanted]: r.ok ? (r.value ?? null) : null })))
        .catch(() => {});
    }, 250);
    return () => clearTimeout(t);
  }, [wanted, running, serverId, values]);

  useEffect(() => {
    if (active >= 0) document.getElementById(optId(active))?.scrollIntoView({ block: "nearest" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  const change = (v: string) => {
    onChange(v);
    setActive(-1);
    setDismissed(false);
    setHistPos(null);
  };
  const fill = (h: Hint) => {
    change(h.n + " ");
    input.current?.focus();
  };
  // the form is sent (Enter, or its Send button): kept for Up/Down
  const remember = () => {
    const c = value.trim();
    if (c) {
      const next = [...history.filter((x) => x !== c), c].slice(-HISTORY_MAX);
      setHistory(next);
      saveHistory(serverId, next);
      // what it changed is asked for again
      const name = c.split(/\s+/)[0].toLowerCase();
      setValues((v) => Object.fromEntries(Object.entries(v).filter(([k]) => k.toLowerCase() !== name)));
    }
    setHistPos(null);
    setActive(-1);
  };
  const rememberRef = useRef(remember);
  rememberRef.current = remember;
  useEffect(() => {
    const form = input.current?.form;
    if (!form) return;
    const on = () => rememberRef.current();
    form.addEventListener("submit", on);
    return () => form.removeEventListener("submit", on);
  }, []);
  const recall = (pos: number | null) => {
    setHistPos(pos);
    onChange(pos === null ? draft.current : history[pos]);
    setActive(-1);
  };

  const key = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const up = e.key === "ArrowUp";
      // the history when nothing's typed (or already in it), like the game's console
      if (histPos !== null || (!value.trim() && history.length)) {
        e.preventDefault();
        if (histPos === null) {
          if (!up) return;
          draft.current = value;
          return recall(history.length - 1);
        }
        const next = histPos + (up ? -1 : 1);
        return recall(next < 0 ? 0 : next >= history.length ? null : next);
      }
      if (shown.length) {
        e.preventDefault();
        setDismissed(false);
        setActive((a) => (up ? (a <= 0 ? shown.length - 1 : a - 1) : a >= shown.length - 1 ? 0 : a + 1));
      }
      return;
    }
    if (e.key === "Tab" && open && !e.shiftKey) {
      e.preventDefault();
      return fill(shown[Math.max(0, active)]);
    }
    // Enter on a highlighted suggestion fills it in; otherwise the form is sent
    if (e.key === "Enter" && open && active >= 0) {
      e.preventDefault();
      return fill(shown[active]);
    }
    if (e.key === "Escape" && (open || histPos !== null)) {
      e.preventDefault();
      if (histPos !== null) return recall(null);
      setDismissed(true);
      setActive(-1);
    }
  };

  const val = focusItem && focusItem.k === "cvar" ? values[focusItem.n] : undefined;
  return (
    <div className="console-input">
      <input
        ref={input}
        className="search grow"
        placeholder="A server command, like status or changelevel de_inferno"
        value={value}
        onChange={(e) => change(e.target.value)}
        onKeyDown={key}
        onFocus={() => {
          setFocused(true);
          if (!loaded && running) setAttempt((a) => a + 1);
        }}
        onBlur={(e) => {
          if (!list.current?.contains(e.relatedTarget as Node)) setFocused(false);
        }}
        spellCheck={false}
        autoComplete="off"
        autoCapitalize="off"
        role="combobox"
        aria-label="Server command"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={open && active >= 0 ? optId(active) : undefined}
        aria-describedby={focusItem ? `${listId}-hint` : undefined}
      />
      {open && (
        <div className="console-suggest" id={listId} role="listbox" aria-label="Suggestions" ref={list}>
          {shown.map((h, i) => (
            <button
              key={h.n}
              id={optId(i)}
              type="button"
              role="option"
              tabIndex={-1}
              aria-selected={i === active}
              className={i === active ? "active" : ""}
              onMouseDown={(e) => e.preventDefault()}
              onMouseEnter={() => setActive(i)}
              onClick={() => fill(h)}
            >
              <span className="cs-name">{h.n}</span>
              <span className="cs-what">{h.a ?? ""}{h.a && h.d ? "  " : ""}{h.d ? <span className="muted">{h.d}</span> : null}</span>
              <span className="cs-from">{h.s ? h.s.replace(/\.amxx$/, "") : h.k === "cvar" ? "setting" : ""}</span>
            </button>
          ))}
        </div>
      )}
      {focusItem && (
        <p className="console-hint small-note" id={`${listId}-hint`} aria-live="polite">
          <code>{focusItem.n}{focusItem.a ? ` ${focusItem.a}` : ""}</code>
          {focusItem.d && <span className="muted"> · {focusItem.d}</span>}
          {val !== undefined && val !== null && <span> · now <code>{val === "" ? '""' : val}</code></span>}
          {focusItem.s && <span className="muted"> · {focusItem.s}</span>}
        </p>
      )}
    </div>
  );
}
