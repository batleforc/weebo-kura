import { describe, expect, it } from "vitest";
import { ObsidianHost } from "../src/host";
import { MemAdapter } from "./fakes";

function host(open: string[] = []) {
  const adapter = new MemAdapter();
  const h = new ObsidianHost({ adapter, pluginDir: ".obsidian/plugins/weebo-kura", vault: "v1", isOpen: (p) => open.includes(p), now: () => 42, random: () => new Uint8Array(16).fill(7) });
  return { adapter, h };
}

describe("the vault host", () => {
  it("lists notes, never dot-folders", async () => {
    const { adapter, h } = host();
    adapter.put("a.md", "a");
    adapter.put("dir/b.md", "bb");
    adapter.put(".obsidian/app.json", "{}");
    adapter.put(".trash/old.md", "x");
    const files = await h.list();
    expect(files.map((f) => f.path).sort()).toEqual(["a.md", "dir/b.md"]);
    expect(files.find((f) => f.path === "dir/b.md")?.size).toBe(2);
  });

  it("writes through a temporary file, creating folders, and never under an open note", async () => {
    const { adapter, h } = host(["open.md"]);
    adapter.put("x/old.md", "old");
    const meta = await h.write("x/old.md", new TextEncoder().encode("new"));
    expect(adapter.text("x/old.md")).toBe("new");
    expect(meta.size).toBe(3);
    await h.write("deep/er/n.md", new TextEncoder().encode("n"));
    expect(adapter.text("deep/er/n.md")).toBe("n");
    expect([...adapter.files.keys()].some((k) => k.includes("tmp-"))).toBe(false);
    await expect(h.write("open.md", new Uint8Array())).rejects.toThrow(/open in the editor/);
  });

  it("renames, trashes, reads, stats and keeps its state", async () => {
    const { adapter, h } = host(["busy.md"]);
    adapter.put("a.md", "a");
    adapter.put("b.md", "b");
    adapter.put("busy.md", "x");
    await expect(h.rename("a.md", "b.md")).rejects.toThrow(/already exists/);
    await expect(h.rename("busy.md", "c.md")).rejects.toThrow(/open/);
    expect((await h.rename("a.md", "sub/c.md")).path).toBe("sub/c.md");
    expect(new TextDecoder().decode(await h.read("sub/c.md"))).toBe("a");
    expect(await h.stat("nope.md")).toBeNull();
    expect(await h.stat("sub")).toBeNull();
    await h.trash("b.md");
    await expect(h.trash("busy.md")).rejects.toThrow(/open in the editor/);
    expect(adapter.trashed).toEqual(["b.md"]);
    expect(await h.loadState()).toBe("");
    await h.save('{"cursor":3}');
    expect(await h.loadState()).toBe('{"cursor":3}');
    expect(h.statePath()).toBe(".obsidian/plugins/weebo-kura/state-v1.json");
    expect(h.nowMs()).toBe(42);
    expect(h.random()[0]).toBe(7);
  });
});

describe("the state file", () => {
  it("is replaced whole, and a save killed after removing the old one still loads", async () => {
    const { adapter, h } = host();
    await h.save('{"cursor":1}');
    await h.save('{"cursor":2}');
    expect(await h.loadState()).toBe('{"cursor":2}');
    expect(adapter.files.has(`${h.statePath()}.tmp`)).toBe(false);
    // Killed between remove and rename: only the complete copy is left.
    await adapter.write(`${h.statePath()}.tmp`, '{"cursor":3}');
    await adapter.remove(h.statePath());
    expect(await h.loadState()).toBe('{"cursor":3}');
  });
});

describe("folders (RFC 0007)", () => {
  it("lists every folder but dot-folders, makes one, and removes only an empty one", async () => {
    const adapter = new MemAdapter();
    adapter.put("JDR/Sessions/s1.md", "x");
    await adapter.mkdir("JDR/Empty");
    await adapter.mkdir(".obsidian/plugins");
    const host = new ObsidianHost({ adapter, pluginDir: ".obsidian/plugins/weebo-kura", vault: "v", isOpen: () => false, now: () => 0, random: () => new Uint8Array(16) });
    expect((await host.folders()).sort()).toEqual(["JDR", "JDR/Empty", "JDR/Sessions"]);
    await host.makeFolder("New/Deep");
    expect(adapter.folders.has("New/Deep")).toBe(true);
    expect(await host.removeFolderIfEmpty("JDR/Sessions")).toBe(false);
    expect(adapter.folders.has("JDR/Sessions")).toBe(true);
    expect(await host.removeFolderIfEmpty("JDR/Empty")).toBe(true);
    expect(adapter.folders.has("JDR/Empty")).toBe(false);
    expect(await host.removeFolderIfEmpty("Gone")).toBe(false);
  });

  it("lists none in the configuration space", async () => {
    const adapter = new MemAdapter();
    await adapter.mkdir(".obsidian/snippets");
    const host = new ObsidianHost({ adapter, pluginDir: ".obsidian/plugins/weebo-kura", vault: "v", isOpen: () => false, now: () => 0, random: () => new Uint8Array(16), root: ".obsidian" });
    expect(await host.folders()).toEqual([]);
  });
});
