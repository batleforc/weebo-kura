<script setup lang="ts">
/** A note's revisions (RFC 0005 §4.5); viewing one is the only action. */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { getRevisions } from "@/api/generated/sdk.gen";
import type { RevisionInfo } from "@/api/generated/types.gen";
import { problemType, retryable } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import BreadCrumbs from "@/components/ui/BreadCrumbs.vue";
import { fileBytes } from "@/reader/content";
import NoteFrame from "@/reader/NoteFrame.vue";
import { frameDocument, MAX_RENDER, renderNote } from "@/reader/render";
import { useAutoRetry } from "@/reader/retry";
import { ago, when } from "@/reader/time";
import { loadVault } from "@/reader/vault";

const { t, locale } = useI18n();
const route = useRoute();
const vaultId = computed(() => String(route.params.v));
const entryId = computed(() => String(route.params.entry));
const revisions = ref<RevisionInfo[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const canRetry = ref(true);
const missing = ref(false);
const shown = ref<{ revision: string; doc: string } | null>(null);
const failure = useAutoRetry(() => load());

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    const { data } = await withDevice((headers) =>
      getRevisions({
        headers,
        path: { v: vaultId.value, e: entryId.value },
        throwOnError: true,
      }),
    );
    revisions.value = data.items;
  } catch (e) {
    if (problemType(e) === "not_found") missing.value = true;
    else {
      error.value = failure(e);
      canRetry.value = retryable(e);
    }
  } finally {
    loading.value = false;
  }
}

async function view(r: RevisionInfo): Promise<void> {
  if (!r.blob || r.size > MAX_RENDER) return;
  const bytes = await fileBytes(vaultId.value, r.blob);
  if (!bytes) return;
  const { entries } = await loadVault(vaultId.value);
  const rendered = renderNote(new TextDecoder().decode(bytes), entries);
  shown.value = {
    revision: r.revision,
    doc: await frameDocument(rendered, async () => null),
  };
}

onMounted(load);
</script>

<template>
  <section>
    <BreadCrumbs :items="[{ label: revisions[0]?.path ?? t('reader.backToNote'), to: `/vaults/${vaultId}/notes/${entryId}` }, { label: t('reader.history') }]" />
    <div v-if="missing" role="alert" data-test="not-found">{{ t("reader.notFound") }}</div>
    <AsyncState
      v-else
      :loading="loading"
      :error="error"
      :empty="revisions.length === 0"
      :empty-message="t('reader.noRevision')"
      :title="t('reader.failed')"
      :can-retry="canRetry"
      @retry="load"
    >
      <h2 class="sr-only">{{ t("reader.history") }}</h2>
      <ul>
        <li v-for="(r, i) in revisions" :key="r.revision" class="border-t border-rule-soft py-1">
          <time :datetime="r.createdAt" :title="when(r.createdAt, locale)">{{ ago(r.createdAt, locale) }}</time>
          <span class="ml-2">{{ r.path }}</span>
          <span v-if="r.author" class="ml-2 text-ink-dim">{{ r.author.kind === "key" ? t("reader.byKey", { name: r.author.name }) : r.author.name }}</span>
          <span v-if="i === 0" class="ml-2 text-ink-dim">{{ t("reader.current") }}</span>
          <span v-else-if="r.deleted" class="ml-2 text-ink-dim">{{ t("reader.deleted") }}</span>
          <button v-else type="button" class="ml-2 border border-rule px-2" @click="view(r)">{{ t("reader.view") }}</button>
        </li>
      </ul>
      <NoteFrame v-if="shown" class="mt-4" :document="shown.doc" :title="t('reader.history')" />
    </AsyncState>
  </section>
</template>
