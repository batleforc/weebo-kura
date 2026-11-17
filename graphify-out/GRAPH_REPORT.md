# Graph Report - weebo-kura  (2026-11-17)

## Corpus Check
- 311 files · ~277,279 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 23 file(s) not represented in the graph (top: (none) 8, .toml 7, .css 3)

## Summary
- 4548 nodes · 9954 edges · 269 communities (222 shown, 47 thin omitted)
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 475 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `62cdb46d`
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
- KeysPage.vue
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
- backup.rs
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
- tenantRef
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
- fs.rs
- KuraPlugin
- api.rs
- http.rs
- parse.rs
- commands.rs
- main.ts
- tests/events.rs
- auth/mod.rs
- Hub
- net.rs
- tasks
- session.ts
- oidc
- Uuid
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
- store/keys.rs
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
- sim.rs
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
- .from
- api
- TenantVaultsPage.vue
- spec
- router.ts
- Algorithm
- wasm.d.ts
- AppState
- crypto.rs
- rules
- Env
- problem
- scripts
- @playwright/test
- .fill
- rules
- limits.rs
- server
- vault
- src/maintenance.rs
- backup
- cnpg
- Documentation
- kura-cli/src/config.rs
- Chart.yaml
- tests/maintenance.rs
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
- databaseUrlSecret
- drill.sh
- run.sh
- deploy
- dev
- perf
- rust
- .object_key
- MaintenanceError
- wire.rs
- android.sh
- soak.sh
- reader.sh
- connect_and_watch
- bindings
- overrides
- client.ts
- AccessPage.vue
- authz.sh
- tenants.yaml
- coverage
- plugin
- console.sh
- dev:certs
- ui
- body.rs
- sw.js
- 4. User-facing design
- serve.rs
- 6. Detailed design
- DevicesPage.vue
- test
- DnsLabel
- dev:certs:renew
- serve
- src/merge.rs
- KuraVault notes
- 7. Security
- 10. Operability
- remote_entries
- 15. Decisions and open questions
- core.ts
- PluginModal
- 4. User-facing design
- vite.config.ts
- name
- CodeModal
- 5. Architecture
- roles
- KuraTenant dev
- KuraTenant other
- Mem
- device.rs
- src/vault.rs
- scalar.rs
- serve
- 6. Detailed design
- 7. Security
- notify.sh
- Notice
- Event
- e2e/reader.spec.ts
- notify_change
- @hey-api/openapi-ts

## God Nodes (most connected - your core abstractions)
1. `ApiError` - 161 edges
2. `SharedState` - 125 edges
3. `ChunkHash` - 111 edges
4. `RemoteError` - 93 edges
5. `VaultPath` - 74 edges
6. `HostError` - 51 edges
7. `Env` - 50 edges
8. `ProblemType` - 49 edges
9. `AppState` - 48 edges
10. `blob_hash()` - 46 edges

## Surprising Connections (you probably didn't know these)
- `6.4 Notifications (migration `0023_notifications.sql`)` --references--> `run()`  [INFERRED]
  docs/RFC/rfc0003-web-console-and-notifications.md → ui/src/pages/SearchPage.vue
- `Progress` --references--> `PluginGuard`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/plugins.ts
- `Progress` --references--> `ProblemError`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/types.ts
- `Progress` --references--> `ConfirmModal`  [INFERRED]
  docs/RFC/rfc0001-implementation-plan.md → plugin/src/main.ts
- `6.P Proof against a real client` --references--> `open()`  [INFERRED]
  docs/RFC/rfc0002-batched-transfers.md → ui/src/pages/BrowsePage.vue

## Import Cycles
- 1-file cycle: `crates/kura-server/src/authz.rs -> crates/kura-server/src/authz.rs`
- 2-file cycle: `crates/kura-server/src/auth/keys.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/src/auth/keys.rs`
- 2-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/app/mod.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/auth/mod.rs -> crates/kura-server/tests/common/app.rs -> crates/kura-server/src/app/mod.rs`
- 3-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`
- 4-file cycle: `crates/kura-server/src/app/mod.rs -> crates/kura-server/src/reconcile/snapshot.rs -> crates/kura-server/src/reconcile/status.rs -> crates/kura-server/src/reconcile/mod.rs -> crates/kura-server/src/app/mod.rs`

## Communities (269 total, 47 thin omitted)

### Community 0 - "authz.rs"
Cohesion: 0.10
Nodes (31): TenantRole, Admin, Member, Owner, VaultRole, Editor, Manager, Viewer (+23 more)

### Community 1 - "reconcile/mod.rs"
Cohesion: 0.10
Nodes (24): ConfigFiles, bytes(), check(), json(), load_previous(), Outcome, Applied, NotWritten (+16 more)

### Community 2 - "common/mod.rs"
Cohesion: 0.11
Nodes (5): admin_url(), COUNTER, ISSUER, ROLE_PASSWORD, TestKey

### Community 3 - "includes"
Cohesion: 0.20
Nodes (10): includes, mise, obsidian, security, skills, template, flatten, taskfile (+2 more)

