//! `/v1/tenants/{t}/notifications` (RFC 0003 §4.4): a tenant's webhook
//! rules. A tenant admin manages every rule of the tenant; a vault manager
//! only rules bound to a vault they manage, for vault events (§4.2, T5). The
//! URL, the signing secret and an `authorization` value are sealed; only the
//! host is ever shown again (T4).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kura_api::{TenantRole, VaultRole};
use serde::{Deserialize, Serialize};
use sqlx::Row as _;
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::device::DeviceCaller;
use crate::authz;
use crate::db;
use crate::notifier::{self, EVENTS, Fact, TENANT_EVENTS};
use crate::problem::{ApiError, Problem, ProblemType};
use crate::store::crypto;

/// Rules a tenant may hold (§4.3).
pub const MAX_RULES: i64 = 20;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Channel {
    /// `webhook`, the only kind.
    #[serde(rename = "type")]
    pub kind: String,
    /// `https`, public or allowed by the operator; never shown again.
    pub url: String,
    /// Sent as the `Authorization` header; never shown again.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRule {
    pub name: String,
    pub events: Vec<String>,
    /// A vault of the tenant; absent: the whole tenant (admins only).
    #[serde(default)]
    pub vault: Option<Uuid>,
    pub channel: Channel,
    #[serde(default = "yes")]
    pub enabled: bool,
}

