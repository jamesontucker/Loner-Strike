// The Sprite maker (T1fix): "Make sprite…" in Tools › Textures & models and in a sprite picker
// (Game Mode Studio's Objective Marker): pictures in as frames, a live animated preview from the
// real build route (tools/spritemaker.rs, the .spr read back), size, how it faces you and how
// it's drawn, then "Make sprite" saves it through the pickers' Import (faked here, so nothing is
// written) and selects it. Runs against a dev server over a scratch game copy (never the real one).
import { expect, test, type Page, type Route } from "@playwright/test";
import { deflateSync } from "node:zlib";

const H = { "X-Panel": "1" };
const json = (r: Route, body: unknown) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });

/** A small solid PNG (w × h, one color), as a file for the page's file input. */
function png(name: string, w: number, h: number, rgba: [number, number, number, number]) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (b: Buffer) => {
    let c = 0xffffffff;
    for (const x of b) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (t: string, d: Buffer) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(d.length);
    const td = Buffer.concat([Buffer.from(t), d]);
    const c = Buffer.alloc(4);
    c.writeUInt32BE(crc(td));
    return Buffer.concat([len, td, c]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array.from({ length: w }, () => rgba).flat())]);
  const raw = Buffer.concat(Array.from({ length: h }, () => row));
  const buffer = Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
  return { name, mimeType: "image/png", buffer };
}

/** The pickers' Import, faked: what was sent, answered as saved under sprites/custom/. */
async function fakeImport(page: Page) {
  const sent: Record<string, unknown>[] = [];
  const builds: Record<string, unknown>[] = [];
  page.on("request", (r) => {
    if (r.url().includes("/api/tools/sprites/build")) builds.push(r.postDataJSON() as Record<string, unknown>);
  });
  await page.route("**/api/tools/assets/import", async (r) => {
    const body = r.request().postDataJSON() as Record<string, unknown>;
    sent.push(body);
    return json(r, { ok: true, path: `sprites/custom/${body.name}`, use: `sprites/custom/${body.name}`, files: [`sprites/custom/${body.name}`] });
  });
  await page.route("**/api/tools/assets/project-files**", (r) => json(r, { files: [] }));
  return { sent, builds };
}

async function makeOne(page: Page, frames: number) {
  const maker = page.getByRole("alertdialog", { name: "Make a sprite" });
  await expect(maker).toBeVisible();
  await maker.getByLabel("Pictures for the sprite").setInputFiles(Array.from({ length: frames }, (_, i) => png(`Fire Ball ${i + 1}.png`, 40, 20, [255, 80 + i * 40, 0, 255])));
  await expect(maker.getByRole("list", { name: "Frames" }).getByRole("img")).toHaveCount(frames);
  // the preview is the sprite read back: one picture a frame, and what's in it
  await expect(maker.getByRole("img", { name: "The sprite as the game draws it" })).toBeVisible({ timeout: 15_000 });
  await expect(maker.locator(".sm-facts")).toContainText(`${frames} frame${frames === 1 ? "" : "s"} · 64 × 64`);
  await expect(maker.getByLabel("Sprite name")).toHaveValue("fire_ball_1");
  return maker;
}

test("Textures & models: Make sprite… makes an animated sprite and puts it in the game", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  const { sent, builds } = await fakeImport(page);
  await page.goto("/tools-dev.html?open=assets");
  await page.getByRole("button", { name: "Make sprite…" }).click();
  const maker = await makeOne(page, 3);
  await expect(maker.getByLabel("Preview speed (frames a second)")).toBeVisible();
  // a smaller size, laid flat, drawn with a see-through color: the preview follows
  await maker.getByLabel("Width", { exact: true }).selectOption("32");
  await maker.getByLabel("How it faces you").selectOption("3");
  await maker.getByLabel("How it's drawn").selectOption("alphatest");
  await expect(maker.getByLabel("See-through color")).toBeVisible();
  await expect(maker.locator(".sm-facts")).toContainText("3 frames · 32 × 64");
  expect(builds.at(-1)).toMatchObject({ width: 32, height: 64, orientation: 3, format: "alphatest", fit: "contain", tint: "#0000ff" });
  expect((builds.at(-1)!.frames as string[]).length).toBe(3);
  await maker.getByLabel("Sprite name").fill("qa_fire");
  await maker.getByRole("button", { name: "Make sprite" }).click();
  await expect(maker).toHaveCount(0);
  await expect(page.locator(".toast", { hasText: "qa_fire.spr is in your game now." })).toBeVisible();
  expect(sent).toHaveLength(1);
  expect(sent[0]).toMatchObject({ kind: "sprite", name: "qa_fire.spr" });
  // what was sent is a real sprite: IDSP, version 2
  const head = Buffer.from(sent[0].data as string, "base64");
  expect(head.subarray(0, 4).toString()).toBe("IDSP");
  expect(head.readInt32LE(4)).toBe(2);
  expect(problems).toEqual([]);
});

const ID = `pw_sprite_${Date.now() % 100000}`;
test("a sprite picker (Objective Marker): Make sprite… selects the new sprite", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  await page.addInitScript(() => {
    localStorage.setItem("ls.studio.panels", JSON.stringify({ left: false, right: true, minimap: false }));
    localStorage.setItem("ls.studio.tour", JSON.stringify({ offered: true, never: false }));
  });
  const tpl = await (await page.request.get("/src/views/tools/studio/templates/team-dm.graph.json")).json();
  tpl.mode.id = ID;
  tpl.mode.name = "Sprite check";
  tpl.nodes.push({ id: "mk", type: "fx.marker", pos: { x: 0, y: -260 }, params: { sprite: "sprites/arrow1.spr" } });
  expect((await (await page.request.post(`/api/tools/studio/graph/${ID}`, { data: { graph: tpl }, headers: H })).json()).ok).toBeTruthy();
  const { sent } = await fakeImport(page);
  await page.goto(`/tools-dev.html?open=studio:${ID}`);
  const node = page.locator('.react-flow__node[data-id="mk"]');
  await node.waitFor({ state: "attached", timeout: 20_000 });
  // (picked from the Problems list, which selects it: no click on a canvas that may still be moving)
  await page.getByRole("region", { name: "Results" }).getByRole("button", { name: "Objective Marker" }).first().click();
  const field = page.locator(".ai-field").first();
  await expect(field.locator("input").first()).toHaveValue("sprites/arrow1.spr");
  await field.getByRole("button", { name: "Choose…" }).click();
  await page.getByRole("dialog", { name: "Choose a sprite" }).getByRole("button", { name: "Make sprite…" }).click();
  const maker = await makeOne(page, 1);
  // one frame: no speed slider
  await expect(maker.getByLabel("Preview speed (frames a second)")).toHaveCount(0);
  await maker.getByRole("button", { name: "Make sprite" }).click();
  await expect(maker).toHaveCount(0);
  await expect(field.locator("input").first()).toHaveValue("sprites/custom/fire_ball_1.spr");
  expect(sent.map((s) => s.name)).toEqual(["fire_ball_1.spr"]);
  await page.request.post(`/api/tools/studio/graph/${ID}/delete`, { headers: H }).catch(() => undefined);
  expect(problems).toEqual([]);
});
