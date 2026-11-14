// Bundles the plugin into main.js with the WASM core embedded (§6.8: a
// community plugin ships main.js, manifest.json and styles.css only), and
// enforces the 3 MB budget of decision 54.
import { build } from "esbuild";
import { statSync } from "node:fs";

const BUDGET = 3 * 1024 * 1024;

await build({
  entryPoints: ["src/main.ts"],
  bundle: true,
  format: "cjs",
  target: "es2022",
  platform: "browser",
  outfile: "main.js",
  external: ["obsidian", "electron", "@codemirror/*", "@lezer/*"],
  loader: { ".wasm": "binary" },
  minify: true,
  legalComments: "none",
  logLevel: "info",
});

const size = statSync("main.js").size;
console.log(`main.js: ${size} bytes (budget ${BUDGET})`);
if (size > BUDGET) {
  console.error("main.js is over the 3 MB budget (decision 54)");
  process.exit(1);
}
