import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { i18n } from "@/i18n";
import { at, refusal } from "@/test-utils";

vi.mock("@/api/generated/sdk.gen", () => ({
  listVaults: vi.fn(),
  listBindings: vi.fn(),
  putBinding: vi.fn(async () => ({ data: {} })),
  deleteBinding: vi.fn(async () => ({ data: undefined })),
  listTokens: vi.fn(),
  createToken: vi.fn(),
  revokeToken: vi.fn(async () => ({ data: undefined })),
  listDevices: vi.fn(),
  revokeDevice: vi.fn(async () => ({ data: undefined })),
  createVault: vi.fn(async () => ({ data: {} })),
  deleteVault: vi.fn(async () => ({ data: undefined })),
  adminStatus: vi.fn(),
  me: vi.fn(),
  registerDevice: vi.fn(async () => ({ data: { id: "dev" } })),
}));

import * as sdk from "@/api/generated/sdk.gen";
import { anything, targets } from "@/manage/targets";
import AccessPage from "./AccessPage.vue";
import HomePage from "./HomePage.vue";
import KeysPage from "./KeysPage.vue";
import StatusPage from "./StatusPage.vue";
import TenantDevicesPage from "./TenantDevicesPage.vue";
import TenantVaultsPage from "./TenantVaultsPage.vue";

const vault = (
  id: string,
  tenant: string,
  role: string,
  managedBy = "config",
) => ({ id, name: id, namespace: tenant, tenant, role, managedBy });

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  i18n.global.locale.value = "en";
  vi.clearAllMocks();
});

describe("targets", () => {
  it("offers what the roles can use, and nothing to a plain member", () => {
    const me = {
      subject: "s",
      platformAdmin: false,
      tenants: [
        { slug: "jdr", role: "admin" },
        { slug: "other", role: "member" },
      ],
    };
    const t = targets(me, [
      vault("a", "jdr", "viewer"),
      vault("b", "other", "manager"),
      vault("c", "other", "editor"),
    ] as never);
    expect(t.vaults.map((v) => v.id)).toEqual(["a", "b"]);
    expect(t.tenants).toEqual(["jdr"]);
    expect(
      anything(
        targets({ ...me, tenants: [{ slug: "x", role: "member" }] }, []),
      ),
    ).toBe(false);
    expect(
      anything(targets({ ...me, tenants: [], platformAdmin: true }, [])),
    ).toBe(true);
  });

  it("lists them on the home page", async () => {
    vi.mocked(sdk.me).mockResolvedValue({
      data: {
        subject: "s",
        platformAdmin: true,
        tenants: [{ slug: "jdr", role: "owner" }],
      },
    } as never);
    vi.mocked(sdk.listVaults).mockResolvedValue({
      data: [vault("campaign", "jdr", "manager")],
    } as never);
    const w = await at(HomePage, "/vaults", "/vaults");
    expect(
      w.find('[data-test=vaults] a[href="/app/vaults/campaign"]').exists(),
    ).toBe(true);
    const run = w.find("[data-test=manage]");
    expect(run.find('a[href="/app/manage/tenants/jdr/vaults"]').exists()).toBe(
      true,
    );
    expect(run.find('a[href="/app/manage/status"]').exists()).toBe(true);
  });
});

