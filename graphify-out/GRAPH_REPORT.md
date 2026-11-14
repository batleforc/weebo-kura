# Graph Report - weebo-kura  (2026-11-14)

## Corpus Check
- 124 files · ~100,103 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 15 file(s) not represented in the graph (top: .toml 6, (none) 6, .hardened 1)

## Summary
- 2102 nodes · 4389 edges · 131 communities (98 shown, 33 thin omitted)
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 190 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `2fc19eb0`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- authz.rs
- reconcile/mod.rs
- auth/tests.rs
- includes
- ProblemType
- reconcile.rs
- kura-server/src/config.rs
- rfc0001-implementation-plan.md
- steps
- Duration
- rules
- auth/keys.rs
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
- vars
- tasks
- parse.rs
- ChunkHash
- tasks
- super
- DnsLabel
- fuzz.yml
- tasks
- commands.rs
- AppState
- limits.rs
- 6. Detailed design
- tasks
- ApiError
- RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults
- tasks
- body.rs
- device.rs
- app
- VaultError
- openapi_contract.rs
- devices.rs
- admin.rs
- VaultPath
- vaults.rs
- metrics.rs
- 7. Security
- devfile.yaml
- 4. User-facing design
- src/sync.rs
- 5. Architecture
- attrs
- 10. Operability
- path.rs
- kura-api
- .claude/CLAUDE.md
- end-of-line.sh
- restage.sh
- whitespace-fixer.sh
- ChunkHash
- KuraTenant dev
- sqlx-sqlite
- serve
- BindingSubject
- sim.rs
- wire.rs
- commits.rs
- ApiError
- auth/mod.rs
- changes.rs
- oidc
- Uuid
- Storage
- test_db
- engine.rs
- src/vault.rs
- Principal
- files.rs
- task
- Algorithm
- Operating kura-server (phases 1 and 2)
- 1. Phase 1 — scaffold, configuration, identity, authorization
- tasks
- AuthError
- fs.rs
- Divergences
- VaultConfig
- snapshot.rs
- env
- approve.sh script
- env
- sync.sh
- BearerToken
- TestDb
- RFC 0001 — implementation plan
- dev:network
- Outcome
- dev:certs
- dev:env
- dev:seed
- dev:token

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 74 edges
2. `ChunkHash` - 65 edges
3. `VaultPath` - 59 edges
4. `RemoteError` - 54 edges
5. `SharedState` - 52 edges
6. `HostError` - 40 edges
7. `ProblemType` - 39 edges
8. `DeviceCaller` - 31 edges
9. `AppState` - 29 edges
10. `attrs` - 29 edges

## Surprising Connections (you probably didn't know these)
- `object_keys_are_keyed_and_unique_per_upload()` --references--> `prefix`  [INFERRED]
  crates/kura-server/src/store/crypto.rs → .github/dependabot.yml
- `params()` --references--> `Chunking`  [INFERRED]
  crates/kura-cli/src/commands.rs → crates/kura-cli/src/api.rs
- `Fail` --implements--> `OidcError`  [INFERRED]
  crates/kura-cli/src/commands.rs → crates/kura-cli/src/api.rs
- `sync_one()` --calls--> `sync()`  [INFERRED]
  crates/kura-cli/src/commands.rs → crates/kura-core/src/sync.rs
- `status()` --calls--> `sync()`  [INFERRED]
  crates/kura-cli/src/commands.rs → crates/kura-core/src/sync.rs

