import { describe, expect, it } from "vitest";
import { AuthError, Session, discover, refresh, secretId, secureUrl, startDevice, username, waitForApproval } from "../src/auth";
import type { Http, HttpRequest, HttpResponse } from "../src/auth";

const D = { tokenEndpoint: "https://auth/token", deviceEndpoint: "https://auth/device" };
const code = { deviceCode: "dc", userCode: "WDJB-MJHT", verificationUri: "https://auth/device", verificationUriComplete: null, expiresIn: 600, interval: 5 };

/** Answers in order; records requests. */
function scripted(answers: (HttpResponse | Error)[]): { http: Http; seen: HttpRequest[] } {
  const seen: HttpRequest[] = [];
  const http: Http = (req) => {
    seen.push(req);
    const a = answers.shift();
    if (a === undefined) return Promise.reject(new Error("no more answers"));
    return a instanceof Error ? Promise.reject(a) : Promise.resolve(a);
  };
  return { http, seen };
}

describe("device code sign-in", () => {
  it("discovers and starts", async () => {
    const { http, seen } = scripted([
      { status: 200, json: { token_endpoint: "t", device_authorization_endpoint: "d" } },
      { status: 200, json: { device_code: "dc", user_code: "U", verification_uri: "v", verification_uri_complete: "vc", expires_in: 60, interval: 2 } },
    ]);
    const d = await discover(http, "https://auth/app/kura/");
    expect(seen[0]?.url).toBe("https://auth/app/kura/.well-known/openid-configuration");
    expect(d).toEqual({ tokenEndpoint: "t", deviceEndpoint: "d" });
    const c = await startDevice(http, d, "kura-plugin");
    expect(c).toMatchObject({ userCode: "U", verificationUriComplete: "vc", expiresIn: 60, interval: 2 });
    expect(seen[1]?.form).toEqual({ client_id: "kura-plugin", scope: "openid profile email offline_access" });
  });

  it("refuses broken discovery and device answers", async () => {
    await expect(discover(scripted([{ status: 500, json: null }]).http, "i")).rejects.toMatchObject({ reason: "unavailable" });
    await expect(discover(scripted([new Error("offline")]).http, "i")).rejects.toThrow(/cannot reach/);
    await expect(discover(scripted([{ status: 200, json: {} }]).http, "i")).rejects.toThrow(/token_endpoint/);
    await expect(startDevice(scripted([{ status: 400, json: null }]).http, D, "c")).rejects.toBeInstanceOf(AuthError);
  });

  it("polls through pending and slow_down to the tokens", async () => {
    const waits: number[] = [];
    const { http } = scripted([
      { status: 400, json: { error: "authorization_pending" } },
      { status: 400, json: { error: "slow_down" } },
      { status: 200, json: { access_token: "a", refresh_token: "r", expires_in: 900 } },
    ]);
    const t = await waitForApproval(http, D, "c", code, (ms) => { waits.push(ms); return Promise.resolve(); }, () => 0);
    expect(t).toEqual({ accessToken: "a", refreshToken: "r", expiresIn: 900 });
    expect(waits).toEqual([5000, 5000, 10000]);
  });

  it("ends on expiry, denial or cancellation", async () => {
    const sleep = (): Promise<void> => Promise.resolve();
    let now = 0;
    const late = waitForApproval(scripted([]).http, D, "c", code, () => { now += 700_000; return Promise.resolve(); }, () => now);
    await expect(late).rejects.toMatchObject({ reason: "expired" });
    await expect(waitForApproval(scripted([{ status: 400, json: { error: "expired_token" } }]).http, D, "c", code, sleep, () => 0)).rejects.toMatchObject({ reason: "expired" });
    await expect(waitForApproval(scripted([{ status: 400, json: { error: "access_denied" } }]).http, D, "c", code, sleep, () => 0)).rejects.toMatchObject({ reason: "denied" });
    await expect(waitForApproval(scripted([]).http, D, "c", code, sleep, () => 0, () => true)).rejects.toThrow(/cancelled/);
  });

  it("refreshes, and signs out only on invalid_grant", async () => {
    expect(await refresh(scripted([{ status: 200, json: { access_token: "a2" } }]).http, D, "c", "r")).toEqual({ accessToken: "a2", refreshToken: null, expiresIn: 300 });
    await expect(refresh(scripted([{ status: 400, json: { error: "invalid_grant" } }]).http, D, "c", "r")).rejects.toMatchObject({ reason: "signed_out" });
    await expect(refresh(scripted([{ status: 502, json: null }]).http, D, "c", "r")).rejects.toMatchObject({ reason: "unavailable" });
  });
});

describe("secrets and the session", () => {
  it("names a secret by server and vault, in SecretStorage's alphabet (decision 53)", () => {
    const id = secretId("https://Kura.example.org/", "0193A9F0-0000-7000-8000-00000000000A");
    expect(id).toMatch(/^[a-z0-9-]+$/);
    expect(id).toBe(secretId("https://kura.example.org", "0193a9f0-0000-7000-8000-00000000000a"));
    expect(id).not.toBe(secretId("https://other.example.org", "0193a9f0-0000-7000-8000-00000000000a"));
  });

  it("sends tokens over https only, http to this machine for tests", () => {
    expect(secureUrl("https://kura.example.org")).toBe(true);
    expect(secureUrl("http://127.0.0.1:58080")).toBe(true);
    expect(secureUrl("http://kura.example.org")).toBe(false);
    expect(secureUrl("not a url")).toBe(false);
  });

  it("reads the username for display only", () => {
    const payload = btoa(JSON.stringify({ preferred_username: "maxime" })).replace(/=+$/, "");
    expect(username(`h.${payload}.s`)).toBe("maxime");
    expect(username("garbage")).toBeNull();
    expect(username("h.!!!.s")).toBeNull();
  });

  it("caches, refreshes once at a time, stores the rotated token first, signs out on invalid_grant", async () => {
    const store = new Map<string, string>();
    const secrets = { getSecret: (id: string) => store.get(id) ?? null, setSecret: (id: string, s: string) => { store.set(id, s); } };
    let now = 0;
    const answers: HttpResponse[] = [
      { status: 200, json: { token_endpoint: "t", device_authorization_endpoint: "d" } },
      { status: 200, json: { access_token: "a2", refresh_token: "r2", expires_in: 900 } },
      { status: 200, json: { token_endpoint: "t", device_authorization_endpoint: "d" } },
      { status: 400, json: { error: "invalid_grant" } },
    ];
    let calls = 0;
    const http: Http = () => { calls++; const a = answers.shift(); return a === undefined ? Promise.reject(new Error("x")) : Promise.resolve(a); };
    const s = new Session(http, "https://auth", "c", secrets, "kura-x", () => now);
    expect(s.signedIn()).toBe(false);
    await expect(s.token()).rejects.toMatchObject({ reason: "signed_out" });
    s.adopt({ accessToken: "a1", refreshToken: "r1", expiresIn: 900 });
    expect(await s.token()).toBe("a1");
    now = 900_000;
    const [x, y] = await Promise.all([s.token(), s.token()]);
    expect([x, y]).toEqual(["a2", "a2"]);
    expect(calls).toBe(2);
    expect(store.get("kura-x")).toBe("r2");
    now = 2_000_000;
    await expect(s.token()).rejects.toMatchObject({ reason: "signed_out" });
    expect(s.signedIn()).toBe(false);
  });
});
