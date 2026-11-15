//! Opt-in search (RFC 0001 §6.7, decision 39): index workers and
//! `POST /vaults/{v}/search`. Only Markdown heads are indexed, in French and
//! English with accents folded; snippets come from decrypting the top
//! results, at most 1 MiB a query.

use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use kura_api::VaultRole;
use kura_core::ChunkHash;
use serde::{Deserialize, Serialize};
use sqlx::Row as _;
use tokio_util::sync::CancellationToken;
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::chunks::read_blob;
use super::reader::Reader;
use crate::authz;
use crate::problem::{ApiError, Problem, ProblemType};

/// A note's text is cut here before `to_tsvector` (§6.7: a `tsvector` stays
/// under 1 MB).
pub const MAX_INDEXED: usize = 256 << 10;
/// Larger files are indexed by path only.
// shortcut: a note is read whole before the cut; read chunk by chunk if
// notes this large become common.
const MAX_READ: i64 = 4 << 20;
/// Decrypted for snippets, per query (§6.7).
const SNIPPET_BUDGET: i64 = 1 << 20;
pub const MAX_RESULTS: u32 = 50;
/// Bounds the cost of a query (review 7 L6).
pub const MAX_QUERY: usize = 256;
const BATCH: i32 = 100;

/// Markdown notes only (§6.7).
pub fn indexable(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md"))
}

/// At most `MAX_INDEXED` bytes of text, cut on a character boundary.
fn cut(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes.get(..MAX_INDEXED).unwrap_or(bytes));
    text.trim_end_matches('\u{FFFD}').to_owned()
}

/// What one round did.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Round {
    pub queued: u64,
    pub dropped: u64,
    pub indexed: u64,
    pub failed: u64,
}

/// One round: backfill or drop tenants whose setting changed, then up to
/// 100 jobs. Every replica runs rounds; claims keep them apart.
pub async fn round(state: &SharedState) -> Result<Round, ApiError> {
    let mut r = Round::default();
    let (queued, dropped): (i64, i64) = sqlx::query_as("SELECT * FROM kura_search_sync()")
        .fetch_one(&state.db)
        .await?;
    r.queued = queued.unsigned_abs();
    r.dropped = dropped.unsigned_abs();
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let jobs = sqlx::query("SELECT * FROM kura_index_claim($1)")
        .bind(BATCH)
        .fetch_all(&state.db)
        .await?;
    for j in &jobs {
        let (vault, entry, revision): (Uuid, Uuid, Uuid) = (
            j.try_get("vault_id")?,
            j.try_get("entry_id")?,
            j.try_get("revision_id")?,
        );
        let outcome = index_one(state, &snap, j).await;
        let set = match outcome {
            Ok(text) => {
                sqlx::query("SELECT kura_index_set($1, $2, $3, $4)")
                    .bind(vault)
                    .bind(entry)
                    .bind(revision)
                    .bind(text)
                    .execute(&state.db)
                    .await
            }
            Err(e) => Err(sqlx::Error::Protocol(format!("{e}"))),
        };
        match set {
            Ok(_) => r.indexed = r.indexed.saturating_add(1),
            Err(e) => {
                tracing::warn!(vault = %vault, entry = %entry, error = %e, "indexing failed, retried");
                sqlx::query("SELECT kura_index_failed($1, $2)")
                    .bind(vault)
                    .bind(entry)
                    .execute(&state.db)
                    .await?;
                r.failed = r.failed.saturating_add(1);
            }
        }
    }
    let (pending, failed): (i64, i64) = sqlx::query_as("SELECT * FROM kura_index_depth()")
        .fetch_one(&state.db)
        .await?;
    state.metrics.index_queue_depth.set(pending);
    state.metrics.index_failed.set(failed);
    #[expect(clippy::cast_precision_loss, reason = "unix seconds fit an f64")]
    state
        .metrics
        .index_round_timestamp_seconds
        .set(time::OffsetDateTime::now_utc().unix_timestamp() as f64);
    Ok(r)
}

