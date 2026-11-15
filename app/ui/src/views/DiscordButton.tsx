// "Join the Loner-Strike Discord": the permanent invite (the same one Settings, setup and the
// macOS Help menu use), opened in the system browser like the app's other external links.
// Shown on Find servers and Tools > Community.

export const DISCORD_URL = "https://discord.gg/K9eBHVVwZE";

/** A simple chat-bubble mark (our own drawing, not Discord's logo). */
function Glyph({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M5 5h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-7l-4 3v-3H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z" />
      <circle cx="9" cy="11" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="11" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}

/** A button-sized link; `note` adds a short line beside it (the card form). */
export function DiscordButton({ note }: { note?: string }) {
  const link = (
    <a className="btn discord-btn" href={DISCORD_URL} target="_blank" rel="noreferrer" title="Opens the Loner-Strike Discord in your browser" style={{ display: "inline-flex", alignItems: "center", gap: 8, whiteSpace: "nowrap", textDecoration: "none" }}>
      <Glyph />
      Join the Loner-Strike Discord
    </a>
  );
  if (!note) return link;
  return (
    <div className="panel discord-card" style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "8px 16px" }}>
      {link}
      <span className="muted small-note" style={{ flex: "1 1 240px" }}>{note}</span>
    </div>
  );
}
