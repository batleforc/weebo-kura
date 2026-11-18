import { onMounted, type Ref, ref } from "vue";

import { withDevice } from "@/api/client";
import { listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";

/** The vault a page acts on, for its name and tenant; `undefined` until known or when hidden. */
export function useVault(id: Ref<string>): Ref<Vault | undefined> {
  const vault = ref<Vault | undefined>();
  onMounted(async () => {
    const all = await withDevice(
      async (headers) =>
        (await listVaults({ headers, throwOnError: true })).data,
    ).catch(() => []);
    vault.value = all.find((x) => x.id === id.value);
  });
  return vault;
}
