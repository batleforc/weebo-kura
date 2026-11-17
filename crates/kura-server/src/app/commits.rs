//! `POST /v1/vaults/{v}/commits` (RFC 0001 §6.3, §6.5): one transaction under
//! the vault row lock, consecutive `seq` values, idempotency per device.
//! Phase 3 has no merge: an edit that diverged from the head is stored as a
//! conflict file (phase 4 adds the three-way merge).

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use kura_api::VaultRole;
use kura_core::wire::{
    CommitRequest, CommitResponse, EntryState, OpResult, Operation, Outcome, RestoreRequest,
    RevisionId, RevisionInfo, RevisionsResponse,
};
use kura_core::{ChunkHash, EntryId, VaultPath};
use sqlx::Row as _;
use uuid::Uuid;

use super::SharedState;
use super::body::{Body, Raw};
use super::chunks::{read_blob, store_content};
use super::device::DeviceCaller;
use super::reader::{KeyCaller, NOT_UPLOADED, Writer};
use crate::authz::{self, VaultAccess};
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};

/// Operations per commit (§4.4).
pub const MAX_OPERATIONS: usize = 1000;
/// A commit body: 1,000 operations of a few hundred bytes each.
pub const MAX_COMMIT_BYTES: usize = 1 << 20;
/// CPU time for all the merges of one commit (§6.3); past it, the remaining
/// diverging edits become conflict files.
const MERGE_BUDGET: std::time::Duration = std::time::Duration::from_secs(2);
/// Attempts when a merged head moves before the lock (§6.3).
const MERGE_ATTEMPTS: u32 = 3;
/// Merges running at once on this replica, each up to 3 × 4 MiB in memory.
/// The permit moves into the merge's thread: a merge past the deadline keeps
/// its slot until it ends, so memory stays bounded (review 4 H1).
static MERGES: std::sync::LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    std::sync::LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(4)));

/// Numbered suffixes tried for an occupied path, each one query under the
/// vault lock; past them, the entry's own id makes the name unique.
const MAX_SUFFIX: u32 = 9;

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

/// The case-folded form paths are unique on (§6.3), computed by the server only.
pub fn path_key(p: &VaultPath) -> String {
    crate::authz::fold(p.as_str())
}

/// An entry as this commit sees it: from the database, then as edited.
#[derive(Debug, Clone)]
pub(super) struct Head {
    pub(super) id: Uuid,
    path: VaultPath,
    key: String,
    revision: Uuid,
    pub(super) seq: i64,
    deleted: bool,
    blob: Option<ChunkHash>,
    size: i64,
}

impl Head {
    pub(super) fn state(&self) -> Result<EntryState, ApiError> {
        Ok(EntryState {
            entry: EntryId::from_uuid(self.id)
                .ok_or_else(|| ApiError::Unavailable("stored entry id is not a UUIDv7".into()))?,
            revision: self.revision,
            seq: u64::try_from(self.seq).unwrap_or_default(),
            path: self.path.clone(),
            deleted: self.deleted,
            blob: self.blob,
            size: u64::try_from(self.size).unwrap_or_default(),
        })
    }
}

/// One new revision, in seq order.
struct Write {
    head: Head,
    base: Option<Uuid>,
}

struct BaseRev {
    entry: Uuid,
    path: String,
    blob: Option<ChunkHash>,
}

fn hash_of(raw: Option<Vec<u8>>) -> Result<Option<ChunkHash>, ApiError> {
    raw.map(|b| {
        <[u8; 32]>::try_from(b.as_slice())
            .map(ChunkHash::from_bytes)
            .map_err(|_| ApiError::Unavailable("stored hash is not 32 bytes".into()))
    })
    .transpose()
}

fn stored_path(s: &str) -> Result<VaultPath, ApiError> {
    VaultPath::parse(s).map_err(|e| ApiError::Unavailable(format!("stored path refused: {e}")))
}

pub(super) fn head_from_row(r: &sqlx::postgres::PgRow) -> Result<Head, ApiError> {
    Ok(Head {
        id: r.try_get("id")?,
        path: stored_path(&r.try_get::<String, _>("path")?)?,
        key: r.try_get("path_key")?,
        revision: r.try_get("head_revision")?,
        seq: r.try_get("last_seq")?,
        deleted: r.try_get("deleted")?,
        blob: hash_of(r.try_get("blob_hash")?)?,
        size: r.try_get("size")?,
    })
}

/// An entry with its head revision's content; a literal, as sqlx requires.
macro_rules! head_select {
    () => {
        "SELECT e.id, e.path, e.path_key, e.head_revision, e.last_seq, e.deleted, r.blob_hash, r.size
         FROM entry e JOIN revision r ON r.id = e.head_revision"
    };
}
pub(super) use head_select;

fn refused(i: usize, detail: impl std::fmt::Display) -> ApiError {
    ApiError::with_detail(
        ProblemType::InvalidRequest,
        format!("operations[{i}]: {detail}"),
    )
}

/// A token's operation outside its write folders: the whole commit is
/// refused, whether the path or the entry is the one outside (RFC 0004 T1).
fn outside(state: &SharedState, i: usize) -> ApiError {
    KeyCaller::refuse(
        state,
        ProblemType::InsufficientRole,
        format!("operations[{i}]: outside the token's write folders"),
    )
}

/// The commit being decided, under the vault lock.
struct Commit<'a> {
    vault: Uuid,
    tx: &'a mut Tx,
    next_seq: i64,
    heads: HashMap<Uuid, Head>,
    /// path_key → the live entry holding it, as of the operations so far.
    occupancy: HashMap<String, Option<Uuid>>,
    writes: Vec<Write>,
    conflict_suffix: String,
    merges: &'a HashMap<usize, Prepared>,
    allow_retry: bool,
    head_moved: bool,
    /// Above 0 once deleted entries were purged (§6.9).
    horizon_seq: i64,
    space: &'a super::space::Space,
}

