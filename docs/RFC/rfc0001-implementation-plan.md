# RFC 0001 — implementation plan

Companion to [RFC 0001](rfc0001-init-project.md) (Accepted, 2026-11-14). The
RFC says *what*; this file says *in which order*, cut into pull requests that
each leave the tree green. Phase 1 is detailed; later phases are detailed when
the previous one closes, because what phase N learns changes phase N+1.

Section numbers (§) and decision numbers refer to the RFC. The accepted RFC
is not edited while it is built (house rule of `batlehub-rfc`): every point
where the build differs from the text goes to the **Divergences** log at the
end of this file, with its reason. At landing, that log becomes the RFC's
`§18 Revision against the tree`; the sources it pins (versions, measurements)
become entries of `§17 References and sources`, where the RFC already sends
the measurements of decisions 52 to 55.

## Progress

Updated as each item lands. Commits are made by the maintainer (signed), so
"done" means green in the working tree, not yet merged.

| Item | Status | Date | Notes |
| --- | --- | --- | --- |
| P1 | done | 2026-11-14 | Tag `v0.31.0` exists at `0d145a8` on `github.com/batleforc/weebo-operator-kit` |
| P2 | done | 2026-11-14 | `k8s-openapi` 0.28 with `v1_36` in the workspace dependencies |
| P3 | done | 2026-11-14 | `task dev:up` / `dev:down` / `test:db` (podman, `postgres:18-alpine` on `127.0.0.1:55433`); CI's `test` job runs a PostgreSQL service so nothing skips there. No devfile sidecar yet: no Che workspace has run this repo; add it with the first one |
| PR 1.1 | done | 2026-11-14 | `task lint`, `task test`, `task audit` green; image builds with podman (29 MB) and Trivy finds nothing. `task coverage-check` has no line to measure until PR 1.2. D2–D5 |
| PR 1.2 | done | 2026-11-14 | 24 unit tests (one per §4.3 document row, defaults, quantities, durations, names), `kura-api` line coverage 96%, `fuzz_config_document` 1.25 M runs in 60 s without a finding. D6–D9 |
| PR 1.3 | done | 2026-11-14 | axum 0.8 app, `server.yaml` loader (oidc row of §4.3, `none` unrepresentable), probes, metrics port, `CancellationToken` + `JoinSet` shutdown, RFC 9457 problems for every §4.4 type, `X-Kura-Client` gate, `dump-spec` → `docs/api/openapi.json`, contract test, Spectral at zero findings, drift gate `task openapi:check` in CI. Workspace line coverage 97%. The T19 log test lands with the first route that has a path parameter (PR 1.7). D10–D13 |
| PR 1.4 | done | 2026-11-14 | Migration `0001` (six tables, forced RLS, owner and `kura_app` policies, three named definer functions), `sql/bootstrap-roles.sql`, `kura-server migrate`, startup schema check with `set_ignore_missing`. 8 integration tests green against PostgreSQL 18, including the gate `every_table_forces_rls` and its self-check. D14–D17, E1 (retired) |
| PR 1.5 | done | 2026-11-14 | Migration `0002` (write guard `kura_config_begin` with numeric versions, eight definer functions), `reconcile::{files, status, snapshot}`, background reconciler and cache tasks, readiness latched on the first stored snapshot. 7 integration tests: `invalid_document_keeps_last_config` (T12), older replica never overwrites, `Orphan` / `Delete` / force-orphan, `restored_document_cancels_deletion`, stored state feeding validation, API bindings kept, fresh install unready until stored. `GET /v1/admin/status` is mounted with authentication in PR 1.6. D18–D22 |
| PR 1.6 | done | 2026-11-14 | `auth::Verifier::verify_access_token` (pinned algorithms, issuer, audience, expiry, `scope` required), JWKS cache of 16 keys refetched at most once a minute through OIDC discovery and persisted with `kura_jwks_replace`, userinfo fallback cached for the token lifetime (10,000 entries), `Caller` extractor, `GET /v1/admin/status` mounted for platform admins. T1: `rejects_token_for_other_audience`, `rejects_alg_none`, `rejects_id_token_as_access_token`, `rejects_expired_token`, plus unknown key, key confusion, userinfo; `jwks_survive_a_restart_without_the_provider` against PostgreSQL. D23–D25 |
| PR 1.7 | done | 2026-11-14 | `POST`/`GET /v1/devices`, `DELETE /v1/devices/{d}`, `auth::require_device` (`DeviceCaller`), migration `0003` (`device_tenant` and four definer functions), daily purge task, `authz::memberships`. 6 end-to-end HTTP tests on PostgreSQL: `revoked_device_is_refused`, `revoked_sid_cannot_register` (T2), ownership and the 20-device limit, tenant admin scope, admin status, purge keeping revoked sessions. D26–D28 |
| PR 1.8 | done | 2026-11-14 | `authz::{require_role, vault_access_in, check_binding}`: effective role is the highest of the tenant role's grant and every matching binding; `GET`/`POST /v1/vaults`, `DELETE /v1/vaults/{v}`, `PUT`/`DELETE /v1/vaults/{v}/bindings/{b}`; file-declared objects read-only. Tests on PostgreSQL: `foreign_vault_is_404` (byte-identical to a missing vault, T4), `manager_cannot_bind_foreign_group`, `manager_cannot_grant_above_own_role` (T15), API vault lifecycle, and the T19 logging half `logs_name_the_route_template_not_the_path`. D29–D31 |
| PR 1.9 | done | 2026-11-14 | `limits::RateLimiter` (per device, per subject before registration, §4.4 defaults, `429` with `Retry-After`) and its gate `every_route_has_a_limit`; 64 KiB body limit answered `413 payload_too_large`; CORS allowlist without credentials; `net::client_addr` from `trustedProxies` only; one audit middleware logging every denial (target `audit`) and counting `kura_authz_denied_total{outcome}`. Tests: `denial_writes_audit_event` (T10 denial half, with the T19 logging half in `tests/logs.rs`), 429, CORS, body limit, client address. D32–D33 |
| PR 1.10 | done | 2026-11-14 | Per-crate coverage gate (`kura-api` 99.5% lines, `kura-server` 92.6%, with PostgreSQL), nightly fuzz workflow (`.github/workflows/fuzz.yml`), §8 as built (below), operator guide `docs/operator.md`. Every gate green on 2026-11-14: `task lint` (clippy, rustfmt, `fuzz:check`, Spectral at zero findings, shellcheck), `task audit` (cargo audit, cargo deny, Trivy on lockfiles, Containerfiles and image), gitleaks, 13 test binaries |
| Review | done | 2026-11-14 | Independent security review of phase 1 (R1–R7 below); R1, R2 and R4 fixed with regression tests, the rest recorded. Also verified: the release binary migrates, serves as `kura_app`, turns ready, answers metrics on its own port and stops on SIGTERM; PostgreSQL over TLS `verify-full` (rustls with ring and aws-lc-rs both linked) passes the database and HTTP suites. D34, D35 |
| Phase 1 | **closed** | 2026-11-14 | Exit criteria (§16) met: T1, T2, T4, T12, T15 and the denial half of T10 green; `fuzz_config_document` runs nightly in CI; lint, audit and the 80% gate green; drift gate active; §8 completed. Open: a devfile sidecar for Che (P3), commits by the maintainer |
| Phase 2 | in progress | 2026-11-14 | T6, T8, T9 green; independent review done (P1–P6 below, P1–P4 and P6 fixed). Open: PR 2.6 measurements (maintainer, real vault), so the phase is not closed |
| PR 2.1 | done | 2026-11-14 | `vault.rs` on `vaultrs` 0.8: Kubernetes auth (or `KURA_VAULT_TOKEN` locally), KV v2 reads of the database and bucket secrets, database credentials re-read every 5 min and applied to new connections (`set_connect_options`); `task dev:vault` and a CI Vault service; 2 integration tests against Vault in dev mode. The S3 service moves to PR 2.4, its first user. D36 |
| PR 2.2 | done | 2026-11-14 | `kura-core::chunk`: `ChunkHash` (BLAKE3, lowercase hex only), `ChunkParams` checked against FastCDC 2020's bounds (its own checks are debug assertions), `chunks()` with the reassembly, bound and locality properties tested; `fuzz_chunking` (1.5 M runs, no finding); the same bounds added to `kura_api::validate`; bare YAML integers accepted as quantities; `kura-core` joins the coverage gate. D37 |
| PR 2.3 | done | 2026-11-14 | `store::crypto`: stored chunk = format byte, codec byte 0, random 96-bit nonce, AES-256-GCM with vault id, chunk hash, format and codec as associated data, plaintext hash rechecked on read; object key = keyed BLAKE3 (`derive_key`) of the chunk hash plus a random suffix; `DataKey` wiped on drop, redacted. `store::keys`: Transit `datakey`, `wrapped_dek` written once and read back (migration `0004`), cache of 1,000 keys for 10 min. Tests: seal/open, every wrong address/vault/key/byte refused, no plaintext at rest, and `two_replicas_creating_one_data_key_keep_one` against PostgreSQL and Vault. D38 |
| PR 2.4 | done | 2026-11-14 | `object_store` 0.14 on S3 (30 s timeout, 3 retries in total), migration `0005` (`chunk`, `blob`, `blob_chunk`, forced RLS), `POST chunks/missing` (refreshes `last_touched`), `PUT`/`GET chunks/{hash}` (hash recomputed before anything is stored, storage before the row, orphan object deleted on a lost race, 4 MiB route limit, immutable caching, `kura_chunk_corrupt_total`), `PUT`/`GET blobs/{hash}`; `task dev:s3` (RustFS) and a CI S3 service. Tests on PostgreSQL + Vault + S3: `hash_mismatch_is_422` (T6), `bucket_object_is_not_plaintext` (T8), `oversized_chunk_is_413` (T9), resume from missing chunks, manifests. D39–D40 |
| PR 2.5 | done | 2026-11-14 | Quota: `quota.storage` refuses a write past it (`422 quota_exceeded`), `maxFileSize` refuses a larger manifest (`413`), `QuotaBelowUsage` advisory condition in `GET /v1/admin/status` from `kura_tenant_usage()` (migration `0006`). Tests `quota_and_roles_bound_uploads`, `quota_below_usage_is_reported` |
| PR 2.6 | **open** | | Needs a copy of the real vault, so the maintainer: deduplication and request counts per chunking setting (decision 52), zstd level 3 gain (decision 38). `kura_core::chunks` is the chunker to measure with |

