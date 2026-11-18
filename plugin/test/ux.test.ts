import { afterEach, describe, expect, it } from "vitest";
import { ago, roleName, setLanguage, t } from "../src/i18n";
import { consoleUrl, entryOf } from "../src/links";
import { icon, newConflicts, tone, word } from "../src/status";
import type { Situation } from "../src/status";

const report = { pushed: 0, pulled: 0, trashed: 0, conflicts: [], unsyncable: [], deferred: [], failed: [], refused: null, resynced: null };
const base: Situation = { signedIn: true, online: true, syncing: false, pending: 0, role: "editor", last: null, error: null };

afterEach(() => { setLanguage("en"); });

describe("words", () => {
  it("speak French when Obsidian does, English otherwise, and fill their slots", () => {
    setLanguage("fr");
    expect(word(base)).toBe("à jour");
    expect(word({ ...base, role: "viewer", pending: 1 })).toBe("lecture seule (lecteur)");
    expect(t("word.syncingN", { n: 3 })).toBe("synchronisation de 3");
    setLanguage("de");
    expect(word(base)).toBe("in sync");
    expect(roleName("owner")).toBe("owner");
  });

  it("say how long ago, coarsely", () => {
    const now = 10_000_000_000;
    expect(ago(now - 5_000, now)).toBe("just now");
    expect(ago(now - 3 * 60_000, now)).toBe("3 min ago");
    expect(ago(now - 2 * 3_600_000, now)).toBe("2 h ago");
    expect(ago(now - 3 * 86_400_000, now)).toBe("3 d ago");
  });
});

describe("the state's icon", () => {
  it("follows the tone, the word kept beside it", () => {
    expect(icon(tone(base))).toBe("cloud-check");
    expect(icon(tone({ ...base, syncing: true }))).toBe("refresh-cw");
    expect(icon(tone({ ...base, online: false }))).toBe("cloud-off");
    expect(icon(tone({ ...base, signedIn: false }))).toBe("alert-circle");
    expect(icon(tone({ ...base, last: { ...report, conflicts: ["a"] } }))).toBe("alert-triangle");
    expect(word({ ...base, last: { ...report, failed: [["a", "b"]] } })).toBe("unsyncable");
  });

  it("announces only the conflicts a run newly found", () => {
    expect(newConflicts(null, { ...report, conflicts: ["a"] })).toEqual(["a"]);
    expect(newConflicts({ ...report, conflicts: ["a"] }, { ...report, conflicts: ["a", "b"] })).toEqual(["b"]);
    expect(newConflicts({ ...report, conflicts: ["a"] }, null)).toEqual([]);
  });
});

describe("console links", () => {
  const state = JSON.stringify({ version: 1, entries: { "e-1": { path: "Sessions/s12.md" }, "e-2": { path: "a.md" } } });
  it("find a synced note's entry by its path, and nothing else", () => {
    expect(entryOf(state, "Sessions/s12.md")).toBe("e-1");
    expect(entryOf(state, "missing.md")).toBeNull();
    expect(entryOf("", "a.md")).toBeNull();
    expect(entryOf('{"entries":3}', "a.md")).toBeNull();
  });
  it("lead to the vault, a note, or its history", () => {
    expect(consoleUrl("https://k.example/", "v1")).toBe("https://k.example/app/vaults/v1");
    expect(consoleUrl("https://k.example", "v1", "e-1")).toBe("https://k.example/app/vaults/v1/notes/e-1");
    expect(consoleUrl("https://k.example", "v1", "e-1", true)).toBe("https://k.example/app/vaults/v1/notes/e-1/history");
  });
});
