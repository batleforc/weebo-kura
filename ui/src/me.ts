/** `GET /v1/me`, once per sign-in: who the shell is for (RFC 0003 §4.4). */

import { defineStore } from "pinia";
import { ref } from "vue";

import { me as fetchMe } from "@/api/generated/sdk.gen";
import type { Me } from "@/api/generated/types.gen";

export const useMe = defineStore("me", () => {
  const me = ref<Me | null>(null);
  // One request in flight, shared: the shell and the page mount together,
  // and each asking on its own ran into the server's limit (critique 2026-11-18).
  let pending: Promise<Me> | null = null;
  async function load(): Promise<Me> {
    if (me.value) return me.value;
    pending ??= fetchMe({ throwOnError: true })
      .then(({ data }) => {
        me.value = data;
        return data;
      })
      .finally(() => {
        pending = null;
      });
    return pending;
  }
  return { me, load };
});
