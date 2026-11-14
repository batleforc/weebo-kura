//! `/v1/vaults/{v}/chunks` and `/v1/vaults/{v}/blobs` (RFC 0001 §4.4, §6.4).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kura_api::VaultRole;
use kura_core::{ChunkHash, blob_hash};
use serde::{Deserialize, Serialize};
use sqlx::Row as _;
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::{Body, Raw};
use super::device::DeviceCaller;
use crate::authz::{self, VaultAccess};
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};
use crate::store::{Storage, crypto};
use object_store::ObjectStoreExt as _;

/// Hashes per `chunks/missing` call (§4.4).
pub const MAX_HASHES: usize = 1000;
/// A chunk body (§4.3, §6.8).
pub const MAX_CHUNK_BYTES: usize = kura_core::chunk::MAX_CHUNK;
/// AES-GCM with random nonces: warn well before 2^32 encryptions under one key (§6.6).
pub const ENCRYPTION_WARNING: i64 = 1 << 30;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MissingRequest {
    /// Chunk hashes, 64 lowercase hex characters each, at most 1,000.
    pub hashes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MissingResponse {
    /// The requested hashes the vault does not hold, in request order.
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlobManifest {
    /// File size in bytes.
    pub size: u64,
    /// Ordered chunk hashes.
    pub chunks: Vec<String>,
}

fn storage(state: &SharedState) -> Result<&Storage, ApiError> {
    state.storage.as_deref().ok_or(ApiError::with_detail(
        ProblemType::Unavailable,
        "no object storage configured",
    ))
}

fn parse_hash(s: &str) -> Result<ChunkHash, ApiError> {
    s.parse().map_err(|_| {
        ApiError::with_detail(
            ProblemType::InvalidRequest,
            "a hash is 64 lowercase hex characters",
        )
    })
}

async fn access(
    state: &SharedState,
    caller: &DeviceCaller,
    v: Uuid,
    min: VaultRole,
) -> Result<VaultAccess, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    authz::require_role(&state.db, &snap, &caller.caller.principal, v, min).await
}

/// Which of these chunks the vault lacks; present ones are kept from garbage collection.
#[utoipa::path(
    post,
    path = "/vaults/{v}/chunks/missing",
    tag = "chunks",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = MissingRequest,
    responses(
        (status = 200, description = "Missing hashes", body = MissingResponse),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn missing_chunks(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
    Body(body): Body<MissingRequest>,
) -> Result<Json<MissingResponse>, ApiError> {
    let a = access(&state, &caller, v, VaultRole::Editor).await?;
    if body.hashes.len() > MAX_HASHES {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "at most 1,000 hashes",
        ));
    }
    let hashes = body
        .hashes
        .iter()
        .map(|h| parse_hash(h))
        .collect::<Result<Vec<_>, _>>()?;
    let raw: Vec<Vec<u8>> = hashes.iter().map(|h| h.as_bytes().to_vec()).collect();
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    // "Present" refreshes last_touched, so GC cannot take it before the commit (§6.4).
    let present: Vec<Vec<u8>> =
        sqlx::query_scalar("UPDATE chunk SET last_touched = now() WHERE vault_id = $1 AND hash = ANY($2) RETURNING hash")
            .bind(v)
            .bind(&raw)
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    let missing = hashes
        .iter()
        .filter(|h| !present.iter().any(|p| p.as_slice() == h.as_bytes()))
        .map(ToString::to_string)
        .collect();
    Ok(Json(MissingResponse { missing }))
}