### Community 4 - "ProblemType"
Cohesion: 0.07
Nodes (31): Problem, RateLimited, Unavailable, body(), every_type_renders_its_status_and_uri(), Problem, problem_response(), ProblemType (+23 more)

### Community 5 - "Env"
Cohesion: 0.10
Nodes (8): Captured, two_replicas_creating_one_data_key_keep_one(), vault(), admin(), config(), reads_database_and_bucket_credentials_from_kv(), refuses_a_bad_address_and_a_bad_token(), server_policy_cannot_delete_key()

### Community 6 - "kura-server/src/config.rs"
Cohesion: 0.11
Nodes (21): alg_none_cannot_be_configured(), ClaimMatch, CorsConfig, discovery_is_optional_and_https(), err(), EXAMPLE, issuer_must_be_https(), missing_file_is_a_read_error() (+13 more)

### Community 7 - "rfc0001-implementation-plan.md"
Cohesion: 0.10
Nodes (14): Divergences, Progress, RFC 0002 — implementation plan, Divergences, Progress, RFC 0003 — implementation plan, Divergences, Progress (+6 more)

### Community 8 - "steps"
Cohesion: 0.23
Nodes (12): steps, BASE, steps, steps, env, if, name, run (+4 more)

### Community 9 - "Duration"
Cohesion: 0.08
Nodes (12): digits(), Duration, Quantity, QuantityRepr, Bytes, Text, ScalarError, Duration (+4 more)

### Community 10 - "rules"
Cohesion: 0.07
Nodes (29): extends, description, given, message, severity, then, overrides, files (+21 more)

### Community 11 - "app/notifications.rs"
Cohesion: 0.16
Nodes (30): DeviceCaller, authorize(), Channel, check_authorization(), check_events(), check_name(), check_url(), create_rule() (+22 more)

### Community 12 - "vars"
Cohesion: 0.08
Nodes (26): cmds, desc, preconditions, cmds, deps, desc, cmds, desc (+18 more)

### Community 13 - "validate/tests.rs"
Cohesion: 0.17
Nodes (27): parse_file(), validate(), binding_names_exactly_one_subject(), binding_paths_are_checked(), chunking_cannot_change_after_creation(), chunking_must_be_ordered_and_at_most_4_mib(), chunking_must_fit_the_algorithms_bounds(), claim_value_mapped_by_two_tenants_is_refused() (+19 more)

### Community 14 - "ChunkParams"
Cohesion: 0.17
Nodes (14): an_edit_in_the_middle_keeps_most_chunks(), blob_hashes_bind_size_and_order(), ChunkError, Hash, Params, chunks(), ChunkSpan, hashes_round_trip_in_lowercase_hex_only() (+6 more)

### Community 15 - "spec.rs"
Cohesion: 0.09
Nodes (37): TenantSlug, ANNOTATIONS, Binding, Chunking, .DEFAULT, ClaimMappings, ConfigCategory, Hotkeys (+29 more)

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
Cohesion: 0.05
Nodes (42): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+34 more)

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
Cohesion: 0.11
Nodes (12): ReasonCode, .SUCCESS, categories_never_include_what_stays_on_the_device(), category(), plugin_of(), synced(), Removed, build() (+4 more)

### Community 30 - "kura-wasm/src/lib.rs"
Cohesion: 0.11
Nodes (29): bytes(), changes(), commit(), File, get_blob(), get_blobs(), get_chunk(), get_chunk_records() (+21 more)

### Community 31 - "fuzz.yml"
Cohesion: 0.13
Nodes (14): runs-on, steps, jobs, fuzz, name, on, schedule, workflow_dispatch (+6 more)

### Community 32 - "tasks"
Cohesion: 0.14
Nodes (14): task, cmds, internal, cmds, desc, cmds, desc, tasks (+6 more)

### Community 33 - "tasks"
Cohesion: 0.05
Nodes (36): cmd, for, var, cmds, desc, cmds, desc, requires (+28 more)

### Community 34 - "KeysPage.vue"
Cohesion: 0.09
Nodes (13): cidrs, confirming, copied, creating, {
  data: tokens,
  loading,
  error,
  forbidden,
  load,
}, days, name, refusal (+5 more)

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
Nodes (37): Body, batch_blobs(), batch_chunks(), BATCH_FETCHES, batch_hashes(), BATCHES_IN_FLIGHT, BlobBatchRequest, BlobBatchResponse (+29 more)

### Community 39 - "RFC 0001 — weebo-kura: self-hosted sync backend for Obsidian vaults"
Cohesion: 0.17
Nodes (12): 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan, 16. Implementation phases, 1. Summary, 2. Motivation, 3. Goals / non-goals (+4 more)

### Community 40 - "tasks"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, cmds, desc, tasks, install (+3 more)

### Community 41 - "app/search.rs"
Cohesion: 0.14
Nodes (18): BATCH, cut(), index_one(), indexable(), markdown_only_cut_on_a_character(), MAX_INDEXED, MAX_QUERY, MAX_READ (+10 more)

### Community 42 - "RFC 0004 — Scoped tokens: several folders, and write access"
Cohesion: 0.08
Nodes (24): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+16 more)

