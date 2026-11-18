// From a note in Obsidian to the same note in the Kura console (RFC 0005):
// the entry id comes from this device's sync state, never from a guess.
import { isRecord } from "./types";

/** The entry a synced file is, by its path in the state the core keeps; null when it is not synced yet. */
export function entryOf(state: string, path: string): string | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(state);
  } catch {
    return null;
  }
  const entries = isRecord(parsed) ? parsed.entries : null;
  if (!isRecord(entries)) return null;
  for (const [id, known] of Object.entries(entries)) {
    if (isRecord(known) && known.path === path) return id;
  }
  return null;
}

/** The console's page for a vault, one of its notes, or that note's history. */
export function consoleUrl(server: string, vault: string, entry: string | null = null, history = false): string {
  const base = `${server.replace(/\/+$/, "")}/app/vaults/${encodeURIComponent(vault)}`;
  if (entry === null) return base;
  return `${base}/notes/${encodeURIComponent(entry)}${history ? "/history" : ""}`;
}
