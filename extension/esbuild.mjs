// Bundles the worker and the popup into dist/firefox and dist/chromium.
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");
const targets = ["firefox", "chromium"];

function copyStatic() {
  const manifest = JSON.parse(readFileSync("manifest.json", "utf8"));
  for (const target of targets) {
    const out = `dist/${target}`;
    mkdirSync(`${out}/popup`, { recursive: true });
    cpSync("icons", `${out}/icons`, { recursive: true });
    cpSync("src/popup/popup.html", `${out}/popup/popup.html`);
    cpSync("src/popup/popup.css", `${out}/popup/popup.css`);
    const m = structuredClone(manifest);
    if (target === "chromium") {
      m.browser_specific_settings = undefined;
    }
    writeFileSync(`${out}/manifest.json`, `${JSON.stringify(m, null, 2)}\n`);
  }
}

rmSync("dist", { recursive: true, force: true });
copyStatic();
const contexts = await Promise.all(
  targets.map((target) =>
    esbuild.context({
      entryPoints: { background: "src/background.ts", "popup/popup": "src/popup/popup.ts" },
      bundle: true,
      format: "iife",
      target: ["firefox121", "chrome120"],
      outdir: `dist/${target}`,
      logLevel: "info",
    }),
  ),
);
if (watch) {
  await Promise.all(contexts.map((c) => c.watch()));
} else {
  await Promise.all(contexts.map((c) => c.rebuild()));
  await Promise.all(contexts.map((c) => c.dispose()));
}
