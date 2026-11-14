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
  const run = async (dryRun = false) => {
    const options = JSON.stringify({ device: id, vault: VAULT, min: 64, avg: 256, max: 1024, maxFileSize: 1 << 20, dryRun });
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
    expect(ra.report.unsyncable.map(([p]) => p)).toEqual(["Q3: budget.md"]);
    expect(server.chunks.size).toBeGreaterThan(1);
    const rb = await b.run();
    expect(rb.report.pulled).toBe(1);
    expect(b.adapter.text("notes/hello.md")).toBe("# Hello\n".repeat(200));
    expect(b.adapter.text(".obsidian/app.json")).toBeUndefined();
    // The state persisted by the host is what the next run starts from.
    expect((await b.run()).report.pulled).toBe(0);
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
