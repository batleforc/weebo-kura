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
/// Chunk payload per `chunks/batch` answer (RFC 0002 §4.2).
pub const MAX_BATCH_BYTES: usize = 4 << 20;
/// Request body of the batch routes: 1,000 hashes of JSON (RFC 0002 §4.3).
pub const MAX_BATCH_BODY: usize = 128 << 10;
/// Objects one `chunks/batch` answer fetches at a time.
pub const BATCH_FETCHES: usize = 8;
/// `chunks/batch` answers one replica builds at once (RFC 0002 §7.5).
pub const BATCHES_IN_FLIGHT: usize = 8;
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
    storage(&state)?;
    let hash = parse_hash(&hash)?;
    // T6: the bytes are their address, checked before anything is stored.
    let data = body.clone();
    let actual = tokio::task::spawn_blocking(move || ChunkHash::of(&data))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    if actual != hash {
        return Err(ApiError::of(ProblemType::HashMismatch));
    }
    store_chunk(&state, &a, v, hash, body).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stores one chunk whose hash was checked: sealed, stored, then recorded
/// within the tenant's quota. Shared by uploads and server-side merges.
pub(super) async fn store_chunk(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    hash: ChunkHash,
    body: bytes::Bytes,
) -> Result<(), ApiError> {
    let store = storage(state)?;
    let len = i64::try_from(body.len()).map_err(|_| ApiError::of(ProblemType::PayloadTooLarge))?;
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
        return Ok(());
    }
    check_quota(state, a, &mut tx, len).await?;
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
    let (tx, inserted, count) = match record_chunk(state, a, v, &hash, len, &object_key).await {
        Ok(recorded) => recorded,
        Err(e) => {
            // Refused before the commit: the transaction rolled back, no row
            // names the object.
            delete_orphan(&objects, &path).await;
            return Err(e);
        }
    };
    // A failed commit is ambiguous: the row may have landed. The object stays
    // for the orphan sweep (migration 0011) rather than leave a row naming a
    // deleted object (review 2026-11-15 #3).
    tx.commit().await?;
    if count >= ENCRYPTION_WARNING {
        tracing::warn!(vault = %v, encryptions = count, "data key nearing its random-nonce bound; rotate the vault key");
    }
    if !inserted {
        // Another upload of the same chunk won; ours is an orphan object.
        delete_orphan(&objects, &path).await;
    }
    Ok(())
}

