// Creature Studio's pickers: the model (search the game's models, see who has each) and the
// sounds table (one row per event: the files as chips with play and remove, "+ Add" opens a
// searchable list, fallbacks shown as a quieter second line).

import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { matches, searchWords } from "../common/search";
import { CreditLine, EmptyImport, ImportBar, ImportZone, ProjectFilesGroup, useAssetImport, useFocusRefresh, type AssetCredit } from "../common/AssetImport";
import { listModels, listSounds, soundUrl, type Checked, type ModelItem, type SoundReport } from "./creaturesApi";
import { KIND_WORDS, SOUND_WORDS } from "./words";

const STAND_IN = "models/hostage.mdl";

/** A small popover under its button; closes on Esc or a click outside. */
function Popover({ open, onClose, children, label }: { open: boolean; onClose: () => void; children: React.ReactNode; label: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const down = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && onClose();
    const key = (e: KeyboardEvent) => e.key === "Escape" && (e.stopPropagation(), onClose());
    document.addEventListener("mousedown", down);
    document.addEventListener("keydown", key, true);
    return () => {
      document.removeEventListener("mousedown", down);
      document.removeEventListener("keydown", key, true);
    };
  }, [open, onClose]);
  // (opens upwards when there isn't room below: the save bar sits at the bottom)
  const [up, setUp] = useState(false);
  useLayoutEffect(() => {
    if (!open || !ref.current) return setUp(false);
    const anchor = ref.current.parentElement?.getBoundingClientRect();
    if (anchor) setUp(anchor.bottom + 360 > window.innerHeight - 70 && anchor.top > 380);
  }, [open]);
  if (!open) return null;
  return (
    <div ref={ref} className={`cs-pop panel${up ? " up" : ""}`} role="dialog" aria-label={label}>
      {children}
    </div>
  );
}

let audio: HTMLAudioElement | null = null;
export function playSound(file: string) {
  audio?.pause();
  audio = new Audio(soundUrl(file));
  void audio.play().catch(() => undefined);
}