impl Commit<'_> {
    async fn occupant(&mut self, key: &str) -> Result<Option<Uuid>, ApiError> {
        if let Some(o) = self.occupancy.get(key) {
            return Ok(*o);
        }
        let o: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM entry WHERE vault_id = $1 AND namespace = $3 AND NOT deleted AND path_key = $2",
        )
        .bind(self.vault)
        .bind(key)
        .bind(&self.space.namespace)
        .fetch_optional(&mut **self.tx)
        .await?;
        self.occupancy.insert(key.to_owned(), o);
        Ok(o)
    }

    async fn head(&mut self, id: Uuid) -> Result<Head, ApiError> {
        if let Some(h) = self.heads.get(&id) {
            return Ok(h.clone());
        }
        let row = sqlx::query(concat!(
            head_select!(),
            " WHERE e.vault_id = $1 AND e.id = $2"
        ))
        .bind(self.vault)
        .bind(id)
        .fetch_one(&mut **self.tx)
        .await?;
        let h = head_from_row(&row)?;
        self.heads.insert(id, h.clone());
        Ok(h)
    }

    /// `path` if free for `owner`, else the first free `path (2)`, `path (3)`… (§6.5).
    async fn free_path(
        &mut self,
        i: usize,
        path: &VaultPath,
        owner: Uuid,
    ) -> Result<VaultPath, ApiError> {
        // `path`, then `(2)` to `(MAX_SUFFIX)`, the last one included
        // (review 2026-11-15 #18).
        for n in 1..=MAX_SUFFIX {
            let candidate = if n == 1 {
                path.clone()
            } else {
                path.suffixed(&n.to_string()).map_err(|e| refused(i, e))?
            };
            match self.occupant(&path_key(&candidate)).await? {
                None => return Ok(candidate),
                Some(o) if o == owner => return Ok(candidate),
                Some(_) => {}
            }
        }
        // Never refuse the commit for a name (review 3.6 F4): the entry id is unique.
        let id = owner.simple().to_string();
        let unique = path
            .suffixed(id.get(id.len().saturating_sub(12)..).unwrap_or(&id))
            .map_err(|e| refused(i, e))?;
        match self.occupant(&path_key(&unique)).await? {
            Some(o) if o != owner => Err(refused(i, "too many files share this name")),
            _ => Ok(unique),
        }
    }

    /// A live conflict file of `path` already holding `blob`: a push retried
    /// while its file could not be moved locally (an open note) lands there
    /// again instead of creating one more (review 4 H2).
    async fn existing_conflict(
        &mut self,
        path: &VaultPath,
        blob: ChunkHash,
    ) -> Result<Option<Head>, ApiError> {
        // The stem and extension as `conflict_path` cuts them: a long name is
        // shortened by the length of this device's suffix, not of a bare
        // "conflict" (review 2026-11-15 #18).
        let Ok(first) = path.suffixed(&self.conflict_suffix) else {
            return Ok(None);
        };
        let first = first.as_str();
        let (Some(n), Some(close)) = (first.rfind(" (conflict"), first.rfind(')')) else {
            return Ok(None);
        };
        let (Some(stem), Some(ext)) = (first.get(..n), first.get(close.saturating_add(1)..)) else {
            return Ok(None);
        };
        let escape = |s: &str| {
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        };
        let like = format!("{} (conflict %){}", escape(stem), escape(ext));
        let rows = sqlx::query(concat!(
            head_select!(),
            " WHERE e.vault_id = $1 AND e.namespace = $4 AND NOT e.deleted AND r.blob_hash = $2 AND e.path LIKE $3 ESCAPE '\\' ORDER BY e.last_seq LIMIT 16"
        ))
        .bind(self.vault)
        .bind(blob.as_bytes().as_slice())
        .bind(like)
        .bind(&self.space.namespace)
        .fetch_all(&mut **self.tx)
        .await?;
        // `%` also crosses folders: the conflict file sits beside `path`.
        for row in &rows {
            let head = head_from_row(row)?;
            let rest = head.path.as_str().get(n..).unwrap_or_default();
            if !rest.contains('/') {
                return Ok(Some(head));
            }
        }
        Ok(None)
    }

    /// Where the pushed content of a conflict goes: `name (conflict <device> <date>).ext`.
    async fn conflict_path(
        &mut self,
        i: usize,
        path: &VaultPath,
        owner: Uuid,
    ) -> Result<VaultPath, ApiError> {
        let first = path
            .suffixed(&self.conflict_suffix)
            .map_err(|e| refused(i, e))?;
        self.free_path(i, &first, owner).await
    }

    fn write(
        &mut self,
        id: Uuid,
        path: VaultPath,
        deleted: bool,
        blob: Option<ChunkHash>,
        size: i64,
        base: Option<Uuid>,
    ) -> Head {
        let key = path_key(&path);
        if let Some(old) = self.heads.get(&id).filter(|h| !h.deleted) {
            self.occupancy.insert(old.key.clone(), None);
        }
        if !deleted {
            self.occupancy.insert(key.clone(), Some(id));
        }
        let head = Head {
            id,
            path,
            key,
            revision: Uuid::new_v4(),
            seq: self.next_seq,
            deleted,
            blob,
            size,
        };
        self.next_seq = self.next_seq.saturating_add(1);
        self.heads.insert(id, head.clone());
        self.writes.push(Write {
            head: head.clone(),
            base,
        });
        head
    }
}

