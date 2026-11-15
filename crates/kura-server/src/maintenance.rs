//! Retention, garbage collection and the `Delete` purge (RFC 0001 §6.9,
//! decisions 24 and 33). One pass at a time across replicas, inside the
//! configured off-peak window, in batches of 1,000 with a pause between
//! them; every step is idempotent, so an interrupted pass loses nothing.

use std::collections::HashSet;
use std::time::Duration;

use futures_util::{StreamExt as _, TryStreamExt as _};
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt as _};
use sqlx::Row as _;
use sqlx::pool::PoolConnection;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::AppState;

/// Rows per batch (§6.9).
pub const BATCH: i32 = 1000;
/// Between batches, so maintenance never holds the database for long.
pub const PAUSE: Duration = Duration::from_secs(1);
/// Taken exclusively by the backup job: while it is held, nothing deletes a
/// chunk or an object, so a dump never names an object its copy lacks (§6.9).
pub const BACKUP_LOCK: i64 = 0x6b75_7261_6261_636b;
/// One pass at a time across replicas.
const PASS_LOCK: i64 = 0x6b75_7261_6d61_696e;
/// How a pass paces itself; tests shorten both.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    /// Between batches.
    pub pause: Duration,
    /// Objects with no row younger than this are never swept: their row may
    /// be about to be written (§6.9). Rows carry their own 24 hours in SQL.
    pub orphan_grace: time::Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pause: PAUSE,
            orphan_grace: time::Duration::hours(24),
        }
    }
}
/// The default `deletedEntryTtl` (§6.9).
const DELETED_ENTRY_TTL_HOURS: u64 = 90 * 24;
/// Retention durations are capped at 100 years: a larger one would overflow
/// PostgreSQL's interval and keeps everything anyway.
const MAX_HOURS: u64 = 100 * 365 * 24;

#[derive(Debug, thiserror::Error)]
pub enum MaintenanceError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error("object storage: {0}")]
    Objects(#[from] object_store::Error),
    /// The session holding the shared backup lock is gone (a failover, an
    /// idle cutoff): a backup may be running, so no object is deleted.
    #[error("the backup lock was lost; object deletion stopped until the next pass")]
    LockLost,
}

/// Errors unless the session `pid` still holds the shared backup lock, the
/// check `backup.rs` makes on its side (review 2026-11-15 #15).
async fn lock_held(db: &sqlx::PgPool, pid: i32) -> Result<(), MaintenanceError> {
    let held: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks WHERE locktype = 'advisory' AND pid = $1 AND mode = 'ShareLock' AND granted)",
    )
    .bind(pid)
    .fetch_one(db)
    .await?;
    if held {
        Ok(())
    } else {
        Err(MaintenanceError::LockLost)
    }
}

/// The off-peak window, `HH:MM-HH:MM` in UTC; equal ends mean all day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct Window {
    start: u16,
    end: u16,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            start: 60,
            end: 5 * 60,
        }
    }
}

impl TryFrom<String> for Window {
    type Error = String;
    fn try_from(s: String) -> Result<Self, String> {
        let minute = |t: &str| -> Option<u16> {
            let (h, m) = t.trim().split_once(':')?;
            let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
            (h < 24 && m < 60).then(|| h.saturating_mul(60).saturating_add(m))
        };
        let (a, b) = s
            .split_once('-')
            .ok_or_else(|| format!("window {s:?}: expected HH:MM-HH:MM"))?;
        match (minute(a), minute(b)) {
            (Some(start), Some(end)) => Ok(Self { start, end }),
            _ => Err(format!("window {s:?}: expected HH:MM-HH:MM in UTC")),
        }
    }
}

impl Window {
    pub fn contains(self, minute_of_day: u16) -> bool {
        match self.start.cmp(&self.end) {
            std::cmp::Ordering::Equal => true,
            std::cmp::Ordering::Less => (self.start..self.end).contains(&minute_of_day),
            // Across midnight.
            std::cmp::Ordering::Greater => minute_of_day >= self.start || minute_of_day < self.end,
        }
    }
}

