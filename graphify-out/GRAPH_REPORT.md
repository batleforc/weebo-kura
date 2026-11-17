# Graph Report - weebo-kura  (2026-11-17)

## Corpus Check
- 314 files · ~285,645 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 27 file(s) not represented in the graph (top: (none) 8, .toml 7, .css 4)

## Summary
- 4632 nodes · 10223 edges · 267 communities (224 shown, 43 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 498 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `49e1b7d7`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- authz.rs
- reconcile/mod.rs
- common/mod.rs
- includes
- ProblemType
- Env
- kura-server/src/config.rs
- rfc0001-implementation-plan.md
- steps
- Duration
- rules
- app/notifications.rs
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
- super
- kura-wasm/src/lib.rs
- fuzz.yml
- tasks
- tasks
- AccessPage.vue
- history.rs
- 6. Detailed design
- tasks
- ApiError
- RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults
- tasks
- app/search.rs
- RFC 0004 — Scoped tokens: several folders, and write access
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
- src/maintenance.rs
- src/sync.rs
- token.rs
- attrs
- RemoteError
- path.rs
- kura-api
- .claude/CLAUDE.md
- end-of-line.sh
- restage.sh
- whitespace-fixer.sh
- Fail
- KuraVault obsidian
- sqlx-sqlite
- package.json
- tasks
- tests/sync.rs
- NotificationsPage.vue
- Notifier
- app/events.rs
- ui/package.json
- auth.ts
- notifier.rs
- scope.rs
- remote.ts
- api.rs
- http.rs
- kura-api/src/lib.rs
- commands.rs
- main.ts
- tests/events.rs
- auth/mod.rs
- Hub
- PathError
- tasks
- session.ts
- oidc
- ApiError
- tests/tokens.rs
- compilerOptions
- engine.rs
- tests/notifications.rs
- auth/keys.rs
- RFC 0003 — Web console (front and back office) and notifications
- task
- tests/config.rs
- app/tests.rs
- files.rs
- tasks
- auth/tests.rs
- Command
- store/mod.rs
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
- VaultRemote
- dev:env
- compilerOptions
- changes.rs
- BrowsePage.vue
- Divergences
- .start
- tests
- values.yaml
- tasks
- init-repo
- keys.sh
- ci.yml
- manifest.json
- app/tokens.rs
- test_db
- AppRemote
- jobs
- test
- s3
- lint
- SearchPage.vue
- postgres
- vault
- pnpm-workspace.yaml
- ui.rs
- deploy.yaml
- biome.json
- sync_one
- api
- DevicesPage.vue
- spec
- ref_vitest
- Algorithm
- wasm.d.ts
- AppState
- crypto.rs
- rules
- Env
- problem
- scripts
- @playwright/test
- vue
- rules
- limits.rs
- server
- vault
- notify.rs
- backup
- cnpg
- Documentation
- kura-cli/src/config.rs
- Chart.yaml
- 1. Phase 1 — scaffold, configuration, identity, authorization
- sim.rs
- ConfirmModal
- RFC 0005 — Folder-restricted viewers and the web reader
- ingress
- resources
- monitoring
- attach.sh
- blob_hash
- image
- tests/search.rs
- ui_src_api_generated_types_gen
- drill.sh
- run.sh
- deploy
- dev
- perf
- rust
- test_db
- dependencies
- wire.rs
- android.sh
- soak.sh
- reader.sh
- connect_and_watch
- bindings
- overrides
- client.ts
- withDevice
- authz.sh
- server
- coverage
- plugin
- console.sh
- dev:certs
- ui
- body.rs
- sw.js
- 4. User-facing design
- snapshot.rs
- RFC 0001 — implementation plan
- ScopeError
- test
- serve.rs
- dev:certs:renew
- serve
- src/merge.rs
- tenants.yaml
- .suffixed
- serve
- 6. Detailed design
- 7. Security
- db
- PluginModal
- networkPolicy
- vite.config.ts
- name
- CodeModal
- KuraTenant jdr
- KuraVault elsewhere
- MERGES
- states.js
- Notice
- 4. User-facing design
- 7. Security
- 4. User-facing design
- 5. Architecture
- 10. Operability
- 15. Decisions and open questions
- claimMappings
- Mem
- super
- notify.sh

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 165 edges
2. `SharedState` - 128 edges
3. `ChunkHash` - 113 edges
4. `RemoteError` - 105 edges
5. `VaultPath` - 75 edges
6. `HostError` - 51 edges
7. `Env` - 50 edges
8. `ProblemType` - 49 edges
9. `AppState` - 48 edges
10. `blob_hash()` - 48 edges

## Surprising Connections (you probably didn't know these)
- `6.P Proof against a real client` --references--> `open()`  [INFERRED]
  docs/RFC/rfc0002-batched-transfers.md → ui/src/pages/BrowsePage.vue
- `6.4 Notifications (migration `0023_notifications.sql`)` --references--> `run()`  [INFERRED]
  docs/RFC/rfc0003-web-console-and-notifications.md → ui/src/pages/SearchPage.vue
- `Progress` --references--> `PluginGuard`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/plugins.ts
- `Progress` --references--> `ProblemError`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/types.ts
- `Progress` --references--> `ConfirmModal`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/main.ts

## Import Cycles
- 1-file cycle: `crates/kura-server/src/app/commits.rs -> crates/kura-server/src/app/commits.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/tests/common/app.rs -> crates/kura-server/src/app/mod.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (267 total, 43 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.10
Nodes (30): TenantRole, Admin, Member, Owner, VaultRole, Editor, Manager, Viewer (+22 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.12
Nodes (21): ConfigFiles, bytes(), check(), json(), load_previous(), Outcome, Applied, NotWritten (+13 more)

### Community 2 - "common/mod.rs"
Cohesion: 0.09
Nodes (6): admin_url(), COUNTER, ISSUER, ROLE_PASSWORD, TestDb, TestKey

### Community 3 - "includes"
Cohesion: 0.20
Nodes (10): includes, mise, obsidian, security, skills, template, flatten, taskfile (+2 more)

### Community 4 - "ProblemType"
Cohesion: 0.07
Nodes (31): Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType (+23 more)

### Community 5 - "Env"
Cohesion: 0.11
Nodes (8): Captured, two_replicas_creating_one_data_key_keep_one(), vault(), admin(), config(), reads_database_and_bucket_credentials_from_kv(), refuses_a_bad_address_and_a_bad_token(), server_policy_cannot_delete_key()

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.11
Nodes (22): alg_none_cannot_be_configured(), CorsConfig, discovery_is_optional_and_https(), err(), EXAMPLE, issuer_must_be_https(), ListenConfig, MaintenanceConfig (+14 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.10
Nodes (14): Divergences, Progress, RFC 0002 — implementation plan, Divergences, Progress, RFC 0003 — implementation plan, Divergences, Progress (+6 more)

### Community 8 - "steps"
Cohesion: 0.23
Nodes (12): steps, BASE, steps, steps, env, if, name, run (+4 more)

### Community 9 - "Duration"
Cohesion: 0.09
Nodes (11): digits(), Duration, Quantity, QuantityRepr, Bytes, Text, ScalarError, Duration (+3 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (29): extends, description, given, message, severity, then, overrides, files (+21 more)

### Community 11 - "app/notifications.rs"
Cohesion: 0.15
Nodes (30): DeviceCaller, authorize(), Channel, check_authorization(), check_events(), check_name(), check_url(), create_rule() (+22 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.16
Nodes (26): parse_file(), binding_names_exactly_one_subject(), binding_paths_are_checked(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused(), defaults_are_on_the_closed_side() (+18 more)

### Community 14 - "ChunkParams"
Cohesion: 0.14
Nodes (13): an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, ChunkSpan, hashes_round_trip_in_lowercase_hex_only(), MAX_CHUNK (+5 more)

### Community 15 - "spec.rs"
Cohesion: 0.09
Nodes (19): DnsLabel, NameError, String, ANNOTATIONS, Binding, ClaimMappings, ConfigSync, DeletionPolicy (+11 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.08
Nodes (20): ApiDoc, app(), app_router(), audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), cors() (+12 more)

### Community 17 - "with"
Cohesion: 0.08
Nodes (23): jobs, update, name, on, schedule, workflow_dispatch, permissions, contents (+15 more)

### Community 18 - "tasks"
Cohesion: 0.09
Nodes (22): cmds, desc, cmds, desc, preconditions, cmds, desc, msg (+14 more)

### Community 19 - "RFC 0002 — Batched chunk and manifest transfers"
Cohesion: 0.07
Nodes (28): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+20 more)

### Community 20 - "src/db.rs"
Cohesion: 0.14
Nodes (16): ACQUIRE_TIMEOUT, begin_subject(), begin_tenant_snapshot(), check_schema(), connect(), crate::problem::ApiError, DbError, BinaryTooOld (+8 more)

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

### Community 26 - "MemAdapter"
Cohesion: 0.06
Nodes (9): Adapter, core, RFC-0002, Entry, FakeServer, json(), MemAdapter, RFC-0002 (+1 more)

### Community 27 - ".json"
Cohesion: 0.20
Nodes (5): Api, DeviceView, ScopeRule, TokenView, VaultView

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "super"
Cohesion: 0.23
Nodes (6): Removed, build(), observed_generation(), ready(), StoredStatus, widened()

### Community 30 - "kura-wasm/src/lib.rs"
Cohesion: 0.08
Nodes (34): bytes(), changes(), commit(), decode(), File, get_blob(), get_blobs(), get_chunk() (+26 more)

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "tasks"
Cohesion: 0.05
Nodes (36): cmd, for, var, cmds, desc, cmds, desc, requires (+28 more)

### Community 34 - "AccessPage.vue"
Cohesion: 0.12
Nodes (13): add(), adding, {
  data: bindings,
  loading,
  error,
  forbidden,
  load,
}, folders, groups, kind, refusal, role (+5 more)

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
Cohesion: 0.10
Nodes (39): Raw, batch_blobs(), batch_chunks(), BATCH_FETCHES, batch_hashes(), BATCHES_IN_FLIGHT, BlobBatchRequest, BlobBatchResponse (+31 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.17
Nodes (12): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 1. Summary, 2. Motivation, 3. Goals / non-goals (+4 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 41 - "app/search.rs"
Cohesion: 0.12
Nodes (18): BATCH, cut(), index_one(), indexable(), markdown_only_cut_on_a_character(), MAX_INDEXED, MAX_QUERY, MAX_READ (+10 more)

### Community 42 - "RFC 0004 — Scoped tokens: several folders, and write access"
Cohesion: 0.04
Nodes (46): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+38 more)

### Community 43 - "helpers.js"
Cohesion: 0.17
Nodes (19): BASE_URL, BLOB, SIZE, TOKEN, VAULT, CLIENT, headers(), pullAll() (+11 more)

### Community 44 - "Vault"
Cohesion: 0.11
Nodes (16): client_err(), decode_b64(), default_region(), kv2_paths_split_into_mount_and_secret(), refresh_credentials(), secrets_never_print(), SERVICE_ACCOUNT_TOKEN, split_kv2() (+8 more)

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "devices.rs"
Cohesion: 0.11
Nodes (20): Body, Caller, needed_claims(), Device, device_event(), device(), list_devices(), MAX_DEVICES (+12 more)

### Community 47 - "types.ts"
Cohesion: 0.13
Nodes (19): obsidian@1.14.4, Controller, ControllerOptions, QUIET_MS, label(), Situation, word(), count() (+11 more)

### Community 48 - "HostError"
Cohesion: 0.07
Nodes (17): a_255_byte_name_is_written(), err(), FsHost, mtime(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp() (+9 more)

### Community 49 - "vaults.rs"
Cohesion: 0.13
Nodes (27): SharedState, ReadAccess, token_denied(), BindingRow, BindingView, ChunkingView, create_vault(), CreateVault (+19 more)

### Community 50 - "metrics.rs"
Cohesion: 0.16
Nodes (6): Metrics, metrics_are_exported_with_the_kura_prefix(), refresh_storage(), render(), router(), storage_once()

### Community 51 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 52 - "devfile.yaml"
Cohesion: 0.33
Nodes (5): metadata, name, parent, uri, schemaVersion

### Community 53 - "src/maintenance.rs"
Cohesion: 0.05
Nodes (48): BackupError, Busy, Db, LockLost, NoStorage, Objects, Other, check_chunks() (+40 more)

### Community 54 - "src/sync.rs"
Cohesion: 0.08
Nodes (44): ChunkParams, chunks(), Applied, Deferred, Done, apply(), BATCH, blob_of() (+36 more)

### Community 55 - "token.rs"
Cohesion: 0.17
Nodes (7): hex_digit(), PREFIX, ScopedToken, SECRET_BYTES, TokenError, Malformed, tokens_round_trip_and_parse_strictly()

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (45): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+37 more)

### Community 57 - "RemoteError"
Cohesion: 0.09
Nodes (8): lock(), Manifest, Prefetch<'a, R>, Prefetch<'_, R>, Remote, RemoteError, Problem, Scripted

### Community 58 - "path.rs"
Cohesion: 0.14
Nodes (10): allowed_char(), check(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX, names_follow_the_rules() (+2 more)

### Community 59 - "kura-api"
Cohesion: 0.40
Nodes (6): kura-api, kura-cli, kura-core, kura-fuzz, kura-server, kura-wasm

### Community 64 - "Fail"
Cohesion: 0.44
Nodes (15): config(), Ctx, devices(), entry_of(), Fail, history(), restore(), search() (+7 more)

### Community 67 - "KuraVault obsidian"
Cohesion: 0.33
Nodes (6): categories, KuraVault obsidian, apiVersion, kind, spec, configSync

### Community 69 - "package.json"
Cohesion: 0.08
Nodes (28): RFC-0001, description, devDependencies, esbuild@0.28.2, eslint@10.14.0, @eslint/js@10.0.1, @types/node@26.6.6, typescript@6.0.3 (+20 more)

### Community 70 - "tasks"
Cohesion: 0.06
Nodes (32): cmds, desc, dir, cmds, deps, desc, dir, cmds (+24 more)

### Community 71 - "tests/sync.rs"
Cohesion: 0.28
Nodes (20): a_crowded_name_still_gets_a_unique_one(), a_path_freed_and_taken_in_one_commit(), a_pull_during_commits_never_passes_an_unseen_change(), blob(), changes_page_by_seq_and_refuse_a_lost_history(), commit_with_foreign_entry_id_is_refused(), concurrent_commits_never_skip_a_seq(), diverging_edits_become_a_conflict_file() (+12 more)

### Community 72 - "NotificationsPage.vue"
Cohesion: 0.07
Nodes (20): admin, authorization, creating, disabled, EVENTS, meStore, mine, name (+12 more)

### Community 73 - "Notifier"
Cohesion: 0.10
Nodes (10): Notifier, NotifierOptions, POLL_MS, SILENCE_MS, SocketLike, Timers, setup(), ManualTimers (+2 more)

### Community 74 - "app/events.rs"
Cohesion: 0.09
Nodes (13): ACCESS_REVOKED, create_ticket(), DEVICE_REVOKED, events(), EventsQuery, PING, SLOW_CONSUMER, Ticket (+5 more)

### Community 75 - "ui/package.json"
Cohesion: 0.09
Nodes (27): @biomejs/biome, @hey-api/openapi-ts, jsdom, tailwindcss, @types/markdown-it, vue-tsc, devDependencies, @biomejs/biome@^2.6.3 (+19 more)

### Community 76 - "auth.ts"
Cohesion: 0.11
Nodes (26): RFC-8628, AuthError, AuthFailure, call(), discover(), Discovery, fnv(), Http (+18 more)

### Community 77 - "notifier.rs"
Cohesion: 0.05
Nodes (39): a_target_is_https_without_credentials(), ip(), MAX_URL, private_and_special_addresses_are_refused(), refused_by_default(), Target, TargetError, AddressRefused (+31 more)

### Community 78 - "scope.rs"
Cohesion: 0.21
Nodes (7): a_scope_reads_and_writes_under_its_folders_only(), empty_scope_sees_nothing(), fold(), like_patterns_escape_wildcards(), MAX_SCOPES, PathScope, scope_of()

### Community 79 - "remote.ts"
Cohesion: 0.13
Nodes (9): Sleep, ApiOptions, CLIENT, Fetch, problem(), retryAfter(), RFC-0002, VaultView (+1 more)

### Community 80 - "api.rs"
Cohesion: 0.13
Nodes (27): a_405_is_an_older_server_and_a_404_is_not(), ATTEMPTS, Chunking, CLIENT, DeviceCode, discover(), Discovery, http() (+19 more)

### Community 81 - "http.rs"
Cohesion: 0.12
Nodes (28): problem(), vault_id(), a_web_device_never_gets_the_configuration_space(), bindings_need_manager(), me_shows_only_own_tenants(), the_console_is_served_with_its_headers(), web(), web_device_is_reused_per_sid() (+20 more)

### Community 82 - "kura-api/src/lib.rs"
Cohesion: 0.10
Nodes (32): TenantSlug, ConfigError, parse_document(), Resource, Tenant, Vault, Source, typed() (+24 more)

### Community 83 - "commands.rs"
Cohesion: 0.14
Nodes (13): a_host_name_is_a_valid_device_name(), BACKOFF_MAX, BACKOFF_MIN, backoff_stays_between_one_and_sixty_seconds(), blob_of(), Diff, diff(), host_name() (+5 more)

### Community 84 - "main.ts"
Cohesion: 0.11
Nodes (22): Tokens, DEFAULTS, readSettings(), registry(), Settings, sleep(), timers, RFC-0001 (+14 more)

### Community 85 - "tests/events.rs"
Cohesion: 0.26
Nodes (13): a_commit_reaches_the_socket(), a_notification_crosses_replicas(), a_socket_closes_when_its_caller_loses_the_vault(), a_ticket_flood_is_limited_per_address(), expired_ticket_is_refused(), next(), replica(), S (+5 more)

### Community 86 - "auth/mod.rs"
Cohesion: 0.08
Nodes (16): algorithm(), AuthError, Invalid, Jwt, Provider, crate::problem::ApiError, LEEWAY_SECS, MAX_TOKEN_BYTES (+8 more)

### Community 87 - "Hub"
Cohesion: 0.18
Nodes (10): Event, Change, Pull, Revoked, four_sockets_per_device_at_most(), Hub, payloads_become_events(), Seat (+2 more)

### Community 88 - "PathError"
Cohesion: 0.14
Nodes (13): nfc_is_required_on_the_wire_and_applied_locally(), PathError, Control, DotSegment, Empty, EmptySegment, Hidden, Ignorable (+5 more)

### Community 89 - "tasks"
Cohesion: 0.07
Nodes (26): tasks, ui:audit, ui:build, ui:client, ui:install, ui:lint, ui:test, cmds (+18 more)

### Community 90 - "session.ts"
Cohesion: 0.06
Nodes (51): RFC-6749, RFC-7636, pinia, installAuth(), discover(), forgetDiscovery(), load(), ProviderMetadata (+43 more)

### Community 91 - "oidc"
Cohesion: 0.07
Nodes (30): mount, role, listen, api, metrics, notifications, enabled, webhook (+22 more)

### Community 92 - "ApiError"
Cohesion: 0.09
Nodes (45): ChunkHash, OpResult, RestoreRequest, store_blob(), store_content(), apply(), apply_config(), BaseRev (+37 more)

### Community 93 - "tests/tokens.rs"
Cohesion: 0.19
Nodes (16): a_token_is_revoked_with_the_binding_that_allowed_it(), INSIDE, json_of(), multi_folder_token_reads_its_folders_only(), not_uploaded(), only_writer_routes_accept_tokens(), OUTSIDE, outside_folders() (+8 more)

### Community 94 - "compilerOptions"
Cohesion: 0.12
Nodes (15): compilerOptions, exactOptionalPropertyTypes, isolatedModules, lib, module, moduleResolution, noEmit, noFallthroughCasesInSwitch (+7 more)

### Community 95 - "engine.rs"
Cohesion: 0.24
Nodes (27): sync(), a_batched_pull_makes_no_single_reads_and_drops_forged_records(), a_change_of_case_lands_on_a_case_insensitive_disk(), a_conflict_on_an_open_note_fails_that_file_only(), a_copied_folder_starts_fresh(), a_known_file_grown_past_the_limit_is_not_deleted(), a_merged_push_writes_the_merged_head(), a_move_killed_before_the_old_file_went_trashes_it_on_restart() (+19 more)

### Community 96 - "tests/notifications.rs"
Cohesion: 0.22
Nodes (12): env_custom(), a_manager_sees_their_vaults_only_and_off_means_409(), a_rule_signs_its_deliveries_and_never_shows_its_secrets(), Admin, drain(), events_reach_their_rules_with_ids_only_and_retries_back_off(), Got, no_webhook_reaches_an_address_the_operator_did_not_allow() (+4 more)

### Community 97 - "auth/keys.rs"
Cohesion: 0.12
Nodes (13): Cache, decode_set(), Discovery, Keys, KeySource, Fixed, Http, MAX_AGE (+5 more)

### Community 98 - "RFC 0003 — Web console (front and back office) and notifications"
Cohesion: 0.05
Nodes (44): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+36 more)

### Community 99 - "task"
Cohesion: 0.15
Nodes (16): cmd, for, task, cmds, cmds, desc, cmds, desc (+8 more)

### Community 100 - "tests/config.rs"
Cohesion: 0.41
Nodes (5): config_namespace_hidden_from_other_member(), configuration_writes_last_wins_without_conflict_files(), D, S, setup()

### Community 101 - "app/tests.rs"
Cohesion: 0.26
Nodes (9): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp(), state() (+1 more)

### Community 102 - "files.rs"
Cohesion: 0.27
Nodes (12): FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES, read_dir() (+4 more)

### Community 103 - "tasks"
Cohesion: 0.13
Nodes (15): desc, cmds, desc, cmds, desc, cmds, desc, cmds (+7 more)

### Community 104 - "auth/tests.rs"
Cohesion: 0.29
Nodes (16): a_retired_key_stops_verifying_after_max_age(), a_slow_fetch_does_not_hold_other_callers(), accepts_a_valid_access_token(), access_claims(), ISSUER, keys_and_userinfo_come_through_oidc_discovery(), missing_role_claims_come_from_userinfo_and_are_cached(), oidc() (+8 more)

### Community 105 - "Command"
Cohesion: 0.09
Nodes (20): DeviceAction, Revoke, TokenAction, Create, List, Revoke, Cli, Command (+12 more)

### Community 106 - "store/mod.rs"
Cohesion: 0.13
Nodes (7): MAX_RETRIES, s3(), Storage, StorageError, TIMEOUT, BucketSecret, DatabaseSecret

### Community 107 - "kura-server/src/main.rs"
Cohesion: 0.21
Nodes (13): backup(), check_chunks(), CONFIG_DIR_ENV, connect(), DATABASE_URL_ENV, dump_spec(), main(), maintain() (+5 more)

### Community 108 - "tests/merge.rs"
Cohesion: 0.37
Nodes (7): a_rename_on_one_side_and_edits_on_both_merge(), concurrent_line_edits_lose_nothing(), edits_of_different_lines_merge(), overlapping_and_non_text_edits_conflict(), rev(), S, setup()

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
Cohesion: 0.11
Nodes (14): DEVICE_HEADER, rate_limit(), require_device(), require_session(), bearer(), client(), KeyCaller, NOT_UPLOADED (+6 more)

### Community 114 - "ObsidianHost"
Cohesion: 0.14
Nodes (7): Progress, HostOptions, ObsidianHost, parent(), toBuffer(), host(), copy()

### Community 115 - "cli.rs"
Cohesion: 0.42
Nodes (14): a_long_absence_resyncs_and_history_restores(), a_rewound_server_gets_everything_back(), a_scoped_token_syncs_its_folders_and_refuses_the_rest(), an_expired_access_token_is_renewed_mid_run(), code(), fake_issuer(), files(), sign_in_by_device_code_and_refresh_with_rotation() (+6 more)

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.10
Nodes (21): 10. The Obsidian plugin, 11. Local stack, 12. Maintenance, backups and restore, 13. Installing with Helm, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults (+13 more)

### Community 118 - "VaultRemote"
Cohesion: 0.15
Nodes (6): body(), check(), exchange(), older_server(), Token, VaultRemote

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 120 - "compilerOptions"
Cohesion: 0.09
Nodes (22): compilerOptions, allowImportingTsExtensions, forceConsistentCasingInFileNames, isolatedModules, jsx, lib, module, moduleResolution (+14 more)

### Community 121 - "changes.rs"
Cohesion: 0.18
Nodes (12): ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE, query() (+4 more)

### Community 123 - "BrowsePage.vue"
Cohesion: 0.05
Nodes (54): 7.4 Secrets and cryptography, doc, download, entries, entryId, error, loadAll(), loading (+46 more)

### Community 124 - "Divergences"
Cohesion: 0.13
Nodes (15): §8 additions (phase 2), §8 additions (phase 3), §8 additions (phase 5), §8 as built (phase 1), Divergences, Phase 1 eco-design review (2026-11-14), Phase 1 security review (2026-11-14), Phase 2 security review (2026-11-14) (+7 more)

### Community 125 - ".start"
Cohesion: 0.14
Nodes (7): Core, loadCore(), KuraPlugin, random16(), Api, KuraSettingTab, device()

### Community 126 - "tests"
Cohesion: 0.14
Nodes (13): alertname, eval_time, exp_alerts, evaluation_interval, exp_annotations, exp_labels, series, values (+5 more)

### Community 127 - "values.yaml"
Cohesion: 0.13
Nodes (14): backupsWaived, key, name, documents, migrate, databaseUrlSecret, podDisruptionBudget, minAvailable (+6 more)

### Community 128 - "tasks"
Cohesion: 0.22
Nodes (8): cmds, desc, tasks, init, update, cmds, desc, version

### Community 129 - "init-repo"
Cohesion: 0.22
Nodes (9): cmds, desc, preconditions, vars, msg, sh, init-repo, NAME (+1 more)

### Community 130 - "keys.sh"
Cohesion: 0.31
Nodes (12): bot(), cleanup(), fail(), gm(), head_seq(), key_curl(), pass(), refused() (+4 more)

### Community 131 - "ci.yml"
Cohesion: 0.25
Nodes (7): name, on, pull_request, push, permissions, contents, branches

### Community 132 - "manifest.json"
Cohesion: 0.25
Nodes (7): author, description, id, isDesktopOnly, minAppVersion, name, version

### Community 133 - "app/tokens.rs"
Cohesion: 0.13
Nodes (25): ScopeMode, Read, Write, ScopeRule, check_cidrs(), check_folder(), check_name(), check_prefix() (+17 more)

### Community 134 - "test_db"
Cohesion: 0.18
Nodes (16): no_key_verifier(), a_failed_tick_is_retried_as_a_local_change(), an_older_replica_never_overwrites_newer_state(), apply(), count_state(), DIRS, invalid_document_keeps_last_config(), orphan_disables_and_delete_schedules() (+8 more)

### Community 135 - "AppRemote"
Cohesion: 0.13
Nodes (4): AppRemote, devices_converge_and_lose_nothing(), Rng, run()

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

### Community 140 - "SearchPage.vue"
Cohesion: 0.18
Nodes (8): asked, disabled, error, hits, query, route, { t }, vaultId

### Community 141 - "postgres"
Cohesion: 0.33
Nodes (6): POSTGRES_PASSWORD, env, image, options, ports, postgres

### Community 142 - "vault"
Cohesion: 0.40
Nodes (5): BAO_DEV_ROOT_TOKEN_ID, vault, env, image, ports

### Community 143 - "pnpm-workspace.yaml"
Cohesion: 0.29
Nodes (6): allowBuilds, esbuild, auditConfig, ignoreGhsas, overrides, minimatch

### Community 144 - "ui.rs"
Cohesion: 0.16
Nodes (9): config(), ConsoleConfig, frame_hash(), headers(), origin(), router(), the_bridge_hash_is_read_strictly(), the_policy_names_the_identity_provider_only() (+1 more)

### Community 145 - "deploy.yaml"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, tasks, alerts:test, helm:lint, vars (+3 more)

### Community 146 - "biome.json"
Cohesion: 0.12
Nodes (16): css, parser, files, includes, formatter, indentStyle, linter, rules (+8 more)

### Community 148 - "sync_one"
Cohesion: 0.22
Nodes (11): attach(), bytes_for_people(), code_of(), confirm(), exit_codes_follow_section_4_5(), human(), params(), remote_entries() (+3 more)

### Community 149 - "api"
Cohesion: 0.67
Nodes (3): flatten, taskfile, api

### Community 150 - "DevicesPage.vue"
Cohesion: 0.11
Nodes (14): emit, { t }, confirming, devices, error, loading, router, session (+6 more)

### Community 151 - "spec"
Cohesion: 0.20
Nodes (10): KuraTenant dev, apiVersion, kind, metadata, spec, maxFileSize, storage, write (+2 more)

### Community 152 - "ref_vitest"
Cohesion: 0.08
Nodes (17): vue-i18n, vue-router, @vue/test-utils, @vue/test-utils@^2.5.1, i18n, localeOf(), failure, router (+9 more)

### Community 153 - "Algorithm"
Cohesion: 0.18
Nodes (11): Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512, RS256 (+3 more)

### Community 155 - "AppState"
Cohesion: 0.13
Nodes (5): AppState, HttpSettings, ConfigError, Invalid, Read

### Community 156 - "crypto.rs"
Cohesion: 0.06
Nodes (38): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, CryptoError, Corrupt, KeyLength, Malformed, Random (+30 more)

### Community 157 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 158 - "Env"
Cohesion: 0.34
Nodes (11): Env, env(), a_restricted_viewer_reads_its_folders_only(), apply(), config(), file(), folders_are_a_viewers_only(), no_route_shows_a_restricted_viewer_a_hidden_note() (+3 more)

### Community 159 - "problem"
Cohesion: 0.23
Nodes (8): admin_status(), AdminStatus, with_quota_conditions(), Me, me(), tenant_role(), TenantMembership, ObjectStatus

### Community 160 - "scripts"
Cohesion: 0.25
Nodes (8): scripts, build, dev, format, generate:client, lint, test, types:check

### Community 161 - "@playwright/test"
Cohesion: 0.24
Nodes (9): @playwright/test, RFC-0003, env(), signIn(), signOut(), RFC-0003, RFC-0003, RFC-0005 (+1 more)

### Community 162 - "vue"
Cohesion: 0.15
Nodes (11): vue, {
  data: status,
  loading,
  error,
  forbidden,
  load,
}, Row, rows, { t }, text(), emit, frame (+3 more)

### Community 163 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 164 - "limits.rs"
Cohesion: 0.23
Nodes (7): counts_per_key_and_route_and_says_when_to_retry(), limit_of(), LIMITS, MAX_TRACKED, RateLimiter, WINDOW, WindowKey

### Community 165 - "server"
Cohesion: 0.29
Nodes (7): audiences, discovery, issuer, platformAdmin, claim, value, oidc

### Community 166 - "vault"
Cohesion: 0.20
Nodes (10): mount, role, bucket, database, vault, mount, address, kubernetesAuth (+2 more)

### Community 167 - "notify.rs"
Cohesion: 0.15
Nodes (6): CHANNEL, HEARTBEAT, listen(), MAX_SOCKETS, notify_change(), QUEUE

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
Cohesion: 0.16
Nodes (8): access_token(), login(), Attached, Config, Credentials, refresh_token(), set_refresh_token(), write_private()

### Community 172 - "Chart.yaml"
Cohesion: 0.29
Nodes (6): apiVersion, appVersion, description, name, type, version

### Community 173 - "1. Phase 1 — scaffold, configuration, identity, authorization"
Cohesion: 0.18
Nodes (11): 1. Phase 1 — scaffold, configuration, identity, authorization, PR 1.10 — Close the phase, PR 1.1 — Repository scaffold, PR 1.2 — `kura-api`: documents, defaults, `validate()`, PR 1.3 — Server skeleton and API contract, PR 1.4 — Database, roles and row-level security, PR 1.5 — Reconciler and `config_state`, PR 1.6 — Token verification (+3 more)

### Community 174 - "sim.rs"
Cohesion: 0.23
Nodes (4): commit(), CommitRequest, CommitResponse, Device

### Community 176 - "RFC 0005 — Folder-restricted viewers and the web reader"
Cohesion: 0.08
Nodes (24): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+16 more)

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
Cohesion: 0.29
Nodes (17): blob_hash(), a_failed_commit_leaves_the_object_for_the_sweep(), batches_answer_in_order_within_their_budget(), bucket_object_is_not_plaintext(), commit_blob(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422(), objects() (+9 more)

### Community 182 - "image"
Cohesion: 0.40
Nodes (5): image, digest, pullPolicy, repository, tag

### Community 183 - "tests/search.rs"
Cohesion: 0.30
Nodes (5): Round, a_failing_index_job_is_kept_after_five_attempts(), S, search_is_opt_in_indexes_markdown_and_follows_edits(), setup()

### Community 184 - "ui_src_api_generated_types_gen"
Cohesion: 0.12
Nodes (14): anything(), Targets, targets(), RFC-0003, RFC-0003, useMe, managed, meStore (+6 more)

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

### Community 191 - "test_db"
Cohesion: 0.26
Nodes (12): test_db(), a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls(), jwks_survive_a_restart_without_the_provider(), pooled_connection_does_not_leak_tenant() (+4 more)

### Community 192 - "dependencies"
Cohesion: 0.15
Nodes (13): dompurify, @fontsource/jetbrains-mono, @fontsource/silkscreen, markdown-it, dependencies, dompurify@^3.4.18, @fontsource/jetbrains-mono@^5.3.0, @fontsource/silkscreen@^5.3.0 (+5 more)

### Community 193 - "wire.rs"
Cohesion: 0.08
Nodes (28): Author, ChangesResponse, commit_bodies_parse_strictly(), eid(), entry_ids_are_uuid_v7(), EntryId, EntryState, Operation (+20 more)

### Community 194 - "android.sh"
Cohesion: 0.52
Nodes (6): adb(), ANDROID_HOME, booted(), sdk(), android.sh script, wait_boot()

### Community 197 - "reader.sh"
Cohesion: 0.35
Nodes (9): api(), as(), cleanup(), entry(), fail(), hits(), pass(), reader.sh script (+1 more)

### Community 198 - "connect_and_watch"
Cohesion: 0.22
Nodes (10): connect_and_watch(), fingerprint(), on_socket(), Socketed, Reconnect, Stop, watch(), Watched (+2 more)

### Community 199 - "bindings"
Cohesion: 0.15
Nodes (15): claimValue, paths, role, KuraVault shared, KuraVault campaign, apiVersion, kind, spec (+7 more)

### Community 200 - "overrides"
Cohesion: 0.29
Nodes (6): auditConfig, ignoreGhsas, overrides, glob, js-yaml, minimatch

### Community 201 - "client.ts"
Cohesion: 0.09
Nodes (20): axios, axios@^1.20.0, browserName(), ensureDevice(), Retryable, signOut(), RFC-0003, KNOWN (+12 more)

### Community 202 - "withDevice"
Cohesion: 0.06
Nodes (42): withDevice(), messageOf(), problemType(), useLoad(), load(), remove(), load(), revoke() (+34 more)

### Community 203 - "authz.sh"
Cohesion: 0.30
Nodes (12): as(), batch_as(), call(), chunk_of(), cleanup(), dev_curl(), fail(), found() (+4 more)

### Community 204 - "server"
Cohesion: 0.17
Nodes (12): window, enabled, webhook, clientId, server, maintenance, notifications, trustedProxies (+4 more)

### Community 205 - "coverage"
Cohesion: 0.67
Nodes (3): flatten, taskfile, coverage

### Community 206 - "plugin"
Cohesion: 0.67
Nodes (3): plugin, flatten, taskfile

### Community 207 - "console.sh"
Cohesion: 0.27
Nodes (13): authorize(), c(), claim(), exchange(), fail(), field(), note(), pass() (+5 more)

### Community 208 - "dev:certs"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:certs

### Community 209 - "ui"
Cohesion: 0.67
Nodes (3): ui, flatten, taskfile

### Community 212 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 213 - "snapshot.rs"
Cohesion: 0.48
Nodes (3): load_if_changed(), REFRESH, run()

### Community 214 - "RFC 0001 — implementation plan"
Cohesion: 0.20
Nodes (10): 0. Before the first line of code, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Phase 4 — merge, notifications, plugin, 5. Phase 5 — history, retention, resync, 6. Phase 6 — operations, 7. Phase 7 — scoped tokens and search, 8. Phase 8 — Obsidian configuration (+2 more)

### Community 215 - "ScopeError"
Cohesion: 0.28
Nodes (9): check_folder(), check_rules(), ScopeError, Both, Count, NotAFolder, Path, Twice (+1 more)

### Community 216 - "test"
Cohesion: 0.67
Nodes (3): test, flatten, taskfile

### Community 218 - "dev:certs:renew"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:certs:renew

### Community 219 - "serve"
Cohesion: 0.52
Nodes (3): Background, background(), serve()

### Community 220 - "src/merge.rs"
Cohesion: 0.27
Nodes (10): edits_of_different_lines_merge(), MAX_MERGE, MAX_MERGE_LINES, Merge, merge3(), Clean, Conflict, mergeable() (+2 more)

### Community 221 - "tenants.yaml"
Cohesion: 0.25
Nodes (7): KuraVault notes, KuraTenant other, apiVersion, kind, metadata, apiVersion, kind

### Community 222 - ".suffixed"
Cohesion: 0.29
Nodes (3): ignorable(), suffixes_always_fit(), suffixes_stay_valid_and_bounded()

### Community 224 - "6. Detailed design"
Cohesion: 0.29
Nodes (7): 6.1 `kura-api`, 6.2 Data model (migration `0021_binding_paths.sql`), 6.3 `authz`, 6.4 Server routes that take a person but not a `Reader`, 6.5 The reader (`ui/`, front office of RFC 0003), 6. Detailed design, 6.P Proof against a real client

### Community 225 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 226 - "db"
Cohesion: 0.21
Nodes (9): ConfigCategory, Hotkeys, Plugins, Snippets, Themes, categories_and_the_kura_plugin(), category(), resolve() (+1 more)

### Community 228 - "networkPolicy"
Cohesion: 0.50
Nodes (4): networkPolicy, enabled, ingressNamespace, monitoringNamespace

### Community 229 - "vite.config.ts"
Cohesion: 0.17
Nodes (8): @tailwindcss/vite, vite, @vitejs/plugin-vue, @tailwindcss/vite@^4.3.5, vite@^8.3.8, @vitejs/plugin-vue@^6.0.10, RFC-0003, RFC-0005

### Community 230 - "name"
Cohesion: 0.48
Nodes (7): metadata, metadata, metadata, metadata, metadata, name, namespace

### Community 232 - "KuraTenant jdr"
Cohesion: 0.50
Nodes (4): KuraTenant jdr, apiVersion, kind, metadata

### Community 233 - "KuraVault elsewhere"
Cohesion: 0.50
Nodes (4): KuraVault elsewhere, apiVersion, kind, spec

### Community 236 - "Notice"
Cohesion: 0.33
Nodes (5): Notice, Change, Ping, Pull, text()

### Community 238 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 239 - "7. Security"
Cohesion: 0.33
Nodes (6): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 240 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 241 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

### Community 242 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 243 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 250 - "claimMappings"
Cohesion: 0.20
Nodes (11): claim, roles, spec, spec, admin, member, owner, enabled (+3 more)

### Community 259 - "super"
Cohesion: 0.12
Nodes (11): ReasonCode, .SUCCESS, categories_never_include_what_stays_on_the_device(), category(), plugin_of(), synced(), API_DOCS_CSP, scalar() (+3 more)

### Community 263 - "notify.sh"
Cohesion: 0.50
Nodes (7): api(), cleanup(), fail(), pass(), serve(), notify.sh script, sql()

## Knowledge Gaps
- **185 isolated node(s):** `Denied`, `InvalidGrant`, `Problem`, `Host`, `Remote` (+180 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 1773 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **43 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ChunkHash` connect `ApiError` to `Env`, `AppRemote`, `ChunkParams`, `crypto.rs`, `Env`, `kura-wasm/src/lib.rs`, `history.rs`, `ApiError`, `app/search.rs`, `sim.rs`, `blob_hash`, `src/sync.rs`, `src/maintenance.rs`, `tests/search.rs`, `RemoteError`, `wire.rs`, `tests/sync.rs`, `api.rs`, `commands.rs`, `tests/events.rs`, `engine.rs`, `tests/config.rs`, `tests/merge.rs`, `VaultRemote`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **Are the 6 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 6 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Denied`, `InvalidGrant`, `Problem` to the rest of the system?**
  _185 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `authz.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10459183673469388 - nodes in this community are weakly interconnected._
- **Why does `ApiError` connect `ApiError` to `authz.rs`, `db`, `ProblemType`, `app/tokens.rs`, `ApiError`, `app/search.rs`, `app/events.rs`, `app/notifications.rs`, `notifier.rs`, `devices.rs`, `app/mod.rs`, `reader.rs`, `body.rs`, `vaults.rs`, `changes.rs`, `crypto.rs`, `problem`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.11954022988505747 - nodes in this community are weakly interconnected._
- **Why does `SharedState` connect `vaults.rs` to `authz.rs`, `app/tokens.rs`, `app/notifications.rs`, `app/mod.rs`, `ui.rs`, `problem`, `ApiError`, `app/search.rs`, `devices.rs`, `src/maintenance.rs`, `app/events.rs`, `notifier.rs`, `serve`, `ApiError`, `serve`, `db`, `app/tests.rs`, `reader.rs`, `changes.rs`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._