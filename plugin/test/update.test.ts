// RFC 0010 §4.2 and §5.4: the version table, and the one-click update that
// writes nothing until all three files check out, and can go back.
import { describe, expect, it } from "vitest";
import { compareVersions, drift, fetchUpdate, goBack, install, parseVersion, previousVersion, releasePage, sha256 } from "../src/update";
import type { Get, UpdateFs } from "../src/update";
import { MemAdapter } from "./fakes";

const DIR = ".obsidian/plugins/weebo-kura";
const enc = (s: string): Uint8Array => new TextEncoder().encode(s);
const manifest = (version: string, id = "weebo-kura"): string => JSON.stringify({ id, version });

/** A server offering `version`, its plugin.json hashes taken from `files` unless overridden. */
async function serving(version: string, over: Partial<Record<string, string>> = {}, sums: Partial<Record<string, string>> = {}, checksums: string | null = null) {
  const files: Record<string, string> = { "main.js": `main ${version}`, "manifest.json": manifest(version), "styles.css": "css", ...over };
  const sha: Record<string, string> = {};
  for (const [k, v] of Object.entries(files)) sha[k] = await sha256(enc(v));
  const routes = new Map<string, string>([
    ["https://kura.test/app/plugin.json", JSON.stringify({ available: true, id: "weebo-kura", version, sha256: { ...sha, ...sums } })],
    ...Object.entries(files).map(([k, v]): [string, string] => [`https://kura.test/app/plugin/${k}`, v]),
  ]);
  if (checksums !== null) routes.set(`https://github.com/batleforc/weebo-kura/releases/download/v${version}/checksums.txt`, checksums);
  const urls: string[] = [];
  const get: Get = (url) => {
    urls.push(url);
    const body = routes.get(url);
    return Promise.resolve(body === undefined ? { status: 404, body: new ArrayBuffer(0) } : { status: 200, body: enc(body).slice().buffer });
  };
  return { get, sha, urls };
}

function running(): MemAdapter {
  const a = new MemAdapter();
  a.put(`${DIR}/main.js`, "main 0.3.0");
  a.put(`${DIR}/manifest.json`, manifest("0.3.0"));
  a.put(`${DIR}/styles.css`, "old css");
  a.put(`${DIR}/data.json`, "{}");
  return a;
}

const snapshot = (a: MemAdapter): Map<string, string | undefined> => new Map([...a.files.keys()].map((k) => [k, a.text(k)]));

describe("client versions (§4.2)", () => {
  it("compares major.minor.patch only, a pre-release as its release, no header as unknown", () => {
    expect(parseVersion("kura-server/0.3.1")).toEqual([0, 3, 1]);
    expect(parseVersion("0.3.1-rc.1+abc")).toEqual([0, 3, 1]);
    expect(parseVersion("latest")).toBeNull();
    expect(drift("0.3.0", "kura-server/0.3.0")).toBe("equal");
    expect(drift("0.3.0", "kura-server/0.3.1")).toBe("older");
    expect(drift("0.3.1", "kura-server/0.3.0")).toBe("newer");
    expect(drift("0.3.0", "kura-server/0.3.0-rc.2")).toBe("equal");
    expect(drift("0.9.0", "kura-server/0.10.0")).toBe("older");
    expect(drift("0.3.0", null)).toBe("unknown");
    expect(drift("0.3.0", "kura-server/dev")).toBe("unknown");
    expect(compareVersions("1.0.0", "0.99.99")).toBeGreaterThan(0);
    expect(releasePage("0.3.1")).toBe("https://github.com/batleforc/weebo-kura/releases/tag/v0.3.1");
  });
});

