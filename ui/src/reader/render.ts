/**
 * A note as an HTML document for the sandboxed frame (RFC 0005 §6.5, T6):
 * Markdown with raw HTML off, wikilinks and links resolved only against the
 * listed entries, images only from the vault, the result sanitised by
 * DOMPurify, then wrapped with the one bridge script. Nothing here is put in
 * the application's own DOM.
 */

import DOMPurify from "dompurify";
import type {
  MarkdownIt as MarkdownItInstance,
  StateInline,
} from "markdown-it";
import MarkdownIt from "markdown-it";

import type { EntryState } from "@/api/generated/types.gen";
import { BRIDGE } from "./bridge";
import { resolver } from "./tree";

/** Internal links carry the entry id in their fragment, and nothing else. */
export const ENTRY_LINK = "#entry:";
/** The largest note rendered; larger notes are offered as a download. */
export const MAX_RENDER = 1 << 20;
/** The largest image inlined; larger images become a link to the file. */
export const MAX_IMAGE = 8 << 20;
/** The largest Mermaid source drawn; a larger one stays a code block. */
export const MAX_DIAGRAM = 50_000;

const IMAGE = /\.(png|jpe?g|gif|webp|avif|svg)$/i;

/** `[[target|label]]` and `![[target]]`, as one inline token. */
function wikilinks(md: MarkdownItInstance): void {
  md.inline.ruler.before(
    "link",
    "wikilink",
    (state: StateInline, silent: boolean) => {
      const start = state.pos;
      const embed = state.src.startsWith("![[", start);
      const open = embed ? start + 3 : start + 2;
      if (!embed && !state.src.startsWith("[[", start)) return false;
      const close = state.src.indexOf("]]", open);
      if (close < 0) return false;
      const inner = state.src.slice(open, close);
      if (inner.includes("\n") || inner.length === 0) return false;
      if (!silent) {
        const [target = "", label] = inner.split("|");
        const token = state.push("wikilink", "", 0);
        token.meta = {
          target: target.trim(),
          label: (label ?? target).trim(),
          embed,
        };
      }
      state.pos = close + 2;
      return true;
    },
  );
}

export interface Rendered {
  /** The sanitised body; images still to fetch carry `data-entry`. */
  body: string;
  /** The entries whose content is embedded as an image. */
  images: EntryState[];
}

