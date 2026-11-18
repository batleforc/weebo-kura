<script setup lang="ts">
/**
 * A tenant's notification rules (RFC 0003 §4.5): only the host of a URL is
 * shown again; a rule that switched itself off is a banner, not a silent
 * state; the signing secret is shown once, as a key's is, on creation and
 * on rotation. Editing keeps what is left empty: the URL and authorization
 * are never shown again, so an empty field means "unchanged".
 */
import { computed, nextTick, onMounted, ref } from "vue";
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
import BreadCrumbs from "@/components/ui/BreadCrumbs.vue";
import ConfirmRow from "@/components/ui/ConfirmRow.vue";
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import StateMark from "@/components/ui/StateMark.vue";
import TableScroll from "@/components/ui/TableScroll.vue";
import { useLoad } from "@/manage/useLoad";
import { useMe } from "@/me";
import { ago, when } from "@/reader/time";

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

const { t, locale } = useI18n();
const route = useRoute();
const meStore = useMe();
/** Under a vault's tab: that vault's rules, in its tenant; else a whole tenant's. */
const inVault = computed(() =>
  route.params.v ? String(route.params.v) : null,
);
const vaults = ref<Vault[]>([]);
const tenant = computed(() =>
  inVault.value
    ? (vaults.value.find((v) => v.id === inVault.value)?.tenant ?? "")
    : String(route.params.t),
);
const disabled = ref(false);
const {
  data: rules,
  loading,
  error,
  forbidden,
  forbiddenDetail,
  canRetry,
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
const shown = computed(() =>
  inVault.value
    ? rules.value.filter((r) => r.vault === inVault.value)
    : rules.value,
);
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
  shown.value.filter((r) => !r.enabled && r.disabledReason),
);

const creating = ref(false);
/** The rule being edited; `null` while creating. */
const editing = ref<RuleView | null>(null);
const removeAuthorization = ref(false);
const name = ref("");
const scope = ref<string>("");
const events = ref<string[]>([]);
const url = ref("");
const authorization = ref("");
const refusal = ref<string | null>(null);
const secret = ref<string | null>(null);
const rotated = ref(false);
const copied = ref(false);
const rotating = ref<RuleView | null>(null);
/** Where focus goes back when a form, dialog or confirmation closes (§11). */
let opener: HTMLElement | null = null;
const firstField = ref<HTMLInputElement | null>(null);
const secretBox = ref<HTMLElement | null>(null);
const confirmBox = ref<HTMLElement | null>(null);
const newRule = ref<HTMLButtonElement | null>(null);
const tested = ref<{ rule: string; result: string } | null>(null);

function vaultName(id: string | null | undefined): string {
  if (!id) return t("notifications.tenantWide");
  return vaults.value.find((v) => v.id === id)?.name ?? id.slice(0, 8);
}

function remember(): void {
  opener =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
}

/** Back to the opener if it is still on the page, else to "New rule". */
async function giveFocusBack(): Promise<void> {
  await nextTick();
  (opener?.isConnected ? opener : newRule.value)?.focus();
  opener = null;
}

async function focusOn(el: { value: HTMLElement | null }): Promise<void> {
  await nextTick();
  el.value?.focus();
}

function openForm(r: RuleView | null): void {
  remember();
  editing.value = r;
  creating.value = true;
  refusal.value = null;
  name.value = r?.name ?? "";
  events.value = r ? [...r.events] : [];
  url.value = "";
  authorization.value = "";
  removeAuthorization.value = false;
  void focusOn(firstField);
}

function closeForm(): void {
  creating.value = false;
  editing.value = null;
  void giveFocusBack();
}

async function showSecret(value: string | null | undefined): Promise<void> {
  secret.value = value ?? null;
  copied.value = false;
  if (secret.value) await focusOn(secretBox);
}

async function copy(): Promise<void> {
  if (secret.value) {
    await navigator.clipboard.writeText(secret.value).catch(() => undefined);
    copied.value = true;
  }
}

function closeSecret(): void {
  secret.value = null;
  copied.value = false;
  void giveFocusBack();
}

function submit(): Promise<void> {
  return editing.value ? save(editing.value) : create();
}

