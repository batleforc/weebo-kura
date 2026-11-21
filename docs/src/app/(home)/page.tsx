import { OperatorHome } from '@weebo/operator-docs/site';
import { site } from '@/lib/shared';

export default function HomePage() {
  return (
    <OperatorHome
      site={site}
      label="Self-hosted sync · Obsidian"
      caption="Sync Obsidian vaults across devices through your own server: notes stored as encrypted, deduplicated chunks on S3, people signed in through OIDC, tenants and vaults declared as YAML."
      actions={[
        { href: '/docs/guides/install', label: 'Get started', primary: true },
        { href: '/docs', label: 'Read the docs' },
        { href: '/docs/ide', label: 'Compose tenants and vaults' },
      ]}
      install={`helm install kura \\
  oci://ghcr.io/batleforc/weebo-kura/charts/kura \\
  --namespace kura --create-namespace -f values.yaml`}
      example={{
        title: 'A vault',
        yaml: `apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata:
  name: knowledge
  namespace: maxime
spec:
  tenantRef: { name: maxime }
  configSync: { categories: [plugins, snippets] }
  bindings:
    - { role: editor, claimValue: kura-maxime }`,
      }}
      features={[
        {
          title: 'Your server, your keys',
          body: 'Chunks are encrypted with per-vault keys wrapped by Vault Transit before they reach the bucket; the server never stores a password.',
          href: '/docs/concepts/security',
          link: 'Security model',
        },
        {
          title: 'Never loses a note',
          body: 'Every write is a revision; concurrent edits merge or keep both copies; deleted notes stay restorable for the retention you set.',
          href: '/docs/concepts/sync',
          link: 'How sync works',
        },
        {
          title: 'On the weebo kit',
          body: 'Tenants and vaults follow the kit’s CRD conventions, ready for the operator; signed image, chart and CLI on every release.',
          href: '/docs/crds',
          link: 'Resource reference',
        },
      ]}
    />
  );
}
