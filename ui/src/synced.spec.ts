import { describe, expect, it } from "vitest";

import type { EntryState } from "@/api/generated/types.gen";
import {
  categoryOf,
  eachLimited,
  formatSize,
  group,
  hiddenReason,
  isHidden,
  manifestProblem,
  readManifest,
  shortName,
} from "./synced";

const entry = (
  path: string,
  blob: string | null = "b",
  size = 10,
): EntryState => ({
  entry: path,
  path,
  revision: "r",
  seq: 1,
  size,
  deleted: false,
  blob,
});

describe("synced configuration", () => {
  it("files paths by category as kura-core does", () => {
    expect(categoryOf("snippets/a/b.css")).toBe("snippets");
    expect(categoryOf("Themes/Minimal/theme.css")).toBe("themes");
    expect(categoryOf("plugins/dv/main.js")).toBe("plugins");
    expect(categoryOf("hotkeys.json")).toBe("hotkeys");
    for (const no of [
      "plugins/weebo-kura/main.js",
      "plugins/x",
      "snippets",
      "app.json",
    ])
      expect(categoryOf(no), no).toBeNull();
  });

  it("groups plugins by id, with their manifest, and says why a file is hidden", () => {
    const g = group([
      entry("plugins/dv/manifest.json"),
      entry("plugins/dv/data.json", null),
      entry("plugins/dv/main.js", null),
      entry("plugins/aa/main.js", null),
      entry("snippets/wide.css"),
      { ...entry("snippets/old.css"), deleted: true },
      entry("hotkeys.json"),
    ]);
    expect(g.plugins.map((p) => p.id)).toEqual(["aa", "dv"]);
    expect(g.plugins[1]?.manifest?.path).toBe("plugins/dv/manifest.json");
    expect(g.snippets.map((e) => e.path)).toEqual(["snippets/wide.css"]);
    expect(g.hotkeys).toHaveLength(1);
    expect(isHidden(entry("plugins/dv/data.json", null))).toBe(true);
    expect(hiddenReason("plugins/dv/data.json")).toBe("settings");
    expect(hiddenReason("plugins/dv/MAIN.JS")).toBe("code");
    expect(shortName("plugins/dv/styles.css")).toBe("styles.css");
    expect(shortName("themes/Minimal/theme.css")).toBe("Minimal/theme.css");
    expect(manifestProblem(g.plugins[0] ?? { id: "", files: [] })).toBe(
      "missing",
    );
    expect(manifestProblem(g.plugins[1] ?? { id: "", files: [] })).toBeNull();
    expect(
      manifestProblem({
        id: "x",
        files: [],
        manifest: entry("plugins/x/manifest.json", "b", 20_000),
      }),
    ).toBe("tooLarge");
  });

  it("reads a manifest's name and version as text, and nothing else", () => {
    const enc = (s: string) => new TextEncoder().encode(s);
    expect(readManifest(enc('{"name":"Dataview","version":"0.5.68"}'))).toEqual(
      {
        name: "Dataview",
        version: "0.5.68",
      },
    );
    expect(readManifest(enc('{"name":"<b>x</b>"}'))?.name).toBe("<b>x</b>");
    expect(
      readManifest(enc('{"name":"' + "n".repeat(500) + '"}'))?.name,
    ).toHaveLength(100);
    for (const bad of ["not json", "[]", '{"name":3}', '{"name":"  "}', "null"])
      expect(readManifest(enc(bad)), bad).toBeNull();
    expect(readManifest(new Uint8Array(17 * 1024))).toBeNull();
  });

  it("formats sizes in the reader's locale", () => {
    expect(formatSize(1234, "en")).toBe("1.2 kB");
    expect(formatSize(2_300_000, "en")).toBe("2.3 MB");
    expect(formatSize(1234, "fr")).toMatch(/^1,2\s?ko$/);
  });

  it("runs a few at a time, a failure never stopping the others", async () => {
    let running = 0;
    let most = 0;
    const done: number[] = [];
    await eachLimited([1, 2, 3, 4, 5, 6], 2, async (n) => {
      running += 1;
      most = Math.max(most, running);
      await new Promise((r) => setTimeout(r, 1));
      running -= 1;
      if (n === 3) throw new Error("one");
      done.push(n);
    });
    expect(most).toBe(2);
    expect(done.sort()).toEqual([1, 2, 4, 5, 6]);
  });
});
