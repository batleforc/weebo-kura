<script setup lang="ts">
/**
 * The §4.5 Forbidden state: the server's `insufficient_role` detail as it
 * came ("manager required, you are viewer"), else the console's own text.
 */
import { useI18n } from "vue-i18n";

defineProps<{ detail?: string | null }>();
const { t } = useI18n();
</script>

<template>
  <p role="alert" data-test="forbidden">
    <!-- The server's details are English whatever the page's language (RGAA 8.7). -->
    <span v-if="detail" lang="en">{{ detail }}</span>
    <template v-else>{{ t("manage.forbidden") }}</template>
  </p>
</template>
