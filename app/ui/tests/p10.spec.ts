// P10: the spray composer, Crosshair & config and Demos (docs/TOOLS-NOTES.md, P10).
// Same harness as tools.spec.ts (tools-dev.html against the real game files). Nothing here
// installs a spray or saves a config: designs are saved in the dev server's app folder (a
// temp folder) and deleted again.
import { expect, test, type Page } from "@playwright/test";

const IGNORED = [/status of 404/, /Download the React DevTools/];

function watch(page: Page) {
  const problems: string[] = [];
  page.on("console", (m) => {
    if (m.type() === "error" && !IGNORED.some((r) => r.test(m.text()))) problems.push(`console: ${m.text()}`);
  });
  page.on("pageerror", (e) => problems.push(`crash: ${e.message}`));
  return problems;
}

/** nothing in the content sticks out past the window */
async function fits(page: Page) {
  const wide = await page.evaluate(() => {
    const w = document.documentElement.clientWidth;
    return [...document.querySelectorAll<HTMLElement>(".content *")]
      .filter((el) => el.offsetParent && el.getBoundingClientRect().right > w + 1 && getComputedStyle(el).position !== "fixed")
      .slice(0, 3)
      .map((el) => `${el.tagName.toLowerCase()}.${[...el.classList].join(".")}`);
  });
  expect(wide, "something sticks out past the window").toEqual([]);
}

/** a small PNG made in the page (a red square on clear) */
async function pngFile(page: Page) {
  const b64 = await page.evaluate(() => {
    const c = document.createElement("canvas");
    c.width = c.height = 64;
    const g = c.getContext("2d")!;
    g.fillStyle = "#e02020";
    g.fillRect(8, 8, 48, 48);
    return c.toDataURL("image/png").split(",")[1];
  });
  return { name: "red-square.png", mimeType: "image/png", buffer: Buffer.from(b64, "base64") };
}

test.describe("spray composer", () => {
  test("layers, handles, undo, the game's look, save and open", async ({ page }, info) => {
    const problems = watch(page);
    await page.goto("/tools-dev.html?open=spray");
    const stage = page.locator(".sc-stage svg");
    await expect(stage).toBeVisible({ timeout: 20_000 });

    // starts from the name-tag template: two layers, and the game's look comes back
    const layers = page.getByRole("listbox", { name: "Layers, top first" }).getByRole("option");
    await expect(layers).toHaveCount(2);
    await expect(page.locator(".sc-wall img.spray-img")).toBeVisible({ timeout: 15_000 });
    await expect(page.locator(".sc-view figcaption").first()).toContainText("96×96 in game");

    // add a sticker and words, pick the words by clicking the canvas
    await page.getByRole("button", { name: "Add a star sticker" }).click();
    await page.getByRole("button", { name: "Words", exact: true }).click();
    await expect(layers).toHaveCount(4);
    await expect(page.getByRole("region", { name: "Words settings" })).toBeVisible();

    // a picture from a file
    await page.locator('.sc-page input[type="file"]').setInputFiles(await pngFile(page));
    await expect(layers).toHaveCount(5);
    await expect(page.getByRole("region", { name: "Picture settings" })).toBeVisible();

    // drag it: one undo step, Undo puts it back, Redo again
    await page.locator(".sc-stage").evaluate((el) => el.scrollIntoView({ block: "center" }));
    const before = await page.locator(".sc-box polygon").getAttribute("points");
    const box = (await stage.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 60, box.y + box.height / 2 + 30, { steps: 6 });
    await page.mouse.up();
    const after = await page.locator(".sc-box polygon").getAttribute("points");
    expect(after).not.toEqual(before);
    await page.keyboard.press("ControlOrMeta+z");
    await expect(page.locator(".sc-box polygon")).toHaveAttribute("points", before!);
    await page.keyboard.press("ControlOrMeta+Shift+z");
    await expect(page.locator(".sc-box polygon")).toHaveAttribute("points", after!);

    // turn it with the knob
    const knob = (await page.locator(".sc-knob").boundingBox())!;
    await page.mouse.move(knob.x + knob.width / 2, knob.y + knob.height / 2);
    await page.mouse.down();
    await page.mouse.move(knob.x + 120, knob.y + 80, { steps: 6 });
    await page.mouse.up();
    await expect(page.getByRole("slider", { name: "Turn" })).not.toHaveValue("0");

    // hide, lock, duplicate, delete
    await page.keyboard.press("ControlOrMeta+d");
    await expect(layers).toHaveCount(6);
    await page.keyboard.press("Delete");
    await expect(layers).toHaveCount(5);
    await page.getByRole("button", { name: /^Hide / }).first().click();
    await expect(page.locator(".sc-layer.hidden")).toHaveCount(1);

    // one colour: the game's look follows
    await page.getByRole("group", { name: "Colours" }).getByRole("button", { name: "One colour" }).click();
    await expect(page.locator(".sc-view figcaption").first()).toContainText("one colour", { timeout: 10_000 });
    await fits(page);
    await page.screenshot({ path: info.outputPath("spray-composer.png"), fullPage: true });

    // save, then it's in Designs & templates and opens again
    const name = `p10 test ${Date.now()}`;
    await page.getByLabel("Design name").fill(name);
    await page.getByRole("region", { name: "Save" }).getByRole("button", { name: "Save design" }).click();
    await expect(page.getByText(`Saved “${name}”.`)).toBeVisible();
    await page.getByRole("button", { name: "Designs & templates…" }).click();
    const sheet = page.getByRole("dialog", { name: "Designs and templates" });
    await expect(sheet.getByRole("button", { name: `Open ${name}` })).toBeVisible();
    await page.screenshot({ path: info.outputPath("spray-designs.png") });
    await sheet.getByRole("button", { name: "Name tag" }).click();
    await expect(layers).toHaveCount(2);
    await page.getByRole("button", { name: "Designs & templates…" }).click();
    await sheet.getByRole("button", { name: `Open ${name}` }).click();
    await expect(layers).toHaveCount(5);
    await expect(page.getByLabel("Design name")).toHaveValue(name);
    // and delete it again
    await page.getByRole("button", { name: "Designs & templates…" }).click();
    await sheet.getByRole("button", { name: `Delete ${name}` }).click();
    await page.getByRole("button", { name: "Delete", exact: true }).last().click();
    await expect(sheet.getByRole("button", { name: `Open ${name}` })).toHaveCount(0);
    expect(problems).toEqual([]);
  });
});
