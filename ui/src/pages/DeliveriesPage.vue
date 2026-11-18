<script setup lang="ts">
/** A rule's last 50 deliveries (RFC 0003 §4.5): coarse results only. */
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { listDeliveries } from "@/api/generated/sdk.gen";
import type { DeliveryView } from "@/api/generated/types.gen";
import AsyncState from "@/components/ui/AsyncState.vue";
import BreadCrumbs from "@/components/ui/BreadCrumbs.vue";
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import { useLoad } from "@/manage/useLoad";
import { when } from "@/reader/time";

const { t, locale } = useI18n();
const route = useRoute();
const tenant = computed(() => String(route.params.t));
const rule = computed(() => String(route.params.n));
const {
  data: deliveries,
  loading,
  error,
  forbidden,
  forbiddenDetail,
  canRetry,
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
    <BreadCrumbs :items="[{ label: t('home.title'), to: '/vaults' }, { label: tenant }, { label: t('manage.notifications'), to: `/manage/tenants/${tenant}/notifications` }]" />
    <h1 class="mb-4">{{ t("notifications.deliveriesTitle") }}</h1>
    <ForbiddenState v-if="forbidden" :detail="forbiddenDetail" />
    <AsyncState v-else :loading="loading" :error="error" :empty="deliveries.length === 0" :empty-message="t('notifications.noDelivery')" :title="t('manage.failed')" :can-retry="canRetry" @retry="load">
      <div class="relative overflow-x-auto" tabindex="0">
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
            <td><time :datetime="d.at">{{ when(d.at, locale) }}</time></td>
            <td>{{ d.event }}</td>
            <td>{{ d.attempts }}</td>
            <td>{{ d.status === "failed" || d.status === "pending" ? t(`notifications.${d.status}`, { result: d.result ?? "" }) : d.result }}</td>
          </tr>
        </tbody>
      </table>
      </div>
    </AsyncState>
  </section>
</template>