describe("access", () => {
  it("shows every grant, edits only the API's, adds with folders, and shows refusals", async () => {
    vi.mocked(sdk.listBindings).mockResolvedValue({
      data: [
        {
          subject: "claim:jdr-players",
          role: "viewer",
          managedBy: "config",
          configSync: [],
          paths: ["Sessions/"],
        },
        { subject: "user:bob", role: "editor", managedBy: "api" },
      ],
    } as never);
    const w = await at(
      AccessPage,
      "/manage/vaults/v1/access",
      "/manage/vaults/:v/access",
    );
    const rows = w.findAll("tbody tr");
    expect(rows[0]?.text()).toContain("the tenant's owners and admins");
    expect(rows[1]?.text()).toContain("group jdr-players");
    expect(rows[1]?.text()).toContain("Sessions/");
    expect(rows[1]?.text()).toContain("read only");
    expect(rows[2]?.find("select").exists()).toBe(true);
    // A role change and a removal each wait for a confirmation under the row.
    await rows[2]?.find("select").setValue("viewer");
    expect(sdk.putBinding).not.toHaveBeenCalled();
    expect(w.find("[role=alertdialog]").text()).toContain("to viewer");
    await w.find("[role=alertdialog] button:last-child").trigger("click");
    expect(w.find("[role=alertdialog]").exists()).toBe(false);
    await rows[2]?.find("select").setValue("viewer");
    await w.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.putBinding).mock.calls[0]?.[0]).toMatchObject({
      path: { v: "v1", b: "user:bob" },
      body: { role: "viewer" },
    });
    await rows[2]?.find("button").trigger("click");
    expect(sdk.deleteBinding).not.toHaveBeenCalled();
    await w.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.deleteBinding).mock.calls[0]?.[0]?.path).toEqual({
      v: "v1",
      b: "user:bob",
    });
    await w.find("button.mt-4").trigger("click");
    await w.find("[data-test=who]").setValue("jdr-guests");
    await w.find("[data-test=folders]").setValue("Sessions, Maps/");
    vi.mocked(sdk.putBinding).mockRejectedValueOnce(
      refusal(
        403,
        "insufficient_role",
        "not a claim value or member of this tenant",
      ),
    );
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.putBinding).mock.calls[1]?.[0]).toMatchObject({
      path: { b: "claim:jdr-guests" },
      body: { role: "viewer", paths: ["Sessions/", "Maps/"] },
    });
    expect(w.find("[role=alert]").text()).toContain(
      "Your role does not allow this.",
    );
  });

  it("warns of a widened binding and of a folder with no note", async () => {
    vi.mocked(sdk.listBindings).mockResolvedValue({
      data: [
        {
          subject: "claim:jdr-mj",
          role: "viewer",
          managedBy: "config",
          configSync: [],
          paths: ["Sessions/"],
          widened: true,
        },
        {
          subject: "claim:jdr-players",
          role: "viewer",
          managedBy: "config",
          configSync: [],
          paths: ["Sessions/", "Cartes/"],
          widened: false,
          emptyFolders: ["Cartes/"],
        },
      ],
    } as never);
    const w = await at(
      AccessPage,
      "/manage/vaults/v1/access",
      "/manage/vaults/:v/access",
    );
    const [mj, players] = [
      w.find("[data-binding='claim:jdr-mj']"),
      w.find("[data-binding='claim:jdr-players']"),
    ];
    expect(mj.find("[data-test=widened]").text()).toBe(
      "effective: whole vault",
    );
    expect(mj.find("[data-test=no-notes]").exists()).toBe(false);
    expect(players.find("[data-test=widened]").exists()).toBe(false);
    expect(players.text()).toContain("Sessions/, Cartes/ (0 notes)");
    expect(players.findAll("[data-test=no-notes]")).toHaveLength(1);
  });

  it("offers the tenant's groups as a list", async () => {
    vi.mocked(sdk.listBindings).mockResolvedValue({ data: [] } as never);
    vi.mocked(sdk.listVaults).mockResolvedValue({
      data: [
        { ...vault("v1", "jdr", "manager"), groups: ["jdr-mj", "jdr-players"] },
      ],
    } as never);
    const w = await at(
      AccessPage,
      "/manage/vaults/v1/access",
      "/manage/vaults/:v/access",
    );
    await w.find("button.mt-4").trigger("click");
    expect(
      w
        .findAll("[data-test=group] option:not([disabled])")
        .map((o) => o.text()),
    ).toEqual(["jdr-mj", "jdr-players"]);
    await w.find("[data-test=group]").setValue("jdr-players");
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.putBinding).mock.calls[0]?.[0]?.path).toEqual({
      v: "v1",
      b: "claim:jdr-players",
    });
  });

  it("says when the role does not allow the page", async () => {
    vi.mocked(sdk.listBindings).mockRejectedValue(
      refusal(403, "insufficient_role"),
    );
    const w = await at(
      AccessPage,
      "/manage/vaults/v1/access",
      "/manage/vaults/:v/access",
    );
    expect(w.find("[data-test=forbidden]").exists()).toBe(true);
  });
});

