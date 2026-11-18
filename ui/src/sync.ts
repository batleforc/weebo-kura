// Where a device stands on a vault (RFC 0006), as the console words it.
import type { DeviceReport, DeviceSync } from "@/api/generated/types.gen";

/** Above this, the server stopped counting: "1,000+". */
export const BEHIND_CAP = 1000;

export type Standing =
  | { kind: "resync" }
  | { kind: "never" }
  | { kind: "current" }
  | { kind: "behind"; n: number; more: boolean };

export function standing(d: DeviceSync): Standing {
  if (d.resync) return { kind: "resync" };
  if (d.behind === undefined || d.behind === null) return { kind: "never" };
  if (d.behind === 0) return { kind: "current" };
  return {
    kind: "behind",
    n: Math.min(d.behind, BEHIND_CAP),
    more: d.behind > BEHIND_CAP,
  };
}

/** What a device's last run left undone, in the order a person acts on it. */
export function leftovers(
  r: DeviceReport,
): { key: "pending" | "conflicts" | "unsyncable" | "failed"; n: number }[] {
  return (["pending", "conflicts", "unsyncable", "failed"] as const)
    .map((key) => ({ key, n: r[key] }))
    .filter((x) => x.n > 0);
}

/** How far from the head, for sorting: a resync first, then most behind. */
function distance(s: Standing): number {
  switch (s.kind) {
    case "resync":
      return Number.MAX_SAFE_INTEGER;
    case "behind":
      return s.n + (s.more ? 1 : 0);
    case "never":
      return -1;
    case "current":
      return 0;
  }
}

/** Most behind first, then by name (§4.5). */
export function byDistance(rows: DeviceSync[]): DeviceSync[] {
  return [...rows].sort(
    (a, b) =>
      distance(standing(b)) - distance(standing(a)) ||
      a.name.localeCompare(b.name),
  );
}
