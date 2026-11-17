import { defineConfig } from "@hey-api/openapi-ts";

// The committed specification, never a running server: `task dump-spec`
// exports it from the utoipa annotations and `task ui:client` regenerates this
// client from it, so the two cannot drift unnoticed (`task ui:client:check`).
export default defineConfig({
  input: "../docs/api/openapi.json",
  output: { path: "src/api/generated", postProcess: [] },
  plugins: [
    {
      name: "@hey-api/client-axios",
      runtimeConfigPath: "./src/api/runtime.ts",
    },
    { name: "@hey-api/sdk" },
    { name: "@hey-api/typescript", enums: "javascript" },
  ],
});
