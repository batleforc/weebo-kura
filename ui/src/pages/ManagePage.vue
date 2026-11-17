<script setup lang="ts">
/** The back office's index (RFC 0003 §4.5): grouped by what it acts on. */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";
import { targets } from "@/manage/targets";
import { useMe } from "@/me";

const { t } = useI18n();
const meStore = useMe();
const vaults = ref<Vault[]>([]);
const managed = computed(() => targets(meStore.me, vaults.value));

onMounted(async () => {
  await meStore.load();
  vaults.value = await withDevice(
    async (headers) => (await listVaults({ headers, throwOnError: true })).data,
  );
});
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("manage.title") }}</h1>
    <p v-if="!managed.vaults.length && !managed.tenants.length && !managed.platform" class="text-ink-dim">
      {{ t("manage.nothing") }}
    </p>
    <h2 v-if="managed.vaults.length" class="mt-4 font-bold">{{ t("manage.vaults") }}</h2>
    <ul>
      <li v-for="v in managed.vaults" :key="v.id" class="flex gap-4">
        <span class="min-w-40">{{ v.tenant }} ▸ {{ v.name }}</span>
        <RouterLink :to="`/manage/vaults/${v.id}/access`" class="underline">{{ t("manage.access") }}</RouterLink>
        <RouterLink :to="`/manage/vaults/${v.id}/keys`" class="underline">{{ t("manage.keys") }}</RouterLink>
      </li>
    </ul>
    <h2 v-if="managed.tenants.length" class="mt-4 font-bold">{{ t("manage.tenants") }}</h2>
    <ul>
      <li v-for="slug in managed.tenants" :key="slug" class="flex gap-4">
        <span class="min-w-40">{{ slug }}</span>
        <RouterLink :to="`/manage/tenants/${slug}/vaults`" class="underline">{{ t("manage.tenantVaults") }}</RouterLink>
        <RouterLink to="/manage/devices" class="underline">{{ t("manage.devices") }}</RouterLink>
      </li>
    </ul>
    <template v-if="managed.platform">
      <h2 class="mt-4 font-bold">{{ t("manage.platform") }}</h2>
      <RouterLink to="/manage/status" class="underline">{{ t("manage.status") }}</RouterLink>
    </template>
  </section>
</template>
