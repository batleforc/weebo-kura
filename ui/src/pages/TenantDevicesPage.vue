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
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import RevokedToggle from "@/components/ui/RevokedToggle.vue";
import StateMark from "@/components/ui/StateMark.vue";
import TableFilter from "@/components/ui/TableFilter.vue";
import TableScroll from "@/components/ui/TableScroll.vue";
import { matching } from "@/filter";
import { useLoad } from "@/manage/useLoad";
import { useMe } from "@/me";
import { personLabel } from "@/people";
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
const others = computed(() =>
  all.value
    .filter((d) => d.owner)
    // Grouped by person, so one person's devices sit together.
    .sort(
      (a, b) =>
        owner(a).label.localeCompare(owner(b).label) ||
        a.name.localeCompare(b.name),
    ),
);

function owner(d: Device): { label: string; title: string } {
  return personLabel(d.owner ?? "", d.ownerName, (id) =>
    t("people.unnamed", { id }),
  );
}
const showRevoked = ref(false);
const revokedCount = computed(
  () => others.value.filter((d) => d.revoked).length,
);
// Every device revoked and folded: say so, not a table of headers only.
const allRevoked = computed(
  () =>
    !showRevoked.value &&
    others.value.length > 0 &&
    revokedCount.value === others.value.length,
);
const query = ref("");
const shown = computed(() =>
  matching(
    showRevoked.value ? others.value : others.value.filter((d) => !d.revoked),
    query.value,
    (d) => [d.owner ?? "", d.ownerName ?? "", d.name],
  ),
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
// Members' devices are a tenant admin's page: a member is told so.
const meStore = useMe();
const admin = computed(
  () =>
    meStore.me === null ||
    meStore.me.tenants.some((x) => x.role === "admin" || x.role === "owner"),
);
onMounted(() => {
  void meStore.load().catch(() => undefined);
  void load();
});
</script>

<template>
  <section>
    <BreadCrumbs :items="[{ label: t('home.title'), to: '/vaults' }]" />
    <h1 class="mb-4">{{ t("manage.devices") }}</h1>
    <ForbiddenState v-if="!admin" />
    <AsyncState
      v-else
      :loading="loading"
      :error="error"
      :empty="others.length === 0"
      :empty-message="t('tenantDevices.empty')"
      :title="t('manage.failed')"
      :can-retry="canRetry" @retry="load"
    >
      <p v-if="refusal" role="alert" class="mb-2 border-l border-rule pl-3">{{ refusal }}</p>
      <TableFilter v-if="!allRevoked" v-model="query" :empty="shown.length === 0" />
      <p v-if="allRevoked" class="text-ink-dim" data-test="no-live">{{ t("state.noLive", { n: revokedCount }, revokedCount) }}</p>
      <TableScroll v-else :label="t('state.tableRegion')">
      <table class="acts w-full text-left">
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
            <td :title="owner(d).title">{{ owner(d).label }}</td>
            <td>{{ d.name }}</td>
            <td>{{ t(`devices.profiles.${d.profile}`) }}</td>
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
            :question="t('devices.confirmRevokeOf', { name: d.name, owner: owner(d).label })"
            :action="t('devices.confirm')"
            @confirm="revoke(d)"
            @cancel="confirming = null"
          />
          </template>
        </tbody>
      </table>
      </TableScroll>
      <RevokedToggle v-model="showRevoked" :count="revokedCount" />
    </AsyncState>
  </section>
</template>
