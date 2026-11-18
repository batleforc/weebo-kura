// The words of §4.5: the status bar, the ribbon label on mobile and the first
// line of the settings panel all show the same text, never colour alone; the
// icon beside it repeats the state, it never replaces the word.
import { roleName, t } from "./i18n";
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

/** The state, coarse: what the icon and the colour cue follow. */
export type Tone = "ok" | "busy" | "warn" | "off" | "error";

interface Reading {
  word: string;
  tone: Tone;
}

function read(s: Situation): Reading {
  if (!s.signedIn || s.error?.kind === "invalid_token" || s.error?.kind === "device_revoked") return { word: t("word.signIn"), tone: "error" };
  if (s.error?.kind === "client_too_old") return { word: t("word.update"), tone: "error" };
  if (s.error?.kind === "history_rewound" || s.error?.kind === "cursor_expired") return { word: t("word.resyncing"), tone: "busy" };
  if (s.error?.kind === "too_many_deletions") return { word: t("word.confirm"), tone: "warn" };
  if (!s.online) return { word: t("word.offline"), tone: "off" };
  if (s.syncing) return { word: s.pending > 0 ? t("word.syncingN", { n: s.pending }) : t("word.syncing"), tone: "busy" };
  const refused = s.last?.refused ?? null;
  if (refused?.kind === "quota_exceeded") return { word: t("word.quota"), tone: "error" };
  if (refused?.kind === "insufficient_role" || (s.role === "viewer" && s.pending > 0)) return { word: t("word.readOnly", { role: roleName(s.role ?? "viewer") }), tone: "warn" };
  const conflicts = s.last?.conflicts.length ?? 0;
  if (conflicts > 0) return { word: conflicts === 1 ? t("word.conflict") : t("word.conflicts", { n: conflicts }), tone: "warn" };
  if ((s.last?.unsyncable.length ?? 0) > 0 || (s.last?.failed.length ?? 0) > 0) return { word: t("word.unsyncable"), tone: "warn" };
  return { word: t("word.inSync"), tone: "ok" };
}

export function word(s: Situation): string {
  return read(s).word;
}

export function tone(s: Situation): Tone {
  return read(s).tone;
}

/** The Lucide icon Obsidian draws for a tone. */
export function icon(tn: Tone): string {
  return { ok: "cloud-check", busy: "refresh-cw", warn: "alert-triangle", off: "cloud-off", error: "alert-circle" }[tn];
}

export function label(s: Situation): string {
  return `Kura: ${word(s)}`;
}

/** Conflicts this run found that the one before had not: worth a notice. */
export function newConflicts(before: Report | null, now: Report | null): string[] {
  const seen = new Set(before?.conflicts ?? []);
  return (now?.conflicts ?? []).filter((c) => !seen.has(c));
}
