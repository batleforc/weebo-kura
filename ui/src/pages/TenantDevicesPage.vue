<script setup lang="ts">
/** Other members' devices in the tenants the caller administers (RFC 0003 §4.5). */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { listDevices, revokeDevice } from "@/api/generated/sdk.gen";
import type { Device } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import BreadCrumbs from "@/components/ui/BreadCrumbs.vue";
import ConfirmRow from "@/components/ui/ConfirmRow.vue";
import RevokedToggle from "@/components/ui/RevokedToggle.vue";
import StateMark from "@/components/ui/StateMark.vue";
import { useLoad } from "@/manage/useLoad";
import { ago, when } from "@/reader/time";

const { t, locale } = useI18n();
const {
  data: all,
  loading,
  error,
  canRetry,
  load,
} = useLoad<Device[]>(
  () =>
    withDevice(
      async (headers) =>
        (await listDevices({ headers, throwOnError: true })).data,
    ),
  [],
);
// The server names the owner only of others' devices, and only to an admin.
const others = computed(() => all.value.filter((d) => d.owner));
const showRevoked = ref(false);
const revokedCount = computed(
  () => others.value.filter((d) => d.revoked).length,
);
const shown = computed(() =>
  showRevoked.value ? others.value : others.value.filter((d) => !d.revoked),
);
const refusal = ref<string | null>(null);
const confirming = ref<Device | null>(null);

async function revoke(d: Device): Promise<void> {
  confirming.value = null;
  try {
    await withDevice((headers) =>
      revokeDevice({ headers, path: { d: d.id }, throwOnError: true }),
    );
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}
onMounted(load);
</script>

<template>
  <section>
    <BreadCrumbs :items="[{ label: t('home.title'), to: '/vaults' }]" />
    <h1 class="mb-4">{{ t("manage.devices") }}</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="others.length === 0"
      :empty-message="t('tenantDevices.empty')"
      :title="t('manage.failed')"
      :can-retry="canRetry" @retry="load"
    >
      <p v-if="refusal" role="alert" class="mb-2 border-l border-rule pl-3">{{ refusal }}</p>
      <div class="relative overflow-x-auto" tabindex="0">
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("devices.owner") }}</th>
            <th scope="col">{{ t("devices.name") }}</th>
            <th scope="col">{{ t("devices.kind") }}</th>
            <th scope="col">{{ t("devices.lastSeen") }}</th>
            <th scope="col">{{ t("devices.state") }}</th>
            <th scope="col"><span class="sr-only">{{ t("devices.revoke") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <template v-for="d in shown" :key="d.id">
          <tr class="border-t border-rule-soft">
            <td>{{ d.owner }}</td>
            <td>{{ d.name }}</td>
            <td>{{ d.profile }}</td>
            <td><time :datetime="d.lastSeen" :title="when(d.lastSeen, locale)">{{ ago(d.lastSeen, locale) }}</time></td>
            <td><StateMark :kind="d.revoked ? 'gone' : 'live'" :word="d.revoked ? t('devices.revoked') : t('devices.active')" /></td>
            <td>
              <button v-if="!d.revoked" type="button" class="border border-accent px-2" @click="confirming = d">
                {{ t("devices.revoke") }}
              </button>
            </td>
          </tr>
          <ConfirmRow
            v-if="confirming?.id === d.id"
            :colspan="6"
            :question="t('devices.confirmRevoke', { name: d.name })"
            :action="t('devices.confirm')"
            @confirm="revoke(d)"
            @cancel="confirming = null"
          />
          </template>
        </tbody>
      </table>
      </div>
      <RevokedToggle v-model="showRevoked" :count="revokedCount" />
    </AsyncState>
  </section>
</template>
