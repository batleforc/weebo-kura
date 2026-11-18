<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { ensureDevice, signOut } from "@/api/client";
import { problemType } from "@/api/problem";
import { useSession } from "@/auth/session";
import PrefsMenu from "@/components/PrefsMenu.vue";
import Chevron from "@/components/ui/Chevron.vue";
import { useMe } from "@/me";
import { sectionOf, sectionsOf, useVaults } from "@/vaults";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const session = useSession();
const meStore = useMe();
const vaultStore = useVaults();
/** The sidebar's filter over the caller's vaults. */
const filter = ref("");
const groups = computed(() => {
  const q = filter.value.trim().toLowerCase();
  const shown = vaultStore.list.filter((v) => v.name.toLowerCase().includes(q));
  return [...new Set(shown.map((v) => v.tenant))].sort().map((tenant) => ({
    tenant,
    vaults: shown
      .filter((v) => v.tenant === tenant)
      .sort((a, b) => a.name.localeCompare(b.name)),
  }));
});
/** The vaults whose sections the sidebar shows; the open vault's unfold by themselves. */
const expanded = ref(new Set<string>());
function toggle(id: string): void {
  const next = new Set(expanded.value);
  if (!next.delete(id)) next.add(id);
  expanded.value = next;
}
watch(
  () => route.params.v,
  (v) => {
    if (typeof v === "string" && !expanded.value.has(v)) toggle(v);
  },
  { immediate: true },
);
/** On a phone the vault list folds under the header; open from 768px. */
const wide = ref(
  typeof matchMedia === "function" && matchMedia("(min-width: 768px)").matches,
);
const shell = computed(() => route.meta.access === "member");
/** Signed in, but in no tenant: registering a device is refused (§4.5). */
const noTenant = ref(false);

watch(
  () => shell.value && session.authenticated,
  async (inside) => {
    if (!inside) return;
    try {
      const me = await meStore.load();
      noTenant.value = me.tenants.length === 0;
      if (!noTenant.value) {
        await ensureDevice();
        await vaultStore.load().catch(() => undefined);
      }
    } catch (error) {
      noTenant.value = problemType(error) === "insufficient_role";
    }
  },
  { immediate: true },
);

async function leave(): Promise<void> {
  await signOut();
  meStore.me = null;
  vaultStore.list = [];
  vaultStore.loaded = false;
  await router.replace({ name: "signin" });
}
</script>

<template>
  <template v-if="shell">
    <a href="#main" class="sr-only focus:not-sr-only focus:absolute focus:left-4 focus:top-2 focus:z-10 focus:bg-ground focus:px-2">{{ t("nav.skip") }}</a>
    <div class="flex min-h-screen flex-wrap">
      <aside class="flex w-full flex-col gap-5 border-b border-rule-soft px-4 py-5 md:w-64 md:border-r md:border-b-0 md:py-6" :aria-label="t('nav.main')">
        <RouterLink to="/vaults" class="font-display text-[length:var(--t-px-md)] font-bold tracking-[0.04em] no-underline">KURA</RouterLink>
        <nav :aria-label="t('nav.main')" class="flex flex-col">
          <RouterLink to="/vaults" class="px-2 py-1.5 text-ink-dim hover:text-ink" exact-active-class="bg-ink font-bold !text-ground" data-test="home">{{ t("nav.home") }}</RouterLink>
        </nav>
        <details :open="wide" class="group">
          <summary class="cursor-pointer px-2 text-ink-dim md:hidden">{{ t("nav.vaults") }}</summary>
          <div class="mt-3 flex flex-col gap-4 md:mt-0">
            <label class="flex flex-col gap-1 px-2 text-ink-dim">{{ t("nav.findVault") }}
              <input v-model="filter" type="search" class="border border-rule bg-ground px-2 py-1 text-ink" :placeholder="t('nav.findVaultHint')" data-test="vault-filter" />
            </label>
            <nav :aria-label="t('nav.vaults')" class="flex flex-col gap-3" data-test="vault-nav">
              <div v-for="g in groups" :key="g.tenant" class="flex flex-col">
                <span class="px-2 pb-1 text-ink-dim">{{ g.tenant }}</span>
                <ul class="flex flex-col">
                  <li v-for="v in g.vaults" :key="v.id">
                    <div class="flex">
                      <button
                        type="button"
                        class="grid w-6 shrink-0 place-items-center text-ink-dim hover:text-ink pointer-coarse:w-11"
                        :aria-expanded="expanded.has(v.id)"
                        :aria-controls="`sections-${v.id}`"
                        :aria-label="t('nav.sectionsOf', { name: v.name })"
                        data-test="vault-expand"
                        @click="toggle(v.id)"
                      ><Chevron :dir="expanded.has(v.id) ? 'down' : 'right'" /></button>
                      <RouterLink
                        :to="`/vaults/${v.id}`"
                        class="min-w-0 flex-1 break-words py-1 pr-2 no-underline hover:text-ink"
                        :class="route.params.v === v.id ? 'font-bold text-ink' : ''"
                        :aria-current="route.params.v === v.id ? 'location' : undefined"
                      >{{ v.name }}</RouterLink>
                    </div>
                    <ul v-if="expanded.has(v.id)" :id="`sections-${v.id}`" class="mb-1 ml-3 border-l border-rule-soft pl-1" data-test="vault-sections">
                      <li v-for="s in sectionsOf(v, meStore.me?.tenants)" :key="s.name">
                        <RouterLink
                          :to="s.to"
                          class="block px-2 py-1 no-underline"
                          :class="route.params.v === v.id && sectionOf(route.name) === s.name ? 'bg-ink font-bold text-ground' : 'text-ink-dim hover:text-ink'"
                          :aria-current="route.params.v === v.id && sectionOf(route.name) === s.name ? 'page' : undefined"
                        >{{ t(s.label) }}</RouterLink>
                      </li>
                    </ul>
                  </li>
                </ul>
              </div>
              <p v-if="vaultStore.loaded && !groups.length" class="px-2 text-ink-dim">{{ t("nav.noVault") }}</p>
            </nav>
          </div>
        </details>
        <div class="flex flex-row flex-wrap items-center gap-x-4 gap-y-2 border-t border-dashed border-rule-soft px-2 pt-4 text-ink-dim md:mt-auto md:flex-col md:items-start">
          <RouterLink to="/devices" class="hover:text-ink" active-class="!text-ink font-bold">{{ t("nav.devices") }}</RouterLink>
          <RouterLink to="/help" class="hover:text-ink" active-class="!text-ink font-bold" data-test="help">{{ t("nav.help") }}</RouterLink>
          <PrefsMenu class="self-start" />
          <span>
            {{ meStore.me?.name ?? meStore.me?.subject ?? "" }} ·
            <button type="button" class="underline hover:text-ink" data-test="sign-out" @click="leave">{{ t("nav.signOut") }}</button>
          </span>
        </div>
      </aside>
      <main id="main" tabindex="-1" class="min-w-0 flex-[999_1_560px] px-4 py-8 outline-none sm:px-8 md:px-12 md:py-10">
        <div v-if="noTenant" role="alert" data-test="no-tenant">
          <p>{{ t("noTenant.text", { name: meStore.me?.name ?? meStore.me?.subject ?? "" }) }}</p>
          <p>{{ t("noTenant.hint") }}</p>
        </div>
        <RouterView v-else />
      </main>
    </div>
  </template>
  <RouterView v-else />
</template>