/// What one pass removed.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub revisions: u64,
    pub entries: u64,
    pub blobs: u64,
    pub chunks: u64,
    pub objects: u64,
    pub orphans: u64,
    pub purged_vaults: u64,
    /// Vaults whose maintenance failed this pass; the others went on.
    pub failed_vaults: u64,
    /// Collection and the purge waited: the backup lock was held, or no
    /// object storage is configured.
    pub gc_skipped: bool,
}

/// Runs a pass every hour while inside `window`, until `shutdown`.
pub async fn run(state: crate::app::SharedState, window: Window, shutdown: CancellationToken) {
    loop {
        let now = time::OffsetDateTime::now_utc();
        let minute = u16::from(now.hour())
            .saturating_mul(60)
            .saturating_add(u16::from(now.minute()));
        let wait = if window.contains(minute) {
            match pass(&state, Settings::default(), &shutdown).await {
                Ok(Some(r)) => tracing::info!(report = ?r, "maintenance pass done"),
                Ok(None) => {}
                Err(e) => {
                    // A whole pass failing alerts too, not only one vault
                    // (review 2026-11-15 #20).
                    state.metrics.maintenance_failures_total.inc();
                    tracing::warn!(error = %e, "maintenance pass failed, retried next hour");
                }
            }
            Duration::from_secs(3600)
        } else {
            Duration::from_secs(600)
        };
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(wait) => {}
        }
    }
}

/// One pass, unless another replica runs one (`None`).
pub async fn pass(
    state: &AppState,
    settings: Settings,
    shutdown: &CancellationToken,
) -> Result<Option<Report>, MaintenanceError> {
    // Session locks on one connection held for the pass, never an open
    // transaction across object storage calls.
    let mut conn = state.db.acquire().await?;
    let mine: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(PASS_LOCK)
        .fetch_one(&mut *conn)
        .await?;
    if !mine {
        return Ok(None);
    }
    let r = run_pass(state, &mut conn, settings, shutdown).await;
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock_all()")
        .execute(&mut *conn)
        .await
    {
        // The connection goes back broken; closing it releases the locks.
        tracing::warn!(error = %e, "maintenance locks not released, closing the connection");
        drop(conn.detach());
    }
    r.map(Some)
}

struct VaultRow {
    id: Uuid,
    retention: Option<kura_api::spec::Retention>,
    active: bool,
    due: bool,
}

