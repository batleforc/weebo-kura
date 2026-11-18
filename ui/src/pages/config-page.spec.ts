import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import * as sdk from "@/api/generated/sdk.gen";
import type { EntryState } from "@/api/generated/types.gen";
import { i18n } from "@/i18n";
import * as content from "@/reader/content";
import { sectionOf, sectionsOf } from "@/vaults";

vi.mock("@/api/generated/sdk.gen", () => ({
  listVaults: vi.fn(),
  me: vi.fn(async () => ({
    data: { subject: "s", platformAdmin: false, tenants: [] },
  })),
  getEntries: vi.fn(),
  getRevisions: vi.fn(async () => ({
    data: {
      items: [
        {
          createdAt: "2026-11-18T10:00:00Z",
          author: { kind: "device", name: "desk-linux" },
        },
      ],
    },
  })),
  registerDevice: vi.fn(async () => ({ data: { id: "dev" } })),
}));
vi.mock("@/reader/content", () => ({ fileBytes: vi.fn() }));

import ConfigPage from "./ConfigPage.vue";

const e = (path: string, blob: string | null, size = 100): EntryState => ({
  entry: path,
  path,
  revision: "r",
  seq: 1,
  size,
  deleted: false,
  ...(blob ? { blob } : {}),
});
const listed = [
  e("snippets/wide.css", "b-snippet"),
  e("plugins/dataview/manifest.json", "b-manifest"),
  e("plugins/dataview/styles.css", "b-styles"),
  e("plugins/dataview/data.json", null),
  e("plugins/dataview/main.js", null, 2_300_000),
  e("plugins/tasks-x/main.js", null),
  e("themes/Big/theme.css", "b-big", 400_000),
];
const bytes: Record<string, string> = {
  "b-snippet":
    "<style>body{background:url(https://evil.example)}</style><img src=x>",
  "b-manifest": '{"name":"Data<b>view</b>","version":"0.5.68"}',
};

async function open(path: string, vault: Record<string, unknown> = {}) {
  vi.mocked(sdk.listVaults).mockResolvedValue({
    data: [
      {
        id: "v1",
        name: "obsidian",
        tenant: "dev",
        role: "editor",
        managedBy: "config",
        configSync: ["plugins", "snippets", "themes"],
        ...vault,
      },
    ],
  } as never);
  const router = createRouter({
    history: createMemoryHistory("/app/"),
    routes: [
      {
        path: "/vaults/:v/config",
        name: "vault-config",
        component: ConfigPage,
      },
    ],
  });
  await router.push(path);
  const w = mount(
    { template: "<RouterView />" },
    { global: { plugins: [router, i18n] } },
  );
  for (let i = 0; i < 6; i += 1) await flushPromises();
  return { w, router };
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.setItem("kura:device", "dev");
  i18n.global.locale.value = "en";
  vi.mocked(sdk.getEntries).mockReset();
  vi.mocked(sdk.getEntries).mockResolvedValue({
    data: { items: listed, next: null },
  } as never);
  vi.mocked(content.fileBytes).mockReset();
  vi.mocked(content.fileBytes).mockImplementation(async (_v, blob) => {
    const text = bytes[blob];
    return text === undefined ? null : new TextEncoder().encode(text);
  });
});

describe("the configuration page", () => {
  it("lists by category and plugin, and never asks for a hidden file", async () => {
    const { w } = await open("/vaults/v1/config");
    const list = w.find("[data-test=list]");
    expect(list.text()).toContain("Snippets");
    expect(list.text()).toContain("wide.css");
    const plugins = w.findAll("[data-test=plugin]").map((p) => p.text());
    expect(plugins[0]).toContain("Data<b>view</b> 0.5.68 · dataview");
    expect(plugins[0]).toContain("data.json");
    expect(plugins[0]).toContain("settings, not shown");
    expect(plugins[0]).toContain("code, not shown");
    expect(plugins[1]).toContain("tasks-x · its manifest is not synced");
    expect(w.find("b").exists()).toBe(false);
    // Hidden files are text, not links; the page only read the manifest.
    expect(w.find('[data-file="plugins/dataview/data.json"]').exists()).toBe(
      false,
    );
    expect(vi.mocked(content.fileBytes).mock.calls.map((c) => c[1])).toEqual([
      "b-manifest",
    ]);
    expect(w.find("[data-test=why-hidden]").exists()).toBe(true);
    expect(w.text()).toContain("Not synced on this vault: Hotkeys");
    expect(vi.mocked(sdk.getEntries).mock.calls[0]?.[0]?.query).toMatchObject({
      space: "config",
      profile: "desktop",
    });
  });

  it("shows a snippet as text, never as style or markup", async () => {
    const { w } = await open(
      "/vaults/v1/config?profile=desktop&file=snippets/wide.css",
    );
    const pre = w.find("[data-test=text]");
    expect(pre.text()).toBe(bytes["b-snippet"]);
    expect(w.find("style").exists()).toBe(false);
    expect(w.find("img").exists()).toBe(false);
    expect(w.text()).toContain("Last edited Nov 18, 2026 by desk-linux");
  });

  it("does not fetch a file over 256 KiB", async () => {
    const { w } = await open("/vaults/v1/config?file=themes/Big/theme.css");
    expect(w.find("[data-test=too-large]").text()).toContain("400 kB");
    expect(
      vi.mocked(content.fileBytes).mock.calls.map((c) => c[1]),
    ).not.toContain("b-big");
  });

  it("asks for the profile in the address", async () => {
    vi.mocked(sdk.getEntries).mockResolvedValue({
      data: { items: [], next: null },
    } as never);
    const { w } = await open("/vaults/v1/config?profile=mobile");
    expect(vi.mocked(sdk.getEntries).mock.calls[0]?.[0]?.query).toMatchObject({
      profile: "mobile",
    });
    expect(
      w.find("[data-test=profile-mobile]").attributes("aria-current"),
    ).toBe("page");
    expect(w.text()).toContain("Nothing synced from this profile yet");
  });

  it("says when the vault syncs no settings, without asking the server", async () => {
    const { w } = await open("/vaults/v1/config", { configSync: [] });
    expect(w.find("[data-test=not-synced]").exists()).toBe(true);
    expect(sdk.getEntries).not.toHaveBeenCalled();
  });

  it("is not a restricted viewer's page", async () => {
    const { w } = await open("/vaults/v1/config", { paths: ["Shared/"] });
    expect(w.find("[data-test=not-yours]").exists()).toBe(true);
    expect(sdk.getEntries).not.toHaveBeenCalled();
  });
});

describe("the vault menu", () => {
  const vault = {
    id: "v1",
    name: "o",
    tenant: "dev",
    role: "viewer",
    managedBy: "config",
    configSync: ["snippets"],
  };
  it("offers Configuration to a member of a vault that syncs settings", () => {
    expect(sectionsOf(vault as never).map((s) => s.name)).toEqual([
      "vault",
      "vault-notes",
      "vault-config",
    ]);
    expect(
      sectionsOf({ ...vault, configSync: [] } as never).map((s) => s.name),
    ).not.toContain("vault-config");
    expect(
      sectionsOf({ ...vault, paths: ["A/"] } as never).map((s) => s.name),
    ).not.toContain("vault-config");
    expect(sectionOf("vault-config")).toBe("vault-config");
  });
});
