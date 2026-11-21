# Graph Report - weebo-kura  (2026-11-21)

## Corpus Check
- 366 files · ~365,268 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 29 file(s) not represented in the graph (top: (none) 10, .toml 7, .css 4)

## Summary
- 5788 nodes · 12898 edges · 353 communities (255 shown, 98 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 617 edges (avg confidence: 0.82)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `70ca299c`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- VaultAccess
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
- ChunkHash
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
- HomePage.vue
- tests/notifications.rs
- 6. Detailed design
- tasks
- ApiError
- RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults
- tasks
- app/search.rs
- RFC 0004 — Scoped tokens: several folders, and write access
- helpers.js
- ref_vitest
- openapi_contract.rs
- SharedState
- ConfigPage.vue
- reader.rs
- client.ts
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
- problem
- ui.rs
- sqlx-sqlite
- package.json
- tasks
- chunk.rs
- auth/mod.rs
- Notifier
- app/events.rs
- PathError
- auth.ts
- Vault
- scope.rs
- ObsidianHost
- api.rs
- VaultRemote
- kura-api/src/lib.rs
- commands.rs
- PluginGuard
- withDevice
- ui/package.json
- Env
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
- app/sync_state.rs
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
- Serialize
- dev:env
- compilerOptions
- Deserialize
- BrowsePage.vue
- Divergences
- core.ts
- tests
- values.yaml
- tasks
- init-repo
- keys.sh
- ci.yml
- manifest.json
- app/tokens.rs
- reconcile.rs
- steps
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
- vaults.rs
- api
- notifier.rs
- tenants.yaml
- RFC 0006 — Sync state per device and per vault
- chunk_stats.rs
- wasm.d.ts
- analyze
- crypto.rs
- rules
- HistoryPanel.vue
- scripts
- @playwright/test
- 6. Detailed design
- rules
- net.rs
- server
- vault
- ScopeError
- backup
- cnpg
- Documentation
- kura-cli/src/config.rs
- Chart.yaml
- problem
- limits.rs
- VaultPath
- RFC 0005 — Folder-restricted viewers and the web reader
- ingress
- resources
- monitoring
- attach.sh
- tests/chunks.rs
- image
- snapshot.rs
- remote.ts
- drill.sh
- run.sh
- deploy
- dev
- perf
- rust
- 18. Revision against the tree (2026-11-17)
- history.rs
- wire.rs
- android.sh
- soak.sh
- reader.sh
- Keys
- bindings
- overrides
- test_db
- Api
- authz.sh
- server
- coverage
- plugin
- console.sh
- dev:certs
- ui
- kura-core/src/config.rs
- sw.js
- connect_and_watch
- AppState
- types.ts
- PaneResizer.vue
- test
- 6. Detailed design
- dev:certs:renew
- app
- token.rs
- KuraTenant other
- notify.rs
- 18. Revision against the tree (2026-11-18)
- duration
- config-view.sh
- 18. Revision against the tree (2026-11-17)
- networkPolicy
- 7. Security
- name
- 7. Security
- KuraTenant jdr
- KuraVault elsewhere
- devices.sh
- states.js
- 7. Security
- 18. Revision against the tree (2026-11-17)
- 7. Security
- 6. Detailed design
- 4. User-facing design
- modals.ts
- 10. Operability
- 7. Security
- 4. User-facing design
- 5. Architecture
- 4. User-facing design
- 2026-11-17T22-55-29Z__ui-src-pages.md
- 10. Operability
- claimMappings
- .fill
- Algorithm
- src/merge.rs
- 2026-11-18T20-15-32Z__ui-src-pages.md
- 1. Phase 1 — scaffold, configuration, identity, authorization
- 15. Decisions and open questions
- 15. Decisions and open questions
- 10. Operability
- 5. Architecture
- notify.sh
- weebo-kura
- Security Policy
- 5. Architecture
- TestKey
- scalar.rs
- dockerfile
- Token
- 6. Detailed design
- KuraVault suite-config
- .trivyignore.yaml
- AuthError
- sim.rs
- 4. User-facing design
- 5. Architecture
- 10. Operability
- 15. Decisions and open questions
- 15. Decisions and open questions

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 203 edges
2. `SharedState` - 149 edges
3. `ChunkHash` - 111 edges
4. `RemoteError` - 107 edges
5. `VaultPath` - 86 edges
6. `Env` - 75 edges
7. `withDevice()` - 61 edges
8. `HostError` - 61 edges
9. `AppState` - 53 edges
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
- `14. Test plan` --references--> `sectionsOf()`  [INFERRED]
  docs/RFC/rfc0008-console-config-view.md → ui/src/vaults.ts

## Import Cycles
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (353 total, 98 thin omitted)

### Community 0 - "VaultAccess"
Cohesion: 0.11
Nodes (34): VaultRole, Editor, Manager, Viewer, on_ping(), Principal, a_binding_without_folders_overrides_folders(), a_tenant_role_grant_overrides_folders() (+26 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.09
Nodes (23): bytes(), json(), load_previous(), log_applied(), log_rejected(), Outcome, Applied, NotWritten (+15 more)

### Community 2 - "common/mod.rs"
Cohesion: 0.14
Nodes (4): admin_url(), ISSUER, ROLE_PASSWORD, TestDb

### Community 3 - "includes"
Cohesion: 0.20
Nodes (10): includes, mise, obsidian, security, skills, template, flatten, taskfile (+2 more)

### Community 4 - "ProblemType"
Cohesion: 0.06
Nodes (31): Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType (+23 more)

### Community 5 - "Env"
Cohesion: 0.09
Nodes (12): capture_logs(), Captured, env_custom(), two_replicas_creating_one_data_key_keep_one(), vault(), logs_name_routes_and_audit_every_denial(), rule_changes_are_audited(), admin() (+4 more)

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.09
Nodes (26): alg_none_cannot_be_configured(), ClaimMatch, CorsConfig, default_plugin_client(), discovery_is_optional_and_https(), err(), EXAMPLE, issuer_must_be_https() (+18 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.10
Nodes (12): Divergences, Progress, RFC 0002 — implementation plan, Divergences, Progress, RFC 0003 — implementation plan, Divergences, Progress (+4 more)

### Community 8 - "steps"
Cohesion: 0.23
Nodes (12): steps, BASE, steps, env, if, name, run, uses (+4 more)

### Community 9 - "Duration"
Cohesion: 0.09
Nodes (11): digits(), Duration, Quantity, QuantityRepr, Bytes, Text, ScalarError, Duration (+3 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (29): extends, description, given, message, severity, then, overrides, files (+21 more)

### Community 11 - "app/notifications.rs"
Cohesion: 0.13
Nodes (32): DeviceCaller, authorization_rejects_control_characters(), authorize(), Channel, check_authorization(), check_events(), check_name(), check_update() (+24 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.16
Nodes (26): parse_file(), binding_names_exactly_one_subject(), binding_paths_are_checked(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused(), defaults_are_on_the_closed_side() (+18 more)

### Community 14 - "ChunkHash"
Cohesion: 0.08
Nodes (8): Remote, RemoteError, Problem, Scripted, AppRemote, decode(), hashes(), RemoteAdapter

### Community 15 - "spec.rs"
Cohesion: 0.10
Nodes (25): ANNOTATIONS, Binding, Chunking, .DEFAULT, .MAX_CHUNK, ClaimMappings, ConfigCategory, Hotkeys (+17 more)

### Community 16 - "app/mod.rs"
Cohesion: 0.11
Nodes (19): account_routes(), ApiDoc, audit_denials(), CLIENT_HEADER, client_too_old(), client_version(), docs_csp(), healthz() (+11 more)

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
Cohesion: 0.17
Nodes (17): ACQUIRE_TIMEOUT, begin_subject(), begin_tenant(), begin_tenant_snapshot(), check_schema(), connect(), crate::problem::ApiError, DbError (+9 more)

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
Cohesion: 0.09
Nodes (21): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+13 more)

### Community 26 - "KuraPlugin"
Cohesion: 0.11
Nodes (28): RFC-0001, ago(), en, fr, Key, roleName(), setLanguage(), t() (+20 more)

### Community 27 - ".json"
Cohesion: 0.21
Nodes (5): Api, DeviceView, ScopeRule, TokenView, VaultView

### Community 28 - "tasks"
Cohesion: 0.11
Nodes (17): cmds, desc, cmds, desc, cmds, desc, cmds, desc (+9 more)

### Community 29 - "super"
Cohesion: 0.13
Nodes (8): ReasonCode, .SUCCESS, Removed, build(), observed_generation(), ready(), StoredStatus, widened()

### Community 30 - "kura-wasm/src/lib.rs"
Cohesion: 0.11
Nodes (33): bytes(), changes(), commit(), File, folders(), get_blob(), get_blobs(), get_chunk() (+25 more)

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "tasks"
Cohesion: 0.05
Nodes (43): cmd, for, var, cmds, desc, cmds, desc, requires (+35 more)

### Community 34 - "HomePage.vue"
Cohesion: 0.10
Nodes (22): anything(), Targets, targets(), RFC-0003, canRetry, devices, error, expiring (+14 more)

### Community 35 - "tests/notifications.rs"
Cohesion: 0.15
Nodes (21): a_manager_sees_their_vaults_only_and_off_means_409(), a_rule_signs_its_deliveries_and_never_shows_its_secrets(), Admin, delivery_result_is_coarse_and_body_unread(), drain(), events_reach_their_rules_with_ids_only_and_retries_back_off(), Got, no_webhook_reaches_an_address_the_operator_did_not_allow() (+13 more)

### Community 36 - "6. Detailed design"
Cohesion: 0.15
Nodes (13): 6.10 Configuration reconciliation, 6.11 Engineering rules, 6.1 Data model, 6.2 Tenancy, authentication and authorization, 6.3 Sync protocol, 6.4 Chunking and storage, 6.5 Conflict resolution, 6.6 Encryption at rest (+5 more)

### Community 37 - "tasks"
Cohesion: 0.15
Nodes (12): task, cmds, desc, cmds, desc, cmds, desc, tasks (+4 more)

### Community 38 - "ApiError"
Cohesion: 0.10
Nodes (44): ChunkHash, Body, batch_blobs(), batch_chunks(), BATCH_FETCHES, batch_hashes(), BATCHES_IN_FLIGHT, BlobBatchRequest (+36 more)

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
Cohesion: 0.07
Nodes (30): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+22 more)

### Community 43 - "helpers.js"
Cohesion: 0.16
Nodes (22): BASE_URL, BLOB, SIZE, TOKEN, VAULT, CLIENT, headers(), pullAll() (+14 more)

### Community 44 - "ref_vitest"
Cohesion: 0.04
Nodes (55): 16. Implementation phases, 6.3 Console, pinia, vue-router, @vue/test-utils, forgetDiscovery(), ConsoleConfig, loadedConfig() (+47 more)

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "SharedState"
Cohesion: 0.11
Nodes (30): Caller, needed_claims(), check_live_quota(), Device, device_event(), device(), DeviceVault, insert_device() (+22 more)

### Community 47 - "ConfigPage.vue"
Cohesion: 0.07
Nodes (41): anyHiddenSettings, applies, canRetry, edited, entries, error, file, groups (+33 more)

### Community 48 - "reader.rs"
Cohesion: 0.08
Nodes (17): DEVICE_HEADER, rate_limit(), require_device(), require_session(), bearer(), client(), console_writes_nothing(), NOT_UPLOADED (+9 more)

### Community 49 - "client.ts"
Cohesion: 0.06
Nodes (25): axios, axios@^1.20.0, browserName(), ensureDevice(), Retryable, signOut(), RFC-0003, RFC-0003 (+17 more)

### Community 50 - "metrics.rs"
Cohesion: 0.17
Nodes (19): batches(), commits(), http(), index(), lifecycle(), maintenance(), Metrics, metrics_are_exported_with_the_kura_prefix() (+11 more)

### Community 51 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 52 - "RFC 0007 — Folders as synced entries"
Cohesion: 0.05
Nodes (43): 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 15. Decisions and open questions, 16. Implementation phases, 17. References and sources (+35 more)

### Community 53 - "src/maintenance.rs"
Cohesion: 0.05
Nodes (51): Round, an_object_gone_since_the_listing_is_skipped(), BackupError, Busy, Db, LockLost, NoStorage, Objects (+43 more)

### Community 54 - "src/sync.rs"
Cohesion: 0.10
Nodes (50): a_refused_push_counts_what_it_kept(), Applied, Deferred, Done, apply(), apply_folder(), BATCH, blob_of() (+42 more)

### Community 55 - "KeysPage.vue"
Cohesion: 0.10
Nodes (20): allRevoked, error, loading, marks, person(), props, query, revokedCount (+12 more)

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (45): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+37 more)

### Community 57 - "ref_vitest"
Cohesion: 0.06
Nodes (50): RFC-6749, RFC-7636, 17. References and sources, installAuth(), discover(), load(), ProviderMetadata, trim() (+42 more)

### Community 58 - "path.rs"
Cohesion: 0.11
Nodes (13): allowed_char(), check(), ignorable(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX (+5 more)

### Community 59 - "kura-api"
Cohesion: 0.40
Nodes (6): kura-api, kura-cli, kura-core, kura-fuzz, kura-server, kura-wasm

### Community 64 - "Fail"
Cohesion: 0.33
Nodes (20): access_token(), attach(), config(), Ctx, device_login(), devices(), diff(), entry_of() (+12 more)

### Community 65 - "problem"
Cohesion: 0.12
Nodes (26): problem(), a_web_device_never_gets_the_configuration_space(), bindings_need_manager(), idle_web_devices_are_purged(), web(), web_device_is_reused_per_sid(), a_binding_alone_links_the_device_to_its_tenant(), a_tenant_admin_sees_and_revokes_the_tenants_devices_only() (+18 more)

### Community 67 - "ui.rs"
Cohesion: 0.14
Nodes (13): check_dir(), config(), ConsoleConfig, frame_hash(), headers(), origin(), PluginClient, router() (+5 more)

### Community 69 - "package.json"
Cohesion: 0.08
Nodes (29): RFC-0001, description, devDependencies, esbuild@0.28.2, eslint@10.14.0, @eslint/js@10.0.1, obsidian@1.14.4, @types/node@26.6.6 (+21 more)

### Community 70 - "tasks"
Cohesion: 0.06
Nodes (32): cmds, desc, dir, cmds, deps, desc, dir, cmds (+24 more)

### Community 71 - "chunk.rs"
Cohesion: 0.15
Nodes (15): an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, ChunkParams, chunks(), ChunkSpan (+7 more)

### Community 72 - "auth/mod.rs"
Cohesion: 0.12
Nodes (7): algorithm(), LEEWAY_SECS, MAX_TOKEN_BYTES, now(), USERINFO_CACHE, UserinfoCache, Verifier

### Community 73 - "Notifier"
Cohesion: 0.14
Nodes (8): Notifier, NotifierOptions, POLL_MS, SILENCE_MS, SocketLike, Timers, FakeSocket, setup()

### Community 74 - "app/events.rs"
Cohesion: 0.07
Nodes (20): ACCESS_REVOKED, create_ticket(), DEVICE_REVOKED, events(), EventsQuery, Notice, Change, Ping (+12 more)

### Community 75 - "PathError"
Cohesion: 0.14
Nodes (13): nfc_is_required_on_the_wire_and_applied_locally(), PathError, Control, DotSegment, Empty, EmptySegment, Hidden, Ignorable (+5 more)

### Community 76 - "auth.ts"
Cohesion: 0.10
Nodes (30): RFC-8628, AuthError, AuthFailure, call(), discover(), Discovery, fnv(), Http (+22 more)

### Community 77 - "Vault"
Cohesion: 0.12
Nodes (18): client_err(), DatabaseSecret, decode_b64(), default_region(), kv2_paths_split_into_mount_and_secret(), refresh_credentials(), rotate_database(), secrets_never_print() (+10 more)

### Community 78 - "scope.rs"
Cohesion: 0.21
Nodes (7): a_scope_reads_and_writes_under_its_folders_only(), empty_scope_sees_nothing(), fold(), like_patterns_escape_wildcards(), MAX_SCOPES, PathScope, scope_of()

### Community 79 - "ObsidianHost"
Cohesion: 0.14
Nodes (6): RFC-0007, HostOptions, ObsidianHost, parent(), toBuffer(), host()

### Community 80 - "api.rs"
Cohesion: 0.13
Nodes (28): a_405_is_an_older_server_and_a_404_is_not(), ATTEMPTS, Chunking, CLIENT, DeviceCode, discover(), Discovery, http() (+20 more)

### Community 81 - "VaultRemote"
Cohesion: 0.18
Nodes (5): body(), check(), exchange(), newer(), VaultRemote

### Community 82 - "kura-api/src/lib.rs"
Cohesion: 0.11
Nodes (29): TenantSlug, ConfigError, parse_document(), Resource, Tenant, Vault, Source, typed() (+21 more)

### Community 83 - "commands.rs"
Cohesion: 0.11
Nodes (17): a_host_name_is_a_valid_device_name(), BACKOFF_MAX, BACKOFF_MIN, backoff_stays_between_one_and_sixty_seconds(), bytes_for_people(), code_of(), Diff, exit_codes_follow_section_4_5() (+9 more)

### Community 84 - "PluginGuard"
Cohesion: 0.09
Nodes (21): Progress, because(), random16(), readSettings(), registry(), Settings, codeHashes(), Consent (+13 more)

### Community 86 - "withDevice"
Cohesion: 0.03
Nodes (95): withDevice(), detailOf(), KNOWN, messageOf(), problemType(), retryable(), RFC-9457, load() (+87 more)

### Community 87 - "ui/package.json"
Cohesion: 0.04
Nodes (55): @biomejs/biome, dompurify, @fontsource/jetbrains-mono, @fontsource/silkscreen, @hey-api/openapi-ts, jsdom, markdown-it, tailwindcss (+47 more)

### Community 88 - "Env"
Cohesion: 0.20
Nodes (7): Basic, Env, env(), file(), Who, Who, the_binding_api_for_an_admin()

### Community 89 - "tasks"
Cohesion: 0.07
Nodes (26): tasks, ui:audit, ui:build, ui:client, ui:install, ui:lint, ui:test, cmds (+18 more)

### Community 90 - "RFC 0008 — The synced Obsidian configuration in the console"
Cohesion: 0.06
Nodes (31): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+23 more)

### Community 91 - "oidc"
Cohesion: 0.06
Nodes (33): mount, role, listen, api, metrics, notifications, enabled, webhook (+25 more)

### Community 92 - "ApiError"
Cohesion: 0.06
Nodes (76): CommitRequest, CommitResponse, OpResult, Outcome, Accepted, Conflicted, Merged, RestoreRequest (+68 more)

### Community 93 - "tests/tokens.rs"
Cohesion: 0.16
Nodes (28): blob_hash(), a_token_is_revoked_with_the_binding_that_allowed_it(), a_token_never_restores(), a_token_reads_only_its_vault(), authors_are_names_never_subjects(), batch_respects_token_prefix(), four_ways_out_and_a_read_only_folder(), INSIDE (+20 more)

### Community 94 - "compilerOptions"
Cohesion: 0.12
Nodes (15): compilerOptions, exactOptionalPropertyTypes, isolatedModules, lib, module, moduleResolution, noEmit, noFallthroughCasesInSwitch (+7 more)

### Community 95 - "engine.rs"
Cohesion: 0.16
Nodes (29): a_batched_pull_makes_no_single_reads_and_drops_forged_records(), a_change_of_case_lands_on_a_case_insensitive_disk(), a_conflict_on_an_open_note_fails_that_file_only(), a_copied_folder_starts_fresh(), a_known_file_grown_past_the_limit_is_not_deleted(), a_merged_push_writes_the_merged_head(), a_move_killed_before_the_old_file_went_trashes_it_on_restart(), a_path_held_by_another_known_entry_is_handed_over() (+21 more)

### Community 96 - "tests/events.rs"
Cohesion: 0.23
Nodes (14): a_commit_reaches_the_socket(), a_notification_crosses_replicas(), a_socket_closes_when_its_caller_loses_the_vault(), a_ticket_flood_is_limited_per_address(), expired_ticket_is_refused(), next(), notice_has_no_path(), replica() (+6 more)

### Community 97 - "auth/keys.rs"
Cohesion: 0.14
Nodes (10): Cache, decode_set(), Discovery, Http, MAX_AGE, MAX_KEYS, provider(), REFETCH_INTERVAL (+2 more)

### Community 98 - "RFC 0003 — Web console (front and back office) and notifications"
Cohesion: 0.17
Nodes (12): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 1. Summary, 2. Motivation, 3. Goals / non-goals (+4 more)

### Community 99 - "task"
Cohesion: 0.15
Nodes (16): cmd, for, task, cmds, cmds, desc, cmds, desc (+8 more)

### Community 100 - "tests/config.rs"
Cohesion: 0.28
Nodes (17): a_note_sharing_bytes_with_hidden_configuration_stays_readable(), a_web_device_lists_and_reads_only_what_holds_no_secret(), config_namespace_hidden_from_other_member(), configuration_swap_in_one_commit(), configuration_writes_last_wins_without_conflict_files(), console_writes_nothing(), content_of_viewable_only(), D (+9 more)

### Community 101 - "app/tests.rs"
Cohesion: 0.26
Nodes (9): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp(), state() (+1 more)

### Community 102 - "files.rs"
Cohesion: 0.22
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

### Community 106 - "app/sync_state.rs"
Cohesion: 0.17
Nodes (11): SyncCounts, caught_up(), COUNT_MAX, DeviceReport, DeviceSync, my_device_vaults(), pushed(), put_report() (+3 more)

### Community 107 - "kura-server/src/main.rs"
Cohesion: 0.17
Nodes (17): backup(), check_chunks(), CONFIG_DIR_ENV, connect(), DATABASE_URL_ENV, dump_spec(), init_tracing(), main() (+9 more)

### Community 108 - "tests/merge.rs"
Cohesion: 0.36
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
Cohesion: 0.26
Nodes (20): a_crowded_name_still_gets_a_unique_one(), a_path_freed_and_taken_in_one_commit(), a_pull_during_commits_never_passes_an_unseen_change(), blob(), changes_page_by_seq_and_refuse_a_lost_history(), commit_with_foreign_entry_id_is_refused(), concurrent_commits_never_skip_a_seq(), diverging_edits_become_a_conflict_file() (+12 more)

### Community 114 - "ObsidianHost"
Cohesion: 0.06
Nodes (10): RFC-0002, Adapter, core, RFC-0002, Entry, FakeServer, json(), MemAdapter (+2 more)

### Community 115 - "cli.rs"
Cohesion: 0.20
Nodes (21): a_long_absence_resyncs_and_history_restores(), a_rewound_server_gets_everything_back(), a_scoped_token_syncs_its_folders_and_refuses_the_rest(), a_write_token_refuses_what_leaves_its_folder(), an_expired_access_token_is_renewed_mid_run(), apply_docs(), code(), dir() (+13 more)

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.09
Nodes (23): 10. The Obsidian plugin, 11. Local stack, 12. Maintenance, backups and restore, 13. Installing with Helm, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults (+15 more)

### Community 118 - "Serialize"
Cohesion: 0.22
Nodes (3): DnsLabel, NameError, String

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 120 - "compilerOptions"
Cohesion: 0.09
Nodes (22): compilerOptions, allowImportingTsExtensions, forceConsistentCasingInFileNames, isolatedModules, jsx, lib, module, moduleResolution (+14 more)

### Community 121 - "Deserialize"
Cohesion: 0.13
Nodes (12): TenantRole, Admin, Member, Owner, admin_status(), AdminStatus, with_quota_conditions(), Me (+4 more)

### Community 123 - "BrowsePage.vue"
Cohesion: 0.03
Nodes (94): mermaid, asking, byName, canRetry, closeHistory(), copied, crumbs, cut (+86 more)

### Community 124 - "Divergences"
Cohesion: 0.13
Nodes (15): §8 additions (phase 2), §8 additions (phase 3), §8 additions (phase 5), §8 as built (phase 1), Divergences, Phase 1 eco-design review (2026-11-14), Phase 1 security review (2026-11-14), Phase 2 security review (2026-11-14) (+7 more)

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
Cohesion: 0.09
Nodes (26): ScopeMode, Read, Write, ScopeRule, check_cidrs(), check_folder(), check_name(), check_prefix() (+18 more)

### Community 134 - "reconcile.rs"
Cohesion: 0.18
Nodes (17): COUNTER, a_config_change_is_seen_before_the_poll(), a_failed_tick_is_retried_as_a_local_change(), an_older_replica_never_overwrites_newer_state(), apply(), count_state(), DIRS, invalid_document_keeps_last_config() (+9 more)

### Community 135 - "steps"
Cohesion: 0.08
Nodes (34): runs-on, steps, permissions, runs-on, steps, jobs, audit, image (+26 more)

### Community 136 - "jobs"
Cohesion: 0.29
Nodes (7): runs-on, jobs, check, plugin, ui, runs-on, runs-on

### Community 137 - "test"
Cohesion: 0.29
Nodes (7): KURA_TEST_DATABASE_URL, KURA_TEST_S3_ENDPOINT, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, test, env, runs-on

### Community 138 - "s3"
Cohesion: 0.29
Nodes (7): RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY, env, image, ports, s3, services

### Community 139 - "lint"
Cohesion: 0.29
Nodes (7): task, cmds, desc, cmds, desc, lint, recu

### Community 140 - "changes.rs"
Cohesion: 0.15
Nodes (17): ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE, query() (+9 more)

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
Cohesion: 0.02
Nodes (113): RFC-3339, 11. Other concerns, vue, vue-i18n, emit, { t }, { t }, box (+105 more)

### Community 145 - "deploy.yaml"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, tasks, alerts:test, helm:lint, vars (+3 more)

### Community 146 - "biome.json"
Cohesion: 0.12
Nodes (16): css, parser, files, includes, formatter, indentStyle, linter, rules (+8 more)

### Community 147 - "PrefsMenu.vue"
Cohesion: 0.12
Nodes (27): announced, close(), id, localeNote, locales, open, outside(), panel (+19 more)

### Community 148 - "vaults.rs"
Cohesion: 0.13
Nodes (27): BindingRow, BindingView, ChunkingView, create_vault(), CreateVault, delete_binding(), delete_vault(), folder_pattern() (+19 more)

### Community 149 - "api"
Cohesion: 0.67
Nodes (3): flatten, taskfile, api

### Community 150 - "notifier.rs"
Cohesion: 0.06
Nodes (37): BACKOFF, classify(), context(), deliver(), EVENTS, Expiring, Fact<'a>, hourly() (+29 more)

### Community 151 - "tenants.yaml"
Cohesion: 0.25
Nodes (7): KuraTenant dev, KuraVault notes, apiVersion, kind, metadata, apiVersion, kind

### Community 152 - "RFC 0006 — Sync state per device and per vault"
Cohesion: 0.17
Nodes (12): 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 17. References and sources, 1. Summary, 2. Motivation, 3. Goals / non-goals (+4 more)

### Community 153 - "chunk_stats.rs"
Cohesion: 0.27
Nodes (8): BATCH_BYTES, BATCH_RECORDS, batches(), files(), KIB, main(), pct(), SETTINGS

### Community 155 - "analyze"
Cohesion: 0.08
Nodes (26): if, permissions, runs-on, steps, strategy, jobs, analyze, language (+18 more)

### Community 156 - "crypto.rs"
Cohesion: 0.05
Nodes (41): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, CryptoError, Corrupt, KeyLength, Malformed, Random (+33 more)

### Community 157 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 158 - "HistoryPanel.vue"
Cohesion: 0.08
Nodes (29): RFC-0005, 18.1 What shipped, 18.2 Divergences from the text, 18.3 What the proof observed, 18.4 Threats against the tests, 18. Revision against the tree (2026-11-18), blobManifest(), call() (+21 more)

### Community 160 - "scripts"
Cohesion: 0.25
Nodes (8): scripts, build, dev, format, generate:client, lint, test, types:check

### Community 161 - "@playwright/test"
Cohesion: 0.22
Nodes (11): RFC-0008, @playwright/test, RFC-0003, RFC-0006, env(), signIn(), signOut(), RFC-0003 (+3 more)

### Community 162 - "6. Detailed design"
Cohesion: 0.29
Nodes (8): 6.1 Data model (migration `0001_init.sql` since the squash, formerly `0027_device_sync.sql`), 6.2 `app/changes.rs`, 6.3 `app/commits.rs`, 6.4 `app/sync_state.rs` (new), 6.5 `ui/`, 6. Detailed design, 6.P Proof against a real client, Device

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

### Community 167 - "ScopeError"
Cohesion: 0.28
Nodes (9): check_folder(), check_rules(), ScopeError, Both, Count, NotAFolder, Path, Twice (+1 more)

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
Cohesion: 0.24
Nodes (5): Attached, Config, Credentials, set_refresh_token(), write_private()

### Community 172 - "Chart.yaml"
Cohesion: 0.29
Nodes (6): apiVersion, appVersion, description, name, type, version

### Community 173 - "problem"
Cohesion: 0.12
Nodes (23): env_stored(), vault_id(), who(), a_folder_is_an_entry_older_clients_never_see(), folders_follow_the_scope_of_notes(), a_device_reports_its_last_run_as_counts(), a_pass_records_where_the_device_stands(), a_stranger_gets_404() (+15 more)

### Community 174 - "limits.rs"
Cohesion: 0.21
Nodes (7): counts_per_key_and_route_and_says_when_to_retry(), limit_of(), LIMITS, MAX_TRACKED, RateLimiter, WINDOW, WindowKey

### Community 175 - "VaultPath"
Cohesion: 0.07
Nodes (15): a_255_byte_name_is_written(), err(), FsHost, mtime(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp() (+7 more)

### Community 176 - "RFC 0005 — Folder-restricted viewers and the web reader"
Cohesion: 0.15
Nodes (13): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 17. References and sources, 1. Summary, 2. Motivation (+5 more)

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

### Community 181 - "tests/chunks.rs"
Cohesion: 0.18
Nodes (22): a_failed_commit_leaves_the_object_for_the_sweep(), batches_answer_in_order_within_their_budget(), bucket_object_is_not_plaintext(), commit_blob(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422(), manifests_are_stored_all_or_none(), objects() (+14 more)

### Community 182 - "image"
Cohesion: 0.40
Nodes (5): image, digest, pullPolicy, repository, tag

### Community 183 - "snapshot.rs"
Cohesion: 0.39
Nodes (3): load_if_changed(), REFRESH, run()

### Community 184 - "remote.ts"
Cohesion: 0.15
Nodes (11): RFC-0006, Sleep, Api, ApiOptions, CLIENT, Fetch, problem(), retryAfter() (+3 more)

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

### Community 191 - "18. Revision against the tree (2026-11-17)"
Cohesion: 0.29
Nodes (7): 18.1 What shipped, 18.2 Where it differs from the text, 18.3 What the proof observed, 18.4 §7, the tests that prove each row, 18.5 §8, supply chain, 18.6 §4.5, mocks against what shipped, 18. Revision against the tree (2026-11-17)

### Community 192 - "history.rs"
Cohesion: 0.10
Nodes (26): a_pass_from_zero_pages_past_the_horizon(), eid(), epochs_change_once_with_the_cluster_identity(), history_and_restore(), purged_deletions_move_the_horizon(), retention_prunes_history_but_never_a_head(), S, setup() (+18 more)

### Community 193 - "wire.rs"
Cohesion: 0.09
Nodes (24): Author, ChangesResponse, commit_bodies_parse_strictly(), eid(), entry_ids_are_uuid_v7(), EntryId, EntryState, Operation (+16 more)

### Community 194 - "android.sh"
Cohesion: 0.52
Nodes (6): adb(), ANDROID_HOME, booted(), sdk(), android.sh script, wait_boot()

### Community 197 - "reader.sh"
Cohesion: 0.33
Nodes (10): api(), as(), chunk_of(), cleanup(), entry(), fail(), hits(), pass() (+2 more)

### Community 198 - "Keys"
Cohesion: 0.29
Nodes (3): Keys, KeySource, Fixed

### Community 199 - "bindings"
Cohesion: 0.17
Nodes (16): claimValue, paths, role, categories, KuraVault campaign, apiVersion, kind, spec (+8 more)

### Community 200 - "overrides"
Cohesion: 0.29
Nodes (6): auditConfig, ignoreGhsas, overrides, glob, js-yaml, minimatch

### Community 201 - "test_db"
Cohesion: 0.26
Nodes (12): test_db(), a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls(), jwks_survive_a_restart_without_the_provider(), pooled_connection_does_not_leak_tenant() (+4 more)

### Community 202 - "Api"
Cohesion: 0.24
Nodes (9): confirm(), remote_entries(), run_sync(), sync_config(), token(), token_create(), token_list(), token_scopes() (+1 more)

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

### Community 210 - "kura-core/src/config.rs"
Cohesion: 0.39
Nodes (8): categories_never_include_what_stays_on_the_device(), category(), like_patterns_cover_viewable(), only_files_without_secrets_are_viewable(), plugin_of(), synced(), viewable(), VIEWABLE_LIKE

### Community 212 - "connect_and_watch"
Cohesion: 0.22
Nodes (10): connect_and_watch(), fingerprint(), on_socket(), Socketed, Reconnect, Stop, watch(), Watched (+2 more)

### Community 213 - "AppState"
Cohesion: 0.13
Nodes (5): AppState, HttpSettings, ConfigError, Invalid, Read

### Community 214 - "types.ts"
Cohesion: 0.09
Nodes (18): Controller, ControllerOptions, QUIET_MS, ProblemsModal, Situation, count(), deletionsToConfirm(), isProblem() (+10 more)

### Community 215 - "PaneResizer.vue"
Cohesion: 0.29
Nodes (9): clamp(), emit, key(), move(), props, remPx(), reset(), up() (+1 more)

### Community 216 - "test"
Cohesion: 0.67
Nodes (3): test, flatten, taskfile

### Community 217 - "6. Detailed design"
Cohesion: 0.25
Nodes (8): 6.1 Data model (migration `0001_init.sql` since the squash, formerly `0020_token_scopes.sql`), 6.2 `authz::PathScope` and the read side, 6.3 `Writer`, in `app/reader.rs`, 6.4 What a token may name: possession, 6.5 The commit, for a token, 6.6 `kura-cli`, 6. Detailed design, 6.P Proof against a real client

### Community 218 - "dev:certs:renew"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:certs:renew

### Community 219 - "app"
Cohesion: 0.17
Nodes (8): app(), app_router(), cors(), openapi_spec(), SecurityAddon, Background, background(), serve()

### Community 220 - "token.rs"
Cohesion: 0.17
Nodes (7): hex_digit(), PREFIX, ScopedToken, SECRET_BYTES, TokenError, Malformed, tokens_round_trip_and_parse_strictly()

### Community 221 - "KuraTenant other"
Cohesion: 0.50
Nodes (4): KuraTenant other, apiVersion, kind, metadata

### Community 223 - "notify.rs"
Cohesion: 0.10
Nodes (22): CHANNEL, check_epoch(), CONFIG_CHANNEL, connect_listener(), Event, Change, Pull, Revoked (+14 more)

### Community 224 - "18. Revision against the tree (2026-11-18)"
Cohesion: 0.22
Nodes (9): 18.1 What shipped, 18.2 Divergences from the text, 18.3 What the proof observed, 18.4 Threats against the tests, 18.5 Supply chain, 18.6 Mocks against what shipped, 18.7 Phase 3 and the proofs, landed the same day, 18.8 Owners by name, and the browser proof (supersedes D13) (+1 more)

### Community 225 - "duration"
Cohesion: 0.14
Nodes (7): MAX_RETRIES, s3(), Storage, StorageError, TIMEOUT, BucketSecret, rotate_bucket()

### Community 226 - "config-view.sh"
Cohesion: 0.40
Nodes (10): api(), as(), cleanup(), entry_of(), fail(), hash_of(), pass(), config-view.sh script (+2 more)

### Community 227 - "18. Revision against the tree (2026-11-17)"
Cohesion: 0.25
Nodes (8): 18.1 What shipped, 18.2 Where the tree differs from the text, 18.3 What the proof observed, 18.4 §7 threats and the tests that prove them, 18.5 Supply chain as shipped, 18.6 The mocks of §4.5 against the console as built, 18.7 Gaps closed on landing (2026-11-17), 18. Revision against the tree (2026-11-17)

### Community 228 - "networkPolicy"
Cohesion: 0.50
Nodes (4): networkPolicy, enabled, ingressNamespace, monitoringNamespace

### Community 229 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

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

### Community 236 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 237 - "18. Revision against the tree (2026-11-17)"
Cohesion: 0.25
Nodes (8): 18.1 What shipped, 18.2 Divergences from the text, 18.3 What the proof observed, 18.4 Threats against the tests, 18.5 Supply chain, 18.6 Mocks against what shipped, 18.7 Header, 18. Revision against the tree (2026-11-17)

### Community 238 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 239 - "6. Detailed design"
Cohesion: 0.29
Nodes (7): 6.1 `kura-api`, 6.2 Data model (migration `0001_init.sql` since the squash, formerly `0021_binding_paths.sql`), 6.3 `authz`, 6.4 Server routes that take a person but not a `Reader`, 6.5 The reader (`ui/`, front office of RFC 0003), 6. Detailed design, 6.P Proof against a real client

### Community 240 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 241 - "modals.ts"
Cohesion: 0.11
Nodes (11): DeviceCode, Ask, Button, CodeModal, ConfirmModal, PluginModal, SetupModal, SwitchModal (+3 more)

### Community 243 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 244 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 245 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 246 - "5. Architecture"
Cohesion: 0.50
Nodes (4): 5.1 Components, 5.2 Sign-in, 5.3 From a fact to a webhook, 5. Architecture

### Community 247 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 249 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 250 - "claimMappings"
Cohesion: 0.13
Nodes (17): claim, roles, spec, spec, spec, maxFileSize, storage, admin (+9 more)

### Community 251 - ".fill"
Cohesion: 0.10
Nodes (7): ChunkUploads, lock(), Manifest, Prefetch, Prefetch<'a, R>, Prefetch<'_, R>, upload_manifests()

### Community 252 - "Algorithm"
Cohesion: 0.18
Nodes (11): Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512, RS256 (+3 more)

### Community 253 - "src/merge.rs"
Cohesion: 0.27
Nodes (10): edits_of_different_lines_merge(), MAX_MERGE, MAX_MERGE_LINES, Merge, merge3(), Clean, Conflict, mergeable() (+2 more)

### Community 255 - "1. Phase 1 — scaffold, configuration, identity, authorization"
Cohesion: 0.10
Nodes (21): 0. Before the first line of code, 1. Phase 1 — scaffold, configuration, identity, authorization, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Phase 4 — merge, notifications, plugin, 5. Phase 5 — history, retention, resync, 6. Phase 6 — operations, 7. Phase 7 — scoped tokens and search (+13 more)

### Community 259 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 260 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 261 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 262 - "5. Architecture"
Cohesion: 0.50
Nodes (4): 5.1 The scope of a person, 5.2 One filter for tokens and people, 5.3 Reading a note in the browser, 5. Architecture

### Community 263 - "notify.sh"
Cohesion: 0.50
Nodes (7): api(), cleanup(), fail(), pass(), serve(), notify.sh script, sql()

### Community 264 - "weebo-kura"
Cohesion: 0.40
Nodes (5): Documentation, Getting started, License, Setup, weebo-kura

### Community 265 - "Security Policy"
Cohesion: 0.25
Nodes (7): Disclosure, Reporting a vulnerability, Safe harbour, Scope, Security Policy, Supported versions, What happens next

### Community 266 - "5. Architecture"
Cohesion: 0.67
Nodes (3): 5.1 Where the fact comes from, 5.2 Who sees which row, 5. Architecture

### Community 268 - "scalar.rs"
Cohesion: 0.36
Nodes (5): API_DOCS_CSP, scalar(), SCALAR_BUNDLE_PATH, scalar_unavailable_html(), the_reference_loads_nothing_from_a_third_party()

### Community 270 - "dockerfile"
Cohesion: 0.29
Nodes (6): autodiscovery, crawlers, dockerfile, digest, filematch, name

### Community 272 - "6. Detailed design"
Cohesion: 0.29
Nodes (7): 6.1 `ui/` and how it is served, 6.2 Sign-in and the web device, 6.3 New read routes, 6.4 Notifications (the migration, formerly `0024_notifications.sql`), 6.5 The guarded client (`net.rs`), 6. Detailed design, 6.P Proof against a real client

### Community 275 - "KuraVault suite-config"
Cohesion: 0.50
Nodes (4): KuraVault suite-config, apiVersion, kind, metadata

### Community 316 - "AuthError"
Cohesion: 0.18
Nodes (6): AuthError, Invalid, Jwt, Provider, crate::problem::ApiError, BearerToken

### Community 317 - "sim.rs"
Cohesion: 0.14
Nodes (8): both_batch_routes_carried_chunks(), Device, devices_converge_and_lose_nothing(), edit_notes(), MemHost, no_line_is_lost(), Rng, run()

### Community 320 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 322 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

### Community 326 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 332 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 334 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

## Knowledge Gaps
- **204 isolated node(s):** `Db`, `Objects`, `NoStorage`, `LockLost`, `Admin` (+199 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 2246 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **98 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `RFC 0003 — Web console (front and back office) and notifications` connect `RFC 0003 — Web console (front and back office) and notifications` to `15. Decisions and open questions`, `ui.rs`, `18. Revision against the tree (2026-11-17)`, `rfc0001-implementation-plan.md`, `7. Security`, `6. Detailed design`, `10. Operability`, `4. User-facing design`, `5. Architecture`?**
  _High betweenness centrality (0.159) - this node is a cross-community bridge._
- **Are the 7 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 7 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Db`, `Objects`, `NoStorage` to the rest of the system?**
  _204 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `VaultAccess` be split into smaller, more focused modules?**
  _Cohesion score 0.1125886524822695 - nodes in this community are weakly interconnected._
- **Why does `17. References and sources` connect `ui.rs` to `RFC 0003 — Web console (front and back office) and notifications`?**
  _High betweenness centrality (0.153) - this node is a cross-community bridge._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09176788124156546 - nodes in this community are weakly interconnected._
- **Should `common/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.13970588235294118 - nodes in this community are weakly interconnected._