## Import Cycles
- 1-file cycle: `crates/kura-server/src/app/commits.rs -> crates/kura-server/src/app/commits.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/tests/common/app.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (131 total, 33 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.19
Nodes (16): TenantRole, Admin, Member, Owner, VaultRole, Editor, Manager, Viewer (+8 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.11
Nodes (16): bytes(), check(), json(), load_previous(), policy(), POLL, reconcile(), ReconcileError (+8 more)

### Community 2 - "auth/tests.rs"
Cohesion: 0.07
Nodes (33): purge_loop(), a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp() (+25 more)

### Community 3 - "includes"
Cohesion: 0.04
Nodes (46): flatten, taskfile, task, flatten, taskfile, flatten, taskfile, includes (+38 more)

### Community 4 - "ProblemType"
Cohesion: 0.09
Nodes (20): ProblemType, AddressNotAllowed, .ALL, ClientTooOld, CursorExpired, DeviceRevoked, DeviceUnknown, HashMismatch (+12 more)

### Community 5 - "reconcile.rs"
Cohesion: 0.23
Nodes (14): a_failed_tick_is_retried_as_a_local_change(), apply(), count_state(), DIRS, invalid_document_keeps_last_config(), orphan_disables_and_delete_schedules(), reapplying_keeps_api_bindings_and_skips_unchanged_files(), restored_document_cancels_deletion() (+6 more)

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.15
Nodes (11): alg_none_cannot_be_configured(), ClaimMatch, CorsConfig, err(), EXAMPLE, issuer_must_be_https(), ListenConfig, missing_file_is_a_read_error() (+3 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.25
Nodes (6): Documentation, Guides, Planned, RFCs, Setup, weebo-kura

### Community 8 - "steps"
Cohesion: 0.05
Nodes (48): runs-on, steps, BAO_DEV_ROOT_TOKEN_ID, BASE, KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN (+40 more)

### Community 9 - "Duration"
Cohesion: 0.09
Nodes (11): digits(), Duration, Quantity, QuantityRepr, Bytes, Text, ScalarError, Duration (+3 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (27): extends, description, given, message, severity, then, overrides, files (+19 more)

### Community 11 - "auth/keys.rs"
Cohesion: 0.14
Nodes (10): Cache, decode_set(), Discovery, Keys, KeySource, Fixed, MAX_AGE, MAX_KEYS (+2 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.16
Nodes (25): parse_file(), binding_names_exactly_one_subject(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused(), defaults_are_on_the_closed_side(), documents_round_trip_through_yaml() (+17 more)

### Community 14 - "kura-api/src/lib.rs"
Cohesion: 0.19
Nodes (18): TenantSlug, ConfigError, Source, KuraTenant, KuraVault, VaultKey, check_removals(), check_tenants() (+10 more)

### Community 15 - "spec.rs"
Cohesion: 0.13
Nodes (21): ANNOTATIONS, Binding, Chunking, .DEFAULT, .MAX_CHUNK, ClaimMappings, ConfigCategory, Hotkeys (+13 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.10
Nodes (15): ApiDoc, audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), docs_csp(), healthz(), MAX_BODY_BYTES (+7 more)

### Community 17 - "with"
Cohesion: 0.09
Nodes (22): jobs, update, name, on, schedule, workflow_dispatch, permissions, contents (+14 more)

### Community 18 - "tasks"
Cohesion: 0.09
Nodes (22): cmds, desc, cmds, desc, preconditions, cmds, desc, msg (+14 more)

### Community 19 - "crypto.rs"
Cohesion: 0.06
Nodes (32): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, CryptoError, Corrupt, KeyLength, Malformed, Random (+24 more)

### Community 20 - "src/db.rs"
Cohesion: 0.16
Nodes (12): ACQUIRE_TIMEOUT, begin_tenant_snapshot(), check_schema(), connect(), crate::problem::ApiError, DbError, Migrate, SchemaTooOld (+4 more)

### Community 21 - "fuzz"
Cohesion: 0.08
Nodes (23): ASAN_OPTIONS, cmds, desc, status, cmds, desc, env, requires (+15 more)

### Community 22 - "Doc / Gen / Guideline"
Cohesion: 0.10
Nodes (19): Backend, Code, Command, Command output, Devfile, Doc / Gen / Guideline, Dockerfile / Containerfile, Eclipse che - Cloud Development Environment (+11 more)

### Community 23 - "http.rs"
Cohesion: 0.06
Nodes (57): bucket_object_is_not_plaintext(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422(), objects(), oversized_chunk_is_413(), put(), quota_and_roles_bound_uploads(), setup() (+49 more)

### Community 24 - "vars"
Cohesion: 0.14
Nodes (14): vars, AK_IMAGE, AK_PORT, NET, PG_CONTAINER, PG_IMAGE, PG_PORT, S3_CONTAINER (+6 more)

### Community 25 - "tasks"
Cohesion: 0.11
Nodes (18): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+10 more)

### Community 26 - "parse.rs"
Cohesion: 0.15
Nodes (11): parse_document(), Resource, Tenant, Vault, typed(), API_VERSION, Document, KIND_TENANT (+3 more)

### Community 27 - "ChunkHash"
Cohesion: 0.06
Nodes (34): Api, ATTEMPTS, body(), check(), Chunking, CLIENT, DeviceCode, DeviceView (+26 more)

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "super"
Cohesion: 0.08
Nodes (15): ReasonCode, .SUCCESS, client_addr(), forwarding_headers_count_only_from_trusted_proxies(), headers(), Removed, build(), observed_generation() (+7 more)

### Community 30 - "DnsLabel"
Cohesion: 0.14
Nodes (4): DnsLabel, NameError, String, Metadata

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "commands.rs"
Cohesion: 0.06
Nodes (46): access_token(), attach(), code_of(), config(), confirm(), Ctx, DeviceAction, Revoke (+38 more)

### Community 34 - "AppState"
Cohesion: 0.15
Nodes (5): AppState, HttpSettings, ConfigError, Invalid, Read

### Community 35 - "limits.rs"
Cohesion: 0.21
Nodes (7): counts_per_key_and_route_and_says_when_to_retry(), limit_of(), LIMITS, MAX_TRACKED, RateLimiter, WINDOW, WindowKey

### Community 36 - "6. Detailed design"
Cohesion: 0.15
Nodes (13): 6.10 Configuration reconciliation, 6.11 Engineering rules, 6.1 Data model, 6.2 Tenancy, authentication and authorization, 6.3 Sync protocol, 6.4 Chunking and storage, 6.5 Conflict resolution, 6.6 Encryption at rest (+5 more)

### Community 37 - "tasks"
Cohesion: 0.15
Nodes (12): task, cmds, desc, cmds, desc, cmds, desc, tasks (+4 more)

### Community 38 - "ApiError"
Cohesion: 0.19
Nodes (19): Body, access(), BlobManifest, check_quota(), delete_orphan(), ENCRYPTION_WARNING, get_blob(), get_chunk() (+11 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.13
Nodes (15): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 15. Decisions and open questions, 16. Implementation phases, 1. Summary, 2. Motivation (+7 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 42 - "device.rs"
Cohesion: 0.29
Nodes (3): DEVICE_HEADER, rate_limit(), require_device()

### Community 43 - "app"
Cohesion: 0.24
Nodes (5): app(), app_router(), cors(), openapi_spec(), SecurityAddon

### Community 44 - "VaultError"
Cohesion: 0.22
Nodes (8): client_err(), decode_b64(), refresh_credentials(), Vault, VaultError, Client, Path, ServiceAccount

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "devices.rs"
Cohesion: 0.19
Nodes (15): Device, device(), list_devices(), MAX_DEVICES, members(), Profile, Desktop, Mobile (+7 more)

### Community 47 - "admin.rs"
Cohesion: 0.36
Nodes (4): admin_status(), AdminStatus, with_quota_conditions(), ObjectStatus

### Community 48 - "VaultPath"
Cohesion: 0.10
Nodes (6): err(), FsHost, mtime(), VaultPath, HostError, Mem

### Community 49 - "vaults.rs"
Cohesion: 0.25
Nodes (16): DeviceCaller, BindingView, ChunkingView, create_vault(), CreateVault, delete_binding(), delete_vault(), list_vaults() (+8 more)

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

### Community 54 - "src/sync.rs"
Cohesion: 0.12
Nodes (30): Applied, Deferred, Done, apply(), BATCH, commit(), COMMIT_ATTEMPTS, download() (+22 more)

### Community 55 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (44): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+36 more)

### Community 57 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 58 - "path.rs"
Cohesion: 0.07
Nodes (26): allowed_char(), check(), ignorable(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX (+18 more)

### Community 59 - "kura-api"
Cohesion: 0.50
Nodes (5): kura-api, kura-cli, kura-core, kura-fuzz, kura-server

### Community 64 - "ChunkHash"
Cohesion: 0.13
Nodes (18): blob_of(), an_edit_in_the_middle_keeps_most_chunks(), blob_hash(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, ChunkParams (+10 more)

### Community 67 - "KuraTenant dev"
Cohesion: 0.07
Nodes (32): claimValue, role, claim, roles, KuraTenant dev, KuraVault notes, KuraVault shared, apiVersion (+24 more)

### Community 69 - "serve"
Cohesion: 0.52
Nodes (3): Background, background(), serve()

### Community 70 - "BindingSubject"
Cohesion: 0.47
Nodes (3): BindingSubject, Claim, User

### Community 71 - "sim.rs"
Cohesion: 0.13
Nodes (6): FileMeta, Device, devices_converge_and_lose_nothing(), MemHost, Rng, run()

### Community 72 - "wire.rs"
Cohesion: 0.13
Nodes (14): commit_bodies_parse_strictly(), eid(), entry_ids_are_uuid_v7(), EntryId, EntryState, Operation, Delete, Put (+6 more)

### Community 75 - "commits.rs"
Cohesion: 0.14
Nodes (13): CommitRequest, CommitResponse, check_blobs(), hash_of(), head_from_row(), head_select!, MAX_COMMIT_BYTES, MAX_OPERATIONS (+5 more)

### Community 79 - "ApiError"
Cohesion: 0.17
Nodes (10): ApiError, Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response() (+2 more)

### Community 80 - "auth/mod.rs"
Cohesion: 0.12
Nodes (7): algorithm(), LEEWAY_SECS, MAX_TOKEN_BYTES, now(), USERINFO_CACHE, UserinfoCache, Verifier

### Community 90 - "changes.rs"
Cohesion: 0.21
Nodes (10): ChangesResponse, ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE (+2 more)

### Community 91 - "oidc"
Cohesion: 0.10
Nodes (19): mount, role, listen, api, metrics, oidc, audiences, issuer (+11 more)

### Community 92 - "Uuid"
Cohesion: 0.33
Nodes (11): apply(), BaseRev, Commit, Head, load_bases(), path_key(), put_new(), refused() (+3 more)

### Community 93 - "Storage"
Cohesion: 0.17
Nodes (6): MAX_RETRIES, s3(), Storage, StorageError, TIMEOUT, BucketSecret

### Community 94 - "test_db"
Cohesion: 0.20
Nodes (13): test_db(), a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls(), jwks_survive_a_restart_without_the_provider(), pooled_connection_does_not_leak_tenant() (+5 more)

### Community 95 - "engine.rs"
Cohesion: 0.39
Nodes (14): sync(), a_copied_folder_starts_fresh(), a_path_held_by_another_known_entry_is_handed_over(), a_refused_push_keeps_local_changes_and_still_pulls(), a_remote_change_never_overwrites_an_unknown_local_file(), an_interrupted_write_is_adopted_on_restart(), an_unwritten_item_holds_the_cursor(), corrupt_content_is_never_written() (+6 more)

### Community 96 - "src/vault.rs"
Cohesion: 0.16
Nodes (8): DatabaseSecret, default_region(), kv2_paths_split_into_mount_and_secret(), secrets_never_print(), SERVICE_ACCOUNT_TOKEN, split_kv2(), TIMEOUT, TOKEN_ENV

### Community 97 - "Principal"
Cohesion: 0.23
Nodes (3): Caller, needed_claims(), Principal

### Community 98 - "files.rs"
Cohesion: 0.26
Nodes (13): ConfigFiles, FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES (+5 more)

### Community 99 - "task"
Cohesion: 0.18
Nodes (14): cmd, for, task, cmds, cmds, cmds, desc, cmds (+6 more)

### Community 100 - "Algorithm"
Cohesion: 0.18
Nodes (11): Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512, RS256 (+3 more)

### Community 101 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.18
Nodes (11): 10. Local stack, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults, 5. Vault and object storage, 6. Authentik, 7. Devices (+3 more)

### Community 102 - "1. Phase 1 — scaffold, configuration, identity, authorization"
Cohesion: 0.18
Nodes (11): 1. Phase 1 — scaffold, configuration, identity, authorization, PR 1.10 — Close the phase, PR 1.1 — Repository scaffold, PR 1.2 — `kura-api`: documents, defaults, `validate()`, PR 1.3 — Server skeleton and API contract, PR 1.4 — Database, roles and row-level security, PR 1.5 — Reconciler and `config_state`, PR 1.6 — Token verification (+3 more)

### Community 103 - "tasks"
Cohesion: 0.18
Nodes (10): desc, cmds, desc, desc, silent, tasks, dev:authentik, dev:down (+2 more)

### Community 104 - "AuthError"
Cohesion: 0.24
Nodes (7): Http, provider(), AuthError, Invalid, Jwt, Provider, crate::problem::ApiError

### Community 105 - "fs.rs"
Cohesion: 0.44
Nodes (7): a_255_byte_name_is_written(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp(), TRASH_DIR, writes_atomically_and_lists_notes_only()

### Community 106 - "Divergences"
Cohesion: 0.22
Nodes (9): §8 additions (phase 2), §8 additions (phase 3), §8 as built (phase 1), Divergences, Phase 1 eco-design review (2026-11-14), Phase 1 security review (2026-11-14), Phase 2 security review (2026-11-14), Phase 3 security review (2026-11-14) (+1 more)

### Community 107 - "VaultConfig"
Cohesion: 0.29
Nodes (5): TransitConfig, VaultConfig, VaultKubernetesAuth, VaultSecrets, Address

### Community 108 - "snapshot.rs"
Cohesion: 0.39
Nodes (3): load_if_changed(), REFRESH, run()

### Community 109 - "env"
Cohesion: 0.25
Nodes (8): KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, test:db, cmds, desc, env

### Community 110 - "approve.sh script"
Cohesion: 0.48
Nodes (5): c(), approve.sh script, step(), c(), token.sh script

### Community 111 - "env"
Cohesion: 0.29
Nodes (7): cmds, desc, env, KURA_CONFIG_DIR, KURA_VAULT_TOKEN, SSL_CERT_FILE, dev:run

### Community 112 - "sync.sh"
Cohesion: 0.48
Nodes (6): as(), fail(), pass(), sync.sh script, sql(), tokens

### Community 115 - "RFC 0001 — implementation plan"
Cohesion: 0.33
Nodes (6): 0. Before the first line of code, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Later phases (outline, detailed when the previous one closes), Progress, RFC 0001 — implementation plan

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Outcome"
Cohesion: 0.50
Nodes (4): Outcome, Applied, NotWritten, Rejected

### Community 118 - "dev:certs"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:certs

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 120 - "dev:seed"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:seed

### Community 121 - "dev:token"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:token

## Knowledge Gaps
- **99 isolated node(s):** `kura-cli`, `Denied`, `InvalidGrant`, `Revoke`, `Login` (+94 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 804 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **33 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ChunkHash` connect `ChunkHash` to `ChunkHash`, `commands.rs`, `auth/tests.rs`, `ApiError`, `sim.rs`, `wire.rs`, `commits.rs`, `crypto.rs`, `src/sync.rs`, `http.rs`, `Uuid`, `engine.rs`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **Are the 5 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 5 INFERRED edges - model-reasoned connections that need verification._
- **What connects `kura-cli`, `Denied`, `InvalidGrant` to the rest of the system?**
  _99 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.11330049261083744 - nodes in this community are weakly interconnected._
- **Why does `VaultPath` connect `VaultPath` to `ChunkHash`, `commands.rs`, `sim.rs`, `wire.rs`, `fs.rs`, `commits.rs`, `src/sync.rs`, `path.rs`, `Uuid`, `engine.rs`?**
  _High betweenness centrality (0.037) - this node is a cross-community bridge._
- **Should `auth/tests.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06605222734254992 - nodes in this community are weakly interconnected._
- **Why does `problem()` connect `http.rs` to `ApiError`, `commits.rs`, `devices.rs`, `admin.rs`, `app/mod.rs`, `vaults.rs`, `changes.rs`?**
  _High betweenness centrality (0.028) - this node is a cross-community bridge._