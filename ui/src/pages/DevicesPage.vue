<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { signOut, withDevice } from "@/api/client";
import { listDevices, revokeDevice } from "@/api/generated/sdk.gen";
import type { Device } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import { useSession } from "@/auth/session";
import AsyncState from "@/components/ui/AsyncState.vue";

const { t } = useI18n();
const router = useRouter();
const session = useSession();
const devices = ref<Device[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
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

function state(d: Device): string {
  if (d.revoked) return t("devices.revoked");
  return d.id === session.device()
    ? t("devices.thisBrowser")
    : t("devices.active");
}
onMounted(load);
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("devices.title") }}</h1>
    <AsyncState
      :loading="loading"
      :error="error"
      :empty="devices.length === 0"
      :empty-message="t('devices.empty')"
      :title="t('devices.failed')"
      @retry="load"
    >
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("devices.name") }}</th>
            <th scope="col">{{ t("devices.kind") }}</th>
            <th scope="col">{{ t("devices.lastSeen") }}</th>
            <th scope="col">{{ t("devices.state") }}</th>
            <th scope="col">{{ t("devices.owner") }}</th>
            <th scope="col"><span class="sr-only">{{ t("devices.revoke") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="d in devices" :key="d.id" class="border-t border-rule-soft" :data-device="d.id">
            <td>{{ d.name }}</td>
            <td>{{ d.profile }}</td>
            <td><time :datetime="d.lastSeen">{{ d.lastSeen.slice(0, 16).replace("T", " ") }}</time></td>
            <td>{{ state(d) }}</td>
            <td>{{ d.owner ?? "" }}</td>
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
        </tbody>
      </table>
      <div v-if="confirming" role="alertdialog" aria-modal="false" class="mt-4 border border-accent p-4">
        <p>{{ t("devices.confirmRevoke", { name: confirming.name }) }}</p>
        <button type="button" class="mr-2 bg-accent px-3 py-1 text-accent-ink" data-test="confirm" @click="revoke(confirming)">
          {{ t("devices.confirm") }}
        </button>
        <button type="button" class="border border-rule px-3 py-1" @click="confirming = null">
          {{ t("devices.cancel") }}
        </button>
      </div>
    </AsyncState>
  </section>
</template>