describe("keys", () => {
  it("lists keys with their folders, shows a new secret once, and revokes", async () => {
    const key = {
      id: "k1",
      name: "recap",
      scopes: [
        { path: "Sessions/", mode: "read" },
        { path: "Resumes/", mode: "write" },
      ],
      cidrs: ["10.42.0.0/16"],
      expiresAt: "2026-12-15T00:00:00Z",
      createdBy: "alice",
      revoked: false,
      prefix: "/",
      mode: "write",
    };
    vi.mocked(sdk.listTokens).mockResolvedValue({ data: [key] } as never);
    vi.mocked(sdk.createToken).mockResolvedValue({
      data: { ...key, token: "kura_secret" },
    } as never);
    // Only the clipboard: a spread navigator would lose its getters.
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText: vi.fn(async () => undefined) },
      configurable: true,
    });
    const w = await at(
      KeysPage,
      "/manage/vaults/v1/keys",
      "/manage/vaults/:v/keys",
    );
    expect(w.find("tbody").text()).toContain(
      "Sessions/ (read), Resumes/ (read and write)",
    );
    expect(w.find("tbody").text()).toContain("never");
    await w.find("[data-test=filter]").setValue("nope");
    expect(w.findAll("tbody tr")).toHaveLength(0);
    expect(w.find("[role=status]").text()).toContain("Nothing matches “nope”");
    await w.find("[data-test=filter]").setValue("RECAP");
    expect(w.findAll("tbody tr")).toHaveLength(1);
    await w.find("[data-test=new-key]").trigger("click");
    await w.find("[data-test=cidrs]").setValue("10.42.0.0/16 192.168.1.0/24");
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.createToken).mock.calls[0]?.[0]?.body).toMatchObject({
      scopes: [{ path: "", mode: "read" }],
      cidrs: ["10.42.0.0/16", "192.168.1.0/24"],
      expiresInDays: 30,
    });
    expect(w.find("[data-test=secret]").text()).toBe("kura_secret");
    await w.find("[data-test=created] button").trigger("click");
    await flushPromises();
    expect(w.text()).toContain("Copied");
    await w.find("[data-test=close]").trigger("click");
    expect(w.text()).not.toContain("kura_secret");
    await w.find("tbody button").trigger("click");
    await w.find("[role=alertdialog] button").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.revokeToken).mock.calls[0]?.[0]?.path).toEqual({
      v: "v1",
      t: "k1",
    });
  });

  it("is forbidden below manager, and shows a refused creation", async () => {
    vi.mocked(sdk.listTokens).mockRejectedValueOnce(
      refusal(403, "insufficient_role"),
    );
    const w = await at(
      KeysPage,
      "/manage/vaults/v1/keys",
      "/manage/vaults/:v/keys",
    );
    expect(w.find("[data-test=forbidden]").exists()).toBe(true);
    vi.mocked(sdk.listTokens).mockResolvedValue({ data: [] } as never);
    vi.mocked(sdk.createToken).mockRejectedValueOnce(
      refusal(
        400,
        "invalid_request",
        "cidrs: 1 to 32 address ranges; a scoped token is always bound to addresses",
      ),
    );
    const ok = await at(
      KeysPage,
      "/manage/vaults/v1/keys",
      "/manage/vaults/:v/keys",
    );
    expect(ok.text()).toContain("No key yet");
    await ok.find("[data-test=new-key]").trigger("click");
    await ok.find("form").trigger("submit");
    await flushPromises();
    expect(ok.find("[role=alert]").text()).toContain(
      "always bound to addresses",
    );
  });
});

