// The same checks as scripts/community-index-check.py (the GitHub workflow's), for the Worker.
// Keep the two in step: test/check.test.mjs runs both on the same cases.

export const MAX_BYTES = 8 << 20;
export const MAX_ITEMS = 5000;
const KINDS = new Set(["map", "mode", "prefab", "spray", "texture"]);
const STATUSES = new Set(["listed", "hidden", "revoked"]);
const KEY = /^(mod|spray):[1-9][0-9]{0,7}$/;

export class Refused extends Error {}

export async function sha256Hex(bytes) {
  const d = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

const isStr = (v) => typeof v === "string";

// bytes: Uint8Array of the new index. Returns { doc, sha } or throws Refused.
export async function check(bytes, shaText, headerSha) {
  if (bytes.length > MAX_BYTES) throw new Refused(`the index is ${bytes.length} bytes; the app reads at most ${MAX_BYTES}`);
  const sha = await sha256Hex(bytes);
  const want = ((shaText || "").trim().split(/\s+/)[0] || "").toLowerCase();
  if (sha !== want || sha !== (headerSha || "").trim().toLowerCase()) throw new Refused("the index doesn't match its sha256");
  let doc;
  try {
    doc = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw new Refused("the index isn't JSON");
  }
  if (!doc || typeof doc !== "object" || Array.isArray(doc) || doc.schema !== 1) throw new Refused("the index isn't schema 1");
  const { items, revoked } = doc;
  const authors = doc.verifiedAuthors === undefined ? [] : doc.verifiedAuthors;
  if (!Array.isArray(items) || !Array.isArray(revoked) || !Array.isArray(authors)) throw new Refused("the index's lists are missing");
  if (items.length > MAX_ITEMS) throw new Refused(`the index has ${items.length} entries; the app reads at most ${MAX_ITEMS}`);
  if (!revoked.every((r) => isStr(r) && KEY.test(r))) throw new Refused("the index's pulled list has something that isn't a page");
  for (const it of items) {
    const ok = it && typeof it === "object" && !Array.isArray(it) && KINDS.has(it.kind) && (it.model === "Mod" || it.model === "Spray")
      && Number.isInteger(it.gb) && it.gb > 0 && it.gb < 100_000_000 && STATUSES.has(it.status)
      && isStr(it.name) && it.name.trim() !== "";
    if (!ok) throw new Refused("the index has an entry the app couldn't read");
  }
  return { doc, sha };
}

export const listed = (doc) => (Array.isArray(doc?.items) ? doc.items : []).filter((it) => it && typeof it === "object" && it.status === "listed").length;

// The re-check guard's rule: more than 3, or 2 or more and over a quarter of what's live.
export function massDrop(live, doc) {
  const before = listed(live), after = listed(doc), drop = before - after;
  return { before, after, drop, refused: drop > 3 || (drop >= 2 && drop > before * 0.25) };
}
