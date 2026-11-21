import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { i18n } from "@/i18n";
import { at, refusal } from "@/test-utils";

vi.mock("@/api/generated/sdk.gen", () => ({
  listRules: vi.fn(),
  createRule: vi.fn(),
  updateRule: vi.fn(async () => ({ data: {} })),
  deleteRule: vi.fn(async () => ({ data: undefined })),
  testRule: vi.fn(async () => ({ data: { result: "200" } })),
  listDeliveries: vi.fn(),
  listVaults: vi.fn(async () => ({
    data: [{ id: "v1", name: "campaign", tenant: "jdr", role: "manager" }],
  })),
  me: vi.fn(),
  registerDevice: vi.fn(async () => ({ data: { id: "dev" } })),
}));

import * as sdk from "@/api/generated/sdk.gen";
import DeliveriesPage from "./DeliveriesPage.vue";
import NotificationsPage from "./NotificationsPage.vue";

const ruleView = (over: object = {}) => ({
  id: "r1",
  name: "hub",
  events: ["token.expiring", "quota.high"],
  vault: null,
  host: "hub.example.org",
  hasAuthorization: true,
  enabled: true,
  consecutiveFailures: 0,
  createdBy: "alice",
  last: { at: "2026-11-15T12:33:21Z", result: "200" },
  ...over,
});

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  i18n.global.locale.value = "en";
  vi.clearAllMocks();
});

describe("notifications page", () => {
  it("shows hosts only, a banner for a switched-off rule, tests and re-enables", async () => {
    vi.mocked(sdk.me).mockResolvedValue({
      data: {
        subject: "a",
        platformAdmin: false,
        tenants: [{ slug: "jdr", role: "admin" }],
      },
    } as never);
    vi.mocked(sdk.listRules).mockResolvedValue({
      data: [
        ruleView(),
        ruleView({
          id: "r2",
          name: "recap",
          vault: "v1",
          enabled: false,
          disabledReason: "failures: timeout",
          host: "10.42.7.15",
        }),
      ],
    } as never);
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    expect(w.find("[data-rule=hub]").text()).toContain("hub.example.org");
    expect(w.find("[data-rule=hub]").text()).toContain("(whole tenant)");
    expect(w.find("[data-rule=recap]").text()).toContain("campaign");
    expect(w.find("[data-test=switched-off]").text()).toContain(
      '"recap" was switched off (failures: timeout)',
    );
    await w.find("[data-rule=hub] [data-test=test]").trigger("click");
    await flushPromises();
    expect(w.find("[data-test=tested]").text()).toBe("Test of hub: 200");
    await w.find("[data-rule=recap] button:nth-child(2)").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.updateRule).mock.calls[0]?.[0]).toMatchObject({
      path: { t: "jdr", n: "r2" },
      body: { enabled: true },
    });
    await w.find("[data-rule=hub] [data-test=remove]").trigger("click");
    expect(sdk.deleteRule).not.toHaveBeenCalled();
    await w.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.deleteRule).mock.calls[0]?.[0]?.path).toEqual({
      t: "jdr",
      n: "r1",
    });
  });

  it("creates a rule, shows its secret once, and the server's refusal as it is", async () => {
    vi.mocked(sdk.me).mockResolvedValue({
      data: {
        subject: "a",
        platformAdmin: false,
        tenants: [{ slug: "jdr", role: "owner" }],
      },
    } as never);
    vi.mocked(sdk.listRules).mockResolvedValue({ data: [] } as never);
    vi.mocked(sdk.createRule)
      .mockRejectedValueOnce(
        refusal(
          400,
          "invalid_request",
          "this address cannot be used: a webhook must be an https address that is public, or one the platform operator allowed",
        ),
      )
      .mockResolvedValueOnce({
        data: { ...ruleView(), secret: "whsec_abc" },
      } as never);
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    expect(w.text()).toContain("No rule yet");
    await w.find("[data-test=new-rule]").trigger("click");
    await w.find("[data-test=name]").setValue("hub");
    await w.find("input[value='quota.high']").setValue(true);
    await w.find("[data-test=url]").setValue("https://10.0.0.5/hook");
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(w.find("[role=alert]").text()).toContain(
      "this address cannot be used",
    );
    await w.find("[data-test=url]").setValue("https://hub.example.org/hook");
    await w.find("[data-test=authorization]").setValue("Bearer wnk_1");
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.createRule).mock.calls[1]?.[0]?.body).toEqual({
      name: "hub",
      events: ["quota.high"],
      vault: null,
      channel: {
        type: "webhook",
        url: "https://hub.example.org/hook",
        authorization: "Bearer wnk_1",
      },
    });
    expect(w.find("[data-test=secret]").text()).toBe("whsec_abc");
    await w.find("[data-test=close]").trigger("click");
    expect(w.text()).not.toContain("whsec_abc");
  });

  it("offers a manager their vaults and vault events only; says when notifications are off", async () => {
    vi.mocked(sdk.me).mockResolvedValue({
      data: {
        subject: "m",
        platformAdmin: false,
        tenants: [{ slug: "jdr", role: "member" }],
      },
    } as never);
    vi.mocked(sdk.listRules).mockResolvedValue({ data: [] } as never);
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    await w.find("[data-test=new-rule]").trigger("click");
    const scopes = w.findAll("[data-test=scope] option").map((o) => o.text());
    expect(scopes).toEqual(["campaign"]);
    expect(w.find("input[value='quota.high']").exists()).toBe(false);
    expect(w.find("input[value='vault.changed']").exists()).toBe(true);
    vi.mocked(sdk.listRules).mockRejectedValue(
      refusal(409, "notifications_disabled"),
    );
    const off = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    expect(off.find("[data-test=disabled]").exists()).toBe(true);
  });
});

