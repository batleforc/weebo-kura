<script setup lang="ts">
/**
 * A tenant's notification rules (RFC 0003 §4.5): only the host of a URL is
 * shown again; a rule that switched itself off is a banner, not a silent
 * state; the signing secret is shown once, as a key's is.
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import {
  createRule,
  deleteRule,
  listRules,
  listVaults,
  testRule,
  updateRule,
} from "@/api/generated/sdk.gen";
import type { RuleView, Vault } from "@/api/generated/types.gen";
import { messageOf, problemType } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import { useLoad } from "@/manage/useLoad";
import { useMe } from "@/me";

/** The closed list of §4.2; tenant events are a tenant admin's. */
const EVENTS = [
  "token.created",
  "token.revoked",
  "token.expiring",
  "token.denied",
  "binding.changed",
  "binding.removed",
  "vault.changed",
  "vault.conflict",
  "device.registered",
  "device.revoked",
  "vault.created",
  "vault.deleted",
  "quota.high",
];
const TENANT_EVENTS = [
  "device.registered",
  "device.revoked",
  "vault.created",
  "vault.deleted",
  "quota.high",
];

const { t } = useI18n();
const route = useRoute();
const meStore = useMe();
const tenant = computed(() => String(route.params.t));
const disabled = ref(false);
const {
  data: rules,
  loading,
  error,
  forbidden,
  load,
} = useLoad<RuleView[]>(async () => {
  try {
    return await withDevice(
      async (headers) =>
        (
          await listRules({
            headers,
            path: { t: tenant.value },
            throwOnError: true,
          })
        ).data,
    );
  } catch (e) {
    if (problemType(e) === "notifications_disabled") {
      disabled.value = true;
      return [];
    }
    throw e;
  }
}, []);
const vaults = ref<Vault[]>([]);
const admin = computed(
  () =>
    meStore.me?.tenants.some(
      (x) => x.slug === tenant.value && x.role !== "member",
    ) ?? false,
);
const mine = computed(() =>
  vaults.value.filter(
    (v) => v.tenant === tenant.value && (admin.value || v.role === "manager"),
  ),
);
const offered = computed(() =>
  admin.value ? EVENTS : EVENTS.filter((e) => !TENANT_EVENTS.includes(e)),
);
const switchedOff = computed(() =>
  rules.value.filter((r) => !r.enabled && r.disabledReason),
);

const creating = ref(false);
const name = ref("");
const scope = ref<string>("");
const events = ref<string[]>([]);
const url = ref("");
const authorization = ref("");
const refusal = ref<string | null>(null);
const secret = ref<string | null>(null);
const tested = ref<{ rule: string; result: string } | null>(null);

function vaultName(id: string | null | undefined): string {
  if (!id) return t("notifications.tenantWide");
  return vaults.value.find((v) => v.id === id)?.name ?? id.slice(0, 8);
}

