// One search rule for every Tools list (maps, entities, textures, models): every word typed
// has to be somewhere in the item's fields, any case, with _ - . and spaces not mattering
// inside a word ("dust 2" finds de_dust2, "func door" finds func_door_rotating).

const squash = (s: string) => s.toLowerCase().replace(/[\s_\-.]+/g, "");

/** The words of a search box, ready for matches(); empty: everything matches. */
export function searchWords(q: string): string[] {
  return q.toLowerCase().split(/\s+/).map(squash).filter(Boolean);
}

/** Whether an item with these fields (null/undefined skipped) matches the words. */
export function matches(words: string[], ...fields: (string | null | undefined)[]): boolean {
  if (!words.length) return true;
  const hay = squash(fields.filter(Boolean).join(" "));
  // ("dust 2": the words also match run together, so the gap in de_dust2 doesn't matter)
  return words.every((w) => hay.includes(w)) || hay.includes(words.join(""));
}
