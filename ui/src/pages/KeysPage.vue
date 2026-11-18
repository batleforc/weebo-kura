<script setup lang="ts">
/**
 * A vault's keys (RFC 0003 §4.5, with RFC 0004's folder rules): co-managed by
 * the vault's managers and the tenant's admins; the secret is shown once, to
 * its creator, and never again (T4).
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { createToken, listTokens, revokeToken } from "@/api/generated/sdk.gen";
import type { ScopeRule, TokenView } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import ConfirmRow from "@/components/ui/ConfirmRow.vue";
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import RevokedToggle from "@/components/ui/RevokedToggle.vue";
import StateMark from "@/components/ui/StateMark.vue";
import TableFilter from "@/components/ui/TableFilter.vue";
import { matching } from "@/filter";
import { useLoad } from "@/manage/useLoad";
import { useMe } from "@/me";
import { ago, when } from "@/reader/time";

const { t, locale } = useI18n();
const route = useRoute();
const v = computed(() => String(route.params.v));
const {
  data: tokens,
  loading,
  error,
  forbidden,
  forbiddenDetail,
  canRetry,
  load,
} = useLoad<TokenView[]>(
  () =>
    withDevice(
      async (headers) =>
        (
          await listTokens({
            headers,
            path: { v: v.value },
            throwOnError: true,
          })
        ).data,
    ),
  [],
);
const creating = ref(false);
const name = ref("");
const rules = ref<{ path: string; mode: "read" | "write" }[]>([
  { path: "", mode: "read" },
]);
const days = ref(30);
const cidrs = ref("");
const refusal = ref<string | null>(null);
/** The secret of the key just created: in this component's memory only. */
const secret = ref<string | null>(null);
const copied = ref(false);
const confirming = ref<TokenView | null>(null);
const meStore = useMe();
const showRevoked = ref(false);
const revokedCount = computed(
  () => tokens.value.filter((k) => k.revoked).length,
);
const query = ref("");
const shown = computed(() =>
  matching(
    showRevoked.value ? tokens.value : tokens.value.filter((k) => !k.revoked),
    query.value,
    (k) => [k.name, k.createdBy, ...k.cidrs],
  ),
);

/** "you" for the caller's own keys, else the subject cut short (the full one on hover). */
function creator(k: TokenView): string {
  if (k.createdBy === meStore.me?.subject) return t("keys.you");
  return k.createdBy.length > 12 ? `${k.createdBy.slice(0, 8)}…` : k.createdBy;
}

function scopes(k: TokenView): string {
  return k.scopes
    .map(
      (r) =>
        `${r.path === "" ? t("reader.whole") : r.path} (${r.mode === "write" ? t("keys.write") : t("keys.read")})`,
    )
    .join(", ");
}