describe("tenant pages", () => {
  it("lists other members' devices only, and revokes one", async () => {
    vi.mocked(sdk.listDevices).mockResolvedValue({
      data: [
        {
          id: "mine",
          name: "laptop",
          profile: "desktop",
          revoked: false,
          lastSeen: "2026-11-17T10:00:00Z",
          createdAt: "",
        },
        {
          id: "bobs",
          name: "bob-pc",
          profile: "desktop",
          revoked: false,
          lastSeen: "2026-11-16T10:00:00Z",
          createdAt: "",
          owner: "bob",
        },
      ],
    } as never);
    const w = await at(TenantDevicesPage, "/manage/devices", "/manage/devices");
    expect(w.findAll("tbody tr")).toHaveLength(1);
    await w.find("tbody button").trigger("click");
    await w.find("[role=alertdialog] button").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.revokeDevice).mock.calls[0]?.[0]?.path).toEqual({
      d: "bobs",
    });
  });

  it("tells a member the tenant's vaults are not theirs, with no form", async () => {
    vi.mocked(sdk.me).mockResolvedValueOnce({
      data: {
        subject: "bob",
        platformAdmin: false,
        tenants: [{ slug: "jdr", role: "member" }],
      },
    } as never);
    vi.mocked(sdk.listVaults).mockResolvedValue({
      data: [vault("campaign", "jdr", "viewer")],
    } as never);
    const w = await at(
      TenantVaultsPage,
      "/manage/tenants/jdr/vaults",
      "/manage/tenants/:t/vaults",
    );
    expect(w.find("[data-test=forbidden] h2").text()).toBe("Not for your role");
    expect(w.find("form").exists()).toBe(false);
    expect(w.find("table").exists()).toBe(false);
  });

  it("creates a vault and deletes only one made through the API", async () => {
    vi.mocked(sdk.listVaults).mockResolvedValue({
      data: [
        vault("old", "jdr", "manager", "api"),
        vault("campaign", "jdr", "manager"),
        vault("x", "other", "viewer"),
      ],
    } as never);
    const w = await at(
      TenantVaultsPage,
      "/manage/tenants/jdr/vaults",
      "/manage/tenants/:t/vaults",
    );
    const rows = w.findAll("tbody tr");
    expect(rows).toHaveLength(2);
    expect(rows[1]?.find("button").exists()).toBe(false);
    await rows[0]?.find("button").trigger("click");
    await w.find("[role=alertdialog] button").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.deleteVault).mock.calls[0]?.[0]?.path).toEqual({
      v: "old",
    });
    await w.find("form input").setValue("maps");
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.createVault).mock.calls[0]?.[0]?.body).toEqual({
      tenant: "jdr",
      name: "maps",
    });
  });

  it("shows the platform status, rejection first", async () => {
    vi.mocked(sdk.adminStatus).mockResolvedValue({
      data: {
        generation: "3fa9c0de99",
        writtenAt: "2026-11-15T12:10:00Z",
        rejection: [
          "vaults.yaml#2 spec.bindings[0].role: unknown variant `editr`",
        ],
        objects: [
          {
            kind: "KuraTenant",
            name: "jdr",
            conditions: [
              {
                type: "Ready",
                status: "True",
                reason: "Reconciled",
                message: "",
              },
            ],
          },
          {
            kind: "KuraVault",
            namespace: "jdr",
            name: "old",
            conditions: [
              {
                type: "Ready",
                status: "False",
                reason: "DeletionScheduled",
                message: "deleted after 2026-12-14",
              },
            ],
          },
        ],
      },
    } as never);
    const w = await at(StatusPage, "/manage/status", "/manage/status");
    expect(w.find("[data-test=rejection]").text()).toContain(
      "unknown variant `editr`",
    );
    expect(w.text()).toContain("Configuration applied");
    expect(w.find("[title^='3fa9c0de']").exists()).toBe(true);
    expect(w.findAll("tbody tr")[1]?.text()).toContain(
      "removed, deletion scheduled — deleted after 2026-12-14",
    );
    vi.mocked(sdk.adminStatus).mockRejectedValueOnce(
      refusal(403, "insufficient_role"),
    );
    const no = await at(StatusPage, "/manage/status", "/manage/status");
    expect(no.find("[data-test=forbidden]").exists()).toBe(true);
  });
});
