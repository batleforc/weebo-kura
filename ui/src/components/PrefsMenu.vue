<script setup lang="ts">
/**
 * Theme and language (batlehub-design, Inputs): one popover, two three-state
 * groups, the chosen cell reversed to an ink block. Escape or a click outside
 * closes it and focus goes back to its trigger.
 */
import { computed, nextTick, onBeforeUnmount, ref, useId } from "vue";
import { useI18n } from "vue-i18n";

import {
  type LocalePref,
  localePref,
  resolvedLocale,
  resolvedTheme,
  setLocale,
  setTheme,
  type ThemePref,
  themePref,
} from "@/prefs";

const { t } = useI18n();
const open = ref(false);
const trigger = ref<HTMLButtonElement | null>(null);
const panel = ref<HTMLElement | null>(null);
const id = useId();
const announced = ref("");

const themes: ThemePref[] = ["system", "light", "dark"];
const locales: LocalePref[] = ["system", "en", "fr"];
const themeNote = computed(() =>
  themePref.value === "system"
    ? t("prefs.systemTheme", { now: t(`prefs.theme.${resolvedTheme()}`) })
    : "",
);
const localeNote = computed(() =>
  localePref.value === "system"
    ? t("prefs.systemLocale", { now: t(`prefs.locale.${resolvedLocale()}`) })
    : "",
);

function outside(e: MouseEvent): void {
  const target = e.target;
  if (!(target instanceof Node)) return;
  if (!panel.value?.contains(target) && !trigger.value?.contains(target))
    close();
}

async function toggle(): Promise<void> {
  if (open.value) return close();
  open.value = true;
  document.addEventListener("mousedown", outside);
  await nextTick();
  panel.value?.querySelector("button")?.focus();
}

function close(): void {
  open.value = false;
  document.removeEventListener("mousedown", outside);
  trigger.value?.focus();
}
onBeforeUnmount(() => document.removeEventListener("mousedown", outside));

function pickTheme(p: ThemePref): void {
  setTheme(p);
  announced.value = t("prefs.themeSet", { name: t(`prefs.theme.${p}`) });
}
function pickLocale(p: LocalePref): void {
  setLocale(p);
  announced.value = t("prefs.localeSet", { name: t(`prefs.locale.${p}`) });
}
</script>

<template>
  <div class="relative" @keydown.esc="open && close()">
    <button
      ref="trigger"
      type="button"
      class="border border-rule px-2 text-ink-dim hover:text-ink"
      :aria-expanded="open"
      :aria-controls="id"
      data-test="prefs"
      @click="toggle"
    >
      {{ t("prefs.title") }}
    </button>
    <div v-if="open" :id="id" ref="panel" class="absolute right-0 top-full z-20 mt-2 grid w-66 gap-4 border border-rule bg-ground p-4">
      <div>
        <p :id="`${id}-theme`" class="mb-1 text-ink-dim">{{ t("prefs.themeLabel") }}</p>
        <div role="group" :aria-labelledby="`${id}-theme`" class="flex border border-rule">
          <button
            v-for="(p, i) in themes"
            :key="p"
            type="button"
            class="flex-1 px-2 py-1"
            :class="[i ? 'border-l border-rule' : '', themePref === p ? 'bg-ink font-bold text-ground' : 'text-ink-dim hover:text-ink']"
            :aria-pressed="themePref === p"
            @click="pickTheme(p)"
          >
            {{ t(`prefs.theme.${p}`) }}
          </button>
        </div>
        <p v-if="themeNote" class="mt-1 text-ink-dim">{{ themeNote }}</p>
      </div>
      <div>
        <p :id="`${id}-locale`" class="mb-1 text-ink-dim">{{ t("prefs.localeLabel") }}</p>
        <div role="group" :aria-labelledby="`${id}-locale`" class="flex border border-rule">
          <button
            v-for="(p, i) in locales"
            :key="p"
            type="button"
            class="flex-1 px-2 py-1"
            :class="[i ? 'border-l border-rule' : '', localePref === p ? 'bg-ink font-bold text-ground' : 'text-ink-dim hover:text-ink']"
            :aria-pressed="localePref === p"
            :lang="p === 'system' ? undefined : p"
            @click="pickLocale(p)"
          >
            {{ t(`prefs.locale.${p}`) }}
          </button>
        </div>
        <p v-if="localeNote" class="mt-1 text-ink-dim">{{ localeNote }}</p>
      </div>
    </div>
    <p role="status" class="sr-only">{{ announced }}</p>
  </div>
</template>
