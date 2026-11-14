// One picker for every field that names a creature type (shared creatures): the map editor's
// ls_creature / ls_wave_spawner keys, the Game Mode Studio's creature params, and anything else.
// It suggests the built-in types, your own (Creature Studio) and installed ones (from community
// items, named <author>.<type>), still takes any typed name, and says so when this game has no
// type by that name.

import { useQuery } from "@tanstack/react-query";
import { useId } from "react";
import { tryGet } from "./api";
import "./creaturePicker.css";

export type CreatureNames = { builtin: string[]; planned?: string[]; own: string[]; installed: { name: string; label?: string; from: string[] }[] };

/** The names this game knows (null while loading, or when the game isn't set up). */
export function useCreatureNames(): CreatureNames | null {
  const q = useQuery({
    queryKey: ["tools", "creatures", "names"],
    queryFn: ({ signal }) => tryGet<CreatureNames>("/api/tools/creatures/names", signal),
    staleTime: 15_000,
  });
  const d = q.data;
  return d && !d.error && Array.isArray(d.builtin) ? d : null;
}

export function allNames(n: CreatureNames): string[] {
  return [...n.builtin, ...(n.planned ?? []), ...n.own, ...n.installed.map((i) => i.name)];
}

/** The words of a value this game has no type for ("" and unknown lists: none). */
export function unknownTypes(value: string, n: CreatureNames | null, list = false): string[] {
  if (!n) return [];
  const known = new Set(allNames(n).map((x) => x.toLowerCase()));
  const words = list ? value.split(/[\s,]+/) : [value];
  return words.map((w) => w.trim()).filter((w) => w && !known.has(w.toLowerCase()));
}

/**
 * The input plus its suggestions and warning. `list`: a space-separated list of types (an
 * ls_wave_spawner's types); a pick is added to the end. The parent decides when to commit
 * (`onChange` on every keystroke, `onCommit` on blur, Enter or a pick).
 */
export function CreaturePicker({
  value,
  onChange,
  onCommit,
  list = false,
  id,
  placeholder,
  className = "",
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  onCommit?: (v: string) => void;
  list?: boolean;
  id?: string;
  placeholder?: string;
  className?: string;
  label?: string;
}) {
  const names = useCreatureNames();
  const auto = useId();
  const listId = `creature-names-${auto.replace(/:/g, "")}`;
  const unknown = unknownTypes(value, names, list);
  const commit = (v: string) => onCommit?.(v);
  const pick = (t: string) => {
    if (!t) return;
    const next = list ? [...value.split(/\s+/).filter(Boolean).filter((w) => w !== t), t].join(" ") : t;
    onChange(next);
    commit(next);
  };
  return (
    <span className={`creature-picker ${className}`}>
      <span className="creature-picker-row">
        <input
          id={id}
          className="creature-picker-input"
          value={value}
          placeholder={placeholder}
          aria-label={label}
          list={list ? undefined : listId}
          spellCheck={false}
          autoComplete="off"
          aria-invalid={unknown.length > 0 || undefined}
          onChange={(e) => onChange(e.target.value)}
          onBlur={(e) => commit(e.target.value.trim())}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit((e.target as HTMLInputElement).value.trim());
          }}
        />
        {names && (
          <select className="creature-picker-add" value="" aria-label={list ? (label ? `Add a type to ${label}` : "Add a type") : label ? `Pick ${label}` : "Pick a type"} onChange={(e) => pick(e.target.value)}>
            <option value="">{list ? "Add…" : "Pick…"}</option>
            <Groups n={names} />
          </select>
        )}
      </span>
      {!list && names && (
        <datalist id={listId}>
          {names.builtin.map((t) => (
            <option key={t} value={t} label="Built in" />
          ))}
          {names.own.map((t) => (
            <option key={t} value={t} label="Yours" />
          ))}
          {names.installed.map((t) => (
            <option key={t.name} value={t.name} label={t.label ?? t.name} />
          ))}
        </datalist>
      )}
      {!list && names?.installed.find((t) => t.name === value)?.label && <span className="creature-picker-shown muted">{names.installed.find((t) => t.name === value)!.label}</span>}
      {unknown.length > 0 && (
        <span className="creature-picker-warn" role="status">
          {unknown.length === 1 ? `This game has no creature type called ${unknown[0]}.` : `This game has no creature types called ${unknown.join(", ")}.`} Make it in the Creature Studio, or pick one.
        </span>
      )}
    </span>
  );
}

function Groups({ n }: { n: CreatureNames }) {
  return (
    <>
      <optgroup label="Built in">
        {n.builtin.map((t) => (
          <option key={t} value={t}>
            {t}
          </option>
        ))}
      </optgroup>
      {n.own.length > 0 && (
        <optgroup label="Yours">
          {n.own.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </optgroup>
      )}
      {n.installed.length > 0 && (
        <optgroup label="Installed">
          {n.installed.map((t) => (
            <option key={t.name} value={t.name}>
              {t.label ?? t.name}
            </option>
          ))}
        </optgroup>
      )}
    </>
  );
}
