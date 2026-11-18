<script setup lang="ts">
/** Where the page sits, one link per step back; the last step is the page itself. */
import { useI18n } from "vue-i18n";

defineProps<{ items: { label: string; to?: string }[] }>();
const { t } = useI18n();
</script>

<template>
  <nav :aria-label="t('reader.breadcrumb')" class="mb-6 flex flex-wrap gap-x-2 text-ink-dim" data-test="crumbs">
    <template v-for="(c, i) in items" :key="i">
      <span v-if="i" aria-hidden="true">▸</span>
      <RouterLink v-if="c.to" :to="c.to" class="underline">{{ c.label }}</RouterLink>
      <span v-else>{{ c.label }}</span>
    </template>
  </nav>
</template>
