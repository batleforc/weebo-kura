<script setup lang="ts">
/**
 * A table's own horizontal scroller: a named region a keyboard can reach,
 * with shadows at whichever edge hides more columns (CSS only, `.scroll-x`).
 */
defineProps<{ label: string }>();
</script>

<template>
  <div class="scroll-x relative overflow-x-auto" role="region" :aria-label="label" tabindex="0">
    <slot />
  </div>
</template>
