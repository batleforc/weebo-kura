import { createHash } from "node:crypto";
import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

import { buildChecks } from "./build-checks.ts";
import { pluginZip } from "./plugin-zip.ts";
import { BRIDGE } from "./src/reader/bridge.ts";

/**
 * `dist/frame-csp.txt`: the CSP source of the note frame's one script
 * (RFC 0005 §6.5). kura-server reads it from `ui.dir` and adds it to
 * `script-src`; without it the bridge does not run and notes keep a fixed
 * height, nothing worse.
 */
function frameHash(): Plugin {
  return {
    name: "kura:frame-csp",
    apply: "build",
    generateBundle() {
      const hash = createHash("sha256").update(BRIDGE).digest("base64");
      this.emitFile({
        type: "asset",
        fileName: "frame-csp.txt",
        source: `'sha256-${hash}'\n`,
      });
    },
  };
}

// Served by kura-server under /app (RFC 0003 §6.1), same origin as the API.
export default defineConfig({
  base: "/app/",
  plugins: [vue(), tailwindcss(), frameHash(), pluginZip(), buildChecks()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  server: {
    // `pnpm dev` against `task dev:run`: the API and config.json come from it.
    proxy: {
      // `ws`: the reader's event socket (RFC 0005 §7.5).
      "/v1": { target: "http://127.0.0.1:58080", ws: true },
      "/app/config.json": "http://127.0.0.1:58080",
    },
  },
  build: {
    outDir: "dist",
    assetsDir: "assets",
    // Fonts are always files: `font-src 'self'` refuses a `data:` font.
    assetsInlineLimit: (file: string) =>
      /\.woff2?$/.test(file) ? false : undefined,
  },
  test: {
    environment: "jsdom",
    // e2e/ is Playwright's, run against a real browser by tests/heavy/reader.sh.
    exclude: ["e2e/**", "node_modules/**"],
    coverage: {
      provider: "v8",
      // public/sw.js runs under `new Function` in its spec, out of v8 coverage.
      include: ["src/**/*.ts", "src/**/*.vue"],
      exclude: [
        "src/api/generated/**",
        "src/main.ts",
        "src/**/*.spec.ts",
        "src/test-utils.ts",
      ],
      thresholds: { lines: 86 },
    },
  },
});
