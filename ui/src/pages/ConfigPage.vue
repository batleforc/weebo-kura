<script setup lang="ts">
/**
 * The caller's own synced Obsidian configuration of a vault (RFC 0008), per
 * profile: listed by category, plugins by name, and the text of a file that
 * cannot hold a secret. Nothing here writes, and nothing is applied: a
 * snippet is text, never a stylesheet (T5). The server withholds a plugin's
 * settings and code; this page never asks for them.
 */
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { getEntries, getRevisions } from "@/api/generated/sdk.gen";
import type { EntryState, RevisionInfo } from "@/api/generated/types.gen";
import { messageOf, retryable } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { fileBytes } from "@/reader/content";
import { day } from "@/reader/time";
import {
  CATEGORIES,
  type Category,
  eachLimited,
  formatSize,
  group,
  hiddenReason,
  isHidden,
  MAX_PLUGINS,
  MAX_SHOWN,
  type Manifest,
  manifestProblem,
  readManifest,
  shortName,
} from "@/synced";
import { useVaults } from "@/vaults";

const { t, locale } = useI18n();
const route = useRoute();
const vaultStore = useVaults();
const v = computed(() => String(route.params.v));
const vault = computed(() => vaultStore.list.find((x) => x.id === v.value));
const profile = computed(() =>
  route.query.profile === "mobile" ? "mobile" : "desktop",
);
const openPath = computed(() =>
  typeof route.query.file === "string" ? route.query.file : null,
);
const applies = computed(
  () =>
    !!vault.value && vault.value.configSync.length > 0 && !vault.value.paths,
);

