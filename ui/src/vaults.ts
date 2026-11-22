/** The caller's vaults, once per sign-in, shared by the sidebar, the home page and the vault pages. */

import { defineStore } from "pinia";
import { ref } from "vue";

import { withDevice } from "@/api/client";
import { listVaults } from "@/api/generated/sdk.gen";
import type { Vault } from "@/api/generated/types.gen";

export const useVaults = defineStore("vaults", () => {
  const list = ref<Vault[]>([]);
  const loaded = ref(false);
  let pending: Promise<Vault[]> | null = null;
  /** The list, fetched once; `force` fetches again (after a vault is created or deleted). */
  async function load(force = false): Promise<Vault[]> {
    if (loaded.value && !force) return list.value;
    pending ??= withDevice(
      async (headers) =>
        (await listVaults({ headers, throwOnError: true })).data,
    ).finally(() => {
      pending = null;
    });
    list.value = await pending;
    loaded.value = true;
    return list.value;
  }
  return { list, loaded, load };
});

/** One tab of a vault: the page, and the i18n key of its label. */
export interface Section {
  name: string;
  to: string;
  label: string;
}

/**
 * A vault's sections, shared by its header and the sidebar: the overview and
 * the notes for everyone, the back office for its managers and the tenant's
 * admins. The API decides; this only chooses what to show.
 */
export function sectionsOf(
  v: Vault,
  tenants: readonly { slug: string; role: string }[] = [],
): Section[] {
  const base = `/vaults/${v.id}`;
  const manages =
    v.role === "manager" ||
    tenants.some((x) => x.slug === v.tenant && x.role !== "member");
  return [
    { name: "vault", to: base, label: "vault.overview" },
    { name: "vault-notes", to: `${base}/notes`, label: "vault.notes" },
    // RFC 0008: every member's own settings; the page says when the vault
    // syncs none. A restricted viewer's scope never holds them.
    ...(!v.paths
      ? [{ name: "vault-config", to: `${base}/config`, label: "vault.config" }]
      : []),
    ...(manages
      ? [
          {
            name: "vault-access",
            to: `${base}/access`,
            label: "manage.access",
          },
          { name: "vault-keys", to: `${base}/keys`, label: "manage.keys" },
          {
            name: "vault-notifications",
            to: `${base}/notifications`,
            label: "manage.notifications",
          },
        ]
      : []),
  ];
}

/** The section a route belongs to: a note, its history and search are Notes. */
export function sectionOf(routeName: unknown): string {
  const name = String(routeName ?? "");
  if (
    [
      "vault-config",
      "vault-access",
      "vault-keys",
      "vault-notifications",
    ].includes(name)
  )
    return name;
  return name === "vault" ? "vault" : "vault-notes";
}
