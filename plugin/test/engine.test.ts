// The real WASM core, loaded as the plugin loads it, between two devices and
// a server in memory: the JavaScript/WASM boundary end to end (decision 54).
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { loadCore } from "../src/core";
import { ObsidianHost } from "../src/host";
import { Api } from "../src/remote";
import { parseOutcome } from "../src/types";
import { FakeServer, MemAdapter } from "./fakes";

const core = loadCore(new Uint8Array(readFileSync(new URL("../src/wasm/kura_bg.wasm", import.meta.url))));
const VAULT = "0193a9f0-0000-7000-8000-00000000000a";

function device(server: FakeServer, id: string, open: string[] = []) {
  const adapter = new MemAdapter();
  const host = new ObsidianHost({
    adapter,
    pluginDir: ".obsidian/plugins/weebo-kura",
    vault: VAULT,
    isOpen: (p) => open.includes(p),
    now: () => Date.now(),
    random: () => crypto.getRandomValues(new Uint8Array(16)),
  });
  const api = new Api({ server: "https://kura.test", token: () => Promise.resolve("t"), device: () => id, fetch: server.fetch, sleep: () => Promise.resolve() });
  const run = async (dryRun = false, allowDeletions = false) => {
    const options = JSON.stringify({ device: id, vault: VAULT, min: 64, avg: 256, max: 1024, maxFileSize: 1 << 20, dryRun, allowDeletions });
    return parseOutcome(await core.sync(host, api.remote(VAULT), await host.loadState(), options));
  };
  return { adapter, run };
}

