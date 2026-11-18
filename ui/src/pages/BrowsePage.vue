<script setup lang="ts">
/** Browse and read (RFC 0005 §4.5): the shared folders, and one note. */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { withDevice } from "@/api/client";
import { getRevisions } from "@/api/generated/sdk.gen";
import type {
  EntryState,
  RevisionInfo,
  Vault,
} from "@/api/generated/types.gen";
import { retryable } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { fileBytes } from "@/reader/content";
import { type Watch, watchVault } from "@/reader/events";
import FolderTree from "@/reader/FolderTree.vue";
import NoteFrame from "@/reader/NoteFrame.vue";
import {
  ENTRY_LINK,
  frameDocument,
  MAX_RENDER,
  renderNote,
} from "@/reader/render";
import { useAutoRetry } from "@/reader/retry";
import { day } from "@/reader/time";
import { buildTree, noteName } from "@/reader/tree";
import { loadVault, MAX_PAGES } from "@/reader/vault";

const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const vaultId = computed(() => String(route.params.v));
const entryId = computed(() =>
  route.params.entry ? String(route.params.entry) : undefined,
);

const vault = ref<Vault | undefined>();
const entries = ref<EntryState[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const canRetry = ref(true);
const note = ref<EntryState | undefined>();
const doc = ref<string | null>(null);
const download = ref<string | null>(null);
const missing = ref(false);
const cut = ref(false);
const edited = ref<RevisionInfo | null>(null);
const tree = computed(() => buildTree(entries.value));
const failure = useAutoRetry(() => loadAll().then(open));

async function loadAll(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    const loaded = await loadVault(vaultId.value);
    vault.value = loaded.vault;
    entries.value = loaded.entries;
    cut.value = loaded.cut;
  } catch (e) {
    error.value = failure(e);
    canRetry.value = retryable(e);
  } finally {
    loading.value = false;
  }
}

/** A `change` notice: the tree again, and the note if it moved on. */
async function refresh(): Promise<void> {
  try {
    const before = note.value?.revision;
    const loaded = await loadVault(vaultId.value);
    vault.value = loaded.vault;
    entries.value = loaded.entries;
    cut.value = loaded.cut;
    const now = entries.value.find(
      (e) => e.entry === entryId.value && !e.deleted,
    );
    if (now?.revision !== before) await open();
  } catch {
    // The next notice, or a reload, tries again.
  }
}

/** "Last edited … by …": the newest revision, when the server gives it. */
async function lastEdit(v: string, entry: string): Promise<void> {
  try {
    const { data } = await withDevice((headers) =>
      getRevisions({ headers, path: { v, e: entry }, throwOnError: true }),
    );
    if (entry === entryId.value) edited.value = data.items[0] ?? null;
  } catch {
    // A line less under the note, nothing worse.
  }
}

async function open(): Promise<void> {
  doc.value = null;
  edited.value = null;
  missing.value = false;
  if (download.value) URL.revokeObjectURL(download.value);
  download.value = null;
  note.value = entries.value.find(
    (e) => e.entry === entryId.value && !e.deleted,
  );
  if (!entryId.value) return;
  if (!note.value?.blob) {
    missing.value = true;
    return;
  }
  const bytes = await fileBytes(vaultId.value, note.value.blob);
  if (!bytes) {
    missing.value = true;
    return;
  }
  // Rendered: Markdown up to 1 MiB; anything else is a download.
  if (!/\.md$/i.test(note.value.path) || bytes.length > MAX_RENDER) {
    const copy = new Uint8Array(bytes);
    download.value = URL.createObjectURL(new Blob([copy]));
    return;
  }
  const rendered = renderNote(new TextDecoder().decode(bytes), entries.value);
  doc.value = await frameDocument(rendered, (e) =>
    e.blob ? fileBytes(vaultId.value, e.blob) : Promise.resolve(null),
  );
  void lastEdit(vaultId.value, note.value.entry);
}

/** A link from the frame: an entry the caller was listed, or an external page. */
function follow(href: string): void {
  if (href.startsWith(ENTRY_LINK)) {
    const id = href.slice(ENTRY_LINK.length);
    if (entries.value.some((e) => e.entry === id))
      void router.push(`/vaults/${vaultId.value}/notes/${id}`);
  } else if (/^https?:\/\//i.test(href)) {
    window.open(href, "_blank", "noopener,noreferrer");
  }
}

// One socket for the open vault, closed on leaving it (RFC 0005 §7.5); a
// burst of notices becomes one refresh.
let socket: Watch | null = null;
let pending: ReturnType<typeof setTimeout> | undefined;
const NOTICE_DEBOUNCE_MS = 1000;
function noticed(): void {
  clearTimeout(pending);
  pending = setTimeout(() => void refresh(), NOTICE_DEBOUNCE_MS);
}
onBeforeUnmount(() => {
  clearTimeout(pending);
  socket?.close();
});

watch(
  vaultId,
  async (v) => {
    socket?.close();
    socket = watchVault(v, noticed);
    await loadAll();
    await open();
  },
  { immediate: true },
);
watch(entryId, () => void open());
</script>

<template>
  <section>
    <h2 class="sr-only">{{ note ? noteName(note.path) : t("reader.folders") }}</h2>
    <div class="mb-3 flex flex-wrap gap-2">
      <span class="min-w-0 break-words text-ink-dim">{{ note?.path ?? "" }}</span>
      <RouterLink :to="`/vaults/${vaultId}/search`" class="ml-auto underline">{{ t("reader.search") }}</RouterLink>
    </div>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="entries.length === 0"
      :empty-message="t('reader.emptyVault')"
      :title="t('reader.failed')"
      :can-retry="canRetry"
      @retry="loadAll"
    >
      <p v-if="cut" role="status" class="mb-3 border border-rule p-2" data-test="cut">
        {{ t("reader.cut", { n: (MAX_PAGES * 1000).toLocaleString(locale) }) }}
      </p>
      <div class="grid gap-4 md:grid-cols-[16rem_1fr]">
        <!-- On a phone, an open note comes first and its folders after it. -->
        <nav :aria-label="t('reader.folders')" class="md:border-r md:border-rule-soft" :class="{ 'order-2 md:order-none': entryId }" data-test="tree">
          <FolderTree :folder="tree" :vault="vaultId" :current="entryId" />
        </nav>
        <article>
          <p v-if="!entryId" class="text-ink-dim">{{ t("reader.pick") }}</p>
          <div v-else-if="missing" role="alert" data-test="not-found">
            <p>{{ t("reader.notFound") }}</p>
            <RouterLink :to="`/vaults/${vaultId}`" class="underline">{{ t("reader.back", { name: vault?.name ?? "" }) }}</RouterLink>
          </div>
          <p v-else-if="download && note">
            <a :href="download" :download="note.path.split('/').pop()" class="underline">{{ t("reader.download", { name: note.path }) }}</a>
          </p>
          <template v-else-if="doc && note">
            <NoteFrame :document="doc" :title="noteName(note.path)" @link="follow" />
            <p class="mt-2 text-ink-dim" data-test="last-edit">
              <template v-if="edited">
                {{ edited.author
                  ? t("reader.lastEditedBy", { date: day(edited.createdAt, locale), who: edited.author.kind === "key" ? t("reader.byKey", { name: edited.author.name }) : edited.author.name })
                  : t("reader.lastEdited", { date: day(edited.createdAt, locale) }) }}
                <span aria-hidden="true">·</span>
              </template>
              <RouterLink :to="`/vaults/${vaultId}/notes/${note.entry}/history`" class="underline">{{ t("reader.history") }}</RouterLink>
            </p>
          </template>
          <p v-else role="status">{{ t("state.loading") }}</p>
        </article>
      </div>
    </AsyncState>
  </section>
</template>
