<script setup lang="ts">
/** Search in one vault (RFC 0005 §4.5): snippets shown as text, never HTML. */
import { computed, ref } from "vue";
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

async function run(): Promise<void> {
  const q = query.value.trim();
  if (q.length < 2) return;
  error.value = null;
  try {
    const { data } = await withDevice((headers) =>
      search({
        headers,
        path: { v: vaultId.value },
        body: { query: q },
        throwOnError: true,
      }),
    );
    hits.value = data.results;
    asked.value = q;
  } catch (e) {
    if (problemType(e) === "search_disabled") disabled.value = true;
    else error.value = messageOf(e);
  }
}
</script>

<template>
  <section>
    <p class="mb-3"><RouterLink :to="`/vaults/${vaultId}`" class="underline">{{ t("reader.backToVault") }}</RouterLink></p>
    <p v-if="disabled" role="status" data-test="search-off">{{ t("reader.searchOff") }}</p>
    <template v-else>
      <form role="search" class="mb-4 flex gap-2" @submit.prevent="run">
        <label for="q" class="sr-only">{{ t("reader.search") }}</label>
        <input id="q" v-model="query" type="search" class="border border-rule bg-ground px-2" minlength="2" data-test="query" />
        <button type="submit" class="border border-rule px-3">{{ t("reader.search") }}</button>
      </form>
      <p v-if="error" role="alert">{{ error }}</p>
      <template v-if="hits">
        <p v-if="hits.length === 0" data-test="no-hit">{{ t("reader.noHit", { query: asked }) }}</p>
        <template v-else>
          <p>{{ t("reader.hits", { n: hits.length }) }}</p>
          <ul>
            <li v-for="h in hits" :key="h.entry" class="border-t border-rule-soft py-1">
              <RouterLink :to="`/vaults/${vaultId}/notes/${h.entry}`" class="underline">{{ h.path }}</RouterLink>
              <span v-if="h.snippet" class="ml-2 text-ink-dim">{{ h.snippet }}</span>
            </li>
          </ul>
        </template>
      </template>
    </template>
  </section>
</template>
