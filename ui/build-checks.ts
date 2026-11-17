/**
 * Build-time checks of RFC 0003, run by `vite build` (so by `task ui:build`
 * and the image build in CI): no inline script in any built HTML (T8,
 * `console_has_no_inline_script`), and the bundle within its budget (§10.3).
 */
import { gzipSync } from "node:zlib";

import type { Plugin } from "vite";

/** §10.3: 300 kB gzipped. */
export const BUDGET_BYTES = 300_000;

/** The `<script>` tags of `html` that are not a bare `src=` reference. */
export function inlineScripts(html: string): string[] {
  return [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script\s*>/gi)]
    .filter(
      ([, attrs = "", body = ""]) =>
        !/\bsrc\s*=/i.test(attrs) || body.trim() !== "",
    )
    .map(([tag]) => tag.slice(0, 80));
}

/** Gzipped size of the JavaScript and CSS among `files`. */
export function gzippedBytes(
  files: { fileName: string; content: string | Uint8Array }[],
): number {
  return files
    .filter((f) => /\.(js|css)$/.test(f.fileName))
    .reduce((sum, f) => sum + gzipSync(f.content, { level: 9 }).length, 0);
}

export function buildChecks(): Plugin {
  return {
    name: "kura:build-checks",
    apply: "build",
    writeBundle(_, bundle) {
      const files = Object.values(bundle).map((f) => ({
        fileName: f.fileName,
        content: f.type === "chunk" ? f.code : f.source,
      }));
      for (const f of files.filter((f) => f.fileName.endsWith(".html"))) {
        const html =
          typeof f.content === "string"
            ? f.content
            : new TextDecoder().decode(f.content);
        const found = inlineScripts(html);
        if (found.length > 0)
          this.error(
            `${f.fileName} has inline script, refused by the CSP (T8): ${found.join(", ")}`,
          );
      }
      // shortcut: counts the reader's chunks too, so it is stricter than
      // §10.3 ("without the reader"); split them out if the total nears the budget.
      const size = gzippedBytes(files);
      const line = `bundle: ${(size / 1000).toFixed(1)} kB gzipped (JS + CSS), budget ${BUDGET_BYTES / 1000} kB`;
      if (size > BUDGET_BYTES) this.error(`${line}: over budget (§10.3)`);
      this.info(line);
    },
  };
}
