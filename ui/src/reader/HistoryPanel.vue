<script setup lang="ts">
/**
 * A note's revisions (RFC 0005 §4.5), beside the note: viewing one is the only
 * action, and it shows in the note's own frame, not under the list.
 */
import { onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { getRevisions } from "@/api/generated/sdk.gen";
import type { EntryState, RevisionInfo } from "@/api/generated/types.gen";
import { problemType, retryable } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { fileBytes } from "@/reader/content";
import { frameDocument, MAX_RENDER, renderNote } from "@/reader/render";
import { useAutoRetry } from "@/reader/retry";
import { ago, when } from "@/reader/time";

/** A past revision, ready for the note's frame. */
export interface Shown {
  revision: string;
  createdAt: string;
  doc: string;
}

const props = defineProps<{
  vault: string;
  entry: string;
  /** The listed entries, for the links of a past revision. */
  entries: readonly EntryState[];
  /** The revision on screen, when it is not the current one. */
  shown?: string;
}>();
const emit = defineEmits<{ show: [Shown | null]; close: [] }>();
const { t, locale } = useI18n();
const revisions = ref<RevisionInfo[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const canRetry = ref(true);
const missing = ref(false);
const failure = useAutoRetry(() => load());

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  missing.value = false;
  try {
    const { data } = await withDevice((headers) =>
      getRevisions({
        headers,
        path: { v: props.vault, e: props.entry },
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

/** A past revision shows when it is Markdown the reader renders. */
const viewable = (r: RevisionInfo) =>
  !!r.blob && r.size <= MAX_RENDER && /\.md$/i.test(r.path);

async function view(r: RevisionInfo): Promise<void> {
  if (!r.blob || !viewable(r)) return;
  const bytes = await fileBytes(props.vault, r.blob);
  if (!bytes) return;
  const rendered = renderNote(new TextDecoder().decode(bytes), props.entries);
  emit("show", {
    revision: r.revision,
    createdAt: r.createdAt,
    doc: await frameDocument(rendered, async () => null),
  });
}

onMounted(load);
watch(
  () => props.entry,
  () => void load(),
);
</script>

<template>
  <section :aria-labelledby="`history-${entry}`" data-test="history">
    <div class="mb-3 flex items-baseline justify-between gap-2">
      <h2 :id="`history-${entry}`" class="label">{{ t("reader.history") }}</h2>
      <button type="button" class="text-ink-dim underline hover:text-ink" aria-keyshortcuts="Escape" data-test="history-close" @click="emit('close')">{{ t("reader.close") }}</button>
    </div>
    <p v-if="missing" role="alert" data-test="not-found">{{ t("reader.notFound") }}</p>
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
      <ol class="flex flex-col">
        <li v-for="(r, i) in revisions" :key="r.revision">
          <button
            v-if="!r.deleted && (i === 0 || viewable(r))"
            type="button"
            class="flex w-full flex-col items-start border-l px-3 py-2 text-left hover:bg-raised"
            :class="(shown ?? revisions[0]?.revision) === r.revision ? 'border-accent bg-raised font-bold text-accent' : 'border-transparent'"
            :aria-current="(shown ?? revisions[0]?.revision) === r.revision ? 'true' : undefined"
            @click="i === 0 ? emit('show', null) : view(r)"
          >
            <time :datetime="r.createdAt" :title="when(r.createdAt, locale)">{{ ago(r.createdAt, locale) }}</time>
            <span class="meta text-ink-dim">
              <template v-if="r.author">{{ r.author.kind === "key" ? t("reader.byKey", { name: r.author.name }) : r.author.name }}</template>
              <template v-if="i === 0"> · {{ t("reader.current") }}</template>
            </span>
            <span v-if="r.path !== revisions[0]?.path" class="meta break-all text-ink-dim">{{ r.path }}</span>
          </button>
          <div v-else class="flex flex-col border-l border-transparent px-3 py-2 text-ink-dim">
            <time :datetime="r.createdAt" :title="when(r.createdAt, locale)">{{ ago(r.createdAt, locale) }}</time>
            <span class="meta">{{ r.deleted ? t("reader.deleted") : t("reader.notShown") }}</span>
          </div>
        </li>
      </ol>
    </AsyncState>
  </section>
</template>
