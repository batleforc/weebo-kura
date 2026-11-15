// Fakes shared by the tests: an Obsidian adapter in memory, manual timers,
// and a kura server in memory behind `fetch`.
import type { Adapter } from "../src/host";

export class MemAdapter implements Adapter {
  files = new Map<string, { data: Uint8Array; mtime: number }>();
  folders = new Set<string>([""]);
  trashed: string[] = [];
  private tick = 1;

  put(path: string, text: string): void {
    this.files.set(path, { data: new TextEncoder().encode(text), mtime: ++this.tick });
    let dir = path;
    while (dir.includes("/")) {
      dir = dir.slice(0, dir.lastIndexOf("/"));
      this.folders.add(dir);
    }
  }
  text(path: string): string | undefined {
    const f = this.files.get(path);
    return f === undefined ? undefined : new TextDecoder().decode(f.data);
  }
  list(path: string): Promise<{ files: string[]; folders: string[] }> {
    const dir = path === "/" ? "" : path;
    const inDir = (p: string): boolean => (dir === "" ? !p.includes("/") : p.startsWith(`${dir}/`) && !p.slice(dir.length + 1).includes("/"));
    return Promise.resolve({ files: [...this.files.keys()].filter(inDir), folders: [...this.folders].filter((f) => f !== "" && inDir(f)) });
  }
  stat(path: string): Promise<{ type: "file" | "folder"; size: number; mtime: number } | null> {
    const f = this.files.get(path);
    if (f !== undefined) return Promise.resolve({ type: "file", size: f.data.byteLength, mtime: f.mtime });
    return Promise.resolve(this.folders.has(path) ? { type: "folder", size: 0, mtime: 0 } : null);
  }
  readBinary(path: string): Promise<ArrayBuffer> {
    const f = this.files.get(path);
    if (f === undefined) return Promise.reject(new Error(`no ${path}`));
    return Promise.resolve(f.data.slice().buffer);
  }
  writeBinary(path: string, data: ArrayBuffer): Promise<void> {
    this.files.set(path, { data: new Uint8Array(data), mtime: ++this.tick });
    return Promise.resolve();
  }
  write(path: string, data: string): Promise<void> {
    this.put(path, data);
    return Promise.resolve();
  }
  exists(path: string): Promise<boolean> {
    return Promise.resolve(this.files.has(path) || this.folders.has(path));
  }
  mkdir(path: string): Promise<void> {
    let dir = path;
    for (;;) {
      this.folders.add(dir);
      if (!dir.includes("/")) break;
      dir = dir.slice(0, dir.lastIndexOf("/"));
    }
    return Promise.resolve();
  }
  rename(from: string, to: string): Promise<void> {
    const f = this.files.get(from);
    if (f === undefined) return Promise.reject(new Error(`no ${from}`));
    this.files.delete(from);
    this.files.set(to, f);
    return Promise.resolve();
  }
  remove(path: string): Promise<void> {
    this.files.delete(path);
    return Promise.resolve();
  }
  trashLocal(path: string): Promise<void> {
    this.trashed.push(path);
    return this.rename(path, `.trash/${path}`);
  }
}

/** Timers fired by hand. */
export class ManualTimers {
  private next = 1;
  pending = new Map<number, { fn: () => void; ms: number }>();
  set = (fn: () => void, ms: number): number => {
    const id = this.next++;
    this.pending.set(id, { fn, ms });
    return id;
  };
  clear = (id: number): void => {
    this.pending.delete(id);
  };
  /** Fires every pending timer once, in order. */
  fireAll(): void {
    const due = [...this.pending.entries()];
    this.pending.clear();
    for (const [, t] of due) t.fn();
  }
  delays(): number[] {
    return [...this.pending.values()].map((t) => t.ms);
  }
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}

interface Entry {
  entry: string;
  revision: string;
  seq: number;
  path: string;
  deleted: boolean;
  blob: string | null;
  size: number;
}

/** A kura server in memory: chunks, blobs, fast-forward commits, changes. */
export class FakeServer {
  chunks = new Map<string, Uint8Array>();
  blobs = new Map<string, { size: number; chunks: string[] }>();
  entries = new Map<string, Entry>();
  seq = 0;
  epoch = "00000000-0000-4000-8000-000000000000";
  /** Deleted entries purged up to here: an older cursor gets 410 (§6.3). */
  horizon = 0;

  /** Retention: deleted entries leave, the horizon moves past them. */
  purgeDeleted(): void {
    for (const [id, e] of this.entries) {
      if (e.deleted) {
        this.entries.delete(id);
        this.horizon = Math.max(this.horizon, e.seq);
      }
    }
  }
  calls: string[] = [];
  /** An older server, without the batch routes of RFC 0002: 405. */
  older = false;

