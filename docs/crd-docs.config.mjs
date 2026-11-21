// Input of `weebo-crd-docs` (@weebo/operator-docs), run by `task recu`
// (`pnpm gen:crds`): ../deploy/crd (kura-crdgen) → content/docs/crds and
// public/crd-schemas. KuraTenant and KuraVault are documents read from files
// today and custom resources once the operator lands; the YAML is the same.
import { defineConfig } from "@weebo/operator-docs/crd-docs";

export default defineConfig({
  // `KuraVault` → example named `my-vault`.
  kindPrefix: "Kura",
  exampleNamespace: "maxime",
  indexIntro:
    "Kura reads these documents from the files of `KURA_CONFIG_DIR` (the chart's `documents`), reconciled live; the future operator applies the same YAML as custom resources. Each page lists every field, its default and its rules.",
  order: ["KuraTenant", "KuraVault"],
  guides: {
    KuraTenant: ["Tenants and vaults", "/docs/guides/tenants-and-vaults"],
    KuraVault: ["Tenants and vaults", "/docs/guides/tenants-and-vaults"],
  },
  examples: {
    KuraTenant: {
      claimMappings: {
        claim: "groups",
        roles: { owner: ["kura-maxime-owners"], member: ["kura-maxime"] },
      },
      vaultNamespaces: ["maxime"],
      retention: { maxRevisionAge: "180d", maxRevisionsPerEntry: 200, deletedEntryTtl: "90d" },
      quota: { storage: "20Gi", maxFileSize: "500Mi" },
    },
    KuraVault: {
      tenantRef: { name: "maxime" },
      configSync: { categories: ["plugins", "snippets"] },
      bindings: [{ role: "editor", claimValue: "kura-maxime" }],
    },
  },
});
