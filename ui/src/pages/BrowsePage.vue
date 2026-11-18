<script setup lang="ts">
/**
 * Browse and read (RFC 0005 §4.5): the shared folders and the finder on the
 * side, one note in a reading column, its history in a drawer beside it.
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
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
import Chevron from "@/components/ui/Chevron.vue";
import { fileBytes } from "@/reader/content";
import { type Watch, watchVault } from "@/reader/events";
import FolderTree from "@/reader/FolderTree.vue";
import HistoryPanel, { type Shown } from "@/reader/HistoryPanel.vue";
import NoteFrame from "@/reader/NoteFrame.vue";
import {
  ENTRY_LINK,
  frameDocument,
  MAX_RENDER,
  renderNote,
} from "@/reader/render";
import { useAutoRetry } from "@/reader/retry";
import { useSearch } from "@/reader/search";
import { day } from "@/reader/time";
import { buildTree, foldersOf, noteName, notesInOrder } from "@/reader/tree";
import { loadVault, MAX_PAGES } from "@/reader/vault";

const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const vaultId = computed(() => String(route.params.v));
const entryId = computed(() =>
  route.params.entry ? String(route.params.entry) : undefined,
);
const historyOpen = computed(
  () => !!entryId.value && route.path.endsWith("/history"),
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
const shown = ref<Shown | null>(null);
const copied = ref(false);
const folders = ref<EntryState[]>([]);
const tree = computed(() => buildTree(entries.value, folders.value));
const order = computed(() => notesInOrder(tree.value));
const failure = useAutoRetry(() => loadAll().then(open));

// ── The folders a reader opened, remembered per vault in this browser only. ──
const opened = ref(new Set<string>());
const foldersKey = () => `kura:folders:${vaultId.value}`;
function loadOpened(): void {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(foldersKey()) ?? "[]");
    opened.value = new Set(
      Array.isArray(raw) ? raw.filter((p) => typeof p === "string") : [],
    );
  } catch {
    opened.value = new Set();
  }
}
function setOpened(paths: string[], open: boolean): void {
  const next = new Set(opened.value);
  for (const p of paths) {
    if (open) next.add(p);
    else next.delete(p);
  }
  opened.value = next;
  try {
    localStorage.setItem(foldersKey(), JSON.stringify([...next]));
  } catch {
    // Private mode: the folders stay as they are for this page only.
  }
}

// ── The finder: names as you type, the text after a pause. ──
const finder = ref<HTMLInputElement | null>(null);
const treeEl = ref<HTMLElement | null>(null);
const {
  query,
  asked,
  hits,
  disabled,
  error: searchError,
  run,
} = useSearch(vaultId);
const asking = computed(() => query.value.trim());
/** Notes whose name or folder holds the words typed, the first 50. */
const byName = computed(() => {
  const q = asking.value.toLowerCase();
  if (!q) return [];
  return order.value
    .filter((n) => n.path.toLowerCase().includes(q))
    .slice(0, 50);
});
function folderOf(path: string): string {
  return path.split("/").slice(0, -1).join("/");
}
/** Enter: the first name, else the first text hit, else search now. */
function pickFirst(): void {
  const first = byName.value[0]?.entry ?? hits.value?.[0]?.entry;
  if (!first) {
    void run();
    return;
  }
  query.value = "";
  void router.push(`/vaults/${vaultId.value}/notes/${first}`);
}

