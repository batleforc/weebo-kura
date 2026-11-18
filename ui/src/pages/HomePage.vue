<script setup lang="ts">
/**
 * Home: what needs doing (a device to sync, keys about to expire), the
 * caller's vaults with their two ways in, their devices, and what they run.
 * The vault pages hold the rest.
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { listDevices, listTokens } from "@/api/generated/sdk.gen";
import type { Device, Vault } from "@/api/generated/types.gen";
import { retryable } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import StateMark from "@/components/ui/StateMark.vue";
import { targets } from "@/manage/targets";
import { useMe } from "@/me";
import { setupLink } from "@/obsidian";
import { useAutoRetry } from "@/reader/retry";
import { ago, when } from "@/reader/time";
import { useVaults } from "@/vaults";

/** A key expiring within this is worth a line on the home page. */
const SOON_MS = 14 * 86_400_000;

const { t, locale } = useI18n();
const origin = window.location.origin;
const meStore = useMe();
const vaultStore = useVaults();
const vaults = computed(() =>
  [...vaultStore.list].sort(
    (a, b) =>
      Date.parse(b.updatedAt ?? "") - Date.parse(a.updatedAt ?? "") ||
      a.name.localeCompare(b.name),
  ),
);
const loading = ref(true);
const error = ref<string | null>(null);
const canRetry = ref(true);
const failure = useAutoRetry(() => load());
const devices = ref<Device[] | null>(null);
const expiring = ref<{ vault: Vault; n: number; at: string }[]>([]);

const managed = computed(() => targets(meStore.me, vaultStore.list));
const manages = computed(
  () =>
    managed.value.vaults.length > 0 ||
    managed.value.tenants.length > 0 ||
    managed.value.platform,
);
const mine = computed(() =>
  (devices.value ?? []).filter((d) => !d.owner && !d.revoked),
);
const latest = computed(
  () =>
    [...mine.value].sort(
      (a, b) => Date.parse(b.lastSeen) - Date.parse(a.lastSeen),
    )[0],
);
/** No Obsidian of the caller's signed in yet: the plugin is the first thing to do. */
const noSyncDevice = computed(
  () => devices.value !== null && !mine.value.some((d) => d.profile !== "web"),
);
const tenantsRun = computed(
  () =>
    new Set(
      managed.value.vaults.map((v) => v.tenant).concat(managed.value.tenants),
    ).size,
);

async function expiringKeys(): Promise<void> {
  const soon = Date.now() + SOON_MS;
  const found = await Promise.all(
    managed.value.vaults.map(async (vault) => {
      const keys = await withDevice(
        async (headers) =>
          (
            await listTokens({
              headers,
              path: { v: vault.id },
              throwOnError: true,
            })
          ).data,
      ).catch(() => []);
      const live = keys.filter(
        (k) => !k.revoked && Date.parse(k.expiresAt) <= soon,
      );
      const first = live.map((k) => k.expiresAt).sort()[0];
      return first ? { vault, n: live.length, at: first } : null;
    }),
  );
  expiring.value = found.filter((x) => x !== null);
}

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    // Without `/me` the vaults still show; only the management section waits on it.
    await Promise.all([
      vaultStore.load(true),
      meStore.load().catch(() => undefined),
    ]);
  } catch (e) {
    error.value = failure(e);
    canRetry.value = retryable(e);
  } finally {
    loading.value = false;
  }
  devices.value = await withDevice(
    async (headers) =>
      (await listDevices({ headers, throwOnError: true })).data,
  ).catch(() => null);
  await expiringKeys();
}
onMounted(load);
</script>