async function save(r: RuleView): Promise<void> {
  refusal.value = null;
  try {
    await withDevice((headers) =>
      updateRule({
        headers,
        path: { t: tenant.value, n: r.id },
        body: {
          name: name.value,
          events: events.value,
          url: url.value || undefined,
          authorization: removeAuthorization.value
            ? ""
            : authorization.value || undefined,
        },
        throwOnError: true,
      }),
    );
    closeForm();
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

function askRotate(r: RuleView): void {
  remember();
  rotating.value = r;
  void focusOn(confirmBox);
}

function cancelRotate(): void {
  rotating.value = null;
  void giveFocusBack();
}

async function rotate(r: RuleView): Promise<void> {
  rotating.value = null;
  refusal.value = null;
  try {
    const { data } = await withDevice((headers) =>
      updateRule({
        headers,
        path: { t: tenant.value, n: r.id },
        body: { rotateSecret: true },
        throwOnError: true,
      }),
    );
    rotated.value = true;
    await load();
    await showSecret(data.secret);
  } catch (e) {
    refusal.value = messageOf(e);
    void giveFocusBack();
  }
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
    creating.value = false;
    url.value = "";
    authorization.value = "";
    rotated.value = false;
    await load();
    await showSecret(data.secret);
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

/** The rule a removal waits to be confirmed for. */
const removing = ref<RuleView | null>(null);

async function remove(r: RuleView): Promise<void> {
  removing.value = null;
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
  scope.value = inVault.value ?? (admin.value ? "" : (mine.value[0]?.id ?? ""));
  await load();
});
</script>

<template>
  <section>
    <template v-if="!inVault">
      <BreadCrumbs :items="[{ label: t('home.title'), to: '/vaults' }, { label: tenant }]" />
      <h1 class="mb-4">{{ t("manage.notifications") }}</h1>
    </template>
    <h2 v-else class="sr-only">{{ t("manage.notifications") }}</h2>
    <p v-if="disabled" role="status" data-test="disabled">{{ t("notifications.off") }}</p>
    <ForbiddenState v-else-if="forbidden" :detail="forbiddenDetail" />
    <AsyncState v-else :loading="loading" :error="error" :empty="false" empty-message="" :title="t('manage.failed')" :can-retry="canRetry" @retry="load">
      <p v-for="r in switchedOff" :key="`off-${r.id}`" role="alert" class="mb-2 border-l border-rule pl-3" data-test="switched-off">
        {{ t("notifications.switchedOff", { name: r.name, reason: r.disabledReason }) }}
      </p>
      <div v-if="secret" ref="secretBox" role="dialog" tabindex="-1" aria-labelledby="rule-created" aria-describedby="rule-once" class="mb-4 border border-accent p-4" data-test="created">
        <h2 id="rule-created" class="font-bold">{{ rotated ? t("notifications.rotated") : t("notifications.created") }}</h2>
        <p class="my-2 break-all"><code data-test="secret">{{ secret }}</code></p>
        <button type="button" class="mr-2 border border-rule px-3" data-test="copy" @click="copy">{{ copied ? t("keys.copied") : t("keys.copy") }}</button>
        <p id="rule-once" class="my-2">{{ t("notifications.once") }}</p>
        <button type="button" class="bg-accent px-3 text-accent-ink" data-test="close" @click="closeSecret">{{ t("keys.stored") }}</button>
      </div>
      <div v-if="rotating" ref="confirmBox" role="alertdialog" tabindex="-1" aria-labelledby="rotate-question" class="mb-4 border border-accent p-4" data-test="confirm-rotate">
        <p id="rotate-question">{{ t("notifications.confirmRotate", { name: rotating.name }) }}</p>
        <button type="button" class="mr-2 bg-accent px-3 text-accent-ink" data-test="confirm" @click="rotate(rotating)">{{ t("notifications.rotate") }}</button>
        <button type="button" class="border border-rule px-3" @click="cancelRotate">{{ t("devices.cancel") }}</button>
      </div>
      <p v-if="tested" role="status" class="mb-2" data-test="tested">{{ t("notifications.tested", tested) }}</p>
      <p v-if="refusal" role="alert" class="mb-2 border-l border-rule pl-3">{{ refusal }}</p>
      <p v-if="shown.length === 0" class="text-ink-dim">{{ t("notifications.empty") }}</p>
      <TableScroll v-else :label="t('state.tableRegion')">
      <table class="acts w-full text-left">
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
          <template v-for="r in shown" :key="r.id">
          <tr class="border-t border-rule-soft" :data-rule="r.name">
            <th scope="row" class="font-normal">{{ r.name }}</th>
            <td>{{ r.events.map((e) => t(`notifications.ev.${e}`)).join(", ") }}</td>
            <td>{{ vaultName(r.vault) }}</td>
            <td>{{ r.host }}</td>
            <td><template v-if="r.last">{{ r.last.result }} · <time :datetime="r.last.at" :title="when(r.last.at, locale)">{{ ago(r.last.at, locale) }}</time></template></td>
            <td><StateMark :kind="r.enabled ? 'live' : r.disabledReason ? 'waiting' : 'off'" :word="r.enabled ? t('notifications.on') : t('notifications.offState')" /></td>
            <!-- Not `flex` on the cell itself: it drops the cell from the table for some screen readers. -->
            <td>
              <div class="flex flex-wrap gap-1">
                <button type="button" class="border border-rule px-2" data-test="test" @click="test(r)">{{ t("notifications.test") }}</button>
                <button v-if="!r.enabled" type="button" class="border border-rule px-2" @click="enable(r, true)">{{ t("notifications.reEnable") }}</button>
                <button type="button" class="border border-rule px-2" data-test="edit" @click="openForm(r)">{{ t("notifications.edit") }}</button>
                <RouterLink :to="`/manage/tenants/${tenant}/notifications/${r.id}`" class="underline">{{ t("notifications.deliveries") }}</RouterLink>
                <!-- The rarer actions, one step away: the row keeps three choices. -->
                <details>
                  <summary class="cursor-pointer border border-rule px-2">{{ t("notifications.more") }}</summary>
                  <div class="mt-1 flex flex-wrap gap-1">
                    <button v-if="r.enabled" type="button" class="border border-rule px-2" @click="enable(r, false)">{{ t("notifications.disable") }}</button>
                    <button type="button" class="border border-rule px-2" data-test="rotate" @click="askRotate(r)">{{ t("notifications.rotate") }}</button>
                    <button type="button" class="border border-accent px-2" data-test="remove" @click="removing = r">{{ t("access.remove") }}</button>
                  </div>
                </details>
              </div>
            </td>
          </tr>
          <ConfirmRow
            v-if="removing?.id === r.id"
            :colspan="7"
            :question="t('notifications.confirmRemove', { name: r.name })"
            :action="t('access.remove')"
            @confirm="remove(r)"
            @cancel="removing = null"
          />
          </template>
        </tbody>
      </table>
      </TableScroll>
      <button v-if="!creating" ref="newRule" type="button" class="mt-4 border border-rule px-3" data-test="new-rule" @click="openForm(null)">
        {{ t("notifications.new") }}
      </button>
      <form v-else class="mt-4 grid max-w-2xl gap-2 border-t border-dashed border-rule-soft pt-4" aria-labelledby="rule-form" @submit.prevent="submit">
        <h2 id="rule-form" class="font-bold">{{ editing ? t("notifications.editTitle", { name: editing.name }) : t("notifications.new") }}</h2>
        <label>{{ t("keys.name") }} <input ref="firstField" v-model="name" required maxlength="64" class="ml-2 border border-rule bg-ground px-1" data-test="name" /></label>
        <label v-if="!editing">{{ t("notifications.scope") }}
          <select v-model="scope" class="ml-2 bg-ground" data-test="scope">
            <option v-if="admin" value="">{{ t("notifications.tenantWide") }}</option>
            <option v-for="v in mine" :key="v.id" :value="v.id">{{ v.name }}</option>
          </select>
        </label>
        <fieldset class="flex flex-wrap gap-3">
          <legend>{{ t("notifications.events") }}</legend>
          <label v-for="e in offered" :key="e"><input v-model="events" type="checkbox" :value="e" /> {{ t(`notifications.ev.${e}`) }} <code class="meta text-ink-dim">{{ e }}</code></label>
        </fieldset>
        <label>{{ t("notifications.webhook") }}
          <input v-model="url" :required="!editing" type="url" class="ml-2 w-96 border border-rule bg-ground px-1" :placeholder="editing ? t('notifications.unchanged', { host: editing.host }) : 'https://hub.example.org/hooks/kura'" data-test="url" />
        </label>
        <label>{{ t("notifications.authorization") }}
          <input v-model="authorization" type="password" autocomplete="off" :disabled="removeAuthorization" :placeholder="editing?.hasAuthorization ? t('notifications.keptAuthorization') : undefined" class="ml-2 border border-rule bg-ground px-1" data-test="authorization" />
        </label>
        <label v-if="editing?.hasAuthorization"><input v-model="removeAuthorization" type="checkbox" data-test="remove-authorization" /> {{ t("notifications.removeAuthorization") }}</label>
        <div class="flex gap-2">
          <button type="button" class="border border-rule px-3" @click="closeForm">{{ t("devices.cancel") }}</button>
          <button type="submit" class="bg-accent px-3 text-accent-ink" data-test="create">{{ editing ? t("notifications.save") : t("keys.create") }}</button>
        </div>
      </form>
    </AsyncState>
  </section>
</template>
