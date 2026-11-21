//! `POST /v1/vaults/{v}/events/ticket` and `GET /v1/vaults/{v}/events`
//! (RFC 0001 §6.3, decision 36, T11): a web view cannot set an
//! `Authorization` header on a WebSocket, so it trades its token for a
//! single-use 30-second ticket, then opens the socket with it.

use std::time::Duration;

use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::Response;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use kura_api::VaultRole;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use sqlx::Row as _;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::SharedState;
use super::device::DeviceCaller;
use crate::authz;
use crate::db;
use crate::notify::Event;
use crate::problem::{ApiError, Problem, ProblemType};

/// A ticket's validity (§6.3).
pub const TICKET_TTL: Duration = Duration::from_secs(30);
/// The ping carrying `head_seq` (§6.3).
pub const PING: Duration = Duration::from_secs(30);

/// Close codes the client reads (4000–4999 are the application's).
pub mod close {
    pub const TICKET_USED: u16 = 4003;
    pub const TICKET_FOR_OTHER_VAULT: u16 = 4004;
    pub const TOKEN_EXPIRED: u16 = 4001;
    pub const DEVICE_REVOKED: u16 = 4002;
    /// The caller no longer reads the vault (plan V4).
    pub const ACCESS_REVOKED: u16 = 4005;
    pub const TOO_MANY_SOCKETS: u16 = 4008;
    pub const SLOW_CONSUMER: u16 = 4009;
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ticket {
    /// Single use, valid `expiresIn` seconds; send as `?ticket=`.
    pub ticket: String,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventsQuery {
    /// The ticket from `POST events/ticket`.
    pub ticket: String,
}

/// What a socket sends, as JSON text frames.
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(
    tag = "type",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Notice {
    /// The vault's head moved: pull.
    Change { seq: u64 },
    /// Notifications may have been lost: pull.
    Pull,
    /// Every 30 s; a client that misses one treats the socket as down.
    Ping { head_seq: u64 },
}

fn hash(ticket: &[u8]) -> Vec<u8> {
    sha2::Sha256::digest(ticket).to_vec()
}

/// A ticket for this device and vault.
#[utoipa::path(
    post,
    path = "/vaults/{v}/events/ticket",
    tag = "sync",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "A single-use ticket", body = Ticket),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn create_ticket(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
) -> Result<Json<Ticket>, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let p = &caller.caller.principal;
    let a = authz::require_role(&state.db, &snap, p, v, VaultRole::Viewer).await?;
    let mut secret = [0u8; 32];
    aws_lc_rs::rand::fill(&mut secret)
        .map_err(|_| ApiError::Unavailable("no randomness".into()))?;
    let token_expires =
        time::OffsetDateTime::from_unix_timestamp(i64::try_from(p.exp).unwrap_or(i64::MAX))
            .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let ttl = i64::try_from(TICKET_TTL.as_secs()).unwrap_or(30);
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    sqlx::query(
        "INSERT INTO ws_ticket (ticket_hash, tenant_id, vault_id, device_id, token_expires_at, expires_at, claims)
         VALUES ($1, $2, $3, $4, $5, now() + make_interval(secs => $6), $7)",
    )
    .bind(hash(&secret))
    .bind(a.tenant)
    .bind(v)
    .bind(caller.device)
    .bind(token_expires)
    .bind(ttl)
    .bind(sqlx::types::Json(&p.claims))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(Ticket {
        ticket: URL_SAFE_NO_PAD.encode(secret),
        expires_in: TICKET_TTL.as_secs(),
    }))
}

/// The vault's notifications, over a WebSocket opened with a ticket.
#[utoipa::path(
    get,
    path = "/vaults/{v}/events",
    tag = "sync",
    params(("v" = Uuid, Path, description = "Vault id"), EventsQuery),
    responses(
        (status = 101, description = "WebSocket: JSON `change`, `pull` and `ping` notices; closed 4003 for a used, expired or unknown ticket, 4004 for another vault's, 4001 when the token behind it expires, 4002 on revocation, 4005 when the caller no longer reads the vault, 4008 past 4 sockets, 4009 when too slow"),
        (status = 400, description = "No ticket", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Over 60 attempts a minute from this address", body = Problem, content_type = "application/problem+json"),
    ),
    security(("ws_ticket" = [])),
)]
pub async fn events(
    State(state): State<SharedState>,
    Path(v): Path<Uuid>,
    peer: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: axum::http::HeaderMap,
    q: Result<Query<EventsQuery>, QueryRejection>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    // No token on this route: limited per client address, before any
    // database work, so random tickets cannot drain the pool (review 4 M1).
    if let Some(axum::Extension(axum::extract::ConnectInfo(addr))) = peer {
        let ip = crate::net::client_addr(addr.ip(), &headers, &state.http.trusted_proxies);
        state
            .limits
            .check(&format!("ip:{ip}"), "GET", "/v1/vaults/{v}/events")
            .map_err(ApiError::RateLimited)?;
    }
    let Query(q) =
        q.map_err(|e| ApiError::with_detail(ProblemType::InvalidRequest, e.body_text()))?;
    // Consumed before the upgrade: whatever happens next, a second use finds nothing.
    let ticket = match URL_SAFE_NO_PAD.decode(q.ticket.as_bytes()) {
        Ok(t) if t.len() == 32 => sqlx::query("SELECT tenant_id, vault_id, device_id, token_expires_at, claims FROM kura_ws_ticket_consume($1)")
            .bind(hash(&t))
            .fetch_optional(&state.db)
            .await?
            .map(|r| -> Result<_, sqlx::Error> {
                let claims: sqlx::types::Json<serde_json::Map<String, serde_json::Value>> = r.try_get("claims")?;
                Ok((r.try_get::<Uuid, _>("vault_id")?, r.try_get::<Uuid, _>("device_id")?, r.try_get::<time::OffsetDateTime, _>("token_expires_at")?, claims.0))
            })
            .transpose()?,
        _ => None,
    };
    Ok(ws.on_upgrade(move |socket| serve(socket, state, v, ticket)))
}