/// The chunk's row and the vault's encryption count, in a transaction left
/// open for the caller to commit: `(transaction, inserted, encryptions)`.
async fn record_chunk<'a>(
    state: &'a SharedState,
    a: &VaultAccess,
    v: Uuid,
    hash: &ChunkHash,
    len: i64,
    object_key: &str,
) -> Result<(sqlx::Transaction<'a, sqlx::Postgres>, bool, i64), ApiError> {
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
    Ok((tx, inserted.is_some(), count))
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
    reader: super::reader::Reader,
    Path((v, hash)): Path<(Uuid, String)>,
) -> Result<Response, ApiError> {
    let ra = reader.access(&state, v, VaultRole::Viewer).await?;
    let hash = parse_hash(&hash)?;
    if !ra.sees_content(&state, &hash, true).await? {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    let a = ra.access;
    let plain = read_chunk(&state, &a, v, hash).await?;
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

/// One stored chunk, decrypted and checked against its address.
pub(super) async fn read_chunk(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    hash: ChunkHash,
) -> Result<Vec<u8>, ApiError> {
    let store = storage(state)?;
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
    open_object(state, key, v, hash, &object_key).await
}

/// A whole stored file version, chunk by chunk.
pub(super) async fn read_blob(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    hash: ChunkHash,
) -> Result<Vec<u8>, ApiError> {
    let store = storage(state)?;
    // Every chunk's object in one transaction, not one per chunk (review
    // 2026-11-15 #20): a merge reads three versions.
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let rows: Vec<(Vec<u8>, Option<String>)> = sqlx::query_as(
        "SELECT bc.chunk_hash, c.object_key FROM blob_chunk bc
         LEFT JOIN chunk c ON c.vault_id = bc.vault_id AND c.hash = bc.chunk_hash
         WHERE bc.vault_id = $1 AND bc.blob_hash = $2 ORDER BY bc.position",
    )
    .bind(v)
    .bind(hash.as_bytes().as_slice())
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let key = store
        .keys
        .get(&state.db, a.tenant, &a.tenant_slug, v)
        .await?;
    let mut out = Vec::new();
    for (c, object_key) in rows {
        let h = <[u8; 32]>::try_from(c.as_slice())
            .map(ChunkHash::from_bytes)
            .map_err(|_| ApiError::Unavailable("stored chunk hash is not 32 bytes".into()))?;
        let object_key = object_key.ok_or(ApiError::of(ProblemType::NotFound))?;
        out.extend_from_slice(&open_object(state, key.clone(), v, h, &object_key).await?);
    }
    Ok(out)
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
    store_blob(&state, &a, v, hash, body.size, &chunks).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Records a manifest whose address was checked; every chunk must be stored.
pub(super) async fn store_blob(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    hash: ChunkHash,
    size: u64,
    chunks: &[ChunkHash],
) -> Result<(), ApiError> {
    let raw: Vec<Vec<u8>> = chunks.iter().map(|h| h.as_bytes().to_vec()).collect();
    let size = i64::try_from(size).map_err(|_| ApiError::of(ProblemType::PayloadTooLarge))?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    // FOR KEY SHARE: garbage collection skips these rows until this commits,
    // and a row it already took is not returned, so the client re-uploads
    // (decision 24).
    let rows = sqlx::query(
        "SELECT hash, size FROM chunk WHERE vault_id = $1 AND hash = ANY($2) FOR KEY SHARE",
    )
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
        // A manifest stored again is touched, so it is not collected before
        // the commit that follows (§6.9); xmax = 0 only on a fresh insert.
        "INSERT INTO blob (vault_id, tenant_id, hash, size) VALUES ($1, $2, $3, $4)
         ON CONFLICT (vault_id, hash) DO UPDATE SET last_touched = now()
         RETURNING xmax::text = '0'",
    )
    .bind(v)
    .bind(a.tenant)
    .bind(hash.as_bytes().as_slice())
    .bind(size)
    .fetch_optional(&mut *tx)
    .await?;
    if inserted == Some(true) {
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
    Ok(())
}

/// Stores `data` as a file version: cut with the vault's chunking, chunks the
/// vault lacks sealed and stored, then the manifest. Returns its address.
pub(super) async fn store_content(
    state: &SharedState,
    a: &VaultAccess,
    v: Uuid,
    data: Vec<u8>,
) -> Result<ChunkHash, ApiError> {
    let params = kura_core::ChunkParams::new(
        a.chunking.min.bytes(),
        a.chunking.avg.bytes(),
        a.chunking.max.bytes(),
    )
    .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let data = bytes::Bytes::from(data);
    let spans = kura_core::chunks(&data, params);
    let hashes: Vec<ChunkHash> = spans.iter().map(|c| c.hash).collect();
    for span in &spans {
        let end = span.offset.saturating_add(span.len);
        store_chunk(
            state,
            a,
            v,
            span.hash,
            data.slice(span.offset..end.min(data.len())),
        )
        .await?;
    }
    let size = data.len() as u64;
    let blob = blob_hash(size, &hashes);
    store_blob(state, a, v, blob, size, &hashes).await?;
    Ok(blob)
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
    reader: super::reader::Reader,
    Path((v, hash)): Path<(Uuid, String)>,
) -> Result<Json<BlobManifest>, ApiError> {
    let ra = reader.access(&state, v, VaultRole::Viewer).await?;
    let hash = parse_hash(&hash)?;
    if !ra.sees_content(&state, &hash, false).await? {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    let a = ra.access;
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

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlobBatchRequest {
    /// Blob hashes, 64 lowercase hex characters each, 1 to 1,000.
    pub blobs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HashedManifest {
    pub hash: String,
    /// File size in bytes.
    pub size: u64,
    /// Ordered chunk hashes.
    pub chunks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BlobBatchResponse {
    /// The manifests found, in request order.
    pub manifests: Vec<HashedManifest>,
    /// Unknown or not visible to the caller: the same answer for both.
    pub missing: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ChunkBatchRequest {
    /// Chunk hashes, 64 lowercase hex characters each, 1 to 1,000.
    pub chunks: Vec<String>,
}

/// 1 to 1,000 hashes, parsed, duplicates dropped, first occurrence kept.
fn batch_hashes(list: &[String]) -> Result<Vec<ChunkHash>, ApiError> {
    if list.is_empty() || list.len() > MAX_HASHES {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "between 1 and 1,000 hashes",
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(list.len());
    for h in list {
        let h = parse_hash(h)?;
        if seen.insert(h) {
            out.push(h);
        }
    }
    Ok(out)
}

fn hash_of(raw: &[u8]) -> Option<ChunkHash> {
    <[u8; 32]>::try_from(raw).ok().map(ChunkHash::from_bytes)
}

/// Many file manifests in one call (RFC 0002 §4.4).
#[utoipa::path(
    post,
    path = "/vaults/{v}/blobs/batch",
    tag = "chunks",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = BlobBatchRequest,
    responses(
        (status = 200, description = "Manifests found, and the hashes that were not", body = BlobBatchResponse),
        (status = 400, description = "No hash, more than 1,000, or one malformed", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn batch_blobs(
    State(state): State<SharedState>,
    reader: super::reader::Reader,
    Path(v): Path<Uuid>,
    Body(body): Body<BlobBatchRequest>,
) -> Result<Json<BlobBatchResponse>, ApiError> {
    let ra = reader.access(&state, v, VaultRole::Viewer).await?;
    let hashes = batch_hashes(&body.blobs)?;
    let visible = ra.sees_contents(&state, &hashes, false).await?;
    let raw: Vec<Vec<u8>> = hashes
        .iter()
        .filter(|h| visible.contains(h))
        .map(|h| h.as_bytes().to_vec())
        .collect();
    let mut tx = db::begin_tenant(&state.db, ra.access.tenant, None).await?;
    let sizes = sqlx::query("SELECT hash, size FROM blob WHERE vault_id = $1 AND hash = ANY($2)")
        .bind(v)
        .bind(&raw)
        .fetch_all(&mut *tx)
        .await?;
    let parts = sqlx::query(
        "SELECT blob_hash, chunk_hash FROM blob_chunk WHERE vault_id = $1 AND blob_hash = ANY($2) ORDER BY blob_hash, position",
    )
    .bind(v)
    .bind(&raw)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut size_of = std::collections::HashMap::new();
    for row in &sizes {
        let (h, size): (Vec<u8>, i64) = (row.try_get("hash")?, row.try_get("size")?);
        if let Some(h) = hash_of(&h) {
            size_of.insert(h, u64::try_from(size).unwrap_or(0));
        }
    }
    let mut chunks_of: std::collections::HashMap<ChunkHash, Vec<String>> =
        std::collections::HashMap::new();
    for row in &parts {
        let (b, c): (Vec<u8>, Vec<u8>) = (row.try_get("blob_hash")?, row.try_get("chunk_hash")?);
        if let (Some(b), Some(c)) = (hash_of(&b), hash_of(&c)) {
            chunks_of.entry(b).or_default().push(c.to_string());
        }
    }
    let mut out = BlobBatchResponse {
        manifests: Vec::new(),
        missing: Vec::new(),
    };
    for h in hashes {
        match size_of.get(&h) {
            Some(&size) => out.manifests.push(HashedManifest {
                hash: h.to_string(),
                size,
                chunks: chunks_of.remove(&h).unwrap_or_default(),
            }),
            None => out.missing.push(h.to_string()),
        }
    }
    let m = &state.metrics.batch_items_total;
    m.with_label_values(&["blobs", "found"])
        .inc_by(out.manifests.len() as u64);
    m.with_label_values(&["blobs", "missing"])
        .inc_by(out.missing.len() as u64);
    Ok(Json(out))
}

/// Many chunks in one answer, decrypted, at most 4 MiB of them (RFC 0002
/// §4.4): records `hash[32] status[1] length[4, big-endian] bytes`, status
/// 0 found, 1 missing (unknown or not visible). Hashes past the budget are
/// left out; the client asks for them again.
#[utoipa::path(
    post,
    path = "/vaults/{v}/chunks/batch",
    tag = "chunks",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = ChunkBatchRequest,
    responses(
        (status = 200, description = "Records in request order, up to 4 MiB of chunks", body = Vec<u8>, content_type = "application/octet-stream"),
        (status = 400, description = "No hash, more than 1,000, or one malformed", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Over 60 calls a minute, or this replica already builds 8 answers; retry after `Retry-After`", body = Problem, content_type = "application/problem+json"),
        (status = 503, description = "Storage failing; retry after `Retry-After`", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn batch_chunks(
    State(state): State<SharedState>,
    reader: super::reader::Reader,
    Path(v): Path<Uuid>,
    Body(body): Body<ChunkBatchRequest>,
) -> Result<Response, ApiError> {
    use futures_util::StreamExt as _;
    let ra = reader.access(&state, v, VaultRole::Viewer).await?;
    let hashes = batch_hashes(&body.chunks)?;
    // Memory per replica is bounded before any work is done (T4). A busy
    // replica is not a failing dependency: 429, never counted as a 503.
    let Ok(_permit) = state.batches.try_acquire() else {
        return Err(ApiError::RateLimited(1));
    };
    let visible = ra.sees_contents(&state, &hashes, true).await?;
    let a = &ra.access;
    let raw: Vec<Vec<u8>> = hashes
        .iter()
        .filter(|h| visible.contains(h))
        .map(|h| h.as_bytes().to_vec())
        .collect();
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let rows = sqlx::query(
        "SELECT hash, size, object_key FROM chunk WHERE vault_id = $1 AND hash = ANY($2)",
    )
    .bind(v)
    .bind(&raw)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut stored = std::collections::HashMap::new();
    for row in &rows {
        let (h, size, key): (Vec<u8>, i32, String) = (
            row.try_get("hash")?,
            row.try_get("size")?,
            row.try_get("object_key")?,
        );
        if let Some(h) = hash_of(&h) {
            stored.insert(h, (usize::try_from(size).unwrap_or(usize::MAX), key));
        }
    }
    // In request order until the next chunk would pass the budget; the
    // budget comes from the stored sizes, so nothing is fetched to be dropped.
    let mut plan: Vec<(ChunkHash, Option<String>)> = Vec::new();
    let mut budget = 0usize;
    for h in hashes {
        match stored.remove(&h) {
            Some((size, key)) => {
                let next = budget.saturating_add(size);
                if budget > 0 && next > MAX_BATCH_BYTES {
                    break;
                }
                budget = next;
                plan.push((h, Some(key)));
            }
            None => plan.push((h, None)),
        }
    }
    let found = plan.iter().filter(|(_, k)| k.is_some()).count();
    let data_key = if found > 0 {
        Some(
            storage(&state)?
                .keys
                .get(&state.db, a.tenant, &a.tenant_slug, v)
                .await?,
        )
    } else {
        None
    };
    let jobs: Vec<(ChunkHash, Option<String>)> = plan.clone();
    let parts: Vec<Result<Option<Vec<u8>>, ApiError>> = futures_util::stream::iter(jobs)
        .map(|(h, key)| {
            let (state, data_key) = (state.clone(), data_key.clone());
            async move {
                let (Some(key), Some(data_key)) = (key, data_key) else {
                    return Ok(None);
                };
                open_object(&state, data_key, v, h, &key).await.map(Some)
            }
        })
        .buffered(BATCH_FETCHES)
        .collect()
        .await;
    let mut out = Vec::with_capacity(budget.saturating_add(plan.len().saturating_mul(37)));
    for ((h, _), part) in plan.iter().zip(parts) {
        out.extend_from_slice(h.as_bytes());
        match part? {
            Some(bytes) => {
                out.push(0);
                out.extend_from_slice(
                    &u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes(),
                );
                out.extend_from_slice(&bytes);
            }
            None => {
                out.push(1);
                out.extend_from_slice(&0u32.to_be_bytes());
            }
        }
    }
    let m = &state.metrics.batch_items_total;
    m.with_label_values(&["chunks", "found"])
        .inc_by(found as u64);
    m.with_label_values(&["chunks", "missing"])
        .inc_by(plan.len().saturating_sub(found) as u64);
    state.metrics.batch_response_bytes.observe(out.len() as f64);
    let mut response = (StatusCode::OK, out).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    Ok(response)
}

/// One stored object, decrypted with the vault's key and checked against its address.
async fn open_object(
    state: &SharedState,
    key: std::sync::Arc<crypto::DataKey>,
    v: Uuid,
    hash: ChunkHash,
    object_key: &str,
) -> Result<Vec<u8>, ApiError> {
    let stored = storage(state)?
        .objects()
        .get(&object_store::path::Path::from(object_key))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let opened = tokio::task::spawn_blocking(move || crypto::open(&key, v, &hash, &stored))
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    opened.map_err(|e| {
        state.metrics.chunk_corrupt_total.inc();
        tracing::error!(vault = %v, error = %e, "stored chunk failed its check");
        ApiError::Unavailable(e.to_string())
    })
}