async fn run_pass(
    state: &AppState,
    conn: &mut PoolConnection<sqlx::Postgres>,
    settings: Settings,
    shutdown: &CancellationToken,
) -> Result<Report, MaintenanceError> {
    let mut report = Report::default();
    let vaults =
        sqlx::query("SELECT vault_id, retention, active, due FROM kura_maintenance_vaults()")
            .fetch_all(&state.db)
            .await?
            .iter()
            .map(|r| {
                Ok(VaultRow {
                    id: r.try_get("vault_id")?,
                    retention: serde_json::from_value(r.try_get("retention")?).ok(),
                    active: r.try_get("active")?,
                    due: r.try_get("due")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;
    let objects = match &state.storage {
        Some(s) => {
            let free: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock_shared($1)")
                .bind(BACKUP_LOCK)
                .fetch_one(&mut **conn)
                .await?;
            free.then(|| s.objects())
        }
        None => None,
    };
    report.gc_skipped = objects.is_none();
    if !report.gc_skipped {
        #[expect(clippy::cast_precision_loss, reason = "unix seconds fit an f64")]
        state
            .metrics
            .maintenance_gc_timestamp_seconds
            .set(time::OffsetDateTime::now_utc().unix_timestamp() as f64);
    }
    // The session holding the locks, checked before each object deletion:
    // every query below runs on other connections of the pool.
    let lock_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut **conn)
        .await?;
    let batches = Batches {
        db: &state.db,
        pause: settings.pause,
        shutdown,
        lock_pid,
    };
    for v in &vaults {
        if shutdown.is_cancelled() {
            break;
        }
        // One vault failing (a timeout, a bad policy) never stops the others.
        if let Err(e) = one_vault(&batches, objects.as_deref(), v, settings, &mut report).await {
            report.failed_vaults = report.failed_vaults.saturating_add(1);
            state.metrics.maintenance_failures_total.inc();
            tracing::warn!(error = %e, vault = %v.id, "maintenance of a vault failed, retried next pass");
        }
    }
    let m = &state.metrics.maintenance_deleted_total;
    for (outcome, n) in [
        ("revision", report.revisions),
        ("entry", report.entries),
        ("blob", report.blobs),
        ("chunk", report.chunks),
        ("object", report.objects),
        ("orphan", report.orphans),
        ("purged", report.purged_vaults),
    ] {
        m.with_label_values(&[outcome]).inc_by(n);
    }
    let awaiting: i64 = sqlx::query_scalar("SELECT kura_tenants_awaiting_key_destruction()")
        .fetch_one(&state.db)
        .await?;
    state.metrics.tenants_awaiting_key_destruction.set(awaiting);
    Ok(report)
}

/// Retention, collection or the purge of one vault.
async fn one_vault(
    batches: &Batches<'_>,
    objects: Option<&dyn ObjectStore>,
    v: &VaultRow,
    settings: Settings,
    report: &mut Report,
) -> Result<(), MaintenanceError> {
    if v.due {
        if let Some(o) = objects
            && purge_vault(batches, o, v.id).await?
        {
            report.purged_vaults = report.purged_vaults.saturating_add(1);
        }
        return Ok(());
    }
    if !v.active {
        return Ok(());
    }
    let ret = v.retention.clone().unwrap_or_default();
    let hours = |d: kura_api::Duration| format!("{} hours", d.hours().min(MAX_HOURS));
    let age = ret.max_revision_age.map(hours);
    let per_entry = ret
        .max_revisions_per_entry
        .and_then(|n| i32::try_from(n).ok());
    report.revisions = report.revisions.saturating_add(
        batches
            .each(|db| {
                let age = age.clone();
                async move {
                    sqlx::query_scalar::<_, i64>(
                        "SELECT kura_prune_revisions($1, $2::interval, $3, $4)",
                    )
                    .bind(v.id)
                    .bind(age)
                    .bind(per_entry)
                    .bind(BATCH)
                    .fetch_one(&db)
                    .await
                }
            })
            .await?,
    );
    let ttl = ret
        .deleted_entry_ttl
        .map_or_else(|| format!("{DELETED_ENTRY_TTL_HOURS} hours"), hours);
    report.entries = report.entries.saturating_add(
        batches
            .each(|db| {
                let ttl = ttl.clone();
                async move {
                    sqlx::query_scalar::<_, i64>("SELECT kura_purge_deleted($1, $2::interval, $3)")
                        .bind(v.id)
                        .bind(ttl)
                        .bind(BATCH)
                        .fetch_one(&db)
                        .await
                }
            })
            .await?,
    );
    let Some(o) = objects else { return Ok(()) };
    report.blobs = report.blobs.saturating_add(
        batches
            .each(|db| async move {
                sqlx::query_scalar::<_, i64>("SELECT kura_gc_blobs($1, $2)")
                    .bind(v.id)
                    .bind(BATCH)
                    .fetch_one(&db)
                    .await
            })
            .await?,
    );
    let (chunks, deleted) = collect_chunks(batches, o, v.id).await?;
    report.chunks = report.chunks.saturating_add(chunks);
    report.objects = report.objects.saturating_add(deleted);
    report.orphans = report
        .orphans
        .saturating_add(sweep_orphans(batches, o, v.id, settings.orphan_grace).await?);
    Ok(())
}

/// Repeats one batch until it removes nothing, pausing between batches.
struct Batches<'a> {
    db: &'a sqlx::PgPool,
    pause: Duration,
    shutdown: &'a CancellationToken,
    /// The session holding the shared backup lock.
    lock_pid: i32,
}

impl Batches<'_> {
    async fn each<F, Fut>(&self, batch: F) -> Result<u64, sqlx::Error>
    where
        F: Fn(sqlx::PgPool) -> Fut,
        Fut: std::future::Future<Output = Result<i64, sqlx::Error>>,
    {
        let mut total = 0u64;
        loop {
            let n = batch(self.db.clone()).await?;
            if n <= 0 {
                return Ok(total);
            }
            total = total.saturating_add(n.unsigned_abs());
            if self.shutdown.is_cancelled() {
                return Ok(total);
            }
            tokio::time::sleep(self.pause).await;
        }
    }
}

/// Unreferenced chunks: each batch's rows are deleted and committed, then
/// the objects they named (decision 24). An object whose deletion fails is
/// left to the orphan sweep. Returns (rows, objects).
async fn collect_chunks(
    b: &Batches<'_>,
    objects: &dyn ObjectStore,
    vault: Uuid,
) -> Result<(u64, u64), MaintenanceError> {
    let (mut rows, mut deleted) = (0u64, 0u64);
    loop {
        lock_held(b.db, b.lock_pid).await?;
        let keys: Vec<String> = sqlx::query_scalar("SELECT kura_gc_chunks($1, $2)")
            .bind(vault)
            .bind(BATCH)
            .fetch_all(b.db)
            .await?;
        if keys.is_empty() {
            return Ok((rows, deleted));
        }
        rows = rows.saturating_add(keys.len() as u64);
        deleted = deleted.saturating_add(delete_objects(objects, keys).await);
        if b.shutdown.is_cancelled() {
            return Ok((rows, deleted));
        }
        tokio::time::sleep(b.pause).await;
    }
}

async fn delete_objects(objects: &dyn ObjectStore, keys: Vec<String>) -> u64 {
    let mut n = 0u64;
    for k in keys {
        match objects.delete(&ObjectPath::from(k.as_str())).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => n = n.saturating_add(1),
            Err(e) => tracing::warn!(error = %e, "object not deleted; the orphan sweep retries"),
        }
    }
    n
}

