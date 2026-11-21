// The vault folder through Obsidian's adapter (§6.8), for the core's `Host`.
// Dot-folders are never walked (decision 28): `.obsidian`, `.trash` and the
// plugin's own folder never sync as notes.
import type { KuraFile, KuraHost } from "./wasm/kura";

/** The subset of Obsidian's `DataAdapter` the host uses. */
export interface Adapter {
  list(path: string): Promise<{ files: string[]; folders: string[] }>;
  stat(path: string): Promise<{ type: "file" | "folder"; size: number; mtime: number } | null>;
  readBinary(path: string): Promise<ArrayBuffer>;
  writeBinary(path: string, data: ArrayBuffer): Promise<void>;
  write(path: string, data: string): Promise<void>;
  exists(path: string): Promise<boolean>;
  mkdir(path: string): Promise<void>;
  /** Obsidian's `rmdir`; not recursive here, so a folder with anything in it stays. */
  rmdir(path: string, recursive: boolean): Promise<void>;
  rename(from: string, to: string): Promise<void>;
  remove(path: string): Promise<void>;
  trashLocal(path: string): Promise<void>;
}

export interface HostOptions {
  adapter: Adapter;
  /** The plugin's own folder, under `.obsidian`: the state and temporary files. */
  pluginDir: string;
  vault: string;
  /** A note open in the editor is not written under the user (decision 55). */
  isOpen: (path: string) => boolean;
  /** Files above this many bytes are listed as unsyncable by the core (§6.8). */
  now: () => number;
  random: () => Uint8Array;
  /** For the configuration space (§6.8): paths are relative to this folder
   * (`.obsidian`), only those kept are listed, the state has a name of its
   * own, and `beforeWrite` runs before any remote file lands. */
  root?: string;
  keep?: (path: string) => boolean;
  stateName?: string;
  beforeWrite?: (path: string) => Promise<void>;
}

function parent(path: string): string {
  const i = path.lastIndexOf("/");
  return i < 0 ? "" : path.slice(0, i);
}

function toBuffer(data: Uint8Array): ArrayBuffer {
  const copy = new Uint8Array(data.byteLength);
  copy.set(data);
  return copy.buffer;
}

export class ObsidianHost implements KuraHost {
  constructor(private readonly o: HostOptions) {}

  statePath(): string {
    return `${this.o.pluginDir}/${this.o.stateName ?? "state"}-${this.o.vault}.json`;
  }

  /** The adapter's path of a path the core names. */
  private full(path: string): string {
    return this.o.root === undefined ? path : `${this.o.root}/${path}`;
  }

  async loadState(): Promise<string> {
    // The temporary copy stands in when a save was killed between its two steps.
    for (const p of [this.statePath(), `${this.statePath()}.tmp`]) {
      if (await this.o.adapter.exists(p)) return new TextDecoder().decode(await this.o.adapter.readBinary(p));
    }
    return "";
  }

  private async file(path: string): Promise<KuraFile | null> {
    const s = await this.o.adapter.stat(this.full(path));
    return s?.type === "file" ? { path, size: s.size, mtime: s.mtime } : null;
  }

  async list(): Promise<KuraFile[]> {
    const out: KuraFile[] = [];
    const root = this.o.root;
    if (root !== undefined && !(await this.o.adapter.exists(root))) return out;
    // Adapter paths back to the core's, relative to the root.
    const rel = (p: string): string => {
      const path = p.replace(/^\/+/, "");
      return root === undefined ? path : path.slice(root.length + 1);
    };
    const walk = async (dir: string): Promise<void> => {
      const { files, folders } = await this.o.adapter.list(dir === "" ? "/" : dir);
      for (const f of files) {
        const meta = await this.file(rel(f));
        if (meta !== null && (this.o.keep?.(meta.path) ?? true)) out.push(meta);
      }
      for (const d of folders) {
        const path = d.replace(/^\/+/, "");
        const name = path.slice(path.lastIndexOf("/") + 1);
        if (!name.startsWith(".")) await walk(path);
      }
    };
    await walk(root ?? "");
    return out;
  }