export function ModelPicker({ value, onChange, report }: { value: string; onChange: (v: string) => void; report?: Checked["models"] }) {
  const models = useQuery({ queryKey: ["tools", "creatures", "models"], queryFn: ({ signal }) => listModels(signal).then((r) => r.models), staleTime: 60_000 });
  const [q, setQ] = useState("");
  const [open, setOpen] = useState(false);
  const parts = value.split(/\s+/).filter(Boolean);
  const first = parts[0] ?? "";
  const standIn = parts.includes(STAND_IN) && first !== STAND_IN;
  const shown = useMemo(() => {
    const w = searchWords(q);
    return (models.data ?? []).filter((m) => matches(w, m.path, m.group)).slice(0, 150);
  }, [models.data, q]);
  const pick = (m: ModelItem) => {
    onChange([m.path, ...parts.slice(1).filter((p) => p !== m.path)].join(" "));
    setOpen(false);
    setQ("");
  };
  const setStandIn = (on: boolean) => onChange((on ? [first, ...parts.slice(1).filter((p) => p !== STAND_IN), STAND_IN] : parts.filter((p, i) => i === 0 || p !== STAND_IN)).join(" "));
  // Import…, a file dropped here, or one from a map project: in the game, then picked
  const [credit, setCredit] = useState<AssetCredit | undefined>(undefined);
  const rescan = useCallback(() => void models.refetch(), [models]);
  useFocusRefresh(rescan);
  const imp = useAssetImport(
    "model",
    (path, cr) => {
      onChange([path, ...parts.slice(1).filter((p) => p !== path)].join(" "));
      setCredit(cr ?? undefined);
      setOpen(false);
      setQ("");
    },
    rescan,
  );
  const c = report?.candidates?.[0];
  return (
    <div className="cs-model" {...imp.dropProps}>
      <div className="cs-pop-anchor">
        <button type="button" className="btn cs-model-btn" aria-expanded={open} onClick={() => setOpen(!open)} title="Pick another model">
          <code>{first.replace(/^models\//, "") || "No model"}</code>
          <span aria-hidden="true">▾</span>
        </button>
        <Popover open={open} onClose={() => setOpen(false)} label="Pick a model">
          <ImportZone imp={imp}>
          <input className="search" type="search" autoFocus placeholder="Search models: zombie, garg, player/gign…" value={q} aria-label="Search models" onChange={(e) => setQ(e.target.value)} />
          <ImportBar kind="model" imp={imp} onRefresh={rescan} refreshing={models.isFetching} />
          {models.isPending && <p className="muted"><span className="spinner" aria-hidden="true" /> Finding models…</p>}
          <ul className="cs-pop-list">
            {shown.map((m) => (
              <li key={m.path}>
                <button type="button" className={`cs-pick${m.path === first ? " on" : ""}`} onClick={() => pick(m)} title={m.note}>
                  <span>{m.path.replace(/^models\//, "")}</span>
                  <span className={`cs-kind ${m.kind}`}>{KIND_WORDS[m.kind]}</span>
                </button>
              </li>
            ))}
          </ul>
          {models.data && shown.length === 0 && <EmptyImport kind="model" imp={imp} what="model" />}
          <ProjectFilesGroup kind="model" q={q} imp={imp} />
          </ImportZone>
        </Popover>
      </div>
      {c && (
        <p className="cs-note">
          <span className={`cs-kind ${c.kind}`}>{KIND_WORDS[c.kind]}</span> {c.note}
          {report?.standIn && ` Your game doesn't have it, so ${report.candidates[report.picked ?? 0]?.path} stands in.`}
        </p>
      )}
      <CreditLine credit={credit} />
      <label className="tl-check cs-check">
        <input type="checkbox" checked={standIn} disabled={!first || first === STAND_IN} onChange={(e) => setStandIn(e.target.checked)} />
        Stand-in where it's missing (the hostage, tinted)
      </label>
    </div>
  );
}

const split = (v: string) => v.split("|").map((g) => g.split(/\s+/).filter(Boolean));
const join = (gs: string[][]) => gs.map((g) => g.join(" ")).filter((g, i) => g || i === 0).join(" | ").trim();

function SoundRow({ i, value, onChange, report }: { i: number; value: string; onChange: (v: string) => void; report?: SoundReport }) {
  const sounds = useQuery({ queryKey: ["tools", "creatures", "sounds"], queryFn: ({ signal }) => listSounds(signal).then((r) => r.sounds), staleTime: 60_000 });
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const groups = split(value);
  const hits = useMemo(() => {
    const w = searchWords(q);
    return (sounds.data ?? []).filter((s) => matches(w, s.file)).slice(0, 80);
  }, [sounds.data, q]);
  const remove = (gi: number, fi: number) => onChange(join(groups.map((g, k) => (k === gi ? g.filter((_, j) => j !== fi) : g))));
  const add = (f: string) => {
    if (!groups[0].includes(f)) onChange(join([[...groups[0], f], ...groups.slice(1)]));
  };
  const rescan = useCallback(() => void sounds.refetch(), [sounds]);
  useFocusRefresh(rescan);
  // (sounds are named from inside sound/, as the game reads them)
  const imp = useAssetImport("sound", (path) => (add(path.replace(/^sound\//i, "")), setQ("")), rescan);
  const chip = (f: string, gi: number, fi: number) => {
    const found = report?.groups[gi]?.files.find((x) => x.file === f)?.found;
    return (
      <span key={`${gi}-${f}-${fi}`} className={`cs-chip${found === false ? " missing" : ""}`} title={found === false ? `${f}: not in your game's folders` : f}>
        <button type="button" className="cs-chip-btn" aria-label={`Play ${f}`} onClick={() => playSound(f)}>
          ▶
        </button>
        <span className="cs-chip-name">{f.replace(/\.wav$/i, "")}</span>
        <button type="button" className="cs-chip-btn" aria-label={`Remove ${f}`} onClick={() => remove(gi, fi)}>
          ×
        </button>
      </span>
    );
  };
  const w = SOUND_WORDS[i];
  return (
    <div className="cs-field cs-snd" role="row">
      <div className="cs-label" role="rowheader">
        <span className="cs-label-name">{w.label}</span>
        <span className="cs-label-note">{w.help}</span>
      </div>
      <div className="cs-ctl" role="cell">
        <div className="cs-chips" {...imp.dropProps}>
          {groups[0].map((f, fi) => chip(f, 0, fi))}
          {groups[0].length === 0 && <span className="muted cs-silent">Silent</span>}
          <span className="cs-pop-anchor">
            <button type="button" className="btn small" aria-expanded={open} onClick={() => setOpen(!open)}>
              + Add
            </button>
            <Popover open={open} onClose={() => setOpen(false)} label={`Add a ${w.label.toLowerCase()} sound`}>
              <ImportZone imp={imp}>
              <input className="search" type="search" autoFocus placeholder="Search sounds: zombie, garg, pain…" value={q} aria-label="Search sounds" onChange={(e) => setQ(e.target.value)} />
              <ImportBar kind="sound" imp={imp} onRefresh={rescan} refreshing={sounds.isFetching} />
              {sounds.isPending && <p className="muted"><span className="spinner" aria-hidden="true" /> Finding sounds…</p>}
              <ul className="cs-pop-list">
                {hits.map((s) => (
                  <li key={s.file} className="cs-pop-sound">
                    <button type="button" className="cs-chip-btn" aria-label={`Play ${s.file}`} onClick={() => playSound(s.file)}>
                      ▶
                    </button>
                    <button type="button" className={`cs-pick${groups[0].includes(s.file) ? " on" : ""}`} onClick={() => add(s.file)}>
                      <span>{s.file}</span>
                      <span className="muted">{groups[0].includes(s.file) ? "added" : s.folder}</span>
                    </button>
                  </li>
                ))}
              </ul>
              {sounds.data && hits.length === 0 && <EmptyImport kind="sound" imp={imp} what="sound" />}
              <ProjectFilesGroup kind="sound" q={q} imp={imp} />
              </ImportZone>
            </Popover>
          </span>
        </div>
        {groups.slice(1).some((g) => g.length) && (
          <div className="cs-chips cs-fallback" title="Played only when none of the files above are in the game">
            <span className="muted cs-fallback-label">If those are missing:</span>
            {groups.slice(1).map((g, k) => g.map((f, fi) => chip(f, k + 1, fi)))}
          </div>
        )}
      </div>
    </div>
  );
}

export function SoundTable({ snd, onChange, reports }: { snd: string[]; onChange: (snd: string[]) => void; reports?: SoundReport[] }) {
  return (
    <div className="cs-fields" role="table" aria-label="Sounds">
      {snd.map((v, i) => (
        <SoundRow key={i} i={i} value={v} report={reports?.[i]} onChange={(nv) => onChange(snd.map((x, j) => (j === i ? nv : x)))} />
      ))}
    </div>
  );
}
