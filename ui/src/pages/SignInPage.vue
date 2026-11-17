<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { stayByDefault, useSession } from "@/auth/session";

const { t } = useI18n();
const route = useRoute();
const session = useSession();
const stay = ref(stayByDefault());
const failure = ref<string | null>(null);

async function signIn(): Promise<void> {
  failure.value = null;
  const to = typeof route.query.to === "string" ? route.query.to : "/vaults";
  try {
    await session.login(to, stay.value);
  } catch (error) {
    failure.value = t("signIn.failed", {
      message: error instanceof Error ? error.message : String(error),
    });
  }
}
</script>

<template>
  <main class="mx-auto mt-16 max-w-md border border-rule p-6">
    <h1>{{ t("app.name") }}</h1>
    <p class="mb-4 text-ink-dim">{{ t("app.tagline") }}</p>
    <p v-if="session.endedUnexpectedly" role="alert" class="mb-4 border border-accent p-3" data-test="ended">
      {{ t("signIn.ended") }}
    </p>
    <p v-if="failure" role="alert" class="mb-4 border border-accent p-3">{{ failure }}</p>
    <button type="button" class="bg-accent px-4 py-2 text-accent-ink" data-test="sign-in" @click="signIn">
      {{ t("signIn.action") }}
    </button>
    <p class="mt-4">
      <label class="flex items-center gap-2">
        <input v-model="stay" type="checkbox" aria-describedby="stay-help" data-test="stay" />
        {{ t("signIn.stay") }}
      </label>
      <span id="stay-help" class="text-ink-dim">{{ t("signIn.stayHelp") }}</span>
    </p>
  </main>
</template>
