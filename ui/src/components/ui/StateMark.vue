<script setup lang="ts">
/**
 * Resolution as state (batlehub-design, the signature): what is live and
 * known renders fine (3×3 dots, ink); what is not renders coarse (2×2 dots,
 * copper, dim or crimson). Always with its word: pattern, word and hue
 * together, never hue alone.
 */
import { computed } from "vue";

const props = defineProps<{
  /** live: in force · waiting: held back, a person must act · off: not in force · gone: revoked · refused: failing */
  kind: "live" | "waiting" | "off" | "gone" | "refused";
  word: string;
}>();

const look = {
  live: { fine: true, lit: 9, tone: "text-ink" },
  waiting: { fine: false, lit: 3, tone: "text-copper" },
  off: { fine: false, lit: 2, tone: "text-ink-dim" },
  gone: { fine: false, lit: 1, tone: "text-ink-dim" },
  refused: { fine: false, lit: 4, tone: "text-accent" },
} as const;

const cells = computed(() => {
  const l = look[props.kind];
  const n = l.fine ? 3 : 2;
  const size = l.fine ? 5 : 8;
  return Array.from({ length: n * n }, (_, i) => ({
    x: (i % n) * (size + 1),
    y: Math.floor(i / n) * (size + 1),
    size,
    lit: i < l.lit,
  }));
});
</script>

<template>
  <span class="inline-flex items-baseline gap-2" :class="look[kind].tone" :data-state="kind">
    <svg aria-hidden="true" width="17" height="17" viewBox="0 0 17 17" class="self-center">
      <rect v-for="(c, i) in cells" :key="i" :x="c.x" :y="c.y" :width="c.size" :height="c.size" fill="currentColor" :opacity="c.lit ? 1 : 0.18" />
    </svg>
    <span class="meta">{{ word }}</span>
  </span>
</template>
