/**
 * The Obsidian plugin, served by the console (`/app/kura-sync.zip`): the
 * built plugin/ (main.js, manifest.json, styles.css) zipped into the folder
 * Obsidian expects, `<manifest id>/`. `/app/plugin.json` says whether this
 * build carries it, which version and the SHA-256 of each file; the three
 * files are also served unzipped under `/app/plugin/` for the plugin's
 * one-click update (RFC 0010 §6.5). `task plugin:build` must run before
 * `vite build` for them to be there.
 */
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { crc32, deflateRawSync } from "node:zlib";

import type { Plugin } from "vite";

/** A deflated zip of `files` (name → bytes): local headers, central directory, end record. */
export function zip(files: [string, Uint8Array][]): Uint8Array {
  const local: Buffer[] = [];
  const central: Buffer[] = [];
  let offset = 0;
  for (const [name, data] of files) {
    const n = Buffer.from(name, "utf8");
    const body = deflateRawSync(data, { level: 9 });
    const crc = crc32(data);
    // 1980-01-01 00:00: a fixed date keeps the build reproducible.
    const common = (h: Buffer, at: number): void => {
      h.writeUInt16LE(20, at); // version needed
      h.writeUInt16LE(0x0800, at + 2); // UTF-8 names
      h.writeUInt16LE(8, at + 4); // deflate
      h.writeUInt16LE(0, at + 6); // time
      h.writeUInt16LE(0x21, at + 8); // date
      h.writeUInt32LE(crc, at + 10);
      h.writeUInt32LE(body.length, at + 14);
      h.writeUInt32LE(data.length, at + 18);
      h.writeUInt16LE(n.length, at + 22);
    };
    const lh = Buffer.alloc(30);
    lh.writeUInt32LE(0x04034b50, 0);
    common(lh, 4);
    local.push(lh, n, body);
    const ch = Buffer.alloc(46);
    ch.writeUInt32LE(0x02014b50, 0);
    ch.writeUInt16LE(20, 4); // made by
    common(ch, 6);
    ch.writeUInt32LE(offset, 42);
    central.push(ch, n);
    offset += lh.length + n.length + body.length;
  }
  const dir = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(files.length, 8);
  end.writeUInt16LE(files.length, 10);
  end.writeUInt32LE(dir.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...local, dir, end]);
}

export function pluginZip(
  dir = join(import.meta.dirname, "../plugin"),
): Plugin {
  return {
    name: "kura:plugin-zip",
    apply: "build",
    generateBundle() {
      const main = join(dir, "main.js");
      if (!existsSync(main)) {
        // The image sets it: a release must not ship a console with no plugin.
        if (process.env.KURA_PLUGIN_REQUIRED)
          this.error(
            "plugin/main.js is missing and KURA_PLUGIN_REQUIRED is set",
          );
        this.warn(
          "plugin/main.js is missing (task plugin:build): the console offers no plugin download",
        );
        this.emitFile({
          type: "asset",
          fileName: "plugin.json",
          source: '{"available":false}\n',
        });
        return;
      }
      const manifest: unknown = JSON.parse(
        readFileSync(join(dir, "manifest.json"), "utf8"),
      );
      const field = (k: string): string => {
        const v =
          typeof manifest === "object" && manifest !== null
            ? Reflect.get(manifest, k)
            : undefined;
        if (typeof v !== "string" || !/^[\w.-]+$/.test(v))
          throw new Error(`plugin/manifest.json: bad ${k}`);
        return v;
      };
      const id = field("id");
      const names = ["main.js", "manifest.json", "styles.css"];
      const read = names.map((f): [string, Buffer] => [
        f,
        readFileSync(join(dir, f)),
      ]);
      const sha256: Record<string, string> = {};
      for (const [f, data] of read) {
        sha256[f] = createHash("sha256").update(data).digest("hex");
        this.emitFile({ type: "asset", fileName: `plugin/${f}`, source: data });
      }
      const bytes = zip(read.map(([f, data]) => [`${id}/${f}`, data]));
      this.emitFile({
        type: "asset",
        fileName: "kura-sync.zip",
        source: bytes,
      });
      this.emitFile({
        type: "asset",
        fileName: "plugin.json",
        source: `${JSON.stringify({ available: true, id, version: field("version"), file: "kura-sync.zip", bytes: bytes.length, sha256 })}\n`,
      });
    },
  };
}
