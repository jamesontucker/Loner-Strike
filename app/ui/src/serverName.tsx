// Server names (the app's sharing::clean_name): Steam's server browser and master list take 64
// bytes with the NUL, so 63 bytes of UTF-8. Counted in bytes, cut between characters (never
// inside one), so "é" counts 2 and an emoji 4.

export const MAX_NAME_BYTES = 63;

const bytes = (s: string) => new TextEncoder().encode(s).length;

/** `s` cut to 63 bytes of UTF-8 at a character boundary (what a name field keeps as you type). */
export function capName(s: string): string {
  if (bytes(s) <= MAX_NAME_BYTES) return s;
  let out = "";
  let used = 0;
  for (const ch of s) {
    const n = bytes(ch);
    if (used + n > MAX_NAME_BYTES) break;
    out += ch;
    used += n;
  }
  return out;
}

/** How many bytes are left, for the "n left" counter (shown in the last 10). */
export const nameBytesLeft = (s: string) => MAX_NAME_BYTES - bytes(s);

/** "n left" under a server name field, in its last 10 bytes. */
export function NameLeft({ name }: { name: string }) {
  const left = nameBytesLeft(name);
  if (left > 10) return null;
  return (
    <small className="name-left" aria-live="polite">
      {left} left
    </small>
  );
}
