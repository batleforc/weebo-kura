import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import { setConfig } from "@/config";
import { i18n } from "@/i18n";
import { json } from "@/test-utils";

vi.mock("@/api/generated/sdk.gen", () => ({
  listVaults: vi.fn(async () => ({
    data: [
      {
        id: "v1",
        name: "notes",
        tenant: "dev",
        role: "editor",
        managedBy: "config",
        configSync: [],
      },
      {
        id: "v2",
        name: "campaign",
        tenant: "jdr",
        role: "viewer",
        managedBy: "config",
        configSync: [],
        paths: ["Sessions/"],
      },
    ],
  })),
  registerDevice: vi.fn(async () => ({ data: { id: "this-browser" } })),
}));

import HelpPage from "./HelpPage.vue";

async function open(plugin: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => json(plugin)),
  );
  const router = createRouter({
    history: createMemoryHistory("/app/"),
    routes: [{ path: "/:p(.*)", component: { template: "<p/>" } }],
  });
  const w = mount(HelpPage, { global: { plugins: [router, i18n] } });
  await flushPromises();
  return w;
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.setItem("kura:device", "this-browser");
  i18n.global.locale.value = "en";
  setConfig({
    issuer: "i",
    discovery: "d",
    clientId: "kura-web",
    plugin: {
      clientId: "kura-plugin",
      issuer: "https://auth.example.org/application/o/kura-plugin/",
    },
  });
});

describe("help", () => {
  it("offers the plugin this server carries, and a setup link per whole vault", async () => {
    const w = await open({
      available: true,
      version: "0.1.0",
      file: "kura-sync.zip",
      bytes: 321541,
    });
    expect(w.find("[data-test=download]").attributes("href")).toBe(
      "/app/kura-sync.zip",
    );
    expect(w.text()).toContain("version 0.1.0");
    // A phone gets its own steps, with the folder where the plugin goes there.
    await w.find("[data-device=android]").trigger("click");
    expect(w.find("[data-steps=android]").text()).toContain(
      "Documents/<your vault>/.obsidian/plugins/weebo-kura/",
    );
    expect(w.find("[data-device=android]").attributes("aria-pressed")).toBe(
      "true",
    );
    expect(w.find("[data-setup=notes]").attributes("href")).toMatch(
      /^obsidian:\/\/kura-setup\?server=.*&kuraVault=v1&kuraVaultName=notes$/,
    );
    // A folder-limited reader reads in the browser: no setup link.
    expect(w.find("[data-setup=campaign]").exists()).toBe(false);
  });

  it("says so when the server was built without the plugin", async () => {
    const w = await open({ available: false });
    expect(w.find("[data-test=download]").exists()).toBe(false);
    expect(w.find("[data-test=no-plugin]").text()).toContain(
      "built without the plugin",
    );
  });
});