  /** RFC 0007: every folder of the notes, dot-folders not walked. The
   * configuration space syncs files only, so it lists none. */
  async folders(): Promise<string[]> {
    const out: string[] = [];
    if (this.o.root !== undefined) return out;
    const walk = async (dir: string): Promise<void> => {
      const { folders } = await this.o.adapter.list(dir === "" ? "/" : dir);
      for (const d of folders) {
        const path = d.replace(/^\/+/, "");
        if (path.slice(path.lastIndexOf("/") + 1).startsWith(".")) continue;
        out.push(path);
        await walk(path);
      }
    };
    await walk("");
    return out;
  }

  async makeFolder(path: string): Promise<void> {
    const full = this.full(path);
    if (!(await this.o.adapter.exists(full))) await this.o.adapter.mkdir(full);
  }

  /** Only a folder that holds nothing: never a file lost (RFC 0007 §4.2). */
  async removeFolderIfEmpty(path: string): Promise<boolean> {
    const full = this.full(path);
    if (!(await this.o.adapter.exists(full))) return false;
    const { files, folders } = await this.o.adapter.list(full);
    if (files.length > 0 || folders.length > 0) return false;
    await this.o.adapter.rmdir(full, false);
    return true;
  }

  async read(path: string): Promise<Uint8Array> {
    return new Uint8Array(await this.o.adapter.readBinary(this.full(path)));
  }

  async stat(path: string): Promise<KuraFile | null> {
    return this.file(path);
  }

  private async mkdirs(path: string): Promise<void> {
    const dir = parent(this.full(path));
    if (dir !== "" && !(await this.o.adapter.exists(dir))) await this.o.adapter.mkdir(dir);
  }

  async write(path: string, data: Uint8Array): Promise<KuraFile> {
    if (this.o.isOpen(path)) throw new Error(`${path} is open in the editor; written once it is closed`);
    // Before anything lands: a plugin whose code changes is disabled first (§6.8).
    await this.o.beforeWrite?.(path);
    await this.mkdirs(path);
    // Through a temporary file in the plugin's folder, then into place.
    const rand = Array.from(this.o.random(), (b) => b.toString(16).padStart(2, "0")).join("");
    const tmp = `${this.o.pluginDir}/tmp-${rand}`;
    await this.o.adapter.writeBinary(tmp, toBuffer(data));
    const full = this.full(path);
    if (await this.o.adapter.exists(full)) await this.o.adapter.remove(full);
    await this.o.adapter.rename(tmp, full);
    const meta = await this.file(path);
    if (meta === null) throw new Error(`${path} was not written`);
    return meta;
  }

  async rename(from: string, to: string): Promise<KuraFile> {
    if (await this.o.adapter.exists(this.full(to))) throw new Error(`${to} already exists`);
    if (this.o.isOpen(from)) throw new Error(`${from} is open in the editor`);
    await this.o.beforeWrite?.(to);
    await this.mkdirs(to);
    await this.o.adapter.rename(this.full(from), this.full(to));
    const meta = await this.file(to);
    if (meta === null) throw new Error(`${to} was not moved`);
    return meta;
  }

  async trash(path: string): Promise<void> {
    // Not under the user either (review 2026-11-15 #4): the core retries later.
    if (this.o.isOpen(path)) throw new Error(`${path} is open in the editor; trashed once it is closed`);
    // Obsidian's own `.trash`, never unlinked (§6.3).
    await this.o.adapter.trashLocal(this.full(path));
  }

  async save(state: string): Promise<void> {
    // Never half written (the OS kills a backgrounded app): a torn state would
    // be read as none, bringing deleted notes back (review 2026-11-21 #2).
    const p = this.statePath();
    const tmp = `${p}.tmp`;
    await this.o.adapter.write(tmp, state);
    if (await this.o.adapter.exists(p)) await this.o.adapter.remove(p);
    await this.o.adapter.rename(tmp, p);
  }

  nowMs(): number {
    return this.o.now();
  }

  random(): Uint8Array {
    return this.o.random();
  }
}
