<script setup lang="ts">
/**
 * The devices syncing a vault (RFC 0006 §4.5): every one, owners named, for
 * its managers; the caller's own for anyone else. The server decides which.
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";

import { withDevice } from "@/api/client";
import { vaultSync } from "@/api/generated/sdk.gen";
import type { DeviceSync } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import RevokedToggle from "@/components/ui/RevokedToggle.vue";
import StateMark from "@/components/ui/StateMark.vue";
import TableFilter from "@/components/ui/TableFilter.vue";
import TableScroll from "@/components/ui/TableScroll.vue";
import { matching } from "@/filter";
import { personLabel } from "@/people";
import { ago, when } from "@/reader/time";
import { byDistance, leftovers, type Standing, standing } from "@/sync";

const props = defineProps<{ vault: string; manages: boolean }>();
const { t, locale } = useI18n();
const rows = ref<DeviceSync[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const showRevoked = ref(false);
const query = ref("");
const revokedCount = computed(() => rows.value.filter((d) => d.revoked).length);
// Every device revoked and folded: say so, not a table of headers only.
const allRevoked = computed(
  () =>
    !showRevoked.value &&
    rows.value.length > 0 &&
    revokedCount.value === rows.value.length,
);
const shown = computed(() =>
  byDistance(
    matching(
      showRevoked.value ? rows.value : rows.value.filter((d) => !d.revoked),
      query.value,
      (d) => [d.name, d.owner ?? "", d.ownerName ?? ""],
    ),
  ),
);

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    rows.value = await withDevice(
      async (headers) =>
        (
          await vaultSync({
            headers,
            path: { v: props.vault },
            throwOnError: true,
          })
        ).data,
    );
  } catch (e) {
    error.value = messageOf(e);
  } finally {
    loading.value = false;
  }
}

function person(d: DeviceSync): string {
  return personLabel(d.owner ?? "", d.ownerName, (id) =>
    t("people.unnamed", { id }),
  ).label;
}

const marks: Record<Standing["kind"], "live" | "waiting" | "off" | "refused"> =
  {
    current: "live",
    behind: "waiting",
    never: "off",
    resync: "refused",
  };

function word(s: Standing): string {
  switch (s.kind) {
    case "current":
      return t("sync.current");
    case "never":
      return t("sync.never");
    case "resync":
      return t("sync.resync");
    case "behind":
      return s.more
        ? t("sync.behindMore", { n: s.n })
        : t("sync.behind", { n: s.n }, s.n);
  }
}

onMounted(load);
</script>

<template>
  <section aria-labelledby="devices" class="col-span-full flex flex-col gap-2">
    <h2 id="devices" class="label border-b border-rule pb-2">{{ manages ? t("sync.title") : t("sync.mine") }}</h2>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="rows.length === 0"
      :empty-message="t('sync.empty')"
      :title="t('sync.failed')"
      @retry="load"
    >
      <TableFilter v-if="manages && !allRevoked" v-model="query" :empty="shown.length === 0" />
      <p v-if="allRevoked" class="text-ink-dim" data-test="no-live">{{ t("state.noLive", { n: revokedCount }, revokedCount) }}</p>
      <TableScroll v-else :label="t('state.tableRegion')">
      <table class="w-full text-left" data-test="sync">
        <thead>
          <tr>
            <th scope="col">{{ t("sync.device") }}</th>
            <th v-if="manages" scope="col">{{ t("devices.owner") }}</th>
            <th scope="col">{{ t("sync.caughtUp") }}</th>
            <th scope="col">{{ t("sync.behindCol") }}</th>
            <th scope="col">{{ t("sync.lastRun") }}</th>
            <th v-if="manages" scope="col">{{ t("sync.pushed") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="d in shown" :key="d.device" class="border-t border-rule-soft" :data-device="d.device">
            <td>{{ d.name }}<span v-if="d.revoked" class="ml-2 text-ink-dim">({{ t("devices.revoked") }})</span></td>
            <td v-if="manages" :title="d.owner ?? undefined">{{ d.owner ? person(d) : t("sync.you") }}</td>
            <td>
              <time v-if="d.caughtUpAt" :datetime="d.caughtUpAt" :title="when(d.caughtUpAt, locale)">{{ ago(d.caughtUpAt, locale) }}</time>
              <template v-else>—</template>
            </td>
            <td><StateMark :kind="marks[standing(d).kind]" :word="word(standing(d))" /></td>
            <td data-test="last-run">
              <template v-if="!d.report">—</template>
              <span v-else-if="!leftovers(d.report).length" :title="when(d.report.at, locale)">{{ t("sync.clean") }}</span>
              <span v-else :title="when(d.report.at, locale)" class="text-copper">{{ leftovers(d.report).map((x) => t(`sync.left.${x.key}`, { n: x.n }, x.n)).join(" · ") }}</span>
            </td>
            <td v-if="manages">
              <time v-if="d.pushedAt" :datetime="d.pushedAt" :title="when(d.pushedAt, locale)">{{ ago(d.pushedAt, locale) }}</time>
              <template v-else>—</template>
            </td>
          </tr>
        </tbody>
      </table>
      </TableScroll>
      <p class="text-ink-dim">{{ t("sync.handed") }}</p>
      <RevokedToggle v-model="showRevoked" :count="revokedCount" />
    </AsyncState>
  </section>
</template>