/// Push operations; each comes back accepted or conflicted (merged from phase 4).
#[utoipa::path(
    post,
    path = "/vaults/{v}/commits",
    tag = "sync",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = CommitRequest,
    responses(
        (status = 200, description = "One result per operation, in order", body = CommitResponse),
        (status = 403, description = "Below editor", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
        (status = 413, description = "Over 1 MiB", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "A forbidden path, a blob not uploaded, or an idempotency key reused with another body", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn post_commit(
    State(state): State<SharedState>,
    caller: Writer,
    Path(v): Path<Uuid>,
    Raw(body): Raw,
) -> Result<Json<CommitResponse>, ApiError> {
    // T3: the role is checked on the vault before the body is even parsed.
    let a = caller.access(&state, v).await?;
    let req: CommitRequest = serde_json::from_slice(&body).map_err(|e| {
        let msg = e.to_string();
        let kind = if msg.starts_with(kura_core::path::INVALID_PATH) {
            ProblemType::InvalidPath
        } else {
            ProblemType::InvalidRequest
        };
        ApiError::with_detail(kind, msg)
    })?;
    validate(&req)?;
    let space = super::space::resolve(
        &state,
        req.space.as_deref(),
        &a,
        // A token has no device, so never the configuration space (§6.8).
        match &caller {
            Writer::Device(d) => Some((d.caller.principal.sub.as_str(), d.device)),
            Writer::Key(_) => None,
        },
    )
    .await?;
    for (i, op) in req.operations.iter().enumerate() {
        if let Operation::Put { path, .. } = op
            && !space.allows(path.as_str())
        {
            return Err(ApiError::with_detail(
                ProblemType::InvalidPath,
                format!("operations[{i}]: {path} is not in a synced configuration category"),
            ));
        }
    }
    // A token: the paths it names and the content it names, before any merge
    // work. The heads are checked again under the vault lock (RFC 0004 §6.5).
    if let Some(k) = caller.key() {
        let mut blobs = Vec::new();
        for (i, op) in req.operations.iter().enumerate() {
            if let Operation::Put { path, blob, .. } = op {
                if !k.scope.writes(path.as_str()) {
                    return Err(outside(&state, i));
                }
                blobs.push(*blob);
            }
        }
        let known = k.knows(&state, &a, &blobs, false).await?;
        if !blobs.iter().all(|b| known.contains(b)) {
            return Err(KeyCaller::refuse(
                &state,
                ProblemType::HashMismatch,
                NOT_UPLOADED,
            ));
        }
    }
    // The idempotency hash covers the parsed request, not its whitespace.
    let canonical = serde_json::to_vec(&req).map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let request_hash = ChunkHash::of(&canonical);
    let sub = &caller.subject();
    let device = caller.device_id();

    // A retry finds its stored result before any merge is redone.
    if let Some(stored) =
        stored_result(&state, &a, v, device, &req.idempotency_key, request_hash).await?
    {
        return Ok(Json(stored));
    }
    let cx = Cx {
        state: &state,
        a: &a,
        v,
        device,
        key: caller.key(),
        sub,
        req: &req,
        request_hash,
        space: &space,
    };
    let _timer = state.metrics.commit_duration_seconds.start_timer();
    let deadline = tokio::time::Instant::now()
        .checked_add(MERGE_BUDGET)
        .unwrap_or_else(tokio::time::Instant::now);
    let mut attempt = 1;
    loop {
        // Configuration: the last write wins, nothing is merged (§6.5).
        let merges = if space.is_config() {
            HashMap::new()
        } else {
            prepare(&cx, deadline).await
        };
        let retry = attempt < MERGE_ATTEMPTS;
        match decide(&cx, &merges, retry).await? {
            Decided::Done(r) => return Ok(Json(r)),
            Decided::HeadMoved => attempt = attempt.saturating_add(1),
        }
    }
}

/// What every step of one commit shares.
struct Cx<'a> {
    state: &'a SharedState,
    a: &'a VaultAccess,
    v: Uuid,
    /// The device, or the token's id in its place.
    device: Uuid,
    /// The scoped token writing, when it is not a device.
    key: Option<&'a KeyCaller>,
    sub: &'a str,
    req: &'a CommitRequest,
    request_hash: ChunkHash,
    space: &'a super::space::Space,
}

enum Decided {
    Done(CommitResponse),
    /// A prepared merge's head moved before the lock: prepare again.
    HeadMoved,
}

/// The stored result of this key, if any; another body under it is refused.
async fn stored_result(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    device: Uuid,
    key: &str,
    request_hash: ChunkHash,
) -> Result<Option<CommitResponse>, ApiError> {
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let stored = sqlx::query("SELECT request_hash, result FROM commit_record WHERE vault_id = $1 AND device_id = $2 AND idempotency_key = $3")
        .bind(v)
        .bind(device)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    let Some(row) = stored else { return Ok(None) };
    let hash: Vec<u8> = row.try_get("request_hash")?;
    if hash.as_slice() != request_hash.as_bytes() {
        return Err(ApiError::of(ProblemType::IdempotencyConflict));
    }
    let result: sqlx::types::Json<CommitResponse> = row.try_get("result")?;
    Ok(Some(result.0))
}

/// Under the vault lock: the rules of §6.5, prepared merges used only if
/// their head is still the head.
async fn decide(
    cx: &Cx<'_>,
    merges: &HashMap<usize, Prepared>,
    allow_retry: bool,
) -> Result<Decided, ApiError> {
    let (state, a, v, req, sub) = (cx.state, cx.a, cx.v, cx.req, cx.sub);
    let request_hash = cx.request_hash;
    let mut tx = db::begin_tenant(&state.db, a.tenant, Some(sub)).await?;
    let vault =
        sqlx::query("SELECT head_seq, epoch, horizon_seq FROM vault WHERE id = $1 FOR UPDATE")
            .bind(v)
            .fetch_one(&mut *tx)
            .await?;
    let head_seq: i64 = vault.try_get("head_seq")?;
    let horizon_seq: i64 = vault.try_get("horizon_seq")?;
    // Under the lock, so two retries racing see one stored result (§6.3).
    let stored = sqlx::query("SELECT request_hash, result FROM commit_record WHERE vault_id = $1 AND device_id = $2 AND idempotency_key = $3")
        .bind(v)
        .bind(cx.device)
        .bind(&req.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(row) = stored {
        let hash: Vec<u8> = row.try_get("request_hash")?;
        if hash.as_slice() != request_hash.as_bytes() {
            return Err(ApiError::of(ProblemType::IdempotencyConflict));
        }
        let result: sqlx::types::Json<CommitResponse> = row.try_get("result")?;
        tx.commit().await?;
        return Ok(Decided::Done(result.0));
    }
    // The device's state predates history this server lost (§6.3).
    if req.epoch.is_some_and(|e| {
        vault
            .try_get::<Uuid, _>("epoch")
            .is_ok_and(|ours| ours != e)
    }) {
        return Err(ApiError::of(ProblemType::HistoryRewound));
    }

    // What suffixes a conflict file: the device's name, or the token's.
    let device_name: String = match cx.key {
        Some(k) => k.name.clone(),
        None => {
            sqlx::query_scalar("SELECT name FROM device WHERE id = $1")
                .bind(cx.device)
                .fetch_one(&mut *tx)
                .await?
        }
    };
    let today = time::OffsetDateTime::now_utc().date();
    let ids: Vec<Uuid> = req
        .operations
        .iter()
        .map(|o| *o.entry().as_uuid())
        .collect();
    // An id that lives in another namespace, or another user's
    // configuration, is never written over (phase 8 review).
    let homes = sqlx::query_as::<_, (Uuid, String, Option<String>)>(
        "SELECT id, namespace, owner_sub FROM kura_entry_home($1, $2)",
    )
    .bind(v)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    if let Some((id, _, _)) = homes
        .iter()
        .find(|(_, ns, owner)| *ns != cx.space.namespace || *owner != cx.space.owner)
    {
        let i = ids.iter().position(|x| x == id).unwrap_or_default();
        return Err(refused(i, "this entry id belongs to another space"));
    }
    let heads = sqlx::query(concat!(
        head_select!(),
        " WHERE e.vault_id = $1 AND e.id = ANY($2) AND e.namespace = $3"
    ))
    .bind(v)
    .bind(&ids)
    .bind(&cx.space.namespace)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|r| head_from_row(r).map(|h| (h.id, h)))
    .collect::<Result<HashMap<_, _>, _>>()?;
    let bases = load_bases(&mut tx, v, req).await?;
    // A token writes an entry only where it lies now, under this lock: an id
    // outside its write folders is not moved in, overwritten or deleted, and
    // the base it merges from is one it can read (RFC 0004 §6.5, T1).
    if let Some(k) = cx.key {
        for (i, op) in req.operations.iter().enumerate() {
            let head_ok = heads
                .get(op.entry().as_uuid())
                .is_none_or(|h| k.scope.writes(h.path.as_str()));
            let base = match op {
                Operation::Put { base, .. } => *base,
                Operation::Delete { base, .. } => Some(*base),
            };
            let base_ok = base
                .and_then(|b| bases.get(&b))
                .is_none_or(|b| k.scope.sees(&b.path));
            if !head_ok || !base_ok {
                return Err(outside(state, i));
            }
        }
    }
    check_blobs(&mut tx, v, req).await?;
    let keys: Vec<String> = req
        .operations
        .iter()
        .filter_map(|o| match o {
            Operation::Put { path, .. } => Some(path_key(path)),
            Operation::Delete { .. } => None,
        })
        .collect();
    let occupancy: HashMap<String, Option<Uuid>> = sqlx::query(
        "SELECT path_key, id FROM entry WHERE vault_id = $1 AND namespace = $3 AND NOT deleted AND path_key = ANY($2)",
    )
    .bind(v)
    .bind(&keys)
    .bind(&cx.space.namespace)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|r| Ok((r.try_get::<String, _>("path_key")?, Some(r.try_get::<Uuid, _>("id")?))))
    .collect::<Result<_, sqlx::Error>>()?;
    let missing_keys: HashSet<&String> = keys
        .iter()
        .filter(|k| !occupancy.contains_key(*k))
        .collect();
    let mut occupancy = occupancy;
    for k in missing_keys {
        occupancy.insert(k.clone(), None);
    }

    let mut c = Commit {
        vault: v,
        tx: &mut tx,
        next_seq: head_seq.saturating_add(1),
        heads,
        occupancy,
        writes: Vec::new(),
        conflict_suffix: format!("conflict {device_name} {today}"),
        merges,
        allow_retry,
        head_moved: false,
        horizon_seq,
        space: cx.space,
    };
    let mut results = Vec::with_capacity(req.operations.len());
    for (i, op) in req.operations.iter().enumerate() {
        results.push(apply(&mut c, i, op, &bases).await?);
    }
    if c.head_moved {
        // Rolled back on drop, releasing the lock before merging again.
        return Ok(Decided::HeadMoved);
    }
    let new_head = c.next_seq.saturating_sub(1);
    let writes = std::mem::take(&mut c.writes);
    drop(c);
    store(&mut tx, v, a.tenant, cx.device, sub, cx.space, &writes).await?;
    if !cx.space.is_config() {
        enqueue_index(state, &mut tx, v, a, &writes).await?;
    }
    let response = CommitResponse {
        head_seq: u64::try_from(new_head).unwrap_or_default(),
        results,
    };
    sqlx::query("UPDATE vault SET head_seq = $2 WHERE id = $1")
        .bind(v)
        .bind(new_head)
        .execute(&mut *tx)
        .await?;
    if !writes.is_empty() {
        // Delivered on commit only, to every replica's sockets (§6.3).
        crate::notify::notify_change(&mut tx, v, new_head).await?;
        // RFC 0003: `vault.changed` at most once a vault a minute, with the
        // number of commits; no path, ever (decision 7).
        if !cx.space.is_config() {
            let minute = time::OffsetDateTime::now_utc()
                .unix_timestamp()
                .div_euclid(60);
            crate::notifier::record(
                state,
                &mut tx,
                crate::notifier::Fact {
                    dedupe: Some(format!("vault.changed:{v}:{minute}")),
                    ..crate::notifier::Fact::vault(
                        a,
                        "vault.changed",
                        format!("Vault {} changed", a.name),
                        format!("Tenant {}: notes were changed in the vault.", a.tenant_slug),
                        serde_json::json!({ "headSeq": new_head }),
                    )
                },
            )
            .await?;
        }
        let conflicts = response
            .results
            .iter()
            .filter(|r| r.conflict.is_some())
            .count();
        if conflicts > 0 {
            crate::notifier::record(
                state,
                &mut tx,
                crate::notifier::Fact::vault(
                    a,
                    "vault.conflict",
                    format!("A conflict in {}", a.name),
                    format!("Tenant {}: {conflicts} conflict files were created; both versions are kept.", a.tenant_slug),
                    serde_json::json!({ "conflicts": conflicts }),
                ),
            )
            .await?;
        }
    }
    sqlx::query(
        "INSERT INTO commit_record (vault_id, tenant_id, device_id, idempotency_key, request_hash, result)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(v)
    .bind(a.tenant)
    .bind(cx.device)
    .bind(&req.idempotency_key)
    .bind(request_hash.as_bytes().as_slice())
    .bind(sqlx::types::Json(&response))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if !writes.is_empty() {
        state.metrics.commits_total.inc();
        if cx.key.is_some() {
            state
                .metrics
                .token_writes_total
                .with_label_values(&["applied"])
                .inc();
        }
    }
    Ok(Decided::Done(response))
}

/// With search on for the tenant, each written entry gets an index job in
/// the commit's own transaction (§6.7); a newer one replaces a pending one.
async fn enqueue_index(
    state: &SharedState,
    tx: &mut Tx,
    v: Uuid,
    a: &VaultAccess,
    writes: &[Write],
) -> Result<(), ApiError> {
    let on = state.snapshot().is_some_and(|snap| {
        snap.config
            .tenants
            .get(a.tenant_slug.as_str())
            .is_some_and(|t| t.doc.spec.search.enabled)
    });
    if !on {
        return Ok(());
    }
    // Every write: a rename away from `.md` must drop the old words too
    // (review 7 M3); the worker indexes Markdown heads only.
    let (entries, revisions): (Vec<Uuid>, Vec<Uuid>) =
        writes.iter().map(|w| (w.head.id, w.head.revision)).unzip();
    if entries.is_empty() {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO index_job (vault_id, tenant_id, entry_id, revision_id)
         SELECT $1, $2, e, r FROM unnest($3::uuid[], $4::uuid[]) AS u(e, r)
         ON CONFLICT (vault_id, entry_id) DO UPDATE
             SET revision_id = EXCLUDED.revision_id, attempts = 0, claimed_at = NULL, failed_at = NULL, created_at = now()",
    )
    .bind(v)
    .bind(a.tenant)
    .bind(&entries)
    .bind(&revisions)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn validate(req: &CommitRequest) -> Result<(), ApiError> {
    let key = &req.idempotency_key;
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "idempotencyKey: 1 to 128 visible ASCII characters",
        ));
    }
    if req.operations.is_empty() || req.operations.len() > MAX_OPERATIONS {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "1 to 1,000 operations",
        ));
    }
    let mut seen = HashSet::new();
    for (i, op) in req.operations.iter().enumerate() {
        if !seen.insert(op.entry()) {
            return Err(refused(i, "an entry appears twice in one commit"));
        }
    }
    Ok(())
}

