//! `GET /v1/me` (RFC 0003 §4.4): who the console is signed in as, before it
//! has a device. Only what the caller's own token already says, read
//! through the reconciled tenants; never another subject.

use axum::Json;
use axum::extract::State;
use kura_api::TenantRole;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::SharedState;
use super::caller::Caller;
use crate::authz::memberships;
use crate::problem::{ApiError, Problem, ProblemType};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TenantMembership {
    /// The tenant's slug.
    pub slug: String,
    /// `member`, `admin` or `owner`: the highest role the caller's claims map to.
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    /// The OIDC subject.
    pub subject: String,
    /// `name`, else `preferred_username`, as the identity provider sent it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The tenants the caller belongs to; a binding alone makes a member.
    pub tenants: Vec<TenantMembership>,
    pub platform_admin: bool,
}

const fn tenant_role(r: TenantRole) -> &'static str {
    match r {
        TenantRole::Member => "member",
        TenantRole::Admin => "admin",
        TenantRole::Owner => "owner",
    }
}

/// The signed-in caller: subject, tenant roles, platform admin flag.
#[utoipa::path(
    get,
    path = "/me",
    tag = "me",
    responses(
        (status = 200, description = "The caller", body = Me),
        (status = 401, description = "No valid access token", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Over 60 a minute", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn me(State(state): State<SharedState>, caller: Caller) -> Result<Json<Me>, ApiError> {
    let p = &caller.principal;
    // No device yet: counted per subject, as a registration is.
    state
        .limits
        .check(&p.sub, "GET", "/v1/me")
        .map_err(ApiError::RateLimited)?;
    let snapshot = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let name = ["name", "preferred_username"]
        .iter()
        .find_map(|c| p.claims.get(*c).and_then(serde_json::Value::as_str))
        .map(str::to_owned);
    Ok(Json(Me {
        subject: p.sub.clone(),
        name,
        tenants: memberships(&snapshot, p)
            .into_iter()
            .map(|m| TenantMembership {
                slug: m.slug,
                role: tenant_role(m.role).into(),
            })
            .collect(),
        platform_admin: state.verifier.is_platform_admin(p),
    }))
}
