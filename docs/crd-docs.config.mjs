// Input of `weebo-crd-docs` (@weebo/operator-docs), run by `task recu`
// (`pnpm gen:crds`): ../deploy/crd (kura-crdgen) → content/docs/crds and
// public/crd-schemas. KuraTenant and KuraVault are read from files or applied
// as custom resources through kura-operator (RFC 0009); the YAML is the same.
import { defineConfig } from "@weebo/operator-docs/crd-docs";

export default defineConfig({
  // `KuraVault` → example named `my-vault`.
  kindPrefix: "Kura",
  exampleNamespace: "maxime",
  indexIntro:
    "Kura reads these documents from the files of `KURA_CONFIG_DIR` (the chart's `documents`), reconciled live; or applied as custom resources through kura-operator, whose `KuraInstance` names the server. The YAML is the same. Each page lists every field, its default and its rules.",
  order: ["KuraInstance", "KuraTenant", "KuraVault"],
  guides: {
    KuraInstance: ["Operator", "/docs/guides/operator"],
    KuraTenant: ["Tenants and vaults", "/docs/guides/tenants-and-vaults"],
    KuraVault: ["Tenants and vaults", "/docs/guides/tenants-and-vaults"],
  },
  examples: {
    KuraInstance: {
      url: "http://kura.kura.svc:8080",
      auth: { serviceAccountToken: { audience: "kura" } },
    },
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
