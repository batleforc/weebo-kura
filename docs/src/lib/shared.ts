import { defineSite } from '@weebo/operator-docs/site';

// What makes this site weebo-kura's; everything else comes from
// @weebo/operator-docs.
export const site = defineSite({
  name: 'weebo-kura',
  description:
    'A self-hosted, multi-tenant sync backend for Obsidian vaults: encrypted, deduplicated, signed in through OIDC.',
  git: {
    user: 'batleforc',
    repo: 'weebo-kura',
    branch: 'main',
    contentDir: 'docs/content/docs',
  },
});
