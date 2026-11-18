import { crc32, inflateRawSync } from "node:zlib";
import { describe, expect, it } from "vitest";

import { zip } from "./plugin-zip";

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
