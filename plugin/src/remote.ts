// The kura API over `fetch` (§6.8 plugin transport), for the core's `Remote`
// and for the plugin's own calls (vaults, devices, notification tickets).
import type { KuraRemote } from "./wasm/kura";
import { ProblemError, isProblem, isRecord } from "./types";
import type { Sleep } from "./auth";
import manifest from "../manifest.json";

/** This build's version, from the manifest shipped beside it (RFC 0010 phase 0). */
export const VERSION = manifest.version;
export const CLIENT = `kura-plugin/${VERSION}`;
/** Attempts when the server asks to wait (§6.3). */
const ATTEMPTS = 5;

export type Fetch = (url: string, init: RequestInit) => Promise<Response>;

export interface ApiOptions {
  server: string;
  token: () => Promise<string>;
  device: () => string | null;
  fetch: Fetch;
  sleep: Sleep;
}

export interface VaultView {
  id: string;
  name: string;
  role: string;
  chunking: { min: number; avg: number; max: number };
  /** The `.obsidian` categories that sync (§6.8); empty: none. */
  configSync: string[];
  /** The caller's items shared by both profiles (RFC 0010 §4.1). */
  configShared: string[];
  /** The vault takes plugins installed by its managers. */
  vaultPlugins: boolean;
}

/** A configuration space (RFC 0010 §4.2). */
export type Space = "config" | "config-shared" | "config-vault";

/** The member's shared items (RFC 0010 §4.4). */
export interface Shares {
  defaults: string[];
  choices: Record<string, boolean>;
  shared: string[];
  /** Ids of the plugins installed on the vault. */
  vaultPlugins: string[];
}

/** Where `installPlugin` puts the code. */
export type Target = "vault" | "shared" | "desktop" | "mobile";

const texts = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);

function readShares(v: unknown): Shares {
  if (!isRecord(v)) throw new Error("unexpected shares");
  const choices: Record<string, boolean> = {};
  if (isRecord(v.choices)) for (const [k, x] of Object.entries(v.choices)) if (typeof x === "boolean") choices[k] = x;
  return { defaults: texts(v.defaults), choices, shared: texts(v.shared), vaultPlugins: texts(v.vaultPlugins) };
}

/** Base64 without Node's Buffer, which mobile lacks. */
export function base64(data: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < data.length; i += 0x8000) bin += String.fromCharCode(...data.subarray(i, i + 0x8000));
  return btoa(bin);
}

/** A refusal made here, never sent: a member's vault pass reads only. */
const readOnly = (): Promise<never> =>
  Promise.reject(new ProblemError(403, "insufficient_role", "installing a plugin on the vault needs the manager role"));

