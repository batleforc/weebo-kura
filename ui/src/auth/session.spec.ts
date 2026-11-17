import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { freshIdp, json, metadata } from "@/test-utils";
import { navigation, stayByDefault, useSession } from "./session";

let assigned: string | null;

beforeEach(() => {
  setActivePinia(createPinia());
  assigned = null;
  vi.spyOn(navigation, "assign").mockImplementation((url: string) => {
    assigned = url;
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

/** Starts a sign-in and returns the callback parameters the IdP would send. */
async function startSignIn(stay: boolean, to = "/devices") {
  const session = useSession();
  await session.login(to, stay);
  const url = new URL(assigned ?? "");
  return { session, url, state: url.searchParams.get("state") ?? "" };
}

describe("sign-in", () => {
  // T9, `authorize_always_sends_s256` (decision 27).
  it("always asks for an S256 challenge, the console client and offline access", async () => {
    freshIdp();
    const { url } = await startSignIn(false);
    expect(url.origin + url.pathname).toBe(metadata.authorization_endpoint);
    expect(url.searchParams.get("code_challenge_method")).toBe("S256");
    expect(url.searchParams.get("code_challenge")).toMatch(
      /^[A-Za-z0-9_-]{43}$/,
    );
    expect(url.searchParams.get("client_id")).toBe("kura-web");
    expect(url.searchParams.get("scope")).toBe("openid profile offline_access");
    expect(url.searchParams.get("redirect_uri")).toBe(
      `${window.location.origin}/app/callback`,
    );
  });

  // T9, `callback_rejects_wrong_state`.
  it("refuses a callback whose state is not the one it started, error included", async () => {
    const fetchMock = freshIdp(() =>
      json({ access_token: "a", refresh_token: "r" }),
    );
    const { session } = await startSignIn(false);
    for (const forged of [
      "state=other&code=c",
      "state=other&error=access_denied",
    ]) {
      await expect(
        session.completeLogin(new URLSearchParams(forged)),
      ).rejects.toThrow(/does not match/);
    }
    expect(
      fetchMock.mock.calls.some(([u]) => String(u) === metadata.token_endpoint),
    ).toBe(false);
    expect(session.authenticated).toBe(false);
  });

  // T14, `refresh_token_storage_follows_the_box`.
  it.each([
    [false, sessionStorage, localStorage],
    [true, localStorage, sessionStorage],
  ])(
    "keeps the refresh token where the box says (stay: %s)",
    async (stay, kept, other) => {
      const sent = new URLSearchParams();
      freshIdp((body) => {
        for (const [k, v] of body) {
          sent.set(k, v);
        }
        return json({
          access_token: "access",
          refresh_token: "refresh-1",
          expires_in: 900,
        });
      });
      const { session, state } = await startSignIn(
        stay,
        "https://evil.example/",
      );
      const target = await session.completeLogin(
        new URLSearchParams(`state=${state}&code=the-code`),
      );
      expect(target).toBe("/");
      expect(kept.getItem("kura:refresh-token")).toBe("refresh-1");
      expect(other.getItem("kura:refresh-token")).toBeNull();
      expect(session.accessToken).toBe("access");
      const everything = [localStorage, sessionStorage].flatMap((st) =>
        Array.from(
          { length: st.length },
          (_, i) => st.getItem(st.key(i) ?? "") ?? "",
        ),
      );
      expect(everything).not.toContain("access");
      expect(sent.get("code_verifier")).toMatch(/^[A-Za-z0-9_-]{43}$/);
      expect(sent.get("grant_type")).toBe("authorization_code");
    },
  );

  it("ticks the box by default in the installed application only", () => {
    vi.stubGlobal("matchMedia", (q: string) => ({
      matches: q === "(display-mode: standalone)",
    }));
    expect(stayByDefault()).toBe(true);
    vi.stubGlobal("matchMedia", () => ({ matches: false }));
    expect(stayByDefault()).toBe(false);
  });
});

describe("refresh", () => {
  // T14, `refresh_is_single_flight_across_tabs`.
  it("asks once for many callers, under a lock every tab shares", async () => {
    let calls = 0;
    freshIdp(async (body) => {
      calls += 1;
      expect(body.get("refresh_token")).toBe(`r${calls}`);
      return json({
        access_token: `a${calls}`,
        refresh_token: `r${calls + 1}`,
        expires_in: 900,
      });
    });
    localStorage.setItem("kura:refresh-token", "r1");
    const held: string[] = [];
    vi.stubGlobal("navigator", {
      ...navigator,
      locks: {
        request: async (name: string, run: () => Promise<unknown>) => {
          held.push(name);
          return run();
        },
      },
    });
    const session = useSession();
    const all = await Promise.all([
      session.bearer(),
      session.bearer(),
      session.refresh(),
    ]);
    expect(all).toEqual(["a1", "a1", "a1"]);
    expect(calls).toBe(1);
    expect(held).toEqual(["kura:refresh"]);
    expect(localStorage.getItem("kura:refresh-token")).toBe("r2");
  });

  // Decision 28: `refresh_without_new_token_keeps_the_old_one`.
  it("keeps the stored refresh token when the answer carries none", async () => {
    freshIdp(() => json({ access_token: "a2", expires_in: 900 }));
    sessionStorage.setItem("kura:refresh-token", "kept");
    const session = useSession();
    expect(await session.refresh()).toBe("a2");
    expect(sessionStorage.getItem("kura:refresh-token")).toBe("kept");
  });

  // Decision 26: `unexpected_invalid_grant_shows_the_revoke_hint`.
  it("ends the session on invalid_grant only, and says it ended unexpectedly", async () => {
    let answer = () => new Response("down", { status: 503 });
    freshIdp(() => answer());
    localStorage.setItem("kura:refresh-token", "r");
    const session = useSession();
    expect(await session.refresh()).toBeNull();
    expect(localStorage.getItem("kura:refresh-token")).toBe("r");
    expect(session.endedUnexpectedly).toBe(false);
    answer = () => json({ error: "invalid_grant" }, 400);
    expect(await session.refresh()).toBeNull();
    expect(localStorage.getItem("kura:refresh-token")).toBeNull();
    expect(session.endedUnexpectedly).toBe(true);
    expect(await session.restore()).toBe(false);
  });
});

describe("sign-out", () => {
  // T14, `sign_out_clears_storage_and_closes_the_device` (decisions 24, 25).
  it("signs the device out, clears both storages and fires the revocation unread", async () => {
    const fetchMock = freshIdp(() =>
      json({ access_token: "a", refresh_token: "r", expires_in: 900 }),
    );
    localStorage.setItem("kura:refresh-token", "r0");
    const session = useSession();
    await session.refresh();
    session.setDevice("dev-1");
    const signedOut: string[] = [];
    await session.signOut(async (d) => signedOut.push(d));
    expect(signedOut).toEqual(["dev-1"]);
    expect(localStorage.length + sessionStorage.length).toBe(0);
    expect(session.authenticated).toBe(false);
    const revoke = fetchMock.mock.calls.find(
      ([u]) => String(u) === metadata.revocation_endpoint,
    );
    expect(revoke?.[1]).toMatchObject({
      method: "POST",
      mode: "no-cors",
      keepalive: true,
    });
    expect(new URLSearchParams(String(revoke?.[1]?.body)).get("token")).toBe(
      "r",
    );
    expect(session.endedUnexpectedly).toBe(false);
  });
});
