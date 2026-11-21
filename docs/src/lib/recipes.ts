// Ready-made projects of the CRD IDE (content/docs/ide.mdx). `<…>` marks
// what to fill in; the IDE lists those blanks as issues until they are.

type Recipe = { id: string; label: string; description?: string; yaml: string };

export const RECIPES: Recipe[] = [
  {
    id: 'tenant-and-vault',
    label: 'A tenant and its vault',
    description: 'One team: its groups, its namespace, one vault its members edit.',
    yaml: `apiVersion: kura.weebo.io/v1alpha1
kind: KuraTenant
metadata:
  name: <tenant>
spec:
  claimMappings:
    claim: groups
    roles:
      owner: [<owners-group>]
      member: [<members-group>]
  vaultNamespaces: [<namespace>]
  quota: { storage: 20Gi, maxFileSize: 500Mi }
---
apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata:
  name: <vault>
  namespace: <namespace>
spec:
  tenantRef: { name: <tenant> }
  configSync: { categories: [plugins, snippets, themes] }
  bindings:
    - { role: editor, claimValue: <members-group> }
`,
  },
  {
    id: 'folder-viewers',
    label: 'A vault with folder-restricted viewers',
    description: 'Editors for the team, and readers who see two folders only, from the web reader.',
    yaml: `apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata:
  name: <vault>
  namespace: <namespace>
spec:
  tenantRef: { name: <tenant> }
  bindings:
    - { role: editor, claimValue: <members-group> }
    - { role: viewer, claimValue: <readers-group>, paths: [<folder>/, <other-folder>/] }
`,
  },
];
