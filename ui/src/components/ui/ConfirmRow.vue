<script setup lang="ts">
/**
 * A destructive action's confirmation, as a table row right under the row
 * that asked for it: focused when it opens, Escape cancels, and focus goes
 * back to the button that opened it.
 */
import { onMounted, ref, useId } from "vue";
import { useI18n } from "vue-i18n";

defineProps<{ colspan: number; question: string; action: string }>();
const emit = defineEmits<{ confirm: []; cancel: [] }>();
const { t } = useI18n();
const box = ref<HTMLElement | null>(null);
const id = useId();
let opener: HTMLElement | null = null;

onMounted(() => {
  opener =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  box.value?.focus();
});

function cancel(): void {
  emit("cancel");
  opener?.focus();
}
</script>

<template>
  <tr>
    <td :colspan="colspan" class="pb-2 whitespace-normal">
      <div
        ref="box"
        role="alertdialog"
        tabindex="-1"
        :aria-labelledby="id"
        class="flex flex-wrap items-center gap-2 border-t border-accent pt-2"
        @keydown.esc="cancel"
      >
        <p :id="id" class="mr-2">{{ question }}</p>
        <button type="button" class="bg-accent px-3 text-accent-ink" data-test="confirm" @click="emit('confirm')">{{ action }}</button>
        <button type="button" class="border border-rule px-3" @click="cancel">{{ t("devices.cancel") }}</button>
      </div>
    </td>
  </tr>
</template>
