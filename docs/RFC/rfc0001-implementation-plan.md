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
| Phase 3 | implemented | 2026-11-14 | D34 blocker resolved by D48; CI green on `0621b61`. Every exit test of §16 green (T3, T7, T14 note half, T17, T19 on the new routes, the §14 integration tests, the simulation, both fuzz targets, §6.P observations 2, 7, 11). Not closed: its exit is the first real use, which waits for PR 2.6 (chunking is fixed at vault creation and decision 52 is unmeasured), and S2b/S8 below stay open. The heavy suite also proves a real Authentik sign-in with refreshed tokens and their revocation (D48) |
| PR 3.1 | done | 2026-11-14 | `kura-core::path::VaultPath` (§6.3 rules, `parse` strict on the wire, `from_local` normalizing, `suffixed` for conflict and occupied names within 255 bytes), `wire` (`EntryId` UUIDv7 built from caller-supplied time and randomness, `Operation`, `CommitRequest`/`CommitResponse`, `EntryState`, `ChangesResponse`; `openapi` feature for the server's schemas); `ChunkHash` serde as hex; `wasm32-unknown-unknown` clippy in `task lint`; `fuzz_vault_path` (1.1 M runs) and `fuzz_commit_body` (3.1 M runs) without a finding. Tests `dotdot_is_rejected`, `dot_path_in_note_namespace_is_rejected`. D42 |
| PR 3.2 | done | 2026-11-14 | Migration `0007` (`vault.epoch/head_seq/horizon_seq`, `entry` with the unique live `path_key`, immutable `revision`, `commit_record` with the definer purge `kura_commit_purge` run by the daily loop), `POST /vaults/{v}/commits`: editor role checked before parsing (T3), vault row `FOR UPDATE`, idempotency under the lock by device and body hash, §6.5 rows without merge decided in memory then written set-based in seq order (a path freed and taken in one commit holds), conflict files `name (conflict <device> <date>).ext` built through `VaultPath::suffixed`, occupied paths suffixed `(2)`…, 1 MiB body route, 120/min, `kura_commits_total`. `GET /vaults` carries `chunking` in bytes, `epoch` and `headSeq`. 10 tests on PostgreSQL: `viewer_cannot_commit`, `commit_with_foreign_entry_id_is_refused`, `idempotency_key_reuse_different_body_is_422` (with the stored-result retry), `concurrent_commits_never_skip_a_seq` (16 × 5 operations, seqs 1–80), the §6.5 rows, invalid input failing the whole commit, the purge. D43 |
| PR 3.3 | done | 2026-11-14 | `db::begin_tenant_snapshot` (`REPEATABLE READ READ ONLY` before the first read), `GET /vaults/{v}/changes?since=&epoch=` (one item per entry, keyset by `last_seq`, 1,000 per page with `next`, `410 history_rewound` on a foreign epoch or a cursor above the head, `410 cursor_expired` below the horizon), `GET /vaults/{v}/entries?after=` (live entries by id). Tests: paging over a 1,000-operation commit, `a_pull_during_commits_never_passes_an_unseen_change` (12 writers, a cursor-driven reader sees all 48), T19 extended to `?since=&epoch=` in `tests/logs.rs` |
| PR 3.4 | done | 2026-11-14 | `kura-core::sync`: `Host` (files, persisted state, time, randomness) and `Remote` traits; `LocalState` (format version, device, vault, epoch, cursor, known entries with size/mtime stamps, write journal) discarded on a version or device mismatch; scan rehashing only on a stamp change, renames detected by content, dot-paths skipped, unsafe names and oversize files reported; push uploads only missing chunks then the manifest, commits in batches of 1,000 retried with the same idempotency key; results settle locally (conflict: our file renamed to the server's conflict name and the head written; unified; suffixed names followed); pull by cursor, saved per page, every chunk and blob checked against its address before a temp-file write, never over a local edit or an unknown local file (deferred, so the next push conflicts instead), deletions to `.trash`. `async fn` traits without `Send` for WASM. Tests: `tests/sim.rs` in `kura-server` (4 devices, the real engine against the real server, PostgreSQL + OpenBao + RustFS, 20 rounds with offline gaps, 5 seeds, 43 conflict files: every device converges, the server's live entries match, every synced version is in the history) and `kura-core/tests/engine.rs` (refused push still pulls, corrupt content never written, unknown local file never overwritten, journal adoption, unsyncable and dot files, copied folder). D44 |
| PR 3.5 | done | 2026-11-14 | `kura-cli` (binary `kura`, library of commands so tests drive them): `login` (RFC 8628 device code against the issuer's discovery, `slow_down` and expiry honoured, device registered with a name that must be a valid file name, "belongs to no Kura tenant" exit 2), `attach` (counts of matching, local-only, server-only and differing files before anything moves; matching files adopted without upload, differing ones kept as conflict files), `sync [vault] [--dry-run]`, `status`, `devices [revoke]`; exit codes of §4.5; refresh on every run with the rotated token stored first, `invalid_grant` → exit 2; `device_unknown` re-registers silently; `FsHost` writes through a hidden temp file, never follows, lists or writes through a symbolic link, never walks dot-folders, trashes to `.trash`. Tests: `kura-server/tests/cli.rs` (two devices over real HTTP: attach, download, conflict on both sides, unsyncable exit 4, dry run, unknown vault 3, revoke then exit 2; device code sign-in and refresh rotation against a fake issuer, credentials file 0600), `fs` unit tests (symlink escape refused), `tests/heavy/sync.sh` on the dev stack: §6.P observations 2, 7, 11 pass (`task test:heavy`). Dev blueprint: device codes live 10 min (`access_code_validity`), in the operator guide. Coverage `kura-cli` 81.6%. D45–D47 |
| PR 3.6 | done | 2026-11-14 | Independent security review (S1–S8 below), then a second pass (advisor): a first push of 700 files hit the 600/min limits and aborted (the CLI now honours `Retry-After` on 429/503, missing chunks are asked 1,000 at a time across files, each batch uploads then commits: 700 files attach in 62 s on the dev stack); a held item stopped paging (pages now continue, the saved cursor stops before the first held item, every deferred item holds, test `paging_continues_past_a_held_item`); the simulation now fails on any write failure and on anything still deferred at the end. Real `kura login` against Authentik verified, with refresh rotation, which surfaced the D34 blocker. Independent security review (S1–S7 below): the server side held (tenant isolation, `seq`, unique paths, idempotency, snapshot); six client and path defects fixed with regression tests, two recorded. Coverage gate per crate now four gates (`kura-api` 98.9%, `kura-core` 94.9%, `kura-server` 92.3%, `kura-cli` 81.2% lines). `fuzz_vault_path` and `fuzz_commit_body` in `fuzz:check` and the nightly loop. 152 tests green with PostgreSQL, OpenBao and RustFS, none skipped; `task lint`, `cargo deny`, `cargo audit`, Trivy (0 HIGH/CRITICAL), Spectral, shellcheck; `task test:heavy` passes against the rebuilt server |
| Phase 4 | implemented | 2026-11-14 | Server merge, WebSocket notifications, `kura watch`, the WASM core and the plugin shell, reviewed. Not closed: the plugin has not run inside Obsidian yet, and decisions 53–55 need the Android test phone (maintainer) |
| PR 4.1 | done | 2026-11-14 | `kura-core::merge` (feature `merge`, off in the WASM build): `diffy` 0.5 three-way line merge, `.md`/`.txt` only, UTF-8 only, at most 4 MiB per side, trusting `diffy`'s `Result`. Server: an unlocked idempotency lookup first; `prepare` (no lock, no open transaction) picks the diverging text edits, checks blob sizes, reads base, head and pushed content from storage, merges on the blocking pool under one 2 s deadline per commit and a 4-merge semaphore, stores a clean result with the vault's chunking (`store_content`); `decide` (vault lock) uses a merge only if its head is still the head, otherwise rolls back and prepares again, at most 3 attempts, then a conflict file; no storage, a large or binary file, the budget spent or a quota refusal all fall back to a conflict file; a merge equal to the head writes nothing. `kura_merge_total{outcome}`. Client: a `merged` result writes the head only if the file is still what was pushed, otherwise keeps the old base and merges again next run. Chunk and blob storage factored (`store_chunk`, `store_blob`, `read_chunk`, `read_blob`). Tests: `tests/merge.rs` (different lines merge, overlap and `.canvas` conflict, rename plus edits, 8 concurrent line edits losing nothing), engine `a_merged_push_writes_the_merged_head`, the simulation now edits lines and checks every synced line is in some stored revision (9 merges over 5 seeds), `fuzz_merge` (230,000 runs, no finding). D49 |
| PR 4.2 | done | 2026-11-14 | Migration `0009` (`ws_ticket` hashed, 30 s, forced RLS; definer `kura_ws_ticket_consume` with `DELETE … RETURNING`, `kura_ws_ticket_purge` on each heartbeat, `kura_vault_head`); `POST /vaults/{v}/events/ticket` (viewer, 256 random bits, 10/min); `GET /vaults/{v}/events?ticket=` (the ticket consumed before the upgrade; JSON `change`, `pull` after a lost `LISTEN` with 0–5 s jitter, `ping` with `headSeq` every 30 s; closed 4003 used/expired/unknown, 4004 other vault, 4001 token expired, 4002 device revoked, 4008 past 4 sockets, 4009 slow consumer, 1001 at shutdown); `NOTIFY kura_vault` in the commit transaction and on revocation; `notify::listen` per replica (sqlx `PgListener`, 15 s self-heartbeat, reconnect after two misses or a lost connection). Tests (`tests/events.rs`, real TCP): `ticket_reuse_is_refused`, `ticket_for_other_vault_is_refused`, `expired_ticket_is_refused`, a commit reaching the socket, a notification crossing replicas through PostgreSQL, token expiry, revocation and the fifth socket closing; T19 extended to `?ticket=`. D50 |
| PR 4.3 | done | 2026-11-14 | `kura watch [vault]`: one sync, then a socket (ticket, `ws`/`wss` with rustls and the OS roots, `tokio-tungstenite` 0.29, the version axum links); pulls on `change` and `pull`, pushes local edits once the folder has been quiet 5 s, treats 75 s without a notice as down, reconnects with a fresh token (the server closes at token expiry) after a decorrelated-jitter backoff of 1–60 s, syncing on each retry; stops on Ctrl-C. Tests: `watch_pulls_on_notification_and_pushes_when_quiet` (in process, real TCP and `LISTEN`), §6.P observation 1 in `tests/heavy/sync.sh` (203 ms on the dev stack). `deny`: the rand 0.9 line held by tungstenite, skipped with its holder named. D51 |
| PR 4.4 | done | 2026-11-14 | `kura-wasm` (cdylib): the `kura-core` engine exported as `sync(host, remote, state, options)` and `pathProblem(path)`; `Host` and `Remote` implemented by JavaScript objects whose methods return promises (wasm-bindgen 0.2.130 async externs), values as JSON and byte arrays, every error a rejected promise (a problem `{status, kind, detail}` or a message), no `unsafe` in the crate; `profile.wasm` (release, unstripped) then `wasm-opt -Oz`: **470,650 bytes** (203 KB gzipped), instantiated in 1.5 ms in Node; `task wasm:build`, the merge feature left out (decision 54, size half; the Android load time is for 4.6). Tools by mise: `wasm-bindgen-cli` 0.2.130, binaryen 133, pnpm |
| PR 4.5 | done | 2026-11-14 | `plugin/` (id `weebo-kura`, "Kura Sync", `minAppVersion` 1.11.4): TypeScript 6.0 strict with `noUncheckedIndexedAccess` and `exactOptionalPropertyTypes`, ESLint `strictTypeChecked` with no `any`, no non-null assertion, no type assertion; esbuild bundle with the WASM core embedded: **main.js 657,660 bytes** (budget 3 MB enforced by the build). Modules: `auth` (device code via `requestUrl`, refresh with one call in flight and the rotated token stored first, sign-out only on `invalid_grant`, `SecretStorage` id `kura-<server hash>-<vault id>`, decision 53), `remote` (fetch, `Retry-After`, refusals as `ProblemError` the core reads), `host` (Obsidian adapter, dot-folders never walked, temp file then rename, Obsidian's `.trash`, a note open in the editor refused so the change waits, decision 55), `notifier` (ticket socket, 75 s silence = down, 60 s ±20% poll while down, decorrelated reconnect), `controller` (5 s quiet before a push, one run at a time), `status` (every word of §4.5, status bar on desktop, ribbon label on mobile, first line of the settings panel); `main.ts`/`settings.ts` glue. 27 vitest tests, including the real WASM engine between two devices and a server in memory; coverage 99.4% lines, 92.8% branches (80% gate). `pnpm audit` clean (minimatch overridden to ≥ 11.0.1, GHSA-g3hv-75wg-m249, dev only). CI job `plugin`; `task lint` and `task audit` include it. Server: optional `oidc.discovery` for Authentik's global issuer mode, dev stack moved to it with a `kura-plugin` provider; a `kura-plugin` token registers a device. D52, D53 |
| PR 4.6 | done | 2026-11-14 | Independent security review (V1–V9 below): all fixed (the role re-check on open sockets landed with PR 9.3); 174 Rust tests and 28 plugin tests green, none skipped; four coverage gates (`kura-api` 98.9%, `kura-core` 95.2%, `kura-server` 92.0%, `kura-cli` 82.9%) and the plugin's (99.4% lines); `task lint` (with the plugin, Spectral at zero), `task audit` (cargo audit, deny, pnpm audit, Trivy) clean; `task test:heavy` passes (observation 1 at 200 ms). **Open (maintainer)**: the plugin inside Obsidian on desktop, then Android; decisions 53, 54 (load time) and 55 measured on the test phone |
| Phase 5 | implemented | 2026-11-14 | History, restore, retention, garbage collection, horizon, epochs, full resync with its deletion guard, `Delete` purge (T18), reviewed (W1–W6). Exit criteria met: T18, observation 6 on the dev stack, horizon and full resync, both interleavings of decision 24. The plugin's resync prompt has not run inside Obsidian (phase 4's open item) |
| PR 5.1 | done | 2026-11-14 | Migration `0011`: `cluster_identity` (forced RLS, owner only) and `kura_epoch_check()`, timeline read from the current WAL file name (the checkpoint's lags after a promotion), run on every new pool connection (`after_connect`: a failover breaks them all), on every `LISTEN` heartbeat and at start; two replicas racing regenerate once (row lock, `ON CONFLICT … WHERE`); `kura-server reset-epochs` (owner) for the restore runbook; `kura_vault_epoch_changes_total`. `CommitRequest.epoch` (optional, absent from the body when unset, so stored request hashes are unchanged): another epoch is `410 history_rewound`; an operation on an entry the vault lacks, with a base, is `410 cursor_expired` once `horizon_seq > 0` (400 before, T17 unchanged); a base pruned by retention is handled as overlapping edits. Test `epochs_change_once_with_the_cluster_identity`. D55 |
| PR 5.2 | done | 2026-11-14 | `GET /vaults/{v}/entries/{e}/revisions` (viewer, newest first, keyset `before`, 1,000 a page, `createdAt` RFC 3339, no device or subject), `POST /vaults/{v}/entries/{e}/restore` (editor, `{revision}`: a `Put` with the old blob on the current head through `decide`, so the lock, `seq`, `NOTIFY` and idempotency record apply; a deleted entry comes back at the revision's path; a deletion revision is 400; unknown 404); both 60/min; tag `history`. `kura history <vault> <path>`, `kura restore <vault> <path> <revision>` (then syncs). Tests `history_and_restore`, the CLI half of `a_long_absence_resyncs_and_history_restores`. D59 |
| PR 5.3 | done | 2026-11-14 | `maintenance.window` in `server.yaml` (`HH:MM-HH:MM` UTC, default `01:00-05:00`, equal ends = all day); `maintenance::run` per replica, one pass an hour inside the window, one pass at a time across replicas (`pg_try_advisory_lock`), batches of 1,000 with 1 s pauses, stops between batches at shutdown; `kura-server maintain` runs one pass now (`task dev:maintain`). Definer functions per vault: `kura_prune_revisions` (age, count; no entry's head, a deletion's included), `kura_purge_deleted` (under the vault lock, default `deletedEntryTtl` 90 days, `horizon_seq` raised to the purged `last_seq`). `kura_maintenance_deleted_total{outcome}`. Tests `retention_prunes_history_but_never_a_head`, `purged_deletions_move_the_horizon`. D57 |
| PR 5.4 | done | 2026-11-14 | `blob.last_touched` (touched by a blob `PUT` on conflict), index `revision (vault_id, blob_hash)`; `kura_gc_blobs` and `kura_gc_chunks` lock `FOR UPDATE SKIP LOCKED`, recheck in a new statement, delete; chunk object keys returned and deleted after the commit; `store_blob` and `check_blobs` take `FOR KEY SHARE`; orphan objects swept per vault prefix (`kura_known_objects`, older than 24 h); collection, the sweep and the purge only under the shared side of `BACKUP_LOCK` (`0x6b757261_6261636b`), on one session connection. Tests `collection_removes_only_what_nothing_references`, `a_blob_put_and_collection_never_interleave` (both orders of decision 24), `orphans_are_swept_and_the_backup_lock_pauses_deletion` |
| PR 5.5 | done | 2026-11-14 | `kura-core`: a `410` from the push or the pull runs `resync` once per run: every server state fetched from 0, known entries matched by id; `cursor_expired`: absent and unchanged → `.trash`, absent and changed → pushed as new, deleted and unchanged → `.trash`; `history_rewound`: nothing trashed, absent → pushed as new, a deletion or another version the server holds → `Known.resend` (pushed against its base, never written over by a pull until settled); then cursor 0 under the new epoch, push and pull. Guard: more than 10% of local files or 50 → `SyncError::TooManyDeletions`, nothing changed. `kura sync --yes` (asks on a terminal, else exit 3); kura-wasm `allowDeletions` and `too_many_deletions`; the plugin asks once (`ConfirmModal`), again only on a manual sync, status word "confirm deletions", a notice after a resync. Tests: plugin `resyncs past the horizon…` (real WASM core) and the controller's guard; `a_long_absence_resyncs_and_history_restores`, `a_rewound_server_gets_everything_back`; observation 6 in `tests/heavy/sync.sh`. D56 |
| PR 5.6 | done | 2026-11-14 | `kura_purge_vault` (one table at a time, 1,000 rows a batch, the vault and tenant `delete_after` rechecked under `FOR UPDATE`/`FOR SHARE` in every batch: `-1` once the document came back), then every object under the vault prefix, then `kura_purge_vault_finish` (vault row; the tenant marked `purged_at` after its last vault); `kura_tenants_awaiting_key_destruction` gauge; audit event `vault_purged`. `deploy/vault/kura-server.hcl`: KV `read`, Transit `datakey/plaintext`, `decrypt`, `rewrap` on `kura-*`. Tests T18 `restored_document_cancels_deletion` (removed, due, restored, nothing purged; removed again and due, rows, objects and the vault row gone) and `server_policy_cannot_delete_key` (OpenBao: 403 on key delete, `deletion_allowed` and trim). D58 |
| PR 5.7 | done | 2026-11-14 | Independent security review (W1–W6 below), all fixed but the scan cost of retention (shortcut noted); W2–W4 fixed without a test of their own (an object-store or connection fault is not injectable yet). Sizes after phase 5: `kura_bg.wasm` 488,909 bytes, `main.js` 683,812 bytes. Known limit: a pull held below the horizon (a note left open in Obsidian) resyncs on every run until the note closes, announced once per session; 190 Rust tests (none skipped) and 30 plugin tests green; coverage `kura-api` 98.9%, `kura-core` 94.8%, `kura-server` 91.3%, `kura-cli` 83.3%, plugin 99.4%; `task lint`, `task audit`, `task openapi:check` clean; `task test:heavy` passes with observation 6; operator guide §1, §3, §5, §8, §9, §10, §12. D61 |
| Phase 6 | implemented | 2026-11-14 | Telemetry, Helm chart, alerts, backup and restore drill, k6 load, reviewed (X1–X17). Decision 42 measured: the first attach of 10,000 notes takes 974 s, so the batch chunk download RFC opens: [RFC 0002](rfc0002-batched-transfers.md), Accepted and its phases 1–2 implemented on 2026-11-15: the same attach now takes 33 s. **Open (maintainer)**: installing the chart on the cluster with CloudNativePG, the restore drill there (Vault snapshot, PITR), a soak of hours, resource requests from the cluster run |
| PR 6.1 | done | 2026-11-14 | `commit_duration_seconds` (histogram), `notify_lag_seconds` (each replica times its own heartbeat, `beat:<replica>:<ms>`), `notify_queue_usage`, `ws_connections` (the hub counts its seats), `vault_call_duration_seconds{outcome}` (`Vault::measure`), `resync_total{outcome}` and `unavailable_total` (from the problem type of each response, in the audit middleware), `chunk_bytes_stored`, `dedup_ratio`, `quota_used_ratio` by tenant and `deletions_scheduled`, refreshed every minute (migration `0012`: `kura_tenant_storage`, `kura_deletions_scheduled`). Tests: `storage_commit_and_resync_metrics_move`, the socket count in `a_commit_reaches_the_socket`, Vault timing in `two_replicas_creating_one_data_key_keep_one`, the heartbeat lag in `payloads_become_events`. D62 |
| PR 6.2 | done | 2026-11-14 | `deploy/helm/kura`: Deployment (2 replicas, spread by node, UID 65532, read-only root, no capability, `RuntimeDefault` seccomp, probes, checksum restart on `server.yaml` only), Services (API; metrics apart, never routed), Ingress, ConfigMaps (`server.yaml` from `server` values with `listen` set by the chart; documents re-read live, no `subPath`), ServiceAccount (Vault Kubernetes auth), migration Job (pre-install/pre-upgrade hook, owner URL from `<cnpg>-app`), PodDisruptionBudget, NetworkPolicy (ingress only: API from the ingress namespace, metrics from monitoring), ServiceMonitor, PrometheusRule, CloudNativePG `Cluster` (2 instances, `kura_app` by `postInitApplicationSQLRefs`, barman and `ScheduledBackup` when a destination is set), backup CronJob. `task helm:lint`: `helm lint --strict`, three renders through kubeconform (strict, CRD schemas from the datree catalog), the chart's copy of `bootstrap-roles.sql` diffed with the original; in `task lint`. The rendered `server.yaml` loads. D65 |
| PR 6.3 | done | 2026-11-14 | `files/alerts.yaml`: 13 alerts (unready, down, dependency failing, Vault failing, notification lag, `NOTIFY` queue, corrupt chunk, refused configuration, deletion scheduled, key destruction awaited, epoch change, quota above 90%, maintenance failing). `task alerts:test`: `promtool check rules`, `promtool test rules deploy/alerts/tests.yaml`, and a gate failing on an alert without a test (checked by adding one). `index_queue_depth` waits for search. D62 |
| PR 6.4 | done | 2026-11-14 | `kura-server backup` (`backup::copy`): waits up to 30 min for the exclusive `BACKUP_LOCK`, records the database time T, copies the objects of every vault the second bucket lacks (`KURA_BACKUP_*`), writes `backups/<T>.json`; `kura-server check-chunks --sample N` (`kura_sample_chunks`, decrypt and address check); restore runbook in the operator guide §12 (Vault snapshot, PITR to T, objects back, `migrate`, `reset-epochs`, `check-chunks`); `task dev:drill` passes on the dev stack (717 objects backed up, lost, restored into `kura_drill`, epochs regenerated, 50 chunks decrypted). Test `backup_copies_new_objects_and_the_drill_check_catches_corruption`. D63 |
| PR 6.5 | done | 2026-11-14 | `perf/k6` (01 commit, 02 pull, 03 five devices answered 410 at once), `perf/run.sh` (token, vault, an existing blob; the run's devices revoked after), `perf/attach.sh` (decision 42 with the real CLI), `perf/soak.sh` (RSS sampled, gate +25% after warm-up), `.tasks/perf.yaml`. Measured on the dev stack (release build, laptop): one-note commit **p95 17 ms** (budget 200 ms), pull p95 6.5 ms, five devices answered 410 at once listing 2,511 entries p95 33 ms, propagation 0.26 s (heavy suite), 0 failed requests. **Decision 42: a new device's first attach of 10,000 notes (25 MB) took 974 s**, the push of the first device 980 s: above 5 minutes, so the batch chunk download RFC opens. The time is the §4.4 rate limits, not the server: about 20,000 `GET` of blobs and chunks at 600 a minute per device. `dev:run RELEASE=1`. Dev stack: Authentik's device-code throttle (20/hour per address by default) raised to 1000/hour, `dev/token.sh` retries a `slow_down`, the heavy suite revokes its devices. D64 |
| PR 6.6 | done | 2026-11-14 | Independent review (X1–X15), the attach run (X15, X17) and the heavy suite on a large vault (X16), all fixed; 195 Rust tests (none skipped), 30 plugin tests; coverage `kura-api` 98.9%, `kura-core` 94.7%, `kura-server` 91.5%, `kura-cli` 82.9%; `task lint` (with `helm:lint`, `alerts:test`), `task audit`, gitleaks clean; heavy suite and `task dev:drill` pass; operator guide §1, §8, §11, §12, §13. The soak (`task perf:soak`) is written but not run for hours. D66 |
| Phase 7 | implemented | 2026-11-14 | Scoped tokens (expiry, CIDR, revocation, prefix) on the read endpoints, opt-in search, reviewed (Y1–Y8). Exit criteria met: T5, T16, the token half of T10, observation 3 (and 4, 5) in `tests/heavy/authz.sh`, `fuzz_scoped_token` |
| PR 7.1 | done | 2026-11-14 | `kura-core::token` (`kura_<32 hex id>_<64 hex secret>`, one spelling, `Debug` redacted); migration `0013` (`scoped_token`: SHA-256 only, tenant isolated, `cidr[]` 1–32, expiry ≤ 90 days checked by the table, the grant: `granted_by_binding` emptied with the binding or `granted_by_claim`; definers `kura_scoped_token_lookup`, `_touch` (once a minute), `_purge` (30 days past expiry, daily)); `POST/GET/DELETE /vaults/{v}/tokens` (manager or tenant admin; prefix a folder ending `/`; no `/0`; secret once); `Reader` (device or key): per-address limit before any lookup, constant-time hash compare, expiry, revocation, grant still held (binding still `manager`, or the claim value still mapped to owner/admin), client address by `client_addr_strict` within the CIDRs (403 `address_not_allowed`, audited by the middleware, `last_used_at` untouched), per-token limits, audit `scoped_token_used`; on `GET /vaults` (its one vault), `changes`, `entries`, `revisions`, `blobs`, `chunks`, `search`; a key on any other route is 403. `kura token create/list/revoke` (the §4.5 mock, `--cidr` required, exit 3). Tests `token_outside_cidr_is_refused_and_revoked_or_expired_tokens_are_401` (T5: CIDR, expiry, revocation, tampering, `read_token_cannot_commit`, another vault), `a_token_is_revoked_with_the_binding_that_allowed_it`, `the_cli_issues_lists_and_revokes_scoped_tokens`; `fuzz_scoped_token` 19.4 M runs, no finding. D67 |
| PR 7.2 | done | 2026-11-14 | `authz::fold` (the one comparison form of paths and prefixes), `visible_under_prefix`, `like_prefix` (wildcards escaped): `changes` and `entries` filtered on `path_key`, `revisions` only for an entry whose head is inside and only revisions made inside, `blobs` and `chunks` only through such a revision of such an entry (fails closed past 200 candidates), search filtered too. Test `prefix_token_cannot_fetch_outside_chunk_or_read_renamed_history` (T16: case-folded prefix, renamed out, renamed in, chunks and blobs both ways). D68 |
| PR 7.3 | done | 2026-11-14 | Migration `0014`: `unaccent`, `pg_trgm`, `kura_french`/`kura_english` with accents folded, `search_doc` (head lexemes), `index_job` (one per entry, from every commit while the tenant's search is on), trigram index on `entry.path`, `tenant.search_indexed`; definers `kura_search_sync` (backfill when enabled, drop when disabled), `kura_index_claim` (100, `SKIP LOCKED`, claimed 5 minutes), `kura_index_set` (claimed job and current head only), `kura_index_failed` (5 attempts, then kept), `kura_index_depth`; a worker per replica (`app::search::run`), Markdown only, 256 KiB cut, path words indexed, notes above 4 MiB by path; `POST /vaults/{v}/search` (viewer or key, 2 characters to 256 bytes, ≤ 50 results, `websearch_to_tsquery` French and English plus `pg_trgm` similarity on paths, snippets within 1 MiB decrypted, `409 search_disabled`); `kura_index_queue_depth`, alert `KuraIndexQueueGrowing`; `kura search`. Tests `search_is_opt_in_indexes_markdown_and_follows_edits`, `a_failing_index_job_is_kept_after_five_attempts` |
| PR 7.4 | done | 2026-11-14 | `tests/heavy/authz.sh` (observations 3, 4, 5 on the dev stack; a second tenant in `dev/config`); independent review Y1–Y8, all fixed or recorded; 206 Rust tests (none skipped), 30 plugin tests; coverage `kura-api` 98.9%, `kura-core` 94.9%, `kura-server` 92.1%, `kura-cli` 83.1%; the heavy suites pass (observations 1–7, 11, D34); lint (Spectral, chart, alerts), audit clean. `concurrent_line_edits_lose_nothing` no longer requires a merge to happen (it failed CI on `e95719d` when every merge missed its budget on the runner; nothing was lost) |
| Phase 8 | implemented | 2026-11-14 | Obsidian configuration sync per user and profile, synced plugins off until agreed per device and hash (decision 34), reviewed (Z1–Z9). Exit criteria met but observation 12, which needs Obsidian on desktop and the Android phone (maintainer) |
| PR 8.1 | done | 2026-11-14 | Migration `0015` (additive; 0011–0014 untouched): `entry.namespace` `notes`/`config:desktop`/`config:mobile` with `owner_sub`, uniqueness per namespace and owner, the `entry` row policy keyed on the transaction's subject (another user's configuration is invisible to every query, keys and jobs included), `kura_entry_home`, search functions limited to notes. `app::space`: `space=config` on `changes`, `entries` and commits, the profile read from the device row, categories from `configSync` (case-insensitive, never the Kura plugin), a key refused; commits refuse an id living in another space, never merge, and in configuration the last write wins (a new id on a held path writes onto it, a rename onto a held path deletes the holder, no suffix, no conflict file); restore works on one's own configuration; content served only through an entry visible to the caller (T14); `GET /vaults` gives `configSync`. Tests `config_namespace_hidden_from_other_member`, `configuration_writes_last_wins_without_conflict_files` (with restore). D69, D70 |
| PR 8.2 | done | 2026-11-14 | `kura-core::config` (categories, the Kura plugin and `community-plugins.json`/`workspace*.json` never), `Options::prefer_remote` (configuration: pull first, a local file never synced gives way to the server's, to `.trash`), the cursor reaching the page's head on a quiet view (a configuration space or a prefix would otherwise sit below the horizon forever); CLI: `FsHost::config` (`.obsidian`, its own state `.kura/config-state.json`, links refused), `kura sync` syncs the desktop profile, a dropped category forgets, never deletes elsewhere; WASM `configSynced`; plugin: `ObsidianHost` with root, filter, state name and a `beforeWrite` hook, `PluginGuard` (consent on the device only: accepted/declined hash per plugin, pending, the ones it disabled; an enabled plugin is disabled before new code lands, asked once per hash, turned back on only if the guard turned it off), the registry over `app.plugins` and `community-plugins.json`, the §4.5 modal, a scan every 5 minutes and on resume. Tests `received_plugin_is_disabled`, `plugin_hash_change_disables_it`, the case and keep-off cases, `the_cli_syncs_one_users_configuration_only`, `the_configuration_takes_the_servers_copy_on_first_contact`, `a_quiet_view_moves_its_cursor_to_the_head`. D71 |
| PR 8.3 | done | 2026-11-14 | Independent review Z1–Z9, fixed or recorded; 213 Rust tests (none skipped), 36 plugin tests; lint, audit, heavy suites; §6.P observation 12 for the maintainer |
| Phase 9 | in progress | 2026-11-14 | The tooling is done (PR 9.1); the rest is the migration itself, run by the maintainer: Notion export and conversion, first push, one month of parallel use, cutover |
| PR 9.1 | done | 2026-11-14 | `kura verify <vault>`: the server's live notes against the folder by path and content, with file counts and exact bytes on both sides; exit 0 only when equal (§13's exit criterion). `kura attach` names every file that cannot sync, with its rule, and the bytes it would push, before anything moves; `sync --dry-run` prints the bytes too (§4.5 mock). Folder/server comparison shared by both (`commands::diff`). Tests: CLI round trip (unsyncable name, unpushed file, then equal), `bytes_for_people`, engine `pushed_bytes`; `tests/heavy/sync.sh` runs `kura verify` with the real binary. Migration runbook in the operator guide §9. D72 |
| PR 9.2 | **open** (maintainer) | | Export Notion, convert with Obsidian's importer, `kura attach` (answer no) until nothing is unsyncable, push, `kura verify` prints `Equal.`; Notion read-only from the export date; one month of use; close Notion |
| PR 9.3 | done | 2026-11-14 | Review V4's open half: an open socket re-checks the vault role on every 30 s ping (the first at once) with the claims of the token behind its ticket (migration `0016`, additive: `ws_ticket.claims`, `kura_ws_ticket_consume` returns them), and closes 4005 `access_revoked` when a binding, a mapping or the vault is gone; a ticket without a subject is spent (4003). Clients need no change: they reconnect, and the ticket request is refused. Test `a_socket_closes_when_its_caller_loses_the_vault`; heavy observation 1 passes with real Authentik claims |
| PR 9.4 | done | 2026-11-15 | Phase 1 review leftovers R5 and R6 fixed (see their rows); 216 Rust tests. RFC 0002 (batched transfers) drafted, Draft, three questions open |
| Obsidian by hand | ready | 2026-11-15 | `task obsidian:ca:trust`, `task obsidian:deploy VAULT=…` and the dev vault `obsidian` (with `configSync`, apart from the heavy suites' `notes`) for observation 12 and decisions 53–55 on desktop. Android: `task android:setup|start|obsidian|deploy` (`dev/android.sh`), an emulator on the dev PC reaching the loopback-only stack through `adb reverse`, the dev CA in its system store; written and shellchecked, **not run yet** (setup downloads about 3 GB, left to the maintainer) |

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

## 3. Phase 3 — changes, commits, kura-core, kura-cli

Detailed at the start of phase 3. Exit criteria (§16): T3 (`viewer_cannot_commit`),
T7 (`dotdot_is_rejected`, `fuzz_vault_path`), T14 note-namespace half
(`dot_path_in_note_namespace_is_rejected`, and the client re-parsing what it
receives), T17 (`commit_with_foreign_entry_id_is_refused`,
`idempotency_key_reuse_different_body_is_422`), T19 on the new routes
including `?since=`; the §14 integration tests (concurrent commits never skip
a `seq`, a pull during commits never passes an unseen change, idempotent
retry); the device simulation converging; `fuzz_commit_body`; a working sync
through `kura-cli` on the dev stack (§6.P observations 2, 7, 11 in
`tests/heavy/sync.sh`).

Out of scope, by §16: the three-way merge (phase 4: every diverging edit is a
conflict file, `merged` exists in the wire format but is not produced yet);
history, restore, retention, GC, horizon purge, epochs changing and the full
resync with its deletion guard (phase 5: the simulation's 200-day absence
waits for it). Kept now because the wire format and the local state are
costly to change after first use: `epoch`, `head_seq` and `horizon_seq` on
`vault`, the epoch in the client cursor, `410` answers wired.

| PR | Content | Tests |
| --- | --- | --- |
| 3.1 | `kura-core`: `VaultPath` (§6.3 rules, note and configuration namespaces), `EntryId` (UUIDv7), `Seq`, the commit and changes wire types shared with the server; `wasm32` build check in `task lint`; `fuzz_vault_path` | `dotdot_is_rejected`, `dot_path_in_note_namespace_is_rejected`, NFC and case rules |
| 3.2 | Migration `0007`: `vault.epoch/head_seq/horizon_seq`, `entry`, `revision`, `commit` (7-day purge through a definer function); `POST /vaults/{v}/commits` under the vault row lock, set-based writes, idempotency per device with the body hash, conflict files named through `VaultPath`, its own body limit; chunking parameters and `headSeq` in `GET /vaults`; `fuzz_commit_body` | T3, both T17, concurrent commits never skip a `seq`, idempotent retry, the §6.5 rows without merge |
| 3.3 | `GET /vaults/{v}/changes?since=` (one `REPEATABLE READ` snapshot, keyset by `last_seq`, 1,000 items, `410` on a foreign epoch or a cursor above head), `GET /vaults/{v}/entries` | pull during commits never passes an unseen change, T19 on `?since=` |
| 3.4 | `kura-core` sync engine behind `Files` and `Remote` traits: local state (format version, device id, cursor with epoch), change detection by size and mtime, push (missing chunks, blobs, commit), pull with journal and temp-file rename, conflict files kept, received paths re-parsed | device simulation with N devices and offline periods converges and loses nothing; a hostile fake server cannot write outside the vault |
| 3.5 | `kura-cli`: `login` (device code, issuer from `--issuer` or config), `attach`, `sync` (with `--dry-run`), `status`, `devices` (list, revoke); exit codes of §4.5; refresh token in the OS keyring | CLI tests on a fake server; `tests/heavy/sync.sh` observations 2, 7, 11 on the dev stack |
| 3.6 | Close: independent security review, coverage gate with the new crates, fuzz targets in `fuzz:check` and nightly, §8 additions, operator guide | — |

## 4. Phase 4 — merge, notifications, plugin

Detailed at the start of phase 4. Exit criteria (§16): T11
(`ticket_reuse_is_refused`, `ticket_for_other_vault_is_refused`,
`expired_ticket_is_refused`); `fuzz_merge`; the three-way merge on every text
row of §6.5; device-to-device propagation under 5 seconds without polling
(§6.P observation 1, observation 10); the plugin syncing on desktop, then on
Android; decisions 53, 54 and 55 measured on the Android test phone.

| PR | Content | Tests |
| --- | --- | --- |
| 4.1 | `kura-core::merge`: line three-way merge with `diffy` (decision 43) for `.md` and `.txt`, UTF-8 only, at most 4 MiB per side (§7.3); the server merges diverging text edits before taking the lock (base, head and pushed content read from storage), within a 2 s budget on the blocking pool, stores the merged blob, rechecks the head under the lock, retries at most 3 times, else a conflict file; `merged` outcome settled by the client; `kura_merge_total{outcome}`; `fuzz_merge` | every text row of §6.5, base pruned → conflict, binary → conflict, budget exceeded → conflict, concurrent head move → retry |
| 4.2 | Migration: `ws_ticket` (hashed, 30 s, device and vault, token expiry); `POST /vaults/{v}/events/ticket`, `GET /vaults/{v}/events?ticket=` (WebSocket, consumed with `DELETE … RETURNING`); `NOTIFY` in the commit transaction; one `LISTEN` connection per replica with a 15 s heartbeat and reconnect (a pull hint to every socket after a drop, 0–5 s jitter); a ping with `head_seq` every 30 s; 16-message queue, slow consumers dropped; closed at token expiry; 4 sockets per device; the phase 1 generation poll replaced by `LISTEN` | T11, a notification reaching a socket on another replica, slow consumer dropped, socket closed at token expiry |
| 4.3 | `kura watch`: pull on start, on notification and on resume, 60 s ±20% timer only while the socket is down, local changes pushed after 5 s of quiet, backoff with decorrelated jitter honouring `Retry-After` | §6.P observation 1 in `tests/heavy/sync.sh` |
| 4.4 | `kura-wasm`: `wasm-bindgen` bindings of the `kura-core` engine, `Host` and `Remote` implemented by JavaScript promises; no panic across the boundary; size and load budget (decision 54) | wasm tests in Node |
| 4.5 | `plugin/` (id `weebo-kura`, "Kura Sync"): TypeScript strict shell on the Obsidian API, esbuild bundle with the WASM core embedded, device code sign-in, `SecretStorage` ids with server and vault (decision 53), status bar and ribbon states of §4.5, settings panel, the open note deferred (decision 55), `fetch` transport, 200 MiB default limit | vitest on the shell with fake core traits, 80% gate |
| 4.6 | Close: measurements on the Android phone (maintainer), independent security review, §8 additions, operator guide | — |

## 5. Phase 5 — history, retention, resync

Detailed at the start of phase 5. Exit criteria (§16): T18
(`server_policy_cannot_delete_key`, `restored_document_cancels_deletion`);
§6.P observation 6 in `tests/heavy/sync.sh`; the §14 integration tests
"horizon and full resync" and "garbage collection racing an upload, in both
interleavings of decision 24".

| PR | Content | Tests |
| --- | --- | --- |
| 5.1 | Migration `0011`: `cluster_identity` (PostgreSQL's system identifier and timeline) and the definer `kura_epoch_check()`, which regenerates every vault's `epoch` when either changed; run at startup and on every `LISTEN` heartbeat; `kura-server reset-epochs` for the restore runbook; `kura_vault_epoch_changes_total`. A commit may carry its device's `epoch`: another one answers `410 history_rewound`; an operation naming an entry the vault does not hold, with a base, answers `410 cursor_expired` once the horizon moved (its entry was purged), and a base pruned by retention is handled as overlapping edits (§6.5), so a stale device is sent to resync instead of being refused forever | identity change regenerates epochs once across two replicas; stale commit answers |
| 5.2 | `GET /vaults/{v}/entries/{e}/revisions` (viewer, newest first, keyset by `seq`, 1,000 a page, no subject), `POST /vaults/{v}/entries/{e}/restore` (editor: a new revision with the old content through the commit path, a deletion not restorable); `kura history <path>`, `kura restore <path> <revision>` | restore of an edited and of a deleted entry, foreign revision 404, viewer 403 |
| 5.3 | `maintenance` in `server.yaml` (off-peak window, default 01:00–05:00 UTC); one maintenance task per replica, one pass at a time across replicas (advisory lock); retention per tenant through definer functions, in batches of 1,000 with a pause: revisions past `maxRevisionAge` or beyond `maxRevisionsPerEntry` (never a live entry's head), deleted entries past `deletedEntryTtl` purged under the vault lock, `horizon_seq` advanced to their `last_seq` | count and age pruning keep the head, purge advances the horizon and a stale cursor gets `410 cursor_expired` |
| 5.4 | Garbage collection (§6.9): blobs unreferenced and untouched 24 h (`blob.last_touched`, refreshed by a blob `PUT`), chunks unreferenced and untouched 24 h locked `FOR UPDATE SKIP LOCKED`, row deleted and committed before the object; blob `PUT` and commits take `FOR KEY SHARE`; objects with no row, older than 24 h, swept; object deletion only under the shared side of the backup advisory lock | both interleavings of decision 24, an orphan object swept, nothing deleted while the backup lock is held |
| 5.5 | `kura-core` full resync after `410` (from a pull or a commit): every server entry fetched, local entries matched by id, the deletion guard (more than 10% of local files or 50 stops with `TooManyDeletions`), `history_rewound` deletes nothing and pushes back what the server lost; `kura sync --yes`; the plugin asks before deleting | engine tests for both answers and the guard; observation 6 in the heavy suite |
| 5.6 | `Delete` purge (decision 33): a tenant or vault whose `delete_after` passed is purged in batches (rows, then objects), the tenant's Transit key kept and counted in `kura_tenants_awaiting_key_destruction`; `deploy/vault/kura-server.hcl`, the server's policy, without delete | T18 `restored_document_cancels_deletion`, `server_policy_cannot_delete_key` (OpenBao), a purge removing rows and objects |
| 5.7 | Close: independent security review, §8 additions, operator guide (maintenance, restore runbook, backup lock) | — |

## 6. Phase 6 — operations

Detailed at the start of phase 6. Exit criteria (§16): the chart installs
two replicas against CloudNativePG; the §10.1 alerts exist and are tested;
the restore drill restores PostgreSQL, the bucket and the Transit keys
together, regenerates the epochs and decrypts a sample of chunks; the k6
budgets of §10.3 (a one-note commit under 200 ms at p95, propagation under
5 s) and decision 42 (a first attach of 10,000 notes under 5 minutes) are
measured. Everything that runs on the cluster is the maintainer's step;
this phase brings it ready and proven on the dev stack.

| PR | Content | Tests |
| --- | --- | --- |
| 6.1 | Metrics of §10.1 missing so far: `commit_duration_seconds`, `notify_lag_seconds` (heartbeat round trip), `notify_queue_usage` (`pg_notification_queue_usage()`), `ws_connections`, `vault_call_duration_seconds{outcome}`, `resync_total{outcome}` (410 answers), `chunk_bytes_stored{tenant}`, `dedup_ratio{tenant}` and `quota_used_ratio{tenant}`, refreshed every minute; `index_queue_depth` waits for search (phase 7) | each metric moves in an existing integration test |
| 6.2 | `deploy/helm/kura`: Deployment (2 replicas, spread, non-root, read-only root, probes), Services (API, metrics), Ingress, ConfigMaps (`server.yaml`, documents, no `subPath`), ServiceAccount for Vault, migration Job as a pre-install/pre-upgrade hook, PodDisruptionBudget, NetworkPolicy, ServiceMonitor, optional CloudNativePG `Cluster` with barman; `task helm:lint` (helm lint, kubeconform) in `task lint` | template renders validated by kubeconform |
| 6.3 | PrometheusRule: every alert of §10.1 the metrics allow | `promtool test rules` per alert |
| 6.4 | Backup CronJob (holds `BACKUP_LOCK`, CloudNativePG backup, bucket mirror); `kura-server check-chunks --sample N` (decrypts and checks random chunks); restore runbook; `task dev:drill` (dump, bucket copy, wipe, restore, `reset-epochs`, `check-chunks`, a device resyncs) | the drill on the dev stack |
| 6.5 | `perf/k6`: one-note commit, pull, 5 devices answered 410 at once; `perf/attach.sh` (a first attach of 10,000 notes with the real CLI, decision 42); `perf:soak` with RSS sampling | thresholds of §10.3, measured on the dev stack |
| 6.6 | Close: independent review, §8, operator guide (install, backup, restore, alerts) | — |

## 7. Phase 7 — scoped tokens and search

Detailed at the start of phase 7. Exit criteria (§16): T5
(`token_outside_cidr_is_refused`, `revoked_token_is_401`,
`expired_token_is_401`, `read_token_cannot_commit`), T16
(`prefix_token_cannot_fetch_outside_chunk`,
`prefix_token_cannot_read_renamed_history`), the token-use half of T10, §6.P
observation 3, `fuzz_scoped_token`.

| PR | Content | Tests |
| --- | --- | --- |
| 7.1 | Migration: `scoped_token` (hash only, tenant isolated, kept 30 days past expiry), definer lookup by id before the tenant is known; `kura_<id>_<secret>` parsed once in `kura-core` (256-bit secret, constant-time compare); `POST/GET/DELETE /vaults/{v}/tokens` (manager or tenant admin, expiry ≤ 90 days, non-empty CIDR list without `0.0.0.0/0` or `::/0`, read only, the secret shown once); a `Reader` extractor (OIDC device or key) on `GET /vaults`, `changes`, `entries`, `revisions`, `blobs`, `chunks`; address checked through `trustedProxies`; `last_used_at` at most once a minute and an audit event per use; limits per token; revoked with the binding or tenant grant it came from; a key on a write route is 403; `kura token create/list/revoke` | T5, T10 half |
| 7.2 | `authz::visible_under_prefix`: entries whose head is under the prefix, revisions made under it, blobs and chunks only through such a revision | T16 |
| 7.3 | Search (decision 39, opt-in): `unaccent`, `pg_trgm`, `search_doc`, `index_job` from the commit transaction for Markdown heads, workers claiming 100 jobs `SKIP LOCKED`, 256 KiB per note, 5 attempts; backfill when enabled, index dropped when disabled; `POST /vaults/{v}/search` (viewer or key, query in the body, at least 2 characters, at most 50 results, `websearch_to_tsquery` in French and English, path trigrams, snippets from at most 1 MiB decrypted, `409 search_disabled`); `kura_index_queue_depth` and its alert; `kura search` | index, query, prefix, disabled, failing job |
| 7.4 | Close: `fuzz_scoped_token`, observation 3 in `tests/heavy/authz.sh`, independent review, §8, guide | — |

## 8. Phase 8 — Obsidian configuration

Detailed at the start of phase 8. Exit criteria (§16): the configuration
half of T14 (`config_namespace_hidden_from_other_member`,
`received_plugin_is_disabled`, `plugin_hash_change_disables_it`); §6.P
observation 12 is the maintainer's, on desktop and the Android phone.

| PR | Content | Tests |
| --- | --- | --- |
| 8.1 | Migration: `entry.namespace` `notes` or `config:desktop`/`config:mobile` with `owner_sub`, uniqueness per namespace and owner; `config_entry_pin` (device, entry, accepted hash; isolated by subject, removed with the device). `space=config` on `changes`, `entries` and commits: the namespace is the caller's own, its profile the device's (never the client's word); paths relative to `.obsidian`, only in the vault's `configSync` categories (`plugins/…`, `snippets/…`, `themes/…`, `hotkeys.json`), never `plugins/weebo-kura/`; last write wins, no merge, no conflict file; scoped tokens and search see notes only. `GET/PUT /vaults/{v}/pins` | T14 half (hidden from another member, from a key, from another profile), categories, last write wins, pins |
| 8.2 | `kura-core` unchanged (the remote names its space); a configuration host rooted at `.obsidian` keeping the vault's categories and never `community-plugins.json`, `workspace*.json` or the Kura plugin; `kura sync` syncs the desktop profile when the vault has `configSync`; the plugin scans on resume and every 5 minutes, writes received plugins without enabling them, asks once per plugin and hash (§4.5 mock) and records the pin, disables an enabled plugin whose code hash changed until agreed again | `received_plugin_is_disabled`, `plugin_hash_change_disables_it` (plugin, fakes), CLI config round trip |
| 8.3 | Close: review, guide, §6.P observation 12 handed to the maintainer | — |

## 9. Later phases (outline, detailed when the previous one closes)

| Phase | Content (§16) | Tests and fuzz | Measurement / exit | New local services |
| --- | --- | --- | --- | --- |
| 2 | Chunks and blobs, envelope encryption, keyed per-upload object keys, write-once data key, Vault KV credentials, chunk format with codec byte, quota | T6, T8, T9 (`oversized_chunk_is_413`) | Decision 52 (chunking) and the compression gain; above 20%, plan the codec RFC | MinIO, Vault dev |
| 3 | Changes and commits under the vault lock, idempotency, conflict files; `kura-core`, `kura-cli`, device simulation | T3, T7, T14, T17, T19; `fuzz_vault_path`, `fuzz_commit_body` | Phases 1–3 give a working sync through `kura-cli`: first real use, one tenant, one vault | — |
| 4 | Server-side three-way merge; WebSocket tickets and `LISTEN/NOTIFY` (replaces the phase 1 generation poll); plugin on desktop, then Android | T11; `fuzz_merge` | Decisions 53, 54, 55 on the Android test phone | Obsidian 1.11.4 desktop + Android |
| 5 | History, restore, retention, GC, horizon, epochs, full resync and its deletion guard, `Delete` purge after 30 days | T18 | — | — |
| 6 | Helm chart, telemetry, alerts, backup and restore drill, k6 load and soak | — | Decision 42: first attach above 5 min opens the batch-download RFC | CloudNativePG on the cluster |
| 7 | Opt-in search; scoped tokens with expiry, CIDR, revocation | T5, T16, token half of T10; `fuzz_scoped_token` | — | — |
| 8 | Obsidian configuration sync per user and profile, plugin pins | T14 (configuration half) | — | — |
| 9 | Notion export, first push, one month of parallel use, cutover | `kura verify` (PR 9.1) | File count and bytes equal the export (§13): `kura verify` prints `Equal.` | — |

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

| D34 | review | §6.2, decision 35, T2 | **Confirmed by the owner (2026-11-14).** A token whose Authentik session (`sid`) has a revoked device is refused (`401 device_revoked`) on *every* device id, not only on the revoked one; a token with no `sid` is refused on every device call. Decision 35 accepted a stolen session keeping its access token for at most an hour on a non-revoked device id; without this rule the thief could use another of the user's device ids indefinitely, refreshing in Authentik (review R1). Observed on Authentik 2026.8 (2026-11-14): two device-code logins approved from one browser session carry the same `sid`, so revoking one of those devices cuts them all; devices approved from separate sessions are independent | Closes an unbounded bypass of revocation; the cost is the shared-session case, to observe on the instance | **Resolved by D48 (2026-11-14):** an access token Authentik 2026.8 issues on a refresh carries no `sid` (nor any other session claim: `acr aud auth_time azp email email_verified exp given_name groups iat iss jti name nickname preferred_username scope sub uid`), so under this rule every client is refused (`401 invalid_token`) from its first refresh; seen with the real `kura login` on the dev stack
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
| R5 | The key cache's mutex is held during the provider fetch (up to about 10 s) | Low | **Fixed** (PR 9.4): the fetch is marked, then runs with the lock released; other callers use the cached keys and start no second fetch. Test `a_slow_fetch_does_not_hold_other_callers` (fails on the old code) |
| R6 | `GET /v1/vaults` opens one transaction per vault | Low | **Fixed** (PR 9.4): `authz::vault_accesses_in`, two queries in one transaction per tenant (vaults, then their bindings with `= ANY`), the same role rule; `vault_access_in` is its one-vault case |
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
| D41 | P3 | §13, decision 48 | The local and CI secret store is OpenBao 2.7 (Vault's MPL-2.0 fork, same API) instead of HashiCorp Vault: the 113 tests pass against it. `task dev:stack` adds a local Authentik 2026.8 on https (local CA, the issuer must be https), and `task dev:token` gets a device-code token without a browser | Vault's licence is BSL; the production Vault stays the operator's choice, the client only uses the KV v2 and Transit APIs both serve |
| D42 | 3.1 | §6.3 | The wire carries NFC paths only: the server refuses a non-NFC path (`invalid_path`) instead of normalizing it, and clients normalize local names (`VaultPath::from_local`) before sending. Entry ids are built by `kura-core` from a time and random bytes the shell supplies | The RFC does not say which side normalizes; refusing on the server keeps one spelling per path in storage and makes a client bug visible. No clock or random source in `kura-core` keeps it deterministic in the simulation and free of WASM shims |
| D43 | 3.2 | §6.1, §6.5 | The idempotency table is `commit_record` (COMMIT is an SQL keyword). Phase 3 rules without merge: a `put` whose content equals the head and whose path differs is a rename and always applies (two renames: the later wins); a `delete` whose base is not the head is dropped (`conflicted`, no file); a new entry onto an occupied path with the same blob is unified (the result names the existing entry), with another blob it becomes a conflict file under the pushed id; a `base` that is not a revision of that entry in that vault refuses the commit (`400`, T17). Revisions also record their `path`, so a rename's history is kept | The RFC's table leaves these cases to the implementation; each keeps both versions and never fails a retry |
| D44 | 3.4 | §14 | The device simulation lives in `kura-server/tests/sim.rs`, driving the `kura-core` engine against the real application in process, instead of in `kura-core/tests` against a fake server; `kura-core/tests/engine.rs` keeps scripted cases. The 200-day absence waits for phase 5 | A fake server would re-implement the commit rules and could agree with a wrong engine; the real one cannot |
| D45 | 3.5 | §6.2, §6.8 | The CLI keeps its refresh token in `credentials.json`, mode 0600, in its config folder, not in the OS keyring. **Open, to fix before the CLI is used beyond the owner's machines.** | `keyring` 4 splits into one crate per store and its default Linux store needs a desktop Secret Service; the kernel keyutils store forgets the token at logout. A 0600 file is what `gh` and `kubectl` fall back to. Marked `shortcut:` in `config.rs` |
| D46 | 3.5 | §6.P obs. 11 | The client never sends a dot-path note: it skips dot-folders silently (they are Obsidian's own and phase 8's configuration namespace) instead of exiting 4. Observation 11 is proved on the server side by a direct `POST commits` (`422 invalid_path`) and on the client side by the file not reaching the other device | Every Obsidian vault has `.obsidian`: reporting it on every sync would bury real unsyncable files |
| D47 | 3.5 | §4.5 | `kura login` takes `--issuer` (and `--client-id`, default `kura-cli`) besides `--server`: no unauthenticated discovery endpoint exists on the server (§7.2). `KURA_ACCESS_TOKEN` replaces the sign-in for scripts and the heavy suite | Adding a public endpoint to save one flag would contradict §7.2 |
| D48 | 3.6 | §6.2, decision 35, T2 | Chosen by the owner (option A): a token's session is its `sid`, or, when absent (every token Authentik 2026.8 issues on a refresh), the owner's `auth_time`, which a refresh keeps (checked on the instance: same `auth_time` over two refreshes, `sid` gone). Migration `0008` records `device.session_auth_time`; a revoked session is matched by `sid` or by (`user_sub`, `auth_time`); a token with neither claim is refused. Tests `refreshed_token_without_sid_follows_its_session` and the heavy suite's real sign-in (refreshed tokens accepted, refused after revocation) | Without it every client was refused from its first refresh; `auth_time` is the only session-stable claim left. Cost: two sessions of one user signing in in the same second share a revocation (option B, dropping the check, would reopen R1) |
| D49 | 4.1 | §6.3 | The 2 s merge budget is one deadline per commit, not 2 s of CPU per operation: past it, the remaining diverging edits become conflict files. Merges run four at a time per replica. A merged blob whose head moved before the lock is left unreferenced, for the phase 5 collector (with P5) | `spawn_blocking` cannot be cancelled, and 1,000 operations at 2 s each would outlast every client timeout; the semaphore bounds memory to 4 × 12 MiB |
| D50 | 4.2 | §6.3, §6.10 | A socket's "16-message queue" is the replica's broadcast channel of 256 events: a socket more than 256 events behind is closed (4009) and pulls on reconnect. The configuration snapshot keeps its phase 1 generation poll instead of moving to `LISTEN`. The slow-consumer close and the pull after a lost `LISTEN` have no automated test | One channel per replica keeps the fan-out allocation-free; the poll is one indexed read and works when `LISTEN` does not; forcing a lag or a dropped `LISTEN` deterministically needs fault injection the suite does not have yet |
| D51 | 4.3 | §6.3 | While the socket is down, `kura watch` pulls at each reconnection attempt (1–60 s, decorrelated jitter) rather than on a separate 60 s ±20% timer; local changes are found by a 5 s scan of the folder's sizes and times, not by file-system events | One loop instead of two timers; the scan reads metadata only, and a file-system watcher would add a dependency the plugin does not need (Obsidian gives it events) |
| D52 | 4.5 | §4.1, §6.2 | Two client types mean two Authentik applications, each with its own issuer by default, while the server checks one. Both providers run in Authentik's **global issuer mode** (one issuer, the base URL) and `server.yaml` gains an optional `oidc.discovery`, since that base URL serves no discovery document (checked on the dev instance: 404). Clients keep discovering from their own application URL | Accepting several issuers would widen token validation; one issuer and an explicit discovery URL keep `iss` exact |
| D53 | 4.5 | §6.8 | The plugin writes a remote file through a temporary file in its own folder, then removes the old file and renames: Obsidian's adapter has no atomic replace, so a crash between the two steps leaves the old file gone and the new one in the plugin folder, which the core's journal adopts on the next run. Coverage excludes `main.ts` and `settings.ts`, Obsidian glue with no decision of its own. The plugin has not yet run inside Obsidian | The adapter is the only file API on mobile; the gate measures the modules that decide; running in Obsidian is PR 4.6's proof on the maintainer's devices |
| D54 | 4.6 | §6.5, §7.3 | A three-way merge also needs at most 20,000 lines a side (`MAX_MERGE_LINES`), besides 4 MiB: `diffy` is quadratic in lines when the sides share few (0.1 s at 4,000 lines a side, measured), so 4 MiB of short lines could take minutes. Longer files become conflict files | Bounds a merge to seconds without replacing the library decision 43 chose |
| D55 | 5.1 | §6.3 | A commit may also answer `410`: `history_rewound` when it carries another epoch, `cursor_expired` when it names an entry the vault does not hold with a base once deletions were purged. A base pruned by retention, on an entry that exists, is no longer refused (T17) but handled as overlapping edits | The push runs before the pull: without these answers a device that edited a purged note, or whose base was pruned, would be refused on every run and never reach its resync. A foreign base on an existing entry now becomes a conflict file of the caller's own content, which reveals nothing |
| D56 | 5.5 | §6.3 | After `history_rewound`, a version the server lost comes back as a conflict file beside the older head the server kept (the server cannot tell they follow each other), and a deletion the server lost is undone (the file is pushed again). A commit whose blob and path equal its base's keeps the head (`accepted`), so a resent unchanged file takes the server's version | Decision 32's "nothing local is ever deleted" over a tidy history: both texts survive. The base rule only matches a client that resends unchanged content, which a normal push never does |
| D57 | 5.3, 5.4 | §6.9 | Maintenance runs hourly inside a `server.yaml` window (default 01:00–05:00 UTC), one pass at a time across replicas, and also on demand (`kura-server maintain`). Retention walks one vault at a time. `blob` gains `last_touched`, which §6.1 does not list. The orphan sweep lists each vault's prefix, never the whole bucket | One pass at a time keeps two replicas from racing the same batches; per-vault batches stay under the 5 s `statement_timeout`; a blob stored again before its commit must not be collected; a bucket may be shared (the tests share one) |
| D58 | 5.6 | §6.10, decision 33 | A tenant purged by `Delete` keeps its row, marked `purged_at`, counted in `kura_tenants_awaiting_key_destruction` until an operator destroys its Transit key; only its vaults' rows and objects are removed | The alert "a tenant awaiting Transit key destruction" needs a record of the tenant; the row holds no content |
| D59 | 5.2 | §4.4, §9 | `revisions` returns no device or subject, only `createdAt`; `restore` reads the head before taking the lock, so a commit landing between the two makes the restore a conflict file rather than a retry | A viewer learns nothing about other users' devices (§9); the race needs two writers within milliseconds and keeps both versions |
| D61 | 5.6 | decision 33 | A tenant removed with `Delete` purges its vaults whatever their own `deletionPolicy`, except a vault annotated force-orphan; the first purge batch gives the vault a new epoch, and an entry leaves with its whole history in one batch | The tenant's policy decides for its content; the annotation is the operator's explicit "keep". A document returning mid-purge then sends every device into `history_rewound`, which pushes its copies back instead of trashing them |
| D62 | 6.1, 6.3 | §10.1 | Dependency alerts come from one counter of 503 answers (`unavailable_total`) plus Vault's own call errors, not one series per dependency: the `warn` line of each 503 names its cause. Traces are the request span in the logs (method, route template, status, duration); there is no OpenTelemetry export. `index_queue_depth` and its alert wait for search (phase 7) | Attributing every failure to a dependency would touch the 32 places that map one; the logs already name it. A trace exporter adds a dependency and a collector the cluster does not run yet |
| D63 | 6.4 | §6.9, §10.3 | The bucket backup is `kura-server backup`, which holds the backup lock only while it copies (and checks it still holds it), records T and the keys at T, and the restore recovers PostgreSQL to T through CloudNativePG's WAL archive, instead of a job holding the lock across a database dump and the bucket copy. The second bucket is never pruned by the tool | The image needs no shell, `psql` or `kubectl`; continuous archiving makes T as good as a dump taken under the lock. Pruning the copy safely needs the oldest kept base backup, an operator fact (guide §12) |
| D64 | 6.5 | §10.3 | k6 commits reference a blob already stored, since k6 cannot hash BLAKE3: uploads are measured by `perf/attach.sh` with the real CLI. The numbers come from the dev stack on one laptop, not the cluster; resource requests stay at the chart's defaults until the cluster run | The commit budget is about the commit path; the client upload path is what decision 42 measures |
| D66 | 6.6 | §6.3, §4.4 | `GET /changes` takes an optional `start`, the cursor a paged pass began from; the horizon is checked against it, not against each page's `since` | A pass from 0 must cross the horizon page by page. A client lying about `start` only misses deletions on its own device, so nothing else needs checking |
| D67 | 7.1 | §6.2 | A token's id is a UUIDv7 in hex and its secret 64 hex digits (not base32 or base64): one strict spelling, no padding, parsed with no dependency. The per-address limit counts a token's requests too, at its route's limit | The format is the server's to choose; one spelling makes `fuzz_scoped_token`'s round trip exact |
| D68 | 7.2 | §6.2 | An entry renamed out of a token's prefix stops appearing in the token's `changes`, so a holder that synced it keeps its last content until a full resync; nothing new is shown. Content checks look at the first 200 revisions naming it and refuse past that | Sending a deletion for it would need a path inside the prefix the entry no longer has; refusing on doubt keeps T16 closed |
| D69 | 8.1 | §6.1, §13 | Configuration entries live in `entry` with `namespace` and `owner_sub`, not a table of their own, and the `entry` row policy now also checks the owner: a new condition on an existing write path, which §13 rules out. It only narrows what the serving role sees to rows it may see; an older replica during a rollout still sees no other user's configuration | One table keeps every query, history, retention, garbage collection and backup path unchanged; the policy is the one place no query can forget |
| D70 | 8.1 | §6.8, T14 | Content is checked against entries the caller can see, not against who uploaded it: a member who knows the exact bytes of another member's configuration file can commit them into their own space. `head_seq` and notifications are per vault, so they show that someone changed something | Content addresses are never exposed across members, so this needs the content already; per-owner content would split deduplication |
| D71 | 8.2 | §6.8, decision 34 | Consent is kept on the device (the plugin's `data.json`, never synced), not in a server `config_entry_pin`: a pin written through the API from another session would otherwise enable code here. It covers a plugin's `main.js`; its `data.json` and CSS apply without asking, though some plugins' settings run code | A server record adds nothing a stolen session could not forge; asking for every settings change would train users to click through |
| D72 | 9.1 | §4.5 mock `kura sync --dry-run ./notion-export` | `kura sync` names a vault, not a folder, so the check before the first push is `kura attach <vault> <folder>` answered no: it lists the unsyncable names and the bytes it would push and changes nothing. `kura sync --dry-run` gives the same check once attached. The §13 count check is `kura verify`, which compares content too, not only counts and bytes |
| D65 | 6.2 | §13 | The NetworkPolicy restricts ingress only; egress stays open. The CloudNativePG `Cluster` is created by the chart (off with `cnpg.enabled=false`) | Egress goes to PostgreSQL, the bucket, Vault and Authentik, whose addresses are the platform's; the chart cannot know them. One chart installs the whole service, as the RFC's prerequisites list it |
| D60 | 5.1 | decision 32 | The timeline is read from the current WAL file name (`pg_walfile_name(pg_current_wal_lsn())`), not `pg_control_checkpoint()`; the check runs on every new pool connection and every `LISTEN` heartbeat, not on a schedule of its own | The checkpoint's timeline lags a promotion until the next checkpoint; a failover breaks every connection, so commits after it run on connections that checked |

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

### Phase 3 security review (2026-11-14)

Independent review of the phase 3 code. Server side checked sound: forced
row-level security and composite tenant keys on the three new tables, `seq`
under the vault lock with `UNIQUE (vault_id, seq)`, writes in operation order
for the unique path index, idempotency scoped to vault, device and key with the
whole parsed body hashed under the lock, the `REPEATABLE READ` snapshot taken
before any read, `kura_commit_purge` scoped.

| # | Finding | Severity | Status |
| --- | --- | --- | --- |
| S1 | A pulled entry moving onto a path this device still holds for another entry (whose own move comes later in the log) either blocked the pull for good (rename onto an existing file) or overwrote that entry's file, which could later be pushed as the other entry's content | high | **Fixed**: an untouched holder's file goes to `.trash` and the entry is forgotten, its own change brings it back; the simulation's host now refuses a rename onto a file, as `FsHost` does; test `a_path_held_by_another_known_entry_is_handed_over` |
| S2 | One local write error (a 245–255-byte name overflowing the temp name; a file and a folder of the same name) aborted the whole pull before the cursor was saved, on every run | high | **Fixed (client)**: the temp file is `.kura/tmp-<random>`; a failing item is reported (`failed`, CLI exit 1), the saved cursor stops before it and paging continues, so later changes still arrive; tests `an_unwritten_item_holds_the_cursor`, `a_255_byte_name_is_written`. **S2b open**: the server still accepts an entry `a` beside `a/b.md`; refuse the prefix clash in commits (one batched query) |
| S3 | A deferred remote entry unknown to this device was skipped for good once the cursor passed it | medium | **Fixed**: the cursor stops before it (same test) |
| S4 | A conflict or occupied-path suffix could exceed 255 bytes (64 four-byte characters in a device name, a long text after the last dot) or run out of numbered suffixes, failing the whole commit on every retry | medium | **Fixed**: `suffixed` bounds the suffix to 64 bytes and keeps only extensions of 16 bytes or less; device names are 1 to 64 bytes; after 9 numbered suffixes the entry id makes the name unique; tests `suffixes_always_fit`, `a_crowded_name_still_gets_a_unique_one` |
| S5 | T14 bypass on some file systems: `OBSIDI~1/…` (an NTFS 8.3 name) or `\u{200C}.obsidian/…` (HFS+ drops default-ignorable characters) | medium | **Fixed**: `VaultPath` refuses default-ignorable code points and `NAME~<digits>` short names, as git does; test `invisible_and_short_names_cannot_stand_for_a_dot_folder` |
| S6 | A hostile server could exhaust the client's memory (capacity from its stated size, unbounded bodies) or loop it forever (pages that do not advance) | low | **Fixed**: capacity bounded by one chunk, JSON bodies ≤ 8 MiB and chunks ≤ 4 MiB as they stream, files over 2 GiB unsyncable both ways (`shortcut:` until downloads stream), pull and attach stop on a page that does not advance |
| S7 | Grouped: `free_path` up to 99 queries per operation under the lock; puts before deletes (a freed path came back suffixed); `.trash`/`.kura`/temp files created through planted links; a stamp taken after a settle rename could mark an edit made meanwhile as synced | low | **Fixed**: 9 numbered tries then the id; deletions first; `.kura` and `.trash` created folder by folder refusing links, temp files `create_new`; a renamed file whose stamp changed gets none, so the next run rehashes it |
| S8 | Downloads and uploads hold a file whole in memory | low | **Open**: stream by chunk before raising the 2 GiB CLI limit (phase 4, with the plugin's own limit) |

### Phase 4 security review (2026-11-14)

Independent review of the phase 4 code. Checked sound: a stale merge refused
under the lock (head rechecked, 3 attempts), idempotency before the merge and
under the lock, merge inputs read inside the vault's tenant, sizes checked
before reads; tickets of 256 bits, hashed, consumed once before the upgrade,
bound to vault, device and token expiry, a socket receiving only its vault's
events, the ticket never logged; the `auth_time` fallback scoped to the
subject twice; server-supplied paths re-parsed before the plugin writes; the
refresh token only in `SecretStorage`; the WASM boundary free of panics.

| # | Finding | Severity | Status |
| --- | --- | --- | --- |
| V1 | A merge past its deadline kept running on the blocking pool with its buffers after its semaphore permit was released, and `diffy` has no cost cap (quadratic in lines): an editor pushing disjoint versions could exhaust the pool for every tenant | high | **Fixed**: the permit moves into the merge's thread (released only when it ends), acquiring it is under the deadline, and a merge needs at most 20,000 lines a side (D54); test `overlapping_binary_or_large_content_conflicts` |
| V2 | An open note in conflict: the local move of the conflict file failed, aborted the run before the state was saved, and the next run created one more conflict file — about 120 a minute with the commit's own notification looping | high | **Fixed**: a settle failure fails that file only and the batch is saved; the server answers the same conflict file for the same content pushed again; tests `the_same_content_pushed_again_reuses_its_conflict_file`, `a_conflict_on_an_open_note_fails_that_file_only` |
| V3 | The socket route takes no token, so its limit was never applied, and each random ticket ran the consume query on the 16-connection pool | medium | **Fixed**: 60 attempts a minute per client address, checked before any database work; test `a_ticket_flood_is_limited_per_address` |
| V4 | A socket outlived a revocation that happened after its ticket was minted, or of a device sharing its session | low | **Fixed**: `kura_device_active` (migration `0010`) on every 30 s ping closes it (4002). The vault role is re-checked on the same ping since phase 9 (migration `0016`: the ticket carries the token's claims for at most 30 s; close 4005 `access_revoked`; test `a_socket_closes_when_its_caller_loses_the_vault`). A failing dependency keeps the socket open |
| V5 | A merged result could exceed 4 MiB or the tenant's `maxFileSize` | low | **Fixed**: past either, a conflict file |
| V6 | `kura watch` exited on a transient error and reconnected without delay after a socket closed at once | low | **Fixed**: only sign-in and refusals end it; other errors and early drops back off 1–60 s |
| V7 | Plugin: the temporary `pending` secret was never cleared; an offline start synced with default chunking; `http://` accepted | low | **Fixed**: cleared after the copy and on sign-out; the vault list is fetched again until known, never guessed; https required (loopback excepted), test `sends tokens over https only` |
| V8 | The server trusted the discovery document: its `issuer` was not compared and its endpoints could be `http` or another host | low | **Fixed**: `KeySource::expecting(oidc.issuer)` refuses another issuer; endpoints must be https (http only on loopback, for tests) |
| V9 | A hostile manifest could grow the client's buffer past the stated size before the final check | low | **Fixed**: the running length is checked before each chunk is kept |

### Phase 5 security review (2026-11-14)

| Id | Finding | Severity | Outcome |
| --- | --- | --- | --- |
| W1 | A vault whose document returned mid-purge kept entries without revisions (invisible, refusing pushes) and its old epoch: after a deleted-entry purge had moved the horizon, devices got `cursor_expired` and trashed their untouched copies, the last ones | high | **Fixed**: an entry leaves with its history in one batch; the first batch sets `purge_started_at` and a new epoch, so returning devices resync under `history_rewound` and push back; test in `restored_document_cancels_deletion` |
| W2 | The vault row was deleted even when listing or deleting its objects failed, and the orphan sweep only visits vaults with a row: objects kept forever | high | **Fixed**: errors from the listing fail the purge, and the row goes only once every object is deleted; retried next pass |
| W3 | Any error in one vault (a timeout, an overflowing interval from an unbounded duration) ended the pass for every vault after it, every hour | medium | **Fixed**: errors isolated per vault, counted in `kura_maintenance_failures_total`; durations capped at 100 years. Shortcut kept: a prune batch ranks the vault's history, noted in the function |
| W4 | A failed epoch check on a new connection was logged at debug level and the connection used | low–medium | **Fixed**: the connection is refused, except before migration 0011 (42883) |
| W5 | The prune and purge functions take their policy as parameters: a bug in the app could empty a history | low | **Fixed**: below one hour, one revision or one day, they remove nothing; test in `retention_prunes_history_but_never_a_head` |
| W6 | A tenant set to `Delete` purged its force-orphan vaults too | low | **Fixed**: the annotation keeps the vault (D61) |
| W7 | (follow-up) The epoch changed only on a vault's first purge ever, and a `resend` flag on a device whose push is refused (a viewer) held its pull forever and let a later `cursor_expired` resync trash the file | medium | **Fixed**: every purge batch changes the epoch (tested over two cycles); a refused push forgets `resend` entries, which a resync never trashes; test `a_refused_resend_keeps_the_local_version`. CLI resync messages now follow the §4.5 mocks |

### Phase 6 security review (2026-11-14)

| Id | Finding | Severity | Outcome |
| --- | --- | --- | --- |
| X1 | The runbook pruned the second bucket by key age, but a chunk keeps its key for as long as a note uses it: every kept restore point would lose old content | high | **Fixed**: `backup` writes `backups/<T>.keys`; the guide prunes only keys no kept list names |
| X2 | The migration hook ran pre-install, before the CloudNativePG cluster and its owner Secret existed: a first install hung | high | **Fixed**: `post-install,pre-upgrade`; the server waits meanwhile (it refuses an older schema) |
| X3 | The runbook no longer ordered the Vault snapshot: one older than T may lack a tenant's Transit key | medium | **Fixed**: snapshot after each copy; restore the first one after T |
| X4 | A dropped session releases the backup lock silently; maintenance could delete objects while the copy still wrote a manifest | medium | **Fixed**: the lock is checked on the same session before the key list and manifest are written (`LockLost`) |
| X5 | Backups were off by default and their failure unalerted | medium | **Fixed**: the chart refuses to render without both backups unless `backupsWaived`; `backup_run` records each copy, `KuraBackupMissing` after 36 hours |
| X6 | `kura_sample_chunks` (content addresses of every tenant) was granted to `kura_app`, and could time out | medium | **Fixed**: owner only; `check-chunks` takes the owner's URL (no statement timeout) |
| X7 | `notify_lag_seconds` was only set when a beat came back, so a stall froze it under 30 s | medium | **Fixed**: each tick raises it by how late the beat is |
| X8 | PostgreSQL had no NetworkPolicy | medium | **Fixed**: kura's pods, its own instances, the operator's namespace, the exporter port for monitoring |
| X9 | `KuraVaultFailing` needed errors in every evaluation for 2 minutes; the RFC says any failure | low | **Fixed**: any error in 10 minutes |
| X10 | Storage gauges could go stale silently | low | **Fixed**: `storage_refreshed_timestamp_seconds`, `KuraStorageMetricsStale` |
| X11 | Editing a document restarted the pods (the checksum covered both ConfigMaps) | low | **Fixed**: the checksum covers `server` only |
| X12 | Job pods carried the server's labels, so the PDB, Services and ServiceMonitor counted them | low | **Fixed**: `app.kubernetes.io/component` (`server`, `migrate`, `backup`) |
| X13 | The dev drill could delete a vault's objects missing from the copy | low | **Fixed**: it checks every vault holding chunks is copied before deleting; its order is documented (no WAL archive on the dev stack) |
| X14 | The perf cleanup revoked every `perf-%` device, including a concurrent run's | low | **Fixed**: device names carry the run id |
| X16 | (found by the heavy suite once `notes` passed 1,000 entries with purged deletions) A pass from cursor 0 got `410 cursor_expired` on its second page, since that page's `since` was below the horizon: attaching or resyncing such a vault failed every time | high | **Fixed**: `changes?start=` names the pass's first cursor, and the horizon is checked against it (D66); test `a_pass_from_zero_pages_past_the_horizon`; core, CLI, WASM, plugin and k6 send it |
| X17 | (found by `perf/attach.sh`) The CLI refreshed its access token once per command: a run longer than the token's life (an attach of 10,000 notes) failed with 401 | medium | **Fixed**: on `invalid_token` the CLI renews from the stored refresh token (rotated, stored first) and sends the request again, once; never with `KURA_ACCESS_TOKEN`; test `an_expired_access_token_is_renewed_mid_run` |
| X15 | (found by `perf/attach.sh`) `POST /chunks/missing` with its 1,000 hashes (about 67 KB) and large manifests exceeded the 64 KiB body limit: 413, and a first push of more than about 950 files failed | high | **Fixed**: those routes take 1 MiB, like commits; test in `upload_resumes_from_missing_chunks_and_manifests_check_out` (413 before, 200 after) |

### Phase 7 security review (2026-11-14)

| Id | Finding | Severity | Outcome |
| --- | --- | --- | --- |
| Y1 | A malformed `X-Forwarded-For` behind a trusted proxy made the proxy's own address the client: a token bound to the pod range let anyone through the ingress | medium | **Fixed**: `net::client_addr_strict` for the CIDR check: behind a trusted proxy the client must be named by a well-formed header and be no proxy, else refused |
| Y2 | Any well-formed key cost a database lookup before any limit | medium | **Fixed**: the route's limit per client address applies before the lookup |
| Y3 | Renaming `x.md` to `x.txt` queued no job, so its words stayed in the index and its dumps | medium | **Fixed**: every write is queued; the worker drops non-Markdown heads; test |
| Y4 | `kura_index_set` and `kura_index_failed` let the serving role write words for, or fail the jobs of, any vault | low | **Fixed**: a claimed job and the entry's current head are required; test |
| Y5 | An entry renamed out of a token's prefix stays in the token holder's copy until its full resync | low | Recorded (D68) |
| Y6 | Search queries had no length bound | low | **Fixed**: 2 characters to 256 bytes; the role's 5 s statement timeout already bounds a query |
| Y7 | The CLI printed snippets and paths written by others with their control characters | low | **Fixed**: stripped |
| Y8 | A snippet could start at the top of a note when lower-casing changed byte lengths | info | **Fixed**: located on characters; test |

### Phase 8 security review (2026-11-14)

No path found from one member to another's configuration: row-level security, explicit filters and `kura_entry_home` hold on every query traced.

| Id | Finding | Severity | Outcome |
| --- | --- | --- | --- |
| Z1 | A new device pushed its default `.obsidian` files over the user's settings (push before pull, a new id on a held path wins), and a configuration entry could not be restored | high | **Fixed**: `prefer_remote` for configuration (pull first, the server's copy wins on first contact, the local one to `.trash`); restore resolves the entry's space for its owner; tests |
| Z2 | `MAIN.JS` or `WEEBO-KURA` slipped past the guard and the Kura exclusion on a case-insensitive file system, up to rewriting Kura's own consent record | high | **Fixed**: categories, the Kura exclusion and the plugin guard compare without case, server and clients; tests |
| Z3 | `review` turned on a plugin the user had turned off whenever its settings synced | medium | **Fixed**: only plugins the guard itself turned off, or newly agreed to; test |
| Z4 | A plugin listed in `community-plugins.json` but not loaded read as off, so new code landed without a disable | medium | **Fixed**: the registry reads that file too, and disabling removes the plugin from it |
| Z5 | A plugin's `data.json` can carry code-like settings (startup scripts) and syncs without asking | medium | Recorded (D71): consent covers `main.js`; a user's own compromised device is T14's accepted residual |
| Z6 | The Kura plugin installed under another folder name would sync its own data | medium | **Fixed**: the plugin's real folder is never kept |
| Z7 | Knowing another member's configuration content hash lets one commit it into one's own space and read it | low | Recorded (D70) |
| Z8 | Notification timing and `head_seq` reveal when another member changes their configuration | low | Recorded (D70) |
| Z9 | Content reads now check an entry names the content, for every reader (a query per chunk) | low | Accepted: one indexed lookup; uncommitted content is no longer served, which no client reads |

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

### §8 additions (phase 3)

| Crate | Version | Licence | Note |
| --- | --- | --- | --- |
| `unicode-normalization` | 0.1.26 | MIT OR Apache-2.0 | NFC paths (§6.3), in `kura-core`, so in WASM too |
| `caseless` | 0.2.2 | MIT | Full Unicode case folding for `path_key`, server only |
| `uuid` features `v4`, `v7` | 1.29 | MIT OR Apache-2.0 | already in the tree; revision and conflict entry ids on the server |
| `clap` | 4.6 | MIT OR Apache-2.0 | `kura-cli` arguments, `derive` only |
| `diffy` | 0.5.2 | MIT OR Apache-2.0 | §8, decision 43; three-way merge, server and fuzz only (feature `merge`) |
| `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys` | 0.2.130, 0.4, 0.3 | MIT OR Apache-2.0 | §8; the plugin's WASM core |
| `tokio-tungstenite` (with `tungstenite`, `rand` 0.9) | 0.29 | MIT | WebSocket server (axum `ws`) and `kura watch`; the version axum links |
| plugin (npm, dev only) | obsidian 1.14.4, typescript 6.0.3, esbuild 0.28.2, vitest 5.1.0, eslint 10.14.0, typescript-eslint 8.72.2 | MIT, Apache-2.0 | §8; nothing at run time but the bundle; `pnpm audit` clean |
| `reqwest` features `form`, `query` | 0.13 | MIT OR Apache-2.0 | already in the tree; the token endpoint and `?since=` |

`cargo deny` and `cargo audit` pass with them on 2026-11-14.

### §8 additions (phase 5)

| Crate | Version | Licence | Note |
| --- | --- | --- | --- |
| `futures-util` | 0.3 | MIT OR Apache-2.0 | already in the tree (axum, sqlx); moved from dev to run-time dependencies of `kura-server` to page bucket listings in the orphan sweep |

No new crate. `.gitleaks.toml` extends the default rules with one allowlist, Spectral's OWASP rule names (`apiN:2023-…`), which `generic-api-key` flagged in the graphify index and failed CI on `5af5497`. `cargo deny`, `cargo audit`, `pnpm audit` and Trivy pass on 2026-11-14.

### Security exceptions

| # | Date | Advisory | Scope | Why it is safe | Expires |
| --- | --- | --- | --- | --- | --- |
| E1 | 2026-11-14 | RUSTSEC-2026-0459 (`libsqlite3-sys` 0.37, C SQLite) | **Retired the same day**, no ignore left | It was a lockfile-only entry of `sqlx` 0.9.0 (optional `sqlx-sqlite`). `[patch.crates-io]` now points `sqlx-sqlite` at an empty stub (`patches/sqlx-sqlite`, rung 3 of the triage ladder), so neither `Cargo.lock` nor any scanner sees SQLite; `deny.toml` bans `libsqlite3-sys` | Remove the stub when sqlx stops listing the driver |
