// A GameBanana page's terms (tools/credits.rs rights_of), shown before download in Get more and
// beside each asset on Publish: can you use it in a map you share, is it ported, who to credit.
// Light: Get more loads it in the start bundle.

export type Checklist = Partial<Record<"install" | "redistribute" | "modify_gb" | "modify_other" | "parts_gb" | "parts_other" | "nsfw" | "commercial", string>>;
export type Licence = {
  page?: string;
  name?: string;
  submitter?: { id: number; name: string };
  credits?: { group: string; authors: { name: string; role?: string; url?: string }[] }[];
  ported?: boolean;
  checklist?: Checklist;
  licence?: string;
};

const WORD: Record<string, string> = { yes: "Yes", ask: "Ask first", only_if_same: "Only if you offer the same rights", no: "No" };

/** "Use in a map you share": the stricter of the two sharing lines. */
export function shareWord(c?: Checklist): string {
  const order = ["no", "only_if_same", "ask", "yes"];
  const a = c?.parts_gb ?? "ask";
  const b = c?.parts_other ?? "ask";
  return WORD[order.find((x) => x === a || x === b) ?? "ask"];
}

export function LicenseTerms({ licence }: { licence?: Licence | null }) {
  if (!licence || !licence.checklist) return null;
  const c = licence.checklist;
  const share = shareWord(c);
  const defaults = c.parts_gb === "ask" && c.parts_other === "ask" && c.commercial === "no";
  const people = (licence.credits ?? []).flatMap((g) => g.authors.map((a) => `${a.name}${g.group ? ` (${g.group})` : ""}`));
  return (
    <div className="panel" style={{ display: "grid", gap: 4, padding: "10px 12px", margin: "8px 0" }} aria-label="Its terms">
      <strong>Its terms</strong>
      <span className="small-note">
        Free to install: {WORD[c.install ?? "ask"]} · Use in a map you share: <strong>{share}</strong> · Commercial use: {WORD[c.commercial ?? "ask"]}
      </span>
      {defaults && <span className="muted small-note">GameBanana's default terms: ask the creator before you ship it in a map you share.</span>}
      {licence.ported && <span className="muted small-note">Marked as ported from another game: allowed on GameBanana only non-profit, with credit.</span>}
      {people.length > 0 && <span className="muted small-note">Credits: {people.join(", ")}</span>}
      {licence.licence && <span className="muted small-note">{licence.licence}</span>}
      <span className="muted small-note">By downloading you agree to these terms. Loner-Strike keeps them with the files, so Publish can credit everyone.</span>
    </div>
  );
}
