// Fonts installed on this computer, for lettering (tools/spray_fonts.rs lists them and hands
// out a family's regular face). The picker shows them in their own face through CSS (the
// webview can use installed fonts by name); the canvas gets the real file as a FontFace, so
// what's drawn and sent to the game is that font, never a stand-in picked by the browser.
//
// A design keeps just the family name. Opened where that family isn't installed, the words
// draw in the fallback (Impact) and the lettering settings say so.

import { getJson } from "../../../api";
import { FONTS, type Doc, type TextLayer } from "./doc";

export type SystemFont = { family: string; styles: string[] };

export const listSystemFonts = () => getJson<{ fonts: SystemFont[]; error?: string }>("/api/tools/spray/fonts");

/** A built-in font (a CSS stack from FONTS), as opposed to an installed family's name. */
export const isBuiltIn = (font: string) => FONTS.some(([v]) => v === font);

/** What a design's fonts can fall back to when one isn't here. */
export const FALLBACK = "Impact";

let listing: Promise<Set<string>> | null = null;
/** The installed family names (lowercase), asked once. */
const installed = () =>
  (listing ??= listSystemFonts()
    .then((r) => new Set((r.fonts ?? []).map((f) => f.family.toLowerCase())))
    .catch(() => {
      listing = null;
      return new Set<string>();
    }));

type State = "loading" | "ready" | "missing";
const state = new Map<string, State>();
const waits = new Map<string, Promise<State>>();

/** Loads an installed family into document.fonts (once). "missing" when it isn't on this
 *  computer or can't be read. */
export function loadSystemFont(family: string): Promise<State> {
  if (isBuiltIn(family) || !family) return Promise.resolve("ready");
  const w = waits.get(family);
  if (w) return w;
  state.set(family, "loading");
  const p = (async (): Promise<State> => {
    try {
      // only families the computer has (no request for one that isn't there)
      const known = await installed();
      if (!known.has(family.toLowerCase())) return "missing";
      const r = await fetch(`/api/tools/spray/font?${new URLSearchParams({ family })}`, { credentials: "same-origin" });
      if (!r.ok) throw new Error(String(r.status));
      const face = new FontFace(family, await r.arrayBuffer());
      await face.load();
      document.fonts.add(face);
      return "ready";
    } catch {
      return "missing";
    }
  })().then((s) => {
    state.set(family, s);
    // the canvas draws again (Stage listens)
    window.dispatchEvent(new Event("ls-font-loaded"));
    return s;
  });
  waits.set(family, p);
  return p;
}

/** "missing" for a family that couldn't be loaded here (after loadSystemFont was asked). */
export const fontState = (family: string): State | undefined => (isBuiltIn(family) ? "ready" : state.get(family));

/** The installed families a design uses. */
export const systemFontsIn = (doc: Doc) => [...new Set(doc.layers.filter((l): l is TextLayer => l.kind === "text").map((l) => l.font).filter((f) => f && !isBuiltIn(f)))];

/** Every installed font the design uses loaded (or found missing): before drawing it for the game. */
export async function fontsReady(doc: Doc): Promise<string[]> {
  const fams = systemFontsIn(doc);
  const got = await Promise.all(fams.map(loadSystemFont));
  return fams.filter((_, i) => got[i] === "missing");
}
