# Documentation

The documentation of weebo-kura, as plain Markdown.

## Guides

- [Operating kura-server](operator.md): `server.yaml`, tenants and vaults, PostgreSQL, Authentik (phase 1).
- [API reference](api/openapi.json): the OpenAPI document generated from the handlers (`task dump-spec`).

## RFCs

| # | Title | Status | Settles |
| --- | --- | --- | --- |
| [0001](RFC/rfc0001-init-project.md) | weebo-kura: self-hosted sync backend for Obsidian vaults | Accepted ([implementation plan](RFC/rfc0001-implementation-plan.md)) | How vaults sync across devices through a self-hosted, multi-tenant backend authenticated by OIDC |
| [0002](RFC/rfc0002-batched-transfers.md) | Batched chunk and manifest transfers | Implemented ([implementation plan](RFC/rfc0002-implementation-plan.md)) | How a device downloads (and, in phase 3, uploads) thousands of small files without one request per chunk and per manifest |
| [0003](RFC/rfc0003-web-console-and-notifications.md) | Web console (front and back office) and notifications | Implemented ([implementation plan](RFC/rfc0003-implementation-plan.md)) | How members and managers use Kura from a browser, and how a tenant is notified of events on a webhook |
| [0004](RFC/rfc0004-scoped-token-folders-and-write.md) | Scoped tokens: several folders, and write access | Implemented ([implementation plan](RFC/rfc0004-implementation-plan.md)) | How a scoped token is limited to several folders of one vault, and may write under some of them |
| [0005](RFC/rfc0005-folder-restricted-viewers.md) | Folder-restricted viewers and the web reader | Implemented ([implementation plan](RFC/rfc0005-implementation-plan.md)) | How people without Obsidian read and search the folders of a vault that a manager opens to them, and nothing else |
| [0006](RFC/rfc0006-device-sync-state.md) | Sync state per device and per vault | Implemented | How the console shows, for one vault, which devices sync it, when each last caught up, and how far behind it is |
| [0007](RFC/rfc0007-folder-entries.md) | Folders as synced entries | Implemented | How a folder, empty or not, reaches every device and the console: created, renamed and removed like a note |
| [0008](RFC/rfc0008-console-config-view.md) | The synced Obsidian configuration in the console | Implemented | How a member sees, from a browser, which Obsidian configuration files sync for them, per device profile, and reads the ones that hold no secret |

### Planned

Not written yet, on purpose: each waits for its trigger. A number is given
only when the RFC is written, one past the highest that ever existed.

| Subject | Trigger | Already framed by RFC 0001 |
| --- | --- | --- |
| Kura operator | After phase 3, once the server and the tenant/vault model run | Decisions 49 and 51: generated from `weebo-operator-kit`, uses the `weebo-kit-*` crates, separate binary driving Kura's admin API; `KuraInstance` kind; the tenant API arrives with it; `KuraTenant` and `KuraVault` files applied unchanged as custom resources |
| Kura TUI | After phase 7, once the read and search API ships | Decisions 6 and 41: a client of the backend with no local vault, built on the §4.4 read and search API |
| Chunk compression codec | Only if phase 2 measures a gain above 20% of stored bytes | Decision 38: the codec byte already in the chunk format; the earlier zstd crate review (decisions 21, 23) as input |
