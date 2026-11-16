// New server, A Linux server: "Use a machine you already have" fills in the address, user and
// key from a server already on it; after Check the machine, a Port field suggests the first port
// free there and names which servers use which ports. A taken port can't be sent. A pasted
// address loses the comma after it.
//   npx playwright test new-server-shared-machine --project=large --project=small
//   (LS_SHOT_DIR=<folder>: also a screenshot of the form, at the project's width)
import { expect, test } from "@playwright/test";

test("a second server on a machine you already have gets its own port", async ({ page }) => {
  const sent: Record<string, unknown>[] = [];
  await page.route(/\/api\/state(\?|$)/, (r) => r.fulfill({ json: { online: false, reason: "not running" } }));
  await page.route(/\/api\/servers(\?|$)/, (r) => r.fulfill({ json: { servers: [] } }));
  await page.route(/\/api\/setup\/scan(\?|$)/, (r) =>
    r.fulfill({ json: { installs: [{ hl: "/fake/Half-Life", kind: "crossover", bottle: "Steam", games: [{ id: "cstrike", name: "Counter-Strike 1.6", installed: true, components: ["core", "foundation"], foreign: false }] }], catalog: [], native_mac: false } }),
  );
  await page.route(/\/api\/app(\?|$)/, (r) =>
    r.fulfill({ json: { version: "1.0.0", game: { id: "cstrike", dir: "/fake/Half-Life/cstrike", hl: "/fake/Half-Life", dedicated: false, installed: ["core", "foundation"], outdated: [], unmanaged: [], unbuilt: [], others: [] } } }),
  );
  await page.route(/\/api\/servers\/machines(\?|$)/, (r) =>
    r.fulfill({ json: { machines: [{ host: "178.156.241.180", user: "cs", key: "/Users/me/.ssh/id_ed25519", ssh_port: 22, servers: [{ name: "Alone Together", port: 27015 }] }] } }),
  );
  await page.route(/\/api\/servers\/remote\/probe$/, async (r) => {
    sent.push(r.request().postDataJSON());
    await r.fulfill({ json: { ok: true, fingerprint: "SHA256:box", known: true, arch: "x86_64", x86: true, port: 27016, used: [{ port: 27015, by: '"Alone Together"' }] } });
  });
  await page.route(/\/api\/servers\/remote$/, async (r) => {
    sent.push(r.request().postDataJSON());
    await r.fulfill({ json: { error: "stop here" } });
  });
  await page.goto("/");
  await page.locator(".side-server.add").click({ timeout: 30_000 });
  const sheet = page.locator(".new-server");
  await sheet.getByRole("button", { name: "A Linux server (VPS)" }).click();

  // a pasted address loses its comma
  const address = sheet.getByPlaceholder("Like 146.235.20.10");
  await address.fill("play.lonerstrike.com,");
  await expect(address).toHaveValue("play.lonerstrike.com");

  // the machine you already have fills it all in
  await sheet.getByRole("combobox", { name: "Use a machine you already have" }).selectOption({ label: "178.156.241.180 (Alone Together)" });
  await expect(address).toHaveValue("178.156.241.180");
  await expect(sheet.getByPlaceholder("ubuntu or root")).toHaveValue("cs");
  await expect(sheet.getByPlaceholder("Pick your key file")).toHaveValue("/Users/me/.ssh/id_ed25519");

  await sheet.getByRole("button", { name: "Check the machine" }).click();
  const port = sheet.getByRole("textbox", { name: "Port" });
  await expect(port).toHaveValue("27016");
  await expect(sheet.getByTestId("new-server-port")).toContainText('Used on this machine: 27015 by "Alone Together".');
  await sheet.getByPlaceholder("Like Friday night frags").fill("Battle Royale");
  const dir = process.env.LS_SHOT_DIR;
  if (dir) await sheet.screenshot({ path: `${dir}/new-server-shared-machine-${page.viewportSize()?.width}.png` });

  // a taken port can't be sent; the free one is
  await port.fill("27015");
  await expect(sheet.getByTestId("new-server-port")).toContainText("Pick a free port");
  await expect(sheet.getByRole("button", { name: /Make the server/ })).toBeDisabled();
  await port.fill("27016");
  await sheet.getByRole("button", { name: /Make the server/ }).click();
  await expect.poll(() => sent.at(-1)?.port).toBe(27016);
  expect(sent.at(-1)).toMatchObject({ host: "178.156.241.180", user: "cs", host_key: "SHA256:box" });
});