### Community 43 - "helpers.js"
Cohesion: 0.17
Nodes (19): BASE_URL, BLOB, SIZE, TOKEN, VAULT, CLIENT, headers(), pullAll() (+11 more)

### Community 44 - "Vault"
Cohesion: 0.18
Nodes (8): client_err(), decode_b64(), Vault, VaultError, Address, Client, Path, ServiceAccount

### Community 45 - "openapi_contract.rs"
Cohesion: 0.29
Nodes (7): every_success_response_declares_its_body(), Exemptions, MIN_ROUTES, NO_BODY, success_responses(), the_guard_reports_a_bodiless_response_and_a_stale_exemption(), violations()

### Community 46 - "devices.rs"
Cohesion: 0.09
Nodes (19): Caller, needed_claims(), Device, device_event(), device(), list_devices(), MAX_DEVICES, members() (+11 more)

### Community 47 - "types.ts"
Cohesion: 0.12
Nodes (16): Controller, ControllerOptions, QUIET_MS, Situation, count(), deletionsToConfirm(), isProblem(), pairs() (+8 more)

### Community 48 - "HostError"
Cohesion: 0.08
Nodes (8): err(), FsHost, VaultPath, FileMeta, HostError, MemHost, HostAdapter, meta()

### Community 49 - "vaults.rs"
Cohesion: 0.16
Nodes (21): BindingRow, BindingView, ChunkingView, create_vault(), CreateVault, delete_binding(), delete_vault(), list_bindings() (+13 more)

### Community 50 - "metrics.rs"
Cohesion: 0.15
Nodes (6): Metrics, metrics_are_exported_with_the_kura_prefix(), refresh_storage(), render(), router(), storage_once()

### Community 51 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 52 - "devfile.yaml"
Cohesion: 0.33
Nodes (5): metadata, name, parent, uri, schemaVersion

### Community 53 - "backup.rs"
Cohesion: 0.15
Nodes (14): BackupError, Busy, Db, LockLost, NoStorage, Objects, Other, check_chunks() (+6 more)

### Community 54 - "src/sync.rs"
Cohesion: 0.10
Nodes (39): Applied, Deferred, Done, apply(), BATCH, blob_of(), COMMIT_ATTEMPTS, download() (+31 more)

### Community 55 - "token.rs"
Cohesion: 0.19
Nodes (7): hex_digit(), PREFIX, ScopedToken, SECRET_BYTES, TokenError, Malformed, tokens_round_trip_and_parse_strictly()

### Community 56 - "attrs"
Cohesion: 0.04
Nodes (45): access_code_validity, access_token_validity, authentication, authorization_flow, certificate_data, client_id, client_type, default (+37 more)

### Community 57 - "ChunkHash"
Cohesion: 0.08
Nodes (10): ChunkHash, lock(), Prefetch<'_, R>, Remote, RemoteError, Problem, Scripted, decode() (+2 more)

### Community 58 - "path.rs"
Cohesion: 0.07
Nodes (26): allowed_char(), check(), ignorable(), INVALID_PATH, MAX_EXTENSION, MAX_PATH, MAX_SEGMENT, MAX_SUFFIX (+18 more)

### Community 59 - "kura-api"
Cohesion: 0.40
Nodes (6): kura-api, kura-cli, kura-core, kura-fuzz, kura-server, kura-wasm

### Community 64 - "Fail"
Cohesion: 0.31
Nodes (21): attach(), config(), confirm(), Ctx, devices(), entry_of(), Fail, fold() (+13 more)

### Community 67 - "tenantRef"
Cohesion: 0.20
Nodes (10): categories, KuraVault elsewhere, apiVersion, kind, spec, spec, spec, configSync (+2 more)

### Community 69 - "package.json"
Cohesion: 0.08
Nodes (30): RFC-0001, description, devDependencies, esbuild@0.28.2, eslint@10.14.0, @eslint/js@10.0.1, obsidian@1.14.4, @types/node@26.6.6 (+22 more)

### Community 70 - "tasks"
Cohesion: 0.06
Nodes (32): cmds, desc, dir, cmds, deps, desc, dir, cmds (+24 more)

### Community 71 - "tests/sync.rs"
Cohesion: 0.28
Nodes (20): a_crowded_name_still_gets_a_unique_one(), a_path_freed_and_taken_in_one_commit(), a_pull_during_commits_never_passes_an_unseen_change(), blob(), changes_page_by_seq_and_refuse_a_lost_history(), commit_with_foreign_entry_id_is_refused(), concurrent_commits_never_skip_a_seq(), diverging_edits_become_a_conflict_file() (+12 more)

### Community 72 - "NotificationsPage.vue"
Cohesion: 0.07
Nodes (37): withDevice(), messageOf(), problemType(), load(), {
  data: deliveries,
  loading,
  error,
  load,
}, load(), revoke(), load() (+29 more)

### Community 73 - "Notifier"
Cohesion: 0.14
Nodes (8): Notifier, NotifierOptions, POLL_MS, SILENCE_MS, SocketLike, Timers, FakeSocket, setup()

### Community 74 - "app/events.rs"
Cohesion: 0.09
Nodes (13): ACCESS_REVOKED, create_ticket(), DEVICE_REVOKED, events(), EventsQuery, PING, SLOW_CONSUMER, Ticket (+5 more)

