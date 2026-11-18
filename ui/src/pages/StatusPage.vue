<script setup lang="ts">
/** Platform status (RFC 0003 §4.5): `GET /v1/admin/status` as a table, rejection first. */
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { adminStatus } from "@/api/generated/sdk.gen";
import type { AdminStatus } from "@/api/generated/types.gen";
import AsyncState from "@/components/ui/AsyncState.vue";
import BreadCrumbs from "@/components/ui/BreadCrumbs.vue";
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import StateMark from "@/components/ui/StateMark.vue";
import TableScroll from "@/components/ui/TableScroll.vue";
import { useLoad } from "@/manage/useLoad";
import { when } from "@/reader/time";

interface Row {
  kind: string;
  name: string;
  ready: string;
  /** Each condition that is not "ready", its code and the server's message. */
  reasons: { code: string; message: string }[];
}

const { t, te, locale } = useI18n();

/** A reason code in words, else the code as it came (a newer server). */
function reason(code: string): string {
  return te(`status.reasons.${code}`) ? t(`status.reasons.${code}`) : code;
}
function kind(k: string): string {
  return te(`status.kinds.${k}`) ? t(`status.kinds.${k}`) : k;
}
const {
  data: status,
  loading,
  error,
  forbidden,
  forbiddenDetail,
  canRetry,
  load,
} = useLoad<AdminStatus | null>(
  () =>
    withDevice(
      async (headers) =>
        (await adminStatus({ headers, throwOnError: true })).data,
    ),
  null,
);
const text = (o: object, k: string) => {
  const v: unknown = Reflect.get(o, k);
  return typeof v === "string" ? v : "";
};
/** The objects are a loose shape in the spec: read field by field. */
const rows = computed<Row[]>(() =>
  (status.value?.objects ?? []).map((o) => {
    const conditions: unknown = Reflect.get(o, "conditions");
    const list = Array.isArray(conditions)
      ? conditions.filter(
          (c): c is object => typeof c === "object" && c !== null,
        )
      : [];
    const ready = list.find((c) => text(c, "type") === "Ready");
    const ns = text(o, "namespace");
    return {
      kind: text(o, "kind"),
      name: ns ? `${ns}/${text(o, "name")}` : text(o, "name"),
      ready: ready ? text(ready, "status") : "",
      reasons: list
        .filter(
          (c) => text(c, "status") !== "True" || text(c, "type") !== "Ready",
        )
        .map((c) => ({ code: text(c, "reason"), message: text(c, "message") })),
    };
  }),
);
onMounted(load);
</script>

<template>
  <section>
    <BreadCrumbs :items="[{ label: t('home.title'), to: '/vaults' }]" />
    <h1 class="mb-4">{{ t("manage.status") }}</h1>
    <ForbiddenState v-if="forbidden" :detail="forbiddenDetail" />
    <AsyncState v-else :loading="loading" :error="error" :empty="false" empty-message="" :title="t('manage.failed')" :can-retry="canRetry" @retry="load">
      <div v-if="status?.rejection.length" role="alert" class="mb-4 border border-accent p-3" data-test="rejection">
        <p class="font-bold">{{ t("status.rejected") }}</p>
        <p v-for="r in status.rejection" :key="r"><code>{{ r }}</code></p>
      </div>
      <p class="mb-2 text-ink-dim" :title="status?.generation ?? undefined">
        {{ t("status.applied", { at: status?.writtenAt ? when(status.writtenAt, locale) : "" }) }}
      </p>
      <TableScroll :label="t('state.tableRegion')">
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("status.kind") }}</th>
            <th scope="col">{{ t("vaults.name") }}</th>
            <th scope="col">{{ t("status.ready") }}</th>
            <th scope="col">{{ t("status.reason") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rows" :key="`${r.kind}/${r.name}`" class="border-t border-rule-soft">
            <td>{{ kind(r.kind) }}</td>
            <td>{{ r.name }}</td>
            <td><StateMark :kind="r.ready === 'True' ? 'live' : r.ready === 'False' ? 'refused' : 'off'" :word="r.ready === 'True' ? t('status.isReady') : r.ready === 'False' ? t('status.notReady') : t('status.unknown')" /></td>
            <td class="whitespace-normal">
              <p v-for="x in r.reasons" :key="x.code + x.message">
                {{ reason(x.code) }}<span v-if="x.message" class="text-ink-dim" lang="en"> — {{ x.message }}</span>
              </p>
            </td>
          </tr>
        </tbody>
      </table>
      </TableScroll>
    </AsyncState>
  </section>
</template>