async fn shut(mut socket: WebSocket, code: u16, reason: &'static str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    drop(socket.send(Message::Close(Some(frame))).await);
}

fn text(n: &Notice) -> Message {
    Message::Text(serde_json::to_string(n).unwrap_or_default().into())
}

/// A pull hint after a lost `LISTEN`, spread over 0–5 s so replicas do not
/// stampede the database (§6.3).
fn jitter() -> Duration {
    let mut b = [0u8; 2];
    if aws_lc_rs::rand::fill(&mut b).is_err() {
        return Duration::ZERO;
    }
    Duration::from_millis(u64::from(u16::from_le_bytes(b)) % 5000)
}

/// A ping tick: the notice to send, or the close code if the device or the role is gone.
async fn on_ping(
    state: &SharedState,
    principal: &crate::auth::Principal,
    device: Uuid,
    v: Uuid,
) -> Result<Notice, (u16, &'static str)> {
    // Revoked since the ticket, or its session was: close (review 4 L1).
    let active: Option<bool> = sqlx::query_scalar("SELECT kura_device_active($1)")
        .bind(device)
        .fetch_one(&state.db)
        .await
        .ok()
        .flatten();
    if active == Some(false) {
        return Err((close::DEVICE_REVOKED, "device_revoked"));
    }
    // The role, as the next request would see it: a binding or a
    // mapping removed, or the vault gone, closes the socket (plan V4).
    // A failing dependency keeps it open, as above.
    if let Some(snap) = state.snapshot()
        && let Err(crate::problem::ApiError::Problem(..)) =
            authz::require_role(&state.db, &snap, principal, v, VaultRole::Viewer).await
    {
        return Err((close::ACCESS_REVOKED, "access_revoked"));
    }
    let head: Option<i64> = sqlx::query_scalar("SELECT kura_vault_head($1)")
        .bind(v)
        .fetch_one(&state.db)
        .await
        .ok()
        .flatten();
    Ok(Notice::Ping {
        head_seq: head.and_then(|h| u64::try_from(h).ok()).unwrap_or_default(),
    })
}

async fn serve(
    mut socket: WebSocket,
    state: SharedState,
    v: Uuid,
    ticket: Option<(
        Uuid,
        Uuid,
        time::OffsetDateTime,
        serde_json::Map<String, serde_json::Value>,
    )>,
) {
    let Some((vault, device, token_expires, claims)) = ticket else {
        return shut(socket, close::TICKET_USED, "ticket_used").await;
    };
    // The token's claims, to re-check the role on every ping; a ticket without them is spent.
    let Some(principal) = crate::auth::Principal::from_claims(claims) else {
        return shut(socket, close::TICKET_USED, "ticket_used").await;
    };
    if vault != v {
        return shut(
            socket,
            close::TICKET_FOR_OTHER_VAULT,
            "ticket_for_other_vault",
        )
        .await;
    }
    let Some(_seat) = state.hub.enter(device) else {
        return shut(socket, close::TOO_MANY_SOCKETS, "too_many_sockets").await;
    };
    let left = u64::try_from(
        token_expires
            .unix_timestamp()
            .saturating_sub(time::OffsetDateTime::now_utc().unix_timestamp()),
    )
    .map_or(Duration::ZERO, Duration::from_secs);
    let expiry = tokio::time::sleep(left);
    tokio::pin!(expiry);
    let mut rx = state.hub.subscribe();
    let mut ping = tokio::time::interval(PING);
    let pull_at = tokio::time::sleep(Duration::MAX);
    tokio::pin!(pull_at);
    let mut pull_pending = false;
    loop {
        tokio::select! {
            () = state.hub.closing.cancelled() => return shut(socket, 1001, "going_away").await,
            () = &mut expiry => return shut(socket, close::TOKEN_EXPIRED, "token_expired").await,
            () = &mut pull_at, if pull_pending => {
                pull_pending = false;
                if socket.send(text(&Notice::Pull)).await.is_err() { return; }
            }
            _ = ping.tick() => {
                let n = match on_ping(&state, &principal, device, v).await {
                    Ok(n) => n,
                    Err((code, reason)) => return shut(socket, code, reason).await,
                };
                if socket.send(text(&n)).await.is_err() { return; }
            }
            e = rx.recv() => match e {
                Ok(Event::Change { vault, seq }) if vault == v => {
                    if socket.send(text(&Notice::Change { seq })).await.is_err() { return; }
                }
                Ok(Event::Pull) if !pull_pending => {
                    pull_pending = true;
                    pull_at.as_mut().reset(tokio::time::Instant::now().checked_add(jitter()).unwrap_or_else(tokio::time::Instant::now));
                }
                Ok(Event::Revoked { device: d }) if d == device => {
                    return shut(socket, close::DEVICE_REVOKED, "device_revoked").await;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    return shut(socket, close::SLOW_CONSUMER, "slow_consumer").await;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            },
            m = socket.recv() => match m {
                Some(Ok(Message::Close(_)) | Err(_)) | None => return,
                Some(Ok(_)) => {}
            },
        }
    }
}
