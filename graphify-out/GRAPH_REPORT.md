# Graph Report - weebo-kura  (2026-11-15)

## Corpus Check
- 223 files · ~194,977 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 19 file(s) not represented in the graph (top: .toml 7, (none) 7, .hardened 1)

## Summary
- 3538 nodes · 7717 edges · 251 communities (170 shown, 81 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 376 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `eb6ee244`
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
- ChunkParams
- spec.rs
- app/mod.rs
- with
- tasks
- RFC 0002 — Batched chunk and manifest transfers
- src/db.rs
- fuzz
- Doc / Gen / Guideline
- tasks
- vars
- tasks
- MemAdapter
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
- Deserialize
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
- token.rs
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
- tests/sync.rs
- .fill
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
- store/keys.rs
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
- Mem
- dev:env
- CryptoError
- app
- Divergences
- remote.ts
- tests
- values.yaml
- tasks
- init-repo
- scalar.rs
- ci.yml
- manifest.json
- app/tokens.rs
- AppRemote
- jobs
- test
- s3
- lint
- postgres
- vault
- pnpm-workspace.yaml
- deploy.yaml
- EntryId
- api
- fs.rs
- ReadAccess
- security
- Algorithm
- wasm.d.ts
- rules
- rules
- limits.rs
- server
- vault
- CodeModal
- backup
- cnpg
- Documentation
- kura-cli/src/config.rs
- Chart.yaml
- .object_key
- 10. Operability
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
- DataKey
- android.sh
- soak.sh
- authz.sh
- dev:certs
- obsidian
- test
- crypto.rs
- dev:certs:renew
- PluginModal
- Token
- 5. Architecture

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 122 edges
2. `ChunkHash` - 100 edges
3. `RemoteError` - 93 edges
4. `SharedState` - 90 edges
5. `VaultPath` - 74 edges
6. `HostError` - 51 edges
7. `ProblemType` - 44 edges
8. `AppState` - 43 edges
9. `blob_hash()` - 40 edges
10. `Env` - 36 edges

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
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (251 total, 81 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.13
Nodes (27): TenantRole, Admin, Member, Owner, VaultRole, Editor, Manager, Viewer (+19 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.10
Nodes (24): ConfigFiles, bytes(), check(), json(), load_previous(), Outcome, Applied, NotWritten (+16 more)

### Community 2 - "common/mod.rs"
Cohesion: 0.09
Nodes (7): admin_url(), COUNTER, ISSUER, ROLE_PASSWORD, TestDb, TestKey, DIRS

### Community 3 - "includes"
Cohesion: 0.20
Nodes (10): flatten, taskfile, includes, coverage, mise, plugin, skills, template (+2 more)

### Community 4 - "ProblemType"
Cohesion: 0.07
Nodes (28): RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType, AddressNotAllowed (+20 more)

### Community 5 - "data_keys.rs"
Cohesion: 0.10
Nodes (11): Captured, CONFIG, env(), two_replicas_creating_one_data_key_keep_one(), vault(), admin(), config(), env() (+3 more)

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.11
Nodes (18): alg_none_cannot_be_configured(), ConfigError, Invalid, Read, CorsConfig, discovery_is_optional_and_https(), err(), EXAMPLE (+10 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.26
Nodes (5): Divergences, Progress, RFC 0002 — implementation plan, Setup, weebo-kura

### Community 8 - "steps"
Cohesion: 0.23
Nodes (12): steps, BASE, steps, steps, env, if, name, run (+4 more)

### Community 9 - "Duration"
Cohesion: 0.08
Nodes (14): digits(), Duration, Quantity, QuantityRepr, Bytes, Text, ScalarError, Duration (+6 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (29): extends, description, given, message, severity, then, overrides, files (+21 more)

### Community 11 - "auth/mod.rs"
Cohesion: 0.08
Nodes (15): algorithm(), AuthError, Invalid, Jwt, Provider, crate::problem::ApiError, LEEWAY_SECS, MAX_TOKEN_BYTES (+7 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.15
Nodes (25): parse_file(), binding_names_exactly_one_subject(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused(), defaults_are_on_the_closed_side(), documents_round_trip_through_yaml() (+17 more)

### Community 14 - "ChunkParams"
Cohesion: 0.14
Nodes (17): blob_of(), server_chunking_is_checked(), an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, ChunkParams (+9 more)

### Community 15 - "spec.rs"
Cohesion: 0.16
Nodes (18): ANNOTATIONS, Binding, ClaimMappings, ConfigCategory, Hotkeys, Plugins, Snippets, Themes (+10 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.10
Nodes (15): ApiDoc, audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), docs_csp(), healthz(), MAX_BODY_BYTES (+7 more)

### Community 17 - "with"
Cohesion: 0.08
Nodes (23): jobs, update, name, on, schedule, workflow_dispatch, permissions, contents (+15 more)

### Community 18 - "tasks"
Cohesion: 0.09
Nodes (22): cmds, desc, cmds, desc, preconditions, cmds, desc, msg (+14 more)

### Community 19 - "RFC 0002 — Batched chunk and manifest transfers"
Cohesion: 0.05
Nodes (41): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+33 more)

### Community 20 - "src/db.rs"
Cohesion: 0.16
Nodes (16): ACQUIRE_TIMEOUT, begin_subject(), begin_tenant(), begin_tenant_snapshot(), check_schema(), connect(), crate::problem::ApiError, DbError (+8 more)

### Community 21 - "fuzz"
Cohesion: 0.08
Nodes (23): ASAN_OPTIONS, cmds, desc, status, cmds, desc, env, requires (+15 more)

### Community 22 - "Doc / Gen / Guideline"
Cohesion: 0.10
Nodes (19): Backend, Code, Command, Command output, Devfile, Doc / Gen / Guideline, Dockerfile / Containerfile, Eclipse che - Cloud Development Environment (+11 more)

### Community 23 - "tasks"
Cohesion: 0.06
Nodes (36): cmds, desc, requires, cmds, desc, cmds, desc, cmds (+28 more)

### Community 24 - "vars"
Cohesion: 0.12
Nodes (16): silent, vars, AK_IMAGE, AK_PORT, NET, PG_CONTAINER, PG_IMAGE, PG_PORT (+8 more)

### Community 25 - "tasks"
Cohesion: 0.11
Nodes (18): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+10 more)

### Community 27 - ".json"
Cohesion: 0.22
Nodes (3): Api, DeviceView, TokenView

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "status.rs"
Cohesion: 0.11
Nodes (11): ReasonCode, .SUCCESS, categories_never_include_what_stays_on_the_device(), category(), plugin_of(), synced(), Removed, build() (+3 more)

### Community 30 - "kura-wasm/src/lib.rs"
Cohesion: 0.11
Nodes (30): bytes(), changes(), commit(), decode(), File, get_blob(), get_blobs(), get_chunk() (+22 more)

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "tasks"
Cohesion: 0.05
Nodes (36): cmd, for, var, cmds, desc, cmds, desc, requires (+28 more)

### Community 34 - "AppState"
Cohesion: 0.14
Nodes (4): MERGES, AppState, cors(), HttpSettings

### Community 35 - "history.rs"
Cohesion: 0.36
Nodes (8): a_pass_from_zero_pages_past_the_horizon(), eid(), epochs_change_once_with_the_cluster_identity(), history_and_restore(), purged_deletions_move_the_horizon(), retention_prunes_history_but_never_a_head(), S, setup()

### Community 36 - "6. Detailed design"
Cohesion: 0.15
Nodes (13): 6.10 Configuration reconciliation, 6.11 Engineering rules, 6.1 Data model, 6.2 Tenancy, authentication and authorization, 6.3 Sync protocol, 6.4 Chunking and storage, 6.5 Conflict resolution, 6.6 Encryption at rest (+5 more)

### Community 37 - "tasks"
Cohesion: 0.15
Nodes (12): task, cmds, desc, cmds, desc, cmds, desc, tasks (+4 more)

### Community 38 - "ApiError"
Cohesion: 0.12
Nodes (37): Body, access(), batch_blobs(), batch_chunks(), BATCH_FETCHES, batch_hashes(), BATCHES_IN_FLIGHT, BlobBatchRequest (+29 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.13
Nodes (15): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 15. Decisions and open questions, 16. Implementation phases, 1. Summary, 2. Motivation (+7 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 41 - "app/search.rs"
Cohesion: 0.14
Nodes (19): BATCH, cut(), index_one(), indexable(), markdown_only_cut_on_a_character(), MAX_INDEXED, MAX_QUERY, MAX_READ (+11 more)

### Community 42 - "Deserialize"
Cohesion: 0.36
Nodes (4): admin_status(), AdminStatus, with_quota_conditions(), ObjectStatus

### Community 43 - "helpers.js"
Cohesion: 0.17
Nodes (19): RFC-0001, BASE_URL, BLOB, SIZE, TOKEN, VAULT, CLIENT, headers() (+11 more)

### Community 44 - "Vault"
Cohesion: 0.11
Nodes (17): client_err(), DatabaseSecret, decode_b64(), default_region(), kv2_paths_split_into_mount_and_secret(), refresh_credentials(), secrets_never_print(), SERVICE_ACCOUNT_TOKEN (+9 more)

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "devices.rs"
Cohesion: 0.12
Nodes (15): Caller, needed_claims(), Device, device(), list_devices(), MAX_DEVICES, members(), Profile (+7 more)

### Community 47 - "types.ts"
Cohesion: 0.13
Nodes (18): RFC-9457, Controller, ControllerOptions, QUIET_MS, problem(), label(), Situation, word() (+10 more)

### Community 48 - "HostError"
Cohesion: 0.09
Nodes (9): err(), FsHost, mtime(), VaultPath, FileMeta, HostError, MemHost, HostAdapter (+1 more)

### Community 49 - "vaults.rs"
Cohesion: 0.17
Nodes (19): DeviceCaller, BindingView, ChunkingView, create_vault(), CreateVault, delete_binding(), delete_vault(), list_vaults() (+11 more)

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
Cohesion: 0.10
Nodes (38): Applied, Deferred, Done, apply(), BATCH, blob_of(), commit(), COMMIT_ATTEMPTS (+30 more)

### Community 55 - "token.rs"
Cohesion: 0.23
Nodes (7): hex_digit(), PREFIX, ScopedToken, SECRET_BYTES, TokenError, Malformed, tokens_round_trip_and_parse_strictly()

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (45): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+37 more)

### Community 57 - "ChunkHash"
Cohesion: 0.11
Nodes (7): ChunkHash, Remote, RemoteError, Problem, Scripted, hashes(), RemoteAdapter

### Community 58 - "path.rs"
Cohesion: 0.08
Nodes (26): allowed_char(), check(), ignorable(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX (+18 more)

### Community 59 - "kura-api"
Cohesion: 0.40
Nodes (6): kura-api, kura-cli, kura-core, kura-fuzz, kura-server, kura-wasm

### Community 64 - "Fail"
Cohesion: 0.23
Nodes (24): access_token(), attach(), config(), connect_and_watch(), Ctx, devices(), entry_of(), Fail (+16 more)

### Community 67 - "KuraTenant dev"
Cohesion: 0.05
Nodes (49): claimValue, role, claim, roles, categories, KuraTenant dev, KuraVault notes, KuraVault obsidian (+41 more)

### Community 69 - "package.json"
Cohesion: 0.08
Nodes (29): RFC-0001, description, devDependencies, esbuild@0.28.2, eslint@10.14.0, @eslint/js@10.0.1, obsidian@1.14.4, @types/node@26.6.6 (+21 more)

### Community 70 - "tasks"
Cohesion: 0.06
Nodes (32): cmds, desc, dir, cmds, deps, desc, dir, cmds (+24 more)

### Community 71 - "tests/sync.rs"
Cohesion: 0.28
Nodes (20): a_crowded_name_still_gets_a_unique_one(), a_path_freed_and_taken_in_one_commit(), a_pull_during_commits_never_passes_an_unseen_change(), blob(), changes_page_by_seq_and_refuse_a_lost_history(), commit_with_foreign_entry_id_is_refused(), concurrent_commits_never_skip_a_seq(), diverging_edits_become_a_conflict_file() (+12 more)

### Community 72 - ".fill"
Cohesion: 0.19
Nodes (4): download(), lock(), Prefetch, Prefetch<'a, R>

### Community 73 - "Notifier"
Cohesion: 0.10
Nodes (10): Notifier, NotifierOptions, POLL_MS, SILENCE_MS, SocketLike, Timers, setup(), ManualTimers (+2 more)

### Community 74 - "app/events.rs"
Cohesion: 0.07
Nodes (20): ACCESS_REVOKED, create_ticket(), DEVICE_REVOKED, events(), EventsQuery, Notice, Change, Ping (+12 more)

### Community 75 - "src/maintenance.rs"
Cohesion: 0.07
Nodes (39): BackupError, Busy, Db, LockLost, NoStorage, Objects, Other, check_chunks() (+31 more)

### Community 76 - "auth.ts"
Cohesion: 0.11
Nodes (25): RFC-8628, AuthError, AuthFailure, call(), discover(), Discovery, fnv(), Http (+17 more)

### Community 77 - "tests/maintenance.rs"
Cohesion: 0.22
Nodes (9): a_blob_put_and_collection_never_interleave(), backup_copies_new_objects_and_the_drill_check_catches_corruption(), collection_removes_only_what_nothing_references(), orphans_are_swept_and_the_backup_lock_pauses_deletion(), purge_keeps_objects_uploaded_after_its_rows_went(), restored_document_cancels_deletion(), S, setup() (+1 more)

### Community 78 - "auth/tests.rs"
Cohesion: 0.33
Nodes (16): a_retired_key_stops_verifying_after_max_age(), a_slow_fetch_does_not_hold_other_callers(), accepts_a_valid_access_token(), access_claims(), ISSUER, keys_and_userinfo_come_through_oidc_discovery(), missing_role_claims_come_from_userinfo_and_are_cached(), oidc() (+8 more)

### Community 79 - ".start"
Cohesion: 0.15
Nodes (8): Core, loadCore(), KuraPlugin, random16(), sleep(), Api, KuraSettingTab, device()

### Community 80 - "api.rs"
Cohesion: 0.15
Nodes (26): ATTEMPTS, Chunking, CLIENT, DeviceCode, discover(), Discovery, http(), MAX_CHUNK (+18 more)

### Community 81 - "http.rs"
Cohesion: 0.12
Nodes (17): problem(), vault_id(), a_binding_alone_links_the_device_to_its_tenant(), cors_allows_the_obsidian_origins_only_and_bodies_are_bounded(), device_links_follow_current_memberships(), device_ownership_and_limits(), foreign_vault_is_404(), idle_devices_are_purged_but_revoked_sessions_are_remembered() (+9 more)

### Community 82 - "parse.rs"
Cohesion: 0.11
Nodes (29): TenantSlug, ConfigError, parse_document(), Resource, Tenant, Vault, Source, typed() (+21 more)

### Community 83 - "commands.rs"
Cohesion: 0.09
Nodes (23): a_host_name_is_a_valid_device_name(), BACKOFF_MAX, BACKOFF_MIN, backoff_stays_between_one_and_sixty_seconds(), bytes_for_people(), code_of(), Diff, diff() (+15 more)

### Community 84 - "main.ts"
Cohesion: 0.12
Nodes (20): Tokens, DEFAULTS, readSettings(), registry(), Settings, timers, codeHashes(), Consent (+12 more)

### Community 85 - "tests/events.rs"
Cohesion: 0.26
Nodes (13): a_commit_reaches_the_socket(), a_notification_crosses_replicas(), a_socket_closes_when_its_caller_loses_the_vault(), a_ticket_flood_is_limited_per_address(), expired_ticket_is_refused(), next(), replica(), S (+5 more)

### Community 87 - "Hub"
Cohesion: 0.11
Nodes (16): CHANNEL, Event, Change, Pull, Revoked, four_sockets_per_device_at_most(), HEARTBEAT, Hub (+8 more)

### Community 88 - "reconcile.rs"
Cohesion: 0.27
Nodes (13): a_failed_tick_is_retried_as_a_local_change(), apply(), count_state(), invalid_document_keeps_last_config(), orphan_disables_and_delete_schedules(), reapplying_keeps_api_bindings_and_skips_unchanged_files(), restored_document_cancels_deletion(), stored_state_feeds_validation() (+5 more)

### Community 89 - "arc"
Cohesion: 0.25
Nodes (3): no_key_verifier(), http_get(), serves_api_and_metrics_then_drains_on_shutdown()

### Community 90 - "changes.rs"
Cohesion: 0.16
Nodes (14): ChangesResponse, ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE (+6 more)

### Community 91 - "oidc"
Cohesion: 0.10
Nodes (20): mount, role, listen, api, metrics, oidc, audiences, discovery (+12 more)

### Community 92 - "Uuid"
Cohesion: 0.05
Nodes (72): edits_of_different_lines_merge(), MAX_MERGE, MAX_MERGE_LINES, Merge, merge3(), Clean, Conflict, mergeable() (+64 more)

### Community 93 - "tests/tokens.rs"
Cohesion: 0.26
Nodes (9): a_token_is_revoked_with_the_binding_that_allowed_it(), INSIDE, json_of(), OUTSIDE, prefix_token_cannot_fetch_outside_chunk_or_read_renamed_history(), S, send(), setup() (+1 more)

### Community 94 - "compilerOptions"
Cohesion: 0.12
Nodes (20): client_addr(), client_addr_strict(), forwarding_headers_count_only_from_trusted_proxies(), headers(), the_strict_form_never_takes_the_proxy_for_the_client(), compilerOptions, exactOptionalPropertyTypes, isolatedModules (+12 more)

### Community 95 - "engine.rs"
Cohesion: 0.26
Nodes (26): sync(), a_batched_pull_makes_no_single_reads_and_drops_forged_records(), a_change_of_case_lands_on_a_case_insensitive_disk(), a_conflict_on_an_open_note_fails_that_file_only(), a_copied_folder_starts_fresh(), a_known_file_grown_past_the_limit_is_not_deleted(), a_merged_push_writes_the_merged_head(), a_move_killed_before_the_old_file_went_trashes_it_on_restart() (+18 more)

### Community 96 - "test_db"
Cohesion: 0.22
Nodes (13): test_db(), a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls(), jwks_survive_a_restart_without_the_provider(), pooled_connection_does_not_leak_tenant() (+5 more)

### Community 97 - "auth/keys.rs"
Cohesion: 0.13
Nodes (13): Cache, decode_set(), Discovery, Keys, KeySource, Fixed, Http, MAX_AGE (+5 more)

### Community 98 - "store/keys.rs"
Cohesion: 0.25
Nodes (5): Cache, CACHE_SIZE, CACHE_TTL, DataKeys, transit_key_name()

### Community 99 - "task"
Cohesion: 0.15
Nodes (16): cmd, for, task, cmds, cmds, desc, cmds, desc (+8 more)

### Community 100 - "tests/config.rs"
Cohesion: 0.36
Nodes (5): config_namespace_hidden_from_other_member(), configuration_writes_last_wins_without_conflict_files(), D, S, setup()

### Community 101 - "app/tests.rs"
Cohesion: 0.26
Nodes (9): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp(), state() (+1 more)

### Community 102 - "files.rs"
Cohesion: 0.22
Nodes (12): FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES, read_dir() (+4 more)

### Community 103 - "tasks"
Cohesion: 0.13
Nodes (15): desc, cmds, desc, cmds, desc, cmds, desc, cmds (+7 more)

### Community 104 - "serialize"
Cohesion: 0.14
Nodes (4): DnsLabel, NameError, String, Metadata

### Community 105 - "Command"
Cohesion: 0.09
Nodes (20): DeviceAction, Revoke, TokenAction, Create, List, Revoke, Cli, Command (+12 more)

### Community 106 - "tests/chunks.rs"
Cohesion: 0.17
Nodes (6): MAX_RETRIES, s3(), Storage, StorageError, TIMEOUT, BucketSecret

### Community 107 - "kura-server/src/main.rs"
Cohesion: 0.24
Nodes (13): backup(), check_chunks(), CONFIG_DIR_ENV, connect(), DATABASE_URL_ENV, dump_spec(), main(), maintain() (+5 more)

### Community 108 - "tests/merge.rs"
Cohesion: 0.15
Nodes (9): Body<T>, Raw, a_rename_on_one_side_and_edits_on_both_merge(), concurrent_line_edits_lose_nothing(), edits_of_different_lines_merge(), overlapping_and_non_text_edits_conflict(), rev(), S (+1 more)

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
Cohesion: 0.16
Nodes (12): DEVICE_HEADER, rate_limit(), require_device(), require_session(), bearer(), client(), KeyCaller, Reader (+4 more)

### Community 114 - "ObsidianHost"
Cohesion: 0.10
Nodes (13): Progress, HostOptions, ObsidianHost, parent(), toBuffer(), core, RFC-0002, Entry (+5 more)

### Community 115 - "cli.rs"
Cohesion: 0.32
Nodes (15): confirm(), a_long_absence_resyncs_and_history_restores(), a_rewound_server_gets_everything_back(), an_expired_access_token_is_renewed_mid_run(), code(), dir(), fake_issuer(), files() (+7 more)

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.11
Nodes (18): 10. The Obsidian plugin, 11. Local stack, 12. Maintenance, backups and restore, 13. Installing with Helm, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults (+10 more)

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 121 - "CryptoError"
Cohesion: 0.18
Nodes (13): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CryptoError, Corrupt, KeyLength, Malformed, Random, Unsupported (+5 more)

### Community 123 - "app"
Cohesion: 0.20
Nodes (7): app(), app_router(), openapi_spec(), SecurityAddon, Background, background(), serve()

### Community 124 - "Divergences"
Cohesion: 0.06
Nodes (36): 0. Before the first line of code, 1. Phase 1 — scaffold, configuration, identity, authorization, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Phase 4 — merge, notifications, plugin, 5. Phase 5 — history, retention, resync, 6. Phase 6 — operations, 7. Phase 7 — scoped tokens and search (+28 more)

### Community 125 - "remote.ts"
Cohesion: 0.14
Nodes (8): Sleep, ApiOptions, CLIENT, Fetch, retryAfter(), RFC-0002, VaultView, api()

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

### Community 130 - "scalar.rs"
Cohesion: 0.36
Nodes (5): API_DOCS_CSP, scalar(), SCALAR_BUNDLE_PATH, scalar_unavailable_html(), the_reference_loads_nothing_from_a_third_party()

### Community 131 - "ci.yml"
Cohesion: 0.25
Nodes (7): name, on, pull_request, push, permissions, contents, branches

### Community 132 - "manifest.json"
Cohesion: 0.25
Nodes (7): author, description, id, isDesktopOnly, minAppVersion, name, version

### Community 133 - "app/tokens.rs"
Cohesion: 0.19
Nodes (17): check_cidrs(), check_prefix(), create_token(), CreateToken, invalid(), IssuedToken, list_tokens(), manage() (+9 more)

### Community 135 - "AppRemote"
Cohesion: 0.13
Nodes (5): AppRemote, Device, devices_converge_and_lose_nothing(), Rng, run()

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

### Community 147 - "EntryId"
Cohesion: 0.08
Nodes (4): EntryId, Operation, Delete, Put

### Community 149 - "api"
Cohesion: 0.67
Nodes (3): flatten, taskfile, api

### Community 150 - "fs.rs"
Cohesion: 0.38
Nodes (7): a_255_byte_name_is_written(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp(), TRASH_DIR, writes_atomically_and_lists_notes_only()

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
Cohesion: 0.23
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

### Community 170 - "Documentation"
Cohesion: 0.50
Nodes (4): Documentation, Guides, Planned, RFCs

### Community 171 - "kura-cli/src/config.rs"
Cohesion: 0.22
Nodes (5): Attached, Config, Credentials, set_refresh_token(), write_private()

### Community 172 - "Chart.yaml"
Cohesion: 0.29
Nodes (6): apiVersion, appVersion, description, name, type, version

### Community 173 - ".object_key"
Cohesion: 0.22
Nodes (8): prefix, interval, updates, commit-message, directory, package-ecosystem, schedule, version

### Community 174 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 176 - "VaultRemote"
Cohesion: 0.20
Nodes (7): a_405_is_an_older_server_and_a_404_is_not(), body(), check(), exchange(), older_server(), username(), VaultRemote

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
Cohesion: 0.30
Nodes (17): blob_hash(), a_failed_commit_leaves_the_object_for_the_sweep(), batches_answer_in_order_within_their_budget(), bucket_object_is_not_plaintext(), commit_blob(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422(), objects() (+9 more)

### Community 182 - "image"
Cohesion: 0.40
Nodes (5): image, digest, pullPolicy, repository, tag

### Community 183 - "tests/search.rs"
Cohesion: 0.34
Nodes (5): Round, a_failing_index_job_is_kept_after_five_attempts(), S, search_is_opt_in_indexes_markdown_and_follows_edits(), setup()

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

### Community 194 - "android.sh"
Cohesion: 0.52
Nodes (6): adb(), ANDROID_HOME, booted(), sdk(), android.sh script, wait_boot()

### Community 203 - "authz.sh"
Cohesion: 0.30
Nodes (12): as(), batch_as(), call(), chunk_of(), cleanup(), dev_curl(), fail(), found() (+4 more)

### Community 208 - "dev:certs"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:certs

### Community 215 - "obsidian"
Cohesion: 0.67
Nodes (3): obsidian, flatten, taskfile

### Community 216 - "test"
Cohesion: 0.67
Nodes (3): test, flatten, taskfile

### Community 217 - "crypto.rs"
Cohesion: 0.15
Nodes (5): CODEC_NONE, FORMAT_V1, HEADER, OBJECT_KEY_CONTEXT, TAG

### Community 218 - "dev:certs:renew"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:certs:renew

### Community 240 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

## Knowledge Gaps
- **139 isolated node(s):** `Problem`, `Host`, `Remote`, `Corrupt`, `TooManyDeletions` (+134 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 1308 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **81 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ChunkHash` connect `ChunkHash` to `CryptoError`, `ApiError`, `AppRemote`, `.fill`, `tests/merge.rs`, `ChunkParams`, `api.rs`, `VaultRemote`, `commands.rs`, `EntryId`, `blob_hash`, `src/sync.rs`, `crypto.rs`, `Uuid`, `kura-wasm/src/lib.rs`, `engine.rs`?**
  _High betweenness centrality (0.040) - this node is a cross-community bridge._
- **Are the 6 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 6 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Problem`, `Host`, `Remote` to the rest of the system?**
  _139 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `authz.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.12682926829268293 - nodes in this community are weakly interconnected._
- **Why does `AppState` connect `AppState` to `authz.rs`, `reconcile/mod.rs`, `limits.rs`, `data_keys.rs`, `kura-server/src/config.rs`, `tests/chunks.rs`, `auth/mod.rs`, `src/maintenance.rs`, `kura-server/src/main.rs`, `app/mod.rs`, `metrics.rs`, `blob_hash`, `tests/events.rs`, `Hub`?**
  _High betweenness centrality (0.034) - this node is a cross-community bridge._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09672830725462304 - nodes in this community are weakly interconnected._
- **Why does `storage` connect `KuraTenant dev` to `data_keys.rs`?**
  _High betweenness centrality (0.033) - this node is a cross-community bridge._