## 0. Before the first line of code

| # | Item | Why | Blocking for |
| --- | --- | --- | --- |
| P1 | Pin `weebo-operator-kit` at tag `v0.31.0` (latest, 2026-11-13) as the git dependency of `kura-api` | §4.1, decision 49 | PR 1.2 |
| P2 | `weebo-kit-api` pulls `k8s-openapi` 0.28 with no `v1_*` feature: the kit leaves the Kubernetes version to the application, and two versions in one graph fail the build. The kura workspace enables one, `v1_36` like the kit's own tests, and the future operator must pick the same | Kit `Cargo.toml`, read 2026-11-14 at `deb5d6e` | PR 1.2 |
| P3 | Local services for integration tests: PostgreSQL in phase 1, plus MinIO and Vault (dev mode) from phase 2. In Eclipse Che as `devfile.yaml` sidecars; outside Che, a `task dev:up` running the same images with podman | No container engine in Che; §14 integration tests are opt-in by env var | PR 1.4 |
| P4 | Add the dependencies §8 does not list but phase 1 needs, each checked by `cargo deny` before landing, and log them in Divergences (see table below) | §8 "Typosquatting" and supply-chain rule | Each PR that adds one |

Dependencies phase 1 needs that §8 does not name yet (proposal, to confirm in
the Divergences log):

| Need | Candidate | Note |
| --- | --- | --- |
| YAML parsing of resource documents and `server.yaml` | `serde_yaml_ng` or `serde-saphyr` | `serde_yaml` is unmaintained; pick the one `cargo deny` accepts |
| Watch `KURA_CONFIG_DIR` (§6.10) | `notify` | Or, smaller: only the 60 s re-read, if the watch proves flaky on the `..data` swap. Shortcut to record if taken |
| Per-device in-memory rate limits (§4.4) | `governor` (via `tower_governor` or a small layer of our own) | |
| Body limits, CORS, timeouts | `tower-http` | Already a dependency of the axum stack |
| Bounded userinfo cache, 10,000 entries (§6.2) | `moka`, or a bounded map of our own | Prefer our own if it stays under ~50 lines |
| Migrations | `sqlx` migrate (already in §8) | Separate job before rollout, `set_ignore_missing` (§13) |
| CIDR parsing for `trustedProxies` | `ipnet` | Also needed by scoped tokens in phase 7 |