/// Upload one chunk; its BLAKE3 must be its address.
#[utoipa::path(
    put,
    path = "/vaults/{v}/chunks/{hash}",
    tag = "chunks",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("hash" = String, Path, description = "BLAKE3 of the plaintext, lowercase hex"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "At most 4 MiB"),
    responses(
        (status = 204, description = "Stored, or already present"),
        (status = 413, description = "Over 4 MiB", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Hash mismatch, or quota exceeded", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn put_chunk(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, hash)): Path<(Uuid, String)>,
    Raw(body): Raw,
) -> Result<StatusCode, ApiError> {
    let a = access(&state, &caller, v, VaultRole::Editor).await?;
    let store = storage(&state)?;
    let hash = parse_hash(&hash)?;
    let len = i64::try_from(body.len()).map_err(|_| ApiError::of(ProblemType::PayloadTooLarge))?;
    // T6: the bytes are their address, checked before anything is stored.
    let data = body.clone();
    let actual = tokio::task::spawn_blocking(move || ChunkHash::of(&data))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    if actual != hash {
        return Err(ApiError::of(ProblemType::HashMismatch));
    }
    // 1. A short transaction: already present? over quota? No connection is
    //    held across the Vault and object storage calls below (review P1).
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let present: bool = sqlx::query_scalar(
        "UPDATE chunk SET last_touched = now() WHERE vault_id = $1 AND hash = $2 RETURNING true",
    )
    .bind(v)
    .bind(hash.as_bytes().as_slice())
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(false);
    if present {
        tx.commit().await?;
        return Ok(StatusCode::NO_CONTENT);
    }
    check_quota(&state, &a, &mut tx, len).await?;
    tx.commit().await?;

    // 2. Seal and store, no transaction open. Storage first: if it fails, no
    //    row points at a missing object (§14).
    let key = store
        .keys
        .get(&state.db, a.tenant, &a.tenant_slug, v)
        .await?;
    let object_key = key
        .object_key(v, &hash)
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let sealed = tokio::task::spawn_blocking(move || crypto::seal(&key, v, &hash, &body))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let path = object_store::path::Path::from(object_key.as_str());
    let objects = store.objects();
    objects
        .put(&path, sealed.into())
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;

    // 3. Record it. The tenant lock serializes this check with every other
    //    upload of the tenant, so concurrent uploads cannot overshoot the
    //    quota (review P3).
    let recorded = record_chunk(&state, &a, v, &hash, len, &object_key).await;
    let inserted = match recorded {
        Ok(inserted) => inserted,
        Err(e) => {
            delete_orphan(&objects, &path).await;
            return Err(e);
        }
    };
    if !inserted {
        // Another upload of the same chunk won; ours is an orphan object.
        delete_orphan(&objects, &path).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn record_chunk(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    hash: &ChunkHash,
    len: i64,
    object_key: &str,
) -> Result<bool, ApiError> {
    let size = i32::try_from(len).map_err(|_| ApiError::of(ProblemType::PayloadTooLarge))?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('kura.quota:' || $1::text))")
        .bind(a.tenant)
        .execute(&mut *tx)
        .await?;
    check_quota(state, a, &mut tx, len).await?;
    let inserted: Option<bool> = sqlx::query_scalar(
        "INSERT INTO chunk (vault_id, tenant_id, hash, size, object_key) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (vault_id, hash) DO NOTHING RETURNING true",
    )
    .bind(v)
    .bind(a.tenant)
    .bind(hash.as_bytes().as_slice())
    .bind(size)
    .bind(object_key)
    .fetch_optional(&mut *tx)
    .await?;
    let count: i64 = sqlx::query_scalar(
        "UPDATE vault SET encryptions = encryptions + 1 WHERE id = $1 RETURNING encryptions",
    )
    .bind(v)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    if count >= ENCRYPTION_WARNING {
        tracing::warn!(vault = %v, encryptions = count, "data key nearing its random-nonce bound; rotate the vault key");
    }
    Ok(inserted.is_some())
}

async fn delete_orphan(
    objects: &std::sync::Arc<dyn object_store::ObjectStore>,
    path: &object_store::path::Path,
) {
    if let Err(e) = objects.delete(path).await {
        tracing::warn!(error = %e, "orphan chunk object not deleted; garbage collection will");
    }
}

/// Writes are refused once the tenant's stored bytes would pass its quota (§4.3).
async fn check_quota(
    state: &SharedState,
    a: &VaultAccess,
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    adding: i64,
) -> Result<(), ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let Some(quota) = snap
        .config
        .tenants
        .get(a.tenant_slug.as_str())
        .and_then(|t| t.doc.spec.quota.storage)
    else {
        return Ok(());
    };
    // shortcut: sums the tenant's chunks on each upload; a running total arrives if uploads get slow.
    let used: i64 = sqlx::query_scalar("SELECT coalesce(sum(size), 0)::bigint FROM chunk")
        .fetch_one(&mut **tx)
        .await?;
    let quota = i64::try_from(quota.bytes()).unwrap_or(i64::MAX);
    if used.saturating_add(adding) > quota {
        return Err(ApiError::with_detail(
            ProblemType::QuotaExceeded,
            format!("storage quota of {quota} bytes reached"),
        ));
    }
    Ok(())
}

/// Download one chunk, decrypted.
#[utoipa::path(
    get,
    path = "/vaults/{v}/chunks/{hash}",
    tag = "chunks",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("hash" = String, Path, description = "BLAKE3 of the plaintext, lowercase hex"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    responses(
        (status = 200, description = "The chunk's plaintext", body = Vec<u8>, content_type = "application/octet-stream"),
        (status = 404, description = "No such vault or chunk", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_chunk(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, hash)): Path<(Uuid, String)>,
) -> Result<Response, ApiError> {
    let a = access(&state, &caller, v, VaultRole::Viewer).await?;
    let store = storage(&state)?;
    let hash = parse_hash(&hash)?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let object_key: Option<String> =
        sqlx::query_scalar("SELECT object_key FROM chunk WHERE vault_id = $1 AND hash = $2")
            .bind(v)
            .bind(hash.as_bytes().as_slice())
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    let object_key = object_key.ok_or(ApiError::of(ProblemType::NotFound))?;
    let key = store
        .keys
        .get(&state.db, a.tenant, &a.tenant_slug, v)
        .await?;
    let stored = store
        .objects()
        .get(&object_store::path::Path::from(object_key.as_str()))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let opened = tokio::task::spawn_blocking(move || crypto::open(&key, v, &hash, &stored))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let plain = opened.map_err(|e| {
        state.metrics.chunk_corrupt_total.inc();
        tracing::error!(vault = %v, error = %e, "stored chunk failed its check");
        ApiError::Unavailable(e.to_string())
    })?;
    let mut response = (StatusCode::OK, plain).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    // A chunk never changes under its address (§6.4).
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, immutable, max-age=31536000"),
    );
    if let Ok(etag) = HeaderValue::from_str(&format!("\"{hash}\"")) {
        headers.insert(header::ETAG, etag);
    }
    Ok(response)
}

/// Store a file version's manifest; every chunk must already be present.
#[utoipa::path(
    put,
    path = "/vaults/{v}/blobs/{hash}",
    tag = "chunks",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("hash" = String, Path, description = "kura_core::blob_hash of the manifest"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    request_body = BlobManifest,
    responses(
        (status = 204, description = "Stored, or already present"),
        (status = 413, description = "Over the tenant's maxFileSize", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Hash mismatch, a chunk missing, or sizes that disagree", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn put_blob(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, hash)): Path<(Uuid, String)>,
    Body(body): Body<BlobManifest>,
) -> Result<StatusCode, ApiError> {
    let a = access(&state, &caller, v, VaultRole::Editor).await?;
    let hash = parse_hash(&hash)?;
    let chunks = body
        .chunks
        .iter()
        .map(|h| parse_hash(h))
        .collect::<Result<Vec<_>, _>>()?;
    if blob_hash(body.size, &chunks) != hash {
        return Err(ApiError::of(ProblemType::HashMismatch));
    }
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let max_file = snap
        .config
        .tenants
        .get(a.tenant_slug.as_str())
        .and_then(|t| t.doc.spec.quota.max_file_size);
    if max_file.is_some_and(|m| body.size > m.bytes()) {
        return Err(ApiError::of(ProblemType::PayloadTooLarge));
    }
    let raw: Vec<Vec<u8>> = chunks.iter().map(|h| h.as_bytes().to_vec()).collect();
    let size = i64::try_from(body.size).map_err(|_| ApiError::of(ProblemType::PayloadTooLarge))?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let rows = sqlx::query("SELECT hash, size FROM chunk WHERE vault_id = $1 AND hash = ANY($2)")
        .bind(v)
        .bind(&raw)
        .fetch_all(&mut *tx)
        .await?;
    let mut total: i64 = 0;
    for h in &raw {
        let row = rows
            .iter()
            .find(|r| r.try_get::<Vec<u8>, _>("hash").is_ok_and(|x| &x == h))
            .ok_or(ApiError::with_detail(
                ProblemType::HashMismatch,
                "a chunk of the manifest is not stored",
            ))?;
        total = total.saturating_add(i64::from(row.try_get::<i32, _>("size")?));
    }
    if total != size {
        return Err(ApiError::with_detail(
            ProblemType::HashMismatch,
            "chunk sizes do not add up to the file size",
        ));
    }
    let inserted: Option<bool> = sqlx::query_scalar(
        "INSERT INTO blob (vault_id, tenant_id, hash, size) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING RETURNING true",
    )
    .bind(v)
    .bind(a.tenant)
    .bind(hash.as_bytes().as_slice())
    .bind(size)
    .fetch_optional(&mut *tx)
    .await?;
    if inserted.is_some() {
        let positions: Vec<i32> = (0..raw.len())
            .filter_map(|i| i32::try_from(i).ok())
            .collect();
        sqlx::query(
            "INSERT INTO blob_chunk (vault_id, tenant_id, blob_hash, position, chunk_hash)
             SELECT $1, $2, $3, p, c FROM unnest($4::int[], $5::bytea[]) AS u(p, c)",
        )
        .bind(v)
        .bind(a.tenant)
        .bind(hash.as_bytes().as_slice())
        .bind(positions)
        .bind(&raw)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A file version's manifest.
#[utoipa::path(
    get,
    path = "/vaults/{v}/blobs/{hash}",
    tag = "chunks",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("hash" = String, Path, description = "Blob hash"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    responses(
        (status = 200, description = "The manifest", body = BlobManifest),
        (status = 404, description = "No such vault or blob", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_blob(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, hash)): Path<(Uuid, String)>,
) -> Result<Json<BlobManifest>, ApiError> {
    let a = access(&state, &caller, v, VaultRole::Viewer).await?;
    let hash = parse_hash(&hash)?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let size: Option<i64> =
        sqlx::query_scalar("SELECT size FROM blob WHERE vault_id = $1 AND hash = $2")
            .bind(v)
            .bind(hash.as_bytes().as_slice())
            .fetch_optional(&mut *tx)
            .await?;
    let size = size.ok_or(ApiError::of(ProblemType::NotFound))?;
    let chunks: Vec<Vec<u8>> =
        sqlx::query_scalar("SELECT chunk_hash FROM blob_chunk WHERE vault_id = $1 AND blob_hash = $2 ORDER BY position")
            .bind(v)
            .bind(hash.as_bytes().as_slice())
            .fetch_all(&mut *tx)
            .await?;
    tx.commit().await?;
    let chunks = chunks
        .iter()
        .filter_map(|c| <[u8; 32]>::try_from(c.as_slice()).ok())
        .map(|c| ChunkHash::from_bytes(c).to_string())
        .collect();
    Ok(Json(BlobManifest {
        size: u64::try_from(size).unwrap_or(0),
        chunks,
    }))
}
