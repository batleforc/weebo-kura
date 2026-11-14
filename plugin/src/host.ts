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
    return `${this.o.pluginDir}/state-${this.o.vault}.json`;
  }

  async loadState(): Promise<string> {
    const p = this.statePath();
    return (await this.o.adapter.exists(p)) ? new TextDecoder().decode(await this.o.adapter.readBinary(p)) : "";
  }

  private async file(path: string): Promise<KuraFile | null> {
    const s = await this.o.adapter.stat(path);
    return s?.type === "file" ? { path, size: s.size, mtime: s.mtime } : null;
  }

  async list(): Promise<KuraFile[]> {
    const out: KuraFile[] = [];
    const walk = async (dir: string): Promise<void> => {
      const { files, folders } = await this.o.adapter.list(dir === "" ? "/" : dir);
      for (const f of files) {
        const path = f.replace(/^\/+/, "");
        const meta = await this.file(path);
        if (meta !== null) out.push(meta);
      }
      for (const d of folders) {
        const path = d.replace(/^\/+/, "");
        const name = path.slice(path.lastIndexOf("/") + 1);
        if (!name.startsWith(".")) await walk(path);
      }
    };
    await walk("");
    return out;
  }

  async read(path: string): Promise<Uint8Array> {
    return new Uint8Array(await this.o.adapter.readBinary(path));
  }

  async stat(path: string): Promise<KuraFile | null> {
    return this.file(path);
  }

  private async mkdirs(path: string): Promise<void> {
    const dir = parent(path);
    if (dir !== "" && !(await this.o.adapter.exists(dir))) await this.o.adapter.mkdir(dir);
  }

  async write(path: string, data: Uint8Array): Promise<KuraFile> {
    if (this.o.isOpen(path)) throw new Error(`${path} is open in the editor; written once it is closed`);
    await this.mkdirs(path);
    // Through a temporary file in the plugin's folder, then into place.
    const rand = Array.from(this.o.random(), (b) => b.toString(16).padStart(2, "0")).join("");
    const tmp = `${this.o.pluginDir}/tmp-${rand}`;
    await this.o.adapter.writeBinary(tmp, toBuffer(data));
    if (await this.o.adapter.exists(path)) await this.o.adapter.remove(path);
    await this.o.adapter.rename(tmp, path);
    const meta = await this.file(path);
    if (meta === null) throw new Error(`${path} was not written`);
    return meta;
  }

  async rename(from: string, to: string): Promise<KuraFile> {
    if (await this.o.adapter.exists(to)) throw new Error(`${to} already exists`);
    if (this.o.isOpen(from)) throw new Error(`${from} is open in the editor`);
    await this.mkdirs(to);
    await this.o.adapter.rename(from, to);
    const meta = await this.file(to);
    if (meta === null) throw new Error(`${to} was not moved`);
    return meta;
  }

  async trash(path: string): Promise<void> {
    // Obsidian's own `.trash`, never unlinked (§6.3).
    await this.o.adapter.trashLocal(path);
  }

  async save(state: string): Promise<void> {
    await this.o.adapter.write(this.statePath(), state);
  }

  nowMs(): number {
    return this.o.now();
  }

  random(): Uint8Array {
    return this.o.random();
  }
}
