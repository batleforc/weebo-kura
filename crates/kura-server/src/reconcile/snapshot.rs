//! The authorization cache: the last reconciled configuration, read from
//! `config_state`, never from this replica's files (decision 31).
//!
//! A `NOTIFY` on `kura_config` (sent by the reconciler's write, through the
//! replica's `LISTEN` connection, see `notify`) reloads at once; the poll every
//! 5 s stays as the fallback, so a lost notification never leaves a replica
//! stale for longer than that.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use kura_api::PreviousState;
use sqlx::PgPool;
use sqlx::Row as _;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::ReconcileError;
use super::files::ConfigFiles;
use super::status::StoredStatus;
use crate::app::AppState;

pub const REFRESH: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: i64,
    pub generation: Vec<u8>,
    pub writer_version: String,
    pub written_at: time::OffsetDateTime,
    pub config: kura_api::Config,
    /// Enabled tenants: slug to id, so a tenant setting can be made.
    pub tenant_ids: BTreeMap<String, Uuid>,
    pub status: StoredStatus,
}

/// The current snapshot, if `config_state` moved past `current`.
pub async fn load_if_changed(
    pool: &PgPool,
    current: Option<i64>,
) -> Result<Option<Snapshot>, ReconcileError> {
    // The id alone first: the documents are fetched only when it moved.
    let latest: Option<i64> = sqlx::query_scalar("SELECT max(id) FROM config_state")
        .fetch_one(pool)
        .await?;
    let Some(id) = latest else { return Ok(None) };
    if Some(id) == current {
        return Ok(None);
    }
    let row = sqlx::query("SELECT generation, writer_version, written_at, documents, status FROM config_state WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let documents: serde_json::Value = row.try_get("documents")?;
    let files: BTreeMap<String, String> =
        serde_json::from_value(documents.get("files").cloned().unwrap_or_default())
            .map_err(|e| ReconcileError::Stored(e.to_string()))?;
    let files = ConfigFiles {
        files: files.into_iter().collect(),
        generation: [0; 32],
    };
    // Stored by a validated write; a failure here means a newer writer used a
    // field this binary does not know (§13): keep the previous snapshot.
    let config = super::check(&files, &PreviousState::default()).map_err(|errors| {
        ReconcileError::Stored(
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        )
    })?;
    let status: StoredStatus = serde_json::from_value(row.try_get("status")?)
        .map_err(|e| ReconcileError::Stored(e.to_string()))?;
    let tenant_ids = sqlx::query("SELECT slug, id FROM kura_tenant_ids()")
        .fetch_all(pool)
        .await?
        .iter()
        .map(|r| Ok((r.try_get("slug")?, r.try_get("id")?)))
        .collect::<Result<_, sqlx::Error>>()?;
    Ok(Some(Snapshot {
        id,
        generation: row.try_get("generation")?,
        writer_version: row.try_get("writer_version")?,
        written_at: row.try_get("written_at")?,
        config,
        tenant_ids,
        status,
    }))
}

/// Refreshes the cache until `shutdown`. Readiness latches on the first
/// snapshot and never drops on a database error (§10.2).
pub async fn run(pool: PgPool, state: Arc<AppState>, shutdown: CancellationToken) {
    loop {
        let current = state.snapshot().map(|s| s.id);
        match load_if_changed(&pool, current).await {
            Ok(Some(snapshot)) => {
                tracing::info!(id = snapshot.id, "configuration loaded from PostgreSQL");
                state.set_snapshot(snapshot);
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!(error = %e, "stored configuration not refreshed, the cached one stays")
            }
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = state.hub.config.notified() => {}
            () = tokio::time::sleep(REFRESH) => {}
        }
    }
}
