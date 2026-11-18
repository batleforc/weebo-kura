//! Where each device stands on a vault (RFC 0006): recorded from the
//! device's own pulls and commits, read back by `GET /vaults/{v}/sync`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kura_api::{TenantRole, VaultRole};
use kura_core::wire::SyncCounts;
use serde::{Deserialize, Serialize};
use sqlx::Row as _;
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::device::DeviceCaller;
use super::devices::rfc3339;
use crate::problem::{ApiError, Problem, ProblemType};
use crate::{authz, db};

/// One device on one vault.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceSync {
    pub device: Uuid,
    pub name: String,
    pub profile: String,
    /// The owner's subject: only in a manager's or tenant admin's answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub revoked: bool,
    /// RFC 3339; absent until the device finished a pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caught_up_at: Option<String>,
    /// Notes it may read changed since; absent until it finished a pass or
    /// when `resync` is set. Capped at 1,001, read as "1,000+".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behind: Option<i64>,
    /// The vault lost history since: the device runs a full resync next.
    pub resync: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pushed_at: Option<String>,
    /// What the device's last run left on its side (phase 3); absent until
    /// a client that reports has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report: Option<DeviceReport>,
}

/// The counts a device reported, and when.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceReport {
    pub pending: u32,
    pub conflicts: u32,
    pub unsyncable: u32,
    pub failed: u32,
    /// RFC 3339.
    pub at: String,
}

/// Above this, a count is a client's mistake, not a vault (§7.3).
const COUNT_MAX: u32 = 1_000_000;

/// Run one recording statement in its own transaction. A failure is logged
/// and counted, never returned: the record informs, sync must not depend on
/// it (§10.2).
async fn record(
    state: &SharedState,
    tenant: Uuid,
    q: sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
) {
    let done = async {
        let mut tx = db::begin_tenant(&state.db, tenant, None).await?;
        q.execute(&mut *tx).await?;
        tx.commit().await?;
        Ok::<(), db::DbError>(())
    }
    .await;
    if let Err(e) = done {
        state.metrics.device_sync_write_failures_total.inc();
        tracing::warn!(error = %e, %tenant, "device sync state not recorded");
    }
}

/// The last page of a pull: the device holds everything up to `seq`. At most
/// one write a minute unless the head moved (§4.2).
pub async fn caught_up(
    state: &SharedState,
    tenant: Uuid,
    vault: Uuid,
    device: Uuid,
    epoch: Uuid,
    seq: i64,
    scope: &[String],
) {
    let q = sqlx::query(
        "INSERT INTO device_sync (vault_id, tenant_id, device_id, epoch, caught_up_seq, caught_up_at, scope)
         VALUES ($1, $2, $3, $4, $5, now(), $6)
         ON CONFLICT (vault_id, device_id) DO UPDATE
         SET epoch = EXCLUDED.epoch, caught_up_seq = EXCLUDED.caught_up_seq,
             caught_up_at = now(), scope = EXCLUDED.scope
         WHERE device_sync.caught_up_seq IS DISTINCT FROM EXCLUDED.caught_up_seq
            OR device_sync.epoch IS DISTINCT FROM EXCLUDED.epoch
            OR device_sync.scope IS DISTINCT FROM EXCLUDED.scope
            OR device_sync.caught_up_at IS NULL
            OR device_sync.caught_up_at < now() - interval '1 minute'",
    )
    .bind(vault)
    .bind(tenant)
    .bind(device)
    .bind(epoch)
    .bind(seq)
    .bind(scope);
    record(state, tenant, q).await;
}

/// A commit from this device succeeded.
pub async fn pushed(state: &SharedState, tenant: Uuid, vault: Uuid, device: Uuid) {
    let q = sqlx::query(
        "INSERT INTO device_sync (vault_id, tenant_id, device_id, pushed_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (vault_id, device_id) DO UPDATE SET pushed_at = now()",
    )
    .bind(vault)
    .bind(tenant)
    .bind(device);
    record(state, tenant, q).await;
}

