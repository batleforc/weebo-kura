<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";

import { ensureDevice, signOut } from "@/api/client";
import { problemType } from "@/api/problem";
import { useSession } from "@/auth/session";
import { useMe } from "@/me";

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const session = useSession();
const meStore = useMe();
const shell = computed(() => route.meta.access === "member");
/**
 * The back office is one more menu, present only when `GET /me` gives a role
 * that can use it (§4.5): a tenant admin or owner, a platform admin. A vault
 * manager without a tenant role finds it from the vault (decision D1 of the
 * phase 2 plan).
 */
const canManage = computed(
  () =>
    !!meStore.me &&
    (meStore.me.platformAdmin ||
      meStore.me.tenants.some((x) => x.role !== "member")),
);
/** Signed in, but in no tenant: registering a device is refused (§4.5). */
const noTenant = ref(false);

watch(
  () => shell.value && session.authenticated,
  async (inside) => {
    if (!inside) return;
    try {
      const me = await meStore.load();
      noTenant.value = me.tenants.length === 0;
      if (!noTenant.value) await ensureDevice();
    } catch (error) {
      noTenant.value = problemType(error) === "insufficient_role";
    }
  },
  { immediate: true },
);

async function leave(): Promise<void> {
  await signOut();
  meStore.me = null;
  await router.replace({ name: "signin" });
}
</script>

<template>
  <template v-if="shell">
    <header class="flex flex-wrap items-center gap-4 border-b border-rule px-4 py-2">
      <span class="font-bold">{{ t("app.name") }}</span>
      <nav :aria-label="t('nav.main')" class="flex gap-4">
        <RouterLink to="/vaults" active-class="underline">{{ t("nav.vaults") }}</RouterLink>
        <RouterLink to="/devices" active-class="underline">{{ t("nav.devices") }}</RouterLink>
        <RouterLink v-if="canManage" to="/manage" active-class="underline" data-test="manage">{{ t("manage.nav") }}</RouterLink>
      </nav>
      <span class="ml-auto text-ink-dim">{{ meStore.me?.name ?? meStore.me?.subject ?? "" }}</span>
      <button type="button" class="border border-rule px-2" data-test="sign-out" @click="leave">
        {{ t("nav.signOut") }}
      </button>
    </header>
    <main class="p-4">
      <div v-if="noTenant" role="alert" data-test="no-tenant">
        <p>{{ t("noTenant.text", { name: meStore.me?.name ?? meStore.me?.subject ?? "" }) }}</p>
        <p>{{ t("noTenant.hint") }}</p>
      </div>
      <RouterView v-else />
    </main>
  </template>
  <RouterView v-else />
</template>
