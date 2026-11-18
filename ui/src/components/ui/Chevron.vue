<script setup lang="ts">
/** The console's one icon (batlehub-design: inline SVG, 1.5px stroke, no glyph-as-icon). */
defineProps<{ dir: "left" | "right" | "down" }>();
</script>

<template>
  <svg
    aria-hidden="true"
    width="14"
    height="14"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="square"
    class="shrink-0"
    :class="{ 'rotate-180': dir === 'left', 'rotate-90': dir === 'down' }"
  >
    <path d="M6 3.5 10.5 8 6 12.5" />
  </svg>
</template>
