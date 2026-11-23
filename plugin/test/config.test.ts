// RFC 0010 on a device, with the real WASM core: the three configuration
// passes, each with its own state, the vault's plugin code read-only for a
// member, written disabled, its updates waiting, and "never" kept off.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { syncConfig } from "../src/config";
import type { ConfigRun } from "../src/config";
import { loadCore } from "../src/core";
import { emptyConsent } from "../src/plugins";
import type { PluginRegistry } from "../src/plugins";
import { Api } from "../src/remote";
import type { VaultView } from "../src/remote";
import { MemAdapter, SpacedServer } from "./fakes";

const core = loadCore(new Uint8Array(readFileSync(new URL("../src/wasm/kura_bg.wasm", import.meta.url))));
const VAULT = "0193a9f0-0000-7000-8000-00000000000c";
const ALL = ["plugins", "snippets", "themes", "hotkeys"];

class Registry implements PluginRegistry {
  enabled = new Set<string>();
  isEnabled(id: string): Promise<boolean> {
    return Promise.resolve(this.enabled.has(id));
  }
  aliased(): Promise<boolean> {
    return Promise.resolve(false);
  }
  enable(id: string): Promise<void> {
    this.enabled.add(id);
    return Promise.resolve();
  }
  disable(id: string): Promise<void> {
    this.enabled.delete(id);
    return Promise.resolve();
  }
}

function device(server: SpacedServer, id: string, role = "editor") {
  const adapter = new MemAdapter();
  const reg = new Registry();
  const consent = emptyConsent();
  const asked: string[] = [];
  const answers: { plugin: boolean; vaultPlugin: boolean | "never"; update: "use" | "later" | "never" } = { plugin: true, vaultPlugin: true, update: "use" };
  const vault: VaultView = { id: VAULT, name: "notes", role, chunking: { min: 64, avg: 256, max: 1024 }, configSync: ALL, configShared: [], vaultPlugins: true };
  const api = new Api({ server: "https://kura.test", token: () => Promise.resolve("t"), device: () => id, fetch: server.fetch, sleep: () => Promise.resolve() });
  const run: ConfigRun = {
    core,
    api,
    adapter,
    reg,
    vault,
    configDir: ".obsidian",
    pluginDir: ".obsidian/plugins/weebo-kura",
    own: "plugins/weebo-kura",
    consent,
    save: () => Promise.resolve(),
    options: { device: id, vault: VAULT, min: 64, avg: 256, max: 1024, maxFileSize: 1 << 20, dryRun: false, allowDeletions: false, folders: true },
    now: () => Date.now(),
    random: () => crypto.getRandomValues(new Uint8Array(16)),
    ask: {
      plugin: (p) => { asked.push(`plugin ${p}`); return Promise.resolve(answers.plugin); },
      vaultPlugin: (p) => { asked.push(`vault ${p}`); return Promise.resolve(answers.vaultPlugin); },
      update: (p) => { asked.push(`update ${p}`); return Promise.resolve(answers.update); },
    },
  };
  return { adapter, reg, consent, asked, answers, vault, sync: () => syncConfig(run) };
}

const A = "0193a9f0-0000-7000-8000-0000000000c1";
const B = "0193a9f0-0000-7000-8000-0000000000c2";
const M = "0193a9f0-0000-7000-8000-0000000000c3";
const ops = (s: SpacedServer, space: string): string[] => s.space(space).ops;

/** A manager's device with the plugin `tasks`, installed on the vault, and a member's. */
async function installed() {
  const server = new SpacedServer();
  server.shares.vaultPlugins = ["tasks"];
  const m = device(server, M, "manager");
  m.adapter.put(".obsidian/plugins/tasks/manifest.json", '{"id":"tasks","version":"1.0.0"}');
  m.adapter.put(".obsidian/plugins/tasks/main.js", "v1");
  m.adapter.put(".obsidian/plugins/tasks/data.json", "{}");
  expect(await m.sync()).toEqual([]);
  const d = device(server, B);
  return { server, m, d };
}

