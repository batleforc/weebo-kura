<script setup lang="ts">
/**
 * A vault at a glance: who opens it and its live keys (for its managers), and
 * the notes changed last, in the order the vault changed them (`seq`: an
 * entry carries no time).
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import { listBindings, listTokens } from "@/api/generated/sdk.gen";
import type {
  BindingView,
  EntryState,
  TokenView,
} from "@/api/generated/types.gen";
import VaultDevices from "@/components/VaultDevices.vue";
import { useMe } from "@/me";
import { ago, when } from "@/reader/time";
import { noteName } from "@/reader/tree";
import { loadVault } from "@/reader/vault";
import { useVaults } from "@/vaults";

const RECENT = 5;

const { t, locale } = useI18n();
const route = useRoute();
const meStore = useMe();
const vaultStore = useVaults();
const id = computed(() => String(route.params.v));
const vault = computed(() => vaultStore.list.find((v) => v.id === id.value));
const manages = computed(
  () =>
    vault.value?.role === "manager" ||
    (meStore.me?.tenants.some(
      (x) => x.slug === vault.value?.tenant && x.role !== "member",
    ) ??
      false),
);
const bindings = ref<BindingView[] | null>(null);
const keys = ref<TokenView[] | null>(null);
const recent = ref<EntryState[] | null>(null);
const live = computed(() => (keys.value ?? []).filter((k) => !k.revoked));
const revoked = computed(() => (keys.value ?? []).length - live.value.length);

function subject(b: BindingView): string {
  const [k, ...rest] = b.subject.split(":");
  return `${k === "claim" ? t("access.group") : t("access.user")} ${rest.join(":")}`;
}

onMounted(async () => {
  await Promise.all([vaultStore.load(), meStore.load()]).catch(() => undefined);
  const v = id.value;
  if (manages.value) {
    void withDevice(
      async (headers) =>
        (await listBindings({ headers, path: { v }, throwOnError: true })).data,
    )
      .then((b) => {
        bindings.value = b;
      })
      .catch(() => undefined);
    void withDevice(
      async (headers) =>
        (await listTokens({ headers, path: { v }, throwOnError: true })).data,
    )
      .then((k) => {
        keys.value = k;
      })
      .catch(() => undefined);
  }
  recent.value = await loadVault(v)
    .then(({ entries }) =>
      entries
        .filter((e) => !e.deleted)
        .sort((a, b) => b.seq - a.seq)
        .slice(0, RECENT),
    )
    .catch(() => []);
});
</script>

<template>
  <div class="grid grid-cols-[repeat(auto-fit,minmax(17rem,1fr))] items-start gap-x-10 gap-y-8">
    <section v-if="manages" aria-labelledby="who" class="flex flex-col gap-2">
      <h2 id="who" class="label border-b border-rule pb-2">{{ t("vault.who") }}</h2>
      <ul data-test="who">
        <li class="flex justify-between gap-3 border-b border-dashed border-rule-soft py-2.5">
          <span>{{ t("access.tenantAdmins") }}</span><span class="text-ink-dim">{{ t("role.manager") }}</span>
        </li>
        <li v-for="b in bindings ?? []" :key="b.subject" class="flex justify-between gap-3 border-b border-dashed border-rule-soft py-2.5">
          <span class="min-w-0 break-all">{{ subject(b) }}</span>
          <span class="text-ink-dim">{{ t(`role.${b.role}`) }}<template v-if="b.paths"> · {{ t("reader.reach", { n: b.paths.length }) }}</template></span>
        </li>
      </ul>
      <RouterLink :to="`/vaults/${id}/access`">{{ t("vault.manageAccess") }}</RouterLink>
    </section>

    <section v-if="manages" aria-labelledby="live-keys" class="flex flex-col gap-2">
      <h2 id="live-keys" class="label border-b border-rule pb-2">{{ t("vault.liveKeys") }}</h2>
      <p v-if="keys && !live.length" class="text-ink-dim">{{ t("vault.noKey") }}</p>
      <ul data-test="live-keys">
        <li v-for="k in live" :key="k.id" class="flex justify-between gap-3 border-b border-dashed border-rule-soft py-2.5">
          <span class="min-w-0 break-all">{{ k.name }}</span>
          <time :datetime="k.expiresAt" :title="when(k.expiresAt, locale)" class="text-ink-dim">{{ ago(k.expiresAt, locale) }}</time>
        </li>
      </ul>
      <p v-if="revoked" class="text-ink-dim">{{ t("vault.revokedHidden", { n: revoked }, revoked) }}</p>
      <RouterLink :to="`/vaults/${id}/keys`">{{ t("vault.manageKeys") }}</RouterLink>
    </section>

    <section aria-labelledby="recent" class="flex flex-col gap-2">
      <h2 id="recent" class="label border-b border-rule pb-2">{{ t("vault.recent") }}</h2>
      <p v-if="recent === null" role="status" class="text-ink-dim">{{ t("state.loading") }}</p>
      <p v-else-if="!recent.length" class="text-ink-dim">{{ t("reader.emptyVault") }}</p>
      <ul v-else data-test="recent">
        <li v-for="e in recent" :key="e.entry" class="border-b border-dashed border-rule-soft py-2.5">
          <RouterLink :to="`/vaults/${id}/notes/${e.entry}`" class="break-words">{{ noteName(e.path) }}</RouterLink>
          <span v-if="e.path.includes('/')" class="ml-2 text-ink-dim">{{ e.path.slice(0, e.path.lastIndexOf("/") + 1) }}</span>
        </li>
      </ul>
      <RouterLink :to="`/vaults/${id}/notes`">{{ t("vault.browse") }}</RouterLink>
    </section>

    <VaultDevices v-if="vault" :vault="id" :manages="manages" />
  </div>
</template>
