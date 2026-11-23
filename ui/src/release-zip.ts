/**
 * An Obsidian plugin's release zip, read in the browser (RFC 0010 §6.4,
 * bounds of §7.3): only `manifest.json`, `main.js` and `styles.css`, at the
 * root or under one top folder, are taken; everything else is ignored, and
 * no name in the zip is ever used as a path.
 */
import { strFromU8, type UnzipFileInfo, unzipSync } from "fflate";

export const MAX_ENTRIES = 64;
export const MAX_TOTAL = 64 * 1024 * 1024;
export const MAX_RATIO = 100;
const FILES = ["manifest.json", "main.js", "styles.css"] as const;

export interface ReleasePlugin {
  id: string;
  name: string;
  version?: string;
  manifest: Uint8Array;
  main: Uint8Array;
  styles?: Uint8Array;
}

export type ZipProblem =
  | "unreadable"
  | "tooMany"
  | "tooLarge"
  | "ratio"
  | "noPlugin"
  | "kura"
  | "noMain";

class Refused extends Error {
  constructor(readonly problem: ZipProblem) {
    super(problem);
  }
}

/** The plugin in a release zip, or why there is none. */
export function readRelease(
  zip: Uint8Array,
): ReleasePlugin | { problem: ZipProblem } {
  try {
    return read(zip);
  } catch (e) {
    return { problem: e instanceof Refused ? e.problem : "unreadable" };
  }
}

function read(zip: Uint8Array): ReleasePlugin {
  // First pass: the directory only, checked against the bounds; nothing inflated.
  const names: string[] = [];
  let total = 0;
  unzipSync(zip, {
    filter: (f: UnzipFileInfo) => {
      if (names.length >= MAX_ENTRIES) throw new Refused("tooMany");
      total += f.originalSize;
      if (total > MAX_TOTAL) throw new Refused("tooLarge");
      if (f.originalSize > MAX_RATIO * Math.max(f.size, 1))
        throw new Refused("ratio");
      names.push(f.name);
      return false;
    },
  });
  const tops = names.flatMap((n) => {
    const parts = n.split("/");
    return parts.length === 2 && parts[1] === "manifest.json" ? [parts[0]] : [];
  });
  const prefix = names.includes("manifest.json")
    ? ""
    : tops.length === 1
      ? `${tops[0]}/`
      : null;
  if (prefix === null) throw new Refused("noPlugin");
  // Second pass: the three files, each inflated into its declared size at most.
  const wanted = new Set(FILES.map((f) => prefix + f));
  const files = unzipSync(zip, { filter: (f) => wanted.has(f.name) });
  const manifest = files[`${prefix}manifest.json`];
  const main = files[`${prefix}main.js`];
  const styles = files[`${prefix}styles.css`];
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder().decode(manifest));
  } catch {
    throw new Refused("noPlugin");
  }
  const field = (k: string): string | undefined => {
    const v =
      typeof value === "object" && value !== null
        ? Reflect.get(value, k)
        : undefined;
    return typeof v === "string" ? v.slice(0, 100) : undefined;
  };
  const id = field("id");
  if (!manifest || !id || !/^[a-z0-9-]{1,64}$/.test(id))
    throw new Refused("noPlugin");
  if (id === "weebo-kura") throw new Refused("kura");
  if (!main) throw new Refused("noMain");
  return {
    id,
    name: field("name")?.trim() || id,
    version: field("version"),
    manifest,
    main,
    ...(styles ? { styles } : {}),
  };
}

/** Bytes as base64, for the install route's JSON body. */
export function base64(bytes: Uint8Array): string {
  return btoa(strFromU8(bytes, true));
}
