// A Color value (graph version 4): a swatch with the browser's color picker, team presets, and how
// opaque it is (alpha). Stored as "#rrggbbaa" (graph.rs color_hex).

const PRESETS: [string, string][] = [
  ["#e03c3cff", "Terrorist red"],
  ["#3c78e0ff", "Counter-Terrorist blue"],
  ["#20c060ff", "Green"],
  ["#ffcc00ff", "Gold"],
  ["#ffffffff", "White"],
  ["#000000ff", "Black"],
];

export function parseColor(v: unknown): { rgb: string; a: number } {
  const s = typeof v === "string" && /^#([0-9a-f]{6}|[0-9a-f]{8})$/i.test(v) ? v.toLowerCase() : "#ffffffff";
  return { rgb: s.slice(0, 7), a: s.length === 9 ? parseInt(s.slice(7, 9), 16) : 255 };
}

export const colorValue = (rgb: string, a: number) => `${rgb}${Math.max(0, Math.min(255, Math.round(a))).toString(16).padStart(2, "0")}`;

export function ColorField({ value, onChange, label = "Color" }: { value: unknown; onChange: (v: string) => void; label?: string }) {
  const { rgb, a } = parseColor(value);
  return (
    <div className="st-color-field">
      <input type="color" aria-label={label} value={rgb} onChange={(e) => onChange(colorValue(e.target.value, a))} />
      <label className="st-color-alpha">
        Alpha
        <input type="range" min={0} max={255} value={a} aria-label={`${label}: alpha`} onChange={(e) => onChange(colorValue(rgb, Number(e.target.value)))} />
      </label>
      <div className="st-color-presets" role="group" aria-label={`${label}: presets`}>
        {PRESETS.map(([c, name]) => (
          <button key={c} type="button" className="st-color-swatch" title={name} aria-label={name} style={{ background: c.slice(0, 7) }} onClick={() => onChange(c)} />
        ))}
      </div>
    </div>
  );
}
