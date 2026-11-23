/**
 * The synced Obsidian configuration as the console lists it (RFC 0008):
 * grouped by category, plugins one by one. The server decides what a
 * browser may read; a file it withholds comes without its `blob`.
 */

import type { EntryState } from "@/api/generated/types.gen";

export type Category = "snippets" | "themes" | "plugins" | "hotkeys";
export const CATEGORIES: readonly Category[] = [
  "snippets",
  "themes",
  "plugins",
  "hotkeys",
];

/** The largest file shown; a larger one is listed only. */
export const MAX_SHOWN = 256 * 1024;
/** The largest plugin manifest read for its name. */
export const MAX_MANIFEST = 16 * 1024;
/** The plugins whose manifest is read; the rest are listed by id. */
export const MAX_PLUGINS = 200;

/** `kura_core::config::category`, mirrored for grouping only. */
export function categoryOf(path: string): Category | null {
  const p = path.toLowerCase();
  const [top = "", ...rest] = p.split("/");
  if (top === "plugins") {
    const [id = "", ...file] = rest;
    return id && id !== "weebo-kura" && file.join("/") ? "plugins" : null;
  }
  if ((top === "snippets" || top === "themes") && rest.join("/")) return top;
  return p === "hotkeys.json" ? "hotkeys" : null;
}

/** `kura_core::config::item_of`, mirrored for grouping: `plugin:<id>` (lower
 * case) for a plugin's folder, else the category; `null` when it does not sync. */
export function itemOf(path: string): string | null {
  const c = categoryOf(path);
  return c === "plugins" ? `plugin:${path.split("/")[1]?.toLowerCase()}` : c;
}

/** Withheld by the server: listed, never read in a browser. */
export function isHidden(e: EntryState): boolean {
  return !e.deleted && !e.blob;
}

/** Why a hidden file is not shown, as a message key. */
export function hiddenReason(path: string): "code" | "settings" | "other" {
  const file = path.toLowerCase().split("/").pop() ?? "";
  return file === "main.js"
    ? "code"
    : file === "data.json"
      ? "settings"
      : "other";
}

export interface Plugin {
  id: string;
  files: EntryState[];
  manifest?: EntryState;
}

export interface Groups {
  snippets: EntryState[];
  themes: EntryState[];
  plugins: Plugin[];
  hotkeys: EntryState[];
}

/** The live files by category, each sorted by path; plugins by id. */
export function group(entries: readonly EntryState[]): Groups {
  const out: Groups = { snippets: [], themes: [], plugins: [], hotkeys: [] };
  const plugins = new Map<string, Plugin>();
  const sorted = entries
    .filter((e) => !e.deleted && !e.folder)
    .sort((a, b) => a.path.localeCompare(b.path));
  for (const e of sorted) {
    const c = categoryOf(e.path);
    if (c === "plugins") {
      const [, id = "", ...rest] = e.path.split("/");
      let p = plugins.get(id.toLowerCase());
      if (!p) {
        p = { id, files: [] };
        plugins.set(id.toLowerCase(), p);
      }
      p.files.push(e);
      if (rest.join("/").toLowerCase() === "manifest.json") p.manifest = e;
    } else if (c) {
      out[c].push(e);
    }
  }
  out.plugins = [...plugins.values()].sort((a, b) => a.id.localeCompare(b.id));
  return out;
}

/** A path shown inside its group: without `plugins/<id>/` or `themes/`. */
export function shortName(path: string): string {
  const parts = path.split("/");
  const top = parts[0]?.toLowerCase();
  if (top === "plugins") return parts.slice(2).join("/");
  if (top === "snippets" || top === "themes") return parts.slice(1).join("/");
  return path;
}

export interface Manifest {
  name: string;
  version?: string;
}

/** A plugin manifest's name and version, as text; `null` when unusable. */
export function readManifest(bytes: Uint8Array): Manifest | null {
  if (bytes.length > MAX_MANIFEST) return null;
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder().decode(bytes));
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null) return null;
  const name = Reflect.get(value, "name");
  const version = Reflect.get(value, "version");
  if (typeof name !== "string" || !name.trim()) return null;
  return {
    name: name.slice(0, 100),
    version: typeof version === "string" ? version.slice(0, 100) : undefined,
  };
}

/** Why a plugin is listed by its id alone, as a message key; `null` when its
 * manifest can be read. */
export function manifestProblem(p: Plugin): "missing" | "tooLarge" | null {
  if (!p.manifest || isHidden(p.manifest)) return "missing";
  return p.manifest.size > MAX_MANIFEST ? "tooLarge" : null;
}

/** `1.2 kB`, `2.3 MB`; `ko`, `Mo` in French. */
export function formatSize(bytes: number, locale: string): string {
  const [unit, value] =
    bytes >= 1_000_000
      ? (["megabyte", bytes / 1_000_000] as const)
      : bytes >= 1_000
        ? (["kilobyte", bytes / 1_000] as const)
        : (["byte", bytes] as const);
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit,
    unitDisplay: "short",
    maximumFractionDigits: unit === "byte" ? 0 : 1,
  }).format(value);
}

/** Runs `task` over `items`, `limit` at a time; each failure is its own. */
export async function eachLimited<T>(
  items: readonly T[],
  limit: number,
  task: (item: T) => Promise<void>,
): Promise<void> {
  let next = 0;
  const worker = async () => {
    while (next < items.length) {
      const item = items[next];
      next += 1;
      if (item !== undefined) await task(item).catch(() => undefined);
    }
  };
  await Promise.all(
    Array.from({ length: Math.min(limit, items.length) }, worker),
  );
}