describe("the WASM core through the plugin's host and remote", () => {
  it("syncs a note from one device to another", async () => {
    const server = new FakeServer();
    const a = device(server, "0193a9f0-0000-7000-8000-0000000000a1");
    const b = device(server, "0193a9f0-0000-7000-8000-0000000000b1");
    a.adapter.put("notes/hello.md", "# Hello\n".repeat(200));
    a.adapter.put(".obsidian/app.json", "{}");
    a.adapter.put("Q3: budget.md", "x");
    const dry = await a.run(true);
    expect(dry.report.pushed).toBe(1);
    expect(server.calls).toEqual([]);
    const ra = await a.run();
    expect(ra.report.pushed).toBe(1);
    // RFC 0002 phase 3: sent in batches, nothing one by one.
    expect(server.calls.filter((c) => c.includes("upload"))).toEqual(["POST /chunks/upload", "POST /blobs/upload"]);
    expect(server.calls.filter((c) => c.startsWith("PUT "))).toEqual([]);
    expect(ra.report.unsyncable.map(([p]) => p)).toEqual(["Q3: budget.md"]);
    expect(server.chunks.size).toBeGreaterThan(1);
    server.calls = [];
    const rb = await b.run();
    expect(rb.report.pulled).toBe(1);
    expect(b.adapter.text("notes/hello.md")).toBe("# Hello\n".repeat(200));
    // RFC 0002: read in batches, nothing one by one.
    expect(server.calls.filter((c) => c.includes("batch"))).toEqual(["POST /blobs/batch", "POST /chunks/batch"]);
    expect(server.calls.filter((c) => c.startsWith("GET /chunks/") || c.startsWith("GET /blobs/"))).toEqual([]);
    expect(b.adapter.text(".obsidian/app.json")).toBeUndefined();
    // The state persisted by the host is what the next run starts from.
    expect((await b.run()).report.pulled).toBe(0);
  });

  it("reads one by one from an older server without the batch routes (RFC 0002)", async () => {
    const server = new FakeServer();
    server.older = true;
    const a = device(server, "0193a9f0-0000-7000-8000-0000000000a9");
    const b = device(server, "0193a9f0-0000-7000-8000-0000000000b9");
    a.adapter.put("one.md", "first\n".repeat(100));
    a.adapter.put("two.md", "second\n".repeat(100));
    await a.run();
    // Sent one by one after one 405 per kind of upload.
    expect(server.calls.filter((c) => c.includes("upload"))).toEqual(["POST /chunks/upload", "POST /blobs/upload"]);
    expect(server.calls.filter((c) => c.startsWith("PUT /blobs/"))).toHaveLength(2);
    server.calls = [];
    expect((await b.run()).report.pulled).toBe(2);
    expect(b.adapter.text("two.md")).toBe("second\n".repeat(100));
    // One 405 per run, then single reads only.
    expect(server.calls.filter((c) => c.includes("batch"))).toEqual(["POST /blobs/batch"]);
    expect(server.calls.some((c) => c.startsWith("GET /chunks/"))).toBe(true);
  });

  it("does not write a note open in the editor, and retries later (decision 55)", async () => {
    const server = new FakeServer();
    const a = device(server, "0193a9f0-0000-7000-8000-0000000000a2");
    const open = ["note.md"];
    const b = device(server, "0193a9f0-0000-7000-8000-0000000000b2", open);
    a.adapter.put("note.md", "remote text\n");
    await a.run();
    const r = await b.run();
    expect(r.report.failed.map(([p]) => p)).toEqual(["note.md"]);
    expect(b.adapter.text("note.md")).toBeUndefined();
    open.pop();
    expect((await b.run()).report.pulled).toBe(1);
    expect(b.adapter.text("note.md")).toBe("remote text\n");
  });

  it("resyncs past the horizon, asking before trashing most files (§6.3)", async () => {
    const server = new FakeServer();
    const a = device(server, "0193a9f0-0000-7000-8000-0000000000a3");
    const b = device(server, "0193a9f0-0000-7000-8000-0000000000b3");
    a.adapter.put("gone.md", "deleted elsewhere\n");
    a.adapter.put("kept.md", "stays\n");
    await a.run();
    await b.run();
    b.adapter.put("new.md", "written while away\n");
    await a.adapter.remove("gone.md");
    await a.run();
    a.adapter.put("later.md", "moves the cursor past the purge\n");
    await a.run();
    server.purgeDeleted();
    // One of three local files, 33%: over the guard, nothing changes.
    await expect(b.run()).rejects.toMatchObject({ kind: "too_many_deletions", deletions: 1, files: 3 });
    expect(b.adapter.text("gone.md")).toBe("deleted elsewhere\n");
    const r = await b.run(false, true);
    expect(r.report).toMatchObject({ resynced: "cursor_expired", trashed: 1 });
    expect(b.adapter.text("gone.md")).toBeUndefined();
    expect(b.adapter.text(".trash/gone.md")).toBe("deleted elsewhere\n");
    expect(b.adapter.text("later.md")).toBe("moves the cursor past the purge\n");
    expect([...server.entries.values()].map((e) => e.path).sort()).toEqual(["kept.md", "later.md", "new.md"]);
  });

  it("rejects with the server's problem, and checks paths like the server", async () => {
    const server = new FakeServer();
    const refusing = async (url: string, init: RequestInit): Promise<Response> =>
      url.includes("/changes")
        ? new Response(JSON.stringify({ type: "/v1/problems/device_revoked" }), { status: 401 })
        : server.fetch(url, init);
    const host = new ObsidianHost({ adapter: new MemAdapter(), pluginDir: "p", vault: VAULT, isOpen: () => false, now: () => 1, random: () => new Uint8Array(16) });
    const api = new Api({ server: "https://kura.test", token: () => Promise.resolve("t"), device: () => "d", fetch: refusing, sleep: () => Promise.resolve() });
    const options = JSON.stringify({ device: VAULT, vault: VAULT, min: 64, avg: 256, max: 1024, maxFileSize: 1 << 20 });
    await expect(core.sync(host, api.remote(VAULT), "", options)).rejects.toMatchObject({ status: 401, kind: "device_revoked" });
    await expect(core.sync(host, api.remote(VAULT), "", "{")).rejects.toMatch(/options/);
    expect(core.pathProblem("notes/ok.md")).toBeUndefined();
    expect(core.pathProblem("../x.md")).toMatch(/not allowed/);
  });
});
