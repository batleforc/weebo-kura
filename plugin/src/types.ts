// What the WASM core returns, checked at the boundary rather than trusted.

/** A problem document the server answered (RFC 9457, §4.4). */
export interface Problem {
  status: number;
  kind: string;
  detail: string | null;
}

/** One sync run, as the core reports it. */
export interface Report {
  pushed: number;
  pulled: number;
  trashed: number;
  conflicts: string[];
  unsyncable: [string, string][];
  deferred: string[];
  failed: [string, string][];
  refused: Problem | null;
  /** A full resync ran after this answer (§6.3). */
  resynced: string | null;
}

/** A refusal thrown to the core: its enumerable `status`, `kind` and
 * `detail` are what kura-wasm reads from the rejection. */
export class ProblemError extends Error implements Problem {
  constructor(
    readonly status: number,
    readonly kind: string,
    readonly detail: string | null,
  ) {
    super(`${kind} (${String(status)})${detail === null ? "" : `: ${detail}`}`);
  }
}

export function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

export function isProblem(v: unknown): v is Problem {
  return isRecord(v) && typeof v.status === "number" && typeof v.kind === "string";
}

function strings(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((s): s is string => typeof s === "string") : [];
}

function pairs(v: unknown): [string, string][] {
  if (!Array.isArray(v)) return [];
  const out: [string, string][] = [];
  for (const p of v) {
    if (Array.isArray(p) && typeof p[0] === "string" && typeof p[1] === "string") out.push([p[0], p[1]]);
  }
  return out;
}

function count(v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? v : 0;
}

/** The `{ state, report }` a sync resolves to. */
export function parseOutcome(v: unknown): { state: string; report: Report } {
  if (!isRecord(v) || !isRecord(v.report)) throw new Error("unexpected answer from the sync core");
  const r = v.report;
  return {
    state: JSON.stringify(v.state ?? null),
    report: {
      pushed: count(r.pushed),
      pulled: count(r.pulled),
      trashed: count(r.trashed),
      conflicts: strings(r.conflicts),
      unsyncable: pairs(r.unsyncable),
      deferred: strings(r.deferred),
      failed: pairs(r.failed),
      refused: isProblem(r.refused) ? r.refused : null,
      resynced: typeof r.resynced === "string" ? r.resynced : null,
    },
  };
}

/** A full resync stopped before trashing more than the guard allows (§6.3). */
export function deletionsToConfirm(v: unknown): { deletions: number; files: number } | null {
  if (!isProblem(v) || v.kind !== "too_many_deletions" || !isRecord(v)) return null;
  return { deletions: count(v.deletions), files: count(v.files) };
}
