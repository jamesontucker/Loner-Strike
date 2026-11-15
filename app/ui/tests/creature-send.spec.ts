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
