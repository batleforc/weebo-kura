import { ref } from "vue";

import { detailOf, messageOf, problemType } from "@/api/problem";

/**
 * One load of a page: loading, error, and whether the server refused the
 * role, with the server's `insufficient_role` detail when it gave one (§4.5).
 */
export function useLoad<T>(fetch: () => Promise<T>, initial: T) {
  const data = ref(initial);
  const loading = ref(true);
  const error = ref<string | null>(null);
  const forbidden = ref(false);
  const forbiddenDetail = ref<string | null>(null);
  async function load(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      data.value = (await fetch()) as typeof data.value;
    } catch (e) {
      if (problemType(e) === "insufficient_role") {
        forbidden.value = true;
        forbiddenDetail.value = detailOf(e) ?? null;
      } else error.value = messageOf(e);
    } finally {
      loading.value = false;
    }
  }
  return { data, loading, error, forbidden, forbiddenDetail, load };
}
