<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";

import { useSession } from "@/auth/session";

const { t } = useI18n();
const router = useRouter();
const session = useSession();
const failure = ref<string | null>(null);

onMounted(async () => {
  const params = new URLSearchParams(window.location.search);
  // The code leaves the address bar and the history at once (§6.2).
  window.history.replaceState(null, "", "/app/callback");
  try {
    const target = await session.completeLogin(params);
    await router.replace(target);
  } catch (error) {
    failure.value = error instanceof Error ? error.message : String(error);
  }
});
</script>

<template>
  <main class="mx-auto mt-16 max-w-md p-6">
    <p v-if="!failure" role="status">{{ t("callback.working") }}</p>
    <div v-else role="alert">
      <p>{{ t("callback.failed", { message: failure }) }}</p>
      <RouterLink to="/signin" class="underline">{{ t("callback.again") }}</RouterLink>
    </div>
  </main>
</template>