/// Objects under the vault's prefix that no chunk row names and older than
/// the grace period: left by a crash between an object PUT and its row (§6.9).
async fn sweep_orphans(
    b: &Batches<'_>,
    objects: &dyn ObjectStore,
    vault: Uuid,
    grace: time::Duration,
) -> Result<u64, MaintenanceError> {
    let cutoff = time::OffsetDateTime::now_utc().saturating_sub(grace);
    let prefix = ObjectPath::from(vault.to_string());
    let mut listing = objects.list(Some(&prefix));
    let mut swept = 0u64;
    let mut page: Vec<String> = Vec::new();
    loop {
        let next = listing.next().await;
        if let Some(meta) = &next {
            let meta = meta.as_ref().map_err(|e| object_store::Error::Generic {
                store: "list",
                source: e.to_string().into(),
            })?;
            if meta.last_modified.timestamp() <= cutoff.unix_timestamp() {
                page.push(meta.location.to_string());
            }
        }
        if page.len() >= BATCH.unsigned_abs() as usize || (next.is_none() && !page.is_empty()) {
            lock_held(b.db, b.lock_pid).await?;
            let known: HashSet<String> =
                sqlx::query_scalar::<_, String>("SELECT kura_known_objects($1, $2)")
                    .bind(vault)
                    .bind(&page)
                    .fetch_all(b.db)
                    .await?
                    .into_iter()
                    .collect();
            let orphans: Vec<String> = page.drain(..).filter(|k| !known.contains(k)).collect();
            swept = swept.saturating_add(delete_objects(objects, orphans).await);
        }
        if next.is_none() {
            return Ok(swept);
        }
    }
}