Gap found while planning (first entry of the Divergences log): §6.2 persists the JWKS in
PostgreSQL, but §6.1 lists no table for it. Phase 1 adds `jwks_key` (`kid`,
`jwk`, `fetched_at`), not tenant-scoped, read through a `SECURITY DEFINER`
function like the other pre-tenant lookups (T4), purged when Authentik stops
publishing the key.

## 1. Phase 1 — scaffold, configuration, identity, authorization

Exit criteria (§16): tests T1, T2, T4, T12, T15 and the denial half of T10
green; `fuzz_config_document` runs in CI; `task lint`, `task audit` and the
80% coverage gate green; the OpenAPI drift gate active; §8 table completed
with real versions, licences and advisories.

### PR 1.1 — Repository scaffold

- Cargo workspace from `.example/Cargo.toml` plus the `weebo-good-practice`
  lint table (the kit's own table is the reference: `unwrap_used`,
  `expect_used`, `panic`, `todo`, indexing, arithmetic, `unsafe_code = deny`)
  and its `clippy.toml` exempting tests.
- Toolchain pinned in `mise.toml` only, as the kit does: `rust` at an exact
  version (not `latest`) with the `rustfmt`, `clippy`, `llvm-tools` and
  `rust-src` components; no `rust-toolchain.toml`. Fuzzing needs nightly:
  the fuzz task calls `cargo +nightly fuzz`, nightly pinned by date in the
  same file.
- Crates, all `publish = false` (decision 47): `kura-api`, `kura-server`
  (binary `kura-server`), `kura-core` (wire types only for now). `kura-cli`
  is created in phase 3, when it has something to do.
- `.tasks/rust.yaml`: `build`, `test`, `cov` (cargo-llvm-cov, 80% gate),
  `fmt`, `clippy -D warnings`; wired into the root `lint` task.
