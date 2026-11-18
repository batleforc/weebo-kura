/** `GET /v1/me`, once per sign-in: who the shell is for (RFC 0003 §4.4). */

import { defineStore } from "pinia";
import { ref } from "vue";

import { me as fetchMe } from "@/api/generated/sdk.gen";
import type { Me } from "@/api/generated/types.gen";

export const useMe = defineStore("me", () => {
  const me = ref<Me | null>(null);
  async function load(): Promise<Me> {
    if (!me.value) {
      const { data } = await fetchMe({ throwOnError: true });
      me.value = data;
    }
    return me.value;
  }
  return { me, load };
});
