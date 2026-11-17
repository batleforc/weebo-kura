//! Outbound notifications (RFC 0003 §5.3, §6.4): a fact of a tenant becomes
//! an event, in the fact's own transaction, only when an enabled rule
//! subscribes to its type; each matching rule gets a delivery; the worker
//! signs and POSTs it through the guarded client, retries with backoff, and
//! switches a rule off after 20 failures. Not to be confused with `notify`,
//! the vault event channel of devices.

use std::collections::HashSet;
use std::sync::RwLock;
use std::time::Duration;

use aws_lc_rs::hmac;
use ipnet::IpNet;
use serde_json::{Value, json};
use sqlx::Row as _;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::app::SharedState;
use crate::db;
use crate::problem::ApiError;
use crate::store::crypto;

/// The closed list of event types (§4.2).
pub const EVENTS: [&str; 14] = [
    "token.created",
    "token.revoked",
    "token.expiring",
    "token.denied",
    "binding.changed",
    "binding.removed",
    "device.registered",
    "device.revoked",
    "vault.created",
    "vault.deleted",
    "vault.changed",
    "vault.conflict",
    "quota.high",
    // The Test button: recorded for that one rule only.
    "ping",
];

/// Events a vault manager may not subscribe to: they concern the tenant.
pub const TENANT_EVENTS: [&str; 5] = [
    "device.registered",
    "device.revoked",
    "quota.high",
    "vault.created",
    "vault.deleted",
];

/// Waits after the 1st to 5th failed attempt; the 6th failure is final (§4.2).
const BACKOFF: [Duration; 5] = [
    Duration::from_secs(60),
    Duration::from_secs(300),
    Duration::from_secs(1_800),
    Duration::from_secs(7_200),
    Duration::from_secs(21_600),
];
/// A rule switches itself off after this many failed deliveries in a row.
pub const MAX_FAILURES: i32 = 20;
/// Deliveries in flight on a replica (§6.4, T6).
const IN_FLIGHT: i32 = 4;
const SUBSCRIPTIONS_EVERY: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_secs(2);

/// The notifier of a replica: its guarded client and subscription cache.
pub struct Notifier {
    pub client: reqwest::Client,
    pub allowed: Vec<IpNet>,
    subscriptions: RwLock<HashSet<(Uuid, String)>>,
}

impl Notifier {
    pub fn new(client: reqwest::Client, allowed: Vec<IpNet>) -> Self {
        Self {
            client,
            allowed,
            subscriptions: RwLock::new(HashSet::new()),
        }
    }

    /// Whether a rule of the tenant listens to this type (cached, 30 s).
    pub fn subscribed(&self, tenant: Uuid, kind: &str) -> bool {
        self.subscriptions
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(&(tenant, kind.to_owned()))
    }

    /// Re-reads the (tenant, type) pairs with an enabled rule.
    pub async fn refresh(&self, pool: &sqlx::PgPool) -> Result<(), sqlx::Error> {
        let rows: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT tenant_id, type FROM kura_notification_subscriptions()")
                .fetch_all(pool)
                .await?;
        *self
            .subscriptions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = rows.into_iter().collect();
        Ok(())
    }

    /// Marks a pair subscribed at once on this replica (a rule just saved).
    pub fn subscribe(&self, tenant: Uuid, events: &[String]) {
        let mut s = self
            .subscriptions
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for e in events {
            s.insert((tenant, e.clone()));
        }
    }
}

/// A fact to record: its tenant, maybe its vault, its type and what the
/// payload says. Names and ids only, never a note path, an OIDC subject, a
/// device name or a secret (§4.2, T5).
pub struct Fact<'a> {
    pub tenant: Uuid,
    pub tenant_slug: &'a str,
    pub vault: Option<(Uuid, &'a str)>,
    pub kind: &'static str,
    pub title: String,
    pub message: String,
    pub data: Value,
    /// Facts of one window count in one event (`vault.changed`, `token.denied`).
    pub dedupe: Option<String>,
}

