/** One vault as the reader sees it: its listed entries, loaded page by page. */

import { withDevice } from "@/api/client";
import { getEntries, listVaults } from "@/api/generated/sdk.gen";
import type { EntryState, Vault } from "@/api/generated/types.gen";

/** Pages of 1,000 at most; a vault larger than this many is cut (T9). */
const MAX_PAGES = 50;

export async function loadVault(
  id: string,
): Promise<{ vault: Vault | undefined; entries: EntryState[] }> {
  const vaults = await withDevice(
    async (headers) => (await listVaults({ headers, throwOnError: true })).data,
  );
  const vault = vaults.find((v) => v.id === id);
  const entries: EntryState[] = [];
  let after: string | undefined;
  for (let page = 0; page < MAX_PAGES; page += 1) {
    const { data } = await withDevice((headers) =>
      getEntries({
        headers,
        path: { v: id },
        query: after ? { after } : {},
        throwOnError: true,
      }),
    );
    entries.push(...data.items);
    if (!data.next) break;
    after = data.next;
  }
  return { vault, entries };
}
