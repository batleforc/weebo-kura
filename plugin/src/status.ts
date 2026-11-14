// The words of §4.5: the status bar, the ribbon label on mobile and the first
// line of the settings panel all show the same text, never colour alone.
import type { Problem, Report } from "./types";

export interface Situation {
  signedIn: boolean;
  online: boolean;
  syncing: boolean;
  /** Local changes not yet pushed. */
  pending: number;
  role: string | null;
  last: Report | null;
  /** The last refusal or failure that stopped a run. */
  error: Problem | null;
}

export function word(s: Situation): string {
  if (!s.signedIn || s.error?.kind === "invalid_token" || s.error?.kind === "device_revoked") return "sign in";
  if (s.error?.kind === "client_too_old") return "update required";
  if (s.error?.kind === "history_rewound" || s.error?.kind === "cursor_expired") return "resyncing";
  if (s.error?.kind === "too_many_deletions") return "confirm deletions";
  if (!s.online) return "offline";
  if (s.syncing) return s.pending > 0 ? `syncing ${String(s.pending)}` : "syncing";
  const refused = s.last?.refused ?? null;
  if (refused?.kind === "quota_exceeded") return "quota full";
  if (refused?.kind === "insufficient_role" || (s.role === "viewer" && s.pending > 0)) return `read-only (${s.role ?? "viewer"})`;
  const conflicts = s.last?.conflicts.length ?? 0;
  if (conflicts > 0) return `${String(conflicts)} conflict${conflicts === 1 ? "" : "s"}`;
  if ((s.last?.unsyncable.length ?? 0) > 0) return "unsyncable";
  return "in sync";
}

export function label(s: Situation): string {
  return `Kura: ${word(s)}`;
}