describe("notifications page, edit and rotate", () => {
  beforeEach(() => {
    vi.mocked(sdk.me).mockResolvedValue({
      data: {
        subject: "a",
        platformAdmin: false,
        tenants: [{ slug: "jdr", role: "admin" }],
      },
    } as never);
    vi.mocked(sdk.listRules).mockResolvedValue({ data: [ruleView()] } as never);
  });

  it("edits a rule, keeping what is left empty and removing the authorization when asked", async () => {
    vi.mocked(sdk.updateRule)
      .mockRejectedValueOnce(
        refusal(400, "invalid_request", "events: at least one event type"),
      )
      .mockResolvedValueOnce({ data: ruleView() } as never);
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    await w.find("[data-rule=hub] [data-test=edit]").trigger("click");
    expect(w.find("form h2").text()).toBe("Edit the rule hub");
    expect(w.find<HTMLInputElement>("[data-test=name]").element.value).toBe(
      "hub",
    );
    expect(w.find("[data-test=scope]").exists()).toBe(false);
    expect(w.find("[data-test=url]").attributes("required")).toBeUndefined();
    await w.find("[data-test=name]").setValue("hub2");
    await w.find("input[value='token.expiring']").setValue(false);
    await w.find("input[value='quota.high']").setValue(false);
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(w.find("[role=alert]").text()).toContain("at least one event");
    await w.find("input[value='vault.changed']").setValue(true);
    await w.find("[data-test=remove-authorization]").setValue(true);
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.updateRule).mock.calls[1]?.[0]).toEqual(
      expect.objectContaining({
        path: { t: "jdr", n: "r1" },
        body: { name: "hub2", events: ["vault.changed"], authorization: "" },
      }),
    );
    expect(w.find("form").exists()).toBe(false);
    await w.find("[data-rule=hub] [data-test=edit]").trigger("click");
    await w.find("[data-test=url]").setValue("https://hub2.example.org/hook");
    await w.find("[data-test=authorization]").setValue("Bearer new");
    await w.find("form").trigger("submit");
    await flushPromises();
    expect(vi.mocked(sdk.updateRule).mock.calls[2]?.[0]?.body).toEqual({
      name: "hub",
      events: ["token.expiring", "quota.high"],
      url: "https://hub2.example.org/hook",
      authorization: "Bearer new",
    });
  });

  it("rotates the secret after a confirmation, shows it once, and copies it", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });
    vi.mocked(sdk.updateRule).mockResolvedValue({
      data: { ...ruleView(), secret: "whsec_new" },
    } as never);
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    await w.find("[data-rule=hub] [data-test=rotate]").trigger("click");
    expect(w.find("[role=alertdialog]").text()).toContain(
      "Rotate the signing secret of hub?",
    );
    await w.find("[role=alertdialog] button.border-rule").trigger("click");
    expect(w.find("[role=alertdialog]").exists()).toBe(false);
    expect(sdk.updateRule).not.toHaveBeenCalled();
    await w.find("[data-rule=hub] [data-test=rotate]").trigger("click");
    await w.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(vi.mocked(sdk.updateRule).mock.calls[0]?.[0]).toMatchObject({
      path: { t: "jdr", n: "r1" },
      body: { rotateSecret: true },
    });
    expect(w.find("[data-test=created] h2").text()).toBe("New signing secret");
    expect(w.find("[data-test=secret]").text()).toBe("whsec_new");
    await w.find("[data-test=copy]").trigger("click");
    await flushPromises();
    expect(writeText).toHaveBeenCalledWith("whsec_new");
    expect(w.find("[data-test=copy]").text()).toBe("Copied");
    await w.find("[data-test=close]").trigger("click");
    expect(w.text()).not.toContain("whsec_new");
  });

  it("shows a refused rotation", async () => {
    vi.mocked(sdk.updateRule).mockRejectedValue(refusal(503, "unavailable"));
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    await w.find("[data-rule=hub] [data-test=rotate]").trigger("click");
    await w.find("[data-test=confirm]").trigger("click");
    await flushPromises();
    expect(w.find("[role=alert]").text()).toContain("unavailable");
    expect(w.find("[data-test=created]").exists()).toBe(false);
  });

  it("repeats the server's insufficient_role detail, in English", async () => {
    vi.mocked(sdk.listRules).mockRejectedValue(
      refusal(
        403,
        "insufficient_role",
        "notifications need a tenant admin, or a vault manager",
      ),
    );
    const w = await at(
      NotificationsPage,
      "/manage/tenants/jdr/notifications",
      "/manage/tenants/:t/notifications",
    );
    const forbidden = w.find("[data-test=forbidden]");
    expect(forbidden.attributes("role")).toBe("alert");
    expect(forbidden.find("[lang=en]").text()).toBe(
      "notifications need a tenant admin, or a vault manager",
    );
  });
});