/// Purges one vault whose `Delete` grace period passed: rows in batches,
/// then every object under its prefix, then the vault row. `false` when its
/// document came back meanwhile (decision 33).
async fn purge_vault(
    b: &Batches<'_>,
    objects: &dyn ObjectStore,
    vault: Uuid,
) -> Result<bool, MaintenanceError> {
    let cutoff = loop {
        // Read before each batch, so the last one's (the batch that finds the
        // vault still due and nothing left) precedes any restore: an object
        // written after it belongs to the vault come back (review 2026-11-15 #8).
        let cutoff: time::OffsetDateTime =
            sqlx::query_scalar("SELECT now()").fetch_one(b.db).await?;
        let n: i64 = sqlx::query_scalar("SELECT kura_purge_vault($1, $2)")
            .bind(vault)
            .bind(BATCH)
            .fetch_one(b.db)
            .await?;
        if n < 0 {
            return Ok(false);
        }
        if n == 0 {
            break cutoff;
        }
        if b.shutdown.is_cancelled() {
            return Ok(false);
        }
        tokio::time::sleep(b.pause).await;
    };
    // The vault row is the only way back to its prefix: it stays until every
    // object is gone, and the next pass tries again.
    if !purge_objects(b.db, b.lock_pid, objects, vault, cutoff).await? {
        return Ok(false);
    }
    let done: bool = sqlx::query_scalar("SELECT kura_purge_vault_finish($1)")
        .bind(vault)
        .fetch_one(b.db)
        .await?;
    if done {
        tracing::warn!(target: "audit", event = "vault_purged", vault = %vault, "vault purged after its Delete grace period");
    }
    Ok(done)
}

/// Deletes a purged vault's objects written before `cutoff` that no chunk
/// row names; `true` once nothing is left under its prefix. An object newer
/// than `cutoff` or named by a row is kept: the vault came back and a device
/// uploaded it again, its row in flight or landed. `lock_pid` is the session
/// holding the shared backup lock; once it lost the lock, nothing is deleted.
pub async fn purge_objects(
    db: &sqlx::PgPool,
    lock_pid: i32,
    objects: &dyn ObjectStore,
    vault: Uuid,
    cutoff: time::OffsetDateTime,
) -> Result<bool, MaintenanceError> {
    let prefix = ObjectPath::from(vault.to_string());
    // shortcut: the prefix is listed whole in memory (about 100 bytes a key);
    // stream it in pages if vaults reach millions of objects.
    let listed: Vec<(String, bool)> = objects
        .list(Some(&prefix))
        .map(|m| {
            m.map(|m| {
                (
                    m.location.to_string(),
                    i128::from(m.last_modified.timestamp_millis())
                        < cutoff.unix_timestamp_nanos() / 1_000_000,
                )
            })
        })
        .try_collect()
        .await?;
    let mut kept = 0usize;
    for page in listed.chunks(BATCH.unsigned_abs() as usize) {
        let old: Vec<String> = page
            .iter()
            .filter(|(_, old)| *old)
            .map(|(k, _)| k.clone())
            .collect();
        lock_held(db, lock_pid).await?;
        let known: HashSet<String> =
            sqlx::query_scalar::<_, String>("SELECT kura_known_objects($1, $2)")
                .bind(vault)
                .bind(&old)
                .fetch_all(db)
                .await?
                .into_iter()
                .collect();
        let gone: Vec<String> = old.into_iter().filter(|k| !known.contains(k)).collect();
        let n = gone.len();
        let deleted = delete_objects(objects, gone).await;
        kept = kept
            .saturating_add(page.len().saturating_sub(n))
            .saturating_add(n.saturating_sub(usize::try_from(deleted).unwrap_or(n)));
    }
    Ok(kept == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_parse_and_wrap_midnight() {
        let w = Window::try_from("01:00-05:00".to_owned()).unwrap();
        assert_eq!(w, Window::default());
        assert!(w.contains(60) && w.contains(299) && !w.contains(300) && !w.contains(0));
        let night = Window::try_from("22:30-02:00".to_owned()).unwrap();
        assert!(night.contains(23 * 60) && night.contains(30) && !night.contains(12 * 60));
        let always = Window::try_from("00:00-00:00".to_owned()).unwrap();
        assert!(always.contains(0) && always.contains(1439));
        for bad in ["", "1-2", "24:00-01:00", "01:60-02:00", "01:00"] {
            assert!(Window::try_from(bad.to_owned()).is_err(), "{bad}");
        }
    }
}