/** A refusal as the core expects it: `{status, kind, detail}`. */
async function problem(r: Response): Promise<ProblemError> {
  let json: unknown = null;
  try {
    json = await r.json();
  } catch {
    // Not a problem document.
  }
  const type = isRecord(json) && typeof json.type === "string" ? json.type : "unknown";
  const detail = isRecord(json) && typeof json.detail === "string" ? json.detail : null;
  return new ProblemError(r.status, type.replace(/^\/v1\/problems\//, ""), detail);
}

function retryAfter(r: Response): number {
  const s = Number(r.headers.get("retry-after") ?? "1");
  return Math.min(60, Math.max(1, Number.isFinite(s) ? s : 1));
}

/** `call`, with a device this server does not know (another server's, or
 * one purged) registered again once by `register`, as the console's
 * `withDevice`. A revoked device is not: it must sign in again. */
export async function withDevice<T>(call: () => Promise<T>, register: () => Promise<void>): Promise<T> {
  try {
    return await call();
  } catch (e) {
    if (!isProblem(e) || e.kind !== "device_unknown") throw e;
    await register();
    return call();
  }
}

export class Api {
  /** The server's `x-kura-server` header, as last answered (RFC 0010 §4.2). */
  serverVersion: string | null = null;

  constructor(private readonly o: ApiOptions) {}

  /** Sends, waiting out 429 and 503 per `Retry-After`; a refusal rejects with its problem. */
  async send(path: string, init: { method: string; body?: BodyInit; json?: unknown }): Promise<Response> {
    for (let attempt = 1; ; attempt++) {
      const headers: Record<string, string> = {
        authorization: `Bearer ${await this.o.token()}`,
        "x-kura-client": CLIENT,
      };
      const device = this.o.device();
      if (device !== null) headers["x-kura-device"] = device;
      let body = init.body;
      if (init.json !== undefined) {
        headers["content-type"] = "application/json";
        body = JSON.stringify(init.json);
      } else if (body !== undefined) {
        headers["content-type"] = "application/octet-stream";
      }
      let r: Response;
      try {
        r = await this.o.fetch(`${this.o.server.replace(/\/+$/, "")}/v1${path}`, { method: init.method, headers, ...(body === undefined ? {} : { body }) });
      } catch (e) {
        throw new Error(`cannot reach the server: ${String(e)}`, { cause: e });
      }
      const seen = r.headers.get("x-kura-server");
      if (seen !== null) this.serverVersion = seen;
      if (r.ok) return r;
      if ((r.status === 429 || r.status === 503) && attempt < ATTEMPTS) {
        await this.o.sleep(retryAfter(r) * 1000);
        continue;
      }
      if (r.status >= 500 && r.status !== 503) throw new Error(`server error ${String(r.status)}`);
      // Rejected with the problem object the core reads (kura-wasm `remote_err`).
      throw await problem(r);
    }
  }

  async json(path: string, init: { method: string; json?: unknown }): Promise<unknown> {
    return (await this.send(path, init)).json();
  }

  async vaults(): Promise<VaultView[]> {
    const v = await this.json("/vaults", { method: "GET" });
    if (!Array.isArray(v)) return [];
    return v.flatMap((x: unknown): VaultView[] => {
      if (!isRecord(x) || typeof x.id !== "string" || typeof x.name !== "string" || typeof x.role !== "string" || !isRecord(x.chunking)) return [];
      const { min, avg, max } = x.chunking;
      if (typeof min !== "number" || typeof avg !== "number" || typeof max !== "number") return [];
      return [{ id: x.id, name: x.name, role: x.role, chunking: { min, avg, max }, configSync: texts(x.configSync), configShared: texts(x.configShared), vaultPlugins: x.vaultPlugins === true }];
    });
  }

  async register(name: string, profile: "desktop" | "mobile"): Promise<string> {
    const v = await this.json("/devices", { method: "POST", json: { name, profile } });
    if (isRecord(v) && typeof v.id === "string") return v.id;
    throw new Error("the server registered no device");
  }

  /** What this device's last run left undone, as counts only (RFC 0006 phase 3). */
  async report(vault: string, counts: Record<string, unknown>): Promise<void> {
    await this.send(`/vaults/${vault}/sync/report`, { method: "PUT", json: counts });
  }

  async ticket(vault: string): Promise<string> {
    const v = await this.json(`/vaults/${vault}/events/ticket`, { method: "POST" });
    if (isRecord(v) && typeof v.ticket === "string") return v.ticket;
    throw new Error("the server gave no ticket");
  }

  /** The notifications socket's URL for a ticket. */
  eventsUrl(vault: string, ticket: string): string {
    const base = this.o.server.replace(/\/+$/, "").replace(/^http/, "ws");
    return `${base}/v1/vaults/${vault}/events?ticket=${encodeURIComponent(ticket)}`;
  }

  async shares(vault: string): Promise<Shares> {
    return readShares(await this.json(`/vaults/${vault}/config/shares`, { method: "GET" }));
  }

  /** The member's own choice for one item (`plugin:<id>`, `snippets`, …). */
  async setShare(vault: string, item: string, shared: boolean): Promise<Shares> {
    return readShares(await this.json(`/vaults/${vault}/config/shares/${encodeURIComponent(item)}`, { method: "PUT", json: { shared } }));
  }

  /** Back to the vault's default for one item. */
  async clearShare(vault: string, item: string): Promise<Shares> {
    return readShares(await this.json(`/vaults/${vault}/config/shares/${encodeURIComponent(item)}`, { method: "DELETE" }));
  }

  /** A plugin's three code files, committed by the server to `target`'s space. */
  async installPlugin(vault: string, id: string, target: Target, files: { manifest: Uint8Array; main: Uint8Array; styles?: Uint8Array }): Promise<unknown> {
    const json = { manifest: base64(files.manifest), main: base64(files.main), ...(files.styles === undefined ? {} : { styles: base64(files.styles) }) };
    return this.json(`/vaults/${vault}/config/plugins/${encodeURIComponent(id)}?target=${target}`, { method: "PUT", json });
  }

  /** The core's view of one vault: its notes, or with a space the user's
   * Obsidian configuration (§6.8, RFC 0010 §4.2). `readOnly` refuses every
   * upload here, before the server; `skip` hides remote paths from the core. */
  remote(vault: string, space?: Space, opts: { readOnly?: boolean; skip?: (path: string) => boolean } = {}): KuraRemote {
    const v = `/vaults/${vault}`;
    const inSpace = space === undefined ? "" : `&space=${space}`;
    const skip = opts.skip;
    if (opts.readOnly === true) {
      return { ...this.remote(vault, space, skip === undefined ? {} : { skip }), missing: readOnly, putChunk: readOnly, putBlob: readOnly, putChunks: readOnly, putBlobs: readOnly, commit: readOnly };
    }
    return {
      missing: async (hashes) => {
        const r = await this.json(`${v}/chunks/missing`, { method: "POST", json: { hashes } });
        return isRecord(r) && Array.isArray(r.missing) ? r.missing.filter((h): h is string => typeof h === "string") : [];
      },
      putChunk: async (hash, data) => {
        await this.send(`${v}/chunks/${hash}`, { method: "PUT", body: new Uint8Array(data) });
      },
      getChunk: async (hash) => new Uint8Array(await (await this.send(`${v}/chunks/${hash}`, { method: "GET" })).arrayBuffer()),
      putBlob: async (hash, size, chunks) => {
        await this.send(`${v}/blobs/${hash}`, { method: "PUT", json: { size, chunks } });
      },
      getBlob: async (hash) => {
        const m = await this.json(`${v}/blobs/${hash}`, { method: "GET" });
        if (isRecord(m) && typeof m.size === "number" && Array.isArray(m.chunks)) {
          return { size: m.size, chunks: m.chunks.filter((c): c is string => typeof c === "string") };
        }
        throw new Error("unexpected manifest");
      },
      // RFC 0002: an older server answers 405 to the batch routes; null
      // makes the core read one by one for the rest of the run.
      getBlobs: async (hashes) => {
        let m: unknown;
        try {
          m = await this.json(`${v}/blobs/batch`, { method: "POST", json: { blobs: hashes } });
        } catch (e) {
          if (isProblem(e) && e.status === 405) return null;
          throw e;
        }
        if (!isRecord(m) || !Array.isArray(m.manifests)) throw new Error("unexpected manifests");
        return m.manifests.flatMap((x: unknown) =>
          isRecord(x) && typeof x.hash === "string" && typeof x.size === "number" && Array.isArray(x.chunks)
            ? [{ hash: x.hash, size: x.size, chunks: x.chunks.filter((c): c is string => typeof c === "string") }]
            : [],
        );
      },
      getChunkRecords: async (hashes) => {
        try {
          const r = await this.send(`${v}/chunks/batch`, { method: "POST", json: { chunks: hashes } });
          return new Uint8Array(await r.arrayBuffer());
        } catch (e) {
          if (isProblem(e) && e.status === 405) return null;
          throw e;
        }
      },
      // RFC 0002 phase 3: false (a 405) makes the core send one by one.
      putChunks: async (records) => {
        try {
          await this.send(`${v}/chunks/upload`, { method: "POST", body: new Uint8Array(records) });
          return true;
        } catch (e) {
          if (isProblem(e) && e.status === 405) return false;
          throw e;
        }
      },
      putBlobs: async (manifests) => {
        try {
          await this.send(`${v}/blobs/upload`, { method: "POST", json: { manifests } });
          return true;
        } catch (e) {
          if (isProblem(e) && e.status === 405) return false;
          throw e;
        }
      },
      commit: (request) =>
        this.json(`${v}/commits`, { method: "POST", json: isRecord(request) && space !== undefined ? { ...request, space } : request }),
      // The notes' folders too (RFC 0007); the configuration space has none.
      // The core's `None` arrives as `undefined` from wasm-bindgen, not `null`.
      changes: async (since, epoch, start) => {
        const r = await this.json(`${v}/changes?since=${String(since)}&start=${String(start)}${epoch == null ? "" : `&epoch=${epoch}`}${space === undefined ? "&folders=true" : inSpace}`, { method: "GET" });
        if (skip === undefined || !isRecord(r) || !Array.isArray(r.items)) return r;
        return { ...r, items: r.items.filter((x: unknown) => !(isRecord(x) && typeof x.path === "string" && skip(x.path))) };
      },
    };
  }
}

export { isProblem };
