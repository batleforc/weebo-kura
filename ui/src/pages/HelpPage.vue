<script setup lang="ts">
/**
 * The console's help. First the one task most people come for, installing
 * the plugin, as steps for the device in hand (desktop, Android, iPhone or
 * iPad); then the other questions, folded. Every word is in the locales.
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";
import { loadedConfig } from "@/config";
import { useMe } from "@/me";
import {
  type Device,
  deviceOf,
  type PluginBuild,
  pluginBuild,
  setupLink,
} from "@/obsidian";

/** Each step: its text key, and what it carries besides its text. */
const STEPS: Record<
  Device,
  { key: string; with?: "download" | "path" | "vaults" | "manual" }[]
> = {
  desktop: [
    { key: "download", with: "download" },
    { key: "unzip", with: "path" },
    { key: "enable" },
    { key: "connect", with: "vaults" },
  ],
  android: [
    { key: "download", with: "download" },
    { key: "androidExtract" },
    { key: "androidMove", with: "path" },
    { key: "enable" },
    { key: "connect", with: "vaults" },
  ],
  ios: [
    { key: "iosIcloud" },
    { key: "iosCopy", with: "download" },
    { key: "iosPlace", with: "path" },
    { key: "enable" },
    { key: "connect", with: "vaults" },
  ],
};

const { t, te, locale } = useI18n();
const meStore = useMe();
const device = ref<Device>(
  deviceOf(navigator.userAgent, navigator.maxTouchPoints ?? 0),
);
const steps = computed(() => STEPS[device.value]);
const build = ref<PluginBuild | null>(null);
const vaults = ref<Vault[]>([]);
const server = window.location.origin;
const pluginIssuer = loadedConfig()?.plugin?.issuer ?? "";
const copied = ref("");
/** Whole-vault grants only: a folder-limited reader reads in the browser. */
const syncable = computed(() => vaults.value.filter((v) => !v.paths));
const manages = computed(
  () =>
    vaults.value.some((v) => v.role === "manager") ||
    (meStore.me?.tenants.some((x) => x.role !== "member") ?? false),
);
const size = computed(() =>
  build.value?.bytes
    ? new Intl.NumberFormat(locale.value, {
        style: "unit",
        unit: "kilobyte",
        maximumFractionDigits: 0,
      }).format(build.value.bytes / 1000)
    : "",
);

async function copy(text: string): Promise<void> {
  await navigator.clipboard.writeText(text).catch(() => undefined);
  copied.value = text;
}

onMounted(async () => {
  void meStore.load().catch(() => undefined);
  build.value = await pluginBuild();
  vaults.value = await withDevice(
    async (headers) => (await listVaults({ headers, throwOnError: true })).data,
  ).catch(() => []);
});
</script>