describe("the configuration passes (RFC 0010 §5.2)", () => {
  it("files each path in its space, each pass with its own state, the server's copy first", async () => {
    const server = new SpacedServer();
    server.shares.shared = ["snippets"];
    const a = device(server, A);
    a.adapter.put(".obsidian/snippets/a.css", "from A");
    a.adapter.put(".obsidian/hotkeys.json", "{}");
    a.adapter.put(".obsidian/plugins/weebo-kura/data.json", "secret");
    expect(await a.sync()).toEqual([]);
    expect(ops(server, "config-shared")).toEqual(["put snippets/a.css"]);
    expect(ops(server, `config:${A}`)).toEqual(["put hotkeys.json"]);
    expect(ops(server, "config-vault")).toEqual([]);
    expect(a.adapter.files.has(`.obsidian/plugins/weebo-kura/config-shared-state-${VAULT}.json`)).toBe(true);
    expect(a.adapter.files.has(`.obsidian/plugins/weebo-kura/config-state-${VAULT}.json`)).toBe(true);
    // Another profile: the shared copy wins over its local default (preferRemote).
    const b = device(server, B);
    b.adapter.put(".obsidian/snippets/a.css", "B's default");
    expect(await b.sync()).toEqual([]);
    expect(b.adapter.text(".obsidian/snippets/a.css")).toBe("from A");
    expect(b.adapter.text(".trash/.obsidian/snippets/a.css")).toBe("B's default");
    expect(b.adapter.text(".obsidian/hotkeys.json")).toBeUndefined();
    expect(ops(server, "config-shared")).toEqual(["put snippets/a.css"]);
  });

  it("goes on when one pass fails", async () => {
    const server = new SpacedServer();
    const fetch = server.fetch;
    server.fetch = (url, init) => (url.includes("space=config-vault") ? Promise.reject(new Error("down")) : fetch(url, init));
    const a = device(server, A);
    a.adapter.put(".obsidian/hotkeys.json", "{}");
    const failures = await a.sync();
    expect(failures.map((f) => f.space)).toEqual(["config-vault"]);
    expect(ops(server, `config:${A}`)).toEqual(["put hotkeys.json"]);
  });

  it("vault_plugin_arrives_disabled: the vault's code only, written off, asked once", async () => {
    const { server, m, d } = await installed();
    expect(ops(server, "config-vault").sort()).toEqual(["put plugins/tasks/main.js", "put plugins/tasks/manifest.json"]);
    // Its settings are the manager's own, per profile.
    expect(ops(server, `config:${M}`)).toEqual(["put plugins/tasks/data.json"]);
    expect(m.asked).toEqual([]);
    d.answers.vaultPlugin = false;
    await d.sync();
    expect(d.adapter.text(".obsidian/plugins/tasks/main.js")).toBe("v1");
    expect(d.adapter.text(".obsidian/plugins/tasks/data.json")).toBeUndefined();
    expect(d.reg.enabled.has("tasks")).toBe(false);
    expect(d.asked).toEqual(["vault tasks"]);
    await d.sync();
    expect(d.asked).toEqual(["vault tasks"]);
  });

  it("a member never pushes the vault's code, even edited locally", async () => {
    const { server, d } = await installed();
    await d.sync();
    const before = [...ops(server, "config-vault")];
    const uploads = server.main.calls.length;
    d.adapter.put(".obsidian/plugins/tasks/main.js", "tampered");
    await d.adapter.remove(".obsidian/plugins/tasks/manifest.json");
    expect(await d.sync()).toEqual([]);
    expect(ops(server, "config-vault")).toEqual(before);
    expect(server.main.calls.slice(uploads).filter((c) => c.startsWith("POST /chunks") || c.startsWith("POST /blobs/upload"))).toEqual([]);
  });

  it("never on this device: not written again, until lifted", async () => {
    const { server, m, d } = await installed();
    d.answers.vaultPlugin = "never";
    await d.sync();
    expect(d.consent.never).toEqual(["tasks"]);
    m.adapter.put(".obsidian/plugins/tasks/main.js", "v2");
    await m.sync();
    expect(ops(server, "config-vault").filter((o) => o === "put plugins/tasks/main.js")).toHaveLength(2);
    await d.sync();
    expect(d.adapter.text(".obsidian/plugins/tasks/main.js")).toBe("v1");
    expect(d.asked).toEqual(["vault tasks"]);
  });

  it("an agreed vault plugin keeps running; new code waits until agreed (decision 10)", async () => {
    const { m, d } = await installed();
    await d.sync();
    expect(d.reg.enabled.has("tasks")).toBe(true);
    m.adapter.put(".obsidian/plugins/tasks/main.js", "v2");
    await m.sync();
    d.answers.update = "later";
    await d.sync();
    expect(d.asked).toEqual(["vault tasks", "update tasks"]);
    expect(d.adapter.text(".obsidian/plugins/tasks/main.js")).toBe("v1");
    expect(d.adapter.text(".obsidian/plugins/tasks/.kura-pending/main.js")).toBe("v2");
    expect(d.reg.enabled.has("tasks")).toBe(true);
    // Not asked again for that code; the waiting file is not a local edit.
    await d.sync();
    expect(d.asked).toHaveLength(2);
    // A newer one is asked about, and swapped in on yes.
    m.adapter.put(".obsidian/plugins/tasks/main.js", "v3");
    await m.sync();
    d.answers.update = "use";
    await d.sync();
    expect(d.asked).toEqual(["vault tasks", "update tasks", "update tasks"]);
    expect(d.adapter.text(".obsidian/plugins/tasks/main.js")).toBe("v3");
    expect(d.adapter.files.has(".obsidian/plugins/tasks/.kura-pending/main.js")).toBe(false);
    expect(d.reg.enabled.has("tasks")).toBe(true);
    // Settled: nothing more to do.
    await d.sync();
    expect(d.asked).toHaveLength(3);
  });

  it("never on an update: the agreed version is turned off and the new code dropped", async () => {
    const { m, d } = await installed();
    await d.sync();
    m.adapter.put(".obsidian/plugins/tasks/main.js", "v2");
    await m.sync();
    d.answers.update = "never";
    await d.sync();
    expect(d.consent.never).toEqual(["tasks"]);
    expect(d.reg.enabled.has("tasks")).toBe(false);
    expect(d.adapter.text(".obsidian/plugins/tasks/main.js")).toBe("v1");
    expect(d.adapter.files.has(".obsidian/plugins/tasks/.kura-pending/main.js")).toBe(false);
    await d.sync();
    expect(d.asked).toEqual(["vault tasks", "update tasks"]);
  });

  it("a manager removes a plugin from the vault by removing its code in the vault pass", async () => {
    const { server, m } = await installed();
    await m.adapter.trashLocal(".obsidian/plugins/tasks/main.js");
    await m.adapter.trashLocal(".obsidian/plugins/tasks/manifest.json");
    expect(await m.sync()).toEqual([]);
    expect(ops(server, "config-vault").filter((o) => o.startsWith("delete"))).toHaveLength(2);
    expect([...server.space("config-vault").entries.values()].every((e) => e.deleted)).toBe(true);
  });
});
