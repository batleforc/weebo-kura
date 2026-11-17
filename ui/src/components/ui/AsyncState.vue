<script setup lang="ts">
/**
 * Loading, then error, then empty, then content, one at a time (after
 * proxy-cache's AsyncState, R31): a failed load never also shows "empty".
 */
import { useI18n } from "vue-i18n";

defineProps<{
  loading: boolean;
  error?: string | null;
  empty?: boolean;
  emptyMessage: string;
  title: string;
}>();
const emit = defineEmits<{ retry: [] }>();
const { t } = useI18n();
</script>

<template>
  <p v-if="loading" role="status" class="text-ink-dim">{{ t("state.loading") }}</p>
  <div v-else-if="error" role="alert" class="border border-accent p-4">
    <p class="font-bold">{{ title }}</p>
    <p class="text-ink-dim">{{ error }}</p>
    <button type="button" class="mt-2 border border-rule px-3 py-1" @click="emit('retry')">
      {{ t("state.retry") }}
    </button>
  </div>
  <p v-else-if="empty" class="text-ink-dim">{{ emptyMessage }}</p>
  <slot v-else />
</template>