### Community 75 - "ui/package.json"
Cohesion: 0.10
Nodes (25): @biomejs/biome, jsdom, tailwindcss, @types/markdown-it, vue-tsc, devDependencies, @biomejs/biome@^2.6.3, jsdom@^31.0.0 (+17 more)

### Community 76 - "auth.ts"
Cohesion: 0.11
Nodes (26): RFC-8628, AuthError, AuthFailure, call(), discover(), Discovery, fnv(), Http (+18 more)

### Community 77 - "notifier.rs"
Cohesion: 0.08
Nodes (28): BACKOFF, classify(), context(), deliver(), EVENTS, Fact, Fact<'a>, hourly() (+20 more)

### Community 78 - "fs.rs"
Cohesion: 0.33
Nodes (8): a_255_byte_name_is_written(), mtime(), never_follows_a_symbolic_link(), p(), STATE_DIR, tmp(), TRASH_DIR, writes_atomically_and_lists_notes_only()

### Community 79 - "KuraPlugin"
Cohesion: 0.27
Nodes (4): KuraPlugin, KuraSettingTab, label(), word()

### Community 80 - "api.rs"
Cohesion: 0.12
Nodes (28): a_405_is_an_older_server_and_a_404_is_not(), ATTEMPTS, Chunking, CLIENT, DeviceCode, discover(), Discovery, http() (+20 more)

### Community 81 - "http.rs"
Cohesion: 0.13
Nodes (28): problem(), vault_id(), a_web_device_never_gets_the_configuration_space(), bindings_need_manager(), me_shows_only_own_tenants(), the_console_is_served_with_its_headers(), web(), web_device_is_reused_per_sid() (+20 more)

### Community 82 - "parse.rs"
Cohesion: 0.15
Nodes (14): ConfigError, parse_document(), Resource, Tenant, Vault, Source, typed(), API_VERSION (+6 more)

### Community 83 - "commands.rs"
Cohesion: 0.13
Nodes (14): a_host_name_is_a_valid_device_name(), BACKOFF_MAX, BACKOFF_MIN, backoff_stays_between_one_and_sixty_seconds(), blob_of(), bytes_for_people(), Diff, diff() (+6 more)

### Community 84 - "main.ts"
Cohesion: 0.12
Nodes (22): Tokens, DEFAULTS, random16(), readSettings(), registry(), Settings, timers, RFC-0001 (+14 more)

### Community 85 - "tests/events.rs"
Cohesion: 0.24
Nodes (13): a_commit_reaches_the_socket(), a_notification_crosses_replicas(), a_socket_closes_when_its_caller_loses_the_vault(), a_ticket_flood_is_limited_per_address(), expired_ticket_is_refused(), next(), replica(), S (+5 more)

### Community 86 - "auth/mod.rs"
Cohesion: 0.09
Nodes (13): algorithm(), AuthError, Invalid, Jwt, Provider, crate::problem::ApiError, LEEWAY_SECS, MAX_TOKEN_BYTES (+5 more)

### Community 87 - "Hub"
Cohesion: 0.14
Nodes (12): CHANNEL, four_sockets_per_device_at_most(), HEARTBEAT, Hub, listen(), MAX_SOCKETS, notify_revoked(), payloads_become_events() (+4 more)

### Community 88 - "net.rs"
Cohesion: 0.12
Nodes (17): a_target_is_https_without_credentials(), ip(), MAX_URL, private_and_special_addresses_are_refused(), refused_by_default(), Target, TargetError, AddressRefused (+9 more)

### Community 89 - "tasks"
Cohesion: 0.07
Nodes (26): tasks, ui:audit, ui:build, ui:client, ui:install, ui:lint, ui:test, cmds (+18 more)

### Community 90 - "session.ts"
Cohesion: 0.07
Nodes (41): RFC-6749, RFC-7636, discover(), forgetDiscovery(), load(), ProviderMetadata, trim(), RFC-0003 (+33 more)

### Community 91 - "oidc"
Cohesion: 0.07
Nodes (30): mount, role, listen, api, metrics, notifications, enabled, webhook (+22 more)

### Community 92 - "Uuid"
Cohesion: 0.10
Nodes (44): OpResult, Raw, apply(), apply_config(), BaseRev, check_blobs(), Commit, Cx (+36 more)

### Community 93 - "tests/tokens.rs"
Cohesion: 0.20
Nodes (16): a_token_is_revoked_with_the_binding_that_allowed_it(), INSIDE, json_of(), multi_folder_token_reads_its_folders_only(), not_uploaded(), only_writer_routes_accept_tokens(), OUTSIDE, outside_folders() (+8 more)

### Community 94 - "compilerOptions"
Cohesion: 0.12
Nodes (15): compilerOptions, exactOptionalPropertyTypes, isolatedModules, lib, module, moduleResolution, noEmit, noFallthroughCasesInSwitch (+7 more)