describe("deliveries page", () => {
  it("lists coarse results", async () => {
    vi.mocked(sdk.listDeliveries).mockResolvedValue({
      data: [
        {
          id: "d1",
          event: "token.expiring",
          at: "2026-11-15T12:33:21Z",
          attempts: 1,
          status: "delivered",
          result: "200",
        },
        {
          id: "d2",
          event: "device.revoked",
          at: "2026-11-14T22:15:44Z",
          attempts: 6,
          status: "failed",
          result: "status_5xx",
        },
      ],
    } as never);
    const w = await at(
      DeliveriesPage,
      "/manage/tenants/jdr/notifications/r1",
      "/manage/tenants/:t/notifications/:n",
    );
    const rows = w.findAll("tbody tr").map((r) => r.text());
    expect(w.find("tbody time").attributes("datetime")).toBe(
      "2026-11-15T12:33:21Z",
    );
    expect(rows[1]).toContain("failed: status_5xx");
  });

  it("says forbidden, with the generic text when the server gave no detail", async () => {
    vi.mocked(sdk.listDeliveries).mockRejectedValue(
      refusal(403, "insufficient_role"),
    );
    const w = await at(
      DeliveriesPage,
      "/manage/tenants/jdr/notifications/r1",
      "/manage/tenants/:t/notifications/:n",
    );
    const forbidden = w.find("[data-test=forbidden]");
    expect(forbidden.find("h2").text()).toBe("Not for your role");
    expect(forbidden.text()).toContain("Your role does not allow this page.");
    expect(forbidden.find("a").attributes("href")).toContain("/vaults");
    expect(w.text()).not.toContain("No delivery yet");
  });
});