const fn yes() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateRule {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub events: Option<Vec<String>>,
    /// A new URL; absent: kept.
    #[serde(default)]
    pub url: Option<String>,
    /// A new value; `""` removes it; absent: kept.
    #[serde(default)]
    pub authorization: Option<String>,
    /// `true` re-enables a rule and forgets its failures.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// A new signing secret, returned once.
    #[serde(default)]
    pub rotate_secret: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LastDelivery {
    pub at: String,
    pub result: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuleView {
    pub id: Uuid,
    pub name: String,
    pub events: Vec<String>,
    pub vault: Option<Uuid>,
    /// The URL's host: all that is shown of it after creation (§4.5).
    pub host: String,
    pub has_authorization: bool,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    pub consecutive_failures: i32,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<LastDelivery>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SavedRule {
    #[serde(flatten)]
    pub rule: RuleView,
    /// `whsec_…`, on creation and rotation only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryView {
    pub id: Uuid,
    pub event: String,
    pub at: String,
    pub attempts: i32,
    /// `pending`, `delivered` or `failed`.
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TestResult {
    /// The coarse result of the one attempt: a status, or `timeout`,
    /// `connect`, `tls`, `address_refused`, `redirect`, `status_4xx`, `status_5xx`.
    pub result: String,
}

fn rfc3339(t: time::OffsetDateTime) -> String {
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

fn invalid(detail: impl Into<String>) -> ApiError {
    ApiError::with_detail(ProblemType::InvalidRequest, detail)
}

/// Who may do what in a tenant's notifications.
struct Scope {
    tenant: Uuid,
    slug: String,
    /// `None`: a tenant admin; `Some`: the vaults a manager manages.
    vaults: Option<Vec<(Uuid, String)>>,
}

impl Scope {
    fn sees(&self, vault: Option<Uuid>) -> bool {
        match &self.vaults {
            None => true,
            Some(vs) => vault.is_some_and(|v| vs.iter().any(|(id, _)| *id == v)),
        }
    }
}

async fn authorize(
    state: &SharedState,
    caller: &DeviceCaller,
    slug: &str,
) -> Result<Scope, ApiError> {
    if state.notifier.is_none() {
        return Err(ApiError::of(ProblemType::NotificationsDisabled));
    }
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let p = &caller.caller.principal;
    // Another tenant answers as a missing one (T11).
    let tenant = *snap
        .tenant_ids
        .get(slug)
        .ok_or(ApiError::of(ProblemType::NotFound))?;
    let role = authz::memberships(&snap, p)
        .into_iter()
        .find(|m| m.tenant == tenant)
        .map(|m| m.role);
    if role.is_some_and(|r| r >= TenantRole::Admin) {
        return Ok(Scope {
            tenant,
            slug: slug.to_owned(),
            vaults: None,
        });
    }
    let accesses = authz::vault_accesses_in(&state.db, &snap, p, tenant, 1000).await?;
    if role.is_none() && accesses.is_empty() {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    let managed: Vec<(Uuid, String)> = accesses
        .into_iter()
        .filter(|a| a.role == VaultRole::Manager)
        .map(|a| (a.vault, a.name))
        .collect();
    if managed.is_empty() {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "notifications need a tenant admin, or a vault manager",
        ));
    }
    Ok(Scope {
        tenant,
        slug: slug.to_owned(),
        vaults: Some(managed),
    })
}

fn check_events(scope: &Scope, events: &[String], vault: Option<Uuid>) -> Result<(), ApiError> {
    if events.is_empty() || events.len() > EVENTS.len() {
        return Err(invalid("events: at least one event type"));
    }
    if let Some(e) = events.iter().find(|e| !EVENTS.contains(&e.as_str())) {
        return Err(invalid(format!("events: {e} is not an event type")));
    }
    // A manager: one vault of theirs, vault events only (§4.2).
    if scope.vaults.is_some() {
        if vault.is_none() || !scope.sees(vault) {
            return Err(ApiError::with_detail(
                ProblemType::InsufficientRole,
                "a vault manager's rule names one vault they manage",
            ));
        }
        if let Some(e) = events.iter().find(|e| TENANT_EVENTS.contains(&e.as_str())) {
            return Err(ApiError::with_detail(
                ProblemType::InsufficientRole,
                format!("{e} is a tenant event: a tenant admin subscribes to it"),
            ));
        }
    }
    Ok(())
}

/// The URL and its host, refused with one message for every case (§4.5).
async fn check_url(
    state: &SharedState,
    url: &str,
) -> Result<(kura_core::webhook::Target, String), ApiError> {
    let n = state
        .notifier
        .as_ref()
        .ok_or(ApiError::of(ProblemType::NotificationsDisabled))?;
    let refused = || invalid(kura_core::webhook::TargetError.to_string());
    let target =
        kura_core::webhook::Target::parse(url, |ip| crate::net::webhook_allowed(ip, &n.allowed))
            .map_err(|_| refused())?;
    if target.literal().is_none() {
        // Every address the name has now must be allowed; the resolver asks
        // again at each attempt (§6.5).
        let host = target.host();
        let addrs: Vec<std::net::IpAddr> = tokio::net::lookup_host((host.as_str(), 443))
            .await
            .map_err(|_| refused())?
            .map(|a| a.ip())
            .collect();
        if addrs.is_empty()
            || !addrs
                .iter()
                .all(|a| crate::net::webhook_allowed(*a, &n.allowed))
        {
            return Err(refused());
        }
    }
    let host = target.host();
    Ok((target, host))
}

fn check_authorization(a: &str) -> Result<(), ApiError> {
    if a.len() > 1024 || a.chars().any(char::is_control) {
        return Err(invalid(
            "authorization: at most 1,024 bytes, no control character",
        ));
    }
    Ok(())
}

fn check_name(n: &str) -> Result<String, ApiError> {
    let n = n.trim();
    if n.is_empty() || n.len() > 64 || n.chars().any(char::is_control) {
        return Err(invalid("name: 1 to 64 bytes"));
    }
    Ok(n.to_owned())
}

fn new_secret() -> Result<(Vec<u8>, String), ApiError> {
    let mut secret = vec![0u8; 32];
    aws_lc_rs::rand::fill(&mut secret)
        .map_err(|_| ApiError::Unavailable("no randomness".into()))?;
    let shown = format!("whsec_{}", URL_SAFE_NO_PAD.encode(&secret));
    Ok((secret, shown))
}

async fn tenant_key(
    state: &SharedState,
    scope: &Scope,
) -> Result<std::sync::Arc<crypto::DataKey>, ApiError> {
    // Without storage keys a secret could not be sealed (§7.4).
    let storage = state.storage.as_ref().ok_or(ApiError::with_detail(
        ProblemType::Unavailable,
        "no storage keys to seal notification secrets",
    ))?;
    storage
        .keys
        .notifications(&state.db, scope.tenant, &scope.slug)
        .await
}

fn seal(key: &crypto::DataKey, rule: Uuid, field: &str, value: &[u8]) -> Result<Vec<u8>, ApiError> {
    crypto::seal_value(key, &notifier::context(rule, field), value)
        .map_err(|e| ApiError::Unavailable(e.to_string()))
}

/// A rule and its last result, for every query that shows rules.
macro_rules! rule_select {
    () => {
        "SELECT r.id, r.name, r.events, r.vault_id, r.host, r.sealed_authorization IS NOT NULL AS has_authorization,
        r.enabled, r.disabled_reason, r.consecutive_failures, r.created_by, l.updated_at AS last_at, l.result AS last_result
     FROM notification_rule r
     LEFT JOIN LATERAL (SELECT updated_at, result FROM notification_delivery d
                        WHERE d.rule_id = r.id AND d.result IS NOT NULL ORDER BY d.updated_at DESC LIMIT 1) l ON true"
    };
}

fn view(r: &sqlx::postgres::PgRow) -> Result<RuleView, ApiError> {
    let last_at: Option<time::OffsetDateTime> = r.try_get("last_at")?;
    let last_result: Option<String> = r.try_get("last_result")?;
    Ok(RuleView {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        events: r.try_get("events")?,
        vault: r.try_get("vault_id")?,
        host: r.try_get("host")?,
        has_authorization: r.try_get("has_authorization")?,
        enabled: r.try_get("enabled")?,
        disabled_reason: r.try_get("disabled_reason")?,
        consecutive_failures: r.try_get("consecutive_failures")?,
        created_by: r.try_get("created_by")?,
        last: last_at.zip(last_result).map(|(at, result)| LastDelivery {
            at: rfc3339(at),
            result,
        }),
    })
}

async fn one(state: &SharedState, scope: &Scope, id: Uuid) -> Result<RuleView, ApiError> {
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    let row = sqlx::query(concat!(rule_select!(), " WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    let rule = row
        .as_ref()
        .map(view)
        .transpose()?
        .ok_or(ApiError::of(ProblemType::NotFound))?;
    // A manager does not learn of a rule outside their vaults (T5).
    if !scope.sees(rule.vault) {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    Ok(rule)
}

/// A tenant's rules: all of them for an admin, a manager's vaults' for a manager.
#[utoipa::path(
    get,
    path = "/tenants/{t}/notifications",
    tag = "notifications",
    params(("t" = String, Path, description = "Tenant slug"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Rules, newest first", body = Vec<RuleView>),
        (status = 403, description = "Neither a tenant admin nor a vault manager", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such tenant", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "notifications_disabled", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn list_rules(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(t): Path<String>,
) -> Result<Json<Vec<RuleView>>, ApiError> {
    let scope = authorize(&state, &caller, &t).await?;
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    let rows = sqlx::query(concat!(
        rule_select!(),
        " ORDER BY r.created_at DESC LIMIT 100"
    ))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let rules = rows.iter().map(view).collect::<Result<Vec<_>, _>>()?;
    Ok(Json(
        rules.into_iter().filter(|r| scope.sees(r.vault)).collect(),
    ))
}

/// Create a rule; the signing secret is in this answer only.
#[utoipa::path(
    post,
    path = "/tenants/{t}/notifications",
    tag = "notifications",
    params(("t" = String, Path, description = "Tenant slug"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = CreateRule,
    responses(
        (status = 201, description = "Created, with its secret, once", body = SavedRule),
        (status = 400, description = "A URL that cannot be used, an unknown event, more than 20 rules", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "A tenant event or a rule without a vault, asked by a manager", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "notifications_disabled", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn create_rule(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(t): Path<String>,
    Body(body): Body<CreateRule>,
) -> Result<(StatusCode, Json<SavedRule>), ApiError> {
    let scope = authorize(&state, &caller, &t).await?;
    let name = check_name(&body.name)?;
    check_events(&scope, &body.events, body.vault)?;
    if body.channel.kind != "webhook" {
        return Err(invalid("channel.type: webhook"));
    }
    if let Some(a) = &body.channel.authorization {
        check_authorization(a)?;
    }
    let (target, host) = check_url(&state, &body.channel.url).await?;
    let key = tenant_key(&state, &scope).await?;
    let id = Uuid::now_v7();
    let (secret, shown) = new_secret()?;
    let sub = &caller.caller.principal.sub;
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('kura.notifications:' || $1::text))")
        .bind(scope.tenant)
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM notification_rule")
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_RULES {
        return Err(invalid("at most 20 rules in a tenant"));
    }
    if let Some(v) = body.vault {
        let known: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM vault WHERE id = $1 AND disabled_at IS NULL)",
        )
        .bind(v)
        .fetch_one(&mut *tx)
        .await?;
        if !known {
            return Err(ApiError::of(ProblemType::NotFound));
        }
    }
    let authorization = body
        .channel
        .authorization
        .as_deref()
        .filter(|a| !a.is_empty())
        .map(|a| seal(&key, id, "authorization", a.as_bytes()))
        .transpose()?;
    sqlx::query(
        "INSERT INTO notification_rule (id, tenant_id, vault_id, name, events, host, sealed_url, sealed_secret, sealed_authorization, enabled, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(id)
    .bind(scope.tenant)
    .bind(body.vault)
    .bind(&name)
    .bind(&body.events)
    .bind(&host)
    .bind(seal(&key, id, "url", target.as_str().as_bytes())?)
    .bind(seal(&key, id, "secret", &secret)?)
    .bind(authorization)
    .bind(body.enabled)
    .bind(sub)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if let Some(n) = &state.notifier
        && body.enabled
    {
        n.subscribe(scope.tenant, &body.events);
    }
    tracing::info!(target: "audit", event = "notification_rule_created", %sub, tenant = %scope.slug, rule = %id, %host);
    Ok((
        StatusCode::CREATED,
        Json(SavedRule {
            rule: one(&state, &scope, id).await?,
            secret: Some(shown),
        }),
    ))
}

/// Change a rule; a re-enabled rule forgets its failures.
#[utoipa::path(
    put,
    path = "/tenants/{t}/notifications/{n}",
    tag = "notifications",
    params(("t" = String, Path, description = "Tenant slug"), ("n" = Uuid, Path, description = "Rule id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = UpdateRule,
    responses(
        (status = 200, description = "Saved; `secret` only with rotateSecret", body = SavedRule),
        (status = 400, description = "A URL that cannot be used, an unknown event", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such rule here", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "notifications_disabled", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn update_rule(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((t, n)): Path<(String, Uuid)>,
    Body(body): Body<UpdateRule>,
) -> Result<Json<SavedRule>, ApiError> {
    let scope = authorize(&state, &caller, &t).await?;
    let current = one(&state, &scope, n).await?;
    let name = body.name.as_deref().map(check_name).transpose()?;
    if let Some(events) = &body.events {
        check_events(&scope, events, current.vault)?;
    }
    if let Some(a) = &body.authorization {
        check_authorization(a)?;
    }
    let url = match &body.url {
        Some(u) => Some(check_url(&state, u).await?),
        None => None,
    };
    let needs_key = url.is_some()
        || body.rotate_secret
        || body.authorization.as_deref().is_some_and(|a| !a.is_empty());
    let key = if needs_key {
        Some(tenant_key(&state, &scope).await?)
    } else {
        None
    };
    let sealed = |field: &str, v: &[u8]| -> Result<Option<Vec<u8>>, ApiError> {
        key.as_ref().map(|k| seal(k, n, field, v)).transpose()
    };
    let secret = if body.rotate_secret {
        Some(new_secret()?)
    } else {
        None
    };
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    sqlx::query(
        "UPDATE notification_rule SET
            name = coalesce($2, name), events = coalesce($3, events),
            host = coalesce($4, host), sealed_url = coalesce($5, sealed_url),
            sealed_secret = coalesce($6, sealed_secret),
            sealed_authorization = CASE WHEN $7 THEN $8 ELSE sealed_authorization END,
            enabled = coalesce($9, enabled),
            consecutive_failures = CASE WHEN $9 THEN 0 ELSE consecutive_failures END,
            disabled_reason = CASE WHEN $9 THEN NULL ELSE disabled_reason END,
            updated_at = now()
         WHERE id = $1",
    )
    .bind(n)
    .bind(name)
    .bind(&body.events)
    .bind(url.as_ref().map(|(_, h)| h.clone()))
    .bind(match &url {
        Some((target, _)) => sealed("url", target.as_str().as_bytes())?,
        None => None,
    })
    .bind(match &secret {
        Some((raw, _)) => sealed("secret", raw)?,
        None => None,
    })
    .bind(body.authorization.is_some())
    .bind(match body.authorization.as_deref() {
        Some(a) if !a.is_empty() => sealed("authorization", a.as_bytes())?,
        _ => None,
    })
    .bind(body.enabled)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let rule = one(&state, &scope, n).await?;
    if let Some(nt) = &state.notifier
        && rule.enabled
    {
        nt.subscribe(scope.tenant, &rule.events);
    }
    tracing::info!(target: "audit", event = "notification_rule_changed", sub = %caller.caller.principal.sub, tenant = %scope.slug, rule = %n, host = %rule.host);
    Ok(Json(SavedRule {
        rule,
        secret: secret.map(|(_, shown)| shown),
    }))
}

/// Delete a rule and its deliveries.
#[utoipa::path(
    delete,
    path = "/tenants/{t}/notifications/{n}",
    tag = "notifications",
    params(("t" = String, Path, description = "Tenant slug"), ("n" = Uuid, Path, description = "Rule id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "No such rule here", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "notifications_disabled", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn delete_rule(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((t, n)): Path<(String, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let scope = authorize(&state, &caller, &t).await?;
    one(&state, &scope, n).await?;
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    sqlx::query("DELETE FROM notification_rule WHERE id = $1")
        .bind(n)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::info!(target: "audit", event = "notification_rule_deleted", sub = %caller.caller.principal.sub, tenant = %scope.slug, rule = %n);
    Ok(StatusCode::NO_CONTENT)
}

/// Send `ping` to this rule now; the answer is the attempt's coarse result.
#[utoipa::path(
    post,
    path = "/tenants/{t}/notifications/{n}/test",
    tag = "notifications",
    params(("t" = String, Path, description = "Tenant slug"), ("n" = Uuid, Path, description = "Rule id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Attempted once", body = TestResult),
        (status = 404, description = "No such rule here", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "notifications_disabled", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn test_rule(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((t, n)): Path<(String, Uuid)>,
) -> Result<Json<TestResult>, ApiError> {
    let scope = authorize(&state, &caller, &t).await?;
    let rule = one(&state, &scope, n).await?;
    let vault_name = rule.vault.and_then(|v| {
        scope
            .vaults
            .as_ref()
            .and_then(|vs| vs.iter().find(|(id, _)| *id == v))
            .map(|(_, nm)| nm.clone())
    });
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    let event = notifier::insert(
        &mut tx,
        Fact {
            tenant: scope.tenant,
            tenant_slug: &scope.slug,
            vault: None,
            kind: "ping",
            title: format!("Test of the rule {}", rule.name),
            message: format!("Kura can reach this webhook for tenant {}.", scope.slug),
            data: serde_json::json!({ "rule": { "id": n, "name": rule.name }, "vaultName": vault_name }),
            dedupe: None,
        },
        Some(n),
    )
    .await?
    .ok_or(ApiError::Unavailable("test event not recorded".into()))?;
    let delivery: Uuid =
        sqlx::query_scalar("SELECT id FROM notification_delivery WHERE event_id = $1")
            .bind(event)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    let outcome = notifier::deliver(&state, delivery, scope.tenant, &scope.slug).await?;
    tracing::info!(target: "audit", event = "notification_rule_tested", sub = %caller.caller.principal.sub, tenant = %scope.slug, rule = %n, result = %outcome.result());
    Ok(Json(TestResult {
        result: outcome.result(),
    }))
}

/// The rule's last 50 deliveries, newest first (kept 7 days).
#[utoipa::path(
    get,
    path = "/tenants/{t}/notifications/{n}/deliveries",
    tag = "notifications",
    params(("t" = String, Path, description = "Tenant slug"), ("n" = Uuid, Path, description = "Rule id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Deliveries", body = Vec<DeliveryView>),
        (status = 404, description = "No such rule here", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "notifications_disabled", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn list_deliveries(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((t, n)): Path<(String, Uuid)>,
) -> Result<Json<Vec<DeliveryView>>, ApiError> {
    let scope = authorize(&state, &caller, &t).await?;
    one(&state, &scope, n).await?;
    let mut tx = db::begin_tenant(&state.db, scope.tenant, None).await?;
    let rows = sqlx::query(
        "SELECT d.id, e.type, d.updated_at, d.attempts, d.status, d.result FROM notification_delivery d
         JOIN notification_event e ON e.id = d.event_id WHERE d.rule_id = $1 ORDER BY d.updated_at DESC LIMIT 50",
    )
    .bind(n)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    rows.iter()
        .map(|r| {
            Ok(DeliveryView {
                id: r.try_get("id")?,
                event: r.try_get("type")?,
                at: rfc3339(r.try_get("updated_at")?),
                attempts: r.try_get("attempts")?,
                status: r.try_get("status")?,
                result: r.try_get("result")?,
            })
        })
        .collect::<Result<_, ApiError>>()
        .map(Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T4: the value is sent verbatim, so CR, LF, NUL and every other
    /// control character are refused at input, as is a value over 1 KiB.
    #[test]
    fn authorization_rejects_control_characters() {
        assert!(check_authorization("Bearer test-key").is_ok());
        assert!(check_authorization(&"a".repeat(1024)).is_ok());
        for bad in [
            "Bearer x\r\nX-Injected: 1",
            "Bearer x\nX-Injected: 1",
            "Bearer x\r",
            "Bearer \0x",
            "Bearer \tx",
            "Bearer \u{7f}x",
            "Bearer \u{85}x",
        ] {
            assert!(check_authorization(bad).is_err(), "{bad:?}");
        }
        assert!(check_authorization(&"a".repeat(1025)).is_err());
    }
}