### Community 95 - "engine.rs"
Cohesion: 0.26
Nodes (26): sync(), a_batched_pull_makes_no_single_reads_and_drops_forged_records(), a_change_of_case_lands_on_a_case_insensitive_disk(), a_conflict_on_an_open_note_fails_that_file_only(), a_copied_folder_starts_fresh(), a_known_file_grown_past_the_limit_is_not_deleted(), a_merged_push_writes_the_merged_head(), a_move_killed_before_the_old_file_went_trashes_it_on_restart() (+18 more)

### Community 96 - "tests/notifications.rs"
Cohesion: 0.21
Nodes (11): a_manager_sees_their_vaults_only_and_off_means_409(), a_rule_signs_its_deliveries_and_never_shows_its_secrets(), Admin, drain(), events_reach_their_rules_with_ids_only_and_retries_back_off(), Got, no_webhook_reaches_an_address_the_operator_did_not_allow(), Receiver (+3 more)

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
Cohesion: 0.36
Nodes (5): config_namespace_hidden_from_other_member(), configuration_writes_last_wins_without_conflict_files(), D, S, setup()

### Community 101 - "app/tests.rs"
Cohesion: 0.26
Nodes (9): a_client_below_its_minimum_is_told_to_upgrade(), call(), get(), healthz_always_answers_and_readyz_follows_the_config(), lazy_pool(), requires_a_token_and_the_platform_admin_claim(), scalar_is_served_with_its_own_csp(), state() (+1 more)

### Community 102 - "files.rs"
Cohesion: 0.30
Nodes (11): FilesError, Io, TooLarge, TooMany, generation_depends_on_names_and_contents(), MAX_FILE_BYTES, MAX_FILES, read_dir() (+3 more)

### Community 103 - "tasks"
Cohesion: 0.13
Nodes (15): desc, cmds, desc, cmds, desc, cmds, desc, cmds (+7 more)

### Community 104 - "auth/tests.rs"
Cohesion: 0.29
Nodes (16): a_retired_key_stops_verifying_after_max_age(), a_slow_fetch_does_not_hold_other_callers(), accepts_a_valid_access_token(), access_claims(), ISSUER, keys_and_userinfo_come_through_oidc_discovery(), missing_role_claims_come_from_userinfo_and_are_cached(), oidc() (+8 more)

### Community 105 - "Command"
Cohesion: 0.08
Nodes (20): DeviceAction, Revoke, TokenAction, Create, List, Revoke, Cli, Command (+12 more)

### Community 106 - "store/mod.rs"
Cohesion: 0.13
Nodes (7): MAX_RETRIES, s3(), StorageError, TIMEOUT, BucketSecret, DatabaseSecret, refresh_credentials()

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
Cohesion: 0.07
Nodes (24): ChangesQuery, EntriesQuery, get_changes(), get_entries(), Head, open(), PAGE, query() (+16 more)

### Community 114 - "ObsidianHost"
Cohesion: 0.15
Nodes (6): Progress, HostOptions, ObsidianHost, parent(), toBuffer(), host()

### Community 115 - "cli.rs"
Cohesion: 0.38
Nodes (15): a_long_absence_resyncs_and_history_restores(), a_rewound_server_gets_everything_back(), a_scoped_token_syncs_its_folders_and_refuses_the_rest(), an_expired_access_token_is_renewed_mid_run(), code(), fake_issuer(), files(), sign_in_by_device_code_and_refresh_with_rotation() (+7 more)

### Community 116 - "dev:network"
Cohesion: 0.40
Nodes (5): cmds, desc, internal, status, dev:network

### Community 117 - "Operating kura-server (phases 1 and 2)"
Cohesion: 0.10
Nodes (21): 10. The Obsidian plugin, 11. Local stack, 12. Maintenance, backups and restore, 13. Installing with Helm, 1. The process, 2. PostgreSQL, 3. `server.yaml`, 4. Tenants and vaults (+13 more)

### Community 118 - "VaultRemote"
Cohesion: 0.17
Nodes (5): body(), check(), exchange(), Token, VaultRemote

### Community 119 - "dev:env"
Cohesion: 0.50
Nodes (4): cmds, desc, status, dev:env

### Community 120 - "compilerOptions"
Cohesion: 0.09
Nodes (22): compilerOptions, allowImportingTsExtensions, forceConsistentCasingInFileNames, isolatedModules, jsx, lib, module, moduleResolution (+14 more)

### Community 121 - "store/keys.rs"
Cohesion: 0.19
Nodes (8): Cache, CACHE_SIZE, CACHE_TTL, DataKeys, Slot, Notifications, Vault, transit_key_name()

### Community 123 - "BrowsePage.vue"
Cohesion: 0.04
Nodes (61): dompurify, markdown-it, dompurify@^3.4.18, markdown-it@^14.3.3, doc, download, entries, entryId (+53 more)

### Community 124 - "Divergences"
Cohesion: 0.06
Nodes (36): 0. Before the first line of code, 1. Phase 1 — scaffold, configuration, identity, authorization, 2. Phase 2 — chunks, blobs, encryption, quota, 3. Phase 3 — changes, commits, kura-core, kura-cli, 4. Phase 4 — merge, notifications, plugin, 5. Phase 5 — history, retention, resync, 6. Phase 6 — operations, 7. Phase 7 — scoped tokens and search (+28 more)