impl<'a> Fact<'a> {
    /// A fact about one vault (most of them).
    pub fn vault(
        a: &'a crate::authz::VaultAccess,
        kind: &'static str,
        title: String,
        message: String,
        data: Value,
    ) -> Self {
        Self {
            tenant: a.tenant,
            tenant_slug: &a.tenant_slug,
            vault: Some((a.vault, &a.name)),
            kind,
            title,
            message,
            data,
            dedupe: None,
        }
    }
}

/// Records `fact` in `tx` when a rule of its tenant subscribes to its type;
/// otherwise nothing, not even a row (§4.2, the uninteresting case).
pub async fn record(
    state: &SharedState,
    tx: &mut sqlx::PgConnection,
    fact: Fact<'_>,
) -> Result<(), ApiError> {
    let Some(n) = &state.notifier else {
        return Ok(());
    };
    if !n.subscribed(fact.tenant, fact.kind) {
        return Ok(());
    }
    insert(tx, fact, None).await.map(drop)
}

/// The event and its deliveries; `only` restricts them to one rule (a test).
/// Returns the event's id when it is new, `None` when it counted in an
/// existing one of its window.
pub async fn insert(
    tx: &mut sqlx::PgConnection,
    fact: Fact<'_>,
    only: Option<Uuid>,
) -> Result<Option<Uuid>, ApiError> {
    let id = Uuid::now_v7();
    let time = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let mut data = fact.data;
    if fact.dedupe.is_some()
        && let Some(o) = data.as_object_mut()
    {
        o.insert("count".into(), json!(1));
    }
    let payload = json!({
        "id": id,
        "type": fact.kind,
        "time": time,
        "tenant": fact.tenant_slug,
        "vault": fact.vault.map(|(v, name)| json!({ "id": v, "name": name })),
        "title": fact.title,
        "message": fact.message,
        "data": data,
    });
    let fresh: Option<bool> = sqlx::query_scalar(
        "INSERT INTO notification_event (id, tenant_id, vault_id, type, payload, dedupe) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (tenant_id, dedupe) DO UPDATE
             SET payload = jsonb_set(notification_event.payload, '{data,count}',
                 to_jsonb(coalesce((notification_event.payload #>> '{data,count}')::int, 1) + 1))
         RETURNING xmax = 0",
    )
    .bind(id)
    .bind(fact.tenant)
    .bind(fact.vault.map(|v| v.0))
    .bind(fact.kind)
    .bind(&payload)
    .bind(&fact.dedupe)
    .fetch_optional(&mut *tx)
    .await?;
    if fresh != Some(true) {
        return Ok(None);
    }
    sqlx::query(
        "INSERT INTO notification_delivery (id, tenant_id, event_id, rule_id)
         SELECT gen_random_uuid(), $1, $2, r.id FROM notification_rule r
         WHERE r.tenant_id = $1 AND ($5::uuid IS NOT NULL AND r.id = $5
                OR $5::uuid IS NULL AND r.enabled AND $3 = ANY(r.events) AND (r.vault_id IS NULL OR r.vault_id = $4))",
    )
    .bind(fact.tenant)
    .bind(id)
    .bind(fact.kind)
    .bind(fact.vault.map(|v| v.0))
    .bind(only)
    .execute(&mut *tx)
    .await?;
    Ok(Some(id))
}

/// `v1=<hex HMAC-SHA256(secret, timestamp + "." + body)>` (§4.1, T3).
pub fn signature(secret: &[u8], timestamp: i64, body: &[u8]) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    let mut ctx = hmac::Context::with_key(&key);
    ctx.update(timestamp.to_string().as_bytes());
    ctx.update(b".");
    ctx.update(body);
    let tag = ctx.sign();
    let hex: String = tag.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    format!("v1={hex}")
}

/// The context a rule's sealed field is bound to (§7.4).
pub fn context(rule: Uuid, field: &str) -> Vec<u8> {
    format!("kura notification {rule} {field}").into_bytes()
}

/// What one attempt came to: coarse, never a response body (§4.5, T2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Delivered(u16),
    /// 410 Gone: the receiver wants no more; the rule switches off.
    Gone,
    Failed(&'static str),
    /// Never retried: the address is refused, or the rule cannot be read.
    Refused(&'static str),
}

impl Outcome {
    pub fn result(&self) -> String {
        match self {
            Self::Delivered(s) => s.to_string(),
            Self::Gone => "410".into(),
            Self::Failed(r) | Self::Refused(r) => (*r).into(),
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Delivered(_) => "delivered",
            Self::Gone => "gone",
            Self::Failed(r) | Self::Refused(r) => r,
        }
    }
}

fn classify(e: &reqwest::Error) -> &'static str {
    let mut source: Option<&dyn std::error::Error> = Some(e);
    while let Some(s) = source {
        if s.is::<crate::net::AddressRefused>() {
            return "address_refused";
        }
        let text = s.to_string().to_lowercase();
        if text.contains("certificate") || text.contains("tls") || text.contains("handshake") {
            return "tls";
        }
        source = s.source();
    }
    if e.is_timeout() { "timeout" } else { "connect" }
}

/// One signed POST of `body` to the rule's URL (§4.1).
pub async fn post(
    n: &Notifier,
    url: &str,
    secret: &[u8],
    authorization: Option<&str>,
    kind: &str,
    delivery: Uuid,
    body: Vec<u8>,
) -> Outcome {
    // A literal is checked here: the resolver never sees it (§6.5).
    let Ok(target) =
        kura_core::webhook::Target::parse(url, |ip| crate::net::webhook_allowed(ip, &n.allowed))
    else {
        return Outcome::Refused("address_refused");
    };
    let timestamp = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut request = n
        .client
        .post(target.as_str())
        .header("content-type", "application/json")
        .header("x-kura-event", kind)
        .header("x-kura-delivery", delivery.to_string())
        .header("x-kura-timestamp", timestamp.to_string())
        .header("x-kura-signature", signature(secret, timestamp, &body));
    if let Some(a) = authorization
        && let Ok(mut value) = reqwest::header::HeaderValue::from_str(a)
    {
        value.set_sensitive(true);
        request = request.header(reqwest::header::AUTHORIZATION, value);
    }
    match request.body(body).send().await {
        Ok(r) => {
            let status = r.status().as_u16();
            // The body is never read (T2).
            drop(r);
            match status {
                200..=299 => Outcome::Delivered(status),
                410 => Outcome::Gone,
                300..=399 => Outcome::Failed("redirect"),
                400..=499 => Outcome::Failed("status_4xx"),
                _ => Outcome::Failed("status_5xx"),
            }
        }
        Err(e) => match classify(&e) {
            "address_refused" => Outcome::Refused("address_refused"),
            other => Outcome::Failed(other),
        },
    }
}

/// The plaintext URL, secret and `authorization` of a rule.
pub struct Opened {
    pub url: String,
    pub secret: Vec<u8>,
    pub authorization: Option<String>,
}

pub async fn open_rule(
    state: &SharedState,
    tenant: Uuid,
    slug: &str,
    rule: Uuid,
    row: &sqlx::postgres::PgRow,
) -> Result<Opened, ApiError> {
    let keys = &state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::Unavailable("no storage keys for notification secrets".into()))?
        .keys;
    let key = keys.notifications(&state.db, tenant, slug).await?;
    let open = |field: &str, sealed: &[u8]| {
        crypto::open_value(&key, &context(rule, field), sealed)
            .map_err(|e| ApiError::Unavailable(e.to_string()))
    };
    let url = open("url", &row.try_get::<Vec<u8>, _>("sealed_url")?)?;
    let secret = open("secret", &row.try_get::<Vec<u8>, _>("sealed_secret")?)?;
    let authorization = match row.try_get::<Option<Vec<u8>>, _>("sealed_authorization")? {
        Some(s) => Some(String::from_utf8_lossy(&open("authorization", &s)?).into_owned()),
        None => None,
    };
    Ok(Opened {
        url: String::from_utf8_lossy(&url).into_owned(),
        secret: secret.to_vec(),
        authorization,
    })
}

/// Attempts one claimed delivery and records what came of it.
pub async fn deliver(
    state: &SharedState,
    id: Uuid,
    tenant: Uuid,
    slug: &str,
) -> Result<Outcome, ApiError> {
    let Some(n) = &state.notifier else {
        return Ok(Outcome::Refused("disabled"));
    };
    let mut tx = db::begin_tenant(&state.db, tenant, None).await?;
    let row = sqlx::query(
        "SELECT d.attempts, d.rule_id, e.type, e.payload, r.enabled, r.sealed_url, r.sealed_secret, r.sealed_authorization
         FROM notification_delivery d JOIN notification_event e ON e.id = d.event_id JOIN notification_rule r ON r.id = d.rule_id
         WHERE d.id = $1 AND d.status = 'pending'",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else {
        return Ok(Outcome::Refused("gone"));
    };
    let rule: Uuid = row.try_get("rule_id")?;
    let attempts: i32 = row.try_get::<i32, _>("attempts")?.saturating_add(1);
    let outcome =
        if row.try_get::<bool, _>("enabled")? || row.try_get::<String, _>("type")? == "ping" {
            match open_rule(state, tenant, slug, rule, &row).await {
                Ok(o) => {
                    let kind: String = row.try_get("type")?;
                    let body = serde_json::to_vec(&row.try_get::<Value, _>("payload")?)
                        .unwrap_or_default();
                    post(
                        n,
                        &o.url,
                        &o.secret,
                        o.authorization.as_deref(),
                        &kind,
                        id,
                        body,
                    )
                    .await
                }
                // Nothing is ever sent unsigned (§10.2): retried later.
                Err(e) => {
                    tracing::warn!(error = %e, "notification secrets unavailable; retried");
                    Outcome::Failed("unavailable")
                }
            }
        } else {
            Outcome::Refused("rule_disabled")
        };
    settle(state, tenant, id, rule, attempts, &outcome).await?;
    state
        .metrics
        .notification_deliveries_total
        .with_label_values(&[outcome.label()])
        .inc();
    Ok(outcome)
}

async fn settle(
    state: &SharedState,
    tenant: Uuid,
    id: Uuid,
    rule: Uuid,
    attempts: i32,
    outcome: &Outcome,
) -> Result<(), ApiError> {
    let mut tx = db::begin_tenant(&state.db, tenant, None).await?;
    let (status, next) = match outcome {
        Outcome::Delivered(_) => ("delivered", None),
        Outcome::Gone | Outcome::Refused(_) => ("failed", None),
        Outcome::Failed(_) => match usize::try_from(attempts.saturating_sub(1))
            .ok()
            .and_then(|i| BACKOFF.get(i))
        {
            Some(wait) => ("pending", Some(*wait)),
            None => ("failed", None),
        },
    };
    sqlx::query(
        "UPDATE notification_delivery SET attempts = $2, status = $3, result = $4, claimed_at = NULL,
             next_attempt_at = now() + make_interval(secs => $5), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(attempts)
    .bind(status)
    .bind(outcome.result())
    .bind(next.map_or(0.0, |d| d.as_secs_f64()))
    .execute(&mut *tx)
    .await?;
    let disabled: Option<String> = match outcome {
        Outcome::Delivered(_) => {
            sqlx::query("UPDATE notification_rule SET consecutive_failures = 0 WHERE id = $1")
                .bind(rule)
                .execute(&mut *tx)
                .await?;
            None
        }
        Outcome::Gone => {
            sqlx::query("UPDATE notification_rule SET enabled = false, disabled_reason = 'gone' WHERE id = $1 AND enabled")
                .bind(rule)
                .execute(&mut *tx)
                .await?;
            Some("gone".into())
        }
        Outcome::Failed(r) | Outcome::Refused(r) if *r != "rule_disabled" && *r != "gone" => {
            sqlx::query_scalar(
                "UPDATE notification_rule SET consecutive_failures = consecutive_failures + 1,
                     enabled = enabled AND consecutive_failures + 1 < $2,
                     disabled_reason = CASE WHEN enabled AND consecutive_failures + 1 >= $2 THEN $3 ELSE disabled_reason END
                 WHERE id = $1 RETURNING CASE WHEN NOT enabled AND disabled_reason = $3 THEN $3 END",
            )
            .bind(rule)
            .bind(MAX_FAILURES)
            .bind(format!("failures: {r}"))
            .fetch_optional(&mut *tx)
            .await?
            .flatten()
        }
        _ => None,
    };
    tx.commit().await?;
    if let Some(reason) = disabled {
        tracing::warn!(target: "audit", event = "notification_rule_disabled", rule = %rule, %reason);
    }
    Ok(())
}

/// `quota.high` once per crossing of 90% of the quota (§4.2), for tenants a
/// rule listens to it in; the flag is in the database, so one replica wins.
async fn quota_check(state: &SharedState, n: &Notifier) -> Result<(), ApiError> {
    let Some(snapshot) = state.snapshot() else {
        return Ok(());
    };
    let rows: Vec<(String, i64, Option<i64>)> =
        sqlx::query_as("SELECT slug, stored, quota FROM kura_tenant_storage()")
            .fetch_all(&state.db)
            .await?;
    for (slug, stored, quota) in rows {
        let Some(&tenant) = snapshot.tenant_ids.get(slug.as_str()) else {
            continue;
        };
        let Some(quota) = quota.filter(|q| *q > 0) else {
            continue;
        };
        if !n.subscribed(tenant, "quota.high") {
            continue;
        }
        let high = stored.saturating_mul(10) >= quota.saturating_mul(9);
        let mut tx = db::begin_tenant(&state.db, tenant, None).await?;
        let crossed: Option<bool> = sqlx::query_scalar(
            "INSERT INTO notification_tenant (tenant_id, quota_high) VALUES ($1, $2)
             ON CONFLICT (tenant_id) DO UPDATE SET quota_high = EXCLUDED.quota_high
             WHERE notification_tenant.quota_high IS DISTINCT FROM EXCLUDED.quota_high
             RETURNING quota_high",
        )
        .bind(tenant)
        .bind(high)
        .fetch_optional(&mut *tx)
        .await?;
        if crossed == Some(true) {
            let percent = stored.saturating_mul(100).checked_div(quota).unwrap_or(100);
            record(
                state,
                &mut tx,
                Fact {
                    tenant,
                    tenant_slug: &slug,
                    vault: None,
                    kind: "quota.high",
                    title: format!("Tenant {slug} is at {percent}% of its quota"),
                    message: format!(
                        "{stored} of {quota} bytes stored; writes are refused at 100%."
                    ),
                    data: json!({ "usedBytes": stored, "quotaBytes": quota }),
                    dedupe: None,
                },
            )
            .await?;
        }
        tx.commit().await?;
    }
    Ok(())
}

/// `token.expiring`, once per token, 7 days before (§4.2); and the 7-day
/// purge of events and deliveries.
async fn hourly(state: &SharedState) -> Result<(), ApiError> {
    let Some(snapshot) = state.snapshot() else {
        return Ok(());
    };
    let rows: Vec<(Uuid, Uuid, Uuid, String, time::OffsetDateTime)> = sqlx::query_as(
        "SELECT tenant_id, vault_id, id, name, expires_at FROM kura_tokens_expiring()",
    )
    .fetch_all(&state.db)
    .await?;
    for (tenant, vault, token, name, expires) in rows {
        let Some(a) = crate::authz::key_access(
            &state.db,
            &snapshot,
            tenant,
            vault,
            kura_api::VaultRole::Viewer,
        )
        .await?
        else {
            continue;
        };
        let at = expires
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        let mut tx = db::begin_tenant(&state.db, tenant, None).await?;
        record(
            state,
            &mut tx,
            Fact::vault(
                &a,
                "token.expiring",
                format!("Token {name} expires in 7 days or less"),
                format!(
                    "Vault {}, tenant {}: renew or replace it.",
                    a.name, a.tenant_slug
                ),
                json!({ "token": { "id": token, "name": name }, "expiresAt": at }),
            ),
        )
        .await?;
        tx.commit().await?;
    }
    let purged: i64 = sqlx::query_scalar("SELECT kura_notification_purge()")
        .fetch_one(&state.db)
        .await?;
    if purged > 0 {
        tracing::info!(purged, "old notification events purged");
    }
    Ok(())
}

/// The worker (§6.4): the subscription cache every 30 s, deliveries due
/// every 2 s, four in flight, the gauges of §10.1; until `shutdown`.
pub async fn run(state: SharedState, shutdown: CancellationToken) {
    let Some(n) = state.notifier.clone() else {
        return;
    };
    let mut refreshed = tokio::time::Instant::now()
        .checked_sub(SUBSCRIPTIONS_EVERY)
        .unwrap_or_else(tokio::time::Instant::now);
    let pending = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let mut last_hourly = refreshed
        .checked_sub(Duration::from_secs(3_600))
        .unwrap_or(refreshed);
    loop {
        if refreshed.elapsed() >= SUBSCRIPTIONS_EVERY {
            if let Err(e) = n.refresh(&state.db).await {
                tracing::warn!(error = %e, "notification subscriptions not refreshed");
            }
            if let Ok(row) = sqlx::query(
                "SELECT pending, oldest_seconds, disabled FROM kura_notification_stats()",
            )
            .fetch_one(&state.db)
            .await
            {
                let m = &state.metrics;
                m.notification_pending
                    .set(row.try_get("pending").unwrap_or_default());
                m.notification_oldest_pending_seconds
                    .set(row.try_get("oldest_seconds").unwrap_or_default());
                m.notification_rules_disabled
                    .set(row.try_get("disabled").unwrap_or_default());
            }
            if let Err(e) = quota_check(&state, &n).await {
                tracing::warn!(error = %e, "quota.high not checked");
            }
            refreshed = tokio::time::Instant::now();
        }
        if last_hourly.elapsed() >= Duration::from_secs(3_600) {
            if let Err(e) = hourly(&state).await {
                tracing::warn!(error = %e, "token.expiring and the notification purge not run");
            }
            last_hourly = tokio::time::Instant::now();
        }
        let free = i32::try_from(pending.available_permits())
            .unwrap_or(0)
            .min(IN_FLIGHT);
        if free > 0 {
            let claimed: Vec<(Uuid, Uuid, String)> =
                sqlx::query_as("SELECT id, tenant_id, slug FROM kura_notification_claim($1)")
                    .bind(free)
                    .fetch_all(&state.db)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "deliveries not claimed");
                        Vec::new()
                    });
            for (id, tenant, slug) in claimed {
                let Ok(permit) = pending.clone().acquire_owned().await else {
                    break;
                };
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = deliver(&state, id, tenant, &slug).await {
                        tracing::warn!(error = %e, "delivery not settled; taken over after 2 minutes");
                    }
                    drop(permit);
                });
            }
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(POLL) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_is_hmac_sha256_of_timestamp_dot_body() {
        // Recomputed by `openssl dgst -sha256 -hmac secret` on "1763210000.{}".
        assert_eq!(
            signature(b"secret", 1_763_210_000, b"{}"),
            format!("v1={}", {
                let key = hmac::Key::new(hmac::HMAC_SHA256, b"secret");
                hmac::sign(&key, b"1763210000.{}")
                    .as_ref()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            })
        );
        assert_ne!(
            signature(b"secret", 1, b"{}"),
            signature(b"secret", 2, b"{}")
        );
    }

    #[test]
    fn the_lists_are_closed_and_consistent() {
        assert!(TENANT_EVENTS.iter().all(|e| EVENTS.contains(e)));
        assert_eq!(Outcome::Failed("status_5xx").result(), "status_5xx");
        assert_eq!(Outcome::Delivered(204).result(), "204");
    }
}
