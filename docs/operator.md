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
```

| Variable | Read by | Value |
| --- | --- | --- |
| `KURA_DATABASE_URL` | serve, migrate | PostgreSQL URL. `migrate` always reads it (the owner's credentials). `serve` reads the `kura_app` URL from Vault KV when `vault` is configured; this variable overrides it for local runs |
| `KURA_VAULT_TOKEN` | serve | Local runs only: a Vault token instead of Kubernetes auth |
| `KURA_CONFIG_DIR` | serve | The directory of resource documents, mounted from a ConfigMap without `subPath` |
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
  issuer: https://auth.example.org/application/o/kura/   # https only
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
  matching `oidc.audiences`, with the `offline_access` and `profile` scope
  mappings, `include_claims_in_id_token` on, and a consent stage in its
  authorization flow.
- The groups named in `claimMappings` and `platformAdmin`.

The server reads discovery, the JWKS and userinfo from the issuer (5 s
timeout, no redirects). Keys are cached and stored in PostgreSQL, so a replica
started while Authentik is down still verifies tokens signed with a known key.
A token must carry `scope`: an ID token presented as an access token is
refused.

## 7. Devices

Each client registers once per install (`POST /v1/devices`, at most 20 live
per user) and sends the returned id in `X-Kura-Device`. Revoking a device
(`DELETE /v1/devices/{d}`, by its owner or a tenant admin) refuses it at once,
and refuses every token of the same Authentik session (`sid`) on any device
and for any new registration (plan D34); end the session in Authentik to
invalidate the refresh token itself. A token without `sid` is refused. Devices idle 180
days are purged daily; revoked ones are kept.

## 8. Signals

| Signal | Where |
| --- | --- |
| `kura_ready` | 1 once a configuration is loaded |
| `kura_config_rejected_total`, `kura_config_warnings` | configuration changes refused; objects not `Ready` |
| `kura_authz_denied_total{outcome}` | every refusal, by problem type |
| Log target `audit` | denials (problem, route template, client address, claimed device), device registration and revocation, binding and vault changes, configuration removals |
| `kura_chunk_corrupt_total` | stored chunks failing their check on read: alert on any |
| `QuotaBelowUsage` condition | in `GET /v1/admin/status`, a tenant storing more than its quota |
| `429` with `Retry-After` | per-device rate limits (§4.4), held in each replica's memory |

## 9. Local stack

Rootless podman, one container at a time, every port on loopback:

| Task | What |
|---|---|
| `task dev:stack` | PostgreSQL 18 (55433), OpenBao 2.7 dev mode (58200, token `kura-dev`), RustFS (59000), Authentik 2026.8 (https://localhost:59443), then `dev:seed` |
| `task dev:seed` | OpenBao KV `mv/kura/{database,bucket}`, Transit key `kura-dev`, database `kura`, `kura_app`, migrations |
| `task dev:run` | `kura-server` with `dev/server.yaml` and `dev/config/` (API 58080, metrics 59090) |
| `task dev:token [AS=alice]` | An access token by the device code flow, no browser |
| `task dev:down` | Removes the containers and the network |

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
