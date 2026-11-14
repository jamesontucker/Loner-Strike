// MCP: an automatic cover card for a mode's GameBanana page (1280×720, GameBanana shows 16:9):
// the mode's name, an emblem, its tags, a short line and "Made with Loner-Strike", in the
// site's colors. Drawn on a canvas in the page, so it works offline and is drawn again whenever
// the name or the line changes. Offered as the first screenshot when the author has none.

export const COVER_W = 1280;
export const COVER_H = 720;

const C = { bg: "#3e4637", side: "#353c2f", panel: "#4c5844", bone: "#d8ded3", dim: "#c2c9b7", signal: "#c4b550", ink: "#26240f", lo: "#292c21" };

export type CoverInfo = { name: string; line: string; tags: string[] };

/** The words wrapped to fit a width (at most `lines` lines, the last cut with "…"). */
export function wrap(words: string, fits: (s: string) => boolean, lines: number): string[] {
  const out: string[] = [];
  let cur = "";
  for (const w of words.split(/\s+/).filter(Boolean)) {
    const next = cur ? `${cur} ${w}` : w;
    if (fits(next)) cur = next;
    else {
      if (cur) out.push(cur);
      cur = w;
      if (out.length === lines) break;
    }
  }
  if (cur && out.length < lines) out.push(cur);
  if (out.length > lines) out.length = lines;
  const used = out.join(" ").split(/\s+/).length;
  if (used < words.split(/\s+/).filter(Boolean).length && out.length) {
    let last = out[out.length - 1];
    while (last && !fits(`${last}…`)) last = last.replace(/\s*\S+$/, "");
    out[out.length - 1] = `${last}…`;
  }
  return out;
}

/** The emblem's letters: the name's first two words' initials ("Last Stand" → "LS"). */
export function initials(name: string): string {
  const w = name.trim().split(/\s+/).filter((x) => /\p{L}|\p{N}/u.test(x));
  return (w.length > 1 ? w[0][0] + w[1][0] : (w[0] ?? "?").slice(0, 2)).toUpperCase();
}

/** The card as a PNG data URL ("" when the page can't draw). */
export async function drawCover(info: CoverInfo): Promise<string> {
  const canvas = document.createElement("canvas");
  canvas.width = COVER_W;
  canvas.height = COVER_H;
  const g = canvas.getContext("2d");
  if (!g) return "";
  // (the app's own fonts, once they're loaded; offline the fallbacks still read well)
  await document.fonts?.ready?.catch?.(() => undefined);
  const head = '"Counter-Strike", "Barlow Semi Condensed", "Arial Narrow", sans-serif';
  const text = '"Barlow Semi Condensed", "Bahnschrift SemiCondensed", "Arial Narrow", Arial, sans-serif';
  // background: the site's olive, a darker side band, a bevel line
  g.fillStyle = C.bg;
  g.fillRect(0, 0, COVER_W, COVER_H);
  g.fillStyle = C.side;
  g.fillRect(0, 0, 360, COVER_H);
  g.fillStyle = C.lo;
  g.fillRect(360, 0, 4, COVER_H);
  // emblem: a ring with the initials
  g.strokeStyle = C.signal;
  g.lineWidth = 10;
  g.beginPath();
  g.arc(180, 300, 110, 0, Math.PI * 2);
  g.stroke();
  g.fillStyle = C.signal;
  g.font = `700 96px ${head}`;
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText(initials(info.name), 180, 306);
  g.font = `600 26px ${text}`;
  g.fillStyle = C.dim;
  g.fillText("GAME MODE", 180, 460);
  // name, up to two lines
  g.textAlign = "left";
  g.textBaseline = "alphabetic";
  g.fillStyle = C.bone;
  g.font = `700 88px ${head}`;
  const nameLines = wrap(info.name || "Untitled mode", (s) => g.measureText(s).width <= 840, 2);
  nameLines.forEach((l, i) => g.fillText(l, 420, 190 + i * 96));
  let y = 190 + nameLines.length * 96 + 10;
  // tags as chips
  g.font = `600 30px ${text}`;
  let x = 420;
  for (const t of info.tags.slice(0, 6)) {
    const w = g.measureText(t).width + 36;
    if (x + w > COVER_W - 40) break;
    g.fillStyle = C.panel;
    g.fillRect(x, y, w, 50);
    g.fillStyle = C.bone;
    g.fillText(t, x + 18, y + 36);
    x += w + 14;
  }
  y += info.tags.length ? 100 : 30;
  // the short line
  g.fillStyle = C.dim;
  g.font = `500 36px ${text}`;
  wrap(info.line, (s) => g.measureText(s).width <= 820, 3).forEach((l, i) => g.fillText(l, 420, y + i * 46));
  // footer
  g.fillStyle = C.signal;
  g.fillRect(420, COVER_H - 96, 64, 6);
  g.font = `700 32px ${text}`;
  g.fillText("Made with Loner-Strike", 420, COVER_H - 50);
  return canvas.toDataURL("image/png");
}