async function loadAll(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    const loaded = await loadVault(vaultId.value);
    vault.value = loaded.vault;
    entries.value = loaded.entries;
    folders.value = loaded.folders;
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
    folders.value = loaded.folders;
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

/**
 * The note of the address. The one on screen stays until the next is ready,
 * so moving from note to note never blanks the page; only the latest asked wins.
 */
let opening = 0;
async function open(): Promise<void> {
  const mine = ++opening;
  shown.value = null;
  const id = entryId.value;
  const found = entries.value.find((e) => e.entry === id && !e.deleted);
  let nextDoc: string | null = null;
  let nextDownload: string | null = null;
  let nextMissing = false;
  if (id) {
    const bytes = found?.blob
      ? await fileBytes(vaultId.value, found.blob)
      : null;
    if (!found || !bytes) {
      nextMissing = true;
    } else if (!/\.md$/i.test(found.path) || bytes.length > MAX_RENDER) {
      // Rendered: Markdown up to 1 MiB; anything else is a download.
      nextDownload = URL.createObjectURL(new Blob([new Uint8Array(bytes)]));
    } else {
      const rendered = renderNote(
        new TextDecoder().decode(bytes),
        entries.value,
      );
      nextDoc = await frameDocument(rendered, (e) =>
        e.blob ? fileBytes(vaultId.value, e.blob) : Promise.resolve(null),
      );
    }
  }
  if (mine !== opening) {
    if (nextDownload) URL.revokeObjectURL(nextDownload);
    return;
  }
  if (download.value) URL.revokeObjectURL(download.value);
  if (note.value?.entry !== found?.entry) edited.value = null;
  note.value = found;
  doc.value = nextDoc;
  download.value = nextDownload;
  missing.value = nextMissing;
  if (found && nextDoc) void lastEdit(vaultId.value, found.entry);
  if (found) {
    setOpened(foldersOf(found.path), true);
    await nextTick();
    treeEl.value
      ?.querySelector(`[data-entry="${found.entry}"]`)
      ?.scrollIntoView?.({ block: "nearest" });
  }
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

// ── Around the note: where it sits, its neighbours, its link, its history. ──
const crumbs = computed(() => {
  const parts = note.value?.path.split("/").slice(0, -1) ?? [];
  return parts.map((name, i) => ({
    name,
    path: `${parts.slice(0, i + 1).join("/")}/`,
  }));
});
const place = computed(() =>
  order.value.findIndex((n) => n.entry === entryId.value),
);
const prev = computed(() =>
  place.value > 0 ? order.value[place.value - 1] : undefined,
);
const next = computed(() =>
  place.value >= 0 ? order.value[place.value + 1] : undefined,
);
const keyHints = computed(() => [
  { keys: "/", label: t("reader.keyFind") },
  { keys: "[ ]", label: t("reader.keyTurn") },
  { keys: "Esc", label: t("reader.keyClose") },
]);
const noteLink = (e: EntryState) => `/vaults/${vaultId.value}/notes/${e.entry}`;

/** A folder of the path, opened and focused in the tree. */
async function reveal(path: string): Promise<void> {
  query.value = "";
  // The folder itself and every one above it: the folders of a file inside it.
  setOpened(foldersOf(`${path}x`), true);
  await nextTick();
  const summary = treeEl.value?.querySelector<HTMLElement>(
    `[data-folder="${CSS.escape(path)}"] > summary`,
  );
  summary?.scrollIntoView?.({ block: "nearest" });
  summary?.focus();
}

let copiedTimer: ReturnType<typeof setTimeout> | undefined;
async function copyLink(): Promise<void> {
  try {
    await navigator.clipboard.writeText(window.location.href);
    copied.value = true;
    clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => (copied.value = false), 2000);
  } catch {
    // No clipboard (an insecure origin, a refusal): the address bar still has it.
  }
}

/** Focus follows the drawer: into it on opening, back to its toggle on closing. */
async function focusOn(selector: string): Promise<void> {
  await nextTick();
  document.querySelector<HTMLElement>(selector)?.focus();
}
watch(historyOpen, (open) =>
  focusOn(open ? "[data-test=history-close]" : "[data-test=history-toggle]"),
);
function closeHistory(): void {
  if (note.value) void router.push(noteLink(note.value));
}

// ── Keys: / or Ctrl+K to find, [ and ] to turn the page, Escape to close. ──
function typing(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
  );
}
function onKey(e: KeyboardEvent): void {
  if (e.defaultPrevented || e.altKey) return;
  const mod = e.ctrlKey || e.metaKey;
  if ((mod && e.key === "k") || (!mod && e.key === "/" && !typing(e.target))) {
    e.preventDefault();
    finder.value?.focus();
    finder.value?.select();
    return;
  }
  if (e.key === "Escape") {
    if (e.target === finder.value && query.value) {
      query.value = "";
    } else if (historyOpen.value) {
      closeHistory();
    }
    return;
  }
  if (mod || typing(e.target)) return;
  const to =
    e.key === "[" ? prev.value : e.key === "]" ? next.value : undefined;
  if (to) {
    e.preventDefault();
    void router.push(noteLink(to));
  }
}
onMounted(() => window.addEventListener("keydown", onKey));

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
  window.removeEventListener("keydown", onKey);
  clearTimeout(pending);
  clearTimeout(copiedTimer);
  socket?.close();
  if (download.value) URL.revokeObjectURL(download.value);
});

watch(
  vaultId,
  async (v) => {
    socket?.close();
    socket = watchVault(v, noticed);
    loadOpened();
    await loadAll();
    await open();
  },
  { immediate: true },
);
watch(entryId, () => void open());
</script>

