// Synced plugins (RFC 0001 §6.8, decision 34): a plugin received from
// another of the user's devices, or a new version of one, never runs before
// the user agrees on this device. The accepted code hash is kept on this
// device only: a pin written through the API by another session enables
// nothing (phase 8 review). Snippets and themes, which are CSS, need no
// agreement.
import type { Adapter } from "./host";
import { isRecord } from "./types";

/** Obsidian's plugin switchboard, as far as the guard needs it. The guard
 * names a plugin by its folder under `plugins/` (lower case); Obsidian
 * enables it by the `id` of its manifest, which may differ. */
export interface PluginRegistry {
  /** Loaded, or listed in `community-plugins.json` to load at the next start,
   * under the folder's name or any id its manifest declares. */
  isEnabled(folder: string): Promise<boolean>;
  enable(folder: string): Promise<void>;
  disable(folder: string): Promise<void>;
  /** The folder's plugin is known under an id other than the folder's name. */
  aliased(folder: string): Promise<boolean>;
}

/** What the registry reads of `.obsidian`. */
export interface RegistryAdapter {
  read(path: string): Promise<string>;
  write(path: string, data: string): Promise<void>;
  list(path: string): Promise<{ files: string[]; folders: string[] }>;
}

/** Obsidian's switchboard (`app.plugins`, not in the public API), read
 * defensively: if its shape changes, configuration sync enables nothing. A
 * folder is resolved to every id it may run under: its name, the id of a
 * loaded plugin whose `dir` is that folder, and the `id` its own
 * `manifest.json` declares, so code landing in a folder named unlike its
 * id still turns that plugin off first. */
export function obsidianRegistry(plugins: unknown, adapter: RegistryAdapter, configDir: string): PluginRegistry | null {
  if (!isRecord(plugins)) return null;
  const enabled = plugins.enabledPlugins;
  const on = plugins.enablePluginAndSave;
  const off = plugins.disablePluginAndSave;
  if (!(enabled instanceof Set) || typeof on !== "function" || typeof off !== "function") return null;
  // What Obsidian loads at its next start, besides what runs now: a plugin
  // listed there but not loaded is enabled too (phase 8 review M2).
  const listFile = `${configDir}/community-plugins.json`;
  const listed = async (): Promise<string[]> => {
    try {
      const v: unknown = JSON.parse(await adapter.read(listFile));
      return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
    } catch {
      return [];
    }
  };
  const lower = (x: unknown): string | null => (typeof x === "string" ? x.toLowerCase() : null);
  /** The folder as it is on disk (`plugins/DataView`), found without regard to case. */
  const onDisk = async (folder: string): Promise<string | null> => {
    try {
      const { folders } = await adapter.list(`${configDir}/plugins`);
      return folders.find((f) => f.split("/").pop()?.toLowerCase() === folder) ?? null;
    } catch {
      return null;
    }
  };
  /** Every id (lower case) the folder's plugin may be enabled under; the
   * manifest's own id first, the one Obsidian enables. */
  const idsOf = async (folder: string): Promise<string[]> => {
    const ids: string[] = [];
    const dir = await onDisk(folder);
    if (dir !== null) {
      try {
        const m: unknown = JSON.parse(await adapter.read(`${dir}/manifest.json`));
        const id = isRecord(m) ? lower(m.id) : null;
        if (id) ids.push(id);
      } catch {
        // No readable manifest: the other names still count.
      }
    }
    const manifests = plugins.manifests;
    if (isRecord(manifests)) {
      for (const m of Object.values(manifests)) {
        if (!isRecord(m) || typeof m.dir !== "string") continue;
        const id = lower(m.id);
        if (id && m.dir.split("/").pop()?.toLowerCase() === folder) ids.push(id);
      }
    }
    ids.push(folder);
    return [...new Set(ids)];
  };
  // Ids compare without regard to case, as the guard folds them.
  const same = (a: unknown, id: string): boolean => lower(a) === id;
  const actual = (id: string, list: string[]): string =>
    [...enabled].find((x): x is string => same(x, id)) ?? list.find((x) => same(x, id)) ?? id;
  const isOn = (id: string, list: string[]): boolean => [...enabled].some((x) => same(x, id)) || list.some((x) => same(x, id));
  return {
    isEnabled: async (folder) => {
      const list = await listed();
      return (await idsOf(folder)).some((id) => isOn(id, list));
    },
    enable: async (folder) => {
      const [id = folder] = await idsOf(folder);
      await Promise.resolve(Reflect.apply(on, plugins, [actual(id, await listed())]));
    },
    disable: async (folder) => {
      const ids = await idsOf(folder);
      const list = await listed();
      for (const id of ids.filter((x) => isOn(x, list))) {
        await Promise.resolve(Reflect.apply(off, plugins, [actual(id, list)]));
      }
      const kept = list.filter((x) => !ids.some((id) => same(x, id)));
      if (kept.length !== list.length) await adapter.write(listFile, JSON.stringify(kept));
    },
    aliased: async (folder) => (await idsOf(folder)).some((id) => id !== folder),
  };
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
  /** Vault plugins declined for good here: never written (RFC 0010 decision 11). */
  never: string[];
}

