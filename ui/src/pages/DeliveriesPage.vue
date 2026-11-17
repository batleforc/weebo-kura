<script setup lang="ts">
/** A rule's last 50 deliveries (RFC 0003 §4.5): coarse results only. */
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { listDeliveries } from "@/api/generated/sdk.gen";
import type { DeliveryView } from "@/api/generated/types.gen";
import AsyncState from "@/components/ui/AsyncState.vue";
import { useLoad } from "@/manage/useLoad";

const { t } = useI18n();
const route = useRoute();
const tenant = computed(() => String(route.params.t));
const rule = computed(() => String(route.params.n));
const {
  data: deliveries,
  loading,
  error,
  load,
} = useLoad<DeliveryView[]>(
  () =>
    withDevice(
      async (headers) =>
        (
          await listDeliveries({
            headers,
            path: { t: tenant.value, n: rule.value },
            throwOnError: true,
          })
        ).data,
    ),
  [],
);
onMounted(load);
</script>

<template>
  <section>
    <p class="mb-3"><RouterLink :to="`/manage/tenants/${tenant}/notifications`" class="underline">{{ t("notifications.back") }}</RouterLink></p>
    <h1 class="mb-4">{{ t("notifications.deliveriesTitle") }}</h1>
    <AsyncState :loading="loading" :error="error" :empty="deliveries.length === 0" :empty-message="t('notifications.noDelivery')" :title="t('manage.failed')" @retry="load">
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("notifications.time") }}</th>
            <th scope="col">{{ t("notifications.event") }}</th>
            <th scope="col">{{ t("notifications.attempt") }}</th>
            <th scope="col">{{ t("notifications.result") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="d in deliveries" :key="d.id" class="border-t border-rule-soft">
            <td>{{ d.at.slice(0, 19).replace("T", " ") }}</td>
            <td>{{ d.event }}</td>
            <td>{{ d.attempts }}</td>
            <td>{{ d.status === "failed" ? `failed: ${d.result ?? ""}` : d.status === "pending" ? `pending (${d.result ?? ""})` : d.result }}</td>
          </tr>
        </tbody>
      </table>
    </AsyncState>
  </section>
</template>
