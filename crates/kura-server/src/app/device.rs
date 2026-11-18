//! `auth::require_device` (RFC 0001 §6.2): every OIDC call but registration
//! names its device in `X-Kura-Device`. A revoked device is refused with
//! `device_revoked`; an unknown one, purged, or owned by another subject
//! (invisible under row-level security) with `device_unknown`.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use uuid::Uuid;

use super::SharedState;
use super::caller::Caller;
use crate::db;
use crate::problem::{ApiError, ProblemType};

pub const DEVICE_HEADER: &str = "x-kura-device";

/// An authenticated caller on a registered, non-revoked device of its own.
pub struct DeviceCaller {
    pub caller: Caller,
    pub device: Uuid,
}

impl FromRequestParts<SharedState> for DeviceCaller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        // A scoped token reaches only the read routes (`Reader`).
        super::reader::refuse_key_on_write(parts)?;
        let caller = Caller::from_request_parts(parts, state).await?;
        let device = parts
            .headers
            .get(DEVICE_HEADER)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| Uuid::parse_str(h).ok())
            .ok_or(ApiError::of(ProblemType::DeviceUnknown))?;
        require_device(state, &caller.principal, device).await?;
        rate_limit(state, parts, &device.to_string())?;
        Ok(Self { caller, device })
    }
}

/// Counts the call against the per-device limit of its route (§4.4).
pub fn rate_limit(state: &SharedState, parts: &Parts, key: &str) -> Result<(), ApiError> {
    let route = parts
        .extensions
        .get::<axum::extract::MatchedPath>()
        .map_or("", |p| p.as_str());
    state
        .limits
        .check(key, parts.method.as_str(), route)
        .map_err(ApiError::RateLimited)
}

/// A token must name its session: `sid` on a sign-in, `auth_time` after a
/// refresh (D34). One with neither cannot be tied to a revocation.
pub fn require_session(p: &crate::auth::Principal) -> Result<(), ApiError> {
    if p.sid.is_none() && p.auth_time.is_none() {
        return Err(ApiError::of(ProblemType::InvalidToken));
    }
    Ok(())
}

/// Refuses a revoked device, and any device used with a token of a session
/// whose device was revoked: otherwise a stolen refresh token would keep
/// working through another of the user's device ids (decision 35, T2).
pub async fn require_device(
    state: &SharedState,
    p: &crate::auth::Principal,
    device: Uuid,
) -> Result<(), ApiError> {
    require_session(p)?;
    let mut tx = db::begin_subject(&state.db, &p.sub).await?;
    let row: Option<(Option<time::OffsetDateTime>, bool, bool)> = sqlx::query_as(
        "SELECT revoked_at,
                EXISTS (SELECT 1 FROM device r WHERE r.revoked_at IS NOT NULL
                        AND (r.session_sid = $2 OR (r.user_sub = $4 AND r.session_auth_time = $3))),
                signed_out_at IS NOT NULL
         FROM device WHERE id = $1",
    )
    .bind(device)
    .bind(p.sid.as_deref())
    .bind(p.auth_time)
    .bind(&p.sub)
    .fetch_optional(&mut *tx)
    .await?;
    match row {
        // A signed-out web device is gone for this browser, nothing more
        // (RFC 0003 decision 24): unknown, not revoked.
        None | Some((None, false, true)) => Err(ApiError::of(ProblemType::DeviceUnknown)),
        Some((Some(_), _, _)) | Some((None, true, _)) => {
            Err(ApiError::of(ProblemType::DeviceRevoked))
        }
        Some((None, false, false)) => {
            // At most one write a minute per device; the tenant links follow the
            // caller's current memberships at the same pace.
            let touched = sqlx::query(
                "UPDATE device SET last_seen = now(), owner_name = $2 WHERE id = $1
                 AND (last_seen < now() - interval '1 minute' OR owner_name IS DISTINCT FROM $2)",
            )
            .bind(device)
            .bind(p.display_name())
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if touched > 0
                && let Some(snapshot) = state.snapshot()
            {
                let tenants = crate::authz::device_tenants(&state.db, &snapshot, p).await?;
                sqlx::query("SELECT kura_device_link($1, $2)")
                    .bind(device)
                    .bind(tenants)
                    .execute(&mut *tx)
                    .await?;
            }
            tx.commit().await?;
            Ok(())
        }
    }
}
