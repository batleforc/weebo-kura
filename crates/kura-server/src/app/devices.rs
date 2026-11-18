//! `/v1/devices` (RFC 0001 §4.4, §6.2, decision 35).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kura_api::TenantRole;
use serde::{Deserialize, Serialize};
use sqlx::Row as _;
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::caller::Caller;
use super::device::DeviceCaller;
use crate::authz::{Membership, memberships};
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};

/// Live devices a user may hold (§4.4).
pub const MAX_DEVICES: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Desktop,
    Mobile,
    /// The web console (RFC 0003 §6.2): one per Authentik session, never the
    /// configuration space, purged after 30 days idle.
    Web,
}

impl Profile {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
            Self::Web => "web",
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterDevice {
    /// Shown in device lists and conflict file names, 1 to 64 bytes.
    pub name: String,
    pub profile: Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: Uuid,
    pub name: String,
    pub profile: String,
    /// RFC 3339.
    pub created_at: String,
    pub last_seen: String,
    pub revoked: bool,
    /// The owner's subject, shown only to a tenant admin for others' devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The owner's name as their identity provider gives it, beside `owner`
    /// (RFC 0006 D13); absent until the owner's device was seen with one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
    /// The vaults this device caught up on (RFC 0006), for the caller's own
    /// devices only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vaults: Vec<DeviceVault>,
}

/// One vault a device follows, and when it last caught up on it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeviceVault {
    pub vault: Uuid,
    /// RFC 3339.
    pub caught_up_at: String,
}

pub(super) fn rfc3339(t: time::OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn device(r: &sqlx::postgres::PgRow, owner: Option<String>) -> Result<Device, sqlx::Error> {
    Ok(Device {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        profile: r.try_get("profile")?,
        created_at: rfc3339(r.try_get("created_at")?),
        last_seen: rfc3339(r.try_get("last_seen")?),
        revoked: r
            .try_get::<Option<time::OffsetDateTime>, _>("revoked_at")?
            .is_some(),
        owner,
        owner_name: None,
        vaults: Vec::new(),
    })
}

fn members(state: &SharedState, caller: &Caller) -> Result<Vec<Membership>, ApiError> {
    let snapshot = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    Ok(memberships(&snapshot, &caller.principal))
}

/// Register this device.
#[utoipa::path(
    post,
    path = "/devices",
    tag = "devices",
    request_body = RegisterDevice,
    responses(
        (status = 200, description = "A web device this Authentik session already has", body = Device),
        (status = 201, description = "Registered; send its id in X-Kura-Device", body = Device),
        (status = 401, description = "Invalid token, or a session whose device was revoked", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a member of any tenant", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Already 20 live devices", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Over 10 registrations a minute", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn register_device(
    State(state): State<SharedState>,
    caller: Caller,
    Body(body): Body<RegisterDevice>,
) -> Result<(StatusCode, Json<Device>), ApiError> {
    // No device yet: registrations are counted per subject.
    state
        .limits
        .check(&caller.principal.sub, "POST", "/v1/devices")
        .map_err(ApiError::RateLimited)?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "name: 1 to 64 bytes",
        ));
    }
    let snapshot = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let ids = crate::authz::device_tenants(&state.db, &snapshot, &caller.principal).await?;
    if ids.is_empty() {
        return Err(ApiError::of(ProblemType::InsufficientRole));
    }
    let sub = &caller.principal.sub;
    super::device::require_session(&caller.principal)?;
    let sid = caller.principal.sid.as_deref();
    let auth_time = caller.principal.auth_time;
    let mut tx = db::begin_subject(&state.db, sub).await?;
    // Serializes the registrations of one user, so the limit holds.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('kura.device:' || $1))")
        .bind(sub)
        .execute(&mut *tx)
        .await?;
    let sid_revoked: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM device WHERE revoked_at IS NOT NULL
                AND (session_sid = $1 OR (user_sub = $3 AND session_auth_time = $2)))",
    )
    .bind(sid)
    .bind(auth_time)
    .bind(sub)
    .fetch_one(&mut *tx)
    .await?;
    if sid_revoked {
        tracing::warn!(target: "audit", event = "device_registration_refused", sub = %sub, "registration from a revoked session");
        return Err(ApiError::of(ProblemType::DeviceRevoked));
    }
    // One web device per Authentik session: a browser signing in again in
    // the same session gets the device it already has (RFC 0003 T12).
    if body.profile == Profile::Web
        && let Some(sid) = sid
    {
        let same = sqlx::query(
            "SELECT id, name, profile, created_at, last_seen, revoked_at FROM device
             WHERE profile = 'web' AND session_sid = $1 AND revoked_at IS NULL AND signed_out_at IS NULL
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(sid)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = same {
            let found = device(&row, None)?;
            tx.commit().await?;
            return Ok((StatusCode::OK, Json(found)));
        }
    }
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM device WHERE revoked_at IS NULL AND signed_out_at IS NULL",
    )
    .fetch_one(&mut *tx)
    .await?;
    if live >= MAX_DEVICES {
        return Err(ApiError::with_detail(
            ProblemType::QuotaExceeded,
            "at most 20 live devices; revoke one first",
        ));
    }
    let row = sqlx::query(
        "INSERT INTO device (user_sub, session_sid, session_auth_time, name, profile, owner_name) VALUES ($1, $2, $5, $3, $4, $6)
         RETURNING id, name, profile, created_at, last_seen, revoked_at",
    )
    .bind(sub)
    .bind(sid)
    .bind(name)
    .bind(body.profile.as_str())
    .bind(auth_time)
    .bind(caller.principal.display_name())
    .fetch_one(&mut *tx)
    .await?;
    let created = device(&row, None)?;
    sqlx::query("SELECT kura_device_link($1, $2)")
        .bind(created.id)
        .bind(&ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!(target: "audit", event = "device_registered", sub = %sub, device = %created.id, profile = body.profile.as_str());
    device_event(&state, &ids, created.id, "device.registered").await;
    Ok((StatusCode::CREATED, Json(created)))
}

