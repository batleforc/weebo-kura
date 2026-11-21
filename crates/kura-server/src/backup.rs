//! The bucket half of a backup, and the restore drill's check (RFC 0001
//! §6.9, §10.3). `copy` holds the backup lock, so maintenance deletes no
//! chunk or object meanwhile, records the database time T it took it at, and
//! copies every object the second bucket lacks. A restore recovers
//! PostgreSQL to T (CloudNativePG archives the WAL continuously): every
//! object the database names at T existed when the copy listed it.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use futures_util::TryStreamExt as _;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt as _, PutPayload, WriteMultipart};
use sqlx::Row as _;
use uuid::Uuid;

use crate::app::AppState;
use crate::maintenance::BACKUP_LOCK;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("object storage: {0}")]
    Objects(#[from] object_store::Error),
    #[error("no object storage configured")]
    NoStorage,
    #[error("the backup lock stayed taken for {0:?}: a maintenance pass is running")]
    Busy(Duration),
    #[error(
        "the backup lock was lost during the copy (the session dropped): this copy is not a backup"
    )]
    LockLost,
    #[error("{0}")]
    Other(String),
}

/// What one copy did; `recover_to` is the point in time to restore
/// PostgreSQL to, RFC 3339.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Copied {
    pub recover_to: String,
    pub vaults: u64,
    pub objects: u64,
    pub copied: u64,
}

/// Waits for the lock this long, retrying every 10 s: a maintenance pass
/// holds its shared side for the whole pass.
const LOCK_WAIT: Duration = Duration::from_secs(1800);

/// Copies every object of every vault that `target` lacks, under the
/// backup lock; writes `backups/<T>.json` in `target` last.
pub async fn copy(state: &AppState, target: &dyn ObjectStore) -> Result<Copied, BackupError> {
    copy_waiting(state, target, LOCK_WAIT).await
}

pub async fn copy_waiting(
    state: &AppState,
    target: &dyn ObjectStore,
    wait: Duration,
) -> Result<Copied, BackupError> {
    let source = state
        .storage
        .as_ref()
        .ok_or(BackupError::NoStorage)?
        .objects();
    let mut conn = state.db.acquire().await?;
    let started = tokio::time::Instant::now();
    loop {
        let mine: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(BACKUP_LOCK)
            .fetch_one(&mut *conn)
            .await?;
        if mine {
            break;
        }
        if started.elapsed() >= wait {
            return Err(BackupError::Busy(wait));
        }
        tokio::time::sleep(Duration::from_secs(10).min(wait)).await;
    }
    let r = copy_locked(state, &mut conn, source.as_ref(), target).await;
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock_all()")
        .execute(&mut *conn)
        .await
    {
        tracing::warn!(error = %e, "backup lock not released, closing the connection");
        drop(conn.detach());
    }
    r
}

async fn copy_locked(
    state: &AppState,
    conn: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    source: &dyn ObjectStore,
    target: &dyn ObjectStore,
) -> Result<Copied, BackupError> {
    // T: from the database's clock, the one point-in-time recovery uses.
    let recover_to: time::OffsetDateTime = sqlx::query_scalar("SELECT now()")
        .fetch_one(&state.db)
        .await?;
    let vaults: Vec<Uuid> = sqlx::query_scalar("SELECT vault_id FROM kura_maintenance_vaults()")
        .fetch_all(&state.db)
        .await?;
    let mut out = Copied {
        recover_to: recover_to
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|e| BackupError::Other(e.to_string()))?,
        vaults: vaults.len() as u64,
        objects: 0,
        copied: 0,
    };
    // The key list streams to the target in parts as the copy goes; it only
    // becomes visible once the lock check below passed.
    let mut keys = None;
    if let Err(e) = copy_objects(source, target, &vaults, &mut out, &mut keys).await {
        if let Some(k) = keys {
            drop(k.abort().await);
        }
        return Err(e);
    }
    // A dropped session releases the lock silently, and maintenance may have
    // deleted objects since: then this copy proves nothing (review 6 M4).
    let held: Result<bool, BackupError> = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND pid = pg_backend_pid() AND mode = 'ExclusiveLock' AND granted)",
    )
    .fetch_one(&mut **conn)
    .await
    .map_err(|_| BackupError::LockLost);
    if !matches!(held, Ok(true)) {
        if let Some(k) = keys {
            drop(k.abort().await);
        }
        return Err(BackupError::LockLost);
    }
    // Every key the database may name at T: pruning the copy deletes only
    // keys no kept restore point lists (operator guide §12).
    let keys_path = ObjectPath::from(format!("backups/{}.keys", out.recover_to));
    match keys {
        Some(k) => drop(k.finish().await?),
        // Nothing listed: a multipart upload cannot be empty.
        None => drop(target.put(&keys_path, PutPayload::default()).await?),
    }
    let manifest =
        serde_json::to_vec_pretty(&out).map_err(|e| BackupError::Other(e.to_string()))?;
    target
        .put(
            &ObjectPath::from(format!("backups/{}.json", out.recover_to)),
            PutPayload::from(manifest),
        )
        .await?;
    sqlx::query("SELECT kura_backup_done($1, $2)")
        .bind(recover_to)
        .bind(i64::try_from(out.objects).unwrap_or(i64::MAX))
        .execute(&state.db)
        .await?;
    Ok(out)
}

