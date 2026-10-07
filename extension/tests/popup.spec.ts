import { type Page, expect, test } from "@playwright/test";

type Config = Record<string, unknown>;

const bookmark = (over: Record<string, unknown> = {}) => ({
  url: "https://ui.shadcn.com/docs",
  title: "shadcn/ui",
  description: "Components",
  tags: ["react"],
  created_ms: Date.UTC(2026, 0, 15),
  updated_ms: Date.UTC(2026, 0, 15),
  deleted: false,
  ...over,
});

async function open(page: Page, config: Config) {
  await page.addInitScript((c) => {
    (window as unknown as { __config: Config }).__config = c;
  }, config);
  await page.goto("/tests/harness.html");
  await page.waitForFunction(() => (window as unknown as { __ready?: boolean }).__ready);
}

const native = (page: Page) =>
  page.evaluate(() => (window as unknown as { __native: Record<string, unknown>[] }).__native);

const shadcnTab = {
  tab: { id: 7, url: "https://ui.shadcn.com/docs", title: "Docs" },
  scrape: { title: "Introduction - shadcn/ui", description: "Beautiful components" },
};

test("fresh save sends a merge add without fetch", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { add: { ok: true, result: { created: true, bookmark: bookmark() } } },
  });
  await expect(page.locator("#title")).toHaveValue("Introduction - shadcn/ui");
  await expect(page.locator("#description")).toHaveValue("Beautiful components");
  const tags = page.locator("#tag-input");
  await expect(tags).toBeFocused();
  await tags.pressSequentially("UI library");
  await tags.press("Enter");
  await tags.pressSequentially("react");
  await page.locator("#save").click();
  await expect(page.locator("#save-status")).toHaveText("✓ Saved");
  const add = (await native(page)).find((r) => r.type === "add");
  expect(add).toEqual({
    type: "add",
    url: "https://ui.shadcn.com/docs",
    title: "Introduction - shadcn/ui",
    description: "Beautiful components",
    tags: ["react", "ui library"],
    fetch: false,
    merge: true,
    origin: "extension",
  });
  expect((await native(page)).map((r) => r.type).sort()).toEqual(["add", "get", "get", "tags"]);
  await page.waitForFunction(() => (window as unknown as { __closed: boolean }).__closed);
  const badge = await page.evaluate(() => (window as unknown as { __badge: unknown }).__badge);
  expect(badge).toEqual({ text: "✓", tabId: 7 });
});

test("Ctrl+Enter saves once", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { add: { ok: true, result: { created: true, bookmark: bookmark() } } },
  });
  await page.locator("#tag-input").press("Control+Enter");
  await expect(page.locator("#save-status")).toHaveText("✓ Saved");
  expect((await native(page)).filter((r) => r.type === "add")).toHaveLength(1);
});

test("already saved shows the banner and updates tags", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: {
      get: { ok: true, result: bookmark() },
      update: { ok: true, result: bookmark({ tags: ["react", "ui"] }) },
    },
  });
  await expect(page.locator("#saved-banner")).toContainText("Already saved on");
  await expect(page.locator(".chip")).toHaveText(["react×"]);
  await expect(page.locator("#save")).toHaveText("Update tags");
  await page.locator("#tag-input").pressSequentially("ui,");
  await page.locator("#save").click();
  await expect(page.locator("#save-status")).toHaveText("✓ Saved");
  const sent = await native(page);
  expect(sent.find((r) => r.type === "add")).toBeUndefined();
  expect(sent.find((r) => r.type === "update")).toEqual({
    type: "update",
    url: "https://ui.shadcn.com/docs",
    tags: ["react", "ui"],
    origin: "extension",
  });
});

test("tombstone is treated as fresh", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { get: { ok: true, result: bookmark({ deleted: true }) } },
  });
  await expect(page.locator("#save")).toHaveText("Save");
  await expect(page.locator("#saved-banner")).toBeHidden();
});

test("tag editor: comma, Enter, spaces, Backspace, suggestions", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: {
      tags: {
        ok: true,
        result: [
          { tag: "rust", count: 3 },
          { tag: "react", count: 9 },
        ],
      },
    },
    local: { recentTags: ["recent one"] },
  });
  const input = page.locator("#tag-input");
  const chips = page.locator(".chips .chip");
  await input.pressSequentially("Dev Tools,");
  await expect(chips).toHaveText(["dev tools×"]);
  await input.pressSequentially("ui library");
  await input.press("Enter");
  await expect(chips).toHaveText(["dev tools×", "ui library×"]);
  await input.pressSequentially("dev tools,");
  await expect(chips).toHaveCount(2);
  await input.press("Backspace");
  await expect(chips).toHaveText(["dev tools×"]);
  await input.pressSequentially("r");
  await expect(page.locator(".suggestions li")).toHaveText(["recent one", "react", "rust"]);
  await input.pressSequentially("ea");
  await input.press("Tab");
  await expect(chips).toHaveText(["dev tools×", "react×"]);
  await expect(input).toHaveValue("");
  await input.pressSequentially("ru");
  await page.locator(".suggestions li", { hasText: "rust" }).click();
  await expect(chips).toHaveText(["dev tools×", "react×", "rust×"]);
});

test("host missing message", async ({ page }) => {
  await open(page, { ...shadcnTab, nativeError: "Specified native messaging host not found." });
  await expect(page.locator("#save-error")).toContainText("native host is not installed");
  await expect(page.locator("#save")).toBeEnabled();
});

