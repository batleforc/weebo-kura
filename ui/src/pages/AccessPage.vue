<script setup lang="ts">
/** A vault's access (RFC 0003 §4.5): who reads it, and with what reach. */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";

import { withDevice } from "@/api/client";
import {
  deleteBinding,
  listBindings,
  listVaults,
  putBinding,
} from "@/api/generated/sdk.gen";
import type { BindingView } from "@/api/generated/types.gen";
import { messageOf } from "@/api/problem";
import AsyncState from "@/components/ui/AsyncState.vue";
import ForbiddenState from "@/components/ui/ForbiddenState.vue";
import { useLoad } from "@/manage/useLoad";

const { t } = useI18n();
const route = useRoute();
const v = computed(() => String(route.params.v));
const {
  data: bindings,
  loading,
  error,
  forbidden,
  forbiddenDetail,
  load,
} = useLoad<BindingView[]>(
  () =>
    withDevice(
      async (headers) =>
        (
          await listBindings({
            headers,
            path: { v: v.value },
            throwOnError: true,
          })
        ).data,
    ),
  [],
);
const adding = ref(false);
const kind = ref<"claim" | "user">("claim");
const who = ref("");
const role = ref("viewer");
const folders = ref("");
const refusal = ref<string | null>(null);
/** The tenant's mapped claim values, the only groups a binding may name. */
const groups = ref<string[]>([]);

/** `claim:jdr-players` as the page says it: "group jdr-players". */
function subject(b: BindingView): string {
  const [k, ...rest] = b.subject.split(":");
  return `${k === "claim" ? t("access.group") : t("access.user")} ${rest.join(":")}`;
}

async function save(
  subjectKey: string,
  r: string,
  paths?: string[],
): Promise<void> {
  refusal.value = null;
  try {
    await withDevice((headers) =>
      putBinding({
        headers,
        path: { v: v.value, b: subjectKey },
        body: { role: r, paths: paths ?? null },
        throwOnError: true,
      }),
    );
    adding.value = false;
    who.value = "";
    folders.value = "";
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

function add(): Promise<void> {
  const paths = folders.value
    .split(",")
    .map((f) => f.trim())
    .filter((f) => f !== "")
    .map((f) => (f.endsWith("/") ? f : `${f}/`));
  return save(
    `${kind.value}:${who.value.trim()}`,
    role.value,
    role.value === "viewer" && paths.length ? paths : undefined,
  );
}

async function remove(b: BindingView): Promise<void> {
  refusal.value = null;
  try {
    await withDevice((headers) =>
      deleteBinding({
        headers,
        path: { v: v.value, b: b.subject },
        throwOnError: true,
      }),
    );
    await load();
  } catch (e) {
    refusal.value = messageOf(e);
  }
}

onMounted(async () => {
  await load();
  const vaults = await withDevice(
    async (headers) => (await listVaults({ headers, throwOnError: true })).data,
  ).catch(() => []);
  groups.value = vaults.find((x) => x.id === v.value)?.groups ?? [];
});
</script>

<template>
  <section>
    <h1 class="mb-4">{{ t("manage.access") }}</h1>
    <ForbiddenState v-if="forbidden" :detail="forbiddenDetail" />
    <AsyncState v-else :loading="loading" :error="error" :empty="false" empty-message="" :title="t('manage.failed')" @retry="load">
      <p v-if="refusal" role="alert" class="mb-2 border border-accent p-2">{{ refusal }}</p>
      <table class="w-full text-left">
        <thead>
          <tr>
            <th scope="col">{{ t("access.subject") }}</th>
            <th scope="col">{{ t("access.role") }}</th>
            <th scope="col">{{ t("access.reach") }}</th>
            <th scope="col">{{ t("access.source") }}</th>
            <th scope="col"><span class="sr-only">{{ t("access.actions") }}</span></th>
          </tr>
        </thead>
        <tbody>
          <!-- The tenant role's grant, so the page answers "who can read this". -->
          <tr class="border-t border-rule-soft text-ink-dim">
            <td>{{ t("access.tenantAdmins") }}</td>
            <td>manager</td>
            <td>{{ t("reader.whole") }}</td>
            <td>{{ t("access.tenantRole") }}</td>
            <td />
          </tr>
          <tr v-for="b in bindings" :key="b.subject" class="border-t border-rule-soft" :data-binding="b.subject">
            <td>{{ subject(b) }}</td>
            <td>
              <select
                v-if="b.managedBy === 'api'"
                :value="b.role"
                :aria-label="t('access.changeRole', { who: subject(b) })"
                class="bg-ground"
                @change="save(b.subject, ($event.target as HTMLSelectElement).value)"
              >
                <option>viewer</option>
                <option>editor</option>
                <option>manager</option>
              </select>
              <span v-else>{{ b.role }}</span>
            </td>
            <!-- RFC 0005 §4.3: the warnings, as text, not colour. -->
            <td v-if="b.paths">
              <span v-for="(f, i) in b.paths" :key="f">{{ i ? ", " : "" }}{{ f }}<span v-if="b.emptyFolders?.includes(f)" class="text-ink-dim" data-test="no-notes"> ({{ t("access.noNotes") }})</span></span>
              <strong v-if="b.widened" class="block" data-test="widened" :title="t('access.widenedWhy')">{{ t("access.widened") }}</strong>
            </td>
            <td v-else>{{ t("reader.whole") }}</td>
            <td>{{ b.managedBy === "api" ? "API" : t("access.configuration") }}</td>
            <td>
              <button v-if="b.managedBy === 'api'" type="button" class="border border-accent px-2" @click="remove(b)">
                {{ t("access.remove") }}
              </button>
              <span v-else class="text-ink-dim">{{ t("access.readOnly") }}</span>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-if="bindings.length === 0" class="mt-2 text-ink-dim">{{ t("access.empty") }}</p>
      <button v-if="!adding" type="button" class="mt-4 border border-rule px-3" @click="adding = true">{{ t("access.add") }}</button>
      <form v-else class="mt-4 grid max-w-lg gap-2 border border-rule p-4" @submit.prevent="add">
        <fieldset class="flex gap-4">
          <legend>{{ t("access.who") }}</legend>
          <label><input v-model="kind" type="radio" value="claim" /> {{ t("access.group") }}</label>
          <label><input v-model="kind" type="radio" value="user" /> {{ t("access.user") }}</label>
        </fieldset>
        <label v-if="kind === 'claim' && groups.length">{{ t("access.group") }}
          <select v-model="who" required class="ml-2 bg-ground" data-test="group">
            <option v-for="g in groups" :key="g" :value="g">{{ g }}</option>
          </select>
        </label>
        <label v-else>{{ kind === "claim" ? t("access.group") : t("access.userSubject") }}
          <input v-model="who" required class="ml-2 border border-rule bg-ground px-1" data-test="who" />
        </label>
        <label>{{ t("access.role") }}
          <select v-model="role" class="ml-2 bg-ground" data-test="role">
            <option>viewer</option>
            <option>editor</option>
            <option>manager</option>
          </select>
        </label>
        <label v-if="role === 'viewer'">{{ t("access.folders") }}
          <input v-model="folders" class="ml-2 border border-rule bg-ground px-1" :placeholder="t('access.foldersHint')" data-test="folders" />
        </label>
        <div class="flex gap-2">
          <button type="button" class="border border-rule px-3" @click="adding = false">{{ t("devices.cancel") }}</button>
          <button type="submit" class="bg-accent px-3 text-accent-ink" data-test="add">{{ t("access.addAction") }}</button>
        </div>
      </form>
    </AsyncState>
  </section>
</template>
