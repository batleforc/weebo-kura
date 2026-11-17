<script setup lang="ts">
/** Platform status (RFC 0003 §4.5): `GET /v1/admin/status` as a table, rejection first. */
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { adminStatus } from "@/api/generated/sdk.gen";
import type { AdminStatus } from "@/api/generated/types.gen";
import AsyncState from "@/components/ui/AsyncState.vue";
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import { useLoad } from "@/manage/useLoad";

interface Row {
  kind: string;
  name: string;
  ready: string;
  reasons: string;
}

const { t } = useI18n();
const {
  data: status,
  loading,
  error,
  forbidden,
  forbiddenDetail,
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
        .map((c) =>
          [text(c, "reason"), text(c, "message")].filter(Boolean).join(": "),
        )
        .join("; "),
    };
  }),
);
onMounted(load);
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("manage.status") }}</h1>
    <ForbiddenState v-if="forbidden" :detail="forbiddenDetail" />
    <AsyncState v-else :loading="loading" :error="error" :empty="false" empty-message="" :title="t('manage.failed')" @retry="load">
      <div v-if="status?.rejection.length" role="alert" class="mb-4 border border-accent p-3" data-test="rejection">
        <p class="font-bold">{{ t("status.rejected") }}</p>
        <p v-for="r in status.rejection" :key="r"><code>{{ r }}</code></p>
      </div>
      <p class="mb-2 text-ink-dim">
        {{ t("status.generation", { generation: (status?.generation ?? "").slice(0, 8), at: status?.writtenAt ?? "" }) }}
      </p>
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
            <td>{{ r.kind }}</td>
            <td>{{ r.name }}</td>
            <td :class="r.ready === 'True' ? '' : 'font-bold text-accent'">{{ r.ready }}</td>
            <td>{{ r.reasons }}</td>
          </tr>
        </tbody>
      </table>
    </AsyncState>
  </section>
</template>
