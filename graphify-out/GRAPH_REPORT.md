# Graph Report - weebo-kura  (2026-11-19)

## Corpus Check
- 388 files · ~360,655 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 29 file(s) not represented in the graph (top: (none) 10, .toml 7, .css 4)

## Summary
- 5471 nodes · 12158 edges · 296 communities (258 shown, 38 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 598 edges (avg confidence: 0.82)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `80cf4666`
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
- RemoteError
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
- KuraPlugin
- .json
- tasks
- super
- kura-wasm/src/lib.rs
- fuzz.yml
- tasks
- tasks
- NotificationsPage.vue
- tests/notifications.rs
- 6. Detailed design
- tasks
- ApiError
- RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults
- tasks
- app/search.rs
- RFC 0004 — Scoped tokens: several folders, and write access
- helpers.js
- HomePage.vue
- openapi_contract.rs
- SharedState
- ConfigPage.vue
- KeysPage.vue
- vaults.rs
- metrics.rs
- 7. Security
- RFC 0007 — Folders as synced entries
- src/maintenance.rs
- src/sync.rs
- KeysPage.vue
- attrs
- ref_vitest
- path.rs
- kura-api
- .claude/CLAUDE.md
- end-of-line.sh
- restage.sh
- whitespace-fixer.sh
- Fail
- ui.rs
- sqlx-sqlite
- package.json
- tasks
- chunk.rs
- auth/mod.rs
- Notifier
- app/events.rs
- admin.rs
- auth.ts
- src/vault.rs
- scope.rs
- ObsidianHost
- api.rs
- http.rs
- kura-api/src/lib.rs
- commands.rs
- PluginGuard
- AppRemote
- withDevice
- devDependencies
- tests/maintenance.rs
- tasks
- RFC 0008 — The synced Obsidian configuration in the console
- oidc
- ApiError
- tests/tokens.rs
- compilerOptions
- engine.rs
- tests/events.rs
- auth/keys.rs
- RFC 0003 — Web console (front and back office) and notifications
- task
- tests/config.rs
- app/tests.rs
- files.rs
- tasks
- auth/tests.rs
- Command
- HelpPage.vue
- kura-server/src/main.rs
- tests/merge.rs
- env
- approve.sh script
- env
- sync.sh
- tests/sync.rs
- ObsidianHost
- cli.rs
- dev:network
- Operating kura-server (phases 1 and 2)
- backup.rs
- dev:env
- compilerOptions
- app/sync_state.rs
- BrowsePage.vue
- Divergences
- build-checks.ts
- tests
- values.yaml
- tasks
- init-repo
- keys.sh
- ci.yml
- manifest.json
- app/tokens.rs
- test_db
- sim.rs
- jobs
- test
- s3
- lint
- changes.rs
- postgres
- vault
- pnpm-workspace.yaml
- KeysPage.vue
- deploy.yaml
- biome.json
- PrefsMenu.vue
- reader.rs
- api
- notifier.rs
- tenants.yaml
- RFC 0006 — Sync state per device and per vault
- Algorithm
- wasm.d.ts
- src/vault.rs
- crypto.rs
- rules
- client.ts
- scripts
- @playwright/test
- 6. Detailed design
- rules
- net.rs
- server
- vault
- .start
- backup
- cnpg
- Documentation
- kura-cli/src/config.rs
- Chart.yaml
- Env
- limits.rs
- VaultPath
- RFC 0005 — Folder-restricted viewers and the web reader
- ingress
- resources
- monitoring
- attach.sh
- blob_hash
- image
- tests/search.rs
- remote.ts
- drill.sh
- run.sh
- deploy
- dev
- perf
- rust
- test_db
- history.rs
- wire.rs
- android.sh
- soak.sh
- reader.sh
- Env
- bindings
- overrides
- deserialize
- sync_one
- authz.sh
- server
- coverage
- plugin
- console.sh
- dev:certs
- ui
- body.rs
- sw.js
- connect_and_watch
- AppState
- types.ts
- PaneResizer.vue
- test
- 6. Detailed design
- dev:certs:renew
- 18. Revision against the tree (2026-11-18)
- token.rs
- KuraTenant other
- notify.rs
- PathError
- duration
- config-view.sh
- kura-core/src/config.rs
- networkPolicy
- 18. Revision against the tree (2026-11-17, gaps still open)
- name
- 7. Security
- KuraTenant jdr
- KuraVault elsewhere
- devices.sh
- states.js
- record
- 18. Revision against the tree (2026-11-17)
- 7. Security
- 6. Detailed design
- updates
- modals.ts
- .json
- dependencies
- 7. Security
- 4. User-facing design
- 4. User-facing design
- 4. User-facing design
- 2026-11-17T22-55-29Z__ui-src-pages.md
- serve
- claimMappings
- .fill
- store/keys.rs
- src/merge.rs
- 2026-11-18T20-15-32Z__ui-src-pages.md
- deliver
- path
- Mem
- 10. Operability
- 5. Architecture
- notify.sh
- hourly
- pkce.ts
- ScopeError
- devfile.yaml
- scalar.rs
- 4. User-facing design
- next
- 6. Detailed design
- 7. Security
- fuzz_config_document.rs
- KuraVault suite-config
- FakeSocket
- RFC 0004 — implementation plan
- 10. Operability
- 5. Architecture
- 15. Decisions and open questions
- 18. Revision against the tree (2026-11-17)
- 4. User-facing design
- 10. Operability
- 15. Decisions and open questions
- 5. Architecture

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 171 edges
2. `SharedState` - 133 edges
3. `ChunkHash` - 117 edges
4. `RemoteError` - 106 edges
5. `VaultPath` - 82 edges
6. `Env` - 63 edges
7. `withDevice()` - 61 edges
8. `HostError` - 61 edges
9. `blob_hash()` - 52 edges
10. `ProblemType` - 50 edges

## Surprising Connections (you probably didn't know these)
- `17. References and sources` --references--> `Report`  [INFERRED]
  docs/RFC/rfc0006-device-sync-state.md → plugin/src/types.ts
- `2. Motivation` --references--> `Report`  [INFERRED]
  docs/RFC/rfc0006-device-sync-state.md → plugin/src/types.ts
- `5.1 The pull window` --references--> `apply()`  [INFERRED]
  docs/RFC/rfc0002-batched-transfers.md → ui/src/prefs.ts
- `6.2 `kura-core`` --references--> `apply()`  [INFERRED]
  docs/RFC/rfc0002-batched-transfers.md → ui/src/prefs.ts
- `18.2 Divergences from the text` --references--> `fileBytes()`  [INFERRED]
  docs/RFC/rfc0008-console-config-view.md → ui/src/reader/content.ts

## Import Cycles
- 1-file cycle: `crates/kura-server/src/app/commits.rs -> crates/kura-server/src/app/commits.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/tests/common/app.rs -> crates/kura-server/src/app/mod.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (296 total, 38 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.10
Nodes (35): TenantRole, Admin, Member, Owner, VaultRole, Editor, Manager, Viewer (+27 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.13
Nodes (19): bytes(), json(), load_previous(), Outcome, Applied, NotWritten, Rejected, policy() (+11 more)

### Community 2 - "common/mod.rs"
Cohesion: 0.09
Nodes (6): admin_url(), COUNTER, ISSUER, ROLE_PASSWORD, TestDb, TestKey

### Community 3 - "includes"
Cohesion: 0.20
Nodes (10): includes, mise, obsidian, security, skills, template, flatten, taskfile (+2 more)

### Community 4 - "ProblemType"
Cohesion: 0.06
Nodes (31): Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType (+23 more)

### Community 5 - "Env"
Cohesion: 0.11
Nodes (10): Captured, env_custom(), two_replicas_creating_one_data_key_keep_one(), vault(), rule_changes_are_audited(), admin(), config(), reads_database_and_bucket_credentials_from_kv() (+2 more)

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.09
Nodes (26): alg_none_cannot_be_configured(), ClaimMatch, CorsConfig, default_plugin_client(), discovery_is_optional_and_https(), err(), EXAMPLE, issuer_must_be_https() (+18 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.10
Nodes (11): Divergences, Progress, RFC 0002 — implementation plan, Divergences, Progress, RFC 0003 — implementation plan, Divergences, Progress (+3 more)

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
Cohesion: 0.14
Nodes (30): authorization_rejects_control_characters(), authorize(), Channel, check_authorization(), check_events(), check_name(), check_url(), create_rule() (+22 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.17
Nodes (27): parse_file(), validate(), binding_names_exactly_one_subject(), binding_paths_are_checked(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused() (+19 more)

### Community 14 - "RemoteError"
Cohesion: 0.09
Nodes (8): Manifest, Remote, RemoteError, Problem, Scripted, decode(), hashes(), RemoteAdapter

### Community 15 - "spec.rs"
Cohesion: 0.13
Nodes (22): ANNOTATIONS, Binding, Chunking, .DEFAULT, .MAX_CHUNK, ClaimMappings, ConfigCategory, Hotkeys (+14 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.09
Nodes (18): ApiDoc, app_router(), audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), docs_csp(), healthz() (+10 more)

### Community 17 - "with"
Cohesion: 0.08
Nodes (23): jobs, update, name, on, schedule, workflow_dispatch, permissions, contents (+15 more)

### Community 18 - "tasks"
Cohesion: 0.09
Nodes (22): cmds, desc, cmds, desc, preconditions, cmds, desc, msg (+14 more)

### Community 19 - "RFC 0002 — Batched chunk and manifest transfers"
Cohesion: 0.04
Nodes (48): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+40 more)

### Community 20 - "src/db.rs"
Cohesion: 0.13
Nodes (15): ACQUIRE_TIMEOUT, begin_tenant_snapshot(), check_schema(), connect(), crate::problem::ApiError, DbError, BinaryTooOld, Migrate (+7 more)

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

### Community 26 - "KuraPlugin"
Cohesion: 0.11
Nodes (20): ago(), en, fr, Key, roleName(), setLanguage(), t(), consoleUrl() (+12 more)

### Community 27 - ".json"
Cohesion: 0.18
Nodes (5): Api, DeviceView, ScopeRule, TokenView, VaultView

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "super"
Cohesion: 0.16
Nodes (8): ReasonCode, .SUCCESS, Removed, build(), observed_generation(), ready(), StoredStatus, widened()

### Community 30 - "kura-wasm/src/lib.rs"
Cohesion: 0.11
Nodes (32): bytes(), changes(), commit(), File, folders(), get_blob(), get_blobs(), get_chunk() (+24 more)

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "tasks"
Cohesion: 0.05
Nodes (43): cmd, for, var, cmds, desc, cmds, desc, requires (+35 more)

### Community 34 - "NotificationsPage.vue"
Cohesion: 0.05
Nodes (42): admin, askRotate(), authorization, cancelRotate(), closeForm(), closeSecret(), confirmBox, copied (+34 more)

### Community 35 - "tests/notifications.rs"
Cohesion: 0.15
Nodes (17): a_manager_sees_their_vaults_only_and_off_means_409(), a_rule_signs_its_deliveries_and_never_shows_its_secrets(), Admin, delivery_result_is_coarse_and_body_unread(), drain(), events_reach_their_rules_with_ids_only_and_retries_back_off(), Got, no_webhook_reaches_an_address_the_operator_did_not_allow() (+9 more)

### Community 36 - "6. Detailed design"
Cohesion: 0.15
Nodes (13): 6.10 Configuration reconciliation, 6.11 Engineering rules, 6.1 Data model, 6.2 Tenancy, authentication and authorization, 6.3 Sync protocol, 6.4 Chunking and storage, 6.5 Conflict resolution, 6.6 Encryption at rest (+5 more)

### Community 37 - "tasks"
Cohesion: 0.15
Nodes (12): task, cmds, desc, cmds, desc, cmds, desc, tasks (+4 more)

### Community 38 - "ApiError"
Cohesion: 0.09
Nodes (45): ChunkHash, Body, batch_blobs(), batch_chunks(), BATCH_FETCHES, batch_hashes(), BATCHES_IN_FLIGHT, BlobBatchRequest (+37 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.08
Nodes (24): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+16 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 41 - "app/search.rs"
Cohesion: 0.13
Nodes (18): BATCH, cut(), index_one(), indexable(), markdown_only_cut_on_a_character(), MAX_INDEXED, MAX_QUERY, MAX_READ (+10 more)

### Community 42 - "RFC 0004 — Scoped tokens: several folders, and write access"
Cohesion: 0.15
Nodes (13): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 17. References and sources, 1. Summary, 2. Motivation (+5 more)

### Community 43 - "helpers.js"
Cohesion: 0.16
Nodes (22): BASE_URL, BLOB, SIZE, TOKEN, VAULT, CLIENT, headers(), pullAll() (+14 more)

### Community 44 - "HomePage.vue"
Cohesion: 0.08
Nodes (26): anything(), Targets, targets(), RFC-0003, canRetry, devices, error, expiring (+18 more)

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "SharedState"
Cohesion: 0.09
Nodes (24): Caller, needed_claims(), Device, device_event(), device(), DeviceVault, list_devices(), MAX_DEVICES (+16 more)

### Community 47 - "ConfigPage.vue"
Cohesion: 0.07
Nodes (41): anyHiddenSettings, applies, canRetry, edited, entries, error, file, groups (+33 more)

### Community 48 - "KeysPage.vue"
Cohesion: 0.04
Nodes (33): matching(), cidrs, confirming, copied, creating, {
  data: tokens,
  loading,
  error,
  forbidden,
  forbiddenDetail,
  canRetry,
  load,
}, days, meStore (+25 more)

### Community 49 - "vaults.rs"
Cohesion: 0.10
Nodes (28): DEVICE_HEADER, DeviceCaller, rate_limit(), require_device(), require_session(), BindingRow, BindingView, ChunkingView (+20 more)

### Community 50 - "metrics.rs"
Cohesion: 0.14
Nodes (6): Metrics, metrics_are_exported_with_the_kura_prefix(), refresh_storage(), render(), router(), storage_once()

### Community 51 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 52 - "RFC 0007 — Folders as synced entries"
Cohesion: 0.05
Nodes (43): 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 15. Decisions and open questions, 16. Implementation phases, 17. References and sources (+35 more)

### Community 53 - "src/maintenance.rs"
Cohesion: 0.11
Nodes (26): BACKUP_LOCK, BATCH, Batches, collect_chunks(), delete_objects(), DELETED_ENTRY_TTL_HOURS, lock_held(), MaintenanceError (+18 more)

### Community 54 - "src/sync.rs"
Cohesion: 0.10
Nodes (45): a_refused_push_counts_what_it_kept(), Applied, Deferred, Done, apply(), apply_folder(), BATCH, commit() (+37 more)

### Community 55 - "KeysPage.vue"
Cohesion: 0.09
Nodes (21): allRevoked, error, loading, marks, person(), props, query, revokedCount (+13 more)

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (45): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+37 more)

### Community 57 - "ref_vitest"
Cohesion: 0.05
Nodes (58): RFC-0003, axios, pinia, vue-router, @vue/test-utils, installAuth(), discover(), forgetDiscovery() (+50 more)

### Community 58 - "path.rs"
Cohesion: 0.11
Nodes (13): allowed_char(), check(), ignorable(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX (+5 more)

### Community 59 - "kura-api"
Cohesion: 0.40
Nodes (6): kura-api, kura-cli, kura-core, kura-fuzz, kura-server, kura-wasm

### Community 64 - "Fail"
Cohesion: 0.40
Nodes (15): config(), Ctx, devices(), entry_of(), Fail, history(), restore(), scoped() (+7 more)

### Community 67 - "ui.rs"
Cohesion: 0.14
Nodes (12): check_dir(), config(), ConsoleConfig, frame_hash(), headers(), origin(), PluginClient, the_bridge_hash_is_read_strictly() (+4 more)

### Community 69 - "package.json"
Cohesion: 0.08
Nodes (29): RFC-0001, description, devDependencies, esbuild@0.28.2, eslint@10.14.0, @eslint/js@10.0.1, obsidian@1.14.4, @types/node@26.6.6 (+21 more)

### Community 70 - "tasks"
Cohesion: 0.06
Nodes (32): cmds, desc, dir, cmds, deps, desc, dir, cmds (+24 more)

### Community 71 - "chunk.rs"
Cohesion: 0.13
Nodes (17): an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, ChunkParams, chunks(), ChunkSpan (+9 more)

### Community 72 - "auth/mod.rs"
Cohesion: 0.08
Nodes (15): algorithm(), AuthError, Invalid, Jwt, Provider, crate::problem::ApiError, LEEWAY_SECS, MAX_TOKEN_BYTES (+7 more)

### Community 73 - "Notifier"
Cohesion: 0.14
Nodes (8): Notifier, NotifierOptions, POLL_MS, SILENCE_MS, SocketLike, Timers, FakeSocket, setup()

### Community 74 - "app/events.rs"
Cohesion: 0.07
Nodes (20): ACCESS_REVOKED, create_ticket(), DEVICE_REVOKED, events(), EventsQuery, Notice, Change, Ping (+12 more)

### Community 75 - "admin.rs"
Cohesion: 0.57
Nodes (4): admin_status(), AdminStatus, with_quota_conditions(), ObjectStatus

### Community 76 - "auth.ts"
Cohesion: 0.11
Nodes (24): RFC-8628, AuthError, AuthFailure, call(), discover(), Discovery, fnv(), Http (+16 more)

### Community 77 - "src/vault.rs"
Cohesion: 0.16
Nodes (9): client_err(), decode_b64(), refresh_credentials(), Vault, VaultError, Address, Client, Path (+1 more)

### Community 78 - "scope.rs"
Cohesion: 0.16
Nodes (12): a_scope_reads_and_writes_under_its_folders_only(), empty_scope_sees_nothing(), fold(), like_patterns_escape_wildcards(), MAX_SCOPES, PathScope, scope_of(), ScopeMode (+4 more)

### Community 79 - "ObsidianHost"
Cohesion: 0.09
Nodes (13): RFC-0002, HostOptions, ObsidianHost, parent(), toBuffer(), RFC-0007, core, RFC-0002 (+5 more)

### Community 80 - "api.rs"
Cohesion: 0.11
Nodes (30): a_405_is_an_older_server_and_a_404_is_not(), ATTEMPTS, check(), Chunking, CLIENT, DeviceCode, discover(), Discovery (+22 more)

### Community 81 - "http.rs"
Cohesion: 0.12
Nodes (30): problem(), vault_id(), a_web_device_never_gets_the_configuration_space(), bindings_need_manager(), me_shows_only_own_tenants(), the_console_is_served_with_its_headers(), web(), web_device_is_reused_per_sid() (+22 more)

### Community 82 - "kura-api/src/lib.rs"
Cohesion: 0.11
Nodes (28): TenantSlug, ConfigError, parse_document(), Resource, Tenant, Vault, Source, typed() (+20 more)

### Community 83 - "commands.rs"
Cohesion: 0.14
Nodes (13): a_host_name_is_a_valid_device_name(), BACKOFF_MAX, BACKOFF_MIN, backoff_stays_between_one_and_sixty_seconds(), blob_of(), Diff, diff(), host_name() (+5 more)

### Community 84 - "PluginGuard"
Cohesion: 0.10
Nodes (21): Progress, Tokens, readSettings(), registry(), Settings, codeHashes(), Consent, emptyConsent() (+13 more)

### Community 86 - "withDevice"
Cohesion: 0.05
Nodes (64): withDevice(), detailOf(), KNOWN, messageOf(), problemType(), retryable(), RFC-9457, load() (+56 more)

### Community 87 - "devDependencies"
Cohesion: 0.07
Nodes (34): @biomejs/biome, @fontsource/jetbrains-mono, @fontsource/silkscreen, @hey-api/openapi-ts, jsdom, tailwindcss, @tailwindcss/vite, @vitejs/plugin-vue (+26 more)

### Community 88 - "tests/maintenance.rs"
Cohesion: 0.26
Nodes (9): a_blob_put_and_collection_never_interleave(), backup_copies_new_objects_and_the_drill_check_catches_corruption(), collection_removes_only_what_nothing_references(), orphans_are_swept_and_the_backup_lock_pauses_deletion(), purge_keeps_objects_uploaded_after_its_rows_went(), restored_document_cancels_deletion(), S, setup() (+1 more)

### Community 89 - "tasks"
Cohesion: 0.07
Nodes (26): tasks, ui:audit, ui:build, ui:client, ui:install, ui:lint, ui:test, cmds (+18 more)

### Community 90 - "RFC 0008 — The synced Obsidian configuration in the console"
Cohesion: 0.05
Nodes (37): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 15. Decisions and open questions (+29 more)

### Community 91 - "oidc"
Cohesion: 0.06
Nodes (33): mount, role, listen, api, metrics, notifications, enabled, webhook (+25 more)

### Community 92 - "ApiError"
Cohesion: 0.10
Nodes (45): OpResult, RestoreRequest, RevisionsResponse, apply(), apply_config(), BaseRev, check_blobs(), Commit (+37 more)

### Community 93 - "tests/tokens.rs"
Cohesion: 0.20
Nodes (16): a_token_is_revoked_with_the_binding_that_allowed_it(), INSIDE, json_of(), multi_folder_token_reads_its_folders_only(), not_uploaded(), only_writer_routes_accept_tokens(), OUTSIDE, outside_folders() (+8 more)

### Community 94 - "compilerOptions"
Cohesion: 0.12
Nodes (15): compilerOptions, exactOptionalPropertyTypes, isolatedModules, lib, module, moduleResolution, noEmit, noFallthroughCasesInSwitch (+7 more)

### Community 95 - "engine.rs"
Cohesion: 0.25
Nodes (26): a_batched_pull_makes_no_single_reads_and_drops_forged_records(), a_change_of_case_lands_on_a_case_insensitive_disk(), a_conflict_on_an_open_note_fails_that_file_only(), a_copied_folder_starts_fresh(), a_known_file_grown_past_the_limit_is_not_deleted(), a_merged_push_writes_the_merged_head(), a_move_killed_before_the_old_file_went_trashes_it_on_restart(), a_path_held_by_another_known_entry_is_handed_over() (+18 more)

### Community 96 - "tests/events.rs"
Cohesion: 0.30
Nodes (12): a_commit_reaches_the_socket(), a_notification_crosses_replicas(), a_socket_closes_when_its_caller_loses_the_vault(), a_ticket_flood_is_limited_per_address(), expired_ticket_is_refused(), notice_has_no_path(), replica(), S (+4 more)

### Community 97 - "auth/keys.rs"
Cohesion: 0.13
Nodes (12): Cache, decode_set(), Discovery, Keys, KeySource, Fixed, Http, MAX_AGE (+4 more)

### Community 98 - "RFC 0003 — Web console (front and back office) and notifications"
Cohesion: 0.09
Nodes (23): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+15 more)

### Community 99 - "task"
Cohesion: 0.15
Nodes (16): cmd, for, task, cmds, cmds, desc, cmds, desc (+8 more)

### Community 100 - "tests/config.rs"
Cohesion: 0.29
Nodes (7): a_note_sharing_bytes_with_hidden_configuration_stays_readable(), a_web_device_lists_and_reads_only_what_holds_no_secret(), config_namespace_hidden_from_other_member(), configuration_writes_last_wins_without_conflict_files(), D, S, setup()

### Community 101 - "app/tests.rs"
Cohesion: 0.24
Nodes (9): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp(), state() (+1 more)

### Community 102 - "files.rs"
Cohesion: 0.26
Nodes (13): ConfigFiles, FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES (+5 more)

### Community 103 - "tasks"
Cohesion: 0.13
Nodes (15): desc, cmds, desc, cmds, desc, cmds, desc, cmds (+7 more)

### Community 104 - "auth/tests.rs"
Cohesion: 0.29
Nodes (16): a_retired_key_stops_verifying_after_max_age(), a_slow_fetch_does_not_hold_other_callers(), accepts_a_valid_access_token(), access_claims(), ISSUER, keys_and_userinfo_come_through_oidc_discovery(), missing_role_claims_come_from_userinfo_and_are_cached(), oidc() (+8 more)

### Community 105 - "Command"
Cohesion: 0.09
Nodes (20): DeviceAction, Revoke, TokenAction, Create, List, Revoke, Cli, Command (+12 more)

### Community 106 - "HelpPage.vue"
Cohesion: 0.04
Nodes (56): 14. Test plan, 16. Implementation phases, 17. References and sources, 6.1 `kura-core`, 6.2 Server, 6.3 Console, 6. Detailed design, 6.P Proof against a real client (+48 more)

### Community 107 - "kura-server/src/main.rs"
Cohesion: 0.21
Nodes (13): backup(), check_chunks(), CONFIG_DIR_ENV, connect(), DATABASE_URL_ENV, dump_spec(), main(), maintain() (+5 more)

### Community 108 - "tests/merge.rs"
Cohesion: 0.23
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
Cohesion: 0.29
Nodes (8): as(), cleanup(), fail(), pass(), real(), sync.sh script, sql(), tokens

### Community 113 - "tests/sync.rs"
Cohesion: 0.28
Nodes (20): a_crowded_name_still_gets_a_unique_one(), a_path_freed_and_taken_in_one_commit(), a_pull_during_commits_never_passes_an_unseen_change(), blob(), changes_page_by_seq_and_refuse_a_lost_history(), commit_with_foreign_entry_id_is_refused(), concurrent_commits_never_skip_a_seq(), diverging_edits_become_a_conflict_file() (+12 more)

### Community 114 - "ObsidianHost"
Cohesion: 0.08
Nodes (3): Adapter, MemAdapter, obsidian()

### Community 115 - "cli.rs"
Cohesion: 0.21
Nodes (24): a_long_absence_resyncs_and_history_restores(), a_rewound_server_gets_everything_back(), a_scoped_token_syncs_its_folders_and_refuses_the_rest(), an_expired_access_token_is_renewed_mid_run(), code(), fake_issuer(), files(), folders_sync_between_devices() (+16 more)

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.09
Nodes (23): 10. The Obsidian plugin, 11. Local stack, 12. Maintenance, backups and restore, 13. Installing with Helm, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults (+15 more)

### Community 118 - "backup.rs"
Cohesion: 0.19
Nodes (13): BackupError, Busy, Db, LockLost, NoStorage, Objects, Other, check_chunks() (+5 more)

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 120 - "compilerOptions"
Cohesion: 0.09
Nodes (22): compilerOptions, allowImportingTsExtensions, forceConsistentCasingInFileNames, isolatedModules, jsx, lib, module, moduleResolution (+14 more)

### Community 121 - "app/sync_state.rs"
Cohesion: 0.17
Nodes (11): SyncCounts, caught_up(), COUNT_MAX, DeviceReport, DeviceSync, my_device_vaults(), pushed(), put_report() (+3 more)

### Community 123 - "BrowsePage.vue"
Cohesion: 0.03
Nodes (98): 18.1 What shipped, asking, byName, canRetry, closeHistory(), copied, crumbs, cut (+90 more)

### Community 124 - "Divergences"
Cohesion: 0.06
Nodes (36): 0. Before the first line of code, 1. Phase 1 — scaffold, configuration, identity, authorization, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Phase 4 — merge, notifications, plugin, 5. Phase 5 — history, retention, resync, 6. Phase 6 — operations, 7. Phase 7 — scoped tokens and search (+28 more)

### Community 125 - "build-checks.ts"
Cohesion: 0.13
Nodes (11): vite, BUDGET_BYTES, buildChecks(), gzippedBytes(), inlineScripts(), RFC-0003, vite@^8.3.8, pluginZip() (+3 more)

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
Cohesion: 0.22
Nodes (8): name, on, pull_request, push, workflow_dispatch, permissions, contents, branches

### Community 132 - "manifest.json"
Cohesion: 0.25
Nodes (7): author, description, id, isDesktopOnly, minAppVersion, name, version

### Community 133 - "app/tokens.rs"
Cohesion: 0.16
Nodes (20): check_cidrs(), check_folder(), check_name(), check_prefix(), check_scopes(), create_token(), CreateToken, invalid() (+12 more)

### Community 134 - "test_db"
Cohesion: 0.18
Nodes (16): no_key_verifier(), a_failed_tick_is_retried_as_a_local_change(), an_older_replica_never_overwrites_newer_state(), apply(), count_state(), DIRS, invalid_document_keeps_last_config(), orphan_disables_and_delete_schedules() (+8 more)

### Community 135 - "sim.rs"
Cohesion: 0.19
Nodes (4): Device, devices_converge_and_lose_nothing(), Rng, run()

### Community 136 - "jobs"
Cohesion: 0.29
Nodes (7): runs-on, runs-on, jobs, check, image, plugin, runs-on

### Community 137 - "test"
Cohesion: 0.25
Nodes (8): KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, test, env, runs-on, services

### Community 138 - "s3"
Cohesion: 0.33
Nodes (6): RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY, env, image, ports, s3

### Community 139 - "lint"
Cohesion: 0.29
Nodes (7): task, cmds, desc, cmds, desc, lint, recu

### Community 140 - "changes.rs"
Cohesion: 0.13
Nodes (18): ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE, query() (+10 more)

### Community 141 - "postgres"
Cohesion: 0.33
Nodes (6): POSTGRES_PASSWORD, env, image, options, ports, postgres

### Community 142 - "vault"
Cohesion: 0.40
Nodes (5): BAO_DEV_ROOT_TOKEN_ID, vault, env, image, ports

### Community 143 - "pnpm-workspace.yaml"
Cohesion: 0.29
Nodes (6): allowBuilds, esbuild, auditConfig, ignoreGhsas, overrides, minimatch

### Community 144 - "KeysPage.vue"
Cohesion: 0.03
Nodes (68): RFC-3339, 11. Other concerns, vue, vue-i18n, emit, { t }, { t }, box (+60 more)

### Community 145 - "deploy.yaml"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, tasks, alerts:test, helm:lint, vars (+3 more)

### Community 146 - "biome.json"
Cohesion: 0.12
Nodes (16): css, parser, files, includes, formatter, indentStyle, linter, rules (+8 more)

### Community 147 - "PrefsMenu.vue"
Cohesion: 0.09
Nodes (33): announced, close(), id, localeNote, locales, open, outside(), panel (+25 more)

### Community 148 - "reader.rs"
Cohesion: 0.09
Nodes (14): bearer(), client(), KeyCaller, NOT_UPLOADED, ReadAccess, Reader, Device, Key (+6 more)

### Community 149 - "api"
Cohesion: 0.67
Nodes (3): flatten, taskfile, api

### Community 150 - "notifier.rs"
Cohesion: 0.14
Nodes (13): BACKOFF, context(), EVENTS, IN_FLIGHT, MAX_FAILURES, open_rule(), Opened, POLL (+5 more)

### Community 151 - "tenants.yaml"
Cohesion: 0.25
Nodes (7): KuraTenant dev, KuraVault notes, apiVersion, kind, metadata, apiVersion, kind

### Community 152 - "RFC 0006 — Sync state per device and per vault"
Cohesion: 0.17
Nodes (12): 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 17. References and sources, 1. Summary, 2. Motivation, 3. Goals / non-goals (+4 more)

### Community 153 - "Algorithm"
Cohesion: 0.18
Nodes (11): Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512, RS256 (+3 more)

### Community 155 - "src/vault.rs"
Cohesion: 0.24
Nodes (7): default_region(), kv2_paths_split_into_mount_and_secret(), secrets_never_print(), SERVICE_ACCOUNT_TOKEN, split_kv2(), TIMEOUT, TOKEN_ENV

### Community 156 - "crypto.rs"
Cohesion: 0.11
Nodes (22): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, CryptoError, Corrupt, KeyLength, Malformed, Random (+14 more)

### Community 157 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 158 - "client.ts"
Cohesion: 0.09
Nodes (14): browserName(), ensureDevice(), Retryable, RFC-0003, RFC-0003, call(), ContentError, fileBytes() (+6 more)

### Community 160 - "scripts"
Cohesion: 0.25
Nodes (8): scripts, build, dev, format, generate:client, lint, test, types:check

### Community 161 - "@playwright/test"
Cohesion: 0.22
Nodes (11): RFC-0008, @playwright/test, RFC-0003, RFC-0006, env(), signIn(), signOut(), RFC-0003 (+3 more)

### Community 162 - "6. Detailed design"
Cohesion: 0.29
Nodes (8): 6.1 Data model (migration `0027_device_sync.sql`), 6.2 `app/changes.rs`, 6.3 `app/commits.rs`, 6.4 `app/sync_state.rs` (new), 6.5 `ui/`, 6. Detailed design, 6.P Proof against a real client, Device

### Community 163 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 164 - "net.rs"
Cohesion: 0.12
Nodes (19): a_target_is_https_without_credentials(), ip(), MAX_URL, private_and_special_addresses_are_refused(), refused_by_default(), Target, TargetError, AddressRefused (+11 more)

### Community 165 - "server"
Cohesion: 0.29
Nodes (7): audiences, discovery, issuer, platformAdmin, claim, value, oidc

### Community 166 - "vault"
Cohesion: 0.20
Nodes (10): mount, role, bucket, database, vault, mount, address, kubernetesAuth (+2 more)

### Community 167 - ".start"
Cohesion: 0.17
Nodes (8): Core, loadCore(), because(), http(), random16(), sleep(), Api, device()

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
Cohesion: 0.17
Nodes (8): access_token(), login(), Attached, Config, Credentials, refresh_token(), set_refresh_token(), write_private()

### Community 172 - "Chart.yaml"
Cohesion: 0.29
Nodes (6): apiVersion, appVersion, description, name, type, version

### Community 173 - "Env"
Cohesion: 0.27
Nodes (11): env(), old_events_are_purged_while_notifications_are_off(), a_restricted_viewer_reads_its_folders_only(), apply(), config(), file(), folders_are_a_viewers_only(), no_route_shows_a_restricted_viewer_a_hidden_note() (+3 more)

### Community 174 - "limits.rs"
Cohesion: 0.23
Nodes (7): counts_per_key_and_route_and_says_when_to_retry(), limit_of(), LIMITS, MAX_TRACKED, RateLimiter, WINDOW, WindowKey

### Community 175 - "VaultPath"
Cohesion: 0.06
Nodes (18): a_255_byte_name_is_written(), err(), FsHost, mtime(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp() (+10 more)

### Community 176 - "RFC 0005 — Folder-restricted viewers and the web reader"
Cohesion: 0.12
Nodes (16): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 15. Decisions and open questions, 16. Implementation phases, 17. References and sources, 1. Summary (+8 more)

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
Cohesion: 0.19
Nodes (18): blob_hash(), Storage, a_failed_commit_leaves_the_object_for_the_sweep(), batches_answer_in_order_within_their_budget(), bucket_object_is_not_plaintext(), commit_blob(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422() (+10 more)

### Community 182 - "image"
Cohesion: 0.40
Nodes (5): image, digest, pullPolicy, repository, tag

### Community 183 - "tests/search.rs"
Cohesion: 0.25
Nodes (6): Round, a_failing_index_job_is_kept_after_five_attempts(), S, search_is_opt_in_indexes_markdown_and_follows_edits(), setup(), SyncOptions

### Community 184 - "remote.ts"
Cohesion: 0.12
Nodes (9): RFC-0006, RFC-0007, Sleep, ApiOptions, CLIENT, Fetch, retryAfter(), VaultView (+1 more)

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

### Community 192 - "history.rs"
Cohesion: 0.21
Nodes (10): a_pass_from_zero_pages_past_the_horizon(), eid(), epochs_change_once_with_the_cluster_identity(), history_and_restore(), purged_deletions_move_the_horizon(), retention_prunes_history_but_never_a_head(), S, setup() (+2 more)

### Community 193 - "wire.rs"
Cohesion: 0.07
Nodes (28): Author, commit_bodies_parse_strictly(), CommitRequest, CommitResponse, eid(), entry_ids_are_uuid_v7(), EntryId, Operation (+20 more)

### Community 194 - "android.sh"
Cohesion: 0.52
Nodes (6): adb(), ANDROID_HOME, booted(), sdk(), android.sh script, wait_boot()

### Community 197 - "reader.sh"
Cohesion: 0.33
Nodes (10): api(), as(), chunk_of(), cleanup(), entry(), fail(), hits(), pass() (+2 more)

### Community 198 - "Env"
Cohesion: 0.32
Nodes (6): Env, a_folder_is_an_entry_older_clients_never_see(), file(), folders_follow_the_scope_of_notes(), Who, who()

### Community 199 - "bindings"
Cohesion: 0.17
Nodes (16): claimValue, paths, role, categories, KuraVault campaign, apiVersion, kind, spec (+8 more)

### Community 200 - "overrides"
Cohesion: 0.29
Nodes (6): auditConfig, ignoreGhsas, overrides, glob, js-yaml, minimatch

### Community 201 - "deserialize"
Cohesion: 0.13
Nodes (4): DnsLabel, NameError, String, Metadata

### Community 202 - "sync_one"
Cohesion: 0.23
Nodes (11): attach(), bytes_for_people(), code_of(), confirm(), exit_codes_follow_section_4_5(), human(), params(), remote_entries() (+3 more)

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

### Community 212 - "connect_and_watch"
Cohesion: 0.22
Nodes (10): connect_and_watch(), fingerprint(), on_socket(), Socketed, Reconnect, Stop, watch(), Watched (+2 more)

### Community 213 - "AppState"
Cohesion: 0.10
Nodes (8): AppState, HttpSettings, ConfigError, Invalid, Read, load_if_changed(), REFRESH, run()

### Community 214 - "types.ts"
Cohesion: 0.10
Nodes (19): Controller, ControllerOptions, QUIET_MS, problem(), label(), Situation, word(), count() (+11 more)

### Community 215 - "PaneResizer.vue"
Cohesion: 0.29
Nodes (9): clamp(), emit, key(), move(), props, remPx(), reset(), up() (+1 more)

### Community 216 - "test"
Cohesion: 0.67
Nodes (3): test, flatten, taskfile

### Community 217 - "6. Detailed design"
Cohesion: 0.25
Nodes (8): 6.1 Data model (migration `0020_token_scopes.sql`), 6.2 `authz::PathScope` and the read side, 6.3 `Writer`, in `app/reader.rs`, 6.4 What a token may name: possession, 6.5 The commit, for a token, 6.6 `kura-cli`, 6. Detailed design, 6.P Proof against a real client

### Community 218 - "dev:certs:renew"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:certs:renew

### Community 219 - "18. Revision against the tree (2026-11-18)"
Cohesion: 0.22
Nodes (9): 18.1 What shipped, 18.2 Divergences from the text, 18.3 What the proof observed, 18.4 Threats against the tests, 18.5 Supply chain, 18.6 Mocks against what shipped, 18.7 Phase 3 and the proofs, landed the same day, 18.8 Owners by name, and the browser proof (supersedes D13) (+1 more)

### Community 220 - "token.rs"
Cohesion: 0.17
Nodes (7): hex_digit(), PREFIX, ScopedToken, SECRET_BYTES, TokenError, Malformed, tokens_round_trip_and_parse_strictly()

### Community 221 - "KuraTenant other"
Cohesion: 0.50
Nodes (4): KuraTenant other, apiVersion, kind, metadata

### Community 223 - "notify.rs"
Cohesion: 0.10
Nodes (17): CHANNEL, Event, Change, Pull, Revoked, four_sockets_per_device_at_most(), HEARTBEAT, Hub (+9 more)

### Community 224 - "PathError"
Cohesion: 0.14
Nodes (13): nfc_is_required_on_the_wire_and_applied_locally(), PathError, Control, DotSegment, Empty, EmptySegment, Hidden, Ignorable (+5 more)

### Community 225 - "duration"
Cohesion: 0.18
Nodes (6): MAX_RETRIES, s3(), StorageError, TIMEOUT, BucketSecret, DatabaseSecret

### Community 226 - "config-view.sh"
Cohesion: 0.40
Nodes (10): api(), as(), cleanup(), entry_of(), fail(), hash_of(), pass(), config-view.sh script (+2 more)

### Community 227 - "kura-core/src/config.rs"
Cohesion: 0.39
Nodes (8): categories_never_include_what_stays_on_the_device(), category(), like_patterns_cover_viewable(), only_files_without_secrets_are_viewable(), plugin_of(), synced(), viewable(), VIEWABLE_LIKE

### Community 228 - "networkPolicy"
Cohesion: 0.50
Nodes (4): networkPolicy, enabled, ingressNamespace, monitoringNamespace

### Community 229 - "18. Revision against the tree (2026-11-17, gaps still open)"
Cohesion: 0.25
Nodes (8): 18.1 What shipped, 18.2 Where the tree differs from the text, 18.3 What the proof observed, 18.4 §7 threats and the tests that prove them, 18.5 Supply chain as shipped, 18.6 The mocks of §4.5 against the console as built, 18.7 Gaps closed on landing (2026-11-17), 18. Revision against the tree (2026-11-17)

### Community 230 - "name"
Cohesion: 0.21
Nodes (12): KuraVault obsidian, KuraVault shared, metadata, metadata, apiVersion, kind, metadata, apiVersion (+4 more)

### Community 231 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 232 - "KuraTenant jdr"
Cohesion: 0.50
Nodes (4): KuraTenant jdr, apiVersion, kind, metadata

### Community 233 - "KuraVault elsewhere"
Cohesion: 0.40
Nodes (5): KuraVault elsewhere, apiVersion, kind, metadata, spec

### Community 234 - "devices.sh"
Cohesion: 0.28
Nodes (11): api(), as(), body(), cleanup(), device(), fail(), pass(), row() (+3 more)

### Community 236 - "record"
Cohesion: 0.30
Nodes (5): Fact, insert(), Notifier, quota_check(), record()

### Community 237 - "18. Revision against the tree (2026-11-17)"
Cohesion: 0.22
Nodes (9): 18.1 What shipped, 18.2 Divergences from the text, 18.3 What the proof observed, 18.4 Threats against the tests, 18.5 Supply chain, 18.6 Mocks against what shipped, 18.7 Header, 18.8 Gaps closed on landing (2026-11-17) (+1 more)

### Community 238 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 239 - "6. Detailed design"
Cohesion: 0.29
Nodes (7): 6.1 `kura-api`, 6.2 Data model (migration `0021_binding_paths.sql`), 6.3 `authz`, 6.4 Server routes that take a person but not a `Reader`, 6.5 The reader (`ui/`, front office of RFC 0003), 6. Detailed design, 6.P Proof against a real client

### Community 240 - "updates"
Cohesion: 0.22
Nodes (8): prefix, interval, updates, commit-message, directory, package-ecosystem, schedule, version

### Community 241 - "modals.ts"
Cohesion: 0.08
Nodes (21): DeviceCode, secureUrl(), DEFAULTS, notify(), timers, RFC-0001, Ask, Button (+13 more)

### Community 242 - ".json"
Cohesion: 0.24
Nodes (3): body(), older_server(), VaultRemote

### Community 243 - "dependencies"
Cohesion: 0.17
Nodes (12): dompurify, markdown-it, mermaid, dependencies, axios@^1.20.0, dompurify@^3.4.18, markdown-it@^15.0.3, mermaid@12.3.0 (+4 more)

### Community 244 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 245 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 246 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 247 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 249 - "serve"
Cohesion: 0.24
Nodes (6): app(), cors(), router(), Background, background(), serve()

### Community 250 - "claimMappings"
Cohesion: 0.13
Nodes (17): claim, roles, spec, spec, spec, maxFileSize, storage, admin (+9 more)

### Community 251 - ".fill"
Cohesion: 0.13
Nodes (6): ChunkUploads, download(), lock(), Prefetch, Prefetch<'a, R>, Prefetch<'_, R>

### Community 252 - "store/keys.rs"
Cohesion: 0.16
Nodes (8): Cache, CACHE_SIZE, CACHE_TTL, DataKeys, Slot, Notifications, Vault, transit_key_name()

### Community 253 - "src/merge.rs"
Cohesion: 0.27
Nodes (10): edits_of_different_lines_merge(), MAX_MERGE, MAX_MERGE_LINES, Merge, merge3(), Clean, Conflict, mergeable() (+2 more)

### Community 255 - "deliver"
Cohesion: 0.35
Nodes (9): classify(), deliver(), Outcome, Delivered, Failed, Gone, Refused, post() (+1 more)

### Community 259 - "path"
Cohesion: 0.27
Nodes (8): BATCH_BYTES, BATCH_RECORDS, batches(), files(), KIB, main(), pct(), SETTINGS

### Community 261 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 262 - "5. Architecture"
Cohesion: 0.50
Nodes (4): 5.1 The scope of a person, 5.2 One filter for tokens and people, 5.3 Reading a note in the browser, 5. Architecture

### Community 263 - "notify.sh"
Cohesion: 0.50
Nodes (7): api(), cleanup(), fail(), pass(), serve(), notify.sh script, sql()

### Community 264 - "hourly"
Cohesion: 0.24
Nodes (5): Fact<'a>, hourly(), purge(), run(), warn_rules_while_off()

### Community 265 - "pkce.ts"
Cohesion: 0.40
Nodes (8): RFC-6749, RFC-7636, base64url(), beginAuthorization(), challengeOf(), inAppTarget(), randomToken(), RFC-0003

### Community 266 - "ScopeError"
Cohesion: 0.28
Nodes (9): check_folder(), check_rules(), ScopeError, Both, Count, NotAFolder, Path, Twice (+1 more)

### Community 267 - "devfile.yaml"
Cohesion: 0.33
Nodes (5): metadata, name, parent, uri, schemaVersion

### Community 268 - "scalar.rs"
Cohesion: 0.36
Nodes (5): API_DOCS_CSP, scalar(), SCALAR_BUNDLE_PATH, scalar_unavailable_html(), the_reference_loads_nothing_from_a_third_party()

### Community 269 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 272 - "6. Detailed design"
Cohesion: 0.29
Nodes (7): 6.1 `ui/` and how it is served, 6.2 Sign-in and the web device, 6.3 New read routes, 6.4 Notifications (migration `0023_notifications.sql`), 6.5 The guarded client (`net.rs`), 6. Detailed design, 6.P Proof against a real client

### Community 273 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 275 - "KuraVault suite-config"
Cohesion: 0.50
Nodes (4): KuraVault suite-config, apiVersion, kind, metadata

### Community 277 - "RFC 0004 — implementation plan"
Cohesion: 0.67
Nodes (3): Divergences, Progress, RFC 0004 — implementation plan

### Community 281 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 282 - "5. Architecture"
Cohesion: 0.50
Nodes (4): 5.1 One scope type, two questions, 5.2 Which routes a token reaches, 5.3 A recap written by the bot, 5. Architecture

### Community 286 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 308 - "18. Revision against the tree (2026-11-17)"
Cohesion: 0.29
Nodes (7): 18.1 What shipped, 18.2 Where it differs from the text, 18.3 What the proof observed, 18.4 §7, the tests that prove each row, 18.5 §8, supply chain, 18.6 §4.5, mocks against what shipped, 18. Revision against the tree (2026-11-17)

### Community 313 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 318 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 324 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 325 - "5. Architecture"
Cohesion: 0.67
Nodes (3): 5.1 Where the fact comes from, 5.2 Who sees which row, 5. Architecture

## Knowledge Gaps
- **206 isolated node(s):** `RFC-0001`, `RFC-0001`, `Editor`, `Manager`, `Viewer` (+201 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 2140 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **38 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `RFC 0003 — Web console (front and back office) and notifications` connect `RFC 0003 — Web console (front and back office) and notifications` to `ui.rs`, `18. Revision against the tree (2026-11-17, gaps still open)`, `rfc0001-implementation-plan.md`, `7. Security`, `6. Detailed design`, `4. User-facing design`?**
  _High betweenness centrality (0.155) - this node is a cross-community bridge._
- **Are the 6 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 6 INFERRED edges - model-reasoned connections that need verification._
- **What connects `RFC-0001`, `RFC-0001`, `Editor` to the rest of the system?**
  _206 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `authz.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10448979591836735 - nodes in this community are weakly interconnected._
- **Why does `RFC 0007 — Folders as synced entries` connect `RFC 0007 — Folders as synced entries` to `rfc0001-implementation-plan.md`?**
  _High betweenness centrality (0.141) - this node is a cross-community bridge._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1282051282051282 - nodes in this community are weakly interconnected._
- **Should `common/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09359605911330049 - nodes in this community are weakly interconnected._