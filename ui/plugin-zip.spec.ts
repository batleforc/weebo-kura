import { createHash } from "node:crypto";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { crc32, inflateRawSync } from "node:zlib";
import { describe, expect, it } from "vitest";

import { pluginZip, zip } from "./plugin-zip";

/** The entries of a zip, read back from its central directory. */
function read(z: Buffer): Record<string, string> {
  const end = z.length - 22;
  expect(z.readUInt32LE(end)).toBe(0x06054b50);
  let at = z.readUInt32LE(end + 16);
  const out: Record<string, string> = {};
  for (let i = 0; i < z.readUInt16LE(end + 10); i += 1) {
    expect(z.readUInt32LE(at)).toBe(0x02014b50);
    const size = z.readUInt32LE(at + 20);
    const nameLen = z.readUInt16LE(at + 28);
    const offset = z.readUInt32LE(at + 42);
    const name = z.subarray(at + 46, at + 46 + nameLen).toString();
    const lnLen = z.readUInt16LE(offset + 26);
    const data = inflateRawSync(
      z.subarray(offset + 30 + lnLen, offset + 30 + lnLen + size),
    );
    expect(crc32(data)).toBe(z.readUInt32LE(at + 16));
    out[name] = data.toString();
    at += 46 + nameLen;
  }
  return out;
}

describe("the plugin zip", () => {
  it("holds each file under the plugin's folder, intact", () => {
    const z = Buffer.from(
      zip([
        ["weebo-kura/main.js", Buffer.from("console.log('kura');".repeat(50))],
        ["weebo-kura/manifest.json", Buffer.from('{"id":"weebo-kura"}')],
      ]),
    );
    expect(read(z)).toEqual({
      "weebo-kura/main.js": "console.log('kura');".repeat(50),
      "weebo-kura/manifest.json": '{"id":"weebo-kura"}',
    });
  });
});

describe("plugin.json", () => {
  it("names the SHA-256 of each file emitted under plugin/", () => {
    const dir = mkdtempSync(join(tmpdir(), "kura-plugin-"));
    writeFileSync(join(dir, "main.js"), "console.log(1);");
    writeFileSync(
      join(dir, "manifest.json"),
      '{"id":"weebo-kura","version":"0.3.0"}',
    );
    writeFileSync(join(dir, "styles.css"), ".a{}");
    const out = new Map<string, string | Uint8Array>();
    const ctx = {
      emitFile: (f: { fileName: string; source: string | Uint8Array }) =>
        out.set(f.fileName, f.source),
    };
    const hook = pluginZip(dir).generateBundle;
    const run = typeof hook === "function" ? hook : hook?.handler;
    run?.call(ctx as never, {} as never, {}, true);
    const json: unknown = JSON.parse(String(out.get("plugin.json")));
    const sha256 = (json as { sha256: Record<string, string> }).sha256;
    for (const f of ["main.js", "manifest.json", "styles.css"]) {
      const file = out.get(`plugin/${f}`);
      expect(file, f).toBeDefined();
      expect(sha256[f]).toBe(
        createHash("sha256")
          .update(file ?? "")
          .digest("hex"),
      );
    }
    expect(out.has("kura-sync.zip")).toBe(true);
  });
});
