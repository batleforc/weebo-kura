/** The entries a caller may list, as a folder tree and a link table (RFC 0005 §4.2). */

import type { EntryState } from "@/api/generated/types.gen";

export interface Folder {
  name: string;
  path: string;
  folders: Folder[];
  notes: EntryState[];
}

/** One tree of every listed entry; folders and notes sorted by name. Folder
 * entries (RFC 0007) add the folders no note is in, empty ones included. */
export function buildTree(
  entries: readonly EntryState[],
  folders: readonly EntryState[] = [],
): Folder {
  const root: Folder = { name: "", path: "", folders: [], notes: [] };
  const into = (parts: readonly string[]): Folder => {
    let at = root;
    for (const name of parts) {
      let next = at.folders.find((f) => f.name === name);
      if (!next) {
        next = { name, path: `${at.path}${name}/`, folders: [], notes: [] };
        at.folders.push(next);
      }
      at = next;
    }
    return at;
  };
  for (const f of folders) {
    if (!f.deleted) into(f.path.split("/"));
  }
  for (const e of entries) {
    if (e.deleted) continue;
    const parts = e.path.split("/");
    parts.pop();
    into(parts).notes.push(e);
  }
  const sort = (f: Folder) => {
    f.folders.sort((a, b) => a.name.localeCompare(b.name));
    f.notes.sort((a, b) => a.path.localeCompare(b.path));
    f.folders.forEach(sort);
  };
  sort(root);
  return root;
}

/** A note's name: its file name without `.md`. */
export function noteName(path: string): string {
  return (path.split("/").pop() ?? path).replace(/\.md$/i, "");
}

/**
 * Resolves a link target against the entries the caller was listed, never
 * against the server: a hidden target is indistinguishable from a missing
 * one (T2). Tries the exact path, the path with `.md`, then the note name.
 */
export function resolver(
  entries: readonly EntryState[],
): (target: string) => EntryState | undefined {
  const live = entries.filter((e) => !e.deleted);
  const byPath = new Map(live.map((e) => [e.path.toLowerCase(), e]));
  const byName = new Map<string, EntryState>();
  for (const e of live) {
    const key = noteName(e.path).toLowerCase();
    if (!byName.has(key)) byName.set(key, e);
  }
  return (raw) => {
    const target = decodeURIComponent(raw.split("#")[0] ?? "")
      .replace(/^\.?\//, "")
      .toLowerCase();
    if (!target) return undefined;
    return (
      byPath.get(target) ??
      byPath.get(`${target}.md`) ??
      byName.get(target.replace(/\.md$/, ""))
    );
  };
}

/** Every note in the order the tree shows them: a folder's folders, then its notes. */
export function notesInOrder(folder: Folder): EntryState[] {
  return [...folder.folders.flatMap(notesInOrder), ...folder.notes];
}

/** The folders on the way to a path, as the tree names them: `a/`, `a/b/`. */
export function foldersOf(path: string): string[] {
  const parts = path.split("/").slice(0, -1);
  return parts.map((_, i) => `${parts.slice(0, i + 1).join("/")}/`);
}
