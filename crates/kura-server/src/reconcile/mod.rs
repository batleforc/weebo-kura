//! Configuration reconciliation (RFC 0001 §4.3, §6.10): files are parsed and
//! validated as a whole; a valid set is written to PostgreSQL by one replica
//! at a time, under the guard of `kura_config_begin`; every replica then
//! authorizes from what is stored, never from its own files (decision 31).

pub mod files;
pub mod snapshot;
pub mod status;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use kura_api::{
    Chunking, ConfigError, DnsLabel, ManagedBy, PreviousState, Resource, StoredVault, VaultKey,
};
use sqlx::PgPool;
use sqlx::Row as _;
use tokio_util::sync::CancellationToken;

use crate::app::AppState;
use files::ConfigFiles;

/// This binary's version, the `writer_version` it stamps (§6.10).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The files are re-read this often; a change is applied on the next tick.
pub const POLL: Duration = Duration::from_secs(10);
/// Even with no local change, a reconciliation runs at least this often (§6.10).
pub const RESYNC: Duration = Duration::from_secs(60);

#[derive(Debug, thiserror::Error)]
pub enum ReconcileError {
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Files(#[from] files::FilesError),
    #[error("stored state is unreadable: {0}")]
    Stored(String),
}

/// A removal applied by this reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub kind: String,
    pub namespace: Option<String>,
    pub name: String,
    /// Set with `deletionPolicy: Delete`: the purge date.
    pub delete_after: Option<time::OffsetDateTime>,
}

#[derive(Debug)]
pub enum Outcome {
    /// Written to PostgreSQL by this replica.
    Applied { removed: Vec<Removed> },
    /// Valid, but another writer is newer, or nothing changed.
    NotWritten,
    /// The last valid configuration stays in force (§4.3).
    Rejected(Vec<ConfigError>),
}

/// The stored state validation needs.
pub async fn load_previous(pool: &PgPool) -> Result<PreviousState, ReconcileError> {
    let rows = sqlx::query(
        "SELECT kind, namespace, name, chunking, managed_by, disabled FROM kura_config_previous()",
    )
    .fetch_all(pool)
    .await?;
    let mut previous = PreviousState::default();
    let bad = |e: String| ReconcileError::Stored(e);
    for r in rows {
        let kind: String = r.try_get("kind")?;
        let name =
            DnsLabel::try_from(r.try_get::<String, _>("name")?).map_err(|e| bad(e.to_string()))?;
        let disabled: bool = r.try_get("disabled")?;
        if kind == "tenant" {
            if !disabled {
                previous.tenants.insert(name);
            }
            continue;
        }
        let namespace = r
            .try_get::<Option<String>, _>("namespace")?
            .unwrap_or_default();
        let namespace = DnsLabel::try_from(namespace).map_err(|e| bad(e.to_string()))?;
        let chunking: Option<serde_json::Value> = r.try_get("chunking")?;
        let chunking: Chunking =
            serde_json::from_value(chunking.unwrap_or_default()).map_err(|e| bad(e.to_string()))?;
        let managed_by = if r.try_get::<String, _>("managed_by")? == "api" {
            ManagedBy::Api
        } else {
            ManagedBy::Config
        };
        previous.vaults.insert(
            VaultKey { namespace, name },
            StoredVault {
                chunking,
                managed_by,
            },
        );
    }
    Ok(previous)
}

