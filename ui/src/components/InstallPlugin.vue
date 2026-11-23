<script setup lang="ts">
/**
 * Install a plugin for yourself (RFC 0010 §4.5): a release zip read in the
 * browser, only its three code files sent, for the member's own devices.
 * Never the vault: that is a manager's, from Obsidian.
 */
import { ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { installPlugin } from "@/api/generated/sdk.gen";
import { messageOf } from "@/api/problem";
import {
  base64,
  MAX_TOTAL,
  type ReleasePlugin,
  readRelease,
  type ZipProblem,
} from "@/release-zip";

const props = defineProps<{ vault: string }>();
const emit = defineEmits<{ installed: [] }>();
const { t } = useI18n();

const found = ref<ReleasePlugin | null>(null);
const problem = ref<ZipProblem | null>(null);
const target = ref<"shared" | "desktop" | "mobile">("shared");
const busy = ref(false);
const done = ref<string | null>(null);
const failed = ref<string | null>(null);

async function choose(event: Event): Promise<void> {
  found.value = null;
  problem.value = null;
  done.value = null;
  failed.value = null;
  const file =
    event.target instanceof HTMLInputElement ? event.target.files?.[0] : null;
  if (!file) return;
  if (file.size > MAX_TOTAL) {
    problem.value = "tooLarge";
    return;
  }
  const got = readRelease(new Uint8Array(await file.arrayBuffer()));
  if ("problem" in got) problem.value = got.problem;
  else found.value = got;
}

async function install(): Promise<void> {
  const p = found.value;
  if (!p || busy.value) return;
  busy.value = true;
  failed.value = null;
  try {
    await withDevice((headers) =>
      installPlugin({
        headers,
        path: { v: props.vault, id: p.id },
        query: { target: target.value },
        body: {
          manifest: base64(p.manifest),
          main: base64(p.main),
          ...(p.styles ? { styles: base64(p.styles) } : {}),
        },
        throwOnError: true,
      }),
    );
    done.value = t("config.install.done", { name: p.name });
    found.value = null;
    emit("installed");
  } catch (e) {
    failed.value = messageOf(e);
  } finally {
    busy.value = false;
  }
}

const files = (p: ReleasePlugin) =>
  ["manifest.json", "main.js", ...(p.styles ? ["styles.css"] : [])].join(", ");
</script>

<template>
  <section class="mt-8 border-t border-dashed border-rule-soft pt-4" data-test="install">
    <h3 class="mb-2 font-bold uppercase tracking-wide">{{ t("config.install.title") }}</h3>
    <label class="block">{{ t("config.install.zip") }}
      <input type="file" accept=".zip,application/zip" class="ml-2" data-test="zip" @change="choose" />
    </label>
    <p v-if="problem" role="alert" class="mt-2 border-l border-rule pl-3" data-test="zip-problem">{{ t(`config.install.problem.${problem}`) }}</p>
    <form v-else-if="found" class="mt-2 grid gap-2" @submit.prevent="install">
      <p data-test="found">{{ t("config.install.found", { name: found.name, version: found.version ?? "", id: found.id, files: files(found) }) }}</p>
      <fieldset class="flex flex-wrap gap-4">
        <legend>{{ t("config.install.for") }}</legend>
        <label v-for="x in ['shared', 'desktop', 'mobile'] as const" :key="x">
          <input v-model="target" type="radio" :value="x" :data-test="`target-${x}`" /> {{ t(`config.install.target.${x}`) }}
        </label>
      </fieldset>
      <p class="flex flex-wrap items-baseline justify-between gap-2">
        <span>{{ t("config.install.disabled") }}</span>
        <button type="submit" class="bg-accent px-3 text-accent-ink" :disabled="busy" data-test="install-go">{{ t("config.install.go") }}</button>
      </p>
    </form>
    <p v-if="failed" role="alert" class="mt-2 border-l border-rule pl-3">{{ failed }}</p>
    <p v-if="done" role="status" class="mt-2" data-test="installed">{{ done }}</p>
  </section>
</template>
