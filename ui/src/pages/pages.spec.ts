import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory, createRouter } from "vue-router";
import { useSession } from "@/auth/session";
import { i18n } from "@/i18n";
import { refusal } from "@/test-utils";

vi.mock("@/api/generated/sdk.gen", () => ({
  listVaults: vi.fn(),
  listDevices: vi.fn(),
  revokeDevice: vi.fn(async () => ({ data: undefined })),
  registerDevice: vi.fn(async () => ({ data: { id: "this-browser" } })),
  signOutDevice: vi.fn(async () => ({ data: undefined })),
  me: vi.fn(),
}));

import App from "@/App.vue";
import * as sdk from "@/api/generated/sdk.gen";
import CallbackPage from "./CallbackPage.vue";
import DevicesPage from "./DevicesPage.vue";
import HomePage from "./HomePage.vue";
import SignInPage from "./SignInPage.vue";

const device = (
  id: string,
  name: string,
  profile = "desktop",
  revoked = false,
) => ({
  id,
  name,
  profile,
  revoked,
  createdAt: "2026-11-17T10:00:00Z",
  lastSeen: "2026-11-17T10:00:00Z",
});

async function page(component: object, path = "/") {
  const router = createRouter({
    history: createMemoryHistory("/app/"),
    routes: [
      { path: "/", component: { template: "<p/>" } },
      {
        path: "/signin",
        name: "signin",
        component: { template: "<p>signin</p>" },
      },
      { path: "/vaults", component: { template: "<p>vaults</p>" } },
      { path: "/devices", component: { template: "<p>devices</p>" } },
    ],
  });
  await router.push(path);
  return {
    wrapper: mount(component, { global: { plugins: [router, i18n] } }),
    router,
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  i18n.global.locale.value = "en";
  vi.clearAllMocks();
});

describe("sign-in page", () => {
  it("offers one action and the box; after an unexpected end, the revoke hint", async () => {
    const session = useSession();
    const login = vi.spyOn(session, "login").mockResolvedValue();
    const { wrapper } = await page(SignInPage, "/signin?to=/devices");
    await flushPromises();
    expect(wrapper.find("[data-test=ended]").exists()).toBe(false);
    await wrapper.find("[data-test=stay]").setValue(true);
    await wrapper.find("[data-test=sign-in]").trigger("click");
    expect(login).toHaveBeenCalledWith("/devices", true);
    session.endedUnexpectedly = true;
    await flushPromises();
    expect(wrapper.find("[data-test=ended]").text()).toContain(
      "revoke this browser",
    );
    login.mockRejectedValue(new Error("no idp"));
    await wrapper.find("[data-test=sign-in]").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("no idp");
  });
});

describe("callback page", () => {
  it("lands on the in-app target, or says why it could not", async () => {
    const session = useSession();
    vi.spyOn(session, "completeLogin")
      .mockResolvedValueOnce("/devices")
      .mockRejectedValueOnce(new Error("bad state"));
    const first = await page(CallbackPage);
    await flushPromises();
    expect(first.router.currentRoute.value.path).toBe("/devices");
    const second = await page(CallbackPage);
    await flushPromises();
    expect(second.wrapper.text()).toContain("bad state");
  });
});

describe("my vaults", () => {
  it("lists the vaults, or says it could not and retries", async () => {
    vi.mocked(sdk.listVaults)
      .mockRejectedValueOnce(refusal(503, "unavailable"))
      .mockResolvedValueOnce({
        data: [
          {
            id: "v1",
            name: "campaign",
            tenant: "jdr",
            role: "viewer",
            managedBy: "config",
          },
        ],
      } as never);
    const { wrapper } = await page(HomePage);
    await flushPromises();
    // RFC 0005 §4.5: a 503 says it retries by itself, and keeps the button.
    expect(wrapper.find("[role=alert]").text()).toContain(
      "not answering (503). Retrying in 30 s",
    );
    await wrapper.find("[role=alert] button").trigger("click");
    await flushPromises();
    expect(wrapper.find("[data-test=vaults]").text()).toContain("campaign");
    expect(vi.mocked(sdk.listVaults).mock.calls[0]?.[0]?.headers).toEqual({
      "X-Kura-Device": "this-browser",
    });
  });
});

