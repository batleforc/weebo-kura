# Operating kura-server (phases 1 and 2)

What a deployment needs today: the server's own `server.yaml`, the
`KuraTenant` and `KuraVault` documents, PostgreSQL, and Authentik. Design:
[RFC 0001](RFC/rfc0001-init-project.md) (§4.1, §6.10, §13); what differs from
it while it is built: the Divergences of the
[implementation plan](RFC/rfc0001-implementation-plan.md).

Phase 1 serves identity, devices, vaults and bindings; phase 2 adds encrypted
chunk and blob storage. Notes do not sync end to end yet (phase 3); the Helm
chart arrives in phase 6.

## 1. The process

```text
kura-server [--config /etc/kura/server.yaml]   serve (default config path shown)
kura-server migrate                             apply the database migrations, then exit
kura-server dump-spec                           print the OpenAPI document
kura-server maintain [--config …]               one maintenance pass now (§12), then exit
kura-server reset-epochs                        restore runbook: a new epoch for every vault (§12), as the owner
kura-server backup [--config …]                 copy new objects to the second bucket under the backup lock (§12)
kura-server check-chunks --sample N [--config …] decrypt N random chunks, fail on a bad one (§12)
```

| Variable | Read by | Value |
| --- | --- | --- |
| `KURA_DATABASE_URL` | serve, migrate, reset-epochs | PostgreSQL URL. `migrate` always reads it (the owner's credentials). `serve` reads the `kura_app` URL from Vault KV when `vault` is configured; this variable overrides it for local runs |
| `KURA_VAULT_TOKEN` | serve | Local runs only: a Vault token instead of Kubernetes auth |
| `KURA_CONFIG_DIR` | serve | The directory of resource documents, mounted from a ConfigMap without `subPath` |
| `KURA_BACKUP_ENDPOINT`, `_BUCKET`, `_REGION`, `_ACCESS_KEY_ID`, `_SECRET_ACCESS_KEY` | backup | The second bucket |
| `RUST_LOG` | serve | Log filter, `info` by default. Logs are JSON; audit events use the target `audit` |

Ports: the API on `8080`, Prometheus metrics on `9090` (never exposed by the
ingress). Probes: `GET /healthz` (process alive) and `GET /readyz` (a
configuration is loaded; it never checks a shared dependency, §10.2). The
image runs as UID 65532 and writes nothing to disk, so a read-only root
filesystem works.

## 2. PostgreSQL

1. Once per cluster, as a superuser (CloudNativePG: `postInitApplicationSQL`),
   run [`crates/kura-server/sql/bootstrap-roles.sql`](../crates/kura-server/sql/bootstrap-roles.sql).
   It creates `kura_app`: no superuser, no `BYPASSRLS`, `lock_timeout` and
   `statement_timeout` 5 s, `idle_in_transaction_session_timeout` 10 s. Set its
   password (or let CloudNativePG's managed roles do it).
2. Before each rollout, a job runs `kura-server migrate` as the database owner.
   Migrations only add (expand and contract, §13); a binary refuses to start
   against a schema older than itself and accepts a newer one.
3. The server connects as `kura_app`. Every table forces row-level security:
   a request sees one tenant's rows, or its caller's own devices.

Locally: `task dev:up` starts PostgreSQL 18 with podman and prints the
`KURA_TEST_DATABASE_URL` the integration tests read; `task test:db` runs them.

## 3. `server.yaml`

```yaml
oidc:
  issuer: https://auth.example.org/                     # https only; Authentik's global issuer mode
  discovery: https://auth.example.org/application/o/kura-cli/   # optional: where the keys are found (default: the issuer)
  audiences: [kura-plugin, kura-cli]                     # one client_id per client type, at least one
  algorithms: [RS256]                                    # pinned; never `none`; default [RS256]
  platformAdmin: { claim: groups, value: kura-platform-admins }
trustedProxies: [10.42.0.0/16]   # Traefik's pod CIDR; empty: X-Forwarded-For is ignored
cors:
  allowedOrigins: [app://obsidian.md, capacitor://localhost, http://localhost]   # the default
listen: { api: 0.0.0.0:8080, metrics: 0.0.0.0:9090 }                          # the default
vault:            # accepted now, read from phase 2 on
  address: https://vault.vault:8200
  kubernetesAuth: { mount: kubernetes, role: kura }
  secrets: { database: mv/data/kura/database, bucket: mv/data/kura/bucket }
maintenance:
  window: "01:00-05:00"   # UTC, the default; equal ends mean all day (§12)
```

Unknown fields are refused, so a typo stops the server instead of being
ignored. Platform admins (the `platformAdmin` claim) may read
`GET /v1/admin/status`.

## 4. Tenants and vaults

Every `*.yaml` file in `KURA_CONFIG_DIR`, several documents per file. The
server re-reads the directory every 10 s and at least once a minute; a change
is validated as a whole, and on any error the last valid configuration stays.

```yaml
apiVersion: kura.weebo.io/v1alpha1
kind: KuraTenant                 # cluster-scoped
metadata: { name: maxime }
spec:
  claimMappings:
    claim: groups
    roles: { owner: [kura-maxime-owners], admin: [], member: [kura-maxime] }
  vaultNamespaces: [maxime]      # namespaces allowed to declare vaults for this tenant
  retention: { maxRevisionAge: 180d, maxRevisionsPerEntry: 200, deletedEntryTtl: 90d }
  quota: { storage: 20Gi, maxFileSize: 500Mi }
  search: { enabled: true }
  deletionPolicy: Orphan         # or Delete
---
apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault                  # namespaced
metadata: { name: knowledge, namespace: maxime }
spec:
  tenantRef: { name: maxime }
  chunking: { min: 256Ki, avg: 1Mi, max: 4Mi }   # fixed once the vault exists
  configSync: { categories: [plugins, snippets, themes, hotkeys] }
  bindings:
    - { role: editor, claimValue: kura-maxime }  # or `subject: <OIDC sub>`, exactly one
  deletionPolicy: Orphan
```

Defaults are closed: no `claimMappings`, no members; no `bindings`, only the
tenant's `owner` and `admin` (who open every vault as `manager`); search off;
`.obsidian` not synced; `Orphan`.

Refused changes (§4.3): an unknown kind, field or role; duplicate names; a
vault outside its tenant's `vaultNamespaces` or naming an unknown tenant;
chunking not `min < avg < max` or above 4 MiB, or changed after creation; one
claim value mapped by two tenants; a file declaring a vault created through
the API; removing two tenants at once, or all of them. Each rejection is
logged at `warn` with `file#index field.path: message`, counted in
`kura_config_rejected_total`, and listed in `GET /v1/admin/status`.

Removing a document: `Orphan` disables the object and keeps its data;
`Delete` disables it and schedules its deletion 30 days later, cancelled if
the document comes back. The annotation `kura.weebo.io/force-orphan: "true"`
cancels the `Delete` cleanup. Objects not `Ready` are counted in
`kura_config_warnings`.

Objects declared in files are read-only through the API. A tenant `admin` may
create vaults (`POST /v1/vaults`, in the namespace named after the tenant)
and, with vault `manager`s, bind roles (`PUT /v1/vaults/{v}/bindings/claim:<value>`
or `user:<sub>`), never above their own role and only for the tenant's own
claim values or known members.

### Scoped tokens

For agents and scripts that read a vault without signing in (§6.2): issued by
a vault manager or tenant admin, `Authorization: Bearer kura_…`, read only,
one vault, an optional folder prefix, at most 90 days, from listed address
ranges only (never `0.0.0.0/0` or `::/0`). The client address is read
through `trustedProxies`: behind a proxy, a request that does not name its
client in a well-formed `X-Forwarded-For` is refused. A token dies with the
grant that let its creator issue it (the binding, or the claim value mapped
to `owner`/`admin`), when revoked, or at expiry; rows are purged 30 days
after expiry. Each use is an audit event `scoped_token_used`; a refused
address is an `authz_denied` event with `address_not_allowed`.

### Obsidian configuration

With `configSync: { categories: [plugins, snippets, themes, hotkeys] }` on
a vault, each user's `.obsidian` syncs per device profile (`desktop`,
`mobile`, from the device's registration), never between members, never to
a scoped token, never into search (decision 34). Only those categories
travel: plugin folders, snippets, themes, `hotkeys.json`; never
`community-plugins.json` (what is enabled), the workspace, or the Kura
plugin itself. The last write wins, with no conflict file; earlier versions
stay in history and `kura restore` brings one back. A new device takes the
server's copy of a file it also has (its own goes to `.trash`). A plugin
received from another device, or a new version of one, is written disabled
and enabled only when the user agrees on that device; that agreement is
kept on the device. Plugin settings (`data.json`) and CSS apply without
asking. `kura sync` syncs the desktop profile of a vault folder that has
`.obsidian`.

### Search

Off unless the tenant sets `search: { enabled: true }` (decision 39): the
index is the plaintext words of each Markdown note, in PostgreSQL and its
backups. Enabling queues every Markdown note (the workers of each replica
index 100 at a time); disabling drops the tenant's index. The database
needs `unaccent` and `pg_trgm` (created by the migration; both are trusted
extensions). `kura_index_queue_depth` and `KuraIndexQueueGrowing` watch the
queue; a note that fails 5 times stays in `index_job` with `failed_at`.

## 5. Vault and object storage

Per decision 48 and §6.6, the server logs in to Vault with its Kubernetes
service account (`vault.kubernetesAuth`) and needs a policy allowing only:

- `read` on the two KV v2 paths of `vault.secrets`:
  - database: `{"url": "postgres://kura_app:…@…/kura"}`, re-read every 5
    minutes, so a rotation reaches new connections without a restart;
  - bucket: `{"endpoint", "bucket", "region" (default us-east-1),
    "accessKeyId", "secretAccessKey"}`, read at start;
- `datakey`, `decrypt` and `rewrap` on the Transit keys `kura-<tenant slug>`
  of the mount `vault.transit.mount` (default `transit`).

That policy is [`deploy/vault/kura-server.hcl`](../deploy/vault/kura-server.hcl)
(edit the mount names if yours differ). It grants no delete and no key
configuration: the test `server_policy_cannot_delete_key` checks that this
token gets 403 on deleting a key, on `deletion_allowed` and on trimming
versions (T18).

The Vault login is renewed every 5 minutes: give the Kubernetes auth role a
token TTL of at least 10 minutes.

Creating each tenant's Transit key `kura-<slug>`, allowing its deletion and
deleting it are operator actions under a separate Vault role (decision 33).
Each vault's data key is created by Transit on first upload, stored only
wrapped, and cached in memory at most 10 minutes: during a short Vault
outage, vaults with a cached key keep working.

The bucket holds only ciphertext under keyed object names (`<vault id>/<keyed
hash>-<random>`), so it reveals neither content nor which known file is
stored. Chunks are at most 4 MiB; `quota.storage` and `quota.maxFileSize`
bound each tenant. Locally: `task dev:vault` (OpenBao, same API), `task dev:s3` (RustFS).

## 6. Authentik

Per §13 and §6.2:

- A device code flow, set on the brand (none ships by default).
- One public OAuth provider per client type (`kura-plugin`, `kura-cli`),
  matching `oidc.audiences`, both in **global issuer mode**, so one issuer
  (`oidc.issuer`, Authentik's base URL) serves both; `oidc.discovery` then
  names one application's discovery URL, since the base URL has none, with the `offline_access` and `profile` scope
  mappings, `include_claims_in_id_token` on, and a consent stage in its
  authorization flow.
- The groups named in `claimMappings` and `platformAdmin`.
- The provider's *access code validity* of at least 10 minutes: Authentik
  gives a device code that same lifetime, 1 minute by default, too short to
  type the code on another device.

The server reads discovery, the JWKS and userinfo from the issuer (5 s
timeout, no redirects). Keys are cached and stored in PostgreSQL, so a replica
started while Authentik is down still verifies tokens signed with a known key.
A token must carry `scope`: an ID token presented as an access token is
refused.

## 7. Devices

Each client registers once per install (`POST /v1/devices`, at most 20 live
per user) and sends the returned id in `X-Kura-Device`. Revoking a device
(`DELETE /v1/devices/{d}`, by its owner or a tenant admin) refuses it at once,
and refuses every token of the same Authentik session on any device and for
any new registration (plan D34, D48). The session is the token's `sid`, or,
for the tokens Authentik issues on a refresh (which carry no `sid`), the
owner's `auth_time`, the time that session signed in. End the session in
Authentik to invalidate the refresh token itself. A token with neither claim
is refused. Two sessions of one user signing in in the same second share a
revocation. Devices idle 180 days are purged daily; revoked ones are kept.

## 8. Signals

| Signal | Where |
| --- | --- |
| `kura_ready` | 1 once a configuration is loaded |
| `kura_config_rejected_total`, `kura_config_warnings` | configuration changes refused; objects not `Ready` |
| `kura_authz_denied_total{outcome}` | every refusal, by problem type |
| Log target `audit` | denials (problem, route template, client address, claimed device), device registration and revocation, binding and vault changes, configuration removals |
| `kura_chunk_corrupt_total` | stored chunks failing their check on read: alert on any |
| `QuotaBelowUsage` condition | in `GET /v1/admin/status`, a tenant storing more than its quota |
| `429` with `Retry-After` | per-device rate limits (§4.4), held in each replica's memory; the batch routes of RFC 0002 allow 60 calls a minute each |
| `kura_vault_epoch_changes_total` | epochs regenerated after a promotion or a restore: alert on any (every device resyncs) |
| `kura_commit_duration_seconds` | commit time; the budget is 200 ms at p95 (§10.3) |
| `kura_notify_lag_seconds`, `kura_notify_queue_usage` | a replica's heartbeat round trip through `NOTIFY`; `pg_notification_queue_usage()` |
| `kura_ws_connections` | open notification sockets per replica |
| `kura_vault_call_duration_seconds{outcome}` | Vault Transit calls |
| `kura_resync_total{outcome}` | `410` answers: devices running a full resync |
| `kura_unavailable_total` | `503` answers from a failed dependency; the `warn` log line names it |
| `kura_chunk_bytes_stored`, `kura_dedup_ratio`, `kura_quota_used_ratio` (by `tenant`) | storage, refreshed every minute |
| `kura_deletions_scheduled` | tenants and vaults removed with `Delete`, in their grace period |
| `kura_backup_last_success_timestamp_seconds`, `kura_storage_refreshed_timestamp_seconds` | the last bucket copy; the last storage refresh |
| `kura_index_queue_depth` | search index jobs waiting |
| `kura_batch_items_total{route,outcome}`, `kura_batch_response_bytes` | items asked of `blobs/batch` and `chunks/batch` (RFC 0002): `found` were fetched and sent, `missing` were unknown or not visible; the size of each chunk answer. A `chunks/batch` past the 8 a replica builds at once answers `429` with `Retry-After: 1`, so a busy replica never trips the dependency alert |
| Log target `audit`, `scoped_token_used`, `scoped_token_created`, `scoped_token_revoked` | every use of a scoped token (id, route, client), its issue and revocation |
| `kura_maintenance_failures_total` | vaults whose maintenance failed in a pass (the others went on): alert on a rise |
| `kura_maintenance_deleted_total{outcome}` | revisions, entries, blobs, chunks, objects, orphans and purged vaults removed by maintenance |
| `kura_tenants_awaiting_key_destruction` | tenants purged by `Delete` whose Transit key an operator must destroy: alert on any |
| Log target `audit`, `vault_purged` | a vault purged after its `Delete` grace period |

The chart's PrometheusRule ([`files/alerts.yaml`](../deploy/helm/kura/files/alerts.yaml),
each alert unit-tested by `task alerts:test`) raises: no replica ready, no
replica scraped, dependency failures and Vault failures for 2 minutes,
notification lag above 30 s for 5 minutes, the `NOTIFY` queue over half
full, a corrupt chunk, a refused configuration, a deletion scheduled with
`Delete`, a tenant awaiting key destruction, an epoch change, a tenant above
90% of its quota, stale storage metrics, a failed maintenance pass, and no
bucket copy for 36 hours. Search's `index_queue_depth`
comes with search. Logs carry a span per request (method, route template,
status, duration); there is no trace export yet.

## 9. The `kura` CLI

```bash
kura login --server https://kura.example.org --issuer https://auth.example.org/application/o/kura/
kura attach notes ~/Notes      # shows what will move, asks first (--yes to skip)
kura sync [notes] [--dry-run]
kura status
kura devices [revoke <name>]
kura watch [notes]             # stays in sync: pulls on every notification, pushes edits once quiet
kura sync notes --yes          # a full resync may move many files to .trash without asking
kura search notes "budget 2026" # notes and file names, best first (tenant search on)
kura token create notes --prefix agents/ --expires 30d --cidr 10.42.0.0/16
kura token list notes          # never shows a secret
kura token revoke notes <id>
kura history notes a.md        # the file's revisions, newest first
kura restore notes a.md <rev>  # that revision's content becomes a new revision, then syncs
kura verify notes              # the server holds exactly this folder's notes (exit 0), else lists what differs
```

A full resync follows a `410`: `cursor_expired` (the device was away longer
than `deletedEntryTtl`) moves files deleted elsewhere to `.trash` and pushes
local edits as new files; above 10% of the files or 50 it asks first (exit 3
when it cannot ask). `history_rewound` (the server was restored or failed
over) deletes nothing and pushes back what the server lost; a lost version
comes back as a conflict file beside the older one.

Exit codes: 0 done, 1 unexpected (server unreachable, local I/O), 2 sign in
again, 3 refused by the server (role, quota, client too old; local changes
kept), 4 some files cannot sync (named, with the rule they break). Its files
are in `$KURA_CLI_HOME`, else `~/.config/kura`: `config.json` and
`credentials.json` (the refresh token, mode 0600). `KURA_ACCESS_TOKEN`
replaces the sign-in for scripts and tests. The sync state of a folder is
`<folder>/.kura/state.json`; dot-folders (`.obsidian`, `.trash`, `.kura`)
never sync as notes; deleted files go to `<folder>/.trash`.

### Migrating from Notion (§13, phase 9)

1. Export the Notion workspace (Markdown & CSV, subpages included) and
   convert it with Obsidian's importer into an empty folder.
2. From the export date, Notion is read-only: remove its members' edit rights.
3. `kura attach notes <folder>` and answer **no**: it prints what would be
   pushed (files and bytes) and every name that cannot sync, with its rule,
   and changes nothing. Rename those and run it again until none is listed.
   Once attached, `kura sync notes --dry-run` gives the same check (exit 4
   while a name cannot sync).
4. `kura sync notes` pushes; `kura verify notes` must print `Equal.` (same
   file count, bytes and content as the folder).
5. Attach every other device (`kura attach`, the plugin); keep Notion
   readable for one month of use, then close it.

## 10. The Obsidian plugin

Kura Sync is built by `task plugin:build` into `plugin/main.js` (the WASM
core embedded, at most 3 MB), with `plugin/manifest.json` and
`plugin/styles.css`. Copy the three files into
`<vault>/.obsidian/plugins/weebo-kura/` and enable it (Obsidian 1.11.4 or
later). In its settings: the server URL, the sign-in URL (the `kura-plugin`
application's OIDC URL, e.g. `https://auth.example.org/application/o/kura-plugin/`),
then **Sign in** (device code), then the vault. The refresh token is kept in
Obsidian's `SecretStorage` under an id naming the server and the vault; the
sync state in the plugin's folder; neither syncs. A note open in the editor
is not overwritten: the remote change waits until it is closed. A full
resync that would trash many files asks once; "Sync now" asks again.

## 11. Local stack

Rootless podman, one container at a time, every port on loopback:

| Task | What |
|---|---|
| `task dev:stack` | PostgreSQL 18 (55433), OpenBao 2.7 dev mode (58200, token `kura-dev`), RustFS (59000), Authentik 2026.8 (https://localhost:59443), then `dev:seed` |
| `task dev:seed` | OpenBao KV `mv/kura/{database,bucket}`, Transit key `kura-dev`, database `kura`, `kura_app`, migrations |
| `task dev:run [RELEASE=1]` | `kura-server` with `dev/server.yaml` and `dev/config/` (API 58080, metrics 59090); `RELEASE=1` for the perf tasks |
| `task dev:maintain` | One maintenance pass now |
| `task dev:drill` | The restore drill of §12 on the local stack |
| `task perf:run:commit`, `:pull`, `:resync`, `perf:attach`, `perf:soak` | k6 load (§10.3), decision 42's first attach of 10,000 notes, the memory soak |
| `task dev:token [AS=alice]` | An access token by the device code flow, no browser |
| `task obsidian:ca:trust` / `obsidian:ca:untrust` | Trust (or stop trusting) the dev CA system-wide with `sudo trust anchor`, so Obsidian accepts Authentik on `https://localhost:59443` |
| `task android:setup`, `android:start`, `android:obsidian [APK=…]`, `android:deploy VAULT=…` | An Android emulator (Google APIs image, about 3 GB under `~/Android/Sdk`, KVM) for the plugin on mobile: the dev CA goes into its system store (`adb root`, `-writable-system`), `adb reverse` maps its `localhost:58080` and `:59443` to this PC, so the stack stays on loopback and the URLs are the desktop's ([`dev/android.sh`](../dev/android.sh)) |
| `task obsidian:deploy VAULT=<folder>` | Build the plugin, copy it into that Obsidian vault and print its settings; the dev vault `obsidian` syncs `.obsidian` (plugins, snippets, themes, hotkeys), `notes` is the heavy suites' |
| `task dev:down` | Removes the containers and the network |
| `task test:heavy` | The §6.P proof with the real `kura` binary, once `dev:run` serves (`tests/heavy/sync.sh`, `authz.sh`; `KURA_DEV_LOG=<file>` checks observation 3's audit event) |

Authentik serves a certificate from a local CA (`task dev:certs`, in
`dev/certs/`, gitignored), because the issuer must be https; `dev:run` trusts
it through `SSL_CERT_FILE`. Its admin password is generated once into
`dev/.env.local`. The blueprint `dev/authentik/blueprints/kura.yaml` creates
the `kura-cli` client, the device code flow and three users (password
`kura-dev`): `alice` owns tenant `dev` and is a platform admin, `bob` is a
member, `carol` is in no group.

```bash
task dev:stack && task dev:run        # in one terminal
T=$(task dev:token AS=alice)          # in another
curl -H "authorization: Bearer $T" -H 'content-type: application/json' \
  -d '{"name":"laptop","profile":"desktop"}' http://127.0.0.1:58080/v1/devices
```

## 12. Maintenance, backups and restore

Inside `maintenance.window`, each replica tries one pass an hour; one runs at
a time (an advisory lock). A pass, per vault, in batches of 1,000 with a 1 s
pause:

1. prunes revisions past the tenant's `retention` (`maxRevisionAge`,
   `maxRevisionsPerEntry`); a file's current version, or its deletion, is
   never pruned;
2. purges entries deleted longer than `deletedEntryTtl` (default 90 days)
   and moves the vault's horizon: a device away longer resyncs (§9);
3. collects blobs and chunks nothing references, untouched 24 hours, rows
   first, then their objects, and sweeps objects with no row older than 24
   hours;
4. purges vaults whose `Delete` grace period (30 days) passed: rows, then
   every object under the vault's prefix, then the vault row (kept while an
   object remains, retried next pass). The first batch gives the vault a new
   epoch: if its document comes back mid-purge, devices push their copies
   back. A force-orphan vault is never purged with its tenant. A tenant's
   Transit key is kept: destroy it under the operator role, then the tenant
   leaves `kura_tenants_awaiting_key_destruction`.

`kura-server maintain` runs one pass at once (`task dev:maintain` locally).

**Backups.** Three parts, all required (§10.3):

1. PostgreSQL: CloudNativePG archives the WAL continuously and takes a base
   backup daily (`cnpg.backup` in the chart), so it can be recovered to any
   point in time.
2. The bucket: `kura-server backup` (the chart's `backup` CronJob, outside
   the maintenance window) takes the backup lock `7743220896921379691`
   (`0x6b7572616261636b`, per database: it connects to `kura`), so
   maintenance deletes no chunk or object meanwhile, records the database
   time **T**, copies every object the second bucket lacks (keys are unique
   per upload, so a present key is the same bytes), checks it still holds
   the lock, writes `backups/<T>.keys` (every key at T) and
   `backups/<T>.json`, records the run (`kura_backup_last_success_timestamp_seconds`,
   alert `KuraBackupMissing` after 36 hours), and releases the lock. Any
   other backup tool must hold the same lock while it copies. The second
   bucket is never pruned by the tool. To prune it, delete only keys that
   **no kept** `backups/<T>.keys` lists: never by age, since a chunk keeps
   its key for as long as a note keeps it.
3. Vault: a Raft snapshot **after** each bucket copy, kept as long as the
   restore points it covers; never trim a Transit key version older than
   the oldest kept base backup.

**Restore** (the drill, `task dev:drill` locally):

1. Restore Vault's first snapshot taken after T: an older one may lack
   the Transit key of a tenant created before T.
2. Recover PostgreSQL to **T** of the bucket copy you restore
   (CloudNativePG `bootstrap.recovery` with `recoveryTarget.targetTime`):
   every object the database names at T was in that copy.
3. Copy the second bucket's objects back (`rclone copy` or `mc mirror`).
4. `kura-server migrate`, then `kura-server reset-epochs`, as the owner.
5. `kura-server check-chunks --sample 200`, with the owner's
   `KURA_DATABASE_URL`: decrypts random chunks with their vault's key and
   checks their address; it fails on any bad one.

Every device then gets `410 history_rewound` and pushes back what the
backup lacked, deleting nothing. A failover to a standby needs no step: the
new timeline changes the epochs by itself (`kura_vault_epoch_changes_total`).
A planned switchover changes the timeline too, so every device resyncs
under `history_rewound` then: nothing is deleted, but an edit the old
primary had not replicated comes back as a conflict file, and a deletion it
had not replicated is undone.

## 13. Installing with Helm

The chart is `deploy/helm/kura` (`task helm:lint` renders and validates it).
Prerequisites (§13 of the RFC): the CloudNativePG operator and the
Prometheus operator CRDs, Vault with the KV secrets, a Transit key
`kura-<tenant>` per tenant and the Kubernetes auth role `kura` bound to the
ServiceAccount `kura` with [`deploy/vault/kura-server.hcl`](../deploy/vault/kura-server.hcl),
Authentik as in §6, a DNS name and its TLS Secret.

```bash
helm install kura deploy/helm/kura -n kura --create-namespace -f my-values.yaml
```

| Values | What |
| --- | --- |
| `server` | `server.yaml` (§3), rendered as is; `listen` is set by the chart |
| `documents` | the tenant and vault documents, one key per file, re-read live |
| `ingress.host`, `ingress.tlsSecret` | the API's public name; metrics are never routed |
| `server.trustedProxies` | the ingress controller's pod CIDR |
| `cnpg.*` | a two-instance CloudNativePG cluster with barman (`cnpg.backup.destinationPath`), its bootstrap creating `kura_app`; off: set `migrate.databaseUrlSecret` |
| `backup.*` | the bucket copy CronJob and the Secret naming the second bucket (`KURA_BACKUP_*`) |
| `monitoring.*` | the ServiceMonitor and the PrometheusRule of §8 |

After the first install (the CloudNativePG cluster and its owner Secret
exist only then; the server waits, refusing an empty schema) and before
every upgrade, a hook Job runs `kura-server migrate` with the owner's URL
(`<cnpg.name>-app`). The chart refuses to render without the PostgreSQL
and bucket backups unless `backupsWaived: true`. Pods run as UID 65532 with a read-only
root filesystem, two replicas spread over nodes, a disruption budget of one,
and NetworkPolicies admitting the API from `networkPolicy.ingressNamespace`,
metrics from `networkPolicy.monitoringNamespace`, and PostgreSQL only from
kura's pods, its own instances and the CloudNativePG operator
(`cnpg.operatorNamespace`). After installing,
create the `kura_app` login password (CloudNativePG managed role, or
`ALTER ROLE`) and store its URL in the KV path `vault.secrets.database`.
