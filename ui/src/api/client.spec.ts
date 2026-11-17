import { AxiosError, AxiosHeaders } from "axios";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useSession } from "@/auth/session";
import { refusal } from "@/test-utils";

vi.mock("./generated/sdk.gen", () => ({
  registerDevice: vi.fn(async () => ({ data: { id: `dev-${Math.random()}` } })),
  signOutDevice: vi.fn(async () => ({ data: undefined })),
}));

import { browserName, ensureDevice, signOut, withDevice } from "./client";
import { registerDevice } from "./generated/sdk.gen";
import { messageOf, problemType } from "./problem";

beforeEach(() => {
  setActivePinia(createPinia());
  localStorage.clear();
  vi.mocked(registerDevice).mockClear();
});

describe("the device", () => {
  it("names the browser and its system", () => {
    expect(
      browserName(
        "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
      ),
    ).toBe("Firefox on Linux");
    expect(
      browserName(
        "Mozilla/5.0 (Windows NT 10.0) AppleWebKit Chrome/140 Safari/537 Edg/140",
      ),
    ).toBe("Edge on Windows");
    expect(browserName("curl/8")).toBe("Browser on this system");
  });

  it("is registered once, as a web device, and kept", async () => {
    const [id, same] = await Promise.all([ensureDevice(), ensureDevice()]);
    expect(same).toBe(id);
    expect(await ensureDevice()).toBe(id);
    expect(registerDevice).toHaveBeenCalledTimes(1);
    expect(vi.mocked(registerDevice).mock.calls[0]?.[0]?.body).toMatchObject({
      profile: "web",
    });
    expect(localStorage.getItem("kura:device")).toBe(id);
  });

  it("is registered again, once, when the server no longer knows it", async () => {
    localStorage.setItem("kura:device", "gone");
    const seen: string[] = [];
    const out = await withDevice(async (h) => {
      seen.push(h["X-Kura-Device"]);
      if (seen.length === 1) throw refusal(401, "device_unknown");
      return "ok";
    });
    expect(out).toBe("ok");
    expect(seen[0]).toBe("gone");
    expect(seen[1]).toMatch(/^dev-/);
    await expect(
      withDevice(async () => Promise.reject(refusal(403, "insufficient_role"))),
    ).rejects.toThrow();
  });

  it("signs out through the session", async () => {
    const spy = vi.spyOn(useSession(), "signOut").mockResolvedValue();
    await signOut();
    expect(spy).toHaveBeenCalledTimes(1);
  });
});

describe("problems", () => {
  it("are read by type, and become sentences", () => {
    expect(problemType(refusal(403, "insufficient_role"))).toBe(
      "insufficient_role",
    );
    expect(problemType(new Error("x"))).toBeUndefined();
    expect(messageOf(refusal(403, "insufficient_role"))).toMatch(/role/i);
    expect(
      messageOf(refusal(400, "invalid_request", "name: 1 to 64 bytes")),
    ).toBe("name: 1 to 64 bytes");
    expect(messageOf(refusal(418))).toMatch(/418/);
    const offline = new AxiosError("down", "ERR_NETWORK", {
      headers: new AxiosHeaders(),
    });
    expect(messageOf(offline)).toMatch(/server/i);
    expect(messageOf(new Error("boom"))).toBe("boom");
    expect(messageOf("?")).toMatch(/unexpected/i);
  });
});
