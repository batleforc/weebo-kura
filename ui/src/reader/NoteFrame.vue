<script setup lang="ts">
/**
 * The only place a note's HTML lives (RFC 0005 §6.5, decision 9): an iframe
 * filled through `srcdoc`, sandboxed with `allow-scripts` and never
 * `allow-same-origin`, so its origin is opaque and nothing in it reaches the
 * application's storage or memory. What its bridge sends is untrusted: a
 * number for the height, and a link the application decides about.
 */
import { onBeforeUnmount, onMounted, ref } from "vue";

const props = defineProps<{ document: string; title: string }>();
const emit = defineEmits<{ link: [href: string] }>();
const frame = ref<HTMLIFrameElement | null>(null);
const height = ref(200);

function onMessage(event: MessageEvent): void {
  if (!frame.value || event.source !== frame.value.contentWindow) return;
  const data: unknown = event.data;
  if (typeof data !== "object" || data === null) return;
  const kind = Reflect.get(data, "kind");
  const value = Reflect.get(data, "value");
  if (
    kind === "height" &&
    typeof value === "number" &&
    Number.isFinite(value)
  ) {
    height.value = Math.min(Math.max(Math.ceil(value), 50), 100_000);
  } else if (
    kind === "link" &&
    typeof value === "string" &&
    value.length < 2048
  ) {
    emit("link", value);
  }
}

onMounted(() => window.addEventListener("message", onMessage));
onBeforeUnmount(() => window.removeEventListener("message", onMessage));
defineExpose({ onMessage, height });
</script>

<template>
  <iframe
    ref="frame"
    :title="props.title"
    :srcdoc="props.document"
    sandbox="allow-scripts"
    referrerpolicy="no-referrer"
    class="w-full border-0"
    :style="{ height: `${height}px` }"
    data-test="note-frame"
  />
</template>
