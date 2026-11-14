// Synced plugins (RFC 0001 §6.8, decision 34): a plugin received from
// another of the user's devices, or a new version of one, never runs before
// the user agrees on this device. The accepted code hash is kept on this
// device only: a pin written through the API by another session enables
// nothing (phase 8 review). Snippets and themes, which are CSS, need no
// agreement.
import { isRecord } from "./types";

/** Obsidian's plugin switchboard, as far as the guard needs it. */
export interface PluginRegistry {
  /** Loaded, or listed in `community-plugins.json` to load at the next start. */
  isEnabled(id: string): Promise<boolean>;
  enable(id: string): Promise<void>;
  disable(id: string): Promise<void>;
}

/** What this device decided, saved in the plugin's `data.json`. */
export interface Consent {
  /** Plugin id → the `main.js` blob hash agreed to here. */
  accepted: Record<string, string>;
  /** Plugin id → a hash refused here: not asked again until it changes. */
  declined: Record<string, string>;
  /** Plugins written by a pull and not decided yet. */
  pending: string[];
  /** Plugins the guard turned off before new code landed: the only ones it
   * turns back on without asking (phase 8 review M1). */
  disabled: string[];
}

export function emptyConsent(): Consent {
  return { accepted: {}, declined: {}, pending: [], disabled: [] };
}

function strings(v: unknown): Record<string, string> {
  const out: Record<string, string> = {};
  if (!isRecord(v)) return out;
  for (const [k, x] of Object.entries(v)) if (typeof x === "string") out[k] = x;
  return out;
}

/** Stored consent, field by field: `data.json` is not trusted blindly. */
export function readConsent(v: unknown): Consent {
  if (!isRecord(v)) return emptyConsent();
  return {
    accepted: strings(v.accepted),
    declined: strings(v.declined),
    pending: Array.isArray(v.pending) ? v.pending.filter((x): x is string => typeof x === "string") : [],
    disabled: Array.isArray(v.disabled) ? v.disabled.filter((x): x is string => typeof x === "string") : [],
  };
}

/** The plugin of a path under `.obsidian` (its id in lower case), and
 * whether it is its code: without regard to case, since `MAIN.JS` is
 * `main.js` on a case-insensitive file system (phase 8 review H2). */
export function pluginOf(path: string): { id: string; code: boolean } | null {
  const m = /^plugins\/([^/]+)\/(.+)$/.exec(path.toLowerCase());
  const id = m?.[1];
  const file = m?.[2];
  return id === undefined || file === undefined ? null : { id, code: file === "main.js" };
}

/** Plugin id → its `main.js` blob hash, from the configuration state the core saved. */
export function codeHashes(state: string): Map<string, string> {
  const out = new Map<string, string>();
  let parsed: unknown;
  try {
    parsed = JSON.parse(state);
  } catch {
    return out;
  }
  if (!isRecord(parsed) || !isRecord(parsed.entries)) return out;
  for (const k of Object.values(parsed.entries)) {
    if (!isRecord(k) || typeof k.path !== "string" || typeof k.blob !== "string") continue;
    const p = pluginOf(k.path);
    if (p?.code === true) out.set(p.id, k.blob);
  }
  return out;
}

export class PluginGuard {
  constructor(
    private readonly reg: PluginRegistry,
    private readonly consent: Consent,
    private readonly save: () => Promise<void>,
    /** Asks the user (§4.5 mock); resolves to true for [Enable]. */
    private readonly ask: (id: string) => Promise<boolean>,
  ) {}

  /** Before a remote file lands under `plugins/<id>/`: the plugin awaits a
   * decision, and if its code is about to change while it runs, it is
   * disabled first, so a crash in between never runs unagreed code. */
  async beforeWrite(path: string): Promise<void> {
    const p = pluginOf(path);
    if (p === null) return;
    if (!this.consent.pending.includes(p.id)) this.consent.pending.push(p.id);
    if (p.code && (await this.reg.isEnabled(p.id))) {
      await this.reg.disable(p.id);
      if (!this.consent.disabled.includes(p.id)) this.consent.disabled.push(p.id);
    }
    await this.save();
  }

  /** After a configuration sync: each pending plugin is decided once per
   * code hash. Code agreed to before is turned back on without asking only
   * if the guard turned it off; a plugin the user keeps off stays off. */
  async review(hashes: Map<string, string>): Promise<void> {
    for (const id of [...this.consent.pending]) {
      const hash = hashes.get(id);
      const turnedOff = this.consent.disabled.includes(id);
      if (hash !== undefined && this.consent.declined[id] !== hash) {
        let enable = false;
        if (this.consent.accepted[id] === hash) {
          enable = turnedOff;
        } else if (turnedOff || !(await this.reg.isEnabled(id))) {
          enable = await this.ask(id);
          if (enable) this.consent.accepted[id] = hash;
          else this.consent.declined[id] = hash;
        }
        if (enable && !(await this.reg.isEnabled(id))) await this.reg.enable(id);
      }
      this.consent.pending = this.consent.pending.filter((x) => x !== id);
      this.consent.disabled = this.consent.disabled.filter((x) => x !== id);
      await this.save();
    }
  }
}

/** A configuration state without the entries whose path is no longer kept:
 * a category turned off is forgotten here, never pushed as a deletion. */
export function forget(state: string, keep: (path: string) => boolean): string {
  let parsed: unknown;
  try {
    parsed = JSON.parse(state);
  } catch {
    return state;
  }
  if (!isRecord(parsed) || !isRecord(parsed.entries)) return state;
  const entries = Object.fromEntries(
    Object.entries(parsed.entries).filter(([, k]) => isRecord(k) && typeof k.path === "string" && keep(k.path)),
  );
  return JSON.stringify({ ...parsed, entries });
}