  fetch = async (url: string, init: RequestInit): Promise<Response> => {
    const u = new URL(url);
    const path = u.pathname.replace(/^\/v1\/vaults\/[^/]+/, "");
    const method = init.method ?? "GET";
    this.calls.push(`${method} ${path}`);
    const body = (): Promise<unknown> => Promise.resolve(typeof init.body === "string" ? JSON.parse(init.body) : null);
    if (method === "POST" && path === "/chunks/missing") {
      const req = await body();
      const hashes = typeof req === "object" && req !== null && "hashes" in req && Array.isArray(req.hashes) ? req.hashes : [];
      return json(200, { missing: hashes.filter((h: unknown) => typeof h === "string" && !this.chunks.has(h)) });
    }
    if (method === "POST" && (path === "/blobs/batch" || path === "/chunks/batch")) {
      if (this.older) return new Response(null, { status: 405 });
      const req = await body();
      const key = path === "/blobs/batch" ? "blobs" : "chunks";
      const list: unknown = typeof req !== "object" || req === null ? [] : key === "blobs" ? ("blobs" in req ? req.blobs : []) : "chunks" in req ? req.chunks : [];
      const hashes = Array.isArray(list) ? list.filter((h: unknown): h is string => typeof h === "string") : [];
      if (key === "blobs") {
        const manifests = hashes.flatMap((h) => {
          const b = this.blobs.get(h);
          return b === undefined ? [] : [{ hash: h, ...b }];
        });
        return json(200, { manifests, missing: hashes.filter((h) => !this.blobs.has(h)) });
      }
      const parts: number[] = [];
      for (const h of hashes) {
        const c = this.chunks.get(h);
        for (let i = 0; i < 32; i++) parts.push(parseInt(h.slice(i * 2, i * 2 + 2), 16));
        const len = c?.length ?? 0;
        parts.push(c === undefined ? 1 : 0, (len >>> 24) & 255, (len >>> 16) & 255, (len >>> 8) & 255, len & 255);
        if (c !== undefined) parts.push(...c);
      }
      return new Response(new Uint8Array(parts), { status: 200 });
    }
    const chunk = /^\/chunks\/([0-9a-f]{64})$/.exec(path);
    if (chunk?.[1] !== undefined) {
      if (method === "PUT") {
        this.chunks.set(chunk[1], init.body instanceof Uint8Array ? init.body : new Uint8Array());
        return new Response(null, { status: 204 });
      }
      const c = this.chunks.get(chunk[1]);
      return c === undefined ? json(404, { type: "/v1/problems/not_found" }) : new Response(new Uint8Array(c), { status: 200 });
    }
    const blob = /^\/blobs\/([0-9a-f]{64})$/.exec(path);
    if (blob?.[1] !== undefined) {
      if (method === "PUT") {
        const m = await body();
        if (typeof m === "object" && m !== null && "size" in m && "chunks" in m && typeof m.size === "number" && Array.isArray(m.chunks)) {
          this.blobs.set(blob[1], { size: m.size, chunks: m.chunks.filter((x: unknown): x is string => typeof x === "string") });
        }
        return new Response(null, { status: 204 });
      }
      const b = this.blobs.get(blob[1]);
      return b === undefined ? json(404, { type: "/v1/problems/not_found" }) : json(200, b);
    }
    if (method === "POST" && path === "/commits") {
      const req = await body();
      const ops = typeof req === "object" && req !== null && "operations" in req && Array.isArray(req.operations) ? req.operations : [];
      const results = ops.map((op: Record<string, unknown>) => {
        const entry = String(op.entry);
        const head: Entry = {
          entry,
          revision: crypto.randomUUID(),
          seq: ++this.seq,
          path: typeof op.path === "string" ? op.path : (this.entries.get(entry)?.path ?? ""),
          deleted: op.op === "delete",
          blob: op.op === "delete" ? null : String(op.blob),
          size: op.op === "delete" ? 0 : Number(op.size),
        };
        this.entries.set(entry, head);
        return { outcome: "accepted", head };
      });
      return json(200, { headSeq: this.seq, results });
    }
    if (method === "GET" && path === "/changes") {
      const since = Number(u.searchParams.get("since") ?? "0");
      // As the server: the horizon is checked against the pass's start.
      const start = Number(u.searchParams.get("start") ?? String(since));
      if (start > 0 && start < this.horizon) return json(410, { type: "/v1/problems/cursor_expired" });
      const items = [...this.entries.values()].filter((e) => e.seq > since).sort((a, b) => a.seq - b.seq);
      return json(200, { epoch: this.epoch, headSeq: this.seq, items });
    }
    return json(404, { type: "/v1/problems/not_found" });
  };
}
