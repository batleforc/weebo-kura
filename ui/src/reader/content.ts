/**
 * Note content for the reader (RFC 0005 §6.5): a manifest, then its chunks in
 * batches, concatenated. Sent with `fetch` and `cache: "no-store"`: chunk
 * answers are `private, immutable` for syncing devices, wrong for a shared
 * family computer (T8).
 */

import { ensureDevice } from "@/api/client";
import { useSession } from "@/auth/session";

export class ContentError extends Error {
  constructor(
    readonly status: number,
    readonly type?: string,
  ) {
    super(`The server answered ${status}${type ? ` (${type})` : ""}.`);
  }
}

async function call(path: string, init: RequestInit = {}): Promise<Response> {
  const token = await useSession().bearer();
  const device = await ensureDevice();
  const headers = new Headers(init.headers);
  if (token) headers.set("authorization", `Bearer ${token}`);
  headers.set("x-kura-device", device);
  const response = await fetch(path, { ...init, headers, cache: "no-store" });
  if (!response.ok) {
    const problem = (await response.json().catch(() => ({}))) as {
      type?: string;
    };
    throw new ContentError(
      response.status,
      problem.type?.replace(/^\/v1\/problems\//, ""),
    );
  }
  return response;
}

const RECORD_HEADER = 37;

/** `chunks/batch` records (RFC 0002 §4.4): hash, status, length, bytes. */
export function parseRecords(body: Uint8Array): Map<string, Uint8Array | null> {
  const out = new Map<string, Uint8Array | null>();
  const view = new DataView(body.buffer, body.byteOffset, body.byteLength);
  let at = 0;
  while (at < body.length) {
    if (at + RECORD_HEADER > body.length)
      throw new Error("a chunk record is cut short");
    const hash = Array.from(body.subarray(at, at + 32), (b) =>
      b.toString(16).padStart(2, "0"),
    ).join("");
    const status = body[at + 32];
    const length = view.getUint32(at + 33);
    const start = at + RECORD_HEADER;
    if (start + length > body.length)
      throw new Error("a chunk record is cut short");
    out.set(hash, status === 0 ? body.subarray(start, start + length) : null);
    at = start + length;
  }
  return out;
}

/** The bytes of a stored file, by its blob hash; `null` when not visible. */
export async function fileBytes(
  vault: string,
  blob: string,
): Promise<Uint8Array | null> {
  let manifest: { size: number; chunks: string[] };
  try {
    manifest = (await (
      await call(`/v1/vaults/${vault}/blobs/${blob}`)
    ).json()) as typeof manifest;
  } catch (error) {
    if (error instanceof ContentError && error.status === 404) return null;
    throw error;
  }
  const found = new Map<string, Uint8Array>();
  let wanted = [...new Set(manifest.chunks)];
  while (wanted.length > 0) {
    const response = await call(`/v1/vaults/${vault}/chunks/batch`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ chunks: wanted.slice(0, 1000) }),
    });
    const records = parseRecords(new Uint8Array(await response.arrayBuffer()));
    // An answer is cut at 4 MiB (RFC 0002): what it left out is asked again.
    // A chunk it says is missing makes the file unreadable here.
    let progress = false;
    for (const [hash, data] of records) {
      if (!wanted.includes(hash)) continue;
      if (data === null) return null;
      found.set(hash, data);
      progress = true;
    }
    wanted = wanted.filter((h) => !found.has(h));
    if (!progress) break;
  }
  if (wanted.length > 0) return null;
  const out = new Uint8Array(manifest.size);
  let offset = 0;
  for (const hash of manifest.chunks) {
    const part = found.get(hash);
    if (!part || offset + part.length > out.length) return null;
    out.set(part, offset);
    offset += part.length;
  }
  return offset === out.length ? out : null;
}
