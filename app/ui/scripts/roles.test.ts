// P5: the community roles UI's rules and words (src/views/communityRoles.ts).
//   node scripts/test-roles.mjs

import { test } from "node:test";
import assert from "node:assert/strict";
import { actionsFor, answerPairs, codeShown, describeRoles, inTime, isState, previewInvite, sortApplications, type Application } from "../src/views/communityRoles";

const invite = (o: object) => "LS1." + Buffer.from(JSON.stringify(o)).toString("base64url");

test("an invite says its server and role before it's used", () => {
  const good = { a: ["1.2.3.4"], p: 8031, f: "a".repeat(64), c: "b".repeat(32), s: "Friday Frags", r: "moderator" };
  assert.deepEqual(previewInvite(invite(good)), { server: "Friday Frags", role: "moderator" });
  assert.deepEqual(previewInvite("  " + invite({ ...good, r: "admin" }) + "\n"), { server: "Friday Frags", role: "admin" });
  // control characters out of the name, a long one cut, an empty one named
  assert.equal(previewInvite(invite({ ...good, s: "Bad\u0007Name" }))?.server, "BadName");
  assert.equal(previewInvite(invite({ ...good, s: "x".repeat(200) }))?.server.length, 64);
  assert.equal(previewInvite(invite({ ...good, s: "" }))?.server, "a server");
  // not an invite, or one that can't say a role it doesn't have
  for (const bad of ["", "LS1.", "LS1.nonsense", "hello", invite({ ...good, r: "owner" }), invite({ ...good, r: undefined }), "LS1.%%%%%%%%%%%%%%%%%%%%", invite(good).slice(0, 20)]) {
    assert.equal(previewInvite(bad), null, bad);
  }
});

test("what can be done with an application", () => {
  assert.deepEqual(actionsFor({ state: "approved" }), { invite: true, revoke: false });
  assert.deepEqual(actionsFor({ state: "invited" }), { invite: false, revoke: true });
  assert.deepEqual(actionsFor({ state: "active" }), { invite: false, revoke: true });
  for (const s of ["pending", "denied", "revoked"] as const) assert.deepEqual(actionsFor({ state: s }), { invite: false, revoke: false }, s);
  assert.ok(isState("active") && !isState("owner") && !isState(3));
});

test("the list: what needs doing first, newest first within", () => {
  const a = (id: string, state: Application["state"], at: number): Application => ({ id, state, discord_name: id, steamid: "", decided_at: at });
  const sorted = sortApplications([a("old-ok", "approved", 1), a("mod", "active", 9), a("new-ok", "approved", 5), a("wait", "pending", 0), a("no", "denied", 3), a("sent", "invited", 2)]);
  assert.deepEqual(sorted.map((x) => x.id), ["new-ok", "old-ok", "sent", "wait", "mod", "no"]);
});

test("answers in either shape, strings only", () => {
  assert.deepEqual(answerPairs({ why_you: "I play every night", n: 3 as unknown as string }), [{ q: "why you", a: "I play every night" }]);
  assert.deepEqual(answerPairs([{ q: "Experience", a: "2 years" }, { q: "x" } as { q: string; a: string }]), [{ q: "Experience", a: "2 years" }]);
  assert.deepEqual(answerPairs(undefined), []);
});

test("times and codes", () => {
  assert.equal(inTime(1000 + 90 * 60, 1000), "in 2 hours");
  assert.equal(inTime(1000 + 40 * 60, 1000), "in 40 minutes");
  assert.equal(inTime(1000 + 20, 1000), "in 1 minute");
  assert.equal(inTime(999, 1000), "");
  assert.equal(inTime(null, 1000), "");
  assert.equal(codeShown("abcd2345"), "ABCD-2345");
  assert.equal(codeShown("ABCD-2345"), "ABCD-2345");
});

test("the activity log's words for the roles routes", () => {
  assert.equal(describeRoles("/api/roles/server/s1/applications/a9/invite", { name: "Kim" }), "Made a Moderator invite for Kim");
  assert.equal(describeRoles("/api/roles/server/s1/applications/a9/revoke", null), "Removed a Moderator (application)");
  assert.equal(describeRoles("/api/roles/server/s1/settings", {}), "Changed the Loner-Strike roles settings");
  assert.equal(describeRoles("/api/roles/server/s1/claim", {}), "Claimed the server for Loner-Strike roles");
  assert.equal(describeRoles("/api/admins/ban", {}), null);
});