describe("my devices", () => {
  it("names this browser as what signing out does, and confirms a revocation", async () => {
    localStorage.setItem("kura:device", "this-browser");
    vi.mocked(sdk.listDevices).mockResolvedValue({
      data: [
        device("this-browser", "Firefox on Linux", "web"),
        device("d2", "laptop"),
        device("d3", "old phone", "mobile", true),
      ],
    } as never);
    const session = useSession();
    const signOut = vi.spyOn(session, "signOut").mockResolvedValue();
    const { wrapper, router } = await page(DevicesPage);
    await flushPromises();
    // The revoked device stays folded until asked for.
    expect(wrapper.findAll("tbody tr")).toHaveLength(2);
    await wrapper.find("[data-test=show-revoked]").trigger("click");
    const rows = wrapper.findAll("tbody tr");
    expect(rows.map((r) => r.text())).toEqual([
      expect.stringContaining("this browser"),
      expect.stringContaining("active"),
      expect.stringContaining("revoked"),
    ]);
    expect(rows[2]?.find("button").exists()).toBe(false);
    await rows[1]?.find("[data-test=revoke]").trigger("click");
    expect(wrapper.find("[role=alertdialog]").text()).toContain(
      "Revoke laptop?",
    );
    await wrapper.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.revokeDevice).mock.calls[0]?.[0]?.path).toEqual({
      d: "d2",
    });
    await wrapper.find("[data-test=sign-out-here]").trigger("click");
    await flushPromises();
    expect(signOut).toHaveBeenCalled();
    expect(router.currentRoute.value.name).toBe("signin");
  });

  it("shows a refused revocation", async () => {
    vi.mocked(sdk.listDevices).mockResolvedValue({
      data: [device("d2", "laptop")],
    } as never);
    vi.mocked(sdk.revokeDevice).mockRejectedValueOnce(
      refusal(403, "insufficient_role"),
    );
    const { wrapper } = await page(DevicesPage);
    await flushPromises();
    await wrapper.find("[data-test=revoke]").trigger("click");
    await wrapper.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain("Your role does not allow this.");
  });
});

describe("shell", () => {
  async function shell(me: object) {
    vi.mocked(sdk.me).mockResolvedValue({ data: me } as never);
    const session = useSession();
    session.accessToken = "a";
    const router = createRouter({
      history: createMemoryHistory("/app/"),
      routes: [
        {
          path: "/vaults",
          component: { template: "<p>vaults page</p>" },
          meta: { access: "member" },
        },
        {
          path: "/signin",
          name: "signin",
          component: { template: "<p>signin</p>" },
          meta: { access: "public" },
        },
      ],
    });
    await router.push("/vaults");
    const wrapper = mount(App, { global: { plugins: [router, i18n] } });
    await flushPromises();
    return { wrapper, router, session };
  }

  it("shows the member's pages, and signs out", async () => {
    const { wrapper, router, session } = await shell({
      subject: "s",
      name: "Alice",
      tenants: [{ slug: "jdr", role: "member" }],
      platformAdmin: false,
    });
    expect(wrapper.text()).toContain("Alice");
    expect(wrapper.text()).toContain("vaults page");
    expect(sdk.registerDevice).toHaveBeenCalled();
    const out = vi.spyOn(session, "signOut").mockResolvedValue();
    await wrapper.find("[data-test=sign-out]").trigger("click");
    await flushPromises();
    expect(out).toHaveBeenCalled();
    expect(router.currentRoute.value.name).toBe("signin");
  });

  it("folds each vault's sections under it, the open one unfolded, the manager's tabs for a tenant admin", async () => {
    vi.mocked(sdk.listVaults).mockResolvedValue({
      data: [
        { id: "v1", name: "campagne", tenant: "jdr", role: "viewer" },
        { id: "v2", name: "archives", tenant: "jdr", role: "viewer" },
      ],
    } as never);
    const { wrapper, router } = await shell({
      subject: "s",
      tenants: [{ slug: "jdr", role: "admin" }],
      platformAdmin: false,
    });
    router.addRoute({
      path: "/vaults/:v/:rest(.*)*",
      name: "vault-notes",
      component: { template: "<p/>" },
      meta: { access: "member" },
    });
    await router.push("/vaults/v1/notes");
    await flushPromises();
    const sections = wrapper.findAll("[data-test=vault-sections]");
    expect(sections).toHaveLength(1);
    expect(sections[0]?.findAll("a").map((a) => a.text())).toEqual([
      "Overview",
      "Notes",
      "Access",
      "Keys",
      "Notifications",
    ]);
    expect(sections[0]?.find("[aria-current=page]").text()).toBe("Notes");
    const [first, second] = wrapper.findAll("[data-test=vault-expand]");
    // Sorted by name: archives, then campagne.
    expect(first?.attributes("aria-expanded")).toBe("false");
    await first?.trigger("click");
    expect(wrapper.findAll("[data-test=vault-sections]")).toHaveLength(2);
    await second?.trigger("click");
    expect(second?.attributes("aria-expanded")).toBe("false");
  });

  it("says so when the caller is in no tenant", async () => {
    const { wrapper } = await shell({
      subject: "s",
      name: "Carol",
      tenants: [],
      platformAdmin: false,
    });
    expect(wrapper.find("[data-test=no-tenant]").text()).toContain(
      "not a member of any Kura tenant",
    );
    expect(sdk.registerDevice).not.toHaveBeenCalled();
  });
});
