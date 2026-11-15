// Plugin Studio: Community doesn't take plugins, so the save bar says how to share one by hand,
// with Export… (the .sma and its own .inc files) and Import… (never over a file that's there:
// a taken name asks for another). Every plugin answer is faked with page.route.
import { expect, test, type Route } from "@playwright/test";

const json = (r: Route, body: unknown) => r.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(body) });
const SRC = "#include <amxmodx>\npublic plugin_init() {}\n";

test("Plugin Studio: share line, Export and Import with a taken name", async ({ page }) => {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  const posts: { url: string; body: Record<string, unknown> }[] = [];
  let files = [{ name: "qa.sma", size: SRC.length, modified: 1763100000, readOnly: false }];
  await page.route("**/api/tools/plugins**", async (r) => {
    const url = r.request().url();
    const body = r.request().method() === "POST" ? (r.request().postDataJSON() as Record<string, unknown>) : {};
    if (r.request().method() === "POST") posts.push({ url, body });
    if (/\/plugins\/words/.test(url)) return json(r, { includes: [], symbols: [], unread: [] });
    if (/\/plugins\/log/.test(url)) return json(r, { cursor: "x", lines: [] });
    if (/\/plugins\/file\?/.test(url)) return json(r, { name: new URL(url).searchParams.get("name"), folder: "mine", text: SRC, encoding: "utf8", readOnly: null, reset: null, resetHint: null, history: "" });
    if (/\/plugins\/export/.test(url)) return json(r, { ok: true, saved: ["qa.sma", "helpers.inc"], message: "Saved qa.sma, helpers.inc." });
    if (/\/plugins\/import\/pick/.test(url))
      return json(r, { id: "p1", files: [{ name: "qa.sma", problem: null, taken: true }, { name: "con.inc", problem: "con.inc: Windows keeps that name for a device (like CON or NUL). Pick another name.", taken: false }] });
    if (/\/plugins\/import/.test(url)) {
      files = [...files, { name: "qa_new.sma", size: 10, modified: 1763100001, readOnly: false }];
      return json(r, { ok: true, imported: ["qa_new.sma"], first: "qa_new.sma", message: "Imported qa_new.sma." });
    }
    if (/\/plugins$|\/plugins\?/.test(url))
      return json(r, {
        game: "cstrike",
        compiler: true,
        compilerProblem: null,
        running: false,
        trying: [],
        folders: [
          { id: "mine", label: "My plugins", path: "/x/mine", files, hidden: 0 },
          { id: "game", label: "Game's plugins", path: "/x/scripting", files: [], hidden: 0 },
          { id: "include", label: "Include files", path: "/x/include", files: [], hidden: 0 },
        ],
      });
    return json(r, { ok: true });
  });
  await page.goto("/tools-dev.html?open=plugins:mine/qa.sma");
  const note = page.locator(".ps-share-note");
  await expect(note).toContainText("Community doesn't take plugins. To share one, post its .sma on GameBanana or AlliedModders yourself", { timeout: 20_000 });
  await expect(page.getByRole("button", { name: "Export…" })).toBeEnabled();

  await page.getByRole("button", { name: "Export…" }).click();
  await expect(page.locator(".toast", { hasText: "Saved qa.sma, helpers.inc." })).toBeVisible();
  expect(posts.find((p) => p.url.includes("/export"))!.body).toEqual({ folder: "mine", name: "qa.sma" });

  await page.getByRole("button", { name: "Import…" }).click();
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("There's already a qa.sma in My plugins");
  const box = dialog.getByRole("textbox");
  await box.fill("qa");
  await dialog.getByRole("button", { name: "Import as this" }).click();
  await expect(dialog).toContainText("There's already a file called qa.sma.");
  await box.fill("nul");
  await dialog.getByRole("button", { name: "Import as this" }).click();
  await expect(dialog).toContainText("Windows keeps that name");
  await box.fill("qa_new");
  await dialog.getByRole("button", { name: "Import as this" }).click();
  await expect(page.locator(".toast", { hasText: "Imported qa_new.sma." })).toBeVisible();
  expect(posts.find((p) => /\/import$/.test(p.url))!.body).toEqual({ id: "p1", renames: { "qa.sma": "qa_new.sma" } });
  // the first imported file opens
  await expect(page.locator(".tl-actionbar")).toContainText("qa_new.sma");

  // Community link goes to the Game Mode Studio
  await note.getByRole("button", { name: "Game Mode Studio" }).click();
  await expect(page.getByRole("heading", { name: "Game Mode Studio", level: 2 })).toBeVisible();
  expect(problems).toEqual([]);
});
