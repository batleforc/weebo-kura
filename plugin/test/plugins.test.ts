// Decision 34 on the device: received plugins stay off until agreed, a new
// code hash disables a running plugin before it lands, consent is local.
import { describe, expect, it } from "vitest";
import { ObsidianHost } from "../src/host";
import { PluginGuard, codeHashes, emptyConsent, forget, pluginOf, readConsent } from "../src/plugins";
import type { Consent, PluginRegistry } from "../src/plugins";
import { MemAdapter } from "./fakes";

class Registry implements PluginRegistry {
  enabled = new Set<string>();
  events: string[];
  constructor(events: string[]) {
    this.events = events;
  }
  isEnabled(id: string): Promise<boolean> {
    return Promise.resolve(this.enabled.has(id));
  }
  enable(id: string): Promise<void> {
    this.events.push(`enable ${id}`);
    this.enabled.add(id);
    return Promise.resolve();
  }
  disable(id: string): Promise<void> {
    this.events.push(`disable ${id}`);
    this.enabled.delete(id);
    return Promise.resolve();
  }
}

function setup(answers: boolean[], consent: Consent = emptyConsent()) {
  const events: string[] = [];
  const reg = new Registry(events);
  const asked: string[] = [];
  const guard = new PluginGuard(reg, consent, () => Promise.resolve(), (id) => {
    asked.push(id);
    return Promise.resolve(answers.shift() ?? false);
  });
  const adapter = new MemAdapter();
  const write = adapter.writeBinary.bind(adapter);
  adapter.writeBinary = (path, data) => {
    events.push(`write ${path}`);
    return write(path, data);
  };
  const host = new ObsidianHost({
    adapter,
    pluginDir: ".obsidian/plugins/weebo-kura",
    vault: "v",
    isOpen: () => false,
    now: () => 1,
    random: () => new Uint8Array(16),
    root: ".obsidian",
    keep: (p) => pluginOf(p) !== null || p.startsWith("snippets/"),
    stateName: "config-state",
    beforeWrite: (p) => guard.beforeWrite(p),
  });
  return { guard, reg, events, asked, consent, adapter, host };
}

const code = new TextEncoder().encode("module.exports = {};");

describe("synced plugins (decision 34)", () => {
  it("received_plugin_is_disabled: written off, asked once, never enabled on a no", async () => {
    const t = setup([false]);
    await t.host.write("plugins/dataview/main.js", code);
    expect(t.adapter.text(".obsidian/plugins/dataview/main.js")).toBe("module.exports = {};");
    expect(await t.reg.isEnabled("dataview")).toBe(false);
    await t.guard.review(new Map([["dataview", "h1"]]));
    expect(t.asked).toEqual(["dataview"]);
    expect(await t.reg.isEnabled("dataview")).toBe(false);
    // The same code is not asked about again.
    await t.host.write("plugins/dataview/manifest.json", new TextEncoder().encode("{}"));
    await t.guard.review(new Map([["dataview", "h1"]]));
    expect(t.asked).toEqual(["dataview"]);
  });

  it("treats MAIN.JS as code on a case-insensitive file system (review H2)", async () => {
    const t = setup([false], readConsent({ accepted: { dataview: "h1" } }));
    t.reg.enabled.add("dataview");
    await t.host.write("plugins/DataView/MAIN.JS", code);
    expect(t.events[0]).toBe("disable dataview");
    expect(pluginOf("Plugins/DataView/Main.js")).toEqual({ id: "dataview", code: true });
  });

  it("plugin_hash_change_disables_it before the new code lands, until agreed again", async () => {
    const consent = readConsent({ accepted: { dataview: "h1" } });
    const t = setup([true], consent);
    t.reg.enabled.add("dataview");
    await t.host.write("plugins/dataview/main.js", code);
    const disabled = t.events.indexOf("disable dataview");
    const written = t.events.findIndex((e) => e.startsWith("write"));
    expect(disabled).toBeGreaterThanOrEqual(0);
    expect(disabled).toBeLessThan(written);
    await t.guard.review(new Map([["dataview", "h2"]]));
    expect(t.asked).toEqual(["dataview"]);
    expect(await t.reg.isEnabled("dataview")).toBe(true);
    expect(t.consent.accepted.dataview).toBe("h2");
  });

  it("turns agreed code back on only where the guard turned it off; consent is this device's own", async () => {
    const t = setup([], readConsent({ accepted: { dataview: "h1" }, pending: ["dataview"], disabled: ["dataview"], declined: "junk" }));
    await t.guard.review(new Map([["dataview", "h1"]]));
    expect(t.asked).toEqual([]);
    expect(await t.reg.isEnabled("dataview")).toBe(true);
    // Kept off by the user: a settings change synced from elsewhere leaves it off (review M1).
    const off = setup([], readConsent({ accepted: { tasks: "h3" } }));
    await off.host.write("plugins/tasks/data.json", new TextEncoder().encode("{}"));
    await off.guard.review(new Map([["tasks", "h3"]]));
    expect(off.asked).toEqual([]);
    expect(await off.reg.isEnabled("tasks")).toBe(false);
    expect(t.consent.pending).toEqual([]);
    // A plugin nothing wrote here (installed on this device) is never asked about.
    await t.guard.review(new Map([["local", "h9"]]));
    expect(t.asked).toEqual([]);
  });

  it("reads the code hashes from the configuration state", () => {
    const state = JSON.stringify({
      entries: {
        a: { path: "plugins/dataview/main.js", blob: "h1" },
        b: { path: "plugins/dataview/manifest.json", blob: "m" },
        c: { path: "snippets/a.css", blob: "s" },
      },
    });
    expect([...codeHashes(state)]).toEqual([["dataview", "h1"]]);
    expect(codeHashes("not json").size).toBe(0);
    const kept: unknown = JSON.parse(forget(state, (p) => p.startsWith("plugins/")));
    expect(kept).toMatchObject({ entries: { a: { blob: "h1" }, b: { blob: "m" } } });
    expect(JSON.stringify(kept)).not.toContain("snippets");
    expect(forget("", () => false)).toBe("");
    expect(pluginOf("snippets/a.css")).toBeNull();
  });

  it("the configuration host lists .obsidian's kept files, relative to it", async () => {
    const t = setup([]);
    t.adapter.put(".obsidian/plugins/dataview/main.js", "x");
    t.adapter.put(".obsidian/snippets/a.css", "y");
    t.adapter.put(".obsidian/workspace.json", "{}");
    t.adapter.put("note.md", "z");
    const paths = (await t.host.list()).map((f) => f.path).sort();
    expect(paths).toEqual(["plugins/dataview/main.js", "snippets/a.css"]);
    expect(t.host.statePath()).toBe(".obsidian/plugins/weebo-kura/config-state-v.json");
  });
});
