<script setup lang="ts">
/** Search in one vault (RFC 0005 §4.5): snippets shown as text, never HTML. */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { search } from "@/api/generated/sdk.gen";
import type { SearchHit } from "@/api/generated/types.gen";
import { messageOf, problemType } from "@/api/problem";

const { t } = useI18n();
const route = useRoute();
const vaultId = computed(() => String(route.params.v));
const query = ref("");
const asked = ref("");
const hits = ref<SearchHit[] | null>(null);
const disabled = ref(false);
const error = ref<string | null>(null);

/** Search as the user types, after a pause (RFC 0005 §7.5); submit runs at once. */
const SEARCH_DEBOUNCE_MS = 400;
let pending: ReturnType<typeof setTimeout> | undefined;
let asking = 0;
watch(query, () => {
  clearTimeout(pending);
  pending = setTimeout(() => void run(), SEARCH_DEBOUNCE_MS);
});
onBeforeUnmount(() => clearTimeout(pending));

async function run(): Promise<void> {
  clearTimeout(pending);
  const q = query.value.trim();
  if (q.length < 2) return;
  error.value = null;
  // Only the latest question's answer is shown, whatever order they come in.
  const mine = ++asking;
  try {
    const { data } = await withDevice((headers) =>
      search({
        headers,
        path: { v: vaultId.value },
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
</script>

<template>
  <section>
    <h2 class="label mb-3">{{ t("reader.search") }}</h2>
    <p v-if="disabled" role="status" data-test="search-off">{{ t("reader.searchOff") }}</p>
    <template v-else>
      <form role="search" class="mb-4 flex gap-2" @submit.prevent="run">
        <label for="q" class="sr-only">{{ t("reader.search") }}</label>
        <input id="q" v-model="query" type="search" class="border border-rule bg-ground px-2" minlength="2" data-test="query" />
        <button type="submit" class="border border-rule px-3">{{ t("reader.search") }}</button>
      </form>
      <p v-if="error" role="alert">{{ error }}</p>
      <p role="status" data-test="count">
        <template v-if="hits && hits.length === 0"><span data-test="no-hit">{{ t("reader.noHit", { query: asked }) }}</span></template>
        <template v-else-if="hits">{{ t("reader.hits", { n: hits.length }) }}</template>
      </p>
      <ul v-if="hits && hits.length > 0" :aria-label="t('reader.hits', { n: hits.length })">
        <li v-for="h in hits" :key="h.entry" class="border-t border-rule-soft py-1">
          <RouterLink :to="`/vaults/${vaultId}/notes/${h.entry}`" class="underline">{{ h.path }}</RouterLink>
          <span v-if="h.snippet" class="ml-2 text-ink-dim">{{ h.snippet }}</span>
        </li>
      </ul>
    </template>
  </section>
</template>
