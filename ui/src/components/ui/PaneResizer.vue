<script setup lang="ts">
/**
 * A vertical handle that sets the width of the pane before it, in rem: drag
 * it, or focus it and use the arrows (Shift for bigger steps), Home and End;
 * a double click goes back to the default. A WAI-ARIA window splitter.
 */
const props = defineProps<{
  min: number;
  max: number;
  fallback: number;
  label: string;
}>();
const width = defineModel<number>({ required: true });
const emit = defineEmits<{ done: [] }>();

let startX = 0;
let startWidth = 0;
const remPx = () =>
  Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
const clamp = (v: number) =>
  Math.min(props.max, Math.max(props.min, Math.round(v * 4) / 4));

function down(e: PointerEvent): void {
  if (e.button !== 0 || !(e.currentTarget instanceof HTMLElement)) return;
  e.currentTarget.setPointerCapture(e.pointerId);
  startX = e.clientX;
  startWidth = width.value;
}
function move(e: PointerEvent): void {
  if (
    !(e.currentTarget instanceof HTMLElement) ||
    !e.currentTarget.hasPointerCapture(e.pointerId)
  )
    return;
  width.value = clamp(startWidth + (e.clientX - startX) / remPx());
}
function up(e: PointerEvent): void {
  if (
    !(e.currentTarget instanceof HTMLElement) ||
    !e.currentTarget.hasPointerCapture(e.pointerId)
  )
    return;
  e.currentTarget.releasePointerCapture(e.pointerId);
  emit("done");
}
function key(e: KeyboardEvent): void {
  const step = e.shiftKey ? 4 : 1;
  const next: Record<string, number> = {
    ArrowLeft: width.value - step,
    ArrowRight: width.value + step,
    Home: props.min,
    End: props.max,
  };
  const v = next[e.key];
  if (v === undefined) return;
  e.preventDefault();
  width.value = clamp(v);
  emit("done");
}
function reset(): void {
  width.value = props.fallback;
  emit("done");
}
</script>

<template>
  <div
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
    :aria-label="label"
    :aria-valuemin="min"
    :aria-valuemax="max"
    :aria-valuenow="width"
    :title="label"
    class="group absolute inset-y-0 -right-5 hidden w-3 cursor-col-resize touch-none justify-center md:flex"
    data-test="tree-resizer"
    @pointerdown="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="up"
    @keydown="key"
    @dblclick="reset"
  >
    <span class="w-px bg-rule-soft transition-colors group-hover:bg-ink-dim group-focus-visible:bg-focus" />
  </div>
</template>