### Community 125 - ".start"
Cohesion: 0.13
Nodes (13): Sleep, sleep(), Api, ApiOptions, CLIENT, Fetch, problem(), retryAfter() (+5 more)

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
Cohesion: 0.10
Nodes (30): categories_and_the_kura_plugin(), category(), resolve(), check_cidrs(), check_folder(), check_name(), check_prefix(), check_scopes() (+22 more)

### Community 134 - "test_db"
Cohesion: 0.10
Nodes (30): no_key_verifier(), test_db(), TestDb, a_chunk_cannot_claim_another_tenant(), app_role_is_not_owner(), definer_functions_return_only_what_they_name(), devices_are_isolated_by_subject(), every_table_forces_rls() (+22 more)

### Community 135 - "sim.rs"
Cohesion: 0.15
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
Cohesion: 0.40
Nodes (4): allowBuilds, esbuild, overrides, minimatch

### Community 144 - "ui.rs"
Cohesion: 0.15
Nodes (9): config(), ConsoleConfig, frame_hash(), headers(), origin(), router(), the_bridge_hash_is_read_strictly(), the_policy_names_the_identity_provider_only() (+1 more)

### Community 145 - "deploy.yaml"
Cohesion: 0.17
Nodes (11): cmds, desc, cmds, desc, tasks, alerts:test, helm:lint, vars (+3 more)

### Community 146 - "biome.json"
Cohesion: 0.12
Nodes (16): css, parser, files, includes, formatter, indentStyle, linter, rules (+8 more)

### Community 149 - "api"
Cohesion: 0.67
Nodes (3): flatten, taskfile, api

### Community 150 - "TenantVaultsPage.vue"
Cohesion: 0.05
Nodes (38): vue, vue-i18n, emit, { t }, anything(), Targets, targets(), RFC-0003 (+30 more)

### Community 151 - "spec"
Cohesion: 0.22
Nodes (10): claim, spec, spec, maxFileSize, storage, write, claimMappings, quota (+2 more)

### Community 152 - "router.ts"
Cohesion: 0.07
Nodes (26): RFC-0005, pinia, vue-router, @vue/test-utils, dependencies, pinia@^4.1.0, vue@^3.5.45, vue-i18n@^11.4.18 (+18 more)

### Community 153 - "Algorithm"
Cohesion: 0.18
Nodes (11): Algorithm, EdDSA, ES256, ES384, PS256, PS384, PS512, RS256 (+3 more)

### Community 155 - "AppState"
Cohesion: 0.13
Nodes (5): AppState, HttpSettings, ConfigError, Invalid, Read

### Community 156 - "crypto.rs"
Cohesion: 0.12
Nodes (22): a_chunk_opens_only_at_its_own_address_vault_and_key(), aad(), CODEC_NONE, CryptoError, Corrupt, KeyLength, Malformed, Random (+14 more)

### Community 157 - "rules"
Cohesion: 0.18
Nodes (10): summary, groups, name, rules, severity, alert, annotations, expr (+2 more)

### Community 158 - "Env"
Cohesion: 0.37
Nodes (10): Env, env(), a_restricted_viewer_reads_its_folders_only(), apply(), config(), file(), folders_are_a_viewers_only(), paths() (+2 more)

### Community 159 - "problem"
Cohesion: 0.19
Nodes (8): admin_status(), AdminStatus, with_quota_conditions(), Me, me(), tenant_role(), TenantMembership, ObjectStatus

### Community 160 - "scripts"
Cohesion: 0.25
Nodes (8): scripts, build, dev, format, generate:client, lint, test, types:check

### Community 161 - "@playwright/test"
Cohesion: 0.36
Nodes (6): @playwright/test, RFC-0003, env(), signIn(), RFC-0003, RFC-0005

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

### Community 167 - "src/maintenance.rs"
Cohesion: 0.16
Nodes (13): MaintenanceConfig, BATCH, DELETED_ENTRY_TTL_HOURS, MAX_HOURS, pass(), PASS_LOCK, PAUSE, Report (+5 more)

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
Cohesion: 0.19
Nodes (8): access_token(), login(), Attached, Config, Credentials, refresh_token(), set_refresh_token(), write_private()

### Community 172 - "Chart.yaml"
Cohesion: 0.29
Nodes (6): apiVersion, appVersion, description, name, type, version

### Community 173 - "tests/maintenance.rs"
Cohesion: 0.21
Nodes (9): a_blob_put_and_collection_never_interleave(), backup_copies_new_objects_and_the_drill_check_catches_corruption(), collection_removes_only_what_nothing_references(), orphans_are_swept_and_the_backup_lock_pauses_deletion(), purge_keeps_objects_uploaded_after_its_rows_went(), restored_document_cancels_deletion(), S, setup() (+1 more)

### Community 174 - "sim.rs"
Cohesion: 0.16
Nodes (5): ChunkParams, commit(), CommitRequest, CommitResponse, Device

