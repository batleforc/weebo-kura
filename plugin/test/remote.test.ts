import { describe, expect, it } from "vitest";
import { Api, CLIENT } from "../src/remote";
import type { Fetch } from "../src/remote";

function api(fetch: Fetch, device: string | null = "dev-1") {
  const waits: number[] = [];
  const a = new Api({ server: "https://kura.test/", token: () => Promise.resolve("tok"), device: () => device, fetch, sleep: (ms) => { waits.push(ms); return Promise.resolve(); } });
  return { a, waits };
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}): Response =>
  new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json", ...headers } });

describe("the API client", () => {
  it("sends the token, the device and the client version", async () => {
    let url = "";
    let headers: unknown = null;
    const { a } = api((u, init) => { url = u; headers = init.headers; return Promise.resolve(json(200, [])); });
    await a.vaults();
    expect(url).toBe("https://kura.test/v1/vaults");
    expect(headers).toMatchObject({ authorization: "Bearer tok", "x-kura-device": "dev-1", "x-kura-client": CLIENT });
  });

  it("waits out 429 per Retry-After, then gives up with the problem", async () => {
    const answers = [json(429, { type: "/v1/problems/rate_limited" }, { "retry-after": "7" }), json(200, { id: "d2" })];
    const { a, waits } = api(() => Promise.resolve(answers.shift() ?? json(500, null)));
    expect(await a.register("phone", "mobile")).toBe("d2");
    expect(waits).toEqual([7000]);
    const always = api(() => Promise.resolve(json(429, { type: "/v1/problems/rate_limited" })));
    await expect(always.a.vaults()).rejects.toMatchObject({ status: 429, kind: "rate_limited" });
    expect(always.waits).toHaveLength(4);
  });

  it("maps refusals to problems and outages to errors", async () => {
    await expect(api(() => Promise.resolve(json(403, { type: "/v1/problems/insufficient_role", detail: "editor required" }))).a.vaults())
      .rejects.toMatchObject({ status: 403, kind: "insufficient_role", detail: "editor required" });
    await expect(api(() => Promise.resolve(new Response("oops", { status: 404 }))).a.vaults()).rejects.toMatchObject({ kind: "unknown" });
    await expect(api(() => Promise.resolve(json(500, null))).a.vaults()).rejects.toThrow(/server error 500/);
    await expect(api(() => Promise.reject(new Error("offline"))).a.vaults()).rejects.toThrow(/cannot reach/);
  });

  it("parses vaults, tickets and the events URL", async () => {
    const vaults = [
      { id: "v1", name: "notes", role: "editor", chunking: { min: 1, avg: 2, max: 3 }, configSync: ["plugins", 7] },
      { id: 5 },
      { id: "v2", name: "x", role: "viewer", chunking: { min: "a" } },
    ];
    expect(await api(() => Promise.resolve(json(200, vaults))).a.vaults()).toEqual([
      { id: "v1", name: "notes", role: "editor", chunking: { min: 1, avg: 2, max: 3 }, configSync: ["plugins"] },
    ]);
    expect(await api(() => Promise.resolve(json(200, { x: 1 }))).a.vaults()).toEqual([]);
    expect(await api(() => Promise.resolve(json(200, { ticket: "abc" }))).a.ticket("v1")).toBe("abc");
    await expect(api(() => Promise.resolve(json(200, {}))).a.ticket("v1")).rejects.toThrow(/no ticket/);
    await expect(api(() => Promise.resolve(json(201, {})), null).a.register("x", "desktop")).rejects.toThrow(/no device/);
    expect(api(() => Promise.resolve(json(200, {}))).a.eventsUrl("v1", "t/+")).toBe("wss://kura.test/v1/vaults/v1/events?ticket=t%2F%2B");
  });

  it("exposes one vault to the core", async () => {
    const calls: string[] = [];
    const fetch: Fetch = (url, init) => {
      calls.push(`${init.method ?? ""} ${url.replace("https://kura.test/v1/vaults/v1", "")}`);
      if (url.endsWith("/chunks/missing")) return Promise.resolve(json(200, { missing: ["h1", 2] }));
      if (url.includes("/chunks/") && init.method === "GET") return Promise.resolve(new Response(new Uint8Array([1, 2]), { status: 200 }));
      if (url.includes("/blobs/") && init.method === "GET") return Promise.resolve(json(200, { size: 2, chunks: ["h1"] }));
      return Promise.resolve(new Response(null, { status: 204 }));
    };
    const r = api(fetch).a.remote("v1");
    expect(await r.missing(["h1", "h2"])).toEqual(["h1"]);
    await r.putChunk("h1", new Uint8Array([1]));
    expect(await r.getChunk("h1")).toEqual(new Uint8Array([1, 2]));
    await r.putBlob("b", 2, ["h1"]);
    expect(await r.getBlob("b")).toEqual({ size: 2, chunks: ["h1"] });
    expect(calls).toEqual(["POST /chunks/missing", "PUT /chunks/h1", "GET /chunks/h1", "PUT /blobs/b", "GET /blobs/b"]);
    const bad = api(() => Promise.resolve(json(200, { nope: 1 }))).a.remote("v1");
    await expect(bad.getBlob("b")).rejects.toThrow(/manifest/);
    expect(await bad.missing([])).toEqual([]);
  });

  it("reads in batches, and falls back on an older server's 405 (RFC 0002)", async () => {
    const calls: string[] = [];
    const fetch: Fetch = (url, init) => {
      calls.push(`${init.method ?? ""} ${url.replace("https://kura.test/v1/vaults/v1", "")} ${typeof init.body === "string" ? init.body : ""}`);
      if (url.endsWith("/blobs/batch")) return Promise.resolve(json(200, { manifests: [{ hash: "b", size: 2, chunks: ["h1", 3] }, { nope: 1 }], missing: ["x"] }));
      return Promise.resolve(new Response(new Uint8Array([7, 8]), { status: 200 }));
    };
    const r = api(fetch).a.remote("v1");
    expect(await r.getBlobs(["b", "x"])).toEqual([{ hash: "b", size: 2, chunks: ["h1"] }]);
    expect(await r.getChunkRecords(["h1"])).toEqual(new Uint8Array([7, 8]));
    expect(calls).toEqual(['POST /blobs/batch {"blobs":["b","x"]}', 'POST /chunks/batch {"chunks":["h1"]}']);
    const older = api(() => Promise.resolve(new Response(null, { status: 405 }))).a.remote("v1");
    expect(await older.getBlobs(["b"])).toBeNull();
    expect(await older.getChunkRecords(["h1"])).toBeNull();
    const gone = api(() => Promise.resolve(json(404, { type: "/v1/problems/not_found" }))).a.remote("v1");
    await expect(gone.getBlobs(["b"])).rejects.toMatchObject({ status: 404 });
    await expect(gone.getChunkRecords(["h1"])).rejects.toMatchObject({ status: 404 });
    const odd = api(() => Promise.resolve(json(200, { nope: 1 }))).a.remote("v1");
    await expect(odd.getBlobs(["b"])).rejects.toThrow(/manifests/);
  });
});
