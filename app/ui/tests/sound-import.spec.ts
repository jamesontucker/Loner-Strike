// Sounds come in as WAV, MP3, Ogg or FLAC and become a WAV the game plays well (T1fix,
// tools/soundconv.rs): the asset pickers' Import (Creature Studio's sounds) and the Content
// Browser's Import send the "Sound options" picked, and say what the conversion did. The import
// routes are faked; the rest runs against a dev server over a scratch game copy (never the real one).
import { expect, test, type Route } from "@playwright/test";

const json = (r: Route, body: unknown) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });
const MP3 = Buffer.from([0xff, 0xfb, 0x90, 0x64, 0, 0, 0, 0]);

test("a picker's sound Import sends the options and says what the conversion did", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  const bodies: Record<string, unknown>[] = [];
  await page.route("**/api/tools/assets/**", async (r) => {
    const url = r.request().url();
    if (url.includes("/project-files")) return json(r, { files: [] });
    if (url.includes("/import")) {
      const body = r.request().postDataJSON() as Record<string, unknown>;
      bodies.push(body);
      return json(r, { ok: true, path: "sound/custom/growl.wav", use: "custom/growl.wav", files: ["sound/custom/growl.wav"], converted: "converted from 44.1 kHz stereo MP3 to 11 kHz mono WAV, 8-bit" });
    }
    return json(r, { ok: true });
  });
  await page.goto("/tools-dev.html?open=creatures");
  await page.getByRole("listbox", { name: "Creature types" }).getByRole("option", { name: /Zombie/ }).click();
  const sounds = page.locator("details.cs-section").filter({ hasText: "Sounds" });
  if (!(await sounds.evaluate((d) => (d as HTMLDetailsElement).open))) await sounds.locator("summary").click();
  await sounds.getByRole("button", { name: "+ Add" }).first().click();
  const pop = page.getByRole("dialog", { name: /Add a .* sound/ });
  // the options: 11 kHz, 8-bit (remembered for next time)
  await pop.getByText("Sound options").click();
  await pop.getByRole("combobox", { name: "Sample rate" }).selectOption("11025");
  await pop.getByRole("combobox", { name: "Bit depth" }).selectOption("8");
  await expect(pop.getByText("WAV, MP3, Ogg and FLAC become a WAV the game plays well.")).toBeVisible();
  // an MP3 dropped on the picker
  const dt = await page.evaluateHandle(() => {
    const d = new DataTransfer();
    d.items.add(new File([new Uint8Array([0xff, 0xfb, 0x90, 0x64])], "growl.mp3"));
    return d;
  });
  const target = pop;
  await target.dispatchEvent("dragover", { dataTransfer: dt });
  await target.dispatchEvent("drop", { dataTransfer: dt });
  await expect(page.locator(".toast", { hasText: "growl.wav is in your game now: converted from 44.1 kHz stereo MP3 to 11 kHz mono WAV, 8-bit." })).toBeVisible();
  expect(bodies[0]).toMatchObject({ kind: "sound", name: "growl.mp3", sound: { rate: 11025, bits: 8, stereo: false, trim: false } });
  expect(await page.evaluate(() => localStorage.getItem("ls.import.sound"))).toContain("11025");
  await page.getByRole("button", { name: "Undo changes" }).click().catch(() => undefined);
  expect(problems).toEqual([]);
});

test("the Content Browser's Import takes MP3, Ogg and FLAC and lists what each conversion did", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  let body: { files: { name: string }[]; sound: unknown } | null = null;
  await page.route("**/api/tools/editor/content/import", async (r) => {
    body = r.request().postDataJSON();
    return json(r, {
      imported: [{ name: "wind.wav", path: "sound/wind.wav", converted: "converted from 44.1 kHz stereo Ogg to 22 kHz mono WAV" }],
      renamed: [],
      taken: [],
      refused: [],
      message: "Imported 1 file. wind.wav: converted from 44.1 kHz stereo Ogg to 22 kHz mono WAV.",
    });
  });
  await page.addInitScript(() => {
    localStorage.setItem("ls.editor.nav", JSON.stringify({ preset: "unreal", speed: 3 }));
    localStorage.setItem("ls.editor.navHint", "1");
    localStorage.setItem("ls.editor.tour", JSON.stringify({ offered: true, never: true }));
    localStorage.setItem("ls.import.sound", JSON.stringify({ rate: 22050, bits: 16, stereo: true, trim: true }));
  });
  await page.goto("/editor-dev.html");
  await page.getByRole("button", { name: "New map" }).click();
  const leave = page.getByRole("button", { name: "Leave them" });
  if (await leave.waitFor({ timeout: 1500 }).then(() => true, () => false)) await leave.click();
  await page.locator(".ed-pane").first().waitFor();
  await page.locator("[data-dock-strip] [role=tab]", { hasText: /^Content$/ }).first().click();
  const cb = page.locator('section[aria-label="Content Browser"]').first();
  await expect(cb.getByTestId("content-import-input")).toHaveAttribute("accept", /\.mp3.*\.ogg.*\.flac/);
  await cb.getByTestId("content-import-input").setInputFiles([{ name: "wind.ogg", mimeType: "audio/ogg", buffer: MP3 }]);
  const box = cb.locator(".p5-cb-import");
  await expect(box).toContainText("Imported sound/wind.wav (converted from 44.1 kHz stereo Ogg to 22 kHz mono WAV)");
  expect(body!.files[0].name).toBe("wind.ogg");
  expect(body!.sound).toEqual({ rate: 22050, bits: 16, stereo: true, trim: true });
  expect(problems).toEqual([]);
});
