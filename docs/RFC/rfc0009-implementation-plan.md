# RFC 0009 — implementation plan

The phases of [RFC 0009](rfc0009-kura-operator.md) §16, kept up to date as
they land. Commits are made by the maintainer (signed), so "done" means green
in the working tree. Each phase leaves the tree green, and no phase exposes the
write route before its identity check and limits (RFC §16).

## Progress

| Item | Status | Date | Notes |
| --- | --- | --- | --- |
| Accepted | done | 2026-11-21 | Questions 4, 8 and 9 resolved as recommended: projected ServiceAccount token, no break-glass push, webhook `failurePolicy: Fail` |
| Phase 0 | done | 2026-11-22 | Kit `weebo-si-operator/weebo-operator-kit` v0.32.0 (`8284bb9f`), workspace builds unchanged. `kura-api`: `KuraInstance` (`instance.rs`, cluster-scoped, `url`, `tls`, required `auth.serviceAccountToken.audience`), `Status.instance` (the kit's `BoundInstance`, optional), reason codes `SetRejected`, `SourceIsFiles`, `VaultNamespaceNotAllowed`, `TenantNotFound`, `VaultManagedByApi`, `InstanceUnreachable`, `InstanceUnauthorized` (blocking). `kura-crdgen` writes `kurainstance.yaml`, the instance stands for both schematic roles; docs reference regenerated with its example. Template generated with `cargo-generate` 0.26.0 into the scratchpad; its crates merge in phase 2 (D1) |
| Phase 1 | done | 2026-11-22 | `server.yaml` `documents` (default `files`; `operator` needs `issuer` https, `audience`, `subject` exactly `system:serviceaccount:<ns>:<name>`, plus `caFile`, `tokenFile`) (D6); `auth/operator.rs` `OperatorVerifier` (`RS256` only, `iss`/`aud` exact, `exp` 30 s leeway, `sub` byte-equal, its own unpersisted `Keys` from `KeySource::cluster`), `OperatorCaller`; `app/documents.rs` `put_documents` (409 `documents_source_files` before the token, 401/403, 30/min per subject, 4 MiB, 2,000 documents, `kura_api::pushed_name`, `ConfigFiles::new`, `reconcile()`), answering `DocumentsResult` (`applied`, `outcome`, structured `errors`, the `AdminStatus` reloaded after a write); audit lines `documents_pushed` / `documents_rejected`; `kura_documents_pushes_total{outcome}` (D4); `reconcile::run` only in files mode; ready at start in operator mode (D7); OpenAPI regenerated, Spectral clean. Tests: `tests/documents.rs` (`a_push_applies_like_files`, T1 `documents_refuses_other_identities`, `files_mode_refuses_every_push`, T5 `documents_body_and_count_limits`, T6 `older_writer_refused`, T10 `empty_set_refused`), T7 `tests/documents_audit.rs`, unit T1 and T9 in `auth/operator.rs` (cluster issuer with a bearer, no userinfo, redirect refused), config; `fuzz_documents_body` 2,310,464 runs in 61 s, no finding. Workspace tests green against PostgreSQL. Docs: problem types, API, rate limits, metrics, reason codes, `server.yaml` |
| Phase 2 | todo | | `kura-operator`: controllers, push queue, gateway, status, webhook |
| Phase 3 | todo | | Charts, image, release jobs, §6.P proof on kind, capacity probe |
| Phase 4 | todo | | Docs, perf budget, status `Implemented` with §18 |

## Phase 0 — the kit and the generated operator

Green: `task recu`, `task lint`, `task test` as today; the generated operator
builds and its template tests pass; nothing is deployed.

1. **Kit pin.** `Cargo.toml`: `weebo-kit-api` and `weebo-kit-domain` move from
   `batleforc/weebo-operator-kit` `v0.31.0` to
   `weebo-si-operator/weebo-operator-kit` at the latest tag, v0.32.0 (`8284bb9f`,
   published 2026-11-14; `gh api repos/weebo-si-operator/weebo-operator-kit/tags`
   read 2026-11-21). Update the lint-table comment on
   line 2. `task audit` green before going on.
2. **Generate.** `cargo generate --git
   https://github.com/weebo-si-operator/weebo-operator-kit --tag <pin>
   templates/operator` with `product = Kura`, `license = Apache-2.0`, into the
   scratchpad, never into the tree.
3. **Merge**, per RFC §6.3's table:
   - `api` → into `kura-api` behind `crd`: `KuraInstanceSpec { url, tls:
     Option<TlsOptions>, auth: InstanceAuth }` with CEL (`auth` required); the
     template's `LocalInstance` and example `Object` are not copied.
   - `crdgen` → `kura-crdgen` writes `deploy/crd/kurainstance.yaml` and the
     instance in `links.json`; the stale-CRD test covers it.
   - `domain`, `application`, `adapters-outbound`, `adapters-inbound`,
     `operator`, `testkit` → `crates/operator/<layer>`, crates named
     `kura-operator-*` (`kura-operator` for the binary), added to the
     workspace members. Placeholders left as generated; phase 2 fills them.
   - `Containerfile.operator`, the chart (to `deploy/charts/kura-operator`),
     the workflows calling the kit's `checks.yml` and `publish.yml` are copied
     but **not wired** into `ci.yml` / `release.yml` yet (phase 3).
   - `.tasks/kit.yaml` included as `kit:` in `Taskfile.yml` with
     `flatten: false`; `kit:cog` is not run (RFC §6.3: `cog.toml` holds Kura's
     hooks).
4. **Reason codes** in `crates/kura-api/src/reason.rs`: `SetRejected`,
   `SourceIsFiles`, `VaultNamespaceNotAllowed`, `TenantNotFound`,
   `VaultManagedByApi`, each with its severity and a unit test as
   `InvalidDocument` has; the kit's `InstanceUnreachable`,
   `InstanceUnauthorized`, `Reconciled` reused.
5. **Status**: `kura-api`'s `Status` gains the kit's `instance:
   Option<BoundInstance>` (optional, so the stored statuses of today still
   parse).
6. **Docs**: `task docs:gen` regenerates the CRD reference with
   `KuraInstance`.

Watch: the kit bump may move `weebo-kit-domain`'s `reason_codes!` or the lint
table; any change in behaviour goes in *Divergences*.

## Phase 1 — the server's route

Green: every existing suite unchanged (`files` is the default); the route can
be driven by a test with a forged issuer.

1. **Config** (`crates/kura-server/src/config.rs`, the `Settings` of
   `server.yaml`): `documents: { source: files | operator, operator: {
   issuer, audience, subject } }`, `#[serde(deny_unknown_fields)]`, default
   `files`; startup refuses `operator` unless all three are set and non-empty.
   Test in `tests/config.rs`.
2. **Operator verifier** (`crates/kura-server/src/auth/`, new
   `operator.rs`): JWKS through `Keys` with `KeySource::http(issuer)`
   (`auth/keys.rs:62`, already no redirects and one refetch a minute), a cache
   of its own, `RS256` only, `iss` and `aud` exact, `exp` with 30 s leeway,
   `sub` byte-equal to the configured subject. Principal `OperatorCaller`, an
   extractor no user route takes. Unit tests on each check (`TestKey`).
3. **Route** (`crates/kura-server/src/app/documents.rs`,
   `put_documents`), in this order: source (`409 documents_source_files` in
   `files` mode, before reading the token), identity (`401`/`403`), rate
   (`limits.rs`: `("PUT", "/v1/admin/documents", 30)`, keyed by subject),
   body (4 MiB, 2,000 documents, before parsing), then each document through
   `kura_api::parse`, turned into a `ConfigFiles` (`reconcile/files.rs:14`)
   named `KuraTenant/<name>` / `KuraVault/<ns>/<name>`, sorted, generation the
   same SHA-256, then `reconcile()`. Answers `200` `AdminStatus` plus
   `applied: bool`. Problem type `documents_source_files` added to the
   catalogue.
4. **Wiring**: `reconcile::run` started only in `files` mode; `GET
   /v1/admin/status` also admits `OperatorCaller`, read only.
5. **Generation per object**: in operator mode `status::build` reports the
   pushed `metadata.generation`, not the hash of `observed_generation`
   (`reconcile/status.rs:49`). The status is stored as `jsonb` in
   `config_state`, so this should need no migration, against RFC §13 ("one
   migration adds `generation`"); confirm, and record it as a divergence if
   so.
6. **Audit and metric**: one `target: "audit"` line per call (subject,
   applied, generation, removed objects); `kura_documents_pushes_total{result}`
   with `applied | unchanged | rejected | refused` in `metrics.rs`.
7. **API contract**: utoipa annotations, `task api` regenerates
   `docs/api/openapi.json`, `tests/openapi_contract.rs` green. No console
   change, so no HeyApi regeneration.
8. **Tests** (`crates/kura-server/tests/documents.rs`, new): a push applies
   like files; an unchanged push is `unchanged`; an invalid set leaves the
   stored generation; removal follows `deletionPolicy`. Security rows, named as
   in RFC §14: T1 `documents_refuses_other_identities` (user token, other SA,
   wrong audience, expired, `alg: HS256`), T5
   `documents_body_and_count_limits` and the limits gate test, T6
   `older_writer_refused`, T7 the audit line, T9 `operator_jwks_no_redirect`,
   T10 `empty_set_refused`. Fuzz target `fuzz/fuzz_targets/fuzz_documents_body.rs`
   on the route's parser, 60 s without finding.

## Phase 2 — the operator

Green: `cargo test -p kura-operator-*` with the kit's envtest harness; the
server suites untouched.

1. **Set building** (`kura-operator-domain`): from the two caches, each
   resource to `{apiVersion, kind, metadata: {name, namespace, labels,
   annotations, generation}, spec}`, dropping `status`, `uid`,
   `resourceVersion`, `managedFields` and the last-applied annotation; sorted.
   Unit tests, including a resource with every dropped field set.
   `metadata.generation` must be accepted by `kura_api::Metadata`
   (`deny_unknown_fields`, `spec.rs`): add it there if it is not, as an
   optional field the files never carry.
2. **Push queue** (`kura-operator-application`): one `tokio::sync::Notify`,
   events coalesced over 1 s, drained by the leader only, never before both
   caches have synced. Unit test of the coalescing; T10
   `operator_waits_for_sync`.
3. **Gateway** (`kura-operator-adapters-outbound`, in place of
   `http_gateway.rs`): `reqwest` with rustls, 15 s timeout, bearer re-read from
   the projected token file on every push; `408`/`429`/`5xx` transient,
   `401`/`403` → `InstanceUnauthorized`, connection errors →
   `InstanceUnreachable`, `409` → `SourceIsFiles`.
4. **Controllers** (`kura-operator-adapters-inbound`): `KuraTenant` and
   `KuraVault` on the kit's skeleton, each enqueuing a push; `KuraInstance`
   resolved through the kit's `instances.rs` (the only cluster instance,
   pinned in `status.instance`).
5. **Status and Events**: from the `AdminStatus` answer, one merge patch per
   resource whose conditions changed (`Reconciled`, `InvalidDocument` with the
   server's message on the offender, `SetRejected` naming it on the others,
   `VaultManagedByApi`, the advisory conditions copied), `observedGeneration`;
   `Warning` Events on `Ready=False`.
6. **Webhook** (the kit's `/validate`, fail closed): `KuraVault` whose
   namespace is not in its tenant's `vaultNamespaces`
   (`VaultNamespaceNotAllowed`) or whose tenant is missing (`TenantNotFound`),
   from the cache, calling the cross-object part of `kura_api::validate()`;
   a second `KuraInstance` refused.
7. **Tests** (envtest, fake gateway from the testkit): resources → statuses
   and Events; leader-only pushes; T2 `webhook_refuses_unlisted_namespace`;
   T4 `server_rejects_what_the_webhook_would` (webhook disabled, the real
   server route in a test container or the server's test app).

## Phase 3 — charts, image, release, proof

Green: `task helm:lint`, `task audit` (Trivy on both images), CI on a pull
request; the §6.P suite `ok` on kind.

1. **`deploy/charts/kura-operator`**: the generated chart (kit library chart,
   generated RBAC limited to the three kinds, their status, Events and Lease;
   CRDs under `crds.install` with `resource-policy: keep`; webhook Service and
   certificate; alerts), plus the projected token volume (`audience: kura`,
   `expirationSeconds: 3600`).
2. **`deploy/helm/kura`**: `server.documents` in `values.yaml` and its schema;
   the NetworkPolicy admits the operator's namespace on 8080 when
   `documents.source` is `operator`; the `kura-documents` ConfigMap rendered
   only in `files` mode. T3 as a chart render test in `task helm:lint`.
3. **Image and release**: `Containerfile.operator` (multi-stage, cargo-chef,
   distroless nonroot, digest-pinned and in updatecli); `ci.yml` calls the
   kit's `checks.yml`; `release.yml` calls the kit's `publish.yml` on the same
   `v*` tag; `task version:set` and `version:check` cover the operator chart.
4. **Proof**: `tests/heavy/operator.sh`, observations 1–6 of RFC §6.P on kind
   (Kubernetes 1.36, `kubectl` v1.36, dev Authentik, the real `kura` CLI),
   each printing `ok`/`fail` like `authz.sh`; a `task` entry in
   `.tasks/test.yaml`. A kind cluster is not available inside the Che
   workspace: run it on the maintainer's machine or in CI, and say which in
   the notes.
5. **Capacity probe**: one push of 2,000 documents against the dev stack,
   time of `reconcile()` recorded here and in §18.

## Phase 4 — docs and landing

1. Site guide "Operator" (install, migrating from files following RFC §13,
   RBAC delegation per namespace, T2's warning), `server.yaml`'s
   `documents`, the API page's new route, the install guide's choice of
   source, `KuraInstance` in the generated reference.
2. Perf budget with `weebo-kit-perf` against `Budget::small_cluster`.
3. RFC 0009: `## 18. Revision against the tree`, status `Implemented`,
   `Touches` and `Risk` updated, each §7 row with the test that proves it;
   `docs/content/docs/design.mdx` updated.

## Divergences

| # | Phase | RFC section | Divergence | Reason |
| --- | --- | --- | --- | --- |
| D1 | 0 | §16 phase 0 ("merge its crates … the generated operator builds and its template tests pass") | The generated crates are not merged in phase 0; they land in phase 2, already in Kura's shape | Verbatim, they build only with the template's `KuraObject`, `KuraLocalInstance` and `tokenSecretRef`, which §6.3 deletes: merging them first would add code and tests rewritten one phase later |
| D2 | 0 | §4.3 (`KuraInstance` without `spec.auth` refused "by CEL") | `auth` is a required field of the schema, not a CEL rule | Same refusal at admission (`spec.auth: Required value`), with no rule to maintain |

| D3 | 1 | §4.4, §7.2 (`GET /v1/admin/status` admits the operator) | Not opened to the operator | Every push answers with the full status; a second read surface for the same identity adds nothing |
| D4 | 1 | §6.2 (`kura_documents_pushes_total{result}`) | Label `outcome` | The server's metrics take `tenant`, `vault` and `outcome` labels only (`metrics.rs` header) |
| D5 | 1 | §6.1 ("no change to the specs"), §13 ("one migration adds `generation`") | `Metadata` gains an optional `generation`, which `status::build` reports as `observedGeneration` (the hash stays for files); no migration | `deny_unknown_fields` would refuse `metadata.generation`; the status is already `jsonb` in `config_state` |
| D6 | 1 | §4.1, §6.2 (`issuer`, `audience`, `subject`; JWKS "through `KeySource::http`") | Two more keys, `caFile` and `tokenFile`; `KeySource::cluster` sends kura-server's own ServiceAccount token to the discovery and JWKS; a discovery without `userinfo_endpoint` is accepted (userinfo then fails) | Kubernetes serves its issuer discovery to authenticated ServiceAccounts only (`system:service-account-issuer-discovery` is bound to `system:serviceaccounts`), under the cluster CA, and its discovery has no userinfo |
| D7 | 1 | §13 (rollout) | In operator mode the server is ready at start, before any push | Readiness waited for a first snapshot; a fresh server in operator mode would never get one, the operator reaching it through a Service that routes to ready pods only |
| D8 | 1 | §4.2, §7.1 T6 ("a stale leader's push is refused by `kura_config_begin`'s generation guard") | `kura_config_begin` refuses an older *server version* and an unchanged generation; a stale operator leader of the same version can push once | The guard is about server versions (§6.10 of RFC 0001); the Lease makes one leader and the next push from the cache corrects a stale one, as T6's residual risk says. `older_writer_refused` proves the version guard |
| D9 | 1 | §4.5 (`source` "becomes `KuraVault/family/recipes`") | `KuraVault/family/recipes#1` | The source keeps the files' `file#index` shape; each pushed document is index 1 of its own "file" |

Known in advance, to confirm while building:

- Phase 0, step 3: the RFC's `Touches` names `crates/operator/*`; the existing
  crates are flat (`crates/kura-*`). The plan follows the RFC; flattening
  would be a divergence to record.
