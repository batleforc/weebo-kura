import { beforeEach, describe, expect, it, vi } from "vitest";

describe("preferences", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
    vi.stubGlobal("matchMedia", (q: string) => ({
      matches: q.includes("light"),
      addEventListener: () => undefined,
    }));
  });

  it("follow the device while on system, and store the choice, not its result", async () => {
    const prefs = await import("@/prefs");
    expect(prefs.themePref.value).toBe("system");
    expect(prefs.resolvedTheme()).toBe("light");
    prefs.setTheme("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem("kura:theme")).toBe("dark");
    prefs.setLocale("fr");
    expect(document.documentElement.lang).toBe("fr");
    // The same module instance prefs drives (resetModules gave each test its own).
    const { i18n } = await import("@/i18n");
    expect(i18n.global.locale.value).toBe("fr");
    prefs.setLocale("system");
    expect(localStorage.getItem("kura:locale")).toBe("system");
  });

  it("ignore a stored value they do not know", async () => {
    localStorage.setItem("kura:theme", "sepia");
    const prefs = await import("@/prefs");
    expect(prefs.themePref.value).toBe("system");
  });
});
