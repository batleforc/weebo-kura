# Graph Report - weebo-kura  (2026-11-14)

## Corpus Check
- 99 files · ~73,618 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 15 file(s) not represented in the graph (top: .toml 6, (none) 6, .hardened 1)

## Summary
- 1487 nodes · 2746 edges · 91 communities (67 shown, 24 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 137 edges (avg confidence: 0.78)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `a7eca776`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- authz.rs
- reconcile/mod.rs
- auth/tests.rs
- includes
- ProblemType
- test_db
- config.rs
- 1. Phase 1 — scaffold, configuration, identity, authorization
- steps
- Duration
- rules
- auth/mod.rs
- vars
- validate/tests.rs
- kura-api/src/lib.rs
- spec.rs
- app/mod.rs
- with
- tasks
- crypto.rs
- src/db.rs
- fuzz
- Doc / Gen / Guideline
- http.rs
- test:db
- tasks
- parse.rs
- quantity.rs
- tasks
- super
- DnsLabel
- fuzz.yml
- tasks
- main.rs
- AppState
- store/keys.rs
- 6. Detailed design
- tasks
- ApiError
- RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults
- tasks
- body.rs
- SharedState
- app
- src/vault.rs
- openapi_contract.rs
- devices.rs
- admin.rs
- updates
- vaults.rs
- metrics.rs
- 7. Security
- devfile.yaml
- 4. User-facing design
- CryptoError
- 5. Architecture
- ci.yml
- 10. Operability
- 15. Decisions and open questions
- kura-api
- .claude/CLAUDE.md
- end-of-line.sh
- restage.sh
- whitespace-fixer.sh
- ChunkHash
- test
- sqlx-sqlite
- serve
- BindingSubject
- postgres
- s3
- vault
- jobs
- ChunkError

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 51 edges
2. `SharedState` - 46 edges
3. `ProblemType` - 37 edges
4. `AppState` - 29 edges
5. `test_db()` - 27 edges
6. `DeviceCaller` - 25 edges
7. `ChunkHash` - 21 edges
8. `validate()` - 18 edges
9. `problem()` - 18 edges
10. `env()` - 18 edges

## Surprising Connections (you probably didn't know these)
- `object_keys_are_keyed_and_unique_per_upload()` --references--> `prefix`  [INFERRED]
  crates/kura-server/src/store/crypto.rs → .github/dependabot.yml
- `errors()` --references--> `PreviousState`  [INFERRED]
  crates/kura-api/src/validate/tests.rs → crates/kura-api/src/validate/mod.rs
- `previous_tenants()` --references--> `PreviousState`  [INFERRED]
  crates/kura-api/src/validate/tests.rs → crates/kura-api/src/validate/mod.rs
- `binding_names_exactly_one_subject()` --calls--> `validate()`  [INFERRED]
  crates/kura-api/src/validate/tests.rs → crates/kura-api/src/validate/mod.rs
- `chunking_cannot_change_after_creation()` --calls--> `validate()`  [INFERRED]
  crates/kura-api/src/validate/tests.rs → crates/kura-api/src/validate/mod.rs

## Import Cycles
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/tests/common/app.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (91 total, 24 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.18
Nodes (18): TenantRole, Admin, Member, Owner, VaultRole, Editor, Manager, Viewer (+10 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.05
Nodes (40): ConfigFiles, FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES (+32 more)

### Community 2 - "auth/tests.rs"
Cohesion: 0.08
Nodes (30): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), known_clients_below_their_minimum_are_too_old(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp() (+22 more)

### Community 3 - "includes"
Cohesion: 0.04
Nodes (46): flatten, taskfile, task, flatten, taskfile, flatten, taskfile, includes (+38 more)

### Community 4 - "ProblemType"
Cohesion: 0.07
Nodes (29): Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType (+21 more)

### Community 5 - "test_db"
Cohesion: 0.10
Nodes (28): test_db(), TestDb, a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls(), jwks_survive_a_restart_without_the_provider() (+20 more)

### Community 6 - "config.rs"
Cohesion: 0.07
Nodes (29): alg_none_cannot_be_configured(), Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512 (+21 more)

### Community 7 - "1. Phase 1 — scaffold, configuration, identity, authorization"
Cohesion: 0.05
Nodes (38): 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults, 5. Vault and object storage, 6. Authentik, 7. Devices, 8. Signals (+30 more)

### Community 8 - "steps"
Cohesion: 0.24
Nodes (11): steps, BASE, steps, env, if, name, run, uses (+3 more)

### Community 9 - "Duration"
Cohesion: 0.13
Nodes (9): Duration, Quantity, QuantityRepr, Bytes, Text, String, Chunking, .DEFAULT (+1 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (27): extends, description, given, message, severity, then, overrides, files (+19 more)

### Community 11 - "auth/mod.rs"
Cohesion: 0.05
Nodes (25): Cache, decode_set(), Discovery, Keys, KeySource, Fixed, Http, MAX_AGE (+17 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.16
Nodes (25): parse_file(), binding_names_exactly_one_subject(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused(), defaults_are_on_the_closed_side(), documents_round_trip_through_yaml() (+17 more)

### Community 14 - "kura-api/src/lib.rs"
Cohesion: 0.21
Nodes (18): TenantSlug, ConfigError, KuraTenant, KuraVault, VaultKey, check_removals(), check_tenants(), check_vaults() (+10 more)

### Community 15 - "spec.rs"
Cohesion: 0.13
Nodes (21): ANNOTATIONS, API_VERSION, Binding, ClaimMappings, ConfigCategory, Hotkeys, Plugins, Snippets (+13 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.11
Nodes (14): ApiDoc, audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), docs_csp(), healthz(), MAX_BODY_BYTES (+6 more)

### Community 17 - "with"
Cohesion: 0.09
Nodes (22): jobs, update, name, on, schedule, workflow_dispatch, permissions, contents (+14 more)

### Community 18 - "tasks"
Cohesion: 0.09
Nodes (22): cmds, desc, cmds, desc, preconditions, cmds, desc, msg (+14 more)

### Community 19 - "crypto.rs"
Cohesion: 0.12
Nodes (13): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, DataKey, FORMAT_V1, HEADER, key(), OBJECT_KEY_CONTEXT (+5 more)

### Community 20 - "src/db.rs"
Cohesion: 0.19
Nodes (13): ACQUIRE_TIMEOUT, begin_subject(), begin_tenant(), check_schema(), connect(), crate::problem::ApiError, DbError, Migrate (+5 more)

### Community 21 - "fuzz"
Cohesion: 0.10
Nodes (20): ASAN_OPTIONS, cmds, desc, status, cmds, desc, env, requires (+12 more)

### Community 22 - "Doc / Gen / Guideline"
Cohesion: 0.10
Nodes (19): Backend, Code, Command, Command output, Devfile, Doc / Gen / Guideline, Dockerfile / Containerfile, Eclipse che - Cloud Development Environment (+11 more)

### Community 23 - "http.rs"
Cohesion: 0.05
Nodes (44): blob_hash(), bucket_object_is_not_plaintext(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422(), objects(), oversized_chunk_is_413(), put(), quota_and_roles_bound_uploads() (+36 more)

### Community 24 - "test:db"
Cohesion: 0.05
Nodes (36): cmds, desc, cmds, desc, status, cmds, desc, status (+28 more)

### Community 25 - "tasks"
Cohesion: 0.11
Nodes (18): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+10 more)

### Community 26 - "parse.rs"
Cohesion: 0.16
Nodes (9): parse_document(), Resource, Tenant, Vault, Source, typed(), Document, first() (+1 more)

### Community 27 - "quantity.rs"
Cohesion: 0.12
Nodes (5): digits(), ScalarError, Duration, Quantity, UNITS

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "super"
Cohesion: 0.12
Nodes (10): ReasonCode, .SUCCESS, client_addr(), forwarding_headers_count_only_from_trusted_proxies(), headers(), API_DOCS_CSP, scalar(), SCALAR_BUNDLE_PATH (+2 more)

### Community 30 - "DnsLabel"
Cohesion: 0.18
Nodes (4): DnsLabel, NameError, String, Metadata

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "main.rs"
Cohesion: 0.21
Nodes (8): CONFIG_DIR_ENV, DATABASE_URL_ENV, dump_spec(), main(), migrate(), run(), stop_on_signal(), USAGE

### Community 35 - "store/keys.rs"
Cohesion: 0.11
Nodes (12): counts_per_key_and_route_and_says_when_to_retry(), limit_of(), LIMITS, MAX_TRACKED, RateLimiter, WINDOW, WindowKey, Cache (+4 more)

### Community 36 - "6. Detailed design"
Cohesion: 0.15
Nodes (13): 6.10 Configuration reconciliation, 6.11 Engineering rules, 6.1 Data model, 6.2 Tenancy, authentication and authorization, 6.3 Sync protocol, 6.4 Chunking and storage, 6.5 Conflict resolution, 6.6 Encryption at rest (+5 more)

### Community 37 - "tasks"
Cohesion: 0.15
Nodes (12): task, cmds, desc, cmds, desc, cmds, desc, tasks (+4 more)

### Community 38 - "ApiError"
Cohesion: 0.24
Nodes (18): access(), BlobManifest, check_quota(), delete_orphan(), ENCRYPTION_WARNING, get_blob(), get_chunk(), MAX_CHUNK_BYTES (+10 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.17
Nodes (12): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 1. Summary, 2. Motivation, 3. Goals / non-goals (+4 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 42 - "SharedState"
Cohesion: 0.20
Nodes (7): Caller, needed_claims(), DEVICE_HEADER, DeviceCaller, rate_limit(), require_device(), SharedState

### Community 43 - "app"
Cohesion: 0.24
Nodes (5): app(), app_router(), cors(), openapi_spec(), SecurityAddon

### Community 44 - "src/vault.rs"
Cohesion: 0.07
Nodes (24): provider(), MAX_RETRIES, s3(), Storage, StorageError, TIMEOUT, BucketSecret, client_err() (+16 more)

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "devices.rs"
Cohesion: 0.17
Nodes (13): Device, device(), list_devices(), MAX_DEVICES, members(), Profile, Desktop, Mobile (+5 more)

### Community 47 - "admin.rs"
Cohesion: 0.46
Nodes (4): admin_status(), AdminStatus, with_quota_conditions(), ObjectStatus

### Community 48 - "updates"
Cohesion: 0.25
Nodes (7): interval, updates, commit-message, directory, package-ecosystem, schedule, version

### Community 49 - "vaults.rs"
Cohesion: 0.25
Nodes (14): Body, BindingView, create_vault(), CreateVault, delete_binding(), delete_vault(), list_vaults(), manageable() (+6 more)

### Community 50 - "metrics.rs"
Cohesion: 0.21
Nodes (4): Metrics, metrics_are_exported_with_the_kura_prefix(), render(), router()

### Community 51 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 52 - "devfile.yaml"
Cohesion: 0.33
Nodes (5): metadata, name, parent, uri, schemaVersion

### Community 53 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 54 - "CryptoError"
Cohesion: 0.28
Nodes (7): CryptoError, Corrupt, KeyLength, Malformed, Random, Unsupported, open()

### Community 55 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

### Community 56 - "ci.yml"
Cohesion: 0.25
Nodes (7): name, on, pull_request, push, permissions, contents, branches

### Community 57 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 58 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 59 - "kura-api"
Cohesion: 0.67
Nodes (4): kura-api, kura-core, kura-fuzz, kura-server

### Community 64 - "ChunkHash"
Cohesion: 0.16
Nodes (13): an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkHash, ChunkParams, chunks(), ChunkSpan, hashes_round_trip_in_lowercase_hex_only(), MAX_CHUNK (+5 more)

### Community 67 - "test"
Cohesion: 0.25
Nodes (8): KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, test, env, runs-on, services

### Community 69 - "serve"
Cohesion: 0.52
Nodes (3): Background, background(), serve()

### Community 70 - "BindingSubject"
Cohesion: 0.47
Nodes (3): BindingSubject, Claim, User

### Community 71 - "postgres"
Cohesion: 0.33
Nodes (6): POSTGRES_PASSWORD, env, image, options, ports, postgres

### Community 72 - "s3"
Cohesion: 0.33
Nodes (6): RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY, env, image, ports, s3

### Community 75 - "vault"
Cohesion: 0.33
Nodes (6): SKIP_SETCAP, VAULT_DEV_ROOT_TOKEN_ID, vault, env, image, ports

### Community 79 - "jobs"
Cohesion: 0.40
Nodes (5): runs-on, runs-on, jobs, check, image

### Community 80 - "ChunkError"
Cohesion: 0.67
Nodes (3): ChunkError, Hash, Params

## Knowledge Gaps
- **67 isolated node(s):** `Bytes`, `Text`, `Duration`, `Config`, `Api` (+62 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 601 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **24 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `SharedState` connect `SharedState` to `auth/tests.rs`, `serve`, `ApiError`, `app`, `devices.rs`, `admin.rs`, `app/mod.rs`, `vaults.rs`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Are the 5 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 5 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Bytes`, `Text`, `Duration` to the rest of the system?**
  _67 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.05268065268065268 - nodes in this community are weakly interconnected._
- **Why does `AppState` connect `AppState` to `authz.rs`, `reconcile/mod.rs`, `store/keys.rs`, `config.rs`, `auth/mod.rs`, `src/vault.rs`, `app/mod.rs`, `metrics.rs`, `http.rs`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **Are the 3 inferred relationships involving `AppState` (e.g. with `ConfigError` and `RateLimiter`) actually correct?**
  _`AppState` has 3 INFERRED edges - model-reasoned connections that need verification._
- **Should `auth/tests.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.08148148148148149 - nodes in this community are weakly interconnected._