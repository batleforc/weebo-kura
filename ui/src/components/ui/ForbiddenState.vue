<script setup lang="ts">
/**
 * The §4.5 Forbidden state: what happened in the page's language, who can
 * help, and a way back; the server's own detail ("manager required, you are
 * viewer") under it, as it came.
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

defineProps<{ detail?: string | null }>();
const { t } = useI18n();
const route = useRoute();
// Inside a vault, back to its overview; elsewhere, home.
const back = computed(() =>
  typeof route.params.v === "string"
    ? { to: `/vaults/${route.params.v}`, label: t("forbidden.toVault") }
    : { to: "/vaults", label: t("forbidden.toHome") },
);
</script>

<template>
  <div role="alert" class="flex flex-col gap-2" data-test="forbidden">
    <h2 class="font-bold">{{ t("forbidden.title") }}</h2>
    <p>{{ t("manage.forbidden") }} {{ t("forbidden.ask") }}</p>
    <!-- The server's details are English whatever the page's language (RGAA 8.7). -->
    <p v-if="detail" class="text-ink-dim" lang="en">{{ detail }}</p>
    <RouterLink :to="back.to" class="self-start underline underline-offset-4">{{ back.label }}</RouterLink>
  </div>
</template>
