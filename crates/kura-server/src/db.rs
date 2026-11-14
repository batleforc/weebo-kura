//! PostgreSQL: the pool, migrations, the schema check, and transactions
//! scoped to one tenant or one subject (RFC 0001 §6.1, §6.11, decision 26).
//!
//! Queries are checked at runtime, not by sqlx's macros (plan D14): every
//! query runs in the integration tests against a real PostgreSQL.

use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Every migration of this binary, embedded at build time. Migrations newer
/// than this binary are ignored, so an older binary can run against a newer
/// schema (§13, `set_ignore_missing`).
pub static MIGRATOR: Migrator = Migrator {
    ignore_missing: true,
    ..sqlx::migrate!("./migrations")
};

/// Outbound calls to PostgreSQL wait at most this long for a connection (§6.11).
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(
        "the database schema is at migration {database}, this binary needs {binary}: run `kura-server migrate` first"
    )]
    SchemaTooOld { database: i64, binary: i64 },
}

impl From<DbError> for crate::problem::ApiError {
    fn from(e: DbError) -> Self {
        Self::Unavailable(e.to_string())
    }
}

impl From<sqlx::Error> for crate::problem::ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Unavailable(e.to_string())
    }
}

/// The pool of the application role, `kura_app`.
/// `epochs` counts the vault epochs its connections regenerate.
pub async fn connect(
    url: &str,
    max_connections: u32,
    epochs: Option<prometheus::IntCounter>,
) -> Result<PgPool, DbError> {
    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        // A failover breaks every connection: checking the cluster identity on
        // each new one means no commit lands under an epoch the promotion made
        // stale (decision 32).
        .after_connect(move |conn, _| {
            let epochs = epochs.clone();
            Box::pin(async move {
                match epoch_check(&mut *conn).await {
                    Ok(0) => {}
                    Ok(n) => {
                        if let Some(c) = epochs {
                            c.inc_by(n);
                        }
                        tracing::warn!(
                            vaults = n,
                            "cluster identity changed: vault epochs regenerated"
                        );
                    }
                    // Before migration 0011 (the `migrate` job's own pool).
                    Err(sqlx::Error::Database(d)) if d.code().as_deref() == Some("42883") => {}
                    // A connection that could not check may run under a stale
                    // epoch: it is refused, and the pool tries a new one.
                    Err(e) => {
                        tracing::warn!(error = %e, "epoch check failed, connection refused");
                        return Err(e);
                    }
                }
                Ok(())
            })
        })
        .connect(url)
        .await?)
}

/// Regenerates every vault's epoch when PostgreSQL's system identifier or
/// timeline changed since the last check; returns how many (§6.3).
pub async fn epoch_check<'c, E>(conn: E) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    let n: i64 = sqlx::query_scalar("SELECT kura_epoch_check()")
        .fetch_one(conn)
        .await?;
    Ok(u64::try_from(n).unwrap_or_default())
}

/// The restore runbook's step: a new epoch for every vault, so every device
/// resyncs without deleting (decision 32). Run as the migration owner.
pub async fn epoch_reset(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let n: i64 = sqlx::query_scalar("SELECT kura_epoch_reset()")
        .fetch_one(pool)
        .await?;
    Ok(u64::try_from(n).unwrap_or_default())
}

/// Applies the migrations, connected as the migration owner (a separate job
/// before each rollout, §13).
pub async fn migrate(pool: &PgPool) -> Result<(), DbError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

/// Refuses to serve against a schema older than this binary; a newer one is
/// fine (expand and contract, §13).
pub async fn check_schema(pool: &PgPool) -> Result<(), DbError> {
    let binary = MIGRATOR
        .migrations
        .iter()
        .map(|m| m.version)
        .max()
        .unwrap_or(0);
    let database: Option<i64> =
        sqlx::query_scalar("SELECT max(version) FROM _sqlx_migrations WHERE success")
            .fetch_one(pool)
            .await?;
    let database = database.unwrap_or(0);
    if database < binary {
        return Err(DbError::SchemaTooOld { database, binary });
    }
    Ok(())
}

/// A transaction that sees one tenant's rows, and the caller's own devices
/// when a subject is given. Both settings end with the transaction.
pub async fn begin_tenant(
    pool: &PgPool,
    tenant: Uuid,
    subject: Option<&str>,
) -> Result<Transaction<'static, Postgres>, DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "SELECT set_config('kura.tenant_id', $1, true), set_config('kura.subject', $2, true)",
    )
    .bind(tenant.to_string())
    .bind(subject.unwrap_or(""))
    .execute(&mut *tx)
    .await?;
    Ok(tx)
}

/// [`begin_tenant`] in one `REPEATABLE READ` snapshot, taken before any read:
/// a pull reads its entries and the `head_seq` it returns together (§5.4).
pub async fn begin_tenant_snapshot(
    pool: &PgPool,
    tenant: Uuid,
    subject: &str,
) -> Result<Transaction<'static, Postgres>, DbError> {
    let mut tx = pool
        .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;
    sqlx::query(
        "SELECT set_config('kura.tenant_id', $1, true), set_config('kura.subject', $2, true)",
    )
    .bind(tenant.to_string())
    .bind(subject)
    .execute(&mut *tx)
    .await?;
    Ok(tx)
}

/// A transaction that sees only the caller's own devices.
pub async fn begin_subject(
    pool: &PgPool,
    subject: &str,
) -> Result<Transaction<'static, Postgres>, DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT set_config('kura.subject', $1, true)")
        .bind(subject)
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// The tenant of a vault, before the tenant setting is known (a named
/// `SECURITY DEFINER` lookup, T4).
pub async fn vault_tenant(pool: &PgPool, vault: Uuid) -> Result<Option<Uuid>, DbError> {
    Ok(sqlx::query_scalar("SELECT kura_vault_tenant($1)")
        .bind(vault)
        .fetch_one(pool)
        .await?)
}
