<script setup lang="ts">
/**
 * One vault, everything about it under one header: its overview, its notes
 * (browse, a note, its history, search) and, for its managers, its access,
 * keys and notifications, as tabs.
 */
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { useMe } from "@/me";
import { setupLink } from "@/obsidian";
import { sectionOf, sectionsOf, useVaults } from "@/vaults";

const { t } = useI18n();
const route = useRoute();
const vaultStore = useVaults();
const meStore = useMe();
const origin = window.location.origin;
const id = computed(() => String(route.params.v));
const vault = computed(() => vaultStore.list.find((v) => v.id === id.value));
const obsidian = computed(() =>
  vault.value && !vault.value.paths ? setupLink(origin, vault.value) : null,
);
const current = computed(() => sectionOf(route.name));
/** The reader takes the width; every tab shares the same one-line header. */
const reading = computed(() => current.value === "vault-notes");
const tabs = computed(() =>
  vault.value ? sectionsOf(vault.value, meStore.me?.tenants) : [],
);

onMounted(() => {
  void vaultStore.load().catch(() => undefined);
  void meStore.load().catch(() => undefined);
});
</script>

<template>
  <section class="flex flex-col gap-6">
    <header class="flex flex-wrap items-center gap-x-6 gap-y-3 border-b border-rule-soft pb-4">
      <div class="flex min-w-0 flex-wrap items-baseline gap-x-2">
        <span v-if="vault" class="text-[length:var(--t-head)] text-ink-dim">{{ vault.tenant }} /</span>
        <h1 class="break-all text-[length:var(--t-head)]">{{ vault?.name ?? "…" }}</h1>
        <span v-if="vault" class="meta text-ink-dim" data-test="role">{{ t("vault.youAre", { role: t(`role.${vault.role}`) }) }}</span>
      </div>
      <!-- From 768px the sidebar carries the sections; on a phone it folds away, so they show here. -->
      <nav :aria-label="t('vault.sections')" class="inline-flex max-w-full overflow-x-auto whitespace-nowrap border border-rule md:hidden" data-test="vault-tabs">
        <RouterLink
          v-for="(tab, i) in tabs"
          :key="tab.name"
          :to="tab.to"
          class="px-3 py-1 no-underline"
          :class="[i ? 'border-l border-rule' : '', current === tab.name ? 'bg-ink font-bold text-ground' : 'text-ink-dim hover:text-ink']"
          :aria-current="current === tab.name ? 'page' : undefined"
        >{{ t(tab.label) }}</RouterLink>
      </nav>
      <a v-if="obsidian" :href="obsidian" class="ctl !py-1 md:ml-auto" data-test="open-obsidian">{{ t("vaults.obsidian") }}</a>
    </header>
    <div :class="{ 'max-w-6xl': !reading }">
      <RouterView />
    </div>
  </section>
</template>