export function emptyConsent(): Consent {
  return { accepted: {}, declined: {}, pending: [], disabled: [], never: [] };
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
    never: Array.isArray(v.never) ? v.never.filter((x): x is string => typeof x === "string") : [],
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
    /** Asks the user (§4.5 mock); resolves to true for [Enable], "never"
     * for a vault plugin declined for good (RFC 0010 decision 11). */
    private readonly ask: (id: string) => Promise<boolean | "never">,
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
      let turnedOff = this.consent.disabled.includes(id);
      // Code may land before the manifest that names it after an enabled
      // plugin (`main.js` sorts first): once both are here, unagreed code in
      // a folder running under another id is turned off before anything else.
      if (
        hash !== undefined &&
        !turnedOff &&
        this.consent.accepted[id] !== hash &&
        (await this.reg.aliased(id)) &&
        (await this.reg.isEnabled(id))
      ) {
        await this.reg.disable(id);
        this.consent.disabled.push(id);
        turnedOff = true;
      }
      if (hash !== undefined && this.consent.declined[id] !== hash) {
        let enable = false;
        if (this.consent.accepted[id] === hash) {
          enable = turnedOff;
        } else if (turnedOff || !(await this.reg.isEnabled(id))) {
          const answer = await this.ask(id);
          enable = answer === true;
          if (enable) this.consent.accepted[id] = hash;
          else this.consent.declined[id] = hash;
          if (answer === "never" && !this.consent.never.includes(id)) this.consent.never.push(id);
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

/** The three files a vault may carry for a plugin (RFC 0010 §4.2). */
export const CODE_FILES = ["manifest.json", "main.js", "styles.css"];

/** Where new code of `plugins/<id>/<file>` waits: a dot folder Obsidian does not load. */
export function pendingOf(path: string): string {
  const i = path.lastIndexOf("/");
  return `${path.slice(0, i)}/.kura-pending/${path.slice(i + 1)}`;
}

/** Decision 10: a vault plugin agreed to here keeps running its version;
 * the managers' new code waits in `.kura-pending/` and is swapped in only
 * once the member agrees. The core sees the waiting file in place of the
 * running one (`place`), so neither is ever taken for a local edit. */
export class VaultUpdates {
  private readonly diverted = new Set<string>();
  private readonly known = new Set<string>();

  constructor(
    private readonly o: {
      adapter: Adapter;
      /** `.obsidian`. */
      root: string;
      reg: PluginRegistry;
      consent: Consent;
      save: () => Promise<void>;
      ask: (id: string) => Promise<"use" | "later" | "never">;
    },
  ) {}

  /** The vault pass's state: what it holds, and what already waits on disk. */
  async load(state: string): Promise<void> {
    let parsed: unknown;
    try {
      parsed = JSON.parse(state);
    } catch {
      return;
    }
    if (!isRecord(parsed) || !isRecord(parsed.entries)) return;
    for (const k of Object.values(parsed.entries)) {
      if (!isRecord(k) || typeof k.path !== "string" || pluginOf(k.path) === null) continue;
      this.known.add(k.path);
      if (await this.o.adapter.exists(`${this.o.root}/${pendingOf(k.path)}`)) this.diverted.add(k.path);
    }
  }

  /** The path, under the root, where the core's file is. */
  place(path: string): string {
    return this.diverted.has(path) ? pendingOf(path) : path;
  }

  /** Plugins with new code waiting, by id. */
  waiting(): string[] {
    return [...new Set([...this.diverted].flatMap((p) => pluginOf(p)?.id ?? []))];
  }

  /** Before a vault file lands: true when it goes to `.kura-pending/`, new
   * code of a plugin agreed here whose running file it would replace. */
  async divert(path: string): Promise<boolean> {
    const p = pluginOf(path);
    const name = path.slice(path.lastIndexOf("/") + 1).toLowerCase();
    if (p === null || !CODE_FILES.includes(name) || this.o.consent.accepted[p.id] === undefined) return false;
    if (!this.diverted.has(path)) {
      if (!this.known.has(path) || !(await this.o.adapter.exists(`${this.o.root}/${path}`))) return false;
      this.diverted.add(path);
    }
    if (!this.o.consent.pending.includes(p.id)) this.o.consent.pending.push(p.id);
    await this.o.save();
    return true;
  }

  /** After the vault pass, or from the settings' [Review]: each plugin with
   * new code waiting is asked about once per code hash. */
  async review(hashes: Map<string, string>): Promise<void> {
    const c = this.o.consent;
    for (const id of this.waiting().filter((x) => c.pending.includes(x))) {
      const hash = hashes.get(id);
      if (hash !== undefined && c.accepted[id] === hash) {
        // The code agreed to already: only its manifest or styles changed.
        await this.swap(id);
      } else if (hash !== undefined && c.declined[id] !== hash) {
        const answer = await this.o.ask(id);
        if (answer === "use") {
          c.accepted[id] = hash;
          await this.swap(id);
        } else {
          c.declined[id] = hash;
          if (answer === "never") {
            if (!c.never.includes(id)) c.never.push(id);
            if (await this.o.reg.isEnabled(id)) await this.o.reg.disable(id);
            await this.drop(id);
          }
        }
      }
      c.pending = c.pending.filter((x) => x !== id);
      await this.o.save();
    }
  }

  private files(id: string): string[] {
    return [...this.diverted].filter((p) => pluginOf(p)?.id === id);
  }

  /** The waiting code in place of the running one, the plugin off meanwhile. */
  private async swap(id: string): Promise<void> {
    const a = this.o.adapter;
    const on = await this.o.reg.isEnabled(id);
    if (on) await this.o.reg.disable(id);
    for (const p of this.files(id)) {
      const live = `${this.o.root}/${p}`;
      if (await a.exists(live)) await a.remove(live);
      await a.rename(`${this.o.root}/${pendingOf(p)}`, live);
      this.diverted.delete(p);
    }
    await this.rmdir(id);
    if (on) await this.o.reg.enable(id);
  }

  /** The waiting code, declined for good. */
  private async drop(id: string): Promise<void> {
    for (const p of this.files(id)) {
      await this.o.adapter.remove(`${this.o.root}/${pendingOf(p)}`);
      this.diverted.delete(p);
    }
    await this.rmdir(id);
  }

  private async rmdir(id: string): Promise<void> {
    try {
      await this.o.adapter.rmdir(`${this.o.root}/plugins/${id}/.kura-pending`, false);
    } catch {
      // Something else in it: left as it is.
    }
  }
}
