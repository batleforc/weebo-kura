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
import { useVaults } from "@/vaults";

const { t } = useI18n();
const route = useRoute();
const vaultStore = useVaults();
const meStore = useMe();
const origin = window.location.origin;
const id = computed(() => String(route.params.v));
const vault = computed(() => vaultStore.list.find((v) => v.id === id.value));
const manages = computed(
  () =>
    vault.value?.role === "manager" ||
    (meStore.me?.tenants.some(
      (x) => x.slug === vault.value?.tenant && x.role !== "member",
    ) ??
      false),
);
const obsidian = computed(() =>
  vault.value && !vault.value.paths ? setupLink(origin, vault.value) : null,
);
/** The tab the current page belongs to: a note, its history and search are Notes. */
const current = computed(() => {
  const name = String(route.name ?? "");
  if (["vault-access", "vault-keys", "vault-notifications"].includes(name))
    return name;
  return name === "vault" ? "vault" : "vault-notes";
});
const tabs = computed(() => [
  { name: "vault", to: `/vaults/${id.value}`, label: t("vault.overview") },
  {
    name: "vault-notes",
    to: `/vaults/${id.value}/notes`,
    label: t("vault.notes"),
  },
  ...(manages.value
    ? [
        {
          name: "vault-access",
          to: `/vaults/${id.value}/access`,
          label: t("manage.access"),
        },
        {
          name: "vault-keys",
          to: `/vaults/${id.value}/keys`,
          label: t("manage.keys"),
        },
        {
          name: "vault-notifications",
          to: `/vaults/${id.value}/notifications`,
          label: t("manage.notifications"),
        },
      ]
    : []),
]);

onMounted(() => {
  void vaultStore.load().catch(() => undefined);
  void meStore.load().catch(() => undefined);
});
</script>

<template>
  <section class="flex max-w-6xl flex-col gap-6">
    <header class="flex flex-wrap items-end justify-between gap-4">
      <div class="min-w-0">
        <p v-if="vault" class="text-ink-dim">{{ vault.tenant }} · {{ t("vault.youAre", { role: t(`role.${vault.role}`) }) }}</p>
        <h1 class="break-all">{{ vault?.name ?? "…" }}</h1>
      </div>
      <div class="flex flex-wrap gap-2">
        <RouterLink v-if="current !== 'vault-notes'" :to="`/vaults/${id}/notes`" class="ctl">{{ t("vault.read") }}</RouterLink>
        <a v-if="obsidian" :href="obsidian" class="action" data-test="open-obsidian">{{ t("vaults.obsidian") }}</a>
      </div>
    </header>
    <nav :aria-label="t('vault.sections')" class="flex flex-wrap self-start border border-rule" data-test="vault-tabs">
      <RouterLink
        v-for="(tab, i) in tabs"
        :key="tab.name"
        :to="tab.to"
        class="px-3.5 py-2 no-underline"
        :class="[i ? 'border-l border-rule' : '', current === tab.name ? 'bg-ink font-bold text-ground' : 'text-ink-dim hover:text-ink']"
        :aria-current="current === tab.name ? 'page' : undefined"
      >{{ tab.label }}</RouterLink>
    </nav>
    <RouterView />
  </section>
</template>