/// The text to index for a claimed head, `None` to drop its document.
async fn index_one(
    state: &SharedState,
    snap: &crate::reconcile::snapshot::Snapshot,
    j: &sqlx::postgres::PgRow,
) -> Result<Option<String>, ApiError> {
    let path: String = j.try_get("path")?;
    let deleted: bool = j.try_get("deleted")?;
    if deleted || !indexable(&path) {
        return Ok(None);
    }
    let size: i64 = j.try_get("size")?;
    if size > MAX_READ {
        return Ok(Some(path));
    }
    let (vault, tenant): (Uuid, Uuid) = (j.try_get("vault_id")?, j.try_get("tenant_id")?);
    let a = authz::key_access(&state.db, snap, tenant, vault)
        .await?
        .ok_or(ApiError::of(ProblemType::NotFound))?;
    let raw: Option<Vec<u8>> = j.try_get("blob_hash")?;
    let hash = raw
        .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
        .map(ChunkHash::from_bytes)
        .ok_or(ApiError::of(ProblemType::NotFound))?;
    let bytes = read_blob(state, &a, vault, hash).await?;
    // The path is searchable too, by its words.
    Ok(Some(format!("{path}\n{}", cut(&bytes))))
}

/// Rounds every 5 seconds while jobs remain, until `shutdown`.
pub async fn run(state: SharedState, shutdown: CancellationToken) {
    loop {
        let wait = match round(&state).await {
            Ok(r) if r.indexed > 0 || r.failed > 0 => Duration::from_millis(200),
            Ok(_) => Duration::from_secs(5),
            Err(e) => {
                tracing::warn!(error = %e, "search round failed");
                Duration::from_secs(30)
            }
        };
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(wait) => {}
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SearchRequest {
    /// Web search syntax (`"exact phrase"`, `-word`, `or`); at least 2 characters.
    pub query: String,
    /// At most 50 (the default).
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub entry: Uuid,
    pub path: String,
    pub score: f32,
    /// Text around the first match; absent past the decryption budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResponse {
    pub results: Vec<SearchHit>,
}

/// About 160 characters around the first word of the query found, located
/// on characters so that case changing a byte length never shifts it.
fn snippet(text: &str, query: &str) -> Option<String> {
    let fold = |c: char| c.to_lowercase().next().unwrap_or(c);
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| fold(*c)).collect();
    let at = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .filter_map(|w| {
            let w: Vec<char> = w.chars().map(fold).collect();
            lower.windows(w.len()).position(|win| win == w.as_slice())
        })
        .min()?;
    let piece: String = chars.iter().skip(at.saturating_sub(80)).take(160).collect();
    Some(piece.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Full-text and file-name search in one vault (§6.7): the query travels in
/// the body, never the URL (T19).
#[utoipa::path(
    post,
    path = "/vaults/{v}/search",
    tag = "search",
    params(("v" = Uuid, Path, description = "Vault id")),
    request_body = SearchRequest,
    responses(
        (status = 200, description = "At most 50 results, best first", body = SearchResponse),
        (status = 400, description = "A query under 2 characters", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "search_disabled: the tenant has no index", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn search(
    State(state): State<SharedState>,
    reader: Reader,
    Path(v): Path<Uuid>,
    Body(body): Body<SearchRequest>,
) -> Result<Json<SearchResponse>, ApiError> {
    let ra = reader.access(&state, v, VaultRole::Viewer).await?;
    let enabled = state.snapshot().is_some_and(|s| {
        s.config
            .tenants
            .get(ra.access.tenant_slug.as_str())
            .is_some_and(|t| t.doc.spec.search.enabled)
    });
    if !enabled {
        return Err(ApiError::of(ProblemType::SearchDisabled));
    }
    let query = body.query.trim();
    if query.chars().count() < 2 || query.len() > MAX_QUERY {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "query: 2 characters to 256 bytes",
        ));
    }
    let limit = i64::from(body.limit.unwrap_or(MAX_RESULTS).clamp(1, MAX_RESULTS));
    let mut tx =
        crate::db::begin_tenant_snapshot(&state.db, ra.access.tenant, &reader.subject()).await?;
    // Words in either language, then file names by trigram similarity.
    let rows = sqlx::query(
        "WITH q AS (SELECT websearch_to_tsquery('kura_french', $2) AS fr, websearch_to_tsquery('kura_english', $2) AS en),
         words AS (
             SELECT e.id, e.path, r.blob_hash, r.size,
                    greatest(ts_rank(d.lex_fr, q.fr), ts_rank(d.lex_en, q.en)) AS score
             FROM search_doc d CROSS JOIN q
             JOIN entry e ON e.vault_id = d.vault_id AND e.id = d.entry_id
             JOIN revision r ON r.id = e.head_revision
             WHERE d.vault_id = $1 AND e.namespace = 'notes' AND NOT e.deleted AND e.path_key LIKE $3 ESCAPE '\\'
               AND (d.lex_fr @@ q.fr OR d.lex_en @@ q.en)
             ORDER BY score DESC LIMIT $4
         ),
         names AS (
             SELECT e.id, e.path, r.blob_hash, r.size, (similarity(e.path, $2) / 2)::real AS score
             FROM entry e JOIN revision r ON r.id = e.head_revision
             WHERE e.vault_id = $1 AND e.namespace = 'notes' AND NOT e.deleted AND e.path_key LIKE $3 ESCAPE '\\'
               AND e.path % $2
             ORDER BY score DESC LIMIT $4
         )
         SELECT DISTINCT ON (id) id, path, blob_hash, size, score
         FROM (SELECT * FROM words UNION ALL SELECT * FROM names) h
         ORDER BY id, score DESC",
    )
    .bind(v)
    .bind(query)
    .bind(authz::like_prefix(&ra.prefix_key))
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut hits: Vec<(SearchHit, Option<ChunkHash>, i64)> = rows
        .iter()
        .map(|r| {
            let raw: Option<Vec<u8>> = r.try_get("blob_hash")?;
            Ok((
                SearchHit {
                    entry: r.try_get("id")?,
                    path: r.try_get("path")?,
                    score: r.try_get("score")?,
                    snippet: None,
                },
                raw.and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
                    .map(ChunkHash::from_bytes),
                r.try_get::<i64, _>("size")?,
            ))
        })
        .collect::<Result<_, ApiError>>()?;
    hits.sort_by(|a, b| b.0.score.total_cmp(&a.0.score));
    hits.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    // Snippets from the best results first, within 1 MiB decrypted (§6.7).
    let mut budget = SNIPPET_BUDGET;
    for (hit, blob, size) in &mut hits {
        let Some(blob) = blob else { continue };
        if !indexable(&hit.path) || *size > budget {
            continue;
        }
        budget = budget.saturating_sub(*size);
        if let Ok(bytes) = read_blob(&state, &ra.access, v, *blob).await {
            hit.snippet = snippet(&cut(&bytes), query);
        }
    }
    Ok(Json(SearchResponse {
        results: hits.into_iter().map(|(h, _, _)| h).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_only_cut_on_a_character() {
        assert!(indexable("a/b.md") && indexable("X.MD"));
        assert!(!indexable("a.canvas") && !indexable("md") && !indexable("a.md.png"));
        let text = "é".repeat(MAX_INDEXED);
        let c = cut(text.as_bytes());
        assert!(c.len() <= MAX_INDEXED && c.chars().all(|ch| ch == 'é'));
    }

    #[test]
    fn snippets_surround_the_first_word_found() {
        let text = format!(
            "{} the budget for Q3 is approved {}",
            "x ".repeat(100),
            "y ".repeat(100)
        );
        let s = snippet(&text, "budget approved").unwrap();
        assert!(s.contains("budget for Q3 is approved"), "{s}");
        assert!(s.chars().count() <= 160);
        assert_eq!(snippet("nothing here", "zz"), None);
        // A character whose lower case is longer in bytes comes before the match.
        let s = snippet(&format!("İ{}needle", " ".repeat(10)), "needle").unwrap();
        assert!(s.ends_with("needle"), "{s}");
    }
}
