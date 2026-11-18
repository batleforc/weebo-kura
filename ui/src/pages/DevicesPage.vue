<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { signOut, withDevice } from "@/api/client";
import { listDevices, revokeDevice } from "@/api/generated/sdk.gen";
import type { Device, DeviceVault } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import { useSession } from "@/auth/session";
import AsyncState from "@/components/ui/AsyncState.vue";
import ConfirmRow from "@/components/ui/ConfirmRow.vue";
import RevokedToggle from "@/components/ui/RevokedToggle.vue";
import StateMark from "@/components/ui/StateMark.vue";
import TableScroll from "@/components/ui/TableScroll.vue";
import { ago, when } from "@/reader/time";
import { useVaults } from "@/vaults";

const { t, locale } = useI18n();
const router = useRouter();
const session = useSession();
const vaultStore = useVaults();
const DAY = 86_400_000;
const devices = ref<Device[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
// Others' devices (they carry an owner) are a tenant admin's, under Manage.
const mine = computed(() => devices.value.filter((d) => !d.owner));
const showRevoked = ref(false);
const revokedCount = computed(() => mine.value.filter((d) => d.revoked).length);
// Every device revoked and folded: say so, not a table of headers only.
const allRevoked = computed(
  () =>
    !showRevoked.value &&
    mine.value.length > 0 &&
    revokedCount.value === mine.value.length,
);
const shown = computed(() =>
  showRevoked.value ? mine.value : mine.value.filter((d) => !d.revoked),
);
/** The device a revocation waits to be confirmed for. */
const confirming = ref<Device | null>(null);

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    devices.value = await withDevice(
      async (headers) =>
        (await listDevices({ headers, throwOnError: true })).data,
    );
  } catch (e) {
    error.value = messageOf(e);
  } finally {
    loading.value = false;
  }
}

async function revoke(d: Device): Promise<void> {
  confirming.value = null;
  try {
    await withDevice((headers) =>
      revokeDevice({ headers, path: { d: d.id }, throwOnError: true }),
    );
    await load();
  } catch (e) {
    error.value = messageOf(e);
  }
}

async function signOutHere(): Promise<void> {
  await signOut();
  await router.replace({ name: "signin" });
}

/** A vault by name; its own date when it lags the device by over a day (RFC 0006 §4.5). */
function followed(
  d: Device,
  f: DeviceVault,
): { name: string; lag: string | null } {
  const v = vaultStore.list.find((x) => x.id === f.vault);
  const lagging = Date.parse(d.lastSeen) - Date.parse(f.caughtUpAt) > DAY;
  return {
    name: v ? v.name : f.vault.slice(0, 8),
    lag: lagging ? ago(f.caughtUpAt, locale.value) : null,
  };
}

function state(d: Device): string {
  if (d.revoked) return t("devices.revoked");
  return d.id === session.device()
    ? t("devices.thisBrowser")
    : t("devices.active");
}
onMounted(() => {
  void vaultStore.load().catch(() => undefined);
  void load();
});
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("devices.title") }}</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="mine.length === 0"
      :empty-message="t('devices.empty')"
      :title="t('devices.failed')"
      @retry="load"
    >
      <p v-if="allRevoked" class="text-ink-dim" data-test="no-live">{{ t("state.noLive", { n: revokedCount }, revokedCount) }}</p>
      <TableScroll v-else :label="t('state.tableRegion')">
      <table class="acts w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("devices.name") }}</th>
            <th scope="col">{{ t("devices.kind") }}</th>
            <th scope="col">{{ t("devices.lastSeen") }}</th>
            <th scope="col">{{ t("sync.vaults") }}</th>
            <th scope="col">{{ t("devices.state") }}</th>
            <th scope="col"><span class="sr-only">{{ t("devices.revoke") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <template v-for="d in shown" :key="d.id">
          <tr class="border-t border-rule-soft" :data-device="d.id">
            <td>{{ d.name }}</td>
            <td>{{ t(`devices.profiles.${d.profile}`) }}</td>
            <td><time :datetime="d.lastSeen" :title="when(d.lastSeen, locale)">{{ ago(d.lastSeen, locale) }}</time></td>
            <td data-test="vaults">
              <template v-for="(f, i) in d.vaults ?? []" :key="f.vault"><template v-if="i">, </template>{{ followed(d, f).name }}<span v-if="followed(d, f).lag" class="text-ink-dim"> ({{ followed(d, f).lag }})</span></template>
              <template v-if="!d.vaults?.length">—</template>
            </td>
            <td><StateMark :kind="d.revoked ? 'gone' : 'live'" :word="state(d)" /></td>
            <td>
              <button
                v-if="!d.revoked && d.id === session.device()"
                type="button"
                class="border border-rule px-2"
                data-test="sign-out-here"
                @click="signOutHere"
              >
                {{ t("devices.signOutHere") }}
              </button>
              <button
                v-else-if="!d.revoked"
                type="button"
                class="border border-accent px-2"
                data-test="revoke"
                @click="confirming = d"
              >
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
      </TableScroll>
      <RevokedToggle v-model="showRevoked" :count="revokedCount" />
    </AsyncState>
  </section>
</template>
