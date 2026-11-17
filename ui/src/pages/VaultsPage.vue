<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { withDevice } from "@/api/client";
import { listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";
import AsyncState from "@/components/ui/AsyncState.vue";
import { useAutoRetry } from "@/reader/retry";
import { ago, day } from "@/reader/time";

const { t, locale } = useI18n();
const vaults = ref<Vault[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const failure = useAutoRetry(() => load());

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    vaults.value = await withDevice(
      async (headers) =>
        (await listVaults({ headers, throwOnError: true })).data,
    );
  } catch (e) {
    error.value = failure(e);
  } finally {
    loading.value = false;
  }
}
onMounted(load);
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("vaults.title") }}</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="vaults.length === 0"
      :empty-message="t('vaults.empty')"
      :title="t('vaults.failed')"
      @retry="load"
    >
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("vaults.name") }}</th>
            <th scope="col">{{ t("vaults.tenant") }}</th>
            <th scope="col">{{ t("vaults.role") }}</th>
            <th scope="col">{{ t("vaults.reach") }}</th>
            <th scope="col">{{ t("vaults.updated") }}</th>
            <th scope="col"><span class="sr-only">{{ t("manage.title") }}</span></th>
            <th scope="col">{{ t("vaults.source") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="v in vaults" :key="v.id" class="border-t border-rule-soft">
            <td><RouterLink :to="`/vaults/${v.id}`" class="underline">{{ v.name }}</RouterLink></td>
            <td>{{ v.tenant }}</td>
            <td>{{ v.role }}</td>
            <td>{{ v.paths ? t("reader.reach", { n: v.paths.length }) : t("reader.whole") }}</td>
            <td>
              <time v-if="v.updatedAt" :datetime="v.updatedAt" :title="day(v.updatedAt, locale)">{{ ago(v.updatedAt, locale) }}</time>
            </td>
            <td>
              <RouterLink v-if="v.role === 'manager'" :to="`/manage/vaults/${v.id}/access`" class="underline">
                {{ t("manage.access") }}
              </RouterLink>
            </td>
            <td>{{ v.managedBy }}</td>
          </tr>
        </tbody>
      </table>
    </AsyncState>
  </section>
</template>
