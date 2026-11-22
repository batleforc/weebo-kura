import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      // Obsidian UI glue only: every decision it makes lives in the tested modules.
      exclude: ["src/wasm/**", "src/main.ts", "src/settings.ts", "src/modals.ts"],
      thresholds: { lines: 86, functions: 86, branches: 86, statements: 86 },
    },
  },
});
