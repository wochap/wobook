import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "tests",
  use: {
    // Full Chromium in new headless mode (no separate headless shell download).
    channel: "chromium",
    baseURL: "http://127.0.0.1:4178",
    permissions: ["clipboard-read", "clipboard-write"],
  },
  webServer: {
    command: "node tests/serve.mjs 4178",
    url: "http://127.0.0.1:4178/tests/harness.html",
  },
});
