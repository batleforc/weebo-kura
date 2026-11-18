<script setup lang="ts">
/** A text filter over the table below it, and the line saying when nothing matches. */
import { useId } from "vue";
import { useI18n } from "vue-i18n";

defineProps<{ empty: boolean }>();
const query = defineModel<string>({ required: true });
const { t } = useI18n();
const id = useId();
</script>

<template>
  <div class="mb-2">
    <label :for="id" class="mr-2 text-ink-dim">{{ t("filter.label") }}</label>
    <input :id="id" v-model="query" type="search" class="border border-rule bg-ground px-2" data-test="filter" />
    <!-- Always there, so a screen reader hears the line when it fills. -->
    <p role="status" class="text-ink-dim"><template v-if="empty && query.trim() !== ''">{{ t("filter.none", { q: query.trim() }) }}</template></p>
  </div>
</template>