<template>
  <section class="flex max-w-6xl flex-col gap-10">
    <h1>{{ t("home.title") }}</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="false"
      empty-message=""
      :title="t('vaults.failed')"
      :can-retry="canRetry"
      @retry="load"
    >
      <div class="flex flex-col gap-10">
        <section aria-labelledby="todo" class="flex flex-col gap-2">
          <h2 id="todo" class="label">{{ t("home.todo") }}</h2>
          <ol class="border-t border-rule" data-test="todo">
            <li v-if="noSyncDevice" class="flex flex-wrap items-center gap-4 border-b border-dashed border-rule-soft py-4">
              <div class="min-w-0 flex-[1_1_22rem]">
                <p class="font-bold">{{ t("home.syncTitle") }}</p>
                <p class="text-ink-dim">{{ t("home.syncBody") }}</p>
              </div>
              <RouterLink to="/help#obsidian" class="action">{{ t("home.syncAction") }}</RouterLink>
            </li>
            <li v-for="x in expiring" :key="x.vault.id" class="flex flex-wrap items-center gap-4 border-b border-dashed border-rule-soft py-4">
              <div class="min-w-0 flex-[1_1_22rem]">
                <p class="font-bold">{{ t("home.keysTitle", { n: x.n, vault: x.vault.name, when: ago(x.at, locale) }, x.n) }}</p>
                <p class="text-ink-dim">{{ t("home.keysBody") }}</p>
              </div>
              <RouterLink :to="`/vaults/${x.vault.id}/keys`" class="ctl">{{ t("home.keysAction") }}</RouterLink>
            </li>
            <li v-if="!noSyncDevice && !expiring.length" class="border-b border-dashed border-rule-soft py-4 text-ink-dim">{{ t("home.nothing") }}</li>
          </ol>
        </section>

        <div class="flex flex-wrap items-start gap-10">
          <section aria-labelledby="your-vaults" class="flex min-w-0 flex-[999_1_30rem] flex-col gap-2">
            <h2 id="your-vaults" class="label">{{ t("home.vaults") }}</h2>
            <p v-if="!vaults.length" class="text-ink-dim">{{ t("vaults.empty") }}</p>
            <ul v-else class="border-t border-rule" data-test="vaults">
              <li v-for="v in vaults" :key="v.id" class="flex flex-wrap items-baseline gap-x-5 gap-y-1 border-b border-dashed border-rule-soft py-3.5">
                <RouterLink :to="`/vaults/${v.id}`" class="min-w-0 flex-[1_1_9rem] text-[length:var(--t-row)] font-bold">{{ v.name }}</RouterLink>
                <span class="min-w-0 flex-[1_1_14rem] text-ink-dim">
                  {{ v.tenant }} · {{ t(`role.${v.role}`) }} ·
                  <time v-if="v.updatedAt" :datetime="v.updatedAt" :title="when(v.updatedAt, locale)">{{ ago(v.updatedAt, locale) }}</time>
                  <template v-else>{{ t("home.emptyVault") }}</template>
                  <template v-if="v.paths"> · {{ t("reader.reach", { n: v.paths.length }) }}</template>
                </span>
                <span class="flex gap-4">
                  <RouterLink :to="`/vaults/${v.id}/notes`" class="text-ink-dim hover:text-ink">{{ t("home.read") }}</RouterLink>
                  <a v-if="!v.paths && setupLink(origin, v)" :href="setupLink(origin, v) ?? undefined" :aria-label="t('vaults.obsidianFor', { name: v.name })">{{ t("vaults.obsidian") }}</a>
                </span>
              </li>
            </ul>
          </section>

          <aside :aria-label="t('home.brief')" class="flex min-w-0 flex-[1_1_16rem] flex-col gap-7">
            <section aria-labelledby="your-devices" class="flex flex-col gap-1.5">
              <h2 id="your-devices" class="label">{{ t("home.devices") }}</h2>
              <p v-if="devices" class="flex items-center gap-3">
                <StateMark kind="live" :word="t('home.active', { n: mine.length }, mine.length)" />
              </p>
              <p v-if="latest" class="text-ink-dim">{{ t("home.latest", { name: latest.name, when: ago(latest.lastSeen, locale) }) }}</p>
              <RouterLink to="/devices">{{ t("home.devicesAction") }}</RouterLink>
            </section>
            <section v-if="manages" aria-labelledby="you-run" class="flex flex-col gap-1.5 border-t border-dashed border-rule-soft pt-5" data-test="manage">
              <h2 id="you-run" class="label">{{ t("home.run") }}</h2>
              <p class="text-ink-dim">{{ t("home.runCount", { tenants: tenantsRun, vaults: managed.vaults.length }) }}</p>
              <ul class="flex flex-col gap-1">
                <li v-for="slug in managed.tenants" :key="`v-${slug}`"><RouterLink :to="`/manage/tenants/${slug}/vaults`">{{ t("home.tenantVaults", { tenant: slug }) }}</RouterLink></li>
                <li v-for="slug in managed.tenants" :key="`n-${slug}`"><RouterLink :to="`/manage/tenants/${slug}/notifications`">{{ t("home.tenantNotifications", { tenant: slug }) }}</RouterLink></li>
                <li v-if="managed.tenants.length"><RouterLink to="/manage/devices">{{ t("manage.devices") }}</RouterLink></li>
                <li v-if="managed.platform"><RouterLink to="/manage/status">{{ t("manage.status") }}</RouterLink></li>
              </ul>
            </section>
          </aside>
        </div>
      </div>
    </AsyncState>
  </section>
</template>
