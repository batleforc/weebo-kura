import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";

/** Runs public/sw.js against a fake worker scope; returns its handlers. */
function load() {
  const handlers: Record<string, (event: unknown) => void> = {};
  const self = {
    location: new URL("https://kura.example.org/app/sw.js"),
    addEventListener: (name: string, fn: (event: unknown) => void) => {
      handlers[name] = fn;
    },
  };
  const module = { exports: {} as { cacheable: (u: URL) => boolean } };
  new Function(
    "self",
    "module",
    "caches",
    "fetch",
    readFileSync("public/sw.js", "utf8"),
  )(self, module, { open: () => new Promise(() => undefined) }, fetch);
  return { handlers, cacheable: module.exports.cacheable };
}

describe("service worker", () => {
  // T14, `service_worker_stores_no_api_response`.
  it("keeps the shell only: never the API, config.json or another origin", () => {
    const { cacheable } = load();
    const at = (p: string) => new URL(p, "https://kura.example.org");
    expect(cacheable(at("/app/"))).toBe(true);
    expect(cacheable(at("/app/assets/index-1a2b.js"))).toBe(true);
    for (const never of [
      "/v1/vaults",
      "/v1/me",
      "/app/config.json",
      "/app/assets/x.js?token=1",
      "https://auth.example.org/app/assets/x.js",
    ]) {
      expect(cacheable(at(never)), never).toBe(false);
    }
  });

  it("leaves API requests to the network, untouched", () => {
    const { handlers } = load();
    const respondWith = vi.fn();
    for (const [url, method, mode] of [
      ["https://kura.example.org/v1/vaults", "GET", "cors"],
      ["https://kura.example.org/app/assets/a.js", "POST", "cors"],
      ["https://kura.example.org/v1/devices", "GET", "navigate"],
    ]) {
      handlers.fetch?.({ request: { url, method, mode }, respondWith });
    }
    expect(respondWith).not.toHaveBeenCalled();
    handlers.fetch?.({
      request: {
        url: "https://kura.example.org/app/devices",
        method: "GET",
        mode: "navigate",
      },
      respondWith,
    });
    expect(respondWith).toHaveBeenCalledTimes(1);
  });
});