/// Parses and validates the files against the stored state.
pub fn check(
    files: &ConfigFiles,
    previous: &PreviousState,
) -> Result<kura_api::Config, Vec<ConfigError>> {
    let mut resources: Vec<Resource> = Vec::new();
    let mut errors = Vec::new();
    for (name, content) in &files.files {
        match kura_api::parse_file(name, content) {
            Ok(r) => resources.extend(r),
            Err(e) => errors.extend(e),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    kura_api::validate(resources, previous)
}

fn json<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

fn bytes(q: Option<kura_api::Quantity>) -> Option<i64> {
    q.and_then(|q| i64::try_from(q.bytes()).ok())
}

fn policy(p: kura_api::DeletionPolicy) -> &'static str {
    match p {
        kura_api::DeletionPolicy::Orphan => "Orphan",
        kura_api::DeletionPolicy::Delete => "Delete",
    }
}

/// One reconciliation. `local_change`: this replica's files changed since its
/// previous tick (or it just started).
pub async fn reconcile(
    pool: &PgPool,
    files: &ConfigFiles,
    local_change: bool,
) -> Result<Outcome, ReconcileError> {
    // The cheap check first: an unchanged generation, or a newer writer,
    // ends the tick before any parsing (eco review).
    let mut tx = pool.begin().await?;
    let allowed: bool = sqlx::query_scalar("SELECT kura_config_begin($1, $2, $3)")
        .bind(files.generation.as_slice())
        .bind(VERSION)
        .bind(local_change)
        .fetch_one(&mut *tx)
        .await?;
    if !allowed {
        tx.rollback().await?;
        return Ok(Outcome::NotWritten);
    }
    let previous = load_previous(pool).await?;
    let config = match check(files, &previous) {
        Ok(c) => c,
        Err(errors) => {
            tx.rollback().await?;
            return Ok(Outcome::Rejected(errors));
        }
    };

    for (slug, t) in &config.tenants {
        let spec = &t.doc.spec;
        sqlx::query("SELECT kura_tenant_upsert($1, $2, $3, $4, $5, $6, $7, $8, $9)")
            .bind(slug.as_str())
            .bind(spec.claim_mappings.as_ref().map(json))
            .bind(
                spec.vault_namespaces
                    .iter()
                    .map(DnsLabel::as_str)
                    .collect::<Vec<_>>(),
            )
            .bind(json(&spec.retention))
            .bind(bytes(spec.quota.storage))
            .bind(bytes(spec.quota.max_file_size))
            .bind(spec.search.enabled)
            .bind(policy(spec.deletion_policy))
            .bind(t.doc.metadata.force_orphan())
            .execute(&mut *tx)
            .await?;
    }
    for (key, v) in &config.vaults {
        let spec = &v.doc.spec;
        let id: Option<uuid::Uuid> =
            sqlx::query_scalar("SELECT kura_vault_upsert($1, $2, $3, $4, $5, $6, $7)")
                .bind(&spec.tenant_ref.name)
                .bind(key.namespace.as_str())
                .bind(key.name.as_str())
                .bind(json(&spec.chunking.unwrap_or(Chunking::DEFAULT)))
                .bind(spec.config_sync.as_ref().map(json))
                .bind(policy(spec.deletion_policy))
                .bind(v.doc.metadata.force_orphan())
                .fetch_one(&mut *tx)
                .await?;
        let id =
            id.ok_or_else(|| ReconcileError::Stored(format!("vault {key} could not be written")))?;
        let bindings: Vec<serde_json::Value> = spec
            .bindings
            .iter()
            .map(|b| {
                let (kind, subject) = match (&b.claim_value, &b.subject) {
                    (Some(c), _) => ("claim", c),
                    (None, Some(s)) => ("user", s),
                    (None, None) => ("claim", &String::new()),
                };
                serde_json::json!({ "kind": kind, "subject": subject, "role": json(&b.role) })
            })
            .collect();
        sqlx::query("SELECT kura_bindings_replace($1, $2)")
            .bind(id)
            .bind(serde_json::Value::Array(bindings))
            .execute(&mut *tx)
            .await?;
    }

    let tenants: Vec<&str> = config.tenants.keys().map(DnsLabel::as_str).collect();
    let vaults: Vec<serde_json::Value> = config
        .vaults
        .keys()
        .map(|k| serde_json::json!({ "namespace": k.namespace.as_str(), "name": k.name.as_str() }))
        .collect();
    let rows = sqlx::query(
        "SELECT kind, namespace, name, delete_after FROM kura_config_disable_missing($1, $2)",
    )
    .bind(tenants)
    .bind(serde_json::Value::Array(vaults))
    .fetch_all(&mut *tx)
    .await?;
    let mut removed = Vec::with_capacity(rows.len());
    for r in rows {
        removed.push(Removed {
            kind: r.try_get("kind")?,
            namespace: r.try_get("namespace")?,
            name: r.try_get("name")?,
            delete_after: r.try_get("delete_after")?,
        });
    }

    let documents: BTreeMap<&str, &str> = files
        .files
        .iter()
        .map(|(n, c)| (n.as_str(), c.as_str()))
        .collect();
    let status = status::build(&config, &removed);
    sqlx::query("SELECT kura_config_state_insert($1, $2, $3, $4)")
        .bind(files.generation.as_slice())
        .bind(VERSION)
        .bind(serde_json::json!({ "files": documents }))
        .bind(json(&status))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Outcome::Applied { removed })
}

