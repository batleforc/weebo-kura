import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory } from "vue-router";

import { useSession } from "@/auth/session";
import { localeOf } from "@/i18n";
import { makeRouter, routes, whenTall } from "./router";

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

  it("restores a saved place once the page is tall enough, or after a wait", async () => {
    vi.useFakeTimers();
    let height = 0;
    vi.spyOn(
      document.documentElement,
      "scrollHeight",
      "get",
    ).mockImplementation(() => height);
    const saved = { left: 0, top: 3000 };
    let landed = false;
    void whenTall(saved).then(() => (landed = true));
    await vi.advanceTimersByTimeAsync(500);
    expect(landed).toBe(false);
    height = 3000 + window.innerHeight;
    await vi.advanceTimersByTimeAsync(50);
    expect(landed).toBe(true);
    // A page that never grows lands anyway.
    height = 0;
    landed = false;
    void whenTall(saved).then(() => (landed = true));
    await vi.advanceTimersByTimeAsync(1600);
    expect(landed).toBe(true);
    vi.useRealTimers();
    vi.restoreAllMocks();
  });
});
