/** The caller's vaults, once per sign-in, shared by the sidebar, the home page and the vault pages. */

import { defineStore } from "pinia";
import { ref } from "vue";

import { withDevice } from "@/api/client";
import { listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";

export const useVaults = defineStore("vaults", () => {
  const list = ref<Vault[]>([]);
  const loaded = ref(false);
  let pending: Promise<Vault[]> | null = null;
  /** The list, fetched once; `force` fetches again (after a vault is created or deleted). */
  async function load(force = false): Promise<Vault[]> {
    if (loaded.value && !force) return list.value;
    pending ??= withDevice(
      async (headers) =>
        (await listVaults({ headers, throwOnError: true })).data,
    ).finally(() => {
      pending = null;
    });
    list.value = await pending;
    loaded.value = true;
    return list.value;
  }
  return { list, loaded, load };
});
