//! `POST /v1/vaults/{v}/commits` (RFC 0001 §6.3, §6.5): one transaction under
//! the vault row lock, consecutive `seq` values, idempotency per device.
//! Phase 3 has no merge: an edit that diverged from the head is stored as a
//! conflict file (phase 4 adds the three-way merge).

use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::{Path, State};
use kura_api::VaultRole;
use kura_core::wire::{
    CommitRequest, CommitResponse, EntryState, OpResult, Operation, Outcome, RevisionId,
};
use kura_core::{ChunkHash, EntryId, VaultPath};
use sqlx::Row as _;
use unicode_normalization::UnicodeNormalization as _;
use uuid::Uuid;

use super::SharedState;
use super::body::Raw;
use super::device::DeviceCaller;
use crate::authz;
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};

/// Operations per commit (§4.4).
pub const MAX_OPERATIONS: usize = 1000;
/// A commit body: 1,000 operations of a few hundred bytes each.
pub const MAX_COMMIT_BYTES: usize = 1 << 20;
/// Numbered suffixes tried for an occupied path, each one query under the
/// vault lock; past them, the entry's own id makes the name unique.
const MAX_SUFFIX: u32 = 9;

type Tx = sqlx::Transaction<'static, sqlx::Postgres>;

/// The case-folded form paths are unique on (§6.3), computed by the server only.
pub fn path_key(p: &VaultPath) -> String {
    caseless::default_case_fold_str(p.as_str()).nfc().collect()
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
}

impl Commit<'_> {
    async fn occupant(&mut self, key: &str) -> Result<Option<Uuid>, ApiError> {
        if let Some(o) = self.occupancy.get(key) {
            return Ok(*o);
        }
        let o: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM entry WHERE vault_id = $1 AND namespace = 'notes' AND NOT deleted AND path_key = $2",
        )
        .bind(self.vault)
        .bind(key)
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
        let mut candidate = path.clone();
        for n in 2..=MAX_SUFFIX {
            match self.occupant(&path_key(&candidate)).await? {
                None => return Ok(candidate),
                Some(o) if o == owner => return Ok(candidate),
                Some(_) => {}
            }
            candidate = path.suffixed(&n.to_string()).map_err(|e| refused(i, e))?;
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
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
    Raw(body): Raw,
) -> Result<Json<CommitResponse>, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    // T3: the role is checked on the vault before the body is even parsed.
    let a = authz::require_role(
        &state.db,
        &snap,
        &caller.caller.principal,
        v,
        VaultRole::Editor,
    )
    .await?;
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
    // The idempotency hash covers the parsed request, not its whitespace.
    let canonical = serde_json::to_vec(&req).map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let request_hash = ChunkHash::of(&canonical);
    let sub = &caller.caller.principal.sub;

    let mut tx = db::begin_tenant(&state.db, a.tenant, Some(sub)).await?;
    let head_seq: i64 = sqlx::query_scalar("SELECT head_seq FROM vault WHERE id = $1 FOR UPDATE")
        .bind(v)
        .fetch_one(&mut *tx)
        .await?;
    // Under the lock, so two retries racing see one stored result (§6.3).
    let stored = sqlx::query("SELECT request_hash, result FROM commit_record WHERE vault_id = $1 AND device_id = $2 AND idempotency_key = $3")
        .bind(v)
        .bind(caller.device)
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
        return Ok(Json(result.0));
    }

    let device_name: String = sqlx::query_scalar("SELECT name FROM device WHERE id = $1")
        .bind(caller.device)
        .fetch_one(&mut *tx)
        .await?;
    let today = time::OffsetDateTime::now_utc().date();
    let ids: Vec<Uuid> = req
        .operations
        .iter()
        .map(|o| *o.entry().as_uuid())
        .collect();
    let heads = sqlx::query(concat!(
        head_select!(),
        " WHERE e.vault_id = $1 AND e.id = ANY($2)"
    ))
    .bind(v)
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?
    .iter()
    .map(|r| head_from_row(r).map(|h| (h.id, h)))
    .collect::<Result<HashMap<_, _>, _>>()?;
    let bases = load_bases(&mut tx, v, &req).await?;
    check_blobs(&mut tx, v, &req).await?;
    let keys: Vec<String> = req
        .operations
        .iter()
        .filter_map(|o| match o {
            Operation::Put { path, .. } => Some(path_key(path)),
            Operation::Delete { .. } => None,
        })
        .collect();
    let occupancy: HashMap<String, Option<Uuid>> = sqlx::query(
        "SELECT path_key, id FROM entry WHERE vault_id = $1 AND namespace = 'notes' AND NOT deleted AND path_key = ANY($2)",
    )
    .bind(v)
    .bind(&keys)
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
    };
    let mut results = Vec::with_capacity(req.operations.len());
    for (i, op) in req.operations.iter().enumerate() {
        results.push(apply(&mut c, i, op, &bases).await?);
    }
    let new_head = c.next_seq.saturating_sub(1);
    let writes = std::mem::take(&mut c.writes);
    drop(c);
    store(&mut tx, v, a.tenant, caller.device, sub, &writes).await?;
    let response = CommitResponse {
        head_seq: u64::try_from(new_head).unwrap_or_default(),
        results,
    };
    sqlx::query("UPDATE vault SET head_seq = $2 WHERE id = $1")
        .bind(v)
        .bind(new_head)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO commit_record (vault_id, tenant_id, device_id, idempotency_key, request_hash, result)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(v)
    .bind(a.tenant)
    .bind(caller.device)
    .bind(&req.idempotency_key)
    .bind(request_hash.as_bytes().as_slice())
    .bind(sqlx::types::Json(&response))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if !writes.is_empty() {
        state.metrics.commits_total.inc();
    }
    Ok(Json(response))
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
        sqlx::query("SELECT hash, size FROM blob WHERE vault_id = $1 AND hash = ANY($2)")
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
    // T17: a base must be a revision of this entry in this vault.
    let base = match op {
        Operation::Put { base, .. } => *base,
        Operation::Delete { base, .. } => Some(*base),
    };
    let base_rev = match base {
        None => None,
        Some(b) => match (bases.get(&b), &existing) {
            (Some(r), Some(_)) if r.entry == id => Some(r),
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
            // Both sides changed the content: the head stays, the push becomes a conflict file.
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

/// Set-based writes, in seq order: an entry that frees a path is written
/// before the one that takes it, so the unique path index holds row by row.
async fn store(
    tx: &mut Tx,
    v: Uuid,
    tenant: Uuid,
    device: Uuid,
    sub: &str,
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
        "INSERT INTO entry (vault_id, tenant_id, id, path, path_key, head_revision, last_seq, deleted)
         SELECT $1, $2, u.id, u.path, u.key, u.rev, u.seq, u.deleted
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