/// The reconciler task: re-reads the directory every [`POLL`], reconciles on
/// a local change or every [`RESYNC`], until `shutdown`.
///
/// shortcut: polling instead of an inotify watch (`notify` 9 is a release
/// candidate); a change lands within 10 s. Upgrade if that delay matters.
pub async fn run(
    pool: PgPool,
    dir: std::path::PathBuf,
    state: Arc<AppState>,
    shutdown: CancellationToken,
) {
    run_every(pool, dir, state, shutdown, POLL).await;
}

/// [`run`] with another polling period (tests).
pub async fn run_every(
    pool: PgPool,
    dir: std::path::PathBuf,
    state: Arc<AppState>,
    shutdown: CancellationToken,
    poll: Duration,
) {
    let mut last_local: Option<[u8; 32]> = None;
    let mut last_rejected: BTreeSet<[u8; 32]> = BTreeSet::new();
    let mut since_resync = RESYNC;
    loop {
        let read_dir = dir.clone();
        let files = tokio::task::spawn_blocking(move || files::read_dir(&read_dir)).await;
        match files {
            Ok(Ok(files)) => {
                let local_change = last_local != Some(files.generation);
                if local_change || since_resync >= RESYNC {
                    since_resync = Duration::ZERO;
                    // A failed tick keeps the change pending: the retry must still
                    // count as this replica's own change, or an equal-version
                    // writer would refuse it (kura_config_begin).
                    if tick(&pool, &files, local_change, &state, &mut last_rejected).await {
                        last_local = Some(files.generation);
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "configuration directory unreadable, the stored configuration stays")
            }
            Err(e) => tracing::warn!(error = %e, "configuration reader task failed"),
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(poll) => since_resync = since_resync.saturating_add(poll),
        }
    }
}

async fn tick(
    pool: &PgPool,
    files: &ConfigFiles,
    local_change: bool,
    state: &AppState,
    rejected: &mut BTreeSet<[u8; 32]>,
) -> bool {
    match reconcile(pool, files, local_change).await {
        Ok(Outcome::Applied { removed }) => {
            state.set_rejection(None);
            tracing::info!(removed = removed.len(), "configuration reconciled");
            for r in &removed {
                tracing::warn!(target: "audit", kind = %r.kind, name = %r.name, delete_after = ?r.delete_after, "configuration object removed");
            }
        }
        Ok(Outcome::NotWritten) => state.set_rejection(None),
        Ok(Outcome::Rejected(errors)) => {
            // Counted once per rejected generation, not once per resync.
            if rejected.insert(files.generation) {
                state.metrics.config_rejected_total.inc();
                for e in &errors {
                    tracing::warn!(error = %e, "configuration rejected, the last valid one stays");
                }
            }
            state.set_rejection(Some(errors));
        }
        // A lock wait past lock_timeout means another replica is writing: next tick.
        Err(e) => {
            tracing::warn!(error = %e, "reconciliation failed, retried on the next tick");
            return false;
        }
    }
    true
}