/// List devices: your own, and those of the tenants you administer.
#[utoipa::path(
    get,
    path = "/devices",
    tag = "devices",
    params(("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Devices", body = Vec<Device>),
        (status = 401, description = "Invalid token or device", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn list_devices(
    State(state): State<SharedState>,
    caller: DeviceCaller,
) -> Result<Json<Vec<Device>>, ApiError> {
    let sub = caller.caller.principal.sub.clone();
    let mut tx = db::begin_subject(&state.db, &sub).await?;
    // A signed-out web device is gone for its browser: not listed (it is
    // purged after 30 days idle).
    let rows = sqlx::query("SELECT id, name, profile, created_at, last_seen, revoked_at FROM device WHERE signed_out_at IS NULL ORDER BY created_at, id LIMIT 1000")
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    let mut out = rows
        .iter()
        .map(|r| device(r, None))
        .collect::<Result<Vec<_>, _>>()?;
    for (d, vault, at) in super::sync_state::my_device_vaults(&state, &sub).await? {
        if let Some(own) = out.iter_mut().find(|o| o.id == d) {
            own.vaults.push(DeviceVault {
                vault,
                caught_up_at: rfc3339(at),
            });
        }
    }
    let mut seen: std::collections::HashSet<Uuid> = out.iter().map(|d| d.id).collect();
    for m in members(&state, &caller.caller)?
        .iter()
        .filter(|m| m.role >= TenantRole::Admin)
    {
        let mut tx = db::begin_tenant(&state.db, m.tenant, None).await?;
        let rows = sqlx::query("SELECT id, user_sub, owner_name, name, profile, created_at, last_seen, revoked_at FROM kura_tenant_devices()")
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        for r in &rows {
            let owner: String = r.try_get("user_sub")?;
            let mut d = device(r, (owner != sub).then_some(owner))?;
            if d.owner.is_some() {
                d.owner_name = r.try_get("owner_name")?;
            }
            if seen.insert(d.id) {
                out.push(d);
            }
        }
    }
    Ok(Json(out))
}

/// Revoke a device: yours, or one of a tenant you administer.
#[utoipa::path(
    delete,
    path = "/devices/{d}",
    tag = "devices",
    params(("d" = Uuid, Path, description = "Device id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 204, description = "Revoked"),
        (status = 401, description = "Invalid token or device", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such device you may revoke", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn revoke_device(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(d): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let sub = &caller.caller.principal.sub;
    let mut tx = db::begin_subject(&state.db, sub).await?;
    let own: Option<Uuid> = sqlx::query_scalar(
        "UPDATE device SET revoked_at = coalesce(revoked_at, now()) WHERE id = $1 RETURNING id",
    )
    .bind(d)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut revoked = own.is_some();
    let mut admin_tenant = None;
    if !revoked {
        for m in members(&state, &caller.caller)?
            .iter()
            .filter(|m| m.role >= TenantRole::Admin)
        {
            let mut tx = db::begin_tenant(&state.db, m.tenant, None).await?;
            let r: Option<Uuid> = sqlx::query_scalar("SELECT kura_tenant_device_revoke($1)")
                .bind(d)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
            if r.is_some() {
                revoked = true;
                admin_tenant = Some(m.tenant);
                break;
            }
        }
    }
    if !revoked {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    tracing::info!(target: "audit", event = "device_revoked", sub = %sub, device = %d);
    // The tenants the device is in: the caller's own, or the one an admin
    // revoked it from.
    let tenants: Vec<Uuid> = match (own, admin_tenant) {
        (Some(_), _) => {
            let snapshot = state
                .snapshot()
                .ok_or(ApiError::of(ProblemType::Unavailable))?;
            crate::authz::device_tenants(&state.db, &snapshot, &caller.caller.principal).await?
        }
        (None, Some(t)) => vec![t],
        (None, None) => Vec::new(),
    };
    device_event(&state, &tenants, d, "device.revoked").await;
    // Its sockets close on every replica (§6.2).
    if let Err(e) = crate::notify::notify_revoked(&state.db, d).await {
        tracing::warn!(error = %e, "revocation not broadcast; its sockets close at token expiry");
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `device.registered` or `device.revoked` in each of the device's tenants
/// (RFC 0003 §4.2, §9): its id only, never its kind, name or owner (T5). A device
/// belongs to a person, not a tenant, so each tenant's event has its own
/// transaction, after the fact's; a failure is logged, the fact stands.
async fn device_event(state: &SharedState, tenants: &[Uuid], device: Uuid, kind: &'static str) {
    if state.notifier.is_none() {
        return;
    }
    let Some(snapshot) = state.snapshot() else {
        return;
    };
    for &tenant in tenants {
        let Some(slug) = snapshot
            .tenant_ids
            .iter()
            .find(|(_, id)| **id == tenant)
            .map(|(s, _)| s.clone())
        else {
            continue;
        };
        let done = async {
            let mut tx = db::begin_tenant(&state.db, tenant, None).await?;
            let verb = if kind == "device.revoked" {
                "revoked"
            } else {
                "registered"
            };
            crate::notifier::record(
                state,
                &mut tx,
                crate::notifier::Fact {
                    tenant,
                    tenant_slug: &slug,
                    vault: None,
                    kind,
                    title: format!("A device was {verb}"),
                    message: format!("Tenant {slug}: a device was {verb}."),
                    data: serde_json::json!({ "device": { "id": device } }),
                    dedupe: None,
                },
            )
            .await?;
            tx.commit().await.map_err(ApiError::from)
        };
        if let Err(e) = done.await {
            tracing::warn!(error = %e, %kind, "notification event not recorded");
        }
    }
}

/// Sign this browser out (RFC 0003 §6.2): the caller's own `web` device
/// stops counting as live and its id is refused as unknown. Unlike a
/// revocation it bans nothing: the same Authentik session may sign in again,
/// and the plugin or CLI it approved keep working (decision 24).
#[utoipa::path(
    post,
    path = "/devices/{d}/sign-out",
    tag = "devices",
    params(("d" = Uuid, Path, description = "Your web device")),
    responses(
        (status = 204, description = "Signed out, or already"),
        (status = 401, description = "Invalid token", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "Not one of your web devices", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn sign_out_device(
    State(state): State<SharedState>,
    caller: Caller,
    Path(d): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let sub = &caller.principal.sub;
    state
        .limits
        .check(sub, "POST", "/v1/devices/{d}/sign-out")
        .map_err(ApiError::RateLimited)?;
    let mut tx = db::begin_subject(&state.db, sub).await?;
    let done = sqlx::query(
        "UPDATE device SET signed_out_at = coalesce(signed_out_at, now())
         WHERE id = $1 AND profile = 'web' AND revoked_at IS NULL",
    )
    .bind(d)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    if done == 0 {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    tracing::info!(target: "audit", event = "device_signed_out", sub = %sub, device = %d);
    // Its sockets close on every replica, as on a revocation.
    if let Err(e) = crate::notify::notify_revoked(&state.db, d).await {
        tracing::warn!(error = %e, "sign-out not broadcast; its sockets close at their next check");
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Purges devices idle 180 days (web: 30) and commit records older than 7 days, daily, until `shutdown` (§6.1).
pub async fn purge_loop(pool: sqlx::PgPool, shutdown: tokio_util::sync::CancellationToken) {
    loop {
        match sqlx::query_scalar::<_, i64>("SELECT kura_device_purge()")
            .fetch_one(&pool)
            .await
        {
            Ok(n) if n > 0 => tracing::info!(purged = n, "idle devices purged"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "device purge failed, retried tomorrow"),
        }
        match sqlx::query_scalar::<_, i64>("SELECT kura_scoped_token_purge()")
            .fetch_one(&pool)
            .await
        {
            Ok(n) if n > 0 => tracing::info!(purged = n, "expired scoped tokens purged"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "scoped token purge failed, retried tomorrow"),
        }
        // What tokens uploaded is remembered 24 hours (RFC 0004 T6).
        match sqlx::query_scalar::<_, i64>("SELECT kura_token_upload_purge()")
            .fetch_one(&pool)
            .await
        {
            Ok(n) if n > 0 => tracing::info!(purged = n, "old token uploads purged"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "token upload purge failed, retried tomorrow"),
        }
        // Idempotency records live 7 days (§6.1).
        match sqlx::query_scalar::<_, i64>("SELECT kura_commit_purge()")
            .fetch_one(&pool)
            .await
        {
            Ok(n) if n > 0 => tracing::info!(purged = n, "old commit records purged"),
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "commit record purge failed, retried tomorrow"),
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(std::time::Duration::from_secs(86_400)) => {}
        }
    }
}