async fn load_bases(
    tx: &mut Tx,
    v: Uuid,
    req: &CommitRequest,
) -> Result<HashMap<Uuid, BaseRev>, ApiError> {
    let ids: Vec<Uuid> = req
        .operations
        .iter()
        .filter_map(|o| match o {
            Operation::Put { base, .. } => *base,
            Operation::Delete { base, .. } => Some(*base),
        })
        .collect();
    sqlx::query(
        "SELECT id, entry_id, path, blob_hash FROM revision WHERE vault_id = $1 AND id = ANY($2)",
    )
    .bind(v)
    .bind(&ids)
    .fetch_all(&mut **tx)
    .await?
    .iter()
    .map(|r| {
        Ok((
            r.try_get("id")?,
            BaseRev {
                entry: r.try_get("entry_id")?,
                path: r.try_get("path")?,
                blob: hash_of(r.try_get("blob_hash")?)?,
            },
        ))
    })
    .collect()
}

/// Every pushed blob must be stored, with the size the operation claims.
async fn check_blobs(tx: &mut Tx, v: Uuid, req: &CommitRequest) -> Result<(), ApiError> {
    let wanted: Vec<(usize, ChunkHash, u64)> = req
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, o)| match o {
            Operation::Put { blob, size, .. } => Some((i, *blob, *size)),
            Operation::Delete { .. } => None,
        })
        .collect();
    let raw: Vec<Vec<u8>> = wanted
        .iter()
        .map(|(_, h, _)| h.as_bytes().to_vec())
        .collect();
    let sizes: HashMap<Vec<u8>, i64> =
        // FOR KEY SHARE: garbage collection cannot take a blob this commit uses.
        sqlx::query("SELECT hash, size FROM blob WHERE vault_id = $1 AND hash = ANY($2) FOR KEY SHARE")
            .bind(v)
            .bind(&raw)
            .fetch_all(&mut **tx)
            .await?
            .iter()
            .map(|r| Ok((r.try_get("hash")?, r.try_get("size")?)))
            .collect::<Result<_, sqlx::Error>>()?;
    for (i, h, size) in wanted {
        match sizes.get(h.as_bytes().as_slice()) {
            None => {
                return Err(ApiError::with_detail(
                    ProblemType::HashMismatch,
                    format!("operations[{i}]: blob {h} is not stored; upload it first"),
                ));
            }
            Some(s) if u64::try_from(*s).ok() != Some(size) => {
                return Err(ApiError::with_detail(
                    ProblemType::HashMismatch,
                    format!("operations[{i}]: size differs from the stored blob"),
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// One operation, by the rows of §6.5 (without merge).
async fn apply(
    c: &mut Commit<'_>,
    i: usize,
    op: &Operation,
    bases: &HashMap<Uuid, BaseRev>,
) -> Result<OpResult, ApiError> {
    let id = *op.entry().as_uuid();
    let existing = c.heads.get(&id).cloned();
    if c.space.is_config() {
        return apply_config(c, i, op, existing).await;
    }
    // T17: a base must be a revision of this entry in this vault.
    let base = match op {
        Operation::Put { base, .. } => *base,
        Operation::Delete { base, .. } => Some(*base),
    };
    let base_rev = match base {
        None => None,
        Some(b) => match (bases.get(&b), &existing) {
            (Some(r), Some(_)) if r.entry == id => Some(r),
            // Pruned by retention: handled as overlapping edits (§6.5).
            (None, Some(_)) => None,
            // Its entry was purged while this device was away: it resyncs
            // first, instead of being refused on every push (§6.3).
            (None, None) if c.horizon_seq > 0 => {
                return Err(ApiError::of(ProblemType::CursorExpired));
            }
            _ => {
                return Err(refused(
                    i,
                    "base is not a revision of this entry in this vault",
                ));
            }
        },
    };
    let accepted = |head: Head| {
        Ok(OpResult {
            outcome: Outcome::Accepted,
            head: head.state()?,
            conflict: None,
        })
    };
    match op {
        Operation::Delete { .. } => {
            let Some(h) = existing else {
                return Err(refused(i, "no such entry"));
            };
            if h.deleted {
                return accepted(h);
            }
            if base == Some(h.revision) {
                let head = c.write(id, h.path.clone(), true, None, 0, base);
                return accepted(head);
            }
            // Edited since the device last saw it: the delete is dropped.
            Ok(OpResult {
                outcome: Outcome::Conflicted,
                head: h.state()?,
                conflict: None,
            })
        }
        Operation::Put {
            path, blob, size, ..
        } => {
            let size = i64::try_from(*size).map_err(|_| refused(i, "size too large"))?;
            let Some(h) = existing else {
                return put_new(c, i, id, path, *blob, size).await;
            };
            if h.deleted {
                // An edit against a deleted entry wins and restores it.
                let p = c.free_path(i, path, id).await?;
                return accepted(c.write(id, p, false, Some(*blob), size, base));
            }
            if h.blob == Some(*blob) {
                if h.path == *path {
                    // Already the head, whatever the base: a late retry is a no-op.
                    return accepted(h);
                }
                // A rename; of two renames, the later commit wins.
                let p = c.free_path(i, path, id).await?;
                return accepted(c.write(id, p, false, Some(*blob), size, base));
            }
            // Nothing changed since its base, which is not the head: a device
            // resending after a rewind (§6.3). The head stays.
            if base_rev.is_some_and(|r| r.blob == Some(*blob) && r.path == path.as_str()) {
                return accepted(h);
            }
            let content_unchanged_since_base = base_rev.is_some_and(|r| r.blob == h.blob);
            if base == Some(h.revision) || content_unchanged_since_base {
                // Fast-forward, or a rename on the server and an edit here: both apply.
                let target = match base_rev {
                    Some(r) if base != Some(h.revision) && r.path == path.as_str() => {
                        h.path.clone()
                    }
                    _ => path.clone(),
                };
                let p = c.free_path(i, &target, id).await?;
                return accepted(c.write(id, p, false, Some(*blob), size, base));
            }
            // Both sides changed the content: a merge prepared against this
            // very head becomes the head (§6.5, phase 4).
            if let Some(m) = c.merges.get(&i) {
                if m.head == h.revision {
                    let target = match base_rev {
                        Some(r) if r.path != path.as_str() => path.clone(),
                        _ => h.path.clone(),
                    };
                    if Some(m.blob) == h.blob && target == h.path {
                        return Ok(OpResult {
                            outcome: Outcome::Merged,
                            head: h.state()?,
                            conflict: None,
                        });
                    }
                    let p = c.free_path(i, &target, id).await?;
                    let head = c.write(id, p, false, Some(m.blob), m.size, Some(h.revision));
                    return Ok(OpResult {
                        outcome: Outcome::Merged,
                        head: head.state()?,
                        conflict: None,
                    });
                }
                if c.allow_retry {
                    c.head_moved = true;
                }
            }
            // Otherwise the head stays and the push becomes a conflict file,
            // the same one if this content was pushed before (review 4 H2).
            if let Some(existing) = c.existing_conflict(path, *blob).await? {
                return Ok(OpResult {
                    outcome: Outcome::Conflicted,
                    head: h.state()?,
                    conflict: Some(existing.state()?),
                });
            }
            let conflict_id = Uuid::now_v7();
            let p = c.conflict_path(i, path, conflict_id).await?;
            let conflict = c.write(conflict_id, p, false, Some(*blob), size, None);
            Ok(OpResult {
                outcome: Outcome::Conflicted,
                head: h.state()?,
                conflict: Some(conflict.state()?),
            })
        }
    }
}

/// Configuration (§6.5): Obsidian would load a conflict file as a setting,
/// so the last write wins, with no merge and no suffix; the overwritten
/// version stays in history. The space is one user's own, so the only writes
/// it supersedes are theirs.
async fn apply_config(
    c: &mut Commit<'_>,
    i: usize,
    op: &Operation,
    existing: Option<Head>,
) -> Result<OpResult, ApiError> {
    let id = *op.entry().as_uuid();
    let accepted = |head: Head| {
        Ok(OpResult {
            outcome: Outcome::Accepted,
            head: head.state()?,
            conflict: None,
        })
    };
    match op {
        Operation::Delete { .. } => match existing {
            Some(h) if h.deleted => accepted(h),
            Some(h) => {
                let head = c.write(id, h.path.clone(), true, None, 0, Some(h.revision));
                accepted(head)
            }
            None if c.horizon_seq > 0 => Err(ApiError::of(ProblemType::CursorExpired)),
            None => Err(refused(i, "no such entry")),
        },
        Operation::Put {
            path, blob, size, ..
        } => {
            let size = i64::try_from(*size).map_err(|_| refused(i, "size too large"))?;
            if let Some(h) = existing
                .as_ref()
                .filter(|h| !h.deleted && h.blob == Some(*blob) && h.path == *path)
            {
                return accepted(h.clone());
            }
            let own = existing.as_ref().map(|h| h.revision);
            let (target, base) = match c.occupant(&path_key(path)).await? {
                // A new id on a held path writes onto the entry holding it.
                Some(o) if o != id && existing.is_none() => (o, Some(c.head(o).await?.revision)),
                // A rename onto a held path: the holder is deleted.
                Some(o) if o != id => {
                    let h = c.head(o).await?;
                    c.write(o, h.path.clone(), true, None, 0, Some(h.revision));
                    (id, own)
                }
                _ => (id, own),
            };
            accepted(c.write(target, path.clone(), false, Some(*blob), size, base))
        }
    }
}

/// A new entry; on an occupied path, identical content is unified, other content
/// becomes a conflict file under the pushed id (§6.5).
async fn put_new(
    c: &mut Commit<'_>,
    i: usize,
    id: Uuid,
    path: &VaultPath,
    blob: ChunkHash,
    size: i64,
) -> Result<OpResult, ApiError> {
    match c.occupant(&path_key(path)).await? {
        None => {
            let head = c.write(id, path.clone(), false, Some(blob), size, None);
            Ok(OpResult {
                outcome: Outcome::Accepted,
                head: head.state()?,
                conflict: None,
            })
        }
        Some(o) => {
            let other = c.head(o).await?;
            if other.blob == Some(blob) {
                return Ok(OpResult {
                    outcome: Outcome::Accepted,
                    head: other.state()?,
                    conflict: None,
                });
            }
            if let Some(existing) = c.existing_conflict(path, blob).await? {
                return Ok(OpResult {
                    outcome: Outcome::Conflicted,
                    head: other.state()?,
                    conflict: Some(existing.state()?),
                });
            }
            let p = c.conflict_path(i, path, id).await?;
            let conflict = c.write(id, p, false, Some(blob), size, None);
            Ok(OpResult {
                outcome: Outcome::Conflicted,
                head: other.state()?,
                conflict: Some(conflict.state()?),
            })
        }
    }
}

/// A merge computed before the lock, against `head`.
struct Prepared {
    head: Uuid,
    blob: ChunkHash,
    size: i64,
}

/// Without the lock: for each `put` whose text diverged from a head that also
/// changed since its base, reads base, head and pushed content, merges them
/// within the commit's budget and stores a clean result. Any failure (no
/// storage, a blob too large, binary content, the budget spent, a quota
/// refusal) leaves that operation to become a conflict file.
async fn prepare(cx: &Cx<'_>, deadline: tokio::time::Instant) -> HashMap<usize, Prepared> {
    match prepare_inner(cx, deadline).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "merges not prepared; diverging edits become conflict files");
            HashMap::new()
        }
    }
}

async fn prepare_inner(
    cx: &Cx<'_>,
    deadline: tokio::time::Instant,
) -> Result<HashMap<usize, Prepared>, ApiError> {
    use kura_core::merge::{MAX_MERGE, Merge, merge3, mergeable};
    let mut out = HashMap::new();
    let max_file = cx.state.snapshot().and_then(|snap| {
        snap.config
            .tenants
            .get(cx.a.tenant_slug.as_str())
            .and_then(|t| t.doc.spec.quota.max_file_size)
            .map(|q| q.bytes())
    });
    let candidates: Vec<(usize, Uuid, Uuid, ChunkHash)> = cx
        .req
        .operations
        .iter()
        .enumerate()
        .filter_map(|(i, o)| match o {
            Operation::Put {
                entry,
                path,
                base: Some(b),
                blob,
                ..
            } if mergeable(path) => Some((i, *entry.as_uuid(), *b, *blob)),
            _ => None,
        })
        .collect();
    if candidates.is_empty() || cx.state.storage.is_none() {
        return Ok(out);
    }
    let ids: Vec<Uuid> = candidates.iter().map(|c| c.1).collect();
    let mut tx = db::begin_tenant(&cx.state.db, cx.a.tenant, None).await?;
    let heads = sqlx::query(concat!(
        head_select!(),
        " WHERE e.vault_id = $1 AND e.id = ANY($2)"
    ))
    .bind(cx.v)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|r| head_from_row(r).map(|h| (h.id, h)))
    .collect::<Result<HashMap<_, _>, _>>()?;
    let bases = load_bases(&mut tx, cx.v, cx.req).await?;
    // The sizes of only the blobs involved, before reading any content.
    let involved: Vec<Vec<u8>> = candidates
        .iter()
        .flat_map(|(_, entry, base, pushed)| {
            let head = heads.get(entry).and_then(|h| h.blob);
            let base = bases.get(base).and_then(|b| b.blob);
            [head, base, Some(*pushed)]
        })
        .flatten()
        .map(|h| h.as_bytes().to_vec())
        .collect();
    let sizes: HashMap<Vec<u8>, i64> =
        // FOR KEY SHARE: garbage collection cannot take a blob this commit uses.
        sqlx::query("SELECT hash, size FROM blob WHERE vault_id = $1 AND hash = ANY($2) FOR KEY SHARE")
            .bind(cx.v)
            .bind(&involved)
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(|r| Ok((r.try_get("hash")?, r.try_get("size")?)))
            .collect::<Result<_, sqlx::Error>>()?;
    tx.commit().await?;
    let small = |h: &ChunkHash| {
        sizes
            .get(h.as_bytes().as_slice())
            .is_some_and(|s| usize::try_from(*s).is_ok_and(|s| s <= MAX_MERGE))
    };
    for (i, entry, base, pushed) in candidates {
        let (Some(h), Some(br)) = (heads.get(&entry), bases.get(&base)) else {
            continue;
        };
        let (Some(head_blob), Some(base_blob)) = (h.blob, br.blob) else {
            continue;
        };
        let diverged = br.entry == entry
            && !h.deleted
            && base != h.revision
            && head_blob != pushed
            && base_blob != pushed
            && base_blob != head_blob;
        if !diverged || ![head_blob, base_blob, pushed].iter().all(small) {
            continue;
        }
        let Some(left) = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .filter(|d| !d.is_zero())
        else {
            break;
        };
        let Ok(Ok(permit)) = tokio::time::timeout(left, MERGES.clone().acquire_owned()).await
        else {
            break;
        };
        let (b, o, t) = (
            read_blob(cx.state, cx.a, cx.v, base_blob).await?,
            read_blob(cx.state, cx.a, cx.v, head_blob).await?,
            read_blob(cx.state, cx.a, cx.v, pushed).await?,
        );
        let merged = tokio::time::timeout(
            left,
            tokio::task::spawn_blocking(move || {
                let merged = merge3(&b, &o, &t);
                drop(permit);
                merged
            }),
        )
        .await;
        let outcome = match merged {
            // Larger than a side may be, or than the tenant allows: a conflict file (review 4 L2).
            Ok(Ok(Merge::Clean(data)))
                if data.len() <= kura_core::merge::MAX_MERGE
                    && max_file.is_none_or(|m| data.len() as u64 <= m) =>
            {
                let size = i64::try_from(data.len()).unwrap_or(i64::MAX);
                match store_content(cx.state, cx.a, cx.v, data).await {
                    Ok(blob) => {
                        out.insert(
                            i,
                            Prepared {
                                head: h.revision,
                                blob,
                                size,
                            },
                        );
                        "merged"
                    }
                    Err(_) => "conflict",
                }
            }
            _ => "conflict",
        };
        cx.state
            .metrics
            .merge_total
            .with_label_values(&[outcome])
            .inc();
    }
    Ok(out)
}

/// Set-based writes, in seq order: an entry that frees a path is written
/// before the one that takes it, so the unique path index holds row by row.
async fn store(
    tx: &mut Tx,
    v: Uuid,
    tenant: Uuid,
    device: Uuid,
    sub: &str,
    space: &super::space::Space,
    writes: &[Write],
) -> Result<(), ApiError> {
    if writes.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = writes.iter().map(|w| w.head.id).collect();
    let paths: Vec<&str> = writes.iter().map(|w| w.head.path.as_str()).collect();
    let keys: Vec<&str> = writes.iter().map(|w| w.head.key.as_str()).collect();
    let revs: Vec<RevisionId> = writes.iter().map(|w| w.head.revision).collect();
    let seqs: Vec<i64> = writes.iter().map(|w| w.head.seq).collect();
    let deleted: Vec<bool> = writes.iter().map(|w| w.head.deleted).collect();
    let bases: Vec<Option<Uuid>> = writes.iter().map(|w| w.base).collect();
    let blobs: Vec<Option<Vec<u8>>> = writes
        .iter()
        .map(|w| w.head.blob.map(|b| b.as_bytes().to_vec()))
        .collect();
    let sizes: Vec<i64> = writes.iter().map(|w| w.head.size).collect();
    sqlx::query(
        "INSERT INTO entry (vault_id, tenant_id, id, path, path_key, head_revision, last_seq, deleted, namespace, owner_sub)
         SELECT $1, $2, u.id, u.path, u.key, u.rev, u.seq, u.deleted, $9, $10
         FROM unnest($3::uuid[], $4::text[], $5::text[], $6::uuid[], $7::bigint[], $8::bool[])
              WITH ORDINALITY AS u(id, path, key, rev, seq, deleted, ord)
         ORDER BY u.ord
         ON CONFLICT (vault_id, id) DO UPDATE SET path = EXCLUDED.path, path_key = EXCLUDED.path_key,
             head_revision = EXCLUDED.head_revision, last_seq = EXCLUDED.last_seq, deleted = EXCLUDED.deleted",
    )
    .bind(v)
    .bind(tenant)
    .bind(&ids)
    .bind(&paths)
    .bind(&keys)
    .bind(&revs)
    .bind(&seqs)
    .bind(&deleted)
    .bind(&space.namespace)
    .bind(space.owner.as_deref())
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO revision (id, vault_id, tenant_id, entry_id, seq, base_revision, path, deleted, blob_hash, size, device_id, user_sub)
         SELECT u.rev, $1, $2, u.id, u.seq, u.base, u.path, u.deleted, u.blob, u.size, $3, $4
         FROM unnest($5::uuid[], $6::uuid[], $7::bigint[], $8::uuid[], $9::text[], $10::bool[], $11::bytea[], $12::bigint[])
              AS u(rev, id, seq, base, path, deleted, blob, size)",
    )
    .bind(v)
    .bind(tenant)
    .bind(device)
    .bind(sub)
    .bind(&revs)
    .bind(&ids)
    .bind(&seqs)
    .bind(&bases)
    .bind(&paths)
    .bind(&deleted)
    .bind(&blobs)
    .bind(&sizes)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// An entry's history, newest first (§4.4, §6.9).
#[utoipa::path(
    get,
    path = "/vaults/{v}/entries/{e}/revisions",
    tag = "history",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("e" = Uuid, Path, description = "Entry id"),
        RevisionsQuery,
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    responses(
        (status = 200, description = "At most 1,000 revisions; follow `next` for older ones", body = RevisionsResponse),
        (status = 404, description = "No such vault or entry", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_revisions(
    State(state): State<SharedState>,
    reader: super::reader::Reader,
    Path((v, e)): Path<(Uuid, Uuid)>,
    q: Result<Query<RevisionsQuery>, QueryRejection>,
) -> Result<Json<RevisionsResponse>, ApiError> {
    let q = q
        .map(|Query(q)| q)
        .map_err(|e| ApiError::with_detail(ProblemType::InvalidRequest, e.body_text()))?;
    let ra = reader.access(&state, v, VaultRole::Viewer).await?;
    let before = q
        .before
        .map_or(i64::MAX, |b| i64::try_from(b).unwrap_or(i64::MAX));
    let mut tx = db::begin_tenant_snapshot(&state.db, ra.access.tenant, &reader.subject()).await?;
    // T16: an entry renamed out of a token's prefix disappears with its history.
    let head: Option<String> =
        sqlx::query_scalar("SELECT path FROM entry WHERE vault_id = $1 AND id = $2")
            .bind(v)
            .bind(e)
            .fetch_optional(&mut *tx)
            .await?;
    if !head.is_some_and(|p| ra.sees(&p)) {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    let rows = sqlx::query(
        "SELECT id, seq, path, deleted, blob_hash, size, created_at FROM revision
         WHERE vault_id = $1 AND entry_id = $2 AND seq < $3 ORDER BY seq DESC LIMIT $4",
    )
    .bind(v)
    .bind(e)
    .bind(before)
    .bind(REVISIONS_PAGE.saturating_add(1))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    // The page ends at its last row read, visible or not.
    let page = usize::try_from(REVISIONS_PAGE).unwrap_or(usize::MAX);
    let more = rows.len() > page;
    let last_seq = rows
        .iter()
        .take(page)
        .last()
        .and_then(|r| r.try_get::<i64, _>("seq").ok())
        .and_then(|s| u64::try_from(s).ok());
    let items = rows
        .iter()
        .take(page)
        .filter(|r| r.try_get::<String, _>("path").is_ok_and(|p| ra.sees(&p)))
        .map(|r| {
            let created: time::OffsetDateTime = r.try_get("created_at")?;
            Ok(RevisionInfo {
                revision: r.try_get("id")?,
                seq: u64::try_from(r.try_get::<i64, _>("seq")?).unwrap_or_default(),
                path: stored_path(&r.try_get::<String, _>("path")?)?,
                deleted: r.try_get("deleted")?,
                blob: hash_of(r.try_get("blob_hash")?)?,
                size: u64::try_from(r.try_get::<i64, _>("size")?).unwrap_or_default(),
                created_at: created
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(|e| ApiError::Unavailable(e.to_string()))?,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(RevisionsResponse {
        next: if more { last_seq } else { None },
        items,
    }))
}

/// Revisions per page (§4.4).
const REVISIONS_PAGE: i64 = 1000;

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct RevisionsQuery {
    /// The last `seq` of the previous page.
    pub before: Option<u64>,
}

/// Restores a revision: a new revision with its content, through the commit
/// path, so the vault lock, `seq`, notifications and the conflict rules all
/// apply (§6.9). A deletion is not restorable: restore the revision before it.
#[utoipa::path(
    post,
    path = "/vaults/{v}/entries/{e}/restore",
    tag = "history",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("e" = Uuid, Path, description = "Entry id"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    request_body = RestoreRequest,
    responses(
        (status = 200, description = "The entry's new head", body = OpResult),
        (status = 403, description = "Below editor", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault, entry or revision", body = Problem, content_type = "application/problem+json"),
        (status = 400, description = "The revision is a deletion", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn restore_revision(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, e)): Path<(Uuid, Uuid)>,
    Body(body): Body<RestoreRequest>,
) -> Result<Json<OpResult>, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let p = &caller.caller.principal;
    let a = authz::require_role(&state.db, &snap, p, v, VaultRole::Editor).await?;
    let entry = EntryId::from_uuid(e).ok_or(ApiError::of(ProblemType::NotFound))?;
    // Under the caller's subject: their own configuration entries are
    // restorable too (phase 8 review H1), nobody else's.
    let mut tx = db::begin_tenant(&state.db, a.tenant, Some(&p.sub)).await?;
    let namespace: String =
        sqlx::query_scalar("SELECT namespace FROM entry WHERE vault_id = $1 AND id = $2")
            .bind(v)
            .bind(e)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ApiError::of(ProblemType::NotFound))?;
    let rev = sqlx::query(
        "SELECT path, blob_hash, size FROM revision WHERE vault_id = $1 AND entry_id = $2 AND id = $3",
    )
    .bind(v)
    .bind(e)
    .bind(body.revision)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::of(ProblemType::NotFound))?;
    let head = sqlx::query(concat!(
        head_select!(),
        " WHERE e.vault_id = $1 AND e.id = $2"
    ))
    .bind(v)
    .bind(e)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::of(ProblemType::NotFound))
    .and_then(|r| head_from_row(&r))?;
    tx.commit().await?;
    let blob = hash_of(rev.try_get("blob_hash")?)?.ok_or_else(|| {
        ApiError::with_detail(
            ProblemType::InvalidRequest,
            "this revision is a deletion; restore the revision before it",
        )
    })?;
    // A deleted entry comes back where it was; a live one stays where it is.
    let path = if head.deleted {
        stored_path(&rev.try_get::<String, _>("path")?)?
    } else {
        head.path.clone()
    };
    let size = u64::try_from(rev.try_get::<i64, _>("size")?).unwrap_or_default();
    let space = if namespace == "notes" {
        super::space::Space::notes()
    } else {
        super::space::Space {
            namespace,
            owner: Some(p.sub.clone()),
            categories: a.config.clone(),
        }
    };
    let req = CommitRequest {
        idempotency_key: format!("restore-{}", Uuid::new_v4()),
        operations: vec![Operation::Put {
            entry,
            path,
            base: Some(head.revision),
            blob,
            size,
        }],
        epoch: None,
        space: space.is_config().then(|| "config".to_owned()),
    };
    let canonical = serde_json::to_vec(&req).map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let cx = Cx {
        state: &state,
        a: &a,
        v,
        device: caller.device,
        // Restore stays a device's: a token never rewrites history.
        key: None,
        sub: &p.sub,
        req: &req,
        request_hash: ChunkHash::of(&canonical),
        space: &space,
    };
    match decide(&cx, &HashMap::new(), false).await? {
        Decided::Done(r) => {
            r.results.into_iter().next().map(Json).ok_or_else(|| {
                ApiError::Unavailable("a commit of one operation returned none".into())
            })
        }
        Decided::HeadMoved => Err(ApiError::Unavailable("restore raced a merge".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_keys_fold_case_fully() {
        let k = |s: &str| path_key(&VaultPath::parse(s).unwrap());
        assert_eq!(k("Note.md"), k("note.md"));
        assert_eq!(k("Straße.md"), k("STRASSE.md"));
        assert_ne!(k("a.md"), k("b.md"));
    }
}