<template>
  <article class="max-w-3xl">
    <h1 class="mb-2">{{ t("help.title") }}</h1>
    <p class="mb-10 text-ink-dim">{{ t("help.intro") }}</p>

    <section id="obsidian" class="mb-14 scroll-mt-4" aria-labelledby="obsidian-h">
      <h2 id="obsidian-h" class="mb-2 text-xl font-bold">{{ t("help.obsidian.title") }}</h2>
      <p class="mb-5">{{ t("help.obsidian.lead") }}</p>

      <p :id="'device-label'" class="meta mb-1 text-ink-dim">{{ t("help.device.label") }}</p>
      <div role="group" aria-labelledby="device-label" class="mb-8 flex max-w-md border border-rule">
        <button
          v-for="(d, i) in ['desktop', 'android', 'ios'] as const"
          :key="d"
          type="button"
          class="flex-1 px-2 py-2"
          :class="[i ? 'border-l border-rule' : '', device === d ? 'bg-ink font-bold text-ground' : 'text-ink-dim hover:text-ink']"
          :aria-pressed="device === d"
          :data-device="d"
          @click="device = d"
        >
          {{ t(`help.device.${d}`) }}
        </button>
      </div>

      <ol class="grid gap-8" :data-steps="device">
        <li v-for="(s, i) in steps" :key="`${device}-${s.key}`" class="grid grid-cols-[3rem_1fr] gap-x-4">
          <span aria-hidden="true" class="text-2xl font-bold leading-none text-ink-dim">{{ i + 1 }}</span>
          <div>
            <h3 class="mb-1 font-bold">{{ t(`help.step.${s.key}.title`) }}</h3>
            <p>{{ t(`help.step.${s.key}.body`) }}</p>

            <template v-if="s.with === 'download'">
              <p v-if="build === null" role="status" class="mt-3 text-ink-dim">{{ t("state.loading") }}</p>
              <p v-else-if="build.available" class="mt-3 flex flex-wrap items-baseline gap-x-4 gap-y-2">
                <a :href="`/app/${build.file}`" download class="action" data-test="download">{{ t("help.obsidian.downloadAction") }}</a>
                <span class="text-ink-dim">{{ t("help.obsidian.build", { version: build.version, size }) }}</span>
              </p>
              <p v-else class="mt-3 border-l border-rule pl-3" data-test="no-plugin">{{ t("help.obsidian.notBundled") }}</p>
            </template>

            <p v-if="s.with === 'path'" class="mt-3">
              <code class="break-all bg-raised px-2 py-1">{{ t(`help.path.${device}`) }}</code>
            </p>

            <template v-if="s.with === 'vaults'">
              <ul v-if="syncable.length && setupLink(server)" class="mt-3 grid gap-2">
                <li v-for="v in syncable" :key="v.id" class="flex flex-wrap items-baseline gap-x-3">
                  <a :href="setupLink(server, v) ?? undefined" class="border border-rule px-3 py-1 hover:border-ink" :data-setup="v.name">{{ t("help.obsidian.open", { name: v.name }) }}</a>
                  <span class="meta text-ink-dim">{{ v.tenant }} · {{ v.role }}</span>
                </li>
              </ul>
              <p v-else-if="!setupLink(server)" class="mt-3 text-ink-dim">{{ t("help.obsidian.noLink") }}</p>
              <p class="mt-3 text-ink-dim">{{ t("help.obsidian.code") }}</p>
            </template>
          </div>
        </li>
      </ol>

      <details class="mt-10 border-t border-dashed border-rule-soft pt-4">
        <summary class="cursor-pointer">{{ t("help.obsidian.byHand") }}</summary>
        <dl class="mt-3 grid gap-2">
          <div v-for="[label, value] in [[t('help.obsidian.serverField'), server], [t('help.obsidian.issuerField'), pluginIssuer]]" :key="label" class="flex flex-wrap items-baseline gap-x-3">
            <dt class="meta w-48 text-ink-dim">{{ label }}</dt>
            <dd class="flex min-w-0 items-baseline gap-2">
              <code class="break-all">{{ value || "—" }}</code>
              <button v-if="value" type="button" class="border border-rule px-2" @click="copy(value)">{{ copied === value ? t("keys.copied") : t("keys.copy") }}</button>
            </dd>
          </div>
        </dl>
        <p class="mt-2 text-ink-dim">{{ t("help.obsidian.byHandThen") }}</p>
      </details>
    </section>

    <section aria-labelledby="faq-h">
      <h2 id="faq-h" class="mb-4 text-xl font-bold">{{ t("help.faq") }}</h2>
      <div class="border-t border-rule">
        <details v-for="q in ['reader', 'roles', 'devices', 'trouble', ...(manages ? ['manage'] : [])]" :id="q" :key="q" class="border-b border-dashed border-rule-soft py-3">
          <summary class="cursor-pointer font-bold text-[length:var(--t-row)]">{{ t(`help.${q}.title`) }}</summary>
          <div class="mt-3 grid gap-2">
            <template v-if="q === 'roles'">
              <dl class="grid gap-2">
                <div v-for="r in ['viewer', 'editor', 'manager']" :key="r">
                  <dt class="meta font-bold">{{ r }}</dt>
                  <dd>{{ t(`help.roles.${r}`) }}</dd>
                </div>
              </dl>
              <p class="text-ink-dim">{{ t("help.roles.folders") }}</p>
            </template>
            <template v-else-if="q === 'manage'">
              <dl class="grid gap-2">
                <div v-for="k in ['access', 'keys', 'vaults', 'notifications']" :key="k">
                  <dt class="meta font-bold">{{ t(`help.manage.${k}Title`) }}</dt>
                  <dd>{{ t(`help.manage.${k}`) }}</dd>
                </div>
              </dl>
            </template>
            <template v-else>
              <p v-for="p in ['p1', 'p2', 'p3'].filter((k) => te(`help.${q}.${k}`))" :key="p">{{ t(`help.${q}.${p}`) }}</p>
            </template>
          </div>
        </details>
      </div>
    </section>
  </article>
</template>
