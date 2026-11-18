<script setup lang="ts">
/** Revoked rows stay out of the way until asked for: a list's history, not its state. */
import { useI18n } from "vue-i18n";

defineProps<{ count: number }>();
const shown = defineModel<boolean>({ required: true });
const { t } = useI18n();
</script>

<template>
  <button v-if="count > 0" type="button" class="mt-2 block text-ink-dim underline" :aria-expanded="shown" data-test="show-revoked" @click="shown = !shown">
    {{ shown ? t("state.hideRevoked") : t("state.showRevoked", { n: count }, count) }}
  </button>
</template>
