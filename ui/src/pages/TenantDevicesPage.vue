<script setup lang="ts">
/** Other members' devices in the tenants the caller administers (RFC 0003 §4.5). */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { listDevices, revokeDevice } from "@/api/generated/sdk.gen";
import type { Device } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { useLoad } from "@/manage/useLoad";

const { t } = useI18n();
const {
  data: all,
  loading,
  error,
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
    <h1 class="mb-4">{{ t("manage.devices") }}</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="others.length === 0"
      :empty-message="t('tenantDevices.empty')"
      :title="t('manage.failed')"
      @retry="load"
    >
      <p v-if="refusal" role="alert" class="mb-2">{{ refusal }}</p>
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
          <tr v-for="d in others" :key="d.id" class="border-t border-rule-soft">
            <td>{{ d.owner }}</td>
            <td>{{ d.name }}</td>
            <td>{{ d.profile }}</td>
            <td>{{ d.lastSeen.slice(0, 16).replace("T", " ") }}</td>
            <td>{{ d.revoked ? t("devices.revoked") : t("devices.active") }}</td>
            <td>
              <button v-if="!d.revoked" type="button" class="border border-accent px-2" @click="confirming = d">
                {{ t("devices.revoke") }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-if="confirming" role="alertdialog" class="mt-4 border border-accent p-4">
        <p>{{ t("devices.confirmRevoke", { name: confirming.name }) }}</p>
        <button type="button" class="mr-2 bg-accent px-3 text-accent-ink" @click="revoke(confirming)">{{ t("devices.confirm") }}</button>
        <button type="button" class="border border-rule px-3" @click="confirming = null">{{ t("devices.cancel") }}</button>
      </div>
    </AsyncState>
  </section>
</template>
