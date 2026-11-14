# weebo-kura

Self-hosted, multi-tenant sync backend for Obsidian vaults, authenticated by
OIDC. Design: [RFC 0001](docs/RFC/rfc0001-init-project.md); build order and
progress: [implementation plan](docs/RFC/rfc0001-implementation-plan.md).

## Setup

```bash
mise trust && mise install && task init
```

`task --list` lists every command. The gates the pre-commit hook and CI run
are `task lint`, `task test`, `task coverage-check` and `task audit`.
