# RFC 0004 — implementation plan

The phases of [RFC 0004](rfc0004-scoped-token-folders-and-write.md) §16, kept
up to date as they land. Commits are made by the maintainer (signed), so
"done" means green in the working tree.

## Progress

| Item | Status | Date | Notes |
| --- | --- | --- | --- |
| Phase 1 | done | 2026-11-17 | Migration `0020` (`name`, `scopes`, legacy `'/'`, a trigger filling both for an older binary's insert, `schema_floor` read by `db::check_schema`), `authz::PathScope` (`read: None` is the only whole-vault form), the read sites on `LIKE ANY(scope.like_patterns())`, `visible_under_prefix` and `like_prefix` removed, `name`/`scopes` in the token API. Tests: unit `a_scope_reads_and_writes_under_its_folders_only`, `empty_scope_sees_nothing` (T7), `like_patterns_escape_wildcards`, `scopes_are_checked` (every row of §4.3), `names_are_checked`; integration `multi_folder_token_reads_its_folders_only` (changes, entries, revisions, chunk, blob, blob batch; T7 on a stored row; T8 `legacy_prefix_matches_no_path`; an older binary's insert) |
| Phase 2 | done | 2026-11-17 | `scopedTokens.write` in `kura-api`, `Writer` in `app/reader.rs` (the only place a token is given `editor`), `token_upload` and its daily purge, the possession rule on `chunks/missing`, `PUT blobs` and commits, the path checks before any merge and again on the locked heads, `kura_token_writes_total{outcome}`, `TokenWriteDisabled` on the tenant in `GET /v1/admin/status`. Tests: `token_cannot_write_outside` (T1 four ways plus a read-only folder and a sibling folder, the whole-commit refusal, T5, T9, T10, idempotent retry by token id, the switch off and on), `token_cannot_launder_content` (T2 chunk and blob, T3, T6 purge), `only_writer_routes_accept_tokens` (T4 over `limits::LIMITS`, T6 rate limit) |
| Phase 3 | done | 2026-11-17 | `kura token create --name --read --write` (`--prefix` kept as one `--read`), the list with `r:`/`w:`, `KURA_TOKEN` and `--token-file` for `attach` and `sync` (no login, no device, server from `KURA_SERVER`), the local refusal before any upload through `sync::Options::writable`. Operator guide: `scopedTokens.write`, the kill switch, the bot recipe. Tests: CLI `a_scoped_token_syncs_its_folders_and_refuses_the_rest`, `the_cli_issues_lists_and_revokes_scoped_tokens` (the two refusals of §4.5). **§6.P**: `tests/heavy/keys.sh`, observations 1 to 6 all `ok` on the dev stack on 2026-11-17 |
| Perf | done | 2026-11-17 | `task perf:run:commit` on the dev stack: p95 22.8 ms, p99 30.5 ms, 0 % errors (thresholds 200 ms, 500 ms, 1 %) |
| authz.sh | done | 2026-11-17 | `tests/heavy/authz.sh` `ok` on the dev stack, once its `notes` vault was re-created (D7) |

A code review of the branch on 2026-11-17 found that a change outside the
write folders stopped the whole sync: it is now kept local and reported
(`Report::not_writable`), and the pull still runs (CLI test extended); a
token no longer tries the configuration space; the CLI's hints for a token
were corrected. Writes refused by the tenant switch stay `403
insufficient_role`, as §6.P observation 6 states.

## Divergences

| # | Phase | RFC section | Divergence | Reason |
| --- | --- | --- | --- | --- |
| D1 | 2 | §6.1 (`tenant.token_write` set by the reconciler) | No column: `Writer` and `create_token` read `scopedTokens.write` from the reconciled snapshot, as search reads `search.enabled` | The snapshot is already the authorization cache every replica reads (RFC 0001 decision 31); a column would be a second copy of the same fact |
| D2 | 2 | §6.4 ("`422 invalid_request`") | The possession refusals are `422 hash_mismatch`, detail `content not uploaded by this token`, the same for an existing and an unknown hash | `invalid_request` is a `400` in the problem table; `hash_mismatch` is the `422` a blob or commit naming unstored content already gets, so a client handles one kind |
| D3 | 2 | §6.5 ("the base revision is visible") | Checked for a token on the base the operation names, under the lock, beside the head | Same rule; the base is where a merge reads from |
| D4 | 3 | §6.6 | `GET /vaults` shows a token its own `scopes`; the CLI derives its local refusal from them | The CLI has no other way to know the folders of the token it was handed |
| D5 | 3 | §6.6 | The bot's server comes from `KURA_SERVER` when the CLI has no configuration; `--token-file` is a global flag | A token holder never runs `kura login`, which is what writes the server today |
| D6 | 3 | §4.5 (`campaign  token discord-recap  1 pushed`) and §14 (`fuzz_scoped_token` extended with `scopes`) | Not done: `kura sync` prints its usual lines, and the fuzz target is unchanged | `scopes` is parsed by serde with `deny_unknown_fields` and checked by `check_scopes`, both unit-tested; left for a follow-up **Closed 2026-11-17**: `kura sync` with a token prints `campaign  token discord-recap  1 pushed  0 pulled` (`GET /vaults` gives a token its `tokenName`); the scope rules moved to `kura_core::scope` (feature `scope`, shared by server and CLI) and `fuzz_scoped_token` now drives prefixes, rules and paths through them (795 k runs, no finding). |
| D7 | 3 | §6.P (the vault `campaign` of the dev stack) | `keys.sh` creates a vault `keys-<pid>` through the API and deletes it at the end | A dev OpenBao restart loses the Transit key, leaving the seeded vaults unreadable; a fresh vault gets a fresh data key and makes the suite independent of earlier runs |
| D8 | 1 | §4.3 (`name` 1 to 64 bytes) | Also refused: `/`, `\`, control characters, and a name that cannot suffix a conflict file name | The name suffixes conflict files (§6.3), like a device name |