### Community 176 - "RFC 0005 — Folder-restricted viewers and the web reader"
Cohesion: 0.07
Nodes (30): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability, 11. Other concerns, 12. Alternatives considered, 13. Rollout and compatibility, 14. Test plan (+22 more)

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
Nodes (17): blob_hash(), Storage, a_failed_commit_leaves_the_object_for_the_sweep(), batches_answer_in_order_within_their_budget(), bucket_object_is_not_plaintext(), commit_blob(), concurrent_uploads_respect_the_quota(), hash_mismatch_is_422() (+9 more)

### Community 182 - "image"
Cohesion: 0.40
Nodes (5): image, digest, pullPolicy, repository, tag

### Community 183 - "tests/search.rs"
Cohesion: 0.25
Nodes (6): Round, ListenConfig, a_failing_index_job_is_kept_after_five_attempts(), S, search_is_opt_in_indexes_markdown_and_follows_edits(), setup()

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

### Community 191 - ".object_key"
Cohesion: 0.22
Nodes (8): prefix, interval, updates, commit-message, directory, package-ecosystem, schedule, version

### Community 192 - "MaintenanceError"
Cohesion: 0.23
Nodes (13): Batches, collect_chunks(), delete_objects(), lock_held(), MaintenanceError, Db, LockLost, Objects (+5 more)

### Community 193 - "wire.rs"
Cohesion: 0.09
Nodes (26): ChangesResponse, commit_bodies_parse_strictly(), eid(), entry_ids_are_uuid_v7(), EntryId, EntryState, Operation, Delete (+18 more)

### Community 194 - "android.sh"
Cohesion: 0.52
Nodes (6): adb(), ANDROID_HOME, booted(), sdk(), android.sh script, wait_boot()

### Community 197 - "reader.sh"
Cohesion: 0.38
Nodes (8): api(), as(), cleanup(), entry(), fail(), pass(), reader.sh script, sql()

### Community 198 - "connect_and_watch"
Cohesion: 0.22
Nodes (10): connect_and_watch(), fingerprint(), on_socket(), Socketed, Reconnect, Stop, watch(), Watched (+2 more)

### Community 199 - "bindings"
Cohesion: 0.25
Nodes (8): claimValue, paths, role, KuraVault campaign, apiVersion, kind, spec, bindings

### Community 200 - "overrides"
Cohesion: 0.40
Nodes (4): overrides, glob, js-yaml, minimatch

### Community 201 - "client.ts"
Cohesion: 0.09
Nodes (20): axios, axios@^1.20.0, browserName(), ensureDevice(), Retryable, signOut(), RFC-0003, KNOWN (+12 more)

### Community 202 - "AccessPage.vue"
Cohesion: 0.11
Nodes (13): add(), adding, {
  data: bindings,
  loading,
  error,
  forbidden,
  load,
}, folders, kind, refusal, remove(), role (+5 more)

### Community 203 - "authz.sh"
Cohesion: 0.30
Nodes (12): as(), batch_as(), call(), chunk_of(), cleanup(), dev_curl(), fail(), found() (+4 more)

### Community 204 - "tenants.yaml"
Cohesion: 0.29
Nodes (6): KuraVault obsidian, KuraVault shared, apiVersion, kind, apiVersion, kind

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

### Community 214 - "6. Detailed design"
Cohesion: 0.25
Nodes (8): 6.1 Data model (migration `0020_token_scopes.sql`), 6.2 `authz::PathScope` and the read side, 6.3 `Writer`, in `app/reader.rs`, 6.4 What a token may name: possession, 6.5 The commit, for a token, 6.6 `kura-cli`, 6. Detailed design, 6.P Proof against a real client

### Community 215 - "DevicesPage.vue"
Cohesion: 0.12
Nodes (13): confirming, devices, error, loading, router, session, { t }, confirming (+5 more)

### Community 216 - "test"
Cohesion: 0.67
Nodes (3): test, flatten, taskfile

### Community 217 - "DnsLabel"
Cohesion: 0.16
Nodes (3): DnsLabel, NameError, String

### Community 218 - "dev:certs:renew"
Cohesion: 0.67
Nodes (3): cmds, desc, dev:certs:renew

### Community 219 - "serve"
Cohesion: 0.27
Nodes (5): app(), cors(), Background, background(), serve()

### Community 220 - "src/merge.rs"
Cohesion: 0.27
Nodes (10): edits_of_different_lines_merge(), MAX_MERGE, MAX_MERGE_LINES, Merge, merge3(), Clean, Conflict, mergeable() (+2 more)

### Community 221 - "KuraVault notes"
Cohesion: 0.50
Nodes (4): KuraVault notes, apiVersion, kind, spec

### Community 222 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 223 - "10. Operability"
Cohesion: 0.50
Nodes (4): 10.1 Observability, 10.2 Failure modes, 10.3 Performance and capacity, 10. Operability

### Community 224 - "remote_entries"
Cohesion: 0.47
Nodes (3): remote_entries(), sync_config(), vault_named()

### Community 225 - "15. Decisions and open questions"
Cohesion: 0.67
Nodes (3): 15. Decisions and open questions, Resolved, Still open

### Community 228 - "4. User-facing design"
Cohesion: 0.33
Nodes (6): 4.1 Configuration, 4.2 Behaviour rules, 4.3 Validation, 4.4 API surface, 4.5 Mocks, 4. User-facing design

