//! `GET /v1/admin/status`: the reconciled configuration's status, in the
//! kit's shape, for platform admins (RFC 0001 §4.3, §4.4).

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::SharedState;
use super::device::DeviceCaller;
use crate::problem::{ApiError, Problem, ProblemType};
use crate::reconcile::status::ObjectStatus;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    /// SHA-256 of the stored files, hex; absent before the first reconciliation.
    pub generation: Option<String>,
    pub writer_version: Option<String>,
    /// RFC 3339.
    pub written_at: Option<String>,
    /// One entry per declared or removed object, `Ready` condition first.
    #[schema(value_type = Vec<Object>)]
    pub objects: Vec<ObjectStatus>,
    /// This replica's last rejected change, `file#index path: message`.
    pub rejection: Vec<String>,
}

/// Status of the reconciled configuration.
#[utoipa::path(
    get,
    path = "/admin/status",
    tag = "admin",
    params(("X-Kura-Device" = uuid::Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Configuration status", body = AdminStatus),
        (status = 401, description = "No valid access token", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a platform admin", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn admin_status(
    State(state): State<SharedState>,
    caller: DeviceCaller,
) -> Result<Json<AdminStatus>, ApiError> {
    if !state.verifier.is_platform_admin(&caller.caller.principal) {
        return Err(ApiError::of(ProblemType::InsufficientRole));
    }
    current(&state).await.map(Json)
}

/// The status as this replica knows it; also the answer of a push (RFC 0009).
pub async fn current(state: &SharedState) -> Result<AdminStatus, ApiError> {
    let snapshot = state.snapshot();
    let rejection = state
        .rejection()
        .unwrap_or_default()
        .iter()
        .map(ToString::to_string)
        .collect();
    Ok(match snapshot {
        Some(s) => AdminStatus {
            generation: Some(s.generation.iter().map(|b| format!("{b:02x}")).collect()),
            writer_version: Some(s.writer_version.clone()),
            written_at: s
                .written_at
                .format(&time::format_description::well_known::Rfc3339)
                .ok(),
            objects: with_quota_conditions(state, &s).await?,
            rejection,
        },
        None => AdminStatus {
            generation: None,
            writer_version: None,
            written_at: None,
            objects: Vec::new(),
            rejection,
        },
    })
}

/// Adds the advisory `QuotaBelowUsage` condition to every tenant whose stored
/// bytes pass its quota: reads continue, writes are refused (§4.3). Computed
/// at request time, from the chunks actually stored.
async fn with_quota_conditions(
    state: &SharedState,
    s: &crate::reconcile::snapshot::Snapshot,
) -> Result<Vec<ObjectStatus>, ApiError> {
    use sqlx::Row as _;
    let usage = sqlx::query("SELECT slug, used FROM kura_tenant_usage()")
        .fetch_all(&state.db)
        .await?;
    let mut objects = s.status.objects.clone();
    for row in usage {
        let (slug, used): (String, i64) = (row.try_get("slug")?, row.try_get("used")?);
        let quota = s
            .config
            .tenants
            .get(slug.as_str())
            .and_then(|t| t.doc.spec.quota.storage);
        let Some(quota) = quota.and_then(|q| i64::try_from(q.bytes()).ok()) else {
            continue;
        };
        if used <= quota {
            continue;
        }
        let tenant = objects
            .iter_mut()
            .find(|o| o.kind == kura_api::spec::KIND_TENANT && o.name == slug);
        if let Some(o) = tenant {
            o.conditions.push(weebo_kit_api::Condition {
                type_: kura_api::ReasonCode::QuotaBelowUsage.as_str().into(),
                status: "True".into(),
                reason: kura_api::ReasonCode::QuotaBelowUsage.as_str().into(),
                message: format!(
                    "{used} bytes stored, quota {quota}: reads continue, writes are refused"
                ),
                last_transition_time: None,
                observed_generation: None,
            });
        }
    }
    // RFC 0004 §4.3: write tokens of a tenant whose switch is off.
    let writers = sqlx::query("SELECT slug, tokens FROM kura_write_token_tenants()")
        .fetch_all(&state.db)
        .await?;
    for row in writers {
        let (slug, tokens): (String, i64) = (row.try_get("slug")?, row.try_get("tokens")?);
        let on = s
            .config
            .tenants
            .get(slug.as_str())
            .is_some_and(|t| t.doc.spec.scoped_tokens.write);
        let tenant = objects
            .iter_mut()
            .find(|o| o.kind == kura_api::spec::KIND_TENANT && o.name == slug);
        if let (false, Some(o)) = (on, tenant) {
            o.conditions.push(weebo_kit_api::Condition {
                type_: kura_api::ReasonCode::TokenWriteDisabled.as_str().into(),
                status: "True".into(),
                reason: kura_api::ReasonCode::TokenWriteDisabled.as_str().into(),
                message: format!(
                    "{tokens} tokens carry write rules while scopedTokens.write is off: they read, their writes are refused"
                ),
                last_transition_time: None,
                observed_generation: None,
            });
        }
    }
    Ok(objects)
}
