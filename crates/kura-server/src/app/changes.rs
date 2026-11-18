//! `GET /v1/vaults/{v}/changes` and `GET /v1/vaults/{v}/entries` (RFC 0001
//! §6.3): one item per entry, its current state, read with the `head_seq` it
//! is returned with in one `REPEATABLE READ` snapshot, so a client that
//! advances its cursor never skips a change (§5.4).

use axum::Json;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use kura_api::VaultRole;
use kura_core::wire::ChangesResponse;
use serde::Deserialize;
use sqlx::Row as _;
use utoipa::IntoParams;
use uuid::Uuid;

use super::SharedState;
use super::commits::{head_from_row, head_select};
use super::reader::Reader;
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};

/// Items per page (§4.4).
pub const PAGE: i64 = 1000;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChangesQuery {
    /// The last `seq` this device applied; 0 for a first sync.
    pub since: u64,
    /// The epoch the cursor was read under; a different one answers 410.
    pub epoch: Option<Uuid>,
    /// The cursor this pass began from, when `since` is a later page of it:
    /// the horizon is checked against it, since a pass from 0 crosses the
    /// horizon page by page without having missed anything (absent: `since`).
    pub start: Option<u64>,
    /// `config`: the caller's own Obsidian configuration (§6.8).
    pub space: Option<String>,
    /// Include folder entries (RFC 0007); a client that does not ask is
    /// never sent one.
    #[serde(default)]
    pub folders: bool,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EntriesQuery {
    /// The last entry id of the previous page.
    pub after: Option<Uuid>,
    /// `config`: the caller's own Obsidian configuration (§6.8).
    pub space: Option<String>,
    /// Include folder entries (RFC 0007).
    #[serde(default)]
    pub folders: bool,
}

fn query<T>(q: Result<Query<T>, QueryRejection>) -> Result<T, ApiError> {
    q.map(|Query(v)| v)
        .map_err(|e| ApiError::with_detail(ProblemType::InvalidRequest, e.body_text()))
}

struct Head {
    epoch: Uuid,
    head_seq: i64,
    horizon_seq: i64,
}

/// The transaction, the vault's head, and the `LIKE` patterns of the
/// caller's scope (`%` for the whole vault, T16).
async fn open(
    state: &SharedState,
    reader: &Reader,
    v: Uuid,
    space: Option<&str>,
) -> Result<
    (
        sqlx::Transaction<'static, sqlx::Postgres>,
        Head,
        Vec<String>,
        String,
        Uuid,
    ),
    ApiError,
> {
    let ra = reader.access(state, v, VaultRole::Viewer).await?;
    // The notes, or the caller's own configuration (never a key's, §6.8).
    let space = super::space::resolve(state, space, &ra.access, reader.device()).await?;
    let mut tx = db::begin_tenant_snapshot(&state.db, ra.access.tenant, &reader.subject()).await?;
    let row = sqlx::query("SELECT epoch, head_seq, horizon_seq FROM vault WHERE id = $1")
        .bind(v)
        .fetch_one(&mut *tx)
        .await?;
    let head = Head {
        epoch: row.try_get("epoch")?,
        head_seq: row.try_get("head_seq")?,
        horizon_seq: row.try_get("horizon_seq")?,
    };
    Ok((
        tx,
        head,
        ra.scope.like_patterns(),
        space.namespace,
        ra.access.tenant,
    ))
}

fn response(
    head: &Head,
    rows: &[sqlx::postgres::PgRow],
    next: impl Fn(&super::commits::Head) -> String,
) -> Result<ChangesResponse, ApiError> {
    let mut heads = rows
        .iter()
        .map(head_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let more = heads.len() > usize::try_from(PAGE).unwrap_or(usize::MAX);
    heads.truncate(usize::try_from(PAGE).unwrap_or(usize::MAX));
    Ok(ChangesResponse {
        epoch: head.epoch,
        head_seq: u64::try_from(head.head_seq).unwrap_or_default(),
        next: if more { heads.last().map(next) } else { None },
        items: heads
            .iter()
            .map(super::commits::Head::state)
            .collect::<Result<_, _>>()?,
    })
}

/// Entries changed after a cursor, oldest first.
#[utoipa::path(
    get,
    path = "/vaults/{v}/changes",
    tag = "sync",
    params(("v" = Uuid, Path, description = "Vault id"), ChangesQuery, ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "At most 1,000 entries; follow `next` for more", body = ChangesResponse),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
        (status = 410, description = "cursor_expired, or history_rewound: run a full resync", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_changes(
    State(state): State<SharedState>,
    reader: Reader,
    Path(v): Path<Uuid>,
    q: Result<Query<ChangesQuery>, QueryRejection>,
) -> Result<Json<ChangesResponse>, ApiError> {
    let q = query(q)?;
    let since = i64::try_from(q.since).map_err(|_| ApiError::of(ProblemType::HistoryRewound))?;
    let (mut tx, head, under, ns, tenant) = open(&state, &reader, v, q.space.as_deref()).await?;
    // The server lost history this device has seen (§6.3, decision 32).
    if q.epoch.is_some_and(|e| e != head.epoch) || since > head.head_seq {
        return Err(ApiError::of(ProblemType::HistoryRewound));
    }
    let start = match q.start {
        Some(s) if s > q.since => {
            return Err(ApiError::with_detail(
                ProblemType::InvalidRequest,
                "start is after since",
            ));
        }
        Some(s) => i64::try_from(s).unwrap_or(i64::MAX),
        None => since,
    };
    // A client that lies about `start` only misses deletions itself.
    if start > 0 && start < head.horizon_seq {
        return Err(ApiError::of(ProblemType::CursorExpired));
    }
    let rows = sqlx::query(concat!(
        head_select!(),
        " WHERE e.vault_id = $1 AND e.last_seq > $2 AND (e.path_key LIKE ANY($4) OR (r.blob_hash IS NULL AND NOT e.deleted AND e.path_key || '/' LIKE ANY($4))) AND ($6 OR r.blob_hash IS NOT NULL OR e.deleted) AND e.namespace = $5 ORDER BY e.last_seq LIMIT $3"
    ))
    .bind(v)
    .bind(since)
    .bind(PAGE.saturating_add(1))
    .bind(&under)
    .bind(&ns)
    .bind(q.folders)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let page = response(&head, &rows, |h| h.seq.to_string())?;
    // The last page of a pass of the notes: the device now holds the head
    // (RFC 0006 §4.2). A token or the configuration space records nothing.
    if page.next.is_none()
        && ns == "notes"
        && let Some((_, device)) = reader.device()
    {
        super::sync_state::caught_up(&state, tenant, v, device, head.epoch, head.head_seq, &under)
            .await;
    }
    Ok(Json(page))
}

/// Every live entry, by id: what `kura attach` matches local files against.
#[utoipa::path(
    get,
    path = "/vaults/{v}/entries",
    tag = "sync",
    params(("v" = Uuid, Path, description = "Vault id"), EntriesQuery, ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "At most 1,000 entries; follow `next` for more", body = ChangesResponse),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_entries(
    State(state): State<SharedState>,
    reader: Reader,
    Path(v): Path<Uuid>,
    q: Result<Query<EntriesQuery>, QueryRejection>,
) -> Result<Json<ChangesResponse>, ApiError> {
    let q = query(q)?;
    let (mut tx, head, under, ns, _) = open(&state, &reader, v, q.space.as_deref()).await?;
    let rows = sqlx::query(concat!(
        head_select!(),
        " WHERE e.vault_id = $1 AND NOT e.deleted AND ($2::uuid IS NULL OR e.id > $2) AND (e.path_key LIKE ANY($4) OR (r.blob_hash IS NULL AND NOT e.deleted AND e.path_key || '/' LIKE ANY($4))) AND ($6 OR r.blob_hash IS NOT NULL OR e.deleted) AND e.namespace = $5 ORDER BY e.id LIMIT $3"
    ))
    .bind(v)
    .bind(q.after)
    .bind(PAGE.saturating_add(1))
    .bind(&under)
    .bind(&ns)
    .bind(q.folders)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(response(&head, &rows, |h| h.id.to_string())?))
}
