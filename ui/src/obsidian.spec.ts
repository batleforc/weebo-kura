import { describe, expect, it } from "vitest";

import { setConfig } from "@/config";
import { deviceOf, pluginBuild, setupLink } from "@/obsidian";

describe("the plugin offer", () => {
  it("links the plugin to this server and a vault, or not at all", () => {
    setConfig({ issuer: "i", discovery: "d", clientId: "kura-web" });
    expect(setupLink("https://kura.example.org")).toBeNull();
    setConfig({
      issuer: "i",
      discovery: "d",
      clientId: "kura-web",
      plugin: {
        clientId: "kura-plugin",
        issuer: "https://auth.example.org/application/o/kura-plugin/",
      },
    });
    expect(
      setupLink("https://kura.example.org", { id: "v1", name: "my notes" }),
    ).toBe(
      "obsidian://kura-setup?server=https%3A%2F%2Fkura.example.org&issuer=https%3A%2F%2Fauth.example.org%2Fapplication%2Fo%2Fkura-plugin%2F&clientId=kura-plugin&kuraVault=v1&kuraVaultName=my%20notes",
    );
    setConfig(null);
  });

  it("reads the build's plugin.json, and says unavailable on anything else", async () => {
    const answer = (body: unknown, ok = true) =>
      (async () => ({ ok, json: async () => body })) as unknown as typeof fetch;
    expect(
      await pluginBuild(
        answer({
          available: true,
          version: "0.1.0",
          file: "kura-sync.zip",
          bytes: 10,
        }),
      ),
    ).toEqual({
      available: true,
      version: "0.1.0",
      file: "kura-sync.zip",
      bytes: 10,
    });
    expect(await pluginBuild(answer({ available: false }))).toEqual({
      available: false,
    });
    expect(await pluginBuild(answer("<html>", true))).toEqual({
      available: false,
    });
    expect(await pluginBuild(answer(null, false))).toEqual({
      available: false,
    });
  });

  it("opens on the steps of the device in hand", () => {
    expect(deviceOf("Mozilla/5.0 (Linux; Android 15; Pixel 9)", 5)).toBe(
      "android",
    );
    expect(
      deviceOf("Mozilla/5.0 (iPhone; CPU iPhone OS 19_0 like Mac OS X)", 5),
    ).toBe("ios");
    expect(deviceOf("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)", 5)).toBe(
      "ios",
    );
    expect(deviceOf("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)", 0)).toBe(
      "desktop",
    );
    expect(deviceOf("Mozilla/5.0 (X11; Linux x86_64)", 0)).toBe("desktop");
  });
});
