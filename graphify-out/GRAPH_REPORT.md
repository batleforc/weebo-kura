# Graph Report - weebo-kura  (2026-11-14)

## Corpus Check
- 213 files · ~175,081 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 19 file(s) not represented in the graph (top: .toml 7, (none) 7, .hardened 1)

## Summary
- 3329 nodes · 7283 edges · 231 communities (171 shown, 60 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 374 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `ee747b15`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- authz.rs
- reconcile/mod.rs
- common/mod.rs
- includes
- ProblemType
- data_keys.rs
- kura-server/src/config.rs
- rfc0001-implementation-plan.md
- steps
- Duration
- rules
- auth/mod.rs
- vars
- validate/tests.rs
- chunk.rs
- spec.rs
- app/mod.rs
- with
- tasks
- CryptoError
- src/db.rs
- fuzz
- Doc / Gen / Guideline
- tests/sync.rs
- vars
- tasks
- ObsidianHost
- .json
- tasks
- status.rs
- kura-wasm/src/lib.rs
- fuzz.yml
- tasks
- tasks
- AppState
- history.rs
- 6. Detailed design
- tasks
- ApiError
- RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults
- tasks
- app/search.rs
- admin.rs
- helpers.js
- Vault
- openapi_contract.rs
- devices.rs
- types.ts
- HostError
- vaults.rs
- metrics.rs
- 7. Security
- devfile.yaml
- 4. User-facing design
- src/sync.rs
- 5. Architecture
- attrs
- ChunkHash
- path.rs
- kura-api
- .claude/CLAUDE.md
- end-of-line.sh
- restage.sh
- whitespace-fixer.sh
- Fail
- KuraTenant dev
- sqlx-sqlite
- package.json
- tasks
- sim.rs
- serve
- Notifier
- app/events.rs
- src/maintenance.rs
- auth.ts
- tests/maintenance.rs
- auth/tests.rs
- .start
- api.rs
- http.rs
- parse.rs
- commands.rs
- main.ts
- tests/events.rs
- Hub
- reconcile.rs
- arc
- changes.rs
- oidc
- Uuid
- tests/tokens.rs
- compilerOptions
- engine.rs
- test_db
- auth/keys.rs
- fuzz_target
- task
- tests/config.rs
- app/tests.rs
- files.rs
- tasks
- serialize
- Command
- tests/chunks.rs
- kura-server/src/main.rs
- tests/merge.rs
- env
- approve.sh script
- env
- sync.sh
- reader.rs
- ObsidianHost
- cli.rs
- dev:network
- Operating kura-server (phases 1 and 2)
- BackupError
- dev:env
- super
- Divergences
- remote.ts
- tests
- values.yaml
- tasks
- init-repo
- ci.yml
- manifest.json
- app/tokens.rs
- Event
- jobs
- test
- s3
- lint
- postgres
- vault
- pnpm-workspace.yaml
- deploy.yaml
- Deserialize
- api
- VaultPath
- plugin
- security
- Algorithm
- wasm.d.ts
- rules
- rules
- limits.rs
- server
- vault
- notify_change
- backup
- cnpg
- 1. Phase 1 — scaffold, configuration, identity, authorization
- kura-cli/src/config.rs
- Chart.yaml
- RFC 0001 — implementation plan
- MERGES
- ConfirmModal
- VaultRemote
- ingress
- resources
- monitoring
- attach.sh
- blob_hash
- image
- tests/search.rs
- databaseUrlSecret
- drill.sh
- run.sh
- deploy
- dev
- perf
- rust
- dev:seed
- dev:stack
- dev:token
- soak.sh
- sync_one
- authz.sh
- Notice
- connect_and_watch
- crypto.rs
- src/merge.rs
- store/keys.rs
- space.rs
- updates
- body.rs
- PluginModal
- Token

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 114 edges
2. `ChunkHash` - 86 edges
3. `SharedState` - 86 edges
4. `RemoteError` - 75 edges
5. `VaultPath` - 72 edges
6. `HostError` - 50 edges
7. `ProblemType` - 44 edges
8. `AppState` - 43 edges
9. `blob_hash()` - 38 edges
10. `DeviceCaller` - 34 edges

## Surprising Connections (you probably didn't know these)
- `Progress` --references--> `ConfirmModal`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/main.ts
- `Progress` --references--> `PluginGuard`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/plugins.ts
- `Progress` --references--> `ProblemError`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/types.ts
- `Progress` --references--> `ObsidianHost`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/host.ts
- `backup()` --references--> `var`  [INFERRED]
  crates/kura-server/src/main.rs → .tasks/perf.yaml

## Import Cycles
- 1-file cycle: `crates/kura-server/src/app/commits.rs -> crates/kura-server/src/app/commits.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (231 total, 60 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.11
Nodes (21): TenantRole, Admin, Member, Owner, ReadAccess, Principal, check_binding(), claim_grants_manager() (+13 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.10
Nodes (24): ConfigFiles, bytes(), check(), json(), load_previous(), Outcome, Applied, NotWritten (+16 more)

### Community 2 - "common/mod.rs"
Cohesion: 0.09
Nodes (6): admin_url(), COUNTER, ISSUER, ROLE_PASSWORD, TestDb, TestKey

### Community 3 - "includes"
Cohesion: 0.20
Nodes (10): flatten, taskfile, includes, coverage, mise, skills, template, test (+2 more)

### Community 4 - "ProblemType"
Cohesion: 0.07
Nodes (28): Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType (+20 more)

### Community 5 - "data_keys.rs"
Cohesion: 0.10
Nodes (10): Captured, CONFIG, env(), two_replicas_creating_one_data_key_keep_one(), vault(), admin(), config(), reads_database_and_bucket_credentials_from_kv() (+2 more)

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.12
Nodes (17): alg_none_cannot_be_configured(), ConfigError, Invalid, Read, CorsConfig, discovery_is_optional_and_https(), err(), EXAMPLE (+9 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.25
Nodes (6): Documentation, Guides, Planned, RFCs, Setup, weebo-kura

### Community 8 - "steps"
Cohesion: 0.23
Nodes (12): steps, BASE, steps, steps, env, if, name, run (+4 more)

### Community 9 - "Duration"
Cohesion: 0.09
Nodes (11): digits(), Duration, Quantity, QuantityRepr, Bytes, Text, ScalarError, Duration (+3 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (29): extends, description, given, message, severity, then, overrides, files (+21 more)

### Community 11 - "auth/mod.rs"
Cohesion: 0.09
Nodes (15): algorithm(), AuthError, Invalid, Jwt, Provider, crate::problem::ApiError, LEEWAY_SECS, MAX_TOKEN_BYTES (+7 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.15
Nodes (25): parse_file(), binding_names_exactly_one_subject(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused(), defaults_are_on_the_closed_side(), documents_round_trip_through_yaml() (+17 more)

### Community 14 - "chunk.rs"
Cohesion: 0.09
Nodes (21): an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, chunks(), ChunkSpan, hashes_round_trip_in_lowercase_hex_only() (+13 more)

### Community 15 - "spec.rs"
Cohesion: 0.12
Nodes (22): VaultRole, Editor, Manager, Viewer, ANNOTATIONS, Binding, Chunking, .DEFAULT (+14 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.08
Nodes (19): ApiDoc, app(), app_router(), audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), cors() (+11 more)

### Community 17 - "with"
Cohesion: 0.09
Nodes (22): jobs, update, name, on, schedule, workflow_dispatch, permissions, contents (+14 more)

### Community 18 - "tasks"
Cohesion: 0.09
Nodes (22): cmds, desc, cmds, desc, preconditions, cmds, desc, msg (+14 more)

### Community 19 - "CryptoError"
Cohesion: 0.33
Nodes (5): CryptoError, Corrupt, KeyLength, Malformed, Random

### Community 20 - "src/db.rs"
Cohesion: 0.17
Nodes (16): ACQUIRE_TIMEOUT, begin_subject(), begin_tenant(), begin_tenant_snapshot(), check_schema(), connect(), crate::problem::ApiError, DbError (+8 more)

### Community 21 - "fuzz"
Cohesion: 0.08
Nodes (23): ASAN_OPTIONS, cmds, desc, status, cmds, desc, env, requires (+15 more)

### Community 22 - "Doc / Gen / Guideline"
Cohesion: 0.10
Nodes (19): Backend, Code, Command, Command output, Devfile, Doc / Gen / Guideline, Dockerfile / Containerfile, Eclipse che - Cloud Development Environment (+11 more)

### Community 23 - "tests/sync.rs"
Cohesion: 0.28
Nodes (20): a_crowded_name_still_gets_a_unique_one(), a_path_freed_and_taken_in_one_commit(), a_pull_during_commits_never_passes_an_unseen_change(), blob(), changes_page_by_seq_and_refuse_a_lost_history(), commit_with_foreign_entry_id_is_refused(), concurrent_commits_never_skip_a_seq(), diverging_edits_become_a_conflict_file() (+12 more)

### Community 24 - "vars"
Cohesion: 0.12
Nodes (16): silent, vars, AK_IMAGE, AK_PORT, NET, PG_CONTAINER, PG_IMAGE, PG_PORT (+8 more)

### Community 25 - "tasks"
Cohesion: 0.11
Nodes (18): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+10 more)

### Community 26 - "ObsidianHost"
Cohesion: 0.07
Nodes (6): Adapter, Entry, FakeServer, json(), MemAdapter, host()

### Community 27 - ".json"
Cohesion: 0.22
Nodes (3): Api, DeviceView, TokenView

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "status.rs"
Cohesion: 0.11
Nodes (12): ReasonCode, .SUCCESS, categories_never_include_what_stays_on_the_device(), category(), plugin_of(), synced(), Removed, build() (+4 more)

### Community 30 - "kura-wasm/src/lib.rs"
Cohesion: 0.11
Nodes (28): bytes(), changes(), commit(), decode(), File, get_blob(), get_chunk(), host_err() (+20 more)

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "tasks"
Cohesion: 0.05
Nodes (36): cmd, for, var, cmds, desc, cmds, desc, requires (+28 more)

### Community 35 - "history.rs"
Cohesion: 0.30
Nodes (8): a_pass_from_zero_pages_past_the_horizon(), eid(), epochs_change_once_with_the_cluster_identity(), history_and_restore(), purged_deletions_move_the_horizon(), retention_prunes_history_but_never_a_head(), S, setup()

### Community 36 - "6. Detailed design"
Cohesion: 0.15
Nodes (13): 6.10 Configuration reconciliation, 6.11 Engineering rules, 6.1 Data model, 6.2 Tenancy, authentication and authorization, 6.3 Sync protocol, 6.4 Chunking and storage, 6.5 Conflict resolution, 6.6 Encryption at rest (+5 more)

### Community 37 - "tasks"
Cohesion: 0.15
Nodes (12): task, cmds, desc, cmds, desc, cmds, desc, tasks (+4 more)

### Community 38 - "ApiError"
Cohesion: 0.14
Nodes (24): needed_claims(), access(), BlobManifest, check_quota(), delete_orphan(), ENCRYPTION_WARNING, get_blob(), get_chunk() (+16 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.11
Nodes (19): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+11 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 41 - "app/search.rs"
Cohesion: 0.16
Nodes (17): BATCH, cut(), index_one(), indexable(), markdown_only_cut_on_a_character(), MAX_INDEXED, MAX_QUERY, MAX_READ (+9 more)

### Community 42 - "admin.rs"
Cohesion: 0.48
Nodes (3): admin_status(), AdminStatus, with_quota_conditions()

### Community 43 - "helpers.js"
Cohesion: 0.17
Nodes (19): RFC-0001, BASE_URL, BLOB, SIZE, TOKEN, VAULT, CLIENT, headers() (+11 more)

### Community 44 - "Vault"
Cohesion: 0.12
Nodes (16): client_err(), decode_b64(), default_region(), kv2_paths_split_into_mount_and_secret(), refresh_credentials(), secrets_never_print(), SERVICE_ACCOUNT_TOKEN, split_kv2() (+8 more)

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "devices.rs"
Cohesion: 0.17
Nodes (14): Caller, Device, device(), list_devices(), MAX_DEVICES, members(), Profile, Desktop (+6 more)

### Community 47 - "types.ts"
Cohesion: 0.11
Nodes (20): RFC-9457, Controller, ControllerOptions, QUIET_MS, readSettings(), problem(), label(), Situation (+12 more)

### Community 48 - "HostError"
Cohesion: 0.13
Nodes (6): FileMeta, Host, HostError, Mem, HostAdapter, meta()

### Community 49 - "vaults.rs"
Cohesion: 0.16
Nodes (20): Body, DeviceCaller, BindingView, ChunkingView, create_vault(), CreateVault, delete_binding(), delete_vault() (+12 more)

### Community 50 - "metrics.rs"
Cohesion: 0.14
Nodes (6): Metrics, metrics_are_exported_with_the_kura_prefix(), refresh_storage(), render(), router(), storage_once()

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
Cohesion: 0.13
Nodes (35): Applied, Deferred, Done, apply(), BATCH, blob_of(), commit(), COMMIT_ATTEMPTS (+27 more)

### Community 55 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (45): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+37 more)

### Community 57 - "ChunkHash"
Cohesion: 0.09
Nodes (8): ChunkHash, Remote, RemoteError, Problem, Scripted, AppRemote, hashes(), RemoteAdapter

### Community 58 - "path.rs"
Cohesion: 0.07
Nodes (26): allowed_char(), check(), ignorable(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX (+18 more)

### Community 59 - "kura-api"
Cohesion: 0.40
Nodes (6): kura-api, kura-cli, kura-core, kura-fuzz, kura-server, kura-wasm

### Community 64 - "Fail"
Cohesion: 0.31
Nodes (19): access_token(), attach(), config(), Ctx, devices(), entry_of(), Fail, history() (+11 more)

### Community 67 - "KuraTenant dev"
Cohesion: 0.06
Nodes (42): claimValue, role, claim, roles, KuraTenant dev, KuraVault notes, KuraVault shared, KuraTenant other (+34 more)

### Community 69 - "package.json"
Cohesion: 0.08
Nodes (29): RFC-0001, description, devDependencies, esbuild@0.28.2, eslint@10.14.0, @eslint/js@10.0.1, obsidian@1.14.4, @types/node@26.6.6 (+21 more)

### Community 70 - "tasks"
Cohesion: 0.06
Nodes (32): cmds, desc, dir, cmds, deps, desc, dir, cmds (+24 more)

### Community 71 - "sim.rs"
Cohesion: 0.12
Nodes (5): Device, devices_converge_and_lose_nothing(), MemHost, Rng, run()

### Community 72 - "serve"
Cohesion: 0.52
Nodes (3): Background, background(), serve()

### Community 73 - "Notifier"
Cohesion: 0.09
Nodes (11): Notifier, NotifierOptions, POLL_MS, SILENCE_MS, SocketLike, Timers, setup(), ManualTimers (+3 more)

### Community 74 - "app/events.rs"
Cohesion: 0.09
Nodes (14): create_ticket(), DEVICE_REVOKED, events(), EventsQuery, PING, serve(), shut(), SLOW_CONSUMER (+6 more)

### Community 75 - "src/maintenance.rs"
Cohesion: 0.13
Nodes (23): run(), BATCH, Batches, collect_chunks(), delete_objects(), DELETED_ENTRY_TTL_HOURS, MaintenanceError, Db (+15 more)

### Community 76 - "auth.ts"
Cohesion: 0.11
Nodes (27): RFC-8628, AuthError, AuthFailure, call(), discover(), Discovery, fnv(), Http (+19 more)

### Community 77 - "tests/maintenance.rs"
Cohesion: 0.17
Nodes (10): LOCK_WAIT, BACKUP_LOCK, a_blob_put_and_collection_never_interleave(), backup_copies_new_objects_and_the_drill_check_catches_corruption(), collection_removes_only_what_nothing_references(), orphans_are_swept_and_the_backup_lock_pauses_deletion(), restored_document_cancels_deletion(), S (+2 more)

### Community 78 - "auth/tests.rs"
Cohesion: 0.30
Nodes (15): a_retired_key_stops_verifying_after_max_age(), accepts_a_valid_access_token(), access_claims(), ISSUER, keys_and_userinfo_come_through_oidc_discovery(), missing_role_claims_come_from_userinfo_and_are_cached(), oidc(), refused() (+7 more)

### Community 79 - ".start"
Cohesion: 0.14
Nodes (8): Core, loadCore(), http(), KuraPlugin, random16(), Api, KuraSettingTab, device()

### Community 80 - "api.rs"
Cohesion: 0.14
Nodes (26): ATTEMPTS, Chunking, CLIENT, DeviceCode, discover(), Discovery, http(), MAX_CHUNK (+18 more)

### Community 81 - "http.rs"
Cohesion: 0.18
Nodes (20): problem(), vault_id(), a_tenant_admin_sees_and_revokes_the_tenants_devices_only(), admin_status_is_for_platform_admins(), cors_allows_the_obsidian_origins_only_and_bodies_are_bounded(), device_links_follow_current_memberships(), device_ownership_and_limits(), foreign_vault_is_404() (+12 more)

### Community 82 - "parse.rs"
Cohesion: 0.12
Nodes (28): TenantSlug, ConfigError, parse_document(), Resource, Tenant, Vault, Source, typed() (+20 more)

### Community 83 - "commands.rs"
Cohesion: 0.14
Nodes (16): a_host_name_is_a_valid_device_name(), BACKOFF_MAX, BACKOFF_MIN, backoff_stays_between_one_and_sixty_seconds(), blob_of(), bytes_for_people(), Diff, diff() (+8 more)

### Community 84 - "main.ts"
Cohesion: 0.10
Nodes (21): DeviceCode, CodeModal, DEFAULTS, registry(), Settings, sleep(), timers, RFC-0001 (+13 more)

### Community 85 - "tests/events.rs"
Cohesion: 0.24
Nodes (12): a_commit_reaches_the_socket(), a_notification_crosses_replicas(), a_ticket_flood_is_limited_per_address(), expired_ticket_is_refused(), next(), replica(), S, setup() (+4 more)

### Community 87 - "Hub"
Cohesion: 0.14
Nodes (12): CHANNEL, four_sockets_per_device_at_most(), HEARTBEAT, Hub, listen(), MAX_SOCKETS, notify_revoked(), payloads_become_events() (+4 more)

### Community 88 - "reconcile.rs"
Cohesion: 0.18
Nodes (17): no_key_verifier(), a_failed_tick_is_retried_as_a_local_change(), an_older_replica_never_overwrites_newer_state(), apply(), count_state(), DIRS, files(), invalid_document_keeps_last_config() (+9 more)

### Community 90 - "changes.rs"
Cohesion: 0.19
Nodes (12): ChangesResponse, ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE (+4 more)

### Community 91 - "oidc"
Cohesion: 0.10
Nodes (20): mount, role, listen, api, metrics, oidc, audiences, discovery (+12 more)

### Community 92 - "Uuid"
Cohesion: 0.12
Nodes (42): CommitRequest, CommitResponse, OpResult, apply(), apply_config(), BaseRev, check_blobs(), Commit (+34 more)

### Community 93 - "tests/tokens.rs"
Cohesion: 0.25
Nodes (9): a_token_is_revoked_with_the_binding_that_allowed_it(), INSIDE, json_of(), OUTSIDE, prefix_token_cannot_fetch_outside_chunk_or_read_renamed_history(), S, send(), setup() (+1 more)

### Community 94 - "compilerOptions"
Cohesion: 0.12
Nodes (20): client_addr(), client_addr_strict(), forwarding_headers_count_only_from_trusted_proxies(), headers(), the_strict_form_never_takes_the_proxy_for_the_client(), compilerOptions, exactOptionalPropertyTypes, isolatedModules (+12 more)

### Community 95 - "engine.rs"
Cohesion: 0.30
Nodes (19): sync(), a_conflict_on_an_open_note_fails_that_file_only(), a_copied_folder_starts_fresh(), a_merged_push_writes_the_merged_head(), a_path_held_by_another_known_entry_is_handed_over(), a_quiet_view_moves_its_cursor_to_the_head(), a_refused_push_keeps_local_changes_and_still_pulls(), a_refused_resend_keeps_the_local_version() (+11 more)

### Community 96 - "test_db"
Cohesion: 0.26
Nodes (12): test_db(), a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls(), jwks_survive_a_restart_without_the_provider(), pooled_connection_does_not_leak_tenant() (+4 more)

### Community 97 - "auth/keys.rs"
Cohesion: 0.12
Nodes (13): Cache, decode_set(), Discovery, Keys, KeySource, Fixed, Http, MAX_AGE (+5 more)

### Community 99 - "task"
Cohesion: 0.19
Nodes (13): cmd, for, task, cmds, cmds, desc, cmds, desc (+5 more)

### Community 100 - "tests/config.rs"
Cohesion: 0.29
Nodes (6): ListenConfig, config_namespace_hidden_from_other_member(), configuration_writes_last_wins_without_conflict_files(), D, S, setup()

### Community 101 - "app/tests.rs"
Cohesion: 0.24
Nodes (9): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp(), state() (+1 more)

### Community 102 - "files.rs"
Cohesion: 0.24
Nodes (11): FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES, read_dir() (+3 more)

### Community 103 - "tasks"
Cohesion: 0.15
Nodes (13): desc, cmds, desc, status, cmds, desc, cmds, desc (+5 more)

### Community 104 - "serialize"
Cohesion: 0.16
Nodes (3): DnsLabel, NameError, String

### Community 105 - "Command"
Cohesion: 0.09
Nodes (20): DeviceAction, Revoke, TokenAction, Create, List, Revoke, Cli, Command (+12 more)

### Community 106 - "tests/chunks.rs"
Cohesion: 0.14
Nodes (6): MAX_RETRIES, s3(), StorageError, TIMEOUT, BucketSecret, DatabaseSecret

### Community 107 - "kura-server/src/main.rs"
Cohesion: 0.24
Nodes (13): backup(), check_chunks(), CONFIG_DIR_ENV, connect(), DATABASE_URL_ENV, dump_spec(), main(), maintain() (+5 more)

### Community 108 - "tests/merge.rs"
Cohesion: 0.25
Nodes (8): request_span(), a_rename_on_one_side_and_edits_on_both_merge(), concurrent_line_edits_lose_nothing(), edits_of_different_lines_merge(), overlapping_and_non_text_edits_conflict(), rev(), S, setup()

### Community 109 - "env"
Cohesion: 0.25
Nodes (8): KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, test:db, cmds, desc, env

### Community 110 - "approve.sh script"
Cohesion: 0.48
Nodes (5): c(), approve.sh script, step(), c(), token.sh script

### Community 111 - "env"
Cohesion: 0.20
Nodes (11): cmds, desc, env, cmds, desc, env, KURA_CONFIG_DIR, KURA_VAULT_TOKEN (+3 more)

### Community 112 - "sync.sh"
Cohesion: 0.39
Nodes (8): as(), cleanup(), fail(), pass(), real(), sync.sh script, sql(), tokens

### Community 113 - "reader.rs"
Cohesion: 0.13
Nodes (12): DEVICE_HEADER, rate_limit(), require_device(), require_session(), bearer(), client(), KeyCaller, Reader (+4 more)

### Community 114 - "ObsidianHost"
Cohesion: 0.16
Nodes (5): Progress, HostOptions, ObsidianHost, parent(), toBuffer()

### Community 115 - "cli.rs"
Cohesion: 0.43
Nodes (13): a_long_absence_resyncs_and_history_restores(), a_rewound_server_gets_everything_back(), an_expired_access_token_is_renewed_mid_run(), code(), fake_issuer(), files(), sign_in_by_device_code_and_refresh_with_rotation(), the_cli_issues_lists_and_revokes_scoped_tokens() (+5 more)

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.11
Nodes (18): 10. The Obsidian plugin, 11. Local stack, 12. Maintenance, backups and restore, 13. Installing with Helm, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults (+10 more)

### Community 118 - "BackupError"
Cohesion: 0.20
Nodes (12): BackupError, Busy, Db, LockLost, NoStorage, Objects, Other, check_chunks() (+4 more)

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 123 - "super"
Cohesion: 0.36
Nodes (5): API_DOCS_CSP, scalar(), SCALAR_BUNDLE_PATH, scalar_unavailable_html(), the_reference_loads_nothing_from_a_third_party()

### Community 124 - "Divergences"
Cohesion: 0.13
Nodes (15): §8 additions (phase 2), §8 additions (phase 3), §8 additions (phase 5), §8 as built (phase 1), Divergences, Phase 1 eco-design review (2026-11-14), Phase 1 security review (2026-11-14), Phase 2 security review (2026-11-14) (+7 more)

### Community 125 - "remote.ts"
Cohesion: 0.15
Nodes (7): Sleep, ApiOptions, CLIENT, Fetch, retryAfter(), VaultView, api()

### Community 126 - "tests"
Cohesion: 0.14
Nodes (13): alertname, eval_time, exp_alerts, evaluation_interval, exp_annotations, exp_labels, series, values (+5 more)

### Community 127 - "values.yaml"
Cohesion: 0.17
Nodes (11): backupsWaived, documents, networkPolicy, enabled, ingressNamespace, monitoringNamespace, podDisruptionBudget, minAvailable (+3 more)

### Community 128 - "tasks"
Cohesion: 0.22
Nodes (8): cmds, desc, tasks, init, update, cmds, desc, version

### Community 129 - "init-repo"
Cohesion: 0.22
Nodes (9): cmds, desc, preconditions, vars, msg, sh, init-repo, NAME (+1 more)

### Community 131 - "ci.yml"
Cohesion: 0.25
Nodes (7): name, on, pull_request, push, permissions, contents, branches

### Community 132 - "manifest.json"
Cohesion: 0.25
Nodes (7): author, description, id, isDesktopOnly, minAppVersion, name, version

### Community 133 - "app/tokens.rs"
Cohesion: 0.20
Nodes (15): check_cidrs(), check_prefix(), create_token(), CreateToken, invalid(), IssuedToken, list_tokens(), manage() (+7 more)

### Community 135 - "Event"
Cohesion: 0.33
Nodes (4): Event, Change, Pull, Revoked

### Community 136 - "jobs"
Cohesion: 0.29
Nodes (7): runs-on, runs-on, jobs, check, image, plugin, runs-on

### Community 137 - "test"
Cohesion: 0.29
Nodes (7): KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, test, env, runs-on

### Community 138 - "s3"
Cohesion: 0.29
Nodes (7): RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY, env, image, ports, s3, services

### Community 139 - "lint"
Cohesion: 0.29
Nodes (7): task, cmds, desc, cmds, desc, lint, recu

### Community 141 - "postgres"
Cohesion: 0.33
Nodes (6): POSTGRES_PASSWORD, env, image, options, ports, postgres

### Community 142 - "vault"
Cohesion: 0.40
Nodes (5): BAO_DEV_ROOT_TOKEN_ID, vault, env, image, ports

### Community 143 - "pnpm-workspace.yaml"
Cohesion: 0.40
Nodes (4): allowBuilds, esbuild, overrides, minimatch

### Community 145 - "deploy.yaml"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, tasks, alerts:test, helm:lint, vars (+3 more)

### Community 147 - "Deserialize"
Cohesion: 0.13
Nodes (15): commit_bodies_parse_strictly(), eid(), entry_ids_are_uuid_v7(), EntryId, Operation, Delete, Put, Outcome (+7 more)

### Community 149 - "api"
Cohesion: 0.67
Nodes (3): flatten, taskfile, api

### Community 150 - "VaultPath"
Cohesion: 0.10
Nodes (11): a_255_byte_name_is_written(), err(), FsHost, mtime(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp() (+3 more)

### Community 151 - "plugin"
Cohesion: 0.67
Nodes (3): plugin, flatten, taskfile

### Community 152 - "security"
Cohesion: 0.67
Nodes (3): security, flatten, taskfile

### Community 153 - "Algorithm"
Cohesion: 0.18
Nodes (11): Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512, RS256 (+3 more)

### Community 157 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 163 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 164 - "limits.rs"
Cohesion: 0.21
Nodes (7): counts_per_key_and_route_and_says_when_to_retry(), limit_of(), LIMITS, MAX_TRACKED, RateLimiter, WINDOW, WindowKey

### Community 165 - "server"
Cohesion: 0.18
Nodes (11): window, audiences, discovery, issuer, platformAdmin, claim, value, server (+3 more)

### Community 166 - "vault"
Cohesion: 0.20
Nodes (10): mount, role, bucket, database, vault, mount, address, kubernetesAuth (+2 more)

### Community 168 - "backup"
Cohesion: 0.22
Nodes (9): backup, credentialsSecret, destinationPath, enabled, endpointURL, retention, schedule, targetSecret (+1 more)

### Community 169 - "cnpg"
Cohesion: 0.25
Nodes (8): cnpg, enabled, instances, name, operatorNamespace, storage, size, storageClass

### Community 170 - "1. Phase 1 — scaffold, configuration, identity, authorization"
Cohesion: 0.18
Nodes (11): 1. Phase 1 — scaffold, configuration, identity, authorization, PR 1.10 — Close the phase, PR 1.1 — Repository scaffold, PR 1.2 — `kura-api`: documents, defaults, `validate()`, PR 1.3 — Server skeleton and API contract, PR 1.4 — Database, roles and row-level security, PR 1.5 — Reconciler and `config_state`, PR 1.6 — Token verification (+3 more)

### Community 171 - "kura-cli/src/config.rs"
Cohesion: 0.18
Nodes (5): Attached, Config, Credentials, set_refresh_token(), write_private()

### Community 172 - "Chart.yaml"
Cohesion: 0.29
Nodes (6): apiVersion, appVersion, description, name, type, version

### Community 173 - "RFC 0001 — implementation plan"
Cohesion: 0.20
Nodes (10): 0. Before the first line of code, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Phase 4 — merge, notifications, plugin, 5. Phase 5 — history, retention, resync, 6. Phase 6 — operations, 7. Phase 7 — scoped tokens and search, 8. Phase 8 — Obsidian configuration (+2 more)

### Community 176 - "VaultRemote"
Cohesion: 0.24
Nodes (4): body(), check(), exchange(), VaultRemote

### Community 177 - "ingress"
Cohesion: 0.33
Nodes (6): ingress, annotations, className, enabled, host, tlsSecret

### Community 178 - "resources"
Cohesion: 0.33
Nodes (6): memory, cpu, memory, resources, limits, requests

### Community 179 - "monitoring"
Cohesion: 0.33
Nodes (6): monitoring, prometheusRule, serviceMonitor, enabled, enabled, interval

### Community 180 - "attach.sh"
Cohesion: 0.60
Nodes (5): as(), cleanup(), login(), attach.sh script, sql()

### Community 181 - "blob_hash"
Cohesion: 0.22
Nodes (13): blob_hash(), Storage, bucket_object_is_not_plaintext(), commit_blob(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422(), objects(), oversized_chunk_is_413() (+5 more)

### Community 182 - "image"
Cohesion: 0.40
Nodes (5): image, digest, pullPolicy, repository, tag

### Community 183 - "tests/search.rs"
Cohesion: 0.22
Nodes (6): Round, UNAVAILABLE_RETRY_AFTER, a_failing_index_job_is_kept_after_five_attempts(), S, search_is_opt_in_indexes_markdown_and_follows_edits(), setup()

### Community 184 - "databaseUrlSecret"
Cohesion: 0.50
Nodes (4): key, name, migrate, databaseUrlSecret

### Community 185 - "drill.sh"
Cohesion: 0.83
Nodes (3): server(), drill.sh script, sql()

### Community 187 - "deploy"
Cohesion: 0.67
Nodes (3): flatten, taskfile, deploy

### Community 188 - "dev"
Cohesion: 0.67
Nodes (3): flatten, taskfile, dev

### Community 189 - "perf"
Cohesion: 0.67
Nodes (3): perf, flatten, taskfile

### Community 190 - "rust"
Cohesion: 0.67
Nodes (3): rust, flatten, taskfile

### Community 191 - "dev:seed"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:seed

### Community 192 - "dev:stack"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:stack

### Community 193 - "dev:token"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:token

### Community 196 - "sync_one"
Cohesion: 0.31
Nodes (5): code_of(), confirm(), exit_codes_follow_section_4_5(), sync_config(), sync_one()

### Community 203 - "authz.sh"
Cohesion: 0.36
Nodes (8): as(), call(), cleanup(), fail(), pass(), authz.sh script, sql(), vault()

### Community 206 - "Notice"
Cohesion: 0.33
Nodes (5): Notice, Change, Ping, Pull, text()

### Community 208 - "connect_and_watch"
Cohesion: 0.22
Nodes (10): connect_and_watch(), fingerprint(), on_socket(), Socketed, Reconnect, Stop, watch(), Watched (+2 more)

### Community 217 - "crypto.rs"
Cohesion: 0.13
Nodes (12): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, Unsupported, FORMAT_V1, HEADER, key(), OBJECT_KEY_CONTEXT (+4 more)

### Community 218 - "src/merge.rs"
Cohesion: 0.27
Nodes (10): edits_of_different_lines_merge(), MAX_MERGE, MAX_MERGE_LINES, Merge, merge3(), Clean, Conflict, mergeable() (+2 more)

### Community 219 - "store/keys.rs"
Cohesion: 0.20
Nodes (6): DataKey, Cache, CACHE_SIZE, CACHE_TTL, DataKeys, transit_key_name()

### Community 221 - "space.rs"
Cohesion: 0.23
Nodes (9): ConfigCategory, Hotkeys, Plugins, Snippets, Themes, categories_and_the_kura_plugin(), category(), resolve() (+1 more)

### Community 224 - "updates"
Cohesion: 0.18
Nodes (9): object_keys_are_keyed_and_unique_per_upload(), prefix, interval, updates, commit-message, directory, package-ecosystem, schedule (+1 more)

## Knowledge Gaps
- **136 isolated node(s):** `Revoke`, `Stop`, `Reconnect`, `Stop`, `Dropped` (+131 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 1219 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **60 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `storage` connect `KuraTenant dev` to `data_keys.rs`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Are the 6 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 6 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Revoke`, `Stop`, `Reconnect` to the rest of the system?**
  _136 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `authz.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1064102564102564 - nodes in this community are weakly interconnected._
- **Why does `ChunkHash` connect `ChunkHash` to `chunk.rs`, `Deserialize`, `tests/sync.rs`, `kura-wasm/src/lib.rs`, `ApiError`, `app/search.rs`, `VaultRemote`, `blob_hash`, `src/sync.rs`, `tests/search.rs`, `sim.rs`, `api.rs`, `commands.rs`, `crypto.rs`, `Uuid`, `tests/tokens.rs`, `engine.rs`, `updates`, `fuzz_target`, `tests/config.rs`, `tests/merge.rs`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.0990990990990991 - nodes in this community are weakly interconnected._
- **Should `common/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.08994708994708994 - nodes in this community are weakly interconnected._