<script setup lang="ts">
/** Browse and read (RFC 0005 §4.5): the shared folders, and one note. */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import type { EntryState, Vault } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { fileBytes } from "@/reader/content";
import FolderTree from "@/reader/FolderTree.vue";
import NoteFrame from "@/reader/NoteFrame.vue";
import {
  ENTRY_LINK,
  frameDocument,
  MAX_RENDER,
  renderNote,
} from "@/reader/render";
import { buildTree, noteName } from "@/reader/tree";
import { loadVault } from "@/reader/vault";

const { t } = useI18n();
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
const note = ref<EntryState | undefined>();
const doc = ref<string | null>(null);
const download = ref<string | null>(null);
const missing = ref(false);
const tree = computed(() => buildTree(entries.value));

async function loadAll(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    const loaded = await loadVault(vaultId.value);
    vault.value = loaded.vault;
    entries.value = loaded.entries;
  } catch (e) {
    error.value = messageOf(e);
  } finally {
    loading.value = false;
  }
}

async function open(): Promise<void> {
  doc.value = null;
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

watch(
  vaultId,
  async () => {
    await loadAll();
    await open();
  },
  { immediate: true },
);
watch(entryId, () => void open());
</script>

<template>
  <section>
    <nav :aria-label="t('reader.breadcrumb')" class="mb-3 flex flex-wrap gap-2">
      <RouterLink to="/vaults">{{ t("nav.vaults") }}</RouterLink>
      <span aria-hidden="true">▸</span>
      <RouterLink :to="`/vaults/${vaultId}`">{{ vault?.name ?? "…" }}</RouterLink>
      <template v-if="note">
        <span aria-hidden="true">▸</span><span>{{ note.path }}</span>
      </template>
      <RouterLink :to="`/vaults/${vaultId}/search`" class="ml-auto underline">{{ t("reader.search") }}</RouterLink>
    </nav>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="entries.length === 0"
      :empty-message="t('reader.emptyVault')"
      :title="t('reader.failed')"
      @retry="loadAll"
    >
      <div class="grid gap-4 md:grid-cols-[16rem_1fr]">
        <nav :aria-label="t('reader.folders')" class="border-r border-rule-soft" data-test="tree">
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
            <p class="mt-2 text-ink-dim">
              <RouterLink :to="`/vaults/${vaultId}/notes/${note.entry}/history`" class="underline">{{ t("reader.history") }}</RouterLink>
            </p>
          </template>
          <p v-else role="status">{{ t("state.loading") }}</p>
        </article>
      </div>
    </AsyncState>
  </section>
</template>