describe("the one-click update (§5.4)", () => {
  it("update_writes_only_three_files, keeping the running ones for a way back", async () => {
    const s = await serving("0.3.1");
    const c = await fetchUpdate("https://kura.test/", "0.3.0", s.get);
    expect(c).toMatchObject({ version: "0.3.1", source: "kura.test", verified: false });
    expect(c?.files.map((f) => [f.name, f.sha256])).toEqual(Object.entries(s.sha));
    const a = running();
    const before = snapshot(a);
    if (c === null) throw new Error("no candidate");
    await install(c, a, DIR);
    const changed = [...a.files.keys()].filter((k) => before.get(k) !== a.text(k)).sort();
    expect(changed).toEqual([
      `${DIR}/.kura-previous/main.js`, `${DIR}/.kura-previous/manifest.json`, `${DIR}/.kura-previous/styles.css`,
      `${DIR}/main.js`, `${DIR}/manifest.json`, `${DIR}/styles.css`,
    ]);
    expect(a.text(`${DIR}/main.js`)).toBe("main 0.3.1");
    expect(a.text(`${DIR}/data.json`)).toBe("{}");
    // Go back: the exact files that ran before.
    expect(await previousVersion(a, DIR)).toBe("0.3.0");
    expect(await goBack(a, DIR)).toBe("0.3.0");
    expect(a.text(`${DIR}/main.js`)).toBe("main 0.3.0");
    expect(a.text(`${DIR}/styles.css`)).toBe("old css");
  });

  it("is verified against the release's checksums.txt when it lists the same hashes", async () => {
    const plain = await serving("0.3.1");
    const lines = Object.entries(plain.sha).map(([k, v]) => `${v}  ${k}`).join("\n");
    expect((await fetchUpdate("https://kura.test", "0.3.0", (await serving("0.3.1", {}, {}, lines)).get))?.verified).toBe(true);
    const other = lines.replace(plain.sha["main.js"] ?? "", "0".repeat(64));
    expect((await fetchUpdate("https://kura.test", "0.3.0", (await serving("0.3.1", {}, {}, other)).get))?.verified).toBe(false);
  });

  it("refuses a hash mismatch, and update_failure_changes_nothing", async () => {
    const s = await serving("0.3.1", {}, { "main.js": "f".repeat(64) });
    await expect(fetchUpdate("https://kura.test", "0.3.0", s.get)).rejects.toThrow("main.js does not match its sha256");
    // Fetched only: nothing reaches the disk before every check passes.
    expect(s.urls.every((u) => u.startsWith("https://kura.test/app/"))).toBe(true);
    const a = running();
    const before = snapshot(a);
    const failing: UpdateFs = { exists: (x) => a.exists(x), readBinary: (x) => a.readBinary(x), mkdir: (x) => a.mkdir(x), writeBinary: (p: string, d: ArrayBuffer) => (p === `${DIR}/manifest.json` && new TextDecoder().decode(d).includes("0.3.1") ? Promise.reject(new Error("disk full")) : a.writeBinary(p, d)) };
    const c = await fetchUpdate("https://kura.test", "0.3.0", (await serving("0.3.1")).get);
    if (c === null) throw new Error("no candidate");
    await expect(install(c, failing, DIR)).rejects.toThrow("disk full");
    for (const name of ["main.js", "manifest.json", "styles.css"]) expect(a.text(`${DIR}/${name}`)).toBe(before.get(`${DIR}/${name}`));
  });

  it("checks the manifest's id and version", async () => {
    await expect(fetchUpdate("https://kura.test", "0.3.0", (await serving("0.3.1", { "manifest.json": manifest("0.3.1", "evil") })).get)).rejects.toThrow("not Kura Sync's");
    await expect(fetchUpdate("https://kura.test", "0.3.0", (await serving("0.3.1", { "manifest.json": manifest("0.3.2") })).get)).rejects.toThrow("says 0.3.2, not 0.3.1");
  });

  it("never downgrades, and offers nothing without a console", async () => {
    await expect(fetchUpdate("https://kura.test", "0.3.1", (await serving("0.3.0")).get)).rejects.toThrow("0.3.0 is not newer than 0.3.1");
    await expect(fetchUpdate("https://kura.test", "0.3.1", (await serving("0.3.1")).get)).rejects.toThrow("not newer");
    const none: Get = () => Promise.resolve({ status: 404, body: new ArrayBuffer(0) });
    expect(await fetchUpdate("https://kura.test", "0.3.0", none)).toBeNull();
    const off: Get = () => Promise.resolve({ status: 200, body: enc('{"available":false}').slice().buffer });
    expect(await fetchUpdate("https://kura.test", "0.3.0", off)).toBeNull();
    const broken: Get = () => Promise.resolve({ status: 500, body: new ArrayBuffer(0) });
    await expect(fetchUpdate("https://kura.test", "0.3.0", broken)).rejects.toThrow("answered 500");
  });

  it("cannot go back without a kept version", async () => {
    await expect(goBack(running(), DIR)).rejects.toThrow("no previous version");
  });
});