- `task audit`: `cargo deny check` (licences compatible with Apache-2.0,
  advisories, bans, sources limited to crates.io and the kit's git repo),
  `cargo audit`, Trivy on the image. `deny.toml` and a `NOTICE` file (§11).
- `Containerfile` and `Containerfile.hardened` per `weebo-containerfile`:
  cargo-chef dependency layer, distroless non-root runtime, read-only root
  filesystem, bases pinned by digest.
- mise: `cargo-deny`, `cargo-audit`, `cargo-llvm-cov`, `sqlx-cli`,
  `cargo-fuzz`, Spectral.
- CI: test, coverage and image build + scan jobs added to `ci.yml`, actions
  pinned by SHA.
- `cog.toml`, `devfile.yaml`, `readme.md`: still named `weebo-base`; run
  `task init-repo` first.

### PR 1.2 — `kura-api`: documents, defaults, `validate()`

- `KuraTenant` (cluster-scoped) and `KuraVault` (namespaced, `tenantRef`)
  specs, camelCase, `deny_unknown_fields`, defaults on the closed side (§4.1).
- Parsed types: Kubernetes quantities (`Ki`, `Mi`, `Gi`), durations (`h`,
  `d`), `TenantSlug`, role enums. Constructors are the only check (§6.11).
- `validate(documents, previous_state)` covering every row of §4.3. Three
  rows need the stored state, not the files alone: chunking changed since
  creation, a file vault colliding with an API-managed one, and the
  mass-removal rule. The function therefore takes the previous reconciled
  state as input, so the future operator webhook can call it too.
- `reason_codes!` catalogue: `Reconciled`, `InvalidDocument`,
  `QuotaBelowUsage`, `CidrTokensWithoutTrustedProxies`, `DeletionScheduled`,
  `AwaitingKeyDestruction`.
- Errors carry file, document index and field path
  (`tenants.yaml#2 spec.claimMappings.roles.ownr: unknown role`).
- Tests: one unit case per §4.3 row, defaults, quantity and duration parsing.
- Fuzz: `fuzz_config_document`.

### PR 1.3 — Server skeleton and API contract

- axum 0.8 app; `server.yaml` loading and validation (`oidc.issuer` HTTPS,
  `audiences` non-empty, algorithms never `none`); `trustedProxies`, `cors`.
- `/healthz`, `/readyz` (process state only, §10.2); metrics on a separate
  port; tracing with route templates only, never path parameters (T19).
- Shutdown through a cancellation token, spawned tasks in a `JoinSet`
  (§6.11).
- Problem type (RFC 9457) with every `type` of §4.4, mapped once at the
  boundary; one `thiserror` enum per crate.
- `X-Kura-Client` check returning `426 client_too_old`.
- `utoipa-axum`, Scalar page, `kura-server dump-spec`; `task recu` dumps the
  spec; drift gate, contract test and Spectral lint per `weebo-api-contract`.

### PR 1.4 — Database, roles and row-level security

- `sqlx` migrations for the phase 1 tables only: `config_state`, `tenant`,
  `vault`, `role_binding`, `device`, `jwks_key`. Hashes as `bytea`.
- Two roles: the migration owner, and the application role, not owner,
  without `BYPASSRLS`, with `lock_timeout`, `statement_timeout` and
  `idle_in_transaction_session_timeout` set (§6.11).
- `FORCE ROW LEVEL SECURITY` on every table; tenant and subject set with
  `set_config(…, true)` per transaction, matching nothing when unset; `device`
  isolated by `user_sub` (§6.1).
- Pre-tenant lookups (device, vault, JWKS) and the reconciler's writes as
  named `SECURITY DEFINER` functions, each listed and tested (T4 residual).
- Tests (integration, opt-in by env var with a loud skip):
  `rls_hides_other_tenant`, `unset_tenant_setting_returns_nothing`,
  `pooled_connection_does_not_leak_tenant`, `app_role_is_not_owner`, and the
  gate test `every_table_forces_rls`, which must fail on any future table
  that forgets it.

### PR 1.5 — Reconciler and `config_state`

- Watch `KURA_CONFIG_DIR` including the `..data` symlink swap, re-read every
  60 s; parse and validate the whole set; keep the last valid configuration
  on any error (§4.3).
- Write under `pg_advisory_xact_lock`, only if the binary is at least the
  stored writer version and the generation differs (§6.10).
- Materialise `tenant`, `vault`, `role_binding` rows with
  `managed_by = config`. Removal: `Orphan` disables; `Delete` disables and
  sets `delete_after` (the purge itself is phase 5); `force-orphan`
  annotation honoured.
- Startup: invalid files serve the last stored state; fresh install with no
  state serves probes only and stays unready (decision 31).
- Authorization cache refreshed when `config_state.generation` changes.
  `LISTEN/NOTIFY` arrives in phase 4, so phase 1 polls the generation
  (shortcut, to replace by the notification in phase 4).
- `GET /v1/admin/status` in the kit's status shape; metrics
  `kura_config_rejected_total`, `kura_config_warnings`.
- Tests: `invalid_document_keeps_last_config` (T12); integration: an older
  replica never overwrites newer `config_state`; `Orphan` and the scheduling
  half of `Delete`; `restored_document_cancels_deletion`. That last test is
  part of T18, which §16 puts in phase 5; it lands here because the
  scheduling it checks lands here, and phase 5 adds the purge half.

### PR 1.6 — Token verification

- `auth::verify_access_token`: signature, issuer, audience, expiry, pinned
  algorithms, `scope` required (decision 50). JWKS cache of at most 16 keys,
  refetch on unknown `kid` at most once a minute, persisted in `jwks_key`.
- Role claim from the access token; userinfo fallback with a cache bounded to
  10,000 entries for the token's lifetime (§6.2).
- Tests (T1): `rejects_token_for_other_audience`, `rejects_alg_none`,
  `rejects_id_token_as_access_token`, `rejects_expired_token`, with a test
  key pair. Decision 50 also asks for a real Authentik ID token: that half
  belongs to the heavy suite (§6.P, observation 15).

### PR 1.7 — Devices

- `POST /devices` (server-generated id, never reused, at most 20 live per
  user), `GET /devices` (own; tenant `admin` sees the tenant's),
  `DELETE /devices/{d}` (owner, tenant `admin`).
- `auth::require_device` on `X-Kura-Device`: `device_revoked`,
  `device_unknown`, other subject refused. Revocation refuses any later
  registration from the same `sid` (decision 35).
- Purge job for devices idle 180 days, keeping revoked ids and their `sid`.
- Tests (T2): `revoked_device_is_refused`, `revoked_sid_cannot_register`.

### PR 1.8 — Roles, bindings, vaults

- Role resolution: highest of tenant role (from `claimMappings`) and matching
  vault bindings (§4.2); `authz::require_role` checked on the vault object
  before any work; unreadable vault answers `404`, byte-identical to a missing
  one.
- `GET /vaults`; `POST`/`DELETE /vaults` for API-managed vaults (tenant
  `admin`); `PUT`/`DELETE /vaults/{v}/bindings/{b}` with
  `authz::check_binding` (§6.2). Files-managed objects are read-only through
  the API.
- Tests: `foreign_vault_is_404` (T4), `manager_cannot_bind_foreign_group`,
  `manager_cannot_grant_above_own_role` (T15).

### PR 1.9 — Limits and audit

- Per-device in-memory rate limits with the §4.4 defaults, `429` with
  `Retry-After`; body limits; CORS allowlist; client address from
  `trustedProxies` only.
- Audit events (log target `audit`) for every denial, device registration and
  revocation, binding change; metric `kura_authz_denied_total`.
- Test: `denial_writes_audit_event` (denial half of T10).

### PR 1.10 — Close the phase

- §8 versions, licences, advisories and maintenance from the `cargo deny`
  output, pinned in the Divergences log for §17 and §18.
- Operator doc for this surface: `server.yaml`, `KuraTenant`, `KuraVault`,
  the Authentik prerequisites of §13 (§11, documentation per phase).
- Coverage gate at 80% on each crate.

## 2. Phase 2 — chunks, blobs, encryption, quota

Detailed at the close of phase 1. Exit criteria (§16): T6 (`hash_mismatch_is_422`),
T8 (`bucket_object_is_not_plaintext`), T9 (`oversized_chunk_is_413`) green;
decision 52 measured on the real vault, and the zstd level 3 gain with it
(above 20% of stored bytes, open the codec RFC).

| PR | Content | Tests |
| --- | --- | --- |
| 2.1 | MinIO and Vault (dev mode) in `task dev:up` and as CI services; `vaultrs` 0.8 with Kubernetes auth, a token auth for local runs; database and bucket credentials read from Vault KV and re-read on rotation, retiring `KURA_DATABASE_URL` (D15) | integration, opt-in like PostgreSQL |
| 2.2 | `kura-core`: `ChunkHash` (BLAKE3), the chunk format (format byte, codec byte 0), FastCDC with the vault's parameters; a fuzz target for the chunk decoder | round trip, reassembly property |
| 2.3 | Data keys: Transit `datakey`, `vault.wrapped_dek` written once (`UPDATE … WHERE wrapped_dek IS NULL`, then read back), a secret wrapper that wipes on drop, cache of 1,000 keys for 10 min, encryption counter warning at 2³⁰ | two replicas creating one key keep one |
| 2.4 | `object_store` on S3; object key = keyed BLAKE3 of the chunk hash (`derive_key`) plus a random suffix; migrations for `chunk`, `blob`, `blob_chunk`; `POST chunks/missing`, `PUT`/`GET chunks/{hash}`, `PUT`/`GET blobs/{hash}`; hash recomputed on upload; 4 MiB body; immutable caching | T6, T8, T9; storage failing before the row insert leaves no row |
| 2.5 | Quota: `quota.storage` and `maxFileSize` enforced, `QuotaBelowUsage` condition, `422 quota_exceeded` | quota tests |
| 2.6 | Measurement on a copy of the real vault: deduplication and request counts per chunking setting (decision 52), zstd level 3 gain (decision 38); results in this file | — |

## 3. Later phases (outline, detailed when the previous one closes)

| Phase | Content (§16) | Tests and fuzz | Measurement / exit | New local services |
| --- | --- | --- | --- | --- |
| 2 | Chunks and blobs, envelope encryption, keyed per-upload object keys, write-once data key, Vault KV credentials, chunk format with codec byte, quota | T6, T8, T9 (`oversized_chunk_is_413`) | Decision 52 (chunking) and the compression gain; above 20%, plan the codec RFC | MinIO, Vault dev |
| 3 | Changes and commits under the vault lock, idempotency, conflict files; `kura-core`, `kura-cli`, device simulation | T3, T7, T14, T17, T19; `fuzz_vault_path`, `fuzz_commit_body` | Phases 1–3 give a working sync through `kura-cli`: first real use, one tenant, one vault | — |
| 4 | Server-side three-way merge; WebSocket tickets and `LISTEN/NOTIFY` (replaces the phase 1 generation poll); plugin on desktop, then Android | T11; `fuzz_merge` | Decisions 53, 54, 55 on the Android test phone | Obsidian 1.11.4 desktop + Android |
| 5 | History, restore, retention, GC, horizon, epochs, full resync and its deletion guard, `Delete` purge after 30 days | T18 | — | — |
| 6 | Helm chart, telemetry, alerts, backup and restore drill, k6 load and soak | — | Decision 42: first attach above 5 min opens the batch-download RFC | CloudNativePG on the cluster |
| 7 | Opt-in search; scoped tokens with expiry, CIDR, revocation | T5, T16, token half of T10; `fuzz_scoped_token` | — | — |
| 8 | Obsidian configuration sync per user and profile, plugin pins | T14 (configuration half) | — | — |
| 9 | Notion export, first push, one month of parallel use, cutover | — | File count and bytes equal the export (§13) | — |

The heavy suites of §6.P (`tests/heavy/sync.sh`, `tests/heavy/authz.sh`) grow
with the phase that ships each observation; all must pass before the RFC
moves to Implemented. The operator and TUI RFCs open after phases 3 and 7
(see the [RFC index](../readme.md)).

## Divergences

Every point where the build differs from or completes the RFC, with its
reason and the PR that introduced it. Folded into the RFC's §18 at landing.

| # | PR | RFC | Divergence | Reason |
| --- | --- | --- | --- | --- |
| D1 | 1.4 | §6.1, §6.2 | Table `jwks_key` added | §6.2 persists the JWKS in PostgreSQL; §6.1 lists no table for it |
| D2 | 1.1 | — | `task init-repo` applied by hand without its `gh api -X PUT` call | That call changes a GitHub repository setting; the maintainer runs it, or ticks "Allow GitHub Actions to create and approve pull requests" in Settings > Actions |
| D3 | 1.1 | — | Nightly for `cargo fuzz` pinned by date as `KURA_FUZZ_TOOLCHAIN` in `mise.toml` `[env]` (`nightly-2026-11-01`), installed with rustup | mise installs one Rust toolchain per tool entry; the `fuzz` task calls `cargo +$KURA_FUZZ_TOOLCHAIN` |
| D4 | 1.1 | §8 | Runtime base `distroless/cc-debian13:nonroot`, builder `rust:1.100-slim-trixie` (template: Debian 12) | Current Debian; same glibc in builder and runtime. No writable directory: the server writes nothing to disk |
| D5 | 1.1 | — | `task audit` adds `audit:fs` (Trivy on the repository, the template's former `audit`) beside `audit:rust`, `deny` and `audit:image` | Keeps the Containerfile misconfiguration scan the template already ran |
| D6 | 1.2 | §8 | YAML through `serde_norway` 0.9 (the kit's own choice, neither plan candidate); field paths through `serde_path_to_error` 0.1 | One YAML stack with the kit; the field path of §4.3's error format comes from the deserializer, not from a hand-written walker. Both pass `cargo deny` |
| D7 | 1.2 | §4.3 | The `oidc.issuer` HTTPS / `audiences` row is checked by the `server.yaml` loader of `kura-server` (PR 1.3), not by `kura_api::validate` | `server.yaml` is not a resource document and the operator will never see it |
| D8 | 1.2 | §4.1, §4.3 | Rules the RFC implies, made explicit: a vault's `tenantRef` must name a declared tenant; a binding sets exactly one non-empty `claimValue` or `subject`; claim values are unique per (`claim`, value) across tenants and never empty; names are RFC 1123 labels; `KuraVault` also takes `deletionPolicy` (§4.3 warnings remove vaults with `Orphan` or `Delete`); document index in errors is 1-based | Each closes a case where a typo would otherwise drop a permission silently |
| D9 | 1.2 | §4.1 | `retention` fields are optional and an absent one prunes nothing | The RFC gives no default; phase 5, which applies retention, settles them |
| D10 | 1.3 | §4.4 | `/scalar` serves the template's degraded page (spec embedded, no bundle, same CSP) | No UI build ships the Scalar bundle yet; a CDN copy is refused by the house rule. To revisit with the web UI, or by vendoring the bundle |
| D11 | 1.3 | §4.4, §13 | The spec is committed at `docs/api/openapi.json`; the drift gate is `task openapi:check` (dump, then `git diff --exit-code`), run in CI's `check` job | No `ui/` here (decision 37). The diff against the last release (§13) needs a release; it lands with the first one |
| D12 | 1.3 | §4.1 | `server.yaml` gains `listen.api` (default `0.0.0.0:8080`) and `listen.metrics` (`0.0.0.0:9090`); `oidc.algorithms` defaults to `[RS256]` and must not be empty | The RFC's example names no port; RS256 is Authentik's default and stays pinned, never read from a token |
| D13 | 1.3 | §13 | `X-Kura-Client` minimums live in a table in the code (`kura-cli` and `kura-plugin`, both `0.1.0`); a request without the header or from an unknown client name is not gated; a known client whose version does not parse gets `426` | Scoped tokens (agents, `curl`) send no client header; the minimum moves with a release, not with configuration |
| D14 | 1.4 | §8 | Queries are checked at runtime (`sqlx::query`), not by sqlx's compile-time macros | The macros need a live database or committed offline data refreshed on every commit by `task recu`; every query runs instead in the integration tests against PostgreSQL, which CI never skips |
| D15 | 1.4 | §4.1, decision 48 | Phase 1 reads the database URL from `KURA_DATABASE_URL` | Credentials from Vault KV arrive with phase 2, which brings `vaultrs` |
| D16 | 1.4 | §6.1, §6.11, decision 26 | `kura_app` and its timeouts (`lock_timeout` 5 s, `statement_timeout` 5 s, `idle_in_transaction_session_timeout` 10 s) come from `sql/bootstrap-roles.sql`, run once by the DBA (CloudNativePG `postInitApplicationSQL`), not from a migration. Each table carries an `owner_all` policy for the migration owner, so the `SECURITY DEFINER` functions still work under `FORCE`. `device` is isolated by the subject policy, with no definer function: the subject is known from the token before any tenant | A migration owner may not hold `CREATEROLE`; `FORCE` also binds the owner |
| D17 | 1.4 | §6.1 | Only the columns phase 1 uses: `vault` has no `epoch`, `wrapped_dek`, `head_seq`, `horizon_seq` yet; they arrive with the phase that writes them, as additive migrations | Expand and contract (§13): a column lands with its first reader |

| D18 | 1.5 | §6.10 | The directory is polled every 10 s (SHA-256 of names and contents) instead of watched; a reconciliation also runs every 60 s. Recorded as a `shortcut:` in `reconcile::run` | `notify` is at a release candidate (9.0.0-rc.8); polling sees the `..data` swap with no special case. Upgrade if 10 s matters |
| D19 | 1.5 | §6.10 | Write rule: a replica writes when nothing is stored, when the stored writer is older, or, for an equal version, only when its own files changed since its previous tick (startup counts as a change). Never when the stored writer is newer or the generation is equal. The rule runs inside `kura_config_begin`, under `pg_advisory_xact_lock`, versions compared as integer arrays | "Write if generations differ" lets a replica whose ConfigMap mount lags revert a fresher one every minute |
| D20 | 1.5 | §6.1, §13 | `config_state.documents` stores the files as written (`{"files": {name: content}}`), and the cache re-parses them; a replica that cannot parse a newer writer's documents keeps its previous cache | Re-serialised structs would make an N-1 replica fail on fields it does not know, and change `observedGeneration` on every upgrade |
| D21 | 1.5 | §4.1 | `kura.weebo.io/force-orphan` is remembered on the row (`force_orphan`, migration `0002`) and, on removal, cancels the `Delete` cleanup: the object ends as with `Orphan`, disabled with no deletion date. The alternative reading, leaving it enabled, was rejected as fail-open | The annotation leaves the files with the document, so the row must remember it |
| D22 | 1.5 | §4.3 | Reason code `Orphaned` added to the catalogue, for an object removed with `Orphan` (`Ready: False`) | §4.3 lists the warning but no code for it |

| D23 | 1.6 | §8 | `jsonwebtoken` 12 with its `aws_lc_rs` backend; `reqwest` 0.13 with rustls on aws-lc-rs. `sqlx` keeps ring, so two crypto providers are linked | The `rust_crypto` backend pulls the `rsa` crate (RUSTSEC-2023-0071, Marvin); aws-lc-rs is what reqwest already uses |
| D24 | 1.6 | §6.2, §14 | The T1 tests sign with Ed25519 keys generated at runtime (`aws-lc-rs`), so no private key is committed; RS256 against a real Authentik belongs to the heavy suite (§6.P, observation 15) | Keeps gitleaks quiet without an allowlist |
| D25 | 1.6 | §6.2 | Keys and userinfo come from OIDC discovery (`<issuer>/.well-known/openid-configuration`, redirects refused, 5 s timeout); 30 s leeway on `exp`; a bearer over 16 KiB is refused before parsing; `GET /v1/admin/status` answers `403 insufficient_role` to an authenticated non-admin | Bounds on every outbound call and input (§6.11) |

| D26 | 1.7 | §4.4 | Problem type `invalid_request` (400) added, for a body or parameter that does not parse | §4.4 lists no type for a malformed request; axum's plain-text rejection would break the problem contract |
| D27 | 1.7 | §4.4, §6.1 | A tenant `admin` sees and revokes the devices linked to the tenant in `device_tenant` (migration `0003`), written at registration for every tenant the owner then belongs to; `device` itself stays isolated by subject. Links follow the owner's memberships, refreshed at most once a minute while the device is used (review R2) | `device` has no tenant (§6.1) but §4.4 gives the tenant admin a view of "the tenant's" devices |
| D28 | 1.7 | §4.4, §6.2 | Registration needs a token with `sid` and a membership of at least one enabled tenant (claim mapping, or a subject binding in a file-declared vault), else `403 insufficient_role`; the 21st live device gets `422 quota_exceeded`; registrations of one user are serialized by an advisory lock. `GET /v1/admin/status` also requires `X-Kura-Device` (OIDC = token plus device, §4.4). Closing a revoked device's sockets lands with the sockets (phase 4) | |

| D29 | 1.8 | §6.2 | Tenant `owner` and `admin` open every vault of the tenant as `manager`; `member` only through bindings. A subject is a "known tenant member" for `check_binding` when one of its devices is linked to the tenant (D27) | §6.2 says what each role grants but not as a vault role; "known member" needs a stored trace |
| D30 | 1.8 | §4.4 | `{b}` in `/vaults/{v}/bindings/{b}` is `claim:<value>` or `user:<sub>`; `PUT` takes `{"role": …}`. An API-created vault lives in the namespace named after its tenant's slug, with the built-in chunking; a taken name gets `400 invalid_request`; `DELETE` disables it and schedules the purge 30 days out, like a file `Delete` (purge in phase 5). Changing a file-declared object answers `403 insufficient_role` with the reason | The RFC names the paths, not the binding key or the namespace of API vaults |
| D31 | 1.8 | §4.4 | `GET /v1/vaults` walks every enabled tenant (a binding may open a vault without a tenant role), at most 1,000 vaults per tenant | Tenants are few on this platform; keyset paging lands with the sync API |

| D32 | 1.9 | §4.4 | Rate limits are a table in the code (`limits::LIMITS`, the §4.4 defaults), not yet a `server.yaml` section; counted per device after `require_device`, per subject for `POST /devices`; past 100,000 tracked callers a new caller is not counted (a `shortcut:` in the code) | Configurable limits wait for a deployment that needs other values; counting before authentication would let anyone spend a victim's budget with its device id |
| D33 | 1.9 | §10.1, T10 | The denial audit event carries the problem type, method, route template, client address and the *claimed* `X-Kura-Device`, not the subject: it is written by one middleware from the response, which never sees the verified token. `test denial_writes_audit_event` lives in `tests/logs.rs`, alone in its binary, because a per-test tracing subscriber races with parallel tests | One place for every denial, so none is forgotten; the subject is logged by the handlers' own audit events (registration, revocation, bindings) |

| D34 | review | §6.2, decision 35, T2 | **Confirmed by the owner (2026-11-14).** A token whose Authentik session (`sid`) has a revoked device is refused (`401 device_revoked`) on *every* device id, not only on the revoked one; a token with no `sid` is refused on every device call. Decision 35 accepted a stolen session keeping its access token for at most an hour on a non-revoked device id; without this rule the thief could use another of the user's device ids indefinitely, refreshing in Authentik (review R1). Open §6.P fact: if Authentik gives the same `sid` to devices approved from one browser session, revoking one cuts them all | Closes an unbounded bypass of revocation; the cost is the shared-session case, to observe on the instance |
| D35 | review | §6.10 | A reconciliation tick that fails (database error, lock timeout) leaves the change pending, so the retry still counts as this replica's own change. Found while closing phase 1; test `a_failed_tick_is_retried_as_a_local_change` | Otherwise an equal-version replica would refuse its own retry and a removed binding would stay in force until a restart |

### Phase 1 security review (2026-11-14)

An independent review of the auth, authz, RLS and reconciler code, after
PR 1.10. Fixed findings are in the tree; open ones are carried to the phase
that touches the code.

| # | Finding | Severity | Outcome |
| --- | --- | --- | --- |
| R1 | `require_device` did not check the token's session: a session whose device was revoked could keep working through another device id of the same user, without time limit | High | **Fixed**: a token whose `sid` has a revoked device is refused (`device_revoked`) on every device. Test `revoked_session_cannot_use_another_device`. A session is one device install; two devices sharing a session fall together |
| R2 | `device_tenant` links were written at registration only: a user who left a tenant stayed visible to (and revocable by) its admin, a later membership was not linked, and a revoked device still made a "known member" for `check_binding` | Medium | **Fixed**: links follow current memberships, refreshed at most once a minute with `last_seen` (`kura_device_link` now also removes stale links); `check_binding` ignores revoked devices. Test `device_links_follow_current_memberships` |
| R3 | A replica restarting while its ConfigMap mount still lags counts startup as a local change and may rewrite the older generation | Low | **Open**, accepted for now: kubelet mounts the current ConfigMap at pod start, so the window is the sync lag of a restart racing an update. Revisit with the operator (the documents become custom resources) or by comparing a resourceVersion |
| R4 | Retired signing keys stayed accepted until an unknown `kid` triggered a refetch; `kura_jwks_replace` used `NOT IN`, which purges nothing when one published key has no `kid` | Low | **Fixed**: the cached set is refreshed on use once older than 10 minutes (failed refreshes keep the old keys); `NOT EXISTS` in the purge. Test `a_retired_key_stops_verifying_after_max_age` |
| R5 | The key cache's mutex is held during the provider fetch (up to about 10 s) | Low | Open: at most one fetch a minute; revisit if Authentik latency shows in `p99` |
| R6 | `GET /v1/vaults` opens one transaction per vault | Low | Open: one query per tenant lands with the sync API's keyset paging (phase 3) |
| R7 | `kura_tenant_devices()` stops at 1,000 rows, which `check_binding` relied on | Low | **Fixed** by the eco review: `kura_tenant_has_member`, an indexed `EXISTS` |

### Phase 1 eco-design review (2026-11-14)

The `eco-check` agent (RGESN, GR491) on the phase 1 code. Fixed in the tree:
the daily device purge was never started (a real retention gap, now in
`background()`); a reconciliation tick now asks `kura_config_begin` before
reading the stored state and parsing the files, so an unchanged generation
costs one query; the cache poll reads `max(id)` and fetches the documents only
when it moved; `GET /v1/devices` deduplicates with a set and names its
columns; `check_binding` uses an indexed `kura_tenant_has_member` instead of
reading up to 1,000 devices (also closes R7). Kept: `last_seen` written at
most once a minute per device (a coarser interval would delay membership
changes reaching the device links; product decision for the owner).

### §8 as built (phase 1)

Direct dependencies at the close of phase 1, pinned in `Cargo.lock`, for the
RFC's §17 and §18. `cargo deny check` (advisories including unmaintained and
yanked, licences, bans, sources) and `cargo audit` both pass on 2026-11-14
with no ignore; Trivy finds nothing in `Cargo.lock`, `fuzz/Cargo.lock` or the
image (Debian 13.7 base).

| Crate | Version | Licence | Note |
| --- | --- | --- | --- |
| `axum` | 0.8.9 | MIT | §8 |
| `tokio` / `tokio-util` | 1.54.1 / 0.7.20 | MIT | §8 |
| `tower-http` | 0.7.1 | MIT | trace, CORS (plan P4) |
| `sqlx` | 0.9.0 | MIT OR Apache-2.0 | `sqlx-sqlite` stubbed out (E1) |
| `jsonwebtoken` | 12.0.0 | MIT | aws-lc-rs backend (D23) |
| `reqwest` | 0.13.5 | MIT OR Apache-2.0 | rustls (aws-lc-rs) |
| `utoipa` / `utoipa-axum` / `utoipa-scalar` | 6.0.0 / 0.3.0 / 0.4.0 | MIT OR Apache-2.0 | §8 |
| `weebo-kit-api` / `weebo-kit-domain` | 0.31.0 (tag `v0.31.0`, `0d145a8`) | MIT | §8 |
| `k8s-openapi` | 0.28.0 | Apache-2.0 | through the kit, `v1_36` (P2) |
| `serde` / `serde_json` | 1.0.229 / 1.0.154 | MIT OR Apache-2.0 | §8 |
| `serde_norway` | 0.9.42 | MIT OR Apache-2.0 | D6 |
| `serde_path_to_error` | 0.1.20 | MIT OR Apache-2.0 | D6 |
| `thiserror` | 2.0.21 | MIT OR Apache-2.0 | §8 |
| `anyhow` | 1.0.104 | MIT OR Apache-2.0 | binary only |
| `tracing` / `tracing-subscriber` | 0.1.44 / 0.3.23 | MIT | §8 |
| `prometheus` | 0.14.0 | Apache-2.0 | the kit's choice; OpenTelemetry arrives with phase 6 |
| `uuid` | 1.29.2 | Apache-2.0 OR MIT | §8 |
| `time` | 0.3.55 | MIT OR Apache-2.0 | timestamps from PostgreSQL |
| `sha2` | 0.11.1 | MIT OR Apache-2.0 | configuration generation, userinfo cache key |
| `ipnet` | 2.12.2 | MIT OR Apache-2.0 | `trustedProxies` (P4) |
| `aws-lc-rs`, `base64`, `tower` | 1.19.0, 0.22.1, 0.5.3 | ISC/Apache-2.0, MIT OR Apache-2.0, MIT | tests only (runtime keys, D24) |

Not needed in phase 1 after all (plan P4 candidates): `notify` (polling, D18),
`governor` (own limiter, D32), `moka` (own bounded map). Duplicates accepted in
`deny.toml`, each with its holder: the RustCrypto 0.10 line (sqlx-core),
`getrandom` 0.2 and `windows-sys` 0.52 (ring), `hashbrown` 0.16 (hashlink),
`syn` 2, `base64` 0.22, `tower-http` 0.6 (reqwest), `untrusted` 0.7 (ring).

| D36 | 2.1 | §4.1, decision 48 | KV secret shapes: database `{"url": "postgres://…"}`; bucket `{"endpoint", "bucket", "region" (default `us-east-1`), "accessKeyId", "secretAccessKey"}`. `KURA_DATABASE_URL` still overrides Vault for local runs; `kura-server migrate` always reads it from the environment, because the migration job runs as the owner, whose credentials are not the server's. A rotated URL reaches new connections at once and replaces old ones as the pool recycles them | The RFC names the KV paths, not their keys |

| D37 | 2.2 | §4.3, §6.4 | New validation rule: `chunking` must also fit FastCDC 2020 (min 64 B to 1 MiB, avg 256 B to 4 MiB, max at least 1 KiB, all even); a quantity may be a bare integer of bytes, as in Kubernetes. The chunk decoder fuzz target of the plan becomes `fuzz_chunking` here; the stored-chunk decoder gets its own in PR 2.3 | Outside those bounds the chunker's behaviour is unspecified (debug asserts only), and the plugin would meet it in WASM |

| D38 | 2.3 | §8, §6.6 | AES-256-GCM, randomness and the object-key derivation use `aws-lc-rs` (already linked by `jsonwebtoken` and rustls) and `blake3`, not the `aes-gcm` crate; the tenant's Transit key is named `kura-<tenant slug>` and the Transit mount is `vault.transit.mount` (default `transit`) in `server.yaml`. The stored-chunk decoder has unit tests for every malformed shape but no fuzz target yet: it lives in `kura-server`, whose database and HTTP stack the fuzz build must not carry; it gets one if the decoder moves to a crate of its own | One AEAD implementation in the binary instead of two; FIPS-validated code |

| D39 | 2.4 | §6.4, §4.4 | A blob's address is `kura_core::blob_hash`: BLAKE3 over a domain tag, the file size and the ordered chunk hashes, not the hash of the file bytes. The server checks a manifest against it, checks every chunk is stored and the sizes add up, without reading the file; each chunk was checked against its own hash on upload. `PUT chunks` and `PUT blobs` answer `204` whether new or already present | Verifying a whole-file hash would mean decrypting up to `maxFileSize` per manifest |
| D40 | 2.4 | §8, §13 | The local and CI S3 store is RustFS (Apache-2.0); MinIO publishes no image any more. The bucket must exist: `task dev:s3` creates `kura-test`. Rate limits of 600/min on chunk and blob routes (§4.4 gives a body limit, not a rate, for uploads). The quota check sums the tenant's chunks on each new upload (a `shortcut:` in the code) | |

### Phase 2 security review (2026-11-14)

No AEAD, cross-vault or hash-integrity defect found. Findings:

| # | Finding | Severity | Outcome |
| --- | --- | --- | --- |
| P1 | `PUT chunks` held a database connection across the Vault and S3 calls; 16 first uploads could starve the 16-connection pool and turn every route into `503` | High | **Fixed**: a short transaction for "present?" and the quota, then key, seal and upload with no transaction, then a new transaction to record the row |
| P2 | Bucket credentials were read once; a rotation in Vault KV broke chunk routes until a restart | Medium | **Fixed**: re-read every 5 min with the database secret; a change swaps the bucket client |
| P3 | Concurrent uploads could all pass the quota sum and overshoot it | Medium | **Fixed**: the record step takes a per-tenant advisory lock and re-checks the quota, deleting the uploaded object if refused. Test `concurrent_uploads_respect_the_quota` |
| P4 | Each 4 MiB chunk was copied about three times while sealing or opening | Low | **Fixed**: sealed and opened in place in one buffer |
| P5 | An object can stay in the bucket without a row if the record step dies after the upload | Low | **Open, for phase 5**: the garbage collector must list each vault's prefix and delete objects with no row, not only sweep rows |
| P6 | `chunk`/`blob` rows were not tied to their vault's tenant by the database | Low | **Fixed**: composite foreign keys `(vault_id, tenant_id) → vault (id, tenant_id)`. Test `a_chunk_cannot_claim_another_tenant` |

Also noted: the Vault login is renewed every 5 minutes, so the Kubernetes auth
role must issue tokens living longer than that (operator guide §5).

### §8 additions (phase 2)

| Crate | Version | Licence | Note |
| --- | --- | --- | --- |
| `vaultrs` | 0.8.0 | MIT | §8; brings `rustify` 0.7 with `thiserror` 1 and `syn` 1 (skips in `deny.toml`) |
| `object_store` | 0.14.2 | Apache-2.0 | §8, `aws` feature only |
| `blake3` | 1.8.7 | CC0-1.0 OR Apache-2.0 | §8 |
| `fastcdc` | 5.0.0 | MIT | §8, v2020 |
| `aws-lc-rs` | 1.19.0 | ISC AND (Apache-2.0 OR ISC) | AES-256-GCM instead of `aes-gcm` (D38) |
| `zeroize` | 1.9.1 | Apache-2.0 OR MIT | §8 |
| `bytes`, `base64` | 1, 0.22.1 | MIT, MIT OR Apache-2.0 | already in the tree |

`cargo deny` and `cargo audit` pass with them on 2026-11-14.

### Security exceptions

| # | Date | Advisory | Scope | Why it is safe | Expires |
| --- | --- | --- | --- | --- | --- |
| E1 | 2026-11-14 | RUSTSEC-2026-0459 (`libsqlite3-sys` 0.37, C SQLite) | **Retired the same day**, no ignore left | It was a lockfile-only entry of `sqlx` 0.9.0 (optional `sqlx-sqlite`). `[patch.crates-io]` now points `sqlx-sqlite` at an empty stub (`patches/sqlx-sqlite`, rung 3 of the triage ladder), so neither `Cargo.lock` nor any scanner sees SQLite; `deny.toml` bans `libsqlite3-sys` | Remove the stub when sqlx stops listing the driver |
