import { strToU8, zipSync } from "fflate";
import { describe, expect, it } from "vitest";

import { base64, readRelease } from "./release-zip";

const manifest = (id: string) =>
  strToU8(JSON.stringify({ id, name: "Kanban", version: "2.0.51" }));

describe("a release zip", () => {
  it("takes the three files under one top folder, nothing else", () => {
    const got = readRelease(
      zipSync({
        "kanban-2.0.51/manifest.json": manifest("kanban"),
        "kanban-2.0.51/main.js": strToU8("main"),
        "kanban-2.0.51/README.md": strToU8("readme"),
        "kanban-2.0.51/sub/styles.css": strToU8("deeper, ignored"),
      }),
    );
    expect(got).toMatchObject({
      id: "kanban",
      name: "Kanban",
      version: "2.0.51",
    });
    expect("id" in got && got.styles).toBeFalsy();
    expect("id" in got && new TextDecoder().decode(got.main)).toBe("main");
  });

  it("takes them at the root too", () => {
    const got = readRelease(
      zipSync({
        "manifest.json": manifest("kanban"),
        "main.js": strToU8("m"),
        "styles.css": strToU8("s"),
      }),
    );
    expect("id" in got && new TextDecoder().decode(got.styles)).toBe("s");
  });

  it("refuses a bomb, too many entries, no manifest, the Kura plugin", () => {
    expect(
      readRelease(
        zipSync({
          "manifest.json": manifest("x"),
          "main.js": new Uint8Array(1 << 20),
        }),
      ),
    ).toEqual({ problem: "ratio" });
    const many: Record<string, Uint8Array> = { "manifest.json": manifest("x") };
    for (let i = 0; i < 64; i += 1) many[`f${i}`] = strToU8("x");
    expect(readRelease(zipSync(many))).toEqual({ problem: "tooMany" });
    expect(readRelease(zipSync({ "photo.jpg": strToU8("x") }))).toEqual({
      problem: "noPlugin",
    });
    expect(
      readRelease(zipSync({ "manifest.json": strToU8('{"id":"Bad Id"}') })),
    ).toEqual({ problem: "noPlugin" });
    expect(
      readRelease(
        zipSync({
          "manifest.json": manifest("weebo-kura"),
          "main.js": strToU8("m"),
        }),
      ),
    ).toEqual({ problem: "kura" });
    expect(readRelease(zipSync({ "manifest.json": manifest("x") }))).toEqual({
      problem: "noMain",
    });
    expect(readRelease(strToU8("not a zip"))).toEqual({
      problem: "unreadable",
    });
  });

  it("encodes bytes as base64", () => {
    expect(base64(new Uint8Array([0, 255, 104, 105]))).toBe("AP9oaQ==");
  });
});
