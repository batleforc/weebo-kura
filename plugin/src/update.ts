// Client versions and the one-click update (RFC 0010 §4.2, §5.4): the
// server's version against this build's, and a new build fetched from the
// configured server, checked in memory (SHA-256, manifest id and version,
// no downgrade) before the first write, kept beside the running one for a
// way back. Only the three fixed names of this plugin's own folder are ever
// written: nothing from the server becomes a path.
import { isRecord } from "./types";

export const FILES = ["main.js", "manifest.json", "styles.css"] as const;
export type FileName = (typeof FILES)[number];
const ID = "weebo-kura";
const RELEASES = "https://github.com/batleforc/weebo-kura/releases";

/** `major.minor.patch` of `0.3.1`, `kura-server/0.3.1` or `0.3.1-rc.1+x`;
 * a pre-release compares as its release. Null when it is none. */
export function parseVersion(v: string): [number, number, number] | null {
  const m = /^(?:[\w.-]+\/)?(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/.exec(v.trim());
  return m === null ? null : [Number(m[1]), Number(m[2]), Number(m[3])];
}

/** Negative, zero or positive, as `a` is older, equal or newer; null when unknown. */
export function compareVersions(a: string, b: string): number | null {
  const x = parseVersion(a);
  const y = parseVersion(b);
  if (x === null || y === null) return null;
  for (let i = 0; i < 3; i++) {
    const d = (x[i] ?? 0) - (y[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** This client against the server's `x-kura-server` (§4.2 table). */
export type Drift = "equal" | "older" | "newer" | "unknown";

export function drift(client: string, server: string | null): Drift {
  const d = server === null ? null : compareVersions(client, server);
  return d === null ? "unknown" : d === 0 ? "equal" : d < 0 ? "older" : "newer";
}

/** The release page of a version, offered when the server has no console. */
export function releasePage(version: string): string {
  return `${RELEASES}/tag/v${version}`;
}

export type Get = (url: string) => Promise<{ status: number; body: ArrayBuffer }>;

export interface Candidate {
  version: string;
  /** The server's host, for the confirmation. */
  source: string;
  files: { name: FileName; data: Uint8Array; sha256: string }[];
  /** The release's `checksums.txt` lists the same three hashes (decision 13). */
  verified: boolean;
}

export async function sha256(data: Uint8Array): Promise<string> {
  const copy = new Uint8Array(data.byteLength);
  copy.set(data);
  const d = await crypto.subtle.digest("SHA-256", copy.buffer);
  return Array.from(new Uint8Array(d), (b) => b.toString(16).padStart(2, "0")).join("");
}

const record = (v: unknown): Record<string, unknown> | null => (isRecord(v) ? v : null);

function parse(body: ArrayBuffer): unknown {
  try {
    return JSON.parse(new TextDecoder().decode(body));
  } catch {
    return null;
  }
}

const HEX = /^[0-9a-f]{64}$/;

/** The newer build the server offers, fetched and checked in memory; null
 * when the server serves none (its console is off). Throws with the cause. */
export async function fetchUpdate(server: string, current: string, get: Get): Promise<Candidate | null> {
  const base = server.replace(/\/+$/, "");
  const info = await get(`${base}/app/plugin.json`);
  if (info.status === 404) return null;
  if (info.status !== 200) throw new Error(`plugin.json answered ${String(info.status)}`);
  const p = record(parse(info.body));
  if (p === null) throw new Error("plugin.json is not JSON");
  if (p.available !== true) return null;
  const version = typeof p.version === "string" ? p.version : "";
  if (p.id !== ID || parseVersion(version) === null) throw new Error("plugin.json names no Kura Sync version");
  const order = compareVersions(version, current);
  if (order === null || order <= 0) throw new Error(`${version} is not newer than ${current}`);
  const sums = record(p.sha256);
  const files: Candidate["files"] = [];
  for (const name of FILES) {
    const want = sums?.[name];
    if (typeof want !== "string" || !HEX.test(want)) throw new Error(`plugin.json gives no sha256 for ${name}`);
    const r = await get(`${base}/app/plugin/${name}`);
    if (r.status !== 200) throw new Error(`${name} answered ${String(r.status)}`);
    const data = new Uint8Array(r.body);
    if ((await sha256(data)) !== want) throw new Error(`${name} does not match its sha256`);
    files.push({ name, data, sha256: want });
  }
  const m = record(parse(files.find((f) => f.name === "manifest.json")?.data.slice().buffer ?? new ArrayBuffer(0)));
  if (m?.id !== ID) throw new Error(`manifest.json is not Kura Sync's`);
  if (m.version !== version) throw new Error(`manifest.json says ${String(m.version)}, not ${version}`);
  return { version, source: new URL(base).host, files, verified: await provenance(version, files, get) };
}

/** Decision 13: the release's `checksums.txt` names the same hashes. Any
 * failure to reach or read it is "not verified", never an error. */
export async function provenance(version: string, files: Candidate["files"], get: Get): Promise<boolean> {
  try {
    const r = await get(`${RELEASES}/download/v${version}/checksums.txt`);
    if (r.status !== 200) return false;
    const listed = new Map<string, string>();
    for (const line of new TextDecoder().decode(r.body).split("\n")) {
      const m = /^([0-9a-f]{64}) [ *]?(\S+)$/.exec(line.trim());
      if (m?.[1] !== undefined && m[2] !== undefined) listed.set(m[2].split("/").pop() ?? m[2], m[1]);
    }
    return files.every((f) => listed.get(f.name) === f.sha256);
  } catch {
    return false;
  }
}

/** What the update writes with: the vault adapter. */
export interface UpdateFs {
  exists(path: string): Promise<boolean>;
  readBinary(path: string): Promise<ArrayBuffer>;
  writeBinary(path: string, data: ArrayBuffer): Promise<void>;
  mkdir(path: string): Promise<void>;
}

const buffer = (d: Uint8Array): ArrayBuffer => d.slice().buffer;

async function writeAll(fs: UpdateFs, dir: string, files: { name: FileName; data: Uint8Array }[]): Promise<void> {
  for (const f of files) await fs.writeBinary(`${dir}/${f.name}`, buffer(f.data));
}

async function readAll(fs: UpdateFs, dir: string): Promise<{ name: FileName; data: Uint8Array }[]> {
  const out: { name: FileName; data: Uint8Array }[] = [];
  for (const name of FILES) {
    if (await fs.exists(`${dir}/${name}`)) out.push({ name, data: new Uint8Array(await fs.readBinary(`${dir}/${name}`)) });
  }
  return out;
}

/** The checked build in place of the running one, kept in `.kura-previous/`;
 * a failed write puts the running files back. */
export async function install(c: Candidate, fs: UpdateFs, dir: string): Promise<void> {
  const running = await readAll(fs, dir);
  const keep = `${dir}/.kura-previous`;
  if (!(await fs.exists(keep))) await fs.mkdir(keep);
  await writeAll(fs, keep, running);
  try {
    await writeAll(fs, dir, c.files);
  } catch (e) {
    await writeAll(fs, dir, running);
    throw e;
  }
}

/** The version kept in `.kura-previous/`, if it holds Kura Sync. */
export async function previousVersion(fs: UpdateFs, dir: string): Promise<string | null> {
  const m = `${dir}/.kura-previous/manifest.json`;
  if (!(await fs.exists(m))) return null;
  const v = record(parse(await fs.readBinary(m)));
  return v?.id === ID && typeof v.version === "string" ? v.version : null;
}

/** "Go back": the exact files that ran before, restored. */
export async function goBack(fs: UpdateFs, dir: string): Promise<string> {
  const version = await previousVersion(fs, dir);
  if (version === null) throw new Error("no previous version is kept");
  const files = await readAll(fs, `${dir}/.kura-previous`);
  if (!files.some((f) => f.name === "main.js")) throw new Error("the previous version has no main.js");
  await writeAll(fs, dir, files);
  return version;
}