const entries = ref<EntryState[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const canRetry = ref(true);
const manifests = ref(new Map<string, Manifest | "unreadable">());
const groups = computed(() => group(entries.value));
const anyHiddenSettings = computed(() =>
  entries.value.some((e) => isHidden(e) && hiddenReason(e.path) === "settings"),
);
const synced = computed(() =>
  CATEGORIES.filter((c) => vault.value?.configSync.includes(c)),
);
const notSynced = computed(() =>
  CATEGORIES.filter((c) => !vault.value?.configSync.includes(c)),
);

/** Only the latest load writes: a late answer for another profile is dropped. */
let loads = 0;
async function load(): Promise<void> {
  if (!applies.value) return;
  const mine = ++loads;
  const asked = profile.value;
  loading.value = true;
  error.value = null;
  manifests.value = new Map();
  try {
    const all: EntryState[] = [];
    let after: string | undefined;
    for (let page = 0; page < 50; page += 1) {
      const { data } = await withDevice((headers) =>
        getEntries({
          headers,
          path: { v: v.value },
          query: {
            space: "config",
            profile: asked,
            ...(after ? { after } : {}),
          },
          throwOnError: true,
        }),
      );
      all.push(...data.items);
      if (!data.next) break;
      after = data.next;
    }
    if (mine !== loads) return;
    entries.value = all;
    loading.value = false;
    void readManifests(mine);
  } catch (e) {
    if (mine !== loads) return;
    error.value = messageOf(e);
    canRetry.value = retryable(e);
    loading.value = false;
  }
}

/** Each plugin's name, four manifests at a time; one failure is one plugin. */
async function readManifests(mine: number): Promise<void> {
  const wanted = groups.value.plugins
    .slice(0, MAX_PLUGINS)
    .filter((p) => manifestProblem(p) === null);
  await eachLimited(wanted, 4, async (p) => {
    const blob = p.manifest?.blob;
    if (!blob) return;
    const bytes = await fileBytes(v.value, blob).catch(() => null);
    if (mine !== loads) return;
    const next = new Map(manifests.value);
    next.set(p.id, (bytes && readManifest(bytes)) ?? "unreadable");
    manifests.value = next;
  });
}

function pluginNote(
  id: string,
  problem: "missing" | "tooLarge" | null,
): string {
  if (problem) return t(`config.manifest.${problem}`);
  return manifests.value.get(id) === "unreadable"
    ? t("config.manifest.unreadable")
    : "";
}

// The open file: its text, its last change.
const file = computed(() =>
  openPath.value
    ? entries.value.find((e) => e.path === openPath.value)
    : undefined,
);
const text = ref<string | null>(null);
const textError = ref<string | null>(null);
const edited = ref<RevisionInfo | null>(null);
let opens = 0;
async function open(): Promise<void> {
  const mine = ++opens;
  text.value = null;
  textError.value = null;
  edited.value = null;
  const e = file.value;
  if (!e?.blob || e.size > MAX_SHOWN) return;
  try {
    const bytes = await fileBytes(v.value, e.blob);
    if (mine !== opens) return;
    if (!bytes) {
      textError.value = t("config.unreadable");
      return;
    }
    text.value = new TextDecoder().decode(bytes);
  } catch (err) {
    if (mine === opens) textError.value = messageOf(err);
  }
  try {
    const { data } = await withDevice((headers) =>
      getRevisions({
        headers,
        path: { v: v.value, e: e.entry },
        throwOnError: true,
      }),
    );
    if (mine === opens) edited.value = data.items[0] ?? null;
  } catch {
    // A line less above the text, nothing worse.
  }
}

const listLink = computed(() => ({ query: { profile: profile.value } }));
function fileLink(e: EntryState) {
  return { query: { profile: profile.value, file: e.path } };
}

// Back from a file: focus returns to its row.
watch(openPath, async (now, before) => {
  if (!now && before) {
    await nextTick();
    document
      .querySelector<HTMLElement>(`[data-file="${CSS.escape(before)}"]`)
      ?.focus();
  }
});
watch([profile, applies], () => void load());
watch(file, () => void open());

onMounted(() => {
  void vaultStore.load().catch(() => undefined);
  void load();
});

const label = (c: Category) => t(`config.category.${c}`);
</script>

<template>
  <section class="max-w-[46rem]">
    <h2 class="sr-only">{{ t("vault.config") }}</h2>
    <p v-if="!vault" class="text-ink-dim">{{ t("state.loading") }}</p>
    <p v-else-if="vault.paths" data-test="not-yours">{{ t("config.notForYou") }}</p>
    <p v-else-if="vault.configSync.length === 0" data-test="not-synced">{{ t("config.notSynced") }}</p>
    <template v-else>
      <p class="mb-4 text-ink-dim">{{ t("config.intro") }}</p>
      <nav :aria-label="t('config.profiles')" class="mb-2 flex gap-4">
        <RouterLink
          v-for="p in ['desktop', 'mobile'] as const"
          :key="p"
          :to="{ query: { profile: p } }"
          :aria-current="profile === p ? 'page' : undefined"
          :class="profile === p ? 'font-bold text-ink' : 'text-ink-dim hover:text-ink'"
          :data-test="`profile-${p}`"
        >{{ t(`config.profile.${p}`) }}</RouterLink>
      </nav>
      <p class="mb-6 text-ink-dim">{{ t("config.syncedHere", { list: synced.map(label).join(", ") }) }}</p>

      <AsyncState
        :loading="loading"
        :error="error"
        :empty="entries.length === 0"
        :empty-message="t('config.empty')"
        :title="t('config.failed')"
        :can-retry="canRetry"
        @retry="load"
      >
        <!-- A file open -->
        <article v-if="openPath" data-test="file">
          <p class="mb-2 flex flex-wrap items-baseline justify-between gap-2">
            <RouterLink :to="listLink" class="underline" data-test="back">{{ t("config.back") }}</RouterLink>
            <span class="break-all font-bold">{{ openPath }}</span>
          </p>
          <p v-if="!file" role="alert">{{ t("config.gone") }}</p>
          <p v-else-if="isHidden(file)">{{ t(`config.hidden.${hiddenReason(file.path)}`) }}</p>
          <p v-else-if="file.size > MAX_SHOWN" data-test="too-large">
            {{ t("config.tooLarge", { size: formatSize(file.size, locale), max: formatSize(MAX_SHOWN, locale) }) }}
          </p>
          <template v-else>
            <p v-if="edited" class="mb-3 text-ink-dim">
              {{ edited.author
                ? t("config.lastEditedBy", { date: day(edited.createdAt, locale), who: edited.author.name })
                : t("config.lastEdited", { date: day(edited.createdAt, locale) }) }}
            </p>
            <p v-if="textError" role="alert">{{ textError }}</p>
            <p v-else-if="text === null" role="status" class="text-ink-dim">{{ t("state.loading") }}</p>
            <pre v-else class="overflow-x-auto bg-raised p-3 text-sm" data-test="text">{{ text }}</pre>
          </template>
        </article>

        <!-- The list -->
        <div v-else data-test="list">
          <template v-for="c in synced" :key="c">
            <section v-if="c !== 'plugins' && groups[c].length > 0" class="mb-6">
              <h3 class="mb-2 font-bold uppercase tracking-wide">{{ label(c) }}</h3>
              <ul>
                <li v-for="e in groups[c]" :key="e.entry" class="flex items-baseline justify-between gap-4 py-0.5">
                  <span v-if="isHidden(e)" class="break-all">{{ shortName(e.path) }}</span>
                  <RouterLink v-else :to="fileLink(e)" :data-file="e.path" class="break-all underline">{{ shortName(e.path) }}</RouterLink>
                  <span class="shrink-0 text-ink-dim">
                    {{ formatSize(e.size, locale) }}<template v-if="isHidden(e)"> · {{ t(`config.notShown.${hiddenReason(e.path)}`) }}</template>
                  </span>
                </li>
              </ul>
            </section>
            <section v-else-if="c === 'plugins' && groups.plugins.length > 0" class="mb-6">
              <h3 class="mb-2 font-bold uppercase tracking-wide">{{ label(c) }} ({{ groups.plugins.length }})</h3>
              <ul>
                <li v-for="(p, i) in groups.plugins" :key="p.id" class="mb-3" data-test="plugin">
                  <p>
                    <template v-if="manifests.get(p.id) && manifests.get(p.id) !== 'unreadable'">
                      <span class="font-bold">{{ (manifests.get(p.id) as Manifest).name }}</span>{{ (manifests.get(p.id) as Manifest).version ? ` ${(manifests.get(p.id) as Manifest).version}` : "" }}
                      <span class="text-ink-dim">· {{ p.id }}</span>
                    </template>
                    <template v-else>
                      <span class="font-bold">{{ p.id }}</span>
                      <span v-if="i < MAX_PLUGINS && pluginNote(p.id, manifestProblem(p))" class="text-ink-dim"> · {{ pluginNote(p.id, manifestProblem(p)) }}</span>
                    </template>
                  </p>
                  <ul class="pl-4">
                    <li v-for="e in p.files" :key="e.entry" class="flex items-baseline justify-between gap-4 py-0.5">
                      <span v-if="isHidden(e)" class="break-all">{{ shortName(e.path) }}</span>
                      <RouterLink v-else :to="fileLink(e)" :data-file="e.path" class="break-all underline">{{ shortName(e.path) }}</RouterLink>
                      <span class="shrink-0 text-ink-dim">
                        {{ formatSize(e.size, locale) }}<template v-if="isHidden(e)"> · {{ t(`config.notShown.${hiddenReason(e.path)}`) }}</template>
                      </span>
                    </li>
                  </ul>
                </li>
              </ul>
            </section>
          </template>
          <p v-if="anyHiddenSettings" class="mt-4 border-t border-dashed border-rule-soft pt-3 text-ink-dim" data-test="why-hidden">
            {{ t("config.whyHidden") }}
          </p>
          <p v-if="notSynced.length > 0" class="mt-2 text-ink-dim">
            {{ t("config.notSyncedHere", { list: notSynced.map(label).join(", ") }) }}
          </p>
        </div>
      </AsyncState>
    </template>
  </section>
</template>
