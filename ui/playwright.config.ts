import { defineConfig } from "@playwright/test";

// The reader's proof against a real browser (RFC 0005 §6.P), driven by
// tests/heavy/reader.sh against the local stack; not part of `pnpm test`.
export default defineConfig({
  testDir: "e2e",
  timeout: 60_000,
  use: {
    baseURL: process.env.KURA_DEV_SERVER ?? "http://127.0.0.1:58080",
    headless: true,
    // The dev Authentik is served with the local CA of dev/certs, which this
    // throwaway browser does not trust (test stack only).
    ignoreHTTPSErrors: true,
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
