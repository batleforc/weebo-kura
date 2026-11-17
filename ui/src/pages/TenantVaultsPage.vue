<script setup lang="ts">
/** A tenant's vaults, for its admins: create one, delete one made through the API. */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { createVault, deleteVault, listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { useLoad } from "@/manage/useLoad";

const { t } = useI18n();
const route = useRoute();
const tenant = computed(() => String(route.params.t));
const {
  data: all,
  loading,
  error,
  load,
} = useLoad<Vault[]>(
  () =>
    withDevice(
      async (headers) =>
        (await listVaults({ headers, throwOnError: true })).data,
    ),
  [],
);
const vaults = computed(() =>
  all.value.filter((v) => v.tenant === tenant.value),
);
const name = ref("");
const refusal = ref<string | null>(null);
const confirming = ref<Vault | null>(null);

async function create(): Promise<void> {
  refusal.value = null;
  try {
    await withDevice((headers) =>
      createVault({
        headers,
        body: { tenant: tenant.value, name: name.value.trim() },
        throwOnError: true,
      }),
    );
    name.value = "";
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

async function remove(v: Vault): Promise<void> {
  confirming.value = null;
  try {
    await withDevice((headers) =>
      deleteVault({ headers, path: { v: v.id }, throwOnError: true }),
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
    <h1 class="mb-4">{{ t("manage.tenantVaults") }} ({{ tenant }})</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="vaults.length === 0"
      :empty-message="t('vaults.empty')"
      :title="t('manage.failed')"
      @retry="load"
    >
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("vaults.name") }}</th>
            <th scope="col">{{ t("vaults.source") }}</th>
            <th scope="col"><span class="sr-only">{{ t("tenantVaults.delete") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="v in vaults" :key="v.id" class="border-t border-rule-soft">
            <td>{{ v.namespace }}/{{ v.name }}</td>
            <td>{{ v.managedBy === "api" ? "API" : t("access.configuration") }}</td>
            <td>
              <button v-if="v.managedBy === 'api'" type="button" class="border border-accent px-2" @click="confirming = v">
                {{ t("tenantVaults.delete") }}
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </AsyncState>
    <div v-if="confirming" role="alertdialog" class="mt-4 border border-accent p-4">
      <p>{{ t("tenantVaults.confirmDelete", { name: confirming.name }) }}</p>
      <button type="button" class="mr-2 bg-accent px-3 text-accent-ink" @click="remove(confirming)">{{ t("tenantVaults.delete") }}</button>
      <button type="button" class="border border-rule px-3" @click="confirming = null">{{ t("devices.cancel") }}</button>
    </div>
    <p v-if="refusal" role="alert" class="mt-2 border border-accent p-2">{{ refusal }}</p>
    <form class="mt-4 flex gap-2" @submit.prevent="create">
      <label>{{ t("tenantVaults.newName") }}
        <input v-model="name" required pattern="[a-z0-9]([-a-z0-9]*[a-z0-9])?" maxlength="63" class="ml-2 border border-rule bg-ground px-1" />
      </label>
      <button type="submit" class="border border-rule px-3">{{ t("tenantVaults.create") }}</button>
    </form>
  </section>
</template>