/// Report what this device's last run left on its side: counts only, never
/// paths (RFC 0006 phase 3). A device's own row; a scoped token is refused.
#[utoipa::path(
    put,
    path = "/vaults/{v}/sync/report",
    tag = "vaults",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = SyncCounts,
    responses(
        (status = 204, description = "Recorded"),
        (status = 400, description = "Not four counts, or one over 1,000,000", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "A scoped token", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault, or not readable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn put_report(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
    Body(c): Body<SyncCounts>,
) -> Result<StatusCode, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let access = authz::require_role(
        &state.db,
        &snap,
        &caller.caller.principal,
        v,
        VaultRole::Viewer,
    )
    .await?;
    if [c.pending, c.conflicts, c.unsyncable, c.failed]
        .iter()
        .any(|n| *n > COUNT_MAX)
    {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "a count is over 1,000,000",
        ));
    }
    let as_i32 = |n: u32| i32::try_from(n).unwrap_or(i32::MAX);
    let mut tx = db::begin_tenant(&state.db, access.tenant, None).await?;
    sqlx::query(
        "INSERT INTO device_sync (vault_id, tenant_id, device_id, pending, conflicts, unsyncable, failed, reported_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, now())
         ON CONFLICT (vault_id, device_id) DO UPDATE
         SET pending = EXCLUDED.pending, conflicts = EXCLUDED.conflicts,
             unsyncable = EXCLUDED.unsyncable, failed = EXCLUDED.failed, reported_at = now()",
    )
    .bind(v)
    .bind(access.tenant)
    .bind(caller.device)
    .bind(as_i32(c.pending))
    .bind(as_i32(c.conflicts))
    .bind(as_i32(c.unsyncable))
    .bind(as_i32(c.failed))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The devices syncing a vault: every one for its managers and the tenant's
/// admins, the caller's own for anyone else who reads it.
#[utoipa::path(
    get,
    path = "/vaults/{v}/sync",
    tag = "vaults",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "At most 1,000 devices, by name", body = Vec<DeviceSync>),
        (status = 403, description = "A scoped token", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault, or not readable", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn vault_sync(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
) -> Result<Json<Vec<DeviceSync>>, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let access = authz::require_role(
        &state.db,
        &snap,
        &caller.caller.principal,
        v,
        VaultRole::Viewer,
    )
    .await?;
    let all = access.role == VaultRole::Manager
        || access.tenant_role.is_some_and(|r| r >= TenantRole::Admin);
    let sub = &caller.caller.principal.sub;
    let mut tx = db::begin_tenant(&state.db, access.tenant, Some(sub)).await?;
    let current_epoch: Uuid = sqlx::query_scalar("SELECT epoch FROM vault WHERE id = $1")
        .bind(v)
        .fetch_one(&mut *tx)
        .await?;
    let rows = sqlx::query("SELECT * FROM kura_vault_sync($1, $2)")
        .bind(v)
        .bind(all)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        let epoch: Option<Uuid> = r.try_get("epoch")?;
        let resync = epoch.is_some_and(|e| e != current_epoch);
        let owner: String = r.try_get("user_sub")?;
        out.push(DeviceSync {
            device: r.try_get("device_id")?,
            name: r.try_get("name")?,
            profile: r.try_get("profile")?,
            owner: (all && owner != *sub).then_some(owner),
            revoked: r.try_get("revoked")?,
            caught_up_at: r
                .try_get::<Option<time::OffsetDateTime>, _>("caught_up_at")?
                .map(rfc3339),
            behind: if resync { None } else { r.try_get("behind")? },
            resync,
            pushed_at: r
                .try_get::<Option<time::OffsetDateTime>, _>("pushed_at")?
                .map(rfc3339),
            report: report(r)?,
        });
    }
    Ok(Json(out))
}

fn report(r: &sqlx::postgres::PgRow) -> Result<Option<DeviceReport>, sqlx::Error> {
    let Some(at) = r.try_get::<Option<time::OffsetDateTime>, _>("reported_at")? else {
        return Ok(None);
    };
    let n = |c: &str| -> Result<u32, sqlx::Error> {
        Ok(u32::try_from(r.try_get::<Option<i32>, _>(c)?.unwrap_or_default()).unwrap_or_default())
    };
    Ok(Some(DeviceReport {
        pending: n("pending")?,
        conflicts: n("conflicts")?,
        unsyncable: n("unsyncable")?,
        failed: n("failed")?,
        at: rfc3339(at),
    }))
}

/// The vaults each of the caller's devices caught up on, for `GET /devices`.
pub async fn my_device_vaults(
    state: &SharedState,
    sub: &str,
) -> Result<Vec<(Uuid, Uuid, time::OffsetDateTime)>, ApiError> {
    let mut tx = db::begin_subject(&state.db, sub).await?;
    let rows: Vec<(Uuid, Uuid, time::OffsetDateTime)> =
        sqlx::query_as("SELECT device_id, vault_id, caught_up_at FROM kura_my_device_vaults()")
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(rows)
}
