# Documentation

Every project keeps its documentation here, either as plain markdown or as a
[fumadocs](https://fumadocs.dev/) static site (ask the project owner which one they want).

## Guides

- [Operating kura-server](operator.md): `server.yaml`, tenants and vaults, PostgreSQL, Authentik (phase 1).
- [API reference](api/openapi.json): the OpenAPI document generated from the handlers (`task dump-spec`).

## RFCs

| # | Title | Status | Settles |
| --- | --- | --- | --- |
| [0001](RFC/rfc0001-init-project.md) | weebo-kura: self-hosted sync backend for Obsidian vaults | Accepted ([implementation plan](RFC/rfc0001-implementation-plan.md)) | How vaults sync across devices through a self-hosted, multi-tenant backend authenticated by OIDC |

### Planned

Not written yet, on purpose: each waits for its trigger. A number is given
only when the RFC is written, one past the highest that ever existed.

| Subject | Trigger | Already framed by RFC 0001 |
| --- | --- | --- |
| Kura operator | After phase 3, once the server and the tenant/vault model run | Decisions 49 and 51: generated from `weebo-operator-kit`, uses the `weebo-kit-*` crates, separate binary driving Kura's admin API; `KuraInstance` kind; the tenant API arrives with it; `KuraTenant` and `KuraVault` files applied unchanged as custom resources |
| Kura TUI | After phase 7, once the read and search API ships | Decisions 6 and 41: a client of the backend with no local vault, built on the §4.4 read and search API |
| Chunk compression codec | Only if phase 2 measures a gain above 20% of stored bytes | Decision 38: the codec byte already in the chunk format; the earlier zstd crate review (decisions 21, 23) as input |
| Batched chunk download | Only if the phase 6 load test shows a first attach above 5 minutes | Decision 42: `POST /vaults/{v}/chunks/batch`, up to 4 MiB per call |
