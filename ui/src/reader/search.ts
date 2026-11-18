/**
 * Full-text search in one vault (RFC 0005 §4.5), shared by the reader's side
 * panel and the search page: as the user types, after a pause (§7.5), only the
 * latest question's answer kept. Snippets are text, never HTML.
 */
import { onBeforeUnmount, type Ref, ref, watch } from "vue";

import { withDevice } from "@/api/client";
import { search } from "@/api/generated/sdk.gen";
import type { SearchHit } from "@/api/generated/types.gen";
import { messageOf, problemType } from "@/api/problem";

export const SEARCH_DEBOUNCE_MS = 400;

export function useSearch(vault: Ref<string>) {
  const query = ref("");
  const asked = ref("");
  const hits = ref<SearchHit[] | null>(null);
  const disabled = ref(false);
  const error = ref<string | null>(null);
  let pending: ReturnType<typeof setTimeout> | undefined;
  let asking = 0;

  watch(query, (q) => {
    clearTimeout(pending);
    if (q.trim().length < 2) {
      // A cleared field forgets the last answer, and any late one.
      asking++;
      hits.value = null;
      asked.value = "";
      return;
    }
    pending = setTimeout(() => void run(), SEARCH_DEBOUNCE_MS);
  });
  onBeforeUnmount(() => clearTimeout(pending));

  async function run(): Promise<void> {
    clearTimeout(pending);
    const q = query.value.trim();
    if (q.length < 2 || disabled.value) return;
    error.value = null;
    const mine = ++asking;
    try {
      const { data } = await withDevice((headers) =>
        search({
          headers,
          path: { v: vault.value },
          body: { query: q },
          throwOnError: true,
        }),
      );
      if (mine !== asking) return;
      hits.value = data.results;
      asked.value = q;
    } catch (e) {
      if (mine !== asking) return;
      if (problemType(e) === "search_disabled") disabled.value = true;
      else error.value = messageOf(e);
    }
  }

  return { query, asked, hits, disabled, error, run };
}
