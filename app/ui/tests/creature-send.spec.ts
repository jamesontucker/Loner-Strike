// "Send my creatures to…" (Creature Studio): pick servers and your own types, a spinner while
// they go, a summary, and each server's list (up to date, changed, not there, the server's own)
// with Update and Remove. The two routes are faked; the page runs on a dev server over a scratch
// game copy whose creatures.ini has a type of the player's own (qa_crawler).
import { expect, test, type Route } from "@playwright/test";

const json = (r: Route, body: unknown) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });

test("Send to servers: pick, send with a spinner, then Update and Remove per server", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  const sent: Record<string, unknown>[] = [];
  let state: "missing" | "current" | "changed" = "missing";
  await page.route("**/api/tools/creatures/servers", (r) =>
    json(r, {
      servers: [
        { id: "s1", name: "Home server", remote: false, game: "cstrike", sameGame: true, types: [{ name: "qa_crawler", state }, { name: "zed", state: "theirs" }] },
        { id: "s2", name: "Linux box", remote: true, game: "cstrike", sameGame: true, types: [{ name: "qa_crawler", state: "changed" }] },
      ],
    }),
  );
  await page.route("**/api/tools/creatures/send", async (r) => {
    const body = r.request().postDataJSON() as Record<string, unknown>;
    sent.push(body);
    await new Promise((ok) => setTimeout(ok, 400));
    const servers = body.servers as string[];
    state = (body.remove as string[]).length ? "missing" : "current";
    return json(r, { ok: true, message: `Sent to ${servers.length} server${servers.length === 1 ? "" : "s"}. They use it from their next map.`, results: servers.map((id) => ({ id, name: id === "s1" ? "Home server" : "Linux box", ok: true, types: ["qa_crawler"], sent: body.types, skipped: [], kept: [] })) });
  });
  await page.goto("/tools-dev.html?open=creatures");
  await page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /qa_crawler/ }).click();
  await page.getByRole("button", { name: "Send to servers…" }).click();
  const sheet = page.getByRole("dialog", { name: "Send my creatures to…" });
  await expect(sheet).toContainText("Home server");
  // the type picked in the list starts ticked; two servers picked: the spinner names them
  await expect(sheet.getByRole("checkbox", { name: "qa_crawler" })).toBeChecked();
  await sheet.getByRole("checkbox", { name: /Home server/ }).check();
  await sheet.getByRole("checkbox", { name: /Linux box/ }).check();
  await sheet.getByRole("button", { name: "Send", exact: true }).click();
  await expect(sheet.getByRole("button", { name: "Sending to 2 servers…" })).toBeVisible();
  await expect(page.locator(".toast", { hasText: "Sent to 2 servers." })).toBeVisible();
  expect(sent[0]).toEqual({ servers: ["s1", "s2"], types: ["qa_crawler"], remove: [] });
  await expect(sheet.getByRole("list", { name: "What was sent" })).toContainText("Home server: sent qa_crawler");
  // each server's list
  const home = sheet.getByRole("region", { name: "On Home server" });
  await expect(home).toContainText("Up to date");
  await expect(home).toContainText("The server's own");
  const linux = sheet.getByRole("region", { name: "On Linux box" });
  await expect(linux).toContainText("Changed here");
  await linux.getByRole("button", { name: "Update" }).click();
  await expect.poll(() => sent.length).toBe(2);
  expect(sent[1]).toEqual({ servers: ["s2"], types: ["qa_crawler"], remove: [] });
  await home.getByRole("button", { name: "Remove" }).click();
  await expect.poll(() => sent.length).toBe(3);
  expect(sent[2]).toEqual({ servers: ["s1"], types: [], remove: ["qa_crawler"] });
  await expect(home).toContainText("Not there");
  expect(problems).toEqual([]);
});

test("Send to servers: modes that list their creature types say so, one click per mode or all", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  const listed: Record<string, string[]> = { laststand: ["zombie"], zday: ["zombie", "runner"] };
  const calls: Record<string, unknown>[] = [];
  // two modes that list their types (Last Stand's waves, Zombie Day's cfg list); one that loads all isn't listed
  await page.route("**/api/tools/creatures/servers", (r) =>
    json(r, {
      servers: [
        {
          id: "s1",
          name: "Stand server",
          remote: false,
          game: "cstrike",
          sameGame: true,
          limits: [
            { id: "laststand", name: "Last Stand", how: "waves", types: listed.laststand },
            { id: "zday", name: "Zombie Day", how: "precache", types: listed.zday },
          ],
          types: [{ name: "qa_crawler", state: "current" }],
        },
      ],
    }),
  );
  await page.route("**/api/tools/creatures/send-modes", async (r) => {
    const body = r.request().postDataJSON() as { servers: string[]; modes: string[]; types: string[] };
    calls.push(body);
    await new Promise((ok) => setTimeout(ok, 300));
    for (const m of body.modes.length ? body.modes : Object.keys(listed)) listed[m] = [...listed[m], ...body.types];
    return json(r, { ok: true, message: "Added. The modes load them from the next map.", results: [{ id: "s1", name: "Stand server", ok: true }] });
  });
  await page.goto("/tools-dev.html?open=creatures");
  await page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /qa_crawler/ }).click();
  await page.getByRole("button", { name: "Send to servers…" }).click();
  const srv = page.getByRole("dialog", { name: "Send my creatures to…" }).getByRole("region", { name: "On Stand server" });
  await expect(srv.getByRole("note")).toContainText("These modes on Stand server only load the creature types they list: Last Stand, Zombie Day. Add qa_crawler to their lists to use it there.");
  await srv.getByRole("button", { name: "Add to Last Stand's creatures" }).click();
  await expect(page.locator(".toast", { hasText: "Added. The modes load them" })).toBeVisible();
  expect(calls[0]).toEqual({ servers: ["s1"], modes: ["laststand"], types: ["qa_crawler"] });
  // only Zombie Day is left: no "all" button for one mode
  await expect(srv.getByRole("note")).toContainText("only load the creature types they list: Zombie Day.");
  await expect(srv.getByRole("button", { name: "Add to all of them" })).toHaveCount(0);
  await srv.getByRole("button", { name: "Add to Zombie Day's creatures" }).click();
  await expect.poll(() => calls.length).toBe(2);
  await expect(srv.getByRole("note")).toHaveCount(0);
  await expect(srv).toContainText("In their lists");
  expect(problems).toEqual([]);
});

test("Send to servers: Add to all of them", async ({ page }) => {
  const calls: Record<string, unknown>[] = [];
  await page.route("**/api/tools/creatures/servers", (r) =>
    json(r, { servers: [{ id: "s1", name: "Stand server", remote: false, game: "cstrike", sameGame: true, limits: [{ id: "laststand", name: "Last Stand", how: "waves", types: [] }, { id: "zday", name: "Zombie Day", how: "precache", types: ["zombie"] }], types: [{ name: "qa_crawler", state: "changed" }] }] }),
  );
  await page.route("**/api/tools/creatures/send-modes", (r) => (calls.push(r.request().postDataJSON() as Record<string, unknown>), json(r, { ok: true, message: "Added.", results: [] })));
  await page.goto("/tools-dev.html?open=creatures");
  await page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /qa_crawler/ }).click();
  await page.getByRole("button", { name: "Send to servers…" }).click();
  await page.getByRole("button", { name: "Add to all of them" }).click();
  await expect.poll(() => calls.length).toBe(1);
  expect(calls[0]).toEqual({ servers: ["s1"], modes: [], types: ["qa_crawler"] });
});
