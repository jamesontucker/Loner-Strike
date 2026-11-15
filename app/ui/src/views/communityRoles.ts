// Community roles (docs/notes/Roles.md): the words and rules the UI needs, kept apart from
// React so scripts/roles.test.ts can check them. P5 (UI); the routes are MCP's.

export type AppState = "pending" | "approved" | "denied" | "invited" | "active" | "revoked";

export type Application = {
  id: string;
  state: AppState;
  discord_name: string;
  steamid: string;
  /** where the SteamID was linked: a server's name */
  linked_on?: string | null;
  official_link?: boolean;
  availability?: string;
  answers?: Record<string, string> | { q: string; a: string }[];
  decided_by?: string | null;
  decided_at?: number | null;
  invite?: { state?: string; expires?: number | null } | null;
};

export type RolesSettings = { applications: boolean; discord_mods_decide: boolean; honor_supporters: boolean };
export type RolesServer = { claimed: boolean; official?: boolean; owner_linked?: boolean; owner_code?: string | null; owner_code_expires?: number | null; settings?: RolesSettings; error?: string };

export const STATE_WORD: Record<AppState, string> = {
  pending: "Waiting for a decision",
  approved: "Approved",
  denied: "Denied",
  invited: "Invite sent",
  active: "Moderator",
  revoked: "Removed",
};

/** A state chip's look: good, warn or plain. */
export const STATE_TONE: Record<AppState, "good" | "warn" | ""> = {
  pending: "warn",
  approved: "good",
  denied: "",
  invited: "warn",
  active: "good",
  revoked: "",
};

const STATES = Object.keys(STATE_WORD) as AppState[];
export const isState = (s: unknown): s is AppState => typeof s === "string" && (STATES as string[]).includes(s);

/** What a person may do with an application. Owner and Admin alike: invite once it's approved
 *  (again after an unused invite ran out), and revoke a Moderator or an open invite. */
export function actionsFor(a: Pick<Application, "state">): { invite: boolean; revoke: boolean } {
  return { invite: a.state === "approved", revoke: a.state === "invited" || a.state === "active" };
}

/** The order the list shows: what needs doing first, then the team, then the rest; newest first within. */
export function sortApplications(list: Application[]): Application[] {
  const rank: Record<AppState, number> = { approved: 0, invited: 1, pending: 2, active: 3, denied: 4, revoked: 5 };
  return [...list].sort((x, y) => rank[x.state] - rank[y.state] || (y.decided_at ?? 0) - (x.decided_at ?? 0));
}

/** The answers as question/answer pairs, whatever shape they came in; strings only. */
export function answerPairs(a: Application["answers"]): { q: string; a: string }[] {
  if (Array.isArray(a)) return a.filter((x) => x && typeof x.q === "string" && typeof x.a === "string").map((x) => ({ q: x.q, a: x.a }));
  if (a && typeof a === "object") return Object.entries(a).filter(([, v]) => typeof v === "string").map(([q, v]) => ({ q: q.replace(/_/g, " "), a: v }));
  return [];
}

/** "in 23 hours", "in 40 minutes", "" when it's past or unknown. */
export function inTime(unix: number | null | undefined, now = Date.now() / 1000): string {
  if (!unix || unix <= now) return "";
  const m = Math.max(Math.round((unix - now) / 60), 1);
  if (m < 60) return `in ${m} minute${m === 1 ? "" : "s"}`;
  const h = Math.round(m / 60);
  return `in ${h} hour${h === 1 ? "" : "s"}`;
}

/** A Discord owner code as XXXX-XXXX (what the person types after /server claim code:). */
export function codeShown(code: string): string {
  const c = code.replace(/[^0-9A-Za-z]/g, "").toUpperCase();
  return c.length === 8 ? `${c.slice(0, 4)}-${c.slice(4)}` : c;
}

// ---- an invite's preview (what it says before it's used) ----

export type InvitePreview = { server: string; role: "admin" | "moderator" };

/** What a pasted LS1. invite says about itself (the server's name and the role), read without
 *  using it; null for anything that isn't one. Only for showing: the owner's app decides. */
export function previewInvite(text: string): InvitePreview | null {
  const body = text.trim().replace(/^LS1\./, "");
  if (body === text.trim() || !/^[A-Za-z0-9_-]{16,4096}$/.test(body)) return null;
  try {
    const b64 = body.replace(/-/g, "+").replace(/_/g, "/");
    const bin = atob(b64 + "===".slice((b64.length + 3) % 4));
    const json = new TextDecoder().decode(Uint8Array.from(bin, (c) => c.charCodeAt(0)));
    const v = JSON.parse(json) as { s?: unknown; r?: unknown };
    if (v.r !== "admin" && v.r !== "moderator") return null;
    const server = typeof v.s === "string" ? v.s.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 64).trim() : "";
    return { server: server || "a server", role: v.r };
  } catch {
    return null;
  }
}

// ---- the activity log ----

/** The activity log's words for the roles routes (null: not one of them). */
export function describeRoles(path: string, body: Record<string, unknown> | null): string | null {
  const m = /^\/api\/roles\/server\/[^/]+\/(applications\/[^/]+\/(invite|revoke)|settings|claim|unclaim|owner-code)$/.exec(path);
  if (!m) return null;
  const who = typeof body?.name === "string" ? body.name : "an applicant";
  switch (m[2] ?? m[1]) {
    case "invite":
      return `Made a Moderator invite for ${who}`;
    case "revoke":
      return `Removed ${who === "an applicant" ? "a Moderator" : who} (application)`;
    case "settings":
      return "Changed the Loner-Strike roles settings";
    case "claim":
      return "Claimed the server for Loner-Strike roles";
    case "unclaim":
      return "Unclaimed the server";
    default:
      return "Made a new Discord owner code";
  }
}