async function create(): Promise<void> {
  refusal.value = null;
  const list: ScopeRule[] = rules.value
    .map((r) => ({
      path: r.path.trim() === "" ? "" : r.path.trim().replace(/\/?$/, "/"),
      mode: r.mode,
    }))
    .filter((r, i) => r.path !== "" || i === 0);
  try {
    const { data } = await withDevice((headers) =>
      createToken({
        headers,
        path: { v: v.value },
        body: {
          name: name.value.trim() || null,
          scopes: list,
          expiresInDays: days.value,
          cidrs: cidrs.value.split(/[\s,]+/).filter((c) => c !== ""),
        },
        throwOnError: true,
      }),
    );
    secret.value = data.token;
    creating.value = false;
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

async function copy(): Promise<void> {
  if (secret.value) {
    await navigator.clipboard.writeText(secret.value).catch(() => undefined);
    copied.value = true;
  }
}

function close(): void {
  secret.value = null;
  copied.value = false;
}

async function revoke(k: TokenView): Promise<void> {
  confirming.value = null;
  try {
    await withDevice((headers) =>
      revokeToken({
        headers,
        path: { v: v.value, t: k.id },
        throwOnError: true,
      }),
    );
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

onMounted(() => {
  void meStore.load().catch(() => undefined);
  void load();
});
</script>

<template>
  <section>
    <h2 class="sr-only">{{ t("manage.keys") }}</h2>
    <ForbiddenState v-if="forbidden" :detail="forbiddenDetail" />
    <AsyncState
      v-else
      :loading="loading"
      :error="error"
      :empty="false"
      empty-message=""
      :title="t('manage.failed')"
      :can-retry="canRetry" @retry="load"
    >
      <div v-if="secret" role="dialog" aria-labelledby="created" class="mb-4 border border-accent p-4" data-test="created">
        <h2 id="created" class="font-bold">{{ t("keys.created") }}</h2>
        <p class="my-2 break-all"><code data-test="secret">{{ secret }}</code></p>
        <button type="button" class="mr-2 border border-rule px-3" @click="copy">{{ copied ? t("keys.copied") : t("keys.copy") }}</button>
        <p class="my-2">{{ t("keys.once") }}</p>
        <button type="button" class="bg-accent px-3 text-accent-ink" data-test="close" @click="close">{{ t("keys.stored") }}</button>
      </div>
      <p v-if="refusal" role="alert" class="mb-2 border-l border-rule pl-3">{{ refusal }}</p>
      <p v-if="tokens.length === 0" class="text-ink-dim">{{ t("keys.empty") }}</p>
      <TableFilter v-if="tokens.length > 0" v-model="query" :empty="shown.length === 0" />
      <div v-if="tokens.length > 0" class="relative overflow-x-auto" tabindex="0">
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("keys.name") }}</th>
            <th scope="col">{{ t("keys.scopes") }}</th>
            <th scope="col">{{ t("keys.from") }}</th>
            <th scope="col">{{ t("keys.expires") }}</th>
            <th scope="col">{{ t("keys.lastUsed") }}</th>
            <th scope="col">{{ t("keys.by") }}</th>
            <th scope="col">{{ t("devices.state") }}</th>
            <th scope="col"><span class="sr-only">{{ t("devices.revoke") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <template v-for="k in shown" :key="k.id">
          <tr class="border-t border-rule-soft">
            <td>{{ k.name }}</td>
            <td>{{ scopes(k) }}</td>
            <td>{{ k.cidrs.join(", ") }}</td>
            <td><time :datetime="k.expiresAt" :title="when(k.expiresAt, locale)">{{ ago(k.expiresAt, locale) }}</time></td>
            <td><time v-if="k.lastUsedAt" :datetime="k.lastUsedAt" :title="when(k.lastUsedAt, locale)">{{ ago(k.lastUsedAt, locale) }}</time><template v-else>{{ t("keys.never") }}</template></td>
            <td :title="k.createdBy">{{ creator(k) }}</td>
            <td><StateMark :kind="k.revoked ? 'gone' : 'live'" :word="k.revoked ? t('devices.revoked') : t('devices.active')" /></td>
            <td>
              <button v-if="!k.revoked" type="button" class="border border-accent px-2" @click="confirming = k">
                {{ t("devices.revoke") }}
              </button>
            </td>
          </tr>
          <ConfirmRow
            v-if="confirming?.id === k.id"
            :colspan="8"
            :question="t('keys.confirmRevoke', { name: k.name })"
            :action="t('devices.confirm')"
            @confirm="revoke(k)"
            @cancel="confirming = null"
          />
          </template>
        </tbody>
      </table>
      </div>
      <RevokedToggle v-model="showRevoked" :count="revokedCount" />
      <button v-if="!creating" type="button" class="mt-4 border border-rule px-3" data-test="new-key" @click="creating = true">
        {{ t("keys.new") }}
      </button>
      <form v-else class="mt-4 grid max-w-xl gap-2 border-t border-dashed border-rule-soft pt-4" @submit.prevent="create">
        <label>{{ t("keys.name") }} <input v-model="name" maxlength="64" class="ml-2 border border-rule bg-ground px-1" /></label>
        <fieldset>
          <legend>{{ t("keys.folders") }}</legend>
          <div v-for="(r, i) in rules" :key="i" class="flex gap-2">
            <input v-model="r.path" :aria-label="t('keys.folder')" :placeholder="t('keys.allVault')" class="border border-rule bg-ground px-1" />
            <select v-model="r.mode" :aria-label="t('keys.mode')" class="bg-ground">
              <option value="read">{{ t("keys.read") }}</option>
              <option value="write">{{ t("keys.write") }}</option>
            </select>
          </div>
          <button type="button" class="mt-1 border border-rule px-2" @click="rules.push({ path: '', mode: 'read' })">+ {{ t("keys.addFolder") }}</button>
        </fieldset>
        <label>{{ t("keys.expiresIn") }}
          <input v-model.number="days" type="number" min="1" max="90" class="ml-2 w-20 border border-rule bg-ground px-1" />
        </label>
        <label>{{ t("keys.allowedFrom") }}
          <input v-model="cidrs" class="ml-2 border border-rule bg-ground px-1" placeholder="10.42.0.0/16" data-test="cidrs" />
        </label>
        <div class="flex gap-2">
          <button type="button" class="border border-rule px-3" @click="creating = false">{{ t("devices.cancel") }}</button>
          <button type="submit" class="bg-accent px-3 text-accent-ink" data-test="create">{{ t("keys.create") }}</button>
        </div>
      </form>
    </AsyncState>
  </section>
</template>