### Community 229 - "vite.config.ts"
Cohesion: 0.17
Nodes (8): @tailwindcss/vite, vite, @vitejs/plugin-vue, @tailwindcss/vite@^4.3.5, vite@^8.3.8, @vitejs/plugin-vue@^6.0.10, RFC-0003, RFC-0005

### Community 230 - "name"
Cohesion: 0.48
Nodes (7): metadata, metadata, metadata, metadata, metadata, name, namespace

### Community 240 - "5. Architecture"
Cohesion: 0.40
Nodes (5): 5.1 Components, 5.2 A push crossing the components, 5.3 Deciding the outcome of one pushed operation, 5.4 Invariants, 5. Architecture

### Community 250 - "roles"
Cohesion: 0.50
Nodes (4): roles, admin, member, owner

### Community 251 - "KuraTenant dev"
Cohesion: 0.50
Nodes (4): KuraTenant dev, apiVersion, kind, metadata

### Community 254 - "KuraTenant other"
Cohesion: 0.50
Nodes (4): KuraTenant other, apiVersion, kind, metadata

### Community 257 - "device.rs"
Cohesion: 0.31
Nodes (4): DEVICE_HEADER, rate_limit(), require_device(), require_session()

### Community 258 - "src/vault.rs"
Cohesion: 0.28
Nodes (7): default_region(), kv2_paths_split_into_mount_and_secret(), secrets_never_print(), SERVICE_ACCOUNT_TOKEN, split_kv2(), TIMEOUT, TOKEN_ENV

### Community 259 - "scalar.rs"
Cohesion: 0.36
Nodes (5): API_DOCS_CSP, scalar(), SCALAR_BUNDLE_PATH, scalar_unavailable_html(), the_reference_loads_nothing_from_a_third_party()

### Community 261 - "6. Detailed design"
Cohesion: 0.29
Nodes (7): 6.1 `kura-api`, 6.2 Data model (migration `0021_binding_paths.sql`), 6.3 `authz`, 6.4 Server routes that take a person but not a `Reader`, 6.5 The reader (`ui/`, front office of RFC 0003), 6. Detailed design, 6.P Proof against a real client

### Community 262 - "7. Security"
Cohesion: 0.29
Nodes (7): 7.1 Threat model, 7.2 Authentication and authorisation, 7.3 Input handling and injection, 7.4 Secrets and cryptography, 7.5 Abuse and resource exhaustion, 7.6 Existing defences that still hold, 7. Security

### Community 263 - "notify.sh"
Cohesion: 0.52
Nodes (6): api(), cleanup(), fail(), pass(), notify.sh script, sql()

### Community 264 - "Notice"
Cohesion: 0.33
Nodes (5): Notice, Change, Ping, Pull, text()

### Community 265 - "Event"
Cohesion: 0.33
Nodes (4): Event, Change, Pull, Revoked

## Knowledge Gaps
- **180 isolated node(s):** `RFC-0003`, `RFC-0003`, `RFC-0005`, `RFC-0003`, `Admin` (+175 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 1747 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **47 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ApiError` connect `Uuid` to `authz.rs`, `device.rs`, `ProblemType`, `app/tokens.rs`, `ApiError`, `app/search.rs`, `app/events.rs`, `app/notifications.rs`, `notifier.rs`, `devices.rs`, `app/mod.rs`, `reader.rs`, `body.rs`, `vaults.rs`, `store/keys.rs`, `problem`?**
  _High betweenness centrality (0.025) - this node is a cross-community bridge._
- **Are the 6 inferred relationships involving `SharedState` (e.g. with `call()` and `state()`) actually correct?**
  _`SharedState` has 6 INFERRED edges - model-reasoned connections that need verification._
- **What connects `RFC-0003`, `RFC-0003`, `RFC-0005` to the rest of the system?**
  _180 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `authz.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09803921568627451 - nodes in this community are weakly interconnected._
- **Why does `SharedState` connect `ApiError` to `device.rs`, `serve`, `app/tokens.rs`, `app/tests.rs`, `src/maintenance.rs`, `app/search.rs`, `app/events.rs`, `app/notifications.rs`, `notifier.rs`, `devices.rs`, `app/mod.rs`, `reader.rs`, `ui.rs`, `vaults.rs`, `serve`, `Uuid`, `problem`?**
  _High betweenness centrality (0.024) - this node is a cross-community bridge._
- **Should `reconcile/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.09759759759759759 - nodes in this community are weakly interconnected._
- **Why does `ChunkHash` connect `ChunkHash` to `Env`, `sim.rs`, `ChunkParams`, `crypto.rs`, `Env`, `kura-wasm/src/lib.rs`, `history.rs`, `ApiError`, `app/search.rs`, `tests/maintenance.rs`, `sim.rs`, `blob_hash`, `src/sync.rs`, `tests/search.rs`, `wire.rs`, `tests/sync.rs`, `api.rs`, `commands.rs`, `tests/events.rs`, `Uuid`, `tests/tokens.rs`, `engine.rs`, `tests/config.rs`, `tests/merge.rs`, `VaultRemote`?**
  _High betweenness centrality (0.024) - this node is a cross-community bridge._