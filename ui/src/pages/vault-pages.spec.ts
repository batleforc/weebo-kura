import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";

import * as sdk from "@/api/generated/sdk.gen";
import { i18n } from "@/i18n";

vi.mock("@/api/generated/sdk.gen", () => ({
  listVaults: vi.fn(),
  me: vi.fn(),
  listBindings: vi.fn(async () => ({
    data: [{ subject: "claim:kura-dev", role: "editor", managedBy: "config" }],
  })),
  listTokens: vi.fn(async () => ({
    data: [
      {
        id: "k1",
        name: "live-key",
        revoked: false,
        expiresAt: "2099-01-01T00:00:00Z",
        scopes: [],
        cidrs: [],
        createdBy: "s",
      },
      {
        id: "k2",
        name: "old-key",
        revoked: true,
        expiresAt: "2020-01-01T00:00:00Z",
        scopes: [],
        cidrs: [],
        createdBy: "s",
      },
    ],
  })),
  getEntries: vi.fn(async () => ({
    data: {
      items: [
        {
          entry: "a",
          path: "Old.md",
          seq: 1,
          deleted: false,
          revision: "r",
          size: 1,
        },
        {
          entry: "b",
          path: "Sessions/New.md",
          seq: 9,
          deleted: false,
          revision: "r",
          size: 1,
        },
        {
          entry: "c",
          path: "Gone.md",
          seq: 12,
          deleted: true,
          revision: "r",
          size: 0,
        },
      ],
      next: null,
    },
  })),
  registerDevice: vi.fn(async () => ({ data: { id: "dev" } })),
  vaultSync: vi.fn(async () => ({ data: [] })),
}));

import VaultLayout from "./VaultLayout.vue";
import VaultOverview from "./VaultOverview.vue";

async function open(role: string, tenantRole = "member") {
  vi.mocked(sdk.listVaults).mockResolvedValue({
    data: [
      { id: "v1", name: "notes", tenant: "dev", role, managedBy: "config" },
    ],
  } as never);
  vi.mocked(sdk.me).mockResolvedValue({
    data: {
      subject: "s",
      platformAdmin: false,
      tenants: [{ slug: "dev", role: tenantRole }],
    },
  } as never);
  const router = createRouter({
    history: createMemoryHistory("/app/"),
    routes: [
      {
        path: "/vaults/:v",
        component: VaultLayout,
        children: [
          { path: "", name: "vault", component: VaultOverview },
          {
            path: ":rest(.*)",
            name: "vault-notes",
            component: { template: "<p/>" },
          },
        ],
      },
    ],
  });
  await router.push("/vaults/v1");
  const w = mount(
    { template: "<RouterView />" },
    { global: { plugins: [router, i18n] } },
  );
  await flushPromises();
  await flushPromises();
  return w;
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.setItem("kura:device", "dev");
  i18n.global.locale.value = "en";
});

describe("a vault's page", () => {
  it("gives its managers every tab, who opens it and its live keys", async () => {
    const w = await open("manager");
    expect(w.find("h1").text()).toBe("notes");
    expect(w.findAll("[data-test=vault-tabs] a").map((a) => a.text())).toEqual([
      "Overview",
      "Notes",
      "Access",
      "Keys",
      "Notifications",
    ]);
    expect(w.find("[data-test=who]").text()).toContain("group kura-dev");
    expect(w.find("[data-test=live-keys]").text()).toContain("live-key");
    expect(w.find("[data-test=live-keys]").text()).not.toContain("old-key");
  });

  it("gives a reader the overview and the notes, the latest change first", async () => {
    const w = await open("viewer");
    expect(w.findAll("[data-test=vault-tabs] a").map((a) => a.text())).toEqual([
      "Overview",
      "Notes",
    ]);
    expect(w.find("[data-test=who]").exists()).toBe(false);
    expect(w.findAll("[data-test=recent] li").map((l) => l.text())).toEqual([
      "NewSessions/",
      "Old",
    ]);
  });

  it("shows a manager where each device stands, most behind first", async () => {
    vi.mocked(sdk.vaultSync).mockResolvedValueOnce({
      data: [
        {
          device: "d1",
          name: "laptop",
          profile: "desktop",
          revoked: false,
          resync: false,
          behind: 0,
          caughtUpAt: "2026-11-18T10:00:00Z",
        },
        {
          device: "d2",
          name: "pixel",
          profile: "mobile",
          owner: "bob",
          revoked: false,
          resync: false,
          report: {
            pending: 2,
            conflicts: 0,
            unsyncable: 1,
            failed: 0,
            at: "2026-10-15T10:00:00Z",
          },
          behind: 212,
          caughtUpAt: "2026-10-15T10:00:00Z",
        },
        {
          device: "d3",
          name: "tablet",
          profile: "mobile",
          owner: "carol",
          revoked: false,
          resync: false,
        },
      ],
    } as never);
    const w = await open("manager");
    const rows = w.findAll("[data-test=sync] tbody tr").map((r) => r.text());
    expect(rows[0]).toContain("pixel");
    expect(rows[0]).toContain("bob");
    expect(rows[0]).toContain("212 notes");
    expect(w.findAll("[data-test=last-run]").map((c) => c.text())).toEqual([
      "2 changes not sent · 1 file cannot sync",
      "—",
      "—",
    ]);
    expect(rows[1]).toContain("you");
    expect(rows[1]).toContain("up to date");
    expect(rows[2]).toContain("not yet");
  });

  it("tells a reader with no device on the vault how to start", async () => {
    const w = await open("viewer");
    expect(w.text()).toContain("Your devices on this vault");
    expect(w.text()).toContain("No device has synced this vault yet");
  });

  it("says when the devices could not be loaded", async () => {
    vi.mocked(sdk.vaultSync).mockRejectedValueOnce(new Error("boom"));
    const w = await open("viewer");
    expect(w.text()).toContain("Could not load the devices of this vault.");
  });

  it("counts a tenant admin as a manager of the tenant's vaults", async () => {
    const w = await open("viewer", "admin");
    expect(w.findAll("[data-test=vault-tabs] a")).toHaveLength(5);
  });
});
