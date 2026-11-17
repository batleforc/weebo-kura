import { describe, expect, it } from "vitest";

import { config, loadConfig, setConfig } from "./config";
import { json } from "./test-utils";

describe("config.json", () => {
  it("is read once and refused when incomplete", async () => {
    setConfig(null);
    expect(() => config()).toThrow();
    const ok = {
      issuer: "https://a/",
      discovery: "https://a/o/",
      clientId: "kura-web",
    };
    await expect(loadConfig(async () => json(ok))).resolves.toEqual(ok);
    expect(config().clientId).toBe("kura-web");
    await expect(
      loadConfig(async () => json({ issuer: "https://a/" })),
    ).rejects.toThrow(/discovery/);
    await expect(loadConfig(async () => json({}, 404))).rejects.toThrow(/404/);
  });
});
