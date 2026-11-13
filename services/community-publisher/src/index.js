// Loner-Strike community index publisher: a Cloudflare Worker on a 10-minute Cron Trigger.
//
// Fetches the list service's read-only export (list.lonerstrike.com/community/publish/index), checks it
// exactly as scripts/community-index-check.py does, and writes it to the download bucket
// (dl.lonerstrike.com/community/index.json + .sha256) through an R2 binding. No R2 keys exist for
// this anywhere: the binding is the Worker's own access. It only ever writes the two community/ keys.
// It has no HTTP endpoint (scheduled only). A forced publish (past the mass-drop guard) stays on the
// GitHub workflow, run by hand.
//
// Secrets: COMMUNITY_PUBLISH_TOKEN (the list service's publish token: it can only download the index).
// Optional: ALERT_WEBHOOK (a Discord webhook URL, e.g. in the private moderators' channel) to hear
// about refusals and failures.
import { check, massDrop, Refused, sha256Hex } from "./check.js";

const SOURCE = "https://list.lonerstrike.com/community/publish";
const INDEX_KEY = "community/index.json";
const SHA_KEY = "community/index.json.sha256";
const CACHE = "public, max-age=300";

async function fetchSource(env, path) {
  let last;
  for (let i = 0; i < 3; i++) {
    try {
      const r = await fetch(`${SOURCE}/${path}`, {
        headers: { Authorization: `Bearer ${env.COMMUNITY_PUBLISH_TOKEN}`, "User-Agent": "LonerStrike community publish (Cloudflare Worker)" },
        signal: AbortSignal.timeout(30_000),
        cf: { cacheTtl: 0 },
      });
      if (!r.ok) throw new Error(`${path}: HTTP ${r.status}`);
      const len = Number(r.headers.get("content-length") || 0);
      if (len > 8 << 20) throw new Refused(`${path} is ${len} bytes`);
      return { bytes: new Uint8Array(await r.arrayBuffer()), headers: r.headers };
    } catch (e) {
      if (e instanceof Refused) throw e;
      last = e;
      await new Promise((ok) => setTimeout(ok, 5000));
    }
  }
  throw last;
}

async function alert(env, text) {
  console.log(text);
  if (!env.ALERT_WEBHOOK) return;
  try {
    await fetch(env.ALERT_WEBHOOK, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ content: `Community index publish: ${text}`.slice(0, 1900), allowed_mentions: { parse: [] } }),
    });
  } catch (e) {
    console.log(`alert failed: ${e}`);
  }
}

export async function publish(env) {
  if (!env.COMMUNITY_PUBLISH_TOKEN) throw new Error("the secret COMMUNITY_PUBLISH_TOKEN isn't set");
  const idx = await fetchSource(env, "index");
  if ((idx.headers.get("x-recheck-held-back") || "").trim() === "1") {
    return { done: "held", text: "the list service's last re-check was held back (many pages leaving at once); nothing published until a person looks (lsmod.py status, then lsmod.py release-hold)" };
  }
  const shaFile = await fetchSource(env, "index.sha256");
  const shaText = new TextDecoder().decode(shaFile.bytes);
  const { doc, sha } = await check(idx.bytes, shaText, idx.headers.get("x-index-sha256"));

  const liveObj = await env.BUCKET.get(INDEX_KEY);
  if (liveObj) {
    const raw = new Uint8Array(await liveObj.arrayBuffer());
    if ((await sha256Hex(raw)) === sha) return { done: "same", text: "the index is the same as the live one" };
    let live = null;
    try { live = JSON.parse(new TextDecoder().decode(raw)); } catch { live = null; }
    if (live && typeof live === "object") {
      const m = massDrop(live, doc);
      if (m.refused) throw new Refused(`${m.drop} of ${m.before} listed creations would disappear at once; check with lsmod.py, then run the GitHub "Community index" workflow by hand with force`);
    }
  }
  // the sha256 file first is wrong for a reader that checks one against the other mid-update;
  // index first, then its sha (as the workflow does). Each write is verified by its MD5 etag.
  for (const [key, body, type] of [[INDEX_KEY, idx.bytes, "application/json; charset=utf-8"], [SHA_KEY, shaFile.bytes, "text/plain; charset=utf-8"]]) {
    const put = await env.BUCKET.put(key, body, { httpMetadata: { contentType: type, cacheControl: CACHE } });
    const md5 = [...new Uint8Array(await crypto.subtle.digest("MD5", body))].map((b) => b.toString(16).padStart(2, "0")).join("");
    if (!put || put.etag !== md5) throw new Error(`${key} didn't arrive intact`);
  }
  return { done: "published", text: `published ${idx.bytes.length} bytes, ${doc.items.length} entries, sha256 ${sha}` };
}

export default {
  async scheduled(_event, env, ctx) {
    ctx.waitUntil((async () => {
      try {
        const r = await publish(env);
        console.log(r.text);
        if (r.done === "held") await alert(env, r.text);
      } catch (e) {
        await alert(env, e instanceof Refused ? `refused: ${e.message}` : `failed: ${e.message || e}`);
        throw e;
      }
    })());
  },
};