/** Markdown to a sanitised body, links resolved against `entries`. */
export function renderNote(
  markdown: string,
  entries: readonly EntryState[],
): Rendered {
  const find = resolver(entries);
  const images = new Map<string, EntryState>();
  const md = new MarkdownIt({ html: false, linkify: true });
  md.use(wikilinks);
  const esc = md.utils.escapeHtml;
  const internal = (e: EntryState, label: string) =>
    `<a href="${ENTRY_LINK}${esc(e.entry)}">${esc(label)}</a>`;
  // A target not listed is plain text: no hint of whether it exists (T2).
  const hidden = (label: string) =>
    `<span class="unshared" tabindex="0" title="not shared with you">${esc(label)}</span>`;
  const image = (e: EntryState, alt: string) => {
    images.set(e.entry, e);
    return `<img data-entry="${esc(e.entry)}" alt="${esc(alt)}">`;
  };

  md.renderer.rules.wikilink = (tokens, i) => {
    const meta = tokens[i]?.meta;
    const target = meta?.target;
    const label = meta?.label;
    if (typeof target !== "string" || typeof label !== "string") return "";
    const embed = meta?.embed === true;
    const e = find(target);
    if (!e) return hidden(label);
    return embed && IMAGE.test(e.path) ? image(e, label) : internal(e, label);
  };
  md.renderer.rules.image = (tokens, i) => {
    const token = tokens[i];
    const src = String(token?.attrGet("src") ?? "");
    const alt = token?.content ?? "";
    // A remote image is never loaded: opening a note calls no third party (T7).
    if (/^[a-z][a-z0-9+.-]*:/i.test(src))
      return `<span class="remote">${esc(alt)}</span>`;
    const e = find(src);
    return e && IMAGE.test(e.path) ? image(e, alt) : hidden(alt);
  };
  const defaultOpen =
    md.renderer.rules.link_open ?? ((t, i, o, _e, s) => s.renderToken(t, i, o));
  md.renderer.rules.link_open = (tokens, i, options, env, self) => {
    const token = tokens[i];
    const href = String(token?.attrGet("href") ?? "");
    if (token && !/^(https?:|mailto:)/i.test(href)) {
      const e = find(href);
      token.attrSet("href", e ? `${ENTRY_LINK}${e.entry}` : "#");
    }
    return defaultOpen(tokens, i, options, env, self);
  };

  const body = DOMPurify.sanitize(md.render(markdown), {
    FORBID_TAGS: [
      "style",
      "form",
      "input",
      "button",
      "iframe",
      "object",
      "embed",
    ],
    FORBID_ATTR: ["style", "srcset"],
    ALLOWED_URI_REGEXP: /^(?:https?:|mailto:|#)/i,
  });
  return { body, images: [...images.values()] };
}

/** `data:` URLs for the embedded images, then the frame's whole document. */
export async function frameDocument(
  rendered: Rendered,
  load: (e: EntryState) => Promise<Uint8Array | null>,
): Promise<string> {
  const doc = new DOMParser().parseFromString(
    `<body>${rendered.body}</body>`,
    "text/html",
  );
  for (const img of Array.from(
    doc.querySelectorAll<HTMLImageElement>("img[data-entry]"),
  )) {
    const e = rendered.images.find((x) => x.entry === img.dataset.entry);
    const bytes = e && e.size <= MAX_IMAGE ? await load(e) : null;
    if (e && bytes) {
      img.src = await dataUrl(bytes, mimeOf(e.path));
    } else if (e) {
      // Too large or unreadable: a link that opens the file (its download).
      const a = doc.createElement("a");
      a.href = `${ENTRY_LINK}${e.entry}`;
      a.textContent = img.alt || e.path;
      img.replaceWith(a);
    } else {
      img.replaceWith(doc.createTextNode(img.alt));
    }
    img.removeAttribute("data-entry");
  }
  await diagrams(doc);
  return (
    // The theme it opens in; NoteFrame sends it the next one.
    `<!doctype html><html data-theme="${document.documentElement.dataset.theme === "light" ? "light" : "dark"}"><head><meta charset="utf-8">` +
    '<link rel="stylesheet" href="/app/frame.css"></head>' +
    `<body>${doc.body.innerHTML}<script>${BRIDGE}</script></body></html>`
  );
}

let diagramId = 0;

/**
 * Mermaid blocks drawn as SVG and shown as an `<img>`: an image runs no
 * script and calls no host, so the frame's policy stays as it is. Mermaid
 * parses in the application (decision of 2026-11-18), loaded only when a
 * note has a diagram, in its `strict` mode (its own DOMPurify, no click
 * handlers). A diagram it cannot draw stays a code block.
 */
async function diagrams(doc: Document): Promise<void> {
  const blocks = Array.from(
    doc.querySelectorAll<HTMLElement>("pre > code.language-mermaid"),
  ).filter((c) => (c.textContent ?? "").length <= MAX_DIAGRAM);
  if (blocks.length === 0) return;
  const { default: mermaid } = await import("mermaid");
  // shortcut: drawn in the theme the note opens in, a theme switch keeps it
  // until the note is opened again; redraw on the switch if that shows.
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    htmlLabels: false,
    theme:
      document.documentElement.dataset.theme === "light" ? "default" : "dark",
  });
  for (const code of blocks) {
    const id = `kura-diagram-${++diagramId}`;
    try {
      const { svg } = await mermaid.render(id, code.textContent ?? "");
      const img = doc.createElement("img");
      img.alt = "diagram";
      img.className = "diagram";
      // The SVG says width="100%": its natural width is the viewBox's.
      const width = Number(/viewBox="[\d.-]+ [\d.-]+ ([\d.]+)/.exec(svg)?.[1]);
      if (width > 0) img.width = Math.ceil(width);
      img.src = await dataUrl(new TextEncoder().encode(svg), "image/svg+xml");
      code.parentElement?.replaceWith(img);
    } catch {
      // Mermaid leaves its error drawing in the page on a failure.
      document.getElementById(`d${id}`)?.remove();
    }
  }
}

function mimeOf(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase();
  return ext === "svg"
    ? "image/svg+xml"
    : ext === "jpg"
      ? "image/jpeg"
      : `image/${ext ?? "png"}`;
}

async function dataUrl(bytes: Uint8Array, type: string): Promise<string> {
  let binary = "";
  for (const b of bytes) binary += String.fromCharCode(b);
  return `data:${type};base64,${btoa(binary)}`;
}