async function create(): Promise<void> {
  refusal.value = null;
  try {
    const { data } = await withDevice((headers) =>
      createRule({
        headers,
        path: { t: tenant.value },
        body: {
          name: name.value,
          events: events.value,
          vault: scope.value || null,
          channel: {
            type: "webhook",
            url: url.value,
            authorization: authorization.value || null,
          },
        },
        throwOnError: true,
      }),
    );
    secret.value = data.secret ?? null;
    creating.value = false;
    url.value = "";
    authorization.value = "";
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

async function test(r: RuleView): Promise<void> {
  try {
    const { data } = await withDevice((headers) =>
      testRule({
        headers,
        path: { t: tenant.value, n: r.id },
        throwOnError: true,
      }),
    );
    tested.value = { rule: r.name, result: data.result };
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

async function enable(r: RuleView, on: boolean): Promise<void> {
  try {
    await withDevice((headers) =>
      updateRule({
        headers,
        path: { t: tenant.value, n: r.id },
        body: { enabled: on },
        throwOnError: true,
      }),
    );
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

async function remove(r: RuleView): Promise<void> {
  try {
    await withDevice((headers) =>
      deleteRule({
        headers,
        path: { t: tenant.value, n: r.id },
        throwOnError: true,
      }),
    );
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

onMounted(async () => {
  await meStore.load().catch(() => undefined);
  vaults.value = await withDevice(
    async (headers) => (await listVaults({ headers, throwOnError: true })).data,
  ).catch(() => []);
  if (!admin.value) scope.value = mine.value[0]?.id ?? "";
  await load();
});
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("notifications.title", { tenant }) }}</h1>
    <p v-if="disabled" role="status" data-test="disabled">{{ t("notifications.off") }}</p>
    <p v-else-if="forbidden" role="alert" data-test="forbidden">{{ t("manage.forbidden") }}</p>
    <AsyncState v-else :loading="loading" :error="error" :empty="false" empty-message="" :title="t('manage.failed')" @retry="load">
      <p v-for="r in switchedOff" :key="`off-${r.id}`" role="alert" class="mb-2 border border-accent p-2" data-test="switched-off">
        {{ t("notifications.switchedOff", { name: r.name, reason: r.disabledReason }) }}
      </p>
      <div v-if="secret" role="dialog" aria-labelledby="rule-created" class="mb-4 border border-accent p-4" data-test="created">
        <h2 id="rule-created" class="font-bold">{{ t("notifications.created") }}</h2>
        <p class="my-2 break-all"><code data-test="secret">{{ secret }}</code></p>
        <p class="my-2">{{ t("notifications.once") }}</p>
        <button type="button" class="bg-accent px-3 text-accent-ink" data-test="close" @click="secret = null">{{ t("keys.stored") }}</button>
      </div>
      <p v-if="tested" role="status" class="mb-2" data-test="tested">{{ t("notifications.tested", tested) }}</p>
      <p v-if="refusal" role="alert" class="mb-2 border border-accent p-2">{{ refusal }}</p>
      <p v-if="rules.length === 0" class="text-ink-dim">{{ t("notifications.empty") }}</p>
      <table v-else class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("keys.name") }}</th>
            <th scope="col">{{ t("notifications.events") }}</th>
            <th scope="col">{{ t("notifications.vault") }}</th>
            <th scope="col">{{ t("notifications.to") }}</th>
            <th scope="col">{{ t("notifications.last") }}</th>
            <th scope="col">{{ t("devices.state") }}</th>
            <th scope="col"><span class="sr-only">{{ t("access.actions") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in rules" :key="r.id" class="border-t border-rule-soft" :data-rule="r.name">
            <td>{{ r.name }}</td>
            <td>{{ r.events.join(", ") }}</td>
            <td>{{ vaultName(r.vault) }}</td>
            <td>{{ r.host }}</td>
            <td>{{ r.last ? `${r.last.result} · ${r.last.at.slice(0, 16).replace("T", " ")}` : "" }}</td>
            <td>{{ r.enabled ? t("notifications.on") : t("notifications.offState") }}</td>
            <td class="flex gap-1">
              <button type="button" class="border border-rule px-2" data-test="test" @click="test(r)">{{ t("notifications.test") }}</button>
              <button v-if="!r.enabled" type="button" class="border border-rule px-2" @click="enable(r, true)">{{ t("notifications.reEnable") }}</button>
              <button v-else type="button" class="border border-rule px-2" @click="enable(r, false)">{{ t("notifications.disable") }}</button>
              <RouterLink :to="`/manage/tenants/${tenant}/notifications/${r.id}`" class="underline">{{ t("notifications.deliveries") }}</RouterLink>
              <button type="button" class="border border-accent px-2" @click="remove(r)">{{ t("access.remove") }}</button>
            </td>
          </tr>
        </tbody>
      </table>
      <button v-if="!creating" type="button" class="mt-4 border border-rule px-3" data-test="new-rule" @click="creating = true">
        {{ t("notifications.new") }}
      </button>
      <form v-else class="mt-4 grid max-w-2xl gap-2 border border-rule p-4" @submit.prevent="create">
        <label>{{ t("keys.name") }} <input v-model="name" required maxlength="64" class="ml-2 border border-rule bg-ground px-1" data-test="name" /></label>
        <label>{{ t("notifications.scope") }}
          <select v-model="scope" class="ml-2 bg-ground" data-test="scope">
            <option v-if="admin" value="">{{ t("notifications.tenantWide") }}</option>
            <option v-for="v in mine" :key="v.id" :value="v.id">{{ v.name }}</option>
          </select>
        </label>
        <fieldset class="flex flex-wrap gap-3">
          <legend>{{ t("notifications.events") }}</legend>
          <label v-for="e in offered" :key="e"><input v-model="events" type="checkbox" :value="e" /> {{ e }}</label>
        </fieldset>
        <label>{{ t("notifications.webhook") }}
          <input v-model="url" required type="url" class="ml-2 w-96 border border-rule bg-ground px-1" placeholder="https://hub.example.org/hooks/kura" data-test="url" />
        </label>
        <label>{{ t("notifications.authorization") }}
          <input v-model="authorization" type="password" autocomplete="off" class="ml-2 border border-rule bg-ground px-1" data-test="authorization" />
        </label>
        <div class="flex gap-2">
          <button type="button" class="border border-rule px-3" @click="creating = false">{{ t("devices.cancel") }}</button>
          <button type="submit" class="bg-accent px-3 text-accent-ink" data-test="create">{{ t("keys.create") }}</button>
        </div>
      </form>
    </AsyncState>
  </section>
</template>