/// An object's bytes, or none when it went since the listing: an upload that
/// lost a race or its quota, which no row names, so not part of the copy.
async fn fetch(
    source: &dyn ObjectStore,
    location: &ObjectPath,
) -> Result<Option<bytes::Bytes>, BackupError> {
    match source.get(location).await {
        Ok(r) => Ok(Some(r.bytes().await?)),
        Err(object_store::Error::NotFound { .. }) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Copies the objects `target` lacks, vault by vault, and appends every key
/// of the copy to `keys` (newline-separated, no trailing one), a multipart
/// upload opened at the first key.
async fn copy_objects(
    source: &dyn ObjectStore,
    target: &dyn ObjectStore,
    vaults: &[Uuid],
    out: &mut Copied,
    keys: &mut Option<WriteMultipart>,
) -> Result<(), BackupError> {
    for v in vaults {
        let prefix = ObjectPath::from(v.to_string());
        // Object keys are unique per upload, so a key present is the same bytes.
        let have: HashSet<ObjectPath> = target
            .list(Some(&prefix))
            .map_ok(|m| m.location)
            .try_collect()
            .await?;
        let mut listing = source.list(Some(&prefix));
        while let Some(meta) = listing.try_next().await? {
            let bytes = if have.contains(&meta.location) {
                None
            } else {
                let Some(b) = fetch(source, &meta.location).await? else {
                    continue;
                };
                Some(b)
            };
            let up = match keys {
                Some(up) => {
                    up.wait_for_capacity(4).await?;
                    up.write(b"\n");
                    up
                }
                None => keys.insert(WriteMultipart::new(
                    target
                        .put_multipart(&ObjectPath::from(format!(
                            "backups/{}.keys",
                            out.recover_to
                        )))
                        .await?,
                )),
            };
            up.write(meta.location.as_ref().as_bytes());
            out.objects = out.objects.saturating_add(1);
            let Some(bytes) = bytes else { continue };
            target.put(&meta.location, PutPayload::from(bytes)).await?;
            out.copied = out.copied.saturating_add(1);
        }
    }
    Ok(())
}

/// The restore drill's check: `count` random chunks fetched, decrypted with
/// their vault's data key (Vault Transit) and checked against their address.
/// `owner` is a pool of the database owner: the sample crosses tenants.
/// Returns (checked, failed).
pub async fn check_chunks(
    state: &AppState,
    owner: &sqlx::PgPool,
    count: u32,
) -> Result<(u64, u64), BackupError> {
    let storage: &Arc<crate::store::Storage> =
        state.storage.as_ref().ok_or(BackupError::NoStorage)?;
    let rows = sqlx::query(
        "SELECT vault_id, tenant_id, slug, hash, object_key FROM kura_sample_chunks($1)",
    )
    .bind(i32::try_from(count).unwrap_or(i32::MAX))
    .fetch_all(owner)
    .await?;
    let (mut checked, mut failed) = (0u64, 0u64);
    for r in &rows {
        let vault: Uuid = r.try_get("vault_id")?;
        let tenant: Uuid = r.try_get("tenant_id")?;
        let slug: String = r.try_get("slug")?;
        let raw: Vec<u8> = r.try_get("hash")?;
        let key_name: String = r.try_get("object_key")?;
        let hash = <[u8; 32]>::try_from(raw.as_slice())
            .map(kura_core::ChunkHash::from_bytes)
            .map_err(|_| BackupError::Other("stored hash is not 32 bytes".into()))?;
        checked = checked.saturating_add(1);
        let outcome = async {
            let key = storage
                .keys
                .get(&state.db, tenant, &slug, vault)
                .await
                .map_err(|e| format!("data key: {e:?}"))?;
            let stored = storage
                .objects()
                .get(&ObjectPath::from(key_name.as_str()))
                .await
                .map_err(|e| e.to_string())?
                .bytes()
                .await
                .map_err(|e| e.to_string())?;
            crate::store::crypto::open(&key, vault, &hash, &stored).map_err(|e| e.to_string())
        }
        .await;
        if let Err(e) = outcome {
            failed = failed.saturating_add(1);
            tracing::error!(vault = %vault, error = %e, "sampled chunk failed its check");
        }
    }
    Ok((checked, failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

    /// Review 2026-11-21 #9: an object deleted between the listing and the
    /// read is skipped; the backup does not fail.
    #[tokio::test]
    async fn an_object_gone_since_the_listing_is_skipped() {
        let store = InMemory::new();
        let kept = ObjectPath::from("v/kept");
        store.put(&kept, PutPayload::from_static(b"x")).await.ok();
        let fetched = fetch(&store, &kept).await.ok().flatten();
        assert_eq!(fetched.as_deref(), Some(&b"x"[..]));
        let gone = fetch(&store, &ObjectPath::from("v/gone")).await;
        assert!(matches!(gone, Ok(None)), "{gone:?}");
    }
}
