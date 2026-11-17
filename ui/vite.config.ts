import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

// Served by kura-server under /app (RFC 0003 §6.1), same origin as the API.
export default defineConfig({
  base: "/app/",
  plugins: [vue(), tailwindcss()],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  server: {
    // `pnpm dev` against `task dev:run`: the API and config.json come from it.
    proxy: {
      "/v1": "http://127.0.0.1:58080",
      "/app/config.json": "http://127.0.0.1:58080",
    },
  },
  build: { outDir: "dist", assetsDir: "assets" },
  test: {
    environment: "jsdom",
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
      thresholds: { lines: 80 },
    },
  },
});