<template>
  <section :class="{ 'lg:pr-80': historyOpen }">
    <h2 class="sr-only">{{ note ? noteName(note.path) : t("reader.folders") }}</h2>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="entries.length === 0"
      :empty-message="t('reader.emptyVault')"
      :title="t('reader.failed')"
      :can-retry="canRetry"
      @retry="loadAll"
    >
      <div class="grid gap-x-10 gap-y-8 md:grid-cols-[15rem_minmax(0,1fr)]">
        <!-- On a phone, an open note comes first and its folders after it. -->
        <aside class="flex flex-col gap-3 md:sticky md:top-4 md:max-h-[calc(100vh-2rem)] md:self-start" :class="{ 'order-2 md:order-none': entryId }">
          <form role="search" class="relative" @submit.prevent="pickFirst">
            <label for="finder" class="sr-only">{{ t("reader.find") }}</label>
            <input
              id="finder"
              ref="finder"
              v-model="query"
              type="search"
              autocomplete="off"
              class="w-full border border-rule bg-ground py-1.5 pr-8 pl-2"
              :placeholder="t('reader.findHint')"
              aria-keyshortcuts="/ Control+K"
              aria-controls="finder-results"
              data-test="finder"
            />
            <kbd v-if="!query" aria-hidden="true" class="meta pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 border border-rule-soft px-1 text-ink-dim">/</kbd>
          </form>
          <p v-if="cut" role="status" class="meta border border-rule-soft p-2 text-ink-dim" data-test="cut">
            {{ t("reader.cut", { n: (MAX_PAGES * 1000).toLocaleString(locale) }) }}
          </p>
          <div v-if="asking" id="finder-results" class="-ml-2 min-h-0 overflow-y-auto" data-test="finder-results">
            <h3 class="label px-2 pb-1">{{ t("reader.byName") }}</h3>
            <ul v-if="byName.length">
              <li v-for="n in byName" :key="n.entry">
                <RouterLink :to="noteLink(n)" class="block px-2 py-1 no-underline hover:bg-raised" @click="query = ''">
                  <span class="block break-words">{{ noteName(n.path) }}</span>
                  <span v-if="folderOf(n.path)" class="meta block break-words text-ink-dim">{{ folderOf(n.path) }}</span>
                </RouterLink>
              </li>
            </ul>
            <p v-else class="px-2 text-ink-dim">{{ t("reader.noName") }}</p>
            <template v-if="!disabled && asking.length >= 2">
              <h3 class="label mt-4 px-2 pb-1">{{ t("reader.inText") }}</h3>
              <p v-if="searchError" role="alert" class="px-2">{{ searchError }}</p>
              <p role="status" class="meta px-2 text-ink-dim" data-test="count">
                <template v-if="hits && hits.length === 0 && asked === asking">{{ t("reader.noHit", { query: asked }) }}</template>
                <template v-else-if="!hits || asked !== asking">{{ t("reader.searching") }}</template>
              </p>
              <ul v-if="hits && hits.length > 0">
                <li v-for="h in hits" :key="h.entry">
                  <RouterLink :to="`/vaults/${vaultId}/notes/${h.entry}`" class="block px-2 py-1 no-underline hover:bg-raised" @click="query = ''">
                    <span class="block break-words">{{ noteName(h.path) }}</span>
                    <span v-if="h.snippet" class="meta line-clamp-2 block text-ink-dim">{{ h.snippet }}</span>
                  </RouterLink>
                </li>
              </ul>
            </template>
          </div>
          <nav v-else ref="treeEl" :aria-label="t('reader.folders')" class="-ml-2 min-h-0 overflow-y-auto" data-test="tree">
            <FolderTree :folder="tree" :vault="vaultId" :current="entryId" :open="opened" @toggle="(p, o) => setOpened([p], o)" />
          </nav>
          <dl class="meta hidden grid-cols-[auto_1fr] items-baseline gap-x-2 gap-y-1 text-ink-dim md:grid" :aria-label="t('reader.keys')">
            <template v-for="k in keyHints" :key="k.keys">
              <dt class="flex gap-1"><kbd v-for="key in k.keys.split(' ')" :key="key" class="border border-rule-soft px-1 font-[inherit]">{{ key }}</kbd></dt>
              <dd>{{ k.label }}</dd>
            </template>
          </dl>
        </aside>

        <article class="min-w-0 max-w-[46rem]" :aria-busy="!!entryId && !doc && !download && !missing">
          <p v-if="!entryId" class="text-ink-dim">{{ t("reader.pick") }}</p>
          <div v-else-if="missing" role="alert" data-test="not-found">
            <p>{{ t("reader.notFound") }}</p>
            <RouterLink :to="`/vaults/${vaultId}`" class="underline">{{ t("reader.back", { name: vault?.name ?? "" }) }}</RouterLink>
          </div>
          <template v-else-if="note && (doc || download)">
            <header class="mb-6 flex flex-wrap items-center gap-x-4 gap-y-2 border-b border-rule-soft pb-3" data-test="note-bar">
              <nav :aria-label="t('reader.breadcrumb')" class="flex min-w-0 flex-wrap items-center gap-x-1.5 text-ink-dim">
                <template v-for="c in crumbs" :key="c.path">
                  <button type="button" class="break-all hover:text-ink hover:underline" :title="t('reader.revealFolder', { name: c.name })" @click="reveal(c.path)">{{ c.name }}</button>
                  <span aria-hidden="true">/</span>
                </template>
                <span class="break-all font-bold text-ink" aria-current="page">{{ noteName(note.path) }}</span>
              </nav>
              <div class="ml-auto flex items-center gap-1 text-ink-dim">
                <RouterLink v-if="prev" :to="noteLink(prev)" class="inline-grid size-8 place-items-center no-underline hover:text-ink pointer-coarse:size-11" :aria-label="t('reader.prev', { name: noteName(prev.path) })" :title="t('reader.prev', { name: noteName(prev.path) })" aria-keyshortcuts="[" data-test="prev"><Chevron dir="left" /></RouterLink>
                <RouterLink v-if="next" :to="noteLink(next)" class="inline-grid size-8 place-items-center no-underline hover:text-ink pointer-coarse:size-11" :aria-label="t('reader.next', { name: noteName(next.path) })" :title="t('reader.next', { name: noteName(next.path) })" aria-keyshortcuts="]" data-test="next"><Chevron dir="right" /></RouterLink>
                <button type="button" class="px-2 py-1 hover:text-ink" data-test="copy-link" @click="copyLink">{{ copied ? t("reader.copied") : t("reader.copyLink") }}</button>
                <RouterLink
                  :to="historyOpen ? noteLink(note) : `${noteLink(note)}/history`"
                  class="px-2 py-1 no-underline hover:text-ink"
                  :class="{ 'bg-ink font-bold !text-ground': historyOpen }"
                  :aria-expanded="historyOpen"
                  aria-controls="history-drawer"
                  data-test="history-toggle"
                >{{ t("reader.history") }}</RouterLink>
              </div>
              <span class="sr-only" role="status">{{ copied ? t("reader.copied") : "" }}</span>
            </header>
            <div v-if="shown" role="status" class="mb-4 flex flex-wrap items-baseline gap-x-3 border border-copper px-3 py-2" data-test="past">
              <span>{{ t("reader.pastRevision", { date: day(shown.createdAt, locale) }) }}</span>
              <button type="button" class="underline" @click="shown = null">{{ t("reader.backToCurrent") }}</button>
            </div>
            <p v-if="download && !shown">
              <a :href="download" :download="note.path.split('/').pop()" class="underline">{{ t("reader.download", { name: note.path }) }}</a>
            </p>
            <NoteFrame v-else :document="shown?.doc ?? doc ?? ''" :title="noteName(note.path)" @link="follow" />
            <p v-if="doc && !shown" class="meta mt-8 border-t border-dashed border-rule-soft pt-3 text-ink-dim" data-test="last-edit">
              <template v-if="edited">
                {{ edited.author
                  ? t("reader.lastEditedBy", { date: day(edited.createdAt, locale), who: edited.author.kind === "key" ? t("reader.byKey", { name: edited.author.name }) : edited.author.name })
                  : t("reader.lastEdited", { date: day(edited.createdAt, locale) }) }}
              </template>
            </p>
          </template>
          <p v-else role="status" class="text-ink-dim">{{ t("state.loading") }}</p>
        </article>
      </div>

      <aside
        v-if="historyOpen && note"
        id="history-drawer"
        class="fixed inset-y-0 right-0 z-20 w-full overflow-y-auto border-l border-rule bg-ground p-4 sm:w-80"
        :aria-label="t('reader.history')"
      >
        <HistoryPanel :vault="vaultId" :entry="note.entry" :entries="entries" :shown="shown?.revision" @show="(s) => (shown = s)" @close="closeHistory" />
      </aside>
    </AsyncState>
  </section>
</template>
