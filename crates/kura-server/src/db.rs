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
pub async fn connect(url: &str, max_connections: u32) -> Result<PgPool, DbError> {
    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(url)
        .await?)
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
