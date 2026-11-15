// The kura API over `fetch` (§6.8 plugin transport), for the core's `Remote`
// and for the plugin's own calls (vaults, devices, notification tickets).
import type { KuraRemote } from "./wasm/kura";
import { ProblemError, isProblem, isRecord } from "./types";
import type { Sleep } from "./auth";

export const CLIENT = "kura-plugin/0.1.0";
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
}

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

export class Api {
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
      const configSync = Array.isArray(x.configSync) ? x.configSync.filter((c): c is string => typeof c === "string") : [];
      return [{ id: x.id, name: x.name, role: x.role, chunking: { min, avg, max }, configSync }];
    });
  }

  async register(name: string, profile: "desktop" | "mobile"): Promise<string> {
    const v = await this.json("/devices", { method: "POST", json: { name, profile } });
    if (isRecord(v) && typeof v.id === "string") return v.id;
    throw new Error("the server registered no device");
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

  /** The core's view of one vault: its notes, or with `config` the user's
   * own Obsidian configuration for this device's profile (§6.8). */
  remote(vault: string, space?: "config"): KuraRemote {
    const v = `/vaults/${vault}`;
    const inSpace = space === undefined ? "" : `&space=${space}`;
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
      commit: (request) =>
        this.json(`${v}/commits`, { method: "POST", json: isRecord(request) && space !== undefined ? { ...request, space } : request }),
      changes: (since, epoch, start) =>
        this.json(`${v}/changes?since=${String(since)}&start=${String(start)}${epoch === null ? "" : `&epoch=${epoch}`}${inSpace}`, { method: "GET" }),
    };
  }
}

export { isProblem };
