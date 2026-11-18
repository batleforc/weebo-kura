import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory } from "vue-router";

import { useSession } from "@/auth/session";
import { localeOf } from "@/i18n";
import { makeRouter, routes } from "./router";

beforeEach(() => setActivePinia(createPinia()));

describe("router", () => {
  it("declares who may open every page", () => {
    for (const r of routes.filter((r) => !r.redirect)) {
      expect(["public", "member"], String(r.path)).toContain(r.meta?.access);
    }
  });

  it("sends a signed-out visitor to sign-in, with where they were going", async () => {
    vi.spyOn(useSession(), "restore").mockResolvedValue(false);
    const router = makeRouter(createMemoryHistory("/app/"));
    await router.push("/devices");
    expect(router.currentRoute.value.name).toBe("signin");
    expect(router.currentRoute.value.query.to).toBe("/devices");
  });

  it("lets a restored session in, and refuses a page without access", async () => {
    vi.spyOn(useSession(), "restore").mockResolvedValue(true);
    const router = makeRouter(createMemoryHistory("/app/"));
    await router.push("/devices");
    expect(router.currentRoute.value.name).toBe("devices");
    router.addRoute({ path: "/undeclared", component: { template: "<p/>" } });
    await router.push("/undeclared");
    expect(router.currentRoute.value.name).toBe("devices");
  });

  it("speaks French to a French browser, English otherwise", () => {
    expect(localeOf(["fr-FR", "en"])).toBe("fr");
    expect(localeOf(["de", "en-GB"])).toBe("en");
  });
});
