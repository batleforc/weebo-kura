/** One vault as the reader sees it: its listed entries, loaded page by page. */

import { withDevice } from "@/api/client";
import { getEntries, listVaults } from "@/api/generated/sdk.gen";
import type { EntryState, Vault } from "@/api/generated/types.gen";

/** Pages of 1,000 at most; a vault larger than this many is cut (T9). */
export const MAX_PAGES = 50;

export async function loadVault(id: string): Promise<{
  vault: Vault | undefined;
  /** The notes (files). */
  entries: EntryState[];
  /** The folder entries (RFC 0007): every folder, empty ones included. */
  folders: EntryState[];
  /** More entries follow than the reader lists: the tree is cut (T9). */
  cut: boolean;
}> {
  const vaults = await withDevice(
    async (headers) => (await listVaults({ headers, throwOnError: true })).data,
  );
  const vault = vaults.find((v) => v.id === id);
  const all: EntryState[] = [];
  let after: string | undefined;
  const split = (cut: boolean) => ({
    vault,
    entries: all.filter((e) => !e.folder),
    folders: all.filter((e) => e.folder),
    cut,
  });
  for (let page = 0; page < MAX_PAGES; page += 1) {
    const { data } = await withDevice((headers) =>
      getEntries({
        headers,
        path: { v: id },
        query: after ? { after, folders: true } : { folders: true },
        throwOnError: true,
      }),
    );
    all.push(...data.items);
    if (!data.next) return split(false);
    after = data.next;
  }
  return split(true);
}
