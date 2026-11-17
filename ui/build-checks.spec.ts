import { describe, expect, it } from "vitest";

import { gzippedBytes, inlineScripts } from "./build-checks";

describe("build checks", () => {
  it("refuses inline script, accepts src= scripts", () => {
    const ok = `<script type="module" crossorigin src="/app/assets/index.js"></script>`;
    expect(inlineScripts(ok)).toEqual([]);
    expect(inlineScripts(`${ok}<script>alert(1)</script>`)).toHaveLength(1);
    expect(inlineScripts(`<SCRIPT type="module">x()</SCRIPT >`)).toHaveLength(
      1,
    );
    expect(inlineScripts(`<script src="/a.js">x()</script>`)).toHaveLength(1);
  });

  it("measures JS and CSS only, gzipped", () => {
    const js = "a".repeat(10_000);
    const size = gzippedBytes([
      { fileName: "a.js", content: js },
      { fileName: "font.woff2", content: new Uint8Array(50_000) },
    ]);
    expect(size).toBeGreaterThan(0);
    expect(size).toBeLessThan(js.length);
  });
});