test("daemon down message", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { get: { ok: false, error: { code: "daemon_unavailable", message: "x" } } },
  });
  await expect(page.locator("#save-error")).toContainText("systemctl --user start wobookd");
});

test("hook rejection is shown verbatim and keeps the form", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: {
      add: { ok: false, error: { code: "hook_rejected", message: "blocked by policy" } },
    },
  });
  await page.locator("#tag-input").pressSequentially("keep,");
  await page.locator("#save").click();
  await expect(page.locator("#save-error")).toHaveText("blocked by policy");
  await expect(page.locator("#save")).toBeEnabled();
  await expect(page.locator(".chips .chip")).toHaveText(["keep×"]);
  await expect(page.locator("#title")).toHaveValue("Introduction - shadcn/ui");
});

test("invalid url shows inline", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { add: { ok: false, error: { code: "invalid_url", message: "bad url" } } },
  });
  await page.locator("#save").click();
  await expect(page.locator("#url-error")).toHaveText("bad url");
});

test("internal page disables Save", async ({ page }) => {
  await open(page, { tab: { id: 1, url: "about:addons", title: "Add-ons" }, scrape: "throw" });
  await expect(page.locator("#save")).toBeDisabled();
  await expect(page.locator("#save-status")).toHaveText("This page cannot be bookmarked");
  expect(await native(page)).toEqual([]);
});

test("unscriptable page falls back to the tab title", async ({ page }) => {
  await open(page, {
    tab: { id: 1, url: "https://addons.mozilla.org/", title: "AMO" },
    scrape: "throw",
  });
  await expect(page.locator("#title")).toHaveValue("AMO");
  await expect(page.locator("#save")).toBeEnabled();
});

const other = bookmark({ url: "https://example.org/x", title: "Example", tags: [] });
// Haystack "shadcn/ui\nhttps://ui.shadcn.com/docs\nComponents\nreact".
const hit = {
  bookmark: bookmark(),
  score: 10,
  indices: [0, 1, 3, 4, 7, 8, 18, 19, 37, 40],
  segments: { title: [0, 9], url: [10, 36], description: [37, 47], tags: [48, 53] },
};
const searchConfig = {
  ...shadcnTab,
  responses: {
    list: { ok: true, result: [bookmark(), other] },
    search: {
      ok: true,
      result: [
        hit,
        {
          bookmark: other,
          score: 1,
          indices: [],
          segments: { title: [0, 7], url: [8, 29], description: [30, 30], tags: [31, 31] },
        },
      ],
    },
  },
};

test("search highlights title and url only", async ({ page }) => {
  await open(page, searchConfig);
  await page.locator("#tab-search").click();
  await expect(page.locator(".result")).toHaveCount(2);
  await page.locator("#query").fill("shcn ui");
  await expect(page.locator(".result").first().locator("mark").first()).toBeVisible();
  const first = page.locator(".result").first();
  await expect(first.locator(".result-title mark")).toHaveText(["sh", "dc", "ui"]);
  await expect(first.locator(".result-url mark")).toHaveText(["ui"]);
  await expect(first.locator(".result-url")).toHaveText("ui.shadcn.com/docs");
  await expect(first.locator(".result-tags mark")).toHaveCount(0);
  const sent = await native(page);
  expect(sent.find((r) => r.type === "search")).toEqual({
    type: "search",
    query: "shcn ui",
    limit: 50,
  });
  expect(sent.find((r) => r.type === "list")).toEqual({ type: "list", limit: 20 });
  const stored = await page.evaluate(() => (window as unknown as { __local: Config }).__local);
  expect(stored.lastTab).toBe("search");
});

test("Down then Enter opens the second hit", async ({ page }) => {
  await open(page, searchConfig);
  await page.locator("#tab-search").click();
  await page.locator("#query").fill("ex");
  await expect(page.locator(".result")).toHaveCount(2);
  await page.locator("#query").press("ArrowDown");
  await page.locator("#query").press("Enter");
  await page.waitForFunction(() => (window as unknown as { __closed: boolean }).__closed);
  const created = await page.evaluate(
    () => (window as unknown as { __created: unknown }).__created,
  );
  expect(created).toEqual([{ url: "https://example.org/x" }]);
});

test("click copies the url", async ({ page }) => {
  await open(page, searchConfig);
  await page.locator("#tab-search").click();
  await page.locator(".result").first().click();
  await expect(page.locator(".result").first()).toHaveClass(/copied/);
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    "https://ui.shadcn.com/docs",
  );
  await expect(page.locator(".result").first()).not.toHaveClass(/copied/, { timeout: 2000 });
});

test("no results shows Nothing matches", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { search: { ok: true, result: [] }, list: { ok: true, result: [] } },
  });
  await page.locator("#tab-search").click();
  await page.locator("#query").fill("zzz");
  await expect(page.locator("#empty")).toHaveText("Nothing matches");
  await expect(page.locator("#search-error")).toBeHidden();
});

test("daemon down during search", async ({ page }) => {
  await open(page, {
    ...shadcnTab,
    responses: { list: { ok: false, error: { code: "daemon_unavailable", message: "x" } } },
  });
  await page.locator("#tab-search").click();
  await expect(page.locator("#search-error")).toContainText("wobookd is not running");
});
