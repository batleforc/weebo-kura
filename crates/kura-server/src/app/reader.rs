//! Who may read (RFC 0001 §4.4 "OIDC or key"): a signed-in device, or a
//! scoped token (§6.2, T5). A token is checked in `verify_key`: its hash in
//! constant time, its expiry and revocation, the grant it was issued under,
//! the client address against its CIDR list; each use is audited and
//! touches `last_used_at` at most once a minute.

use std::net::IpAddr;

use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;
use kura_api::VaultRole;
use kura_core::token::ScopedToken;
use sha2::{Digest as _, Sha256};
use sqlx::Row as _;
use uuid::Uuid;

use super::SharedState;
use super::device::{DeviceCaller, rate_limit};
use crate::authz::{self, VaultAccess};
use crate::problem::{ApiError, ProblemType};

/// A verified scoped token.
pub struct KeyCaller {
    pub id: Uuid,
    pub tenant: Uuid,
    pub vault: Uuid,
    /// The token's path prefix, folded (§6.3); empty for the whole vault.
    pub prefix_key: String,
}

pub enum Reader {
    Device(DeviceCaller),
    Key(KeyCaller),
}

/// Where a read may look: a vault, and for a token, under its prefix only.
pub struct ReadAccess {
    pub access: VaultAccess,
    /// Folded; empty when the whole vault is visible.
    pub prefix_key: String,
    /// The signed-in user, whose own configuration is visible (§6.8); none
    /// for a scoped token.
    pub subject: Option<String>,
}

impl ReadAccess {
    pub fn sees(&self, path: &str) -> bool {
        authz::visible_under_prefix(path, &self.prefix_key)
    }

    /// T16: a token under a prefix gets a blob (`chunk` false) or a chunk
    /// (`chunk` true) only through a revision made under its prefix, of an
    /// entry still under it. A chunk shared with files outside is readable
    /// through the inside one: the same bytes.
    pub async fn sees_content(
        &self,
        state: &SharedState,
        hash: &kura_core::ChunkHash,
        chunk: bool,
    ) -> Result<bool, ApiError> {
        // shortcut: the first 200 revisions naming the content are checked; a
        // chunk shared by more files outside the prefix may be refused (fails
        // closed). Index the content by entry if that ever bites.
        let sql = if chunk {
            "SELECT r.path, e.path AS head FROM revision r JOIN entry e ON e.vault_id = r.vault_id AND e.id = r.entry_id
             WHERE r.vault_id = $1 AND r.blob_hash IN (SELECT blob_hash FROM blob_chunk WHERE vault_id = $1 AND chunk_hash = $2)
             LIMIT 200"
        } else {
            "SELECT r.path, e.path AS head FROM revision r JOIN entry e ON e.vault_id = r.vault_id AND e.id = r.entry_id
             WHERE r.vault_id = $1 AND r.blob_hash = $2 LIMIT 200"
        };
        // Under the caller's subject: another user's configuration is out of
        // row-level security's sight, so its content is never served (T14).
        let mut tx =
            crate::db::begin_tenant(&state.db, self.access.tenant, self.subject.as_deref()).await?;
        let rows = sqlx::query(sql)
            .bind(self.access.vault)
            .bind(hash.as_bytes().as_slice())
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        for r in &rows {
            let (path, head): (String, String) = (r.try_get("path")?, r.try_get("head")?);
            if self.sees(&path) && self.sees(&head) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
}

/// A scoped token on a route that writes: read only (§6.2).
pub fn refuse_key_on_write(parts: &Parts) -> Result<(), ApiError> {
    if bearer(parts).is_some_and(|t| t.starts_with(kura_core::token::PREFIX)) {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "scoped tokens are read only",
        ));
    }
    Ok(())
}

impl FromRequestParts<SharedState> for Reader {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        match bearer(parts) {
            Some(t) if t.starts_with(kura_core::token::PREFIX) => {
                let t = t.to_owned();
                Ok(Self::Key(verify_key(parts, state, &t).await?))
            }
            _ => Ok(Self::Device(
                DeviceCaller::from_request_parts(parts, state).await?,
            )),
        }
    }
}

fn client(parts: &Parts, state: &SharedState) -> Option<IpAddr> {
    let peer = parts
        .extensions
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|c| c.0.ip())?;
    crate::net::client_addr_strict(peer, &parts.headers, &state.http.trusted_proxies)
}

async fn verify_key(parts: &Parts, state: &SharedState, text: &str) -> Result<KeyCaller, ApiError> {
    let invalid = || ApiError::of(ProblemType::InvalidToken);
    // Before any database work: random keys cost an address its own route
    // limit, not a query each (review 7 M2).
    let from = client(parts, state);
    let addr = from.map_or_else(|| "unknown".to_owned(), |ip| ip.to_string());
    rate_limit(state, parts, &format!("addr:{addr}"))?;
    let token = ScopedToken::parse(text).map_err(|_| invalid())?;
    let row = sqlx::query("SELECT * FROM kura_scoped_token_lookup($1)")
        .bind(token.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(invalid)?;
    let stored: Vec<u8> = row.try_get("token_hash")?;
    let presented: [u8; 32] = Sha256::digest(token.secret()).into();
    aws_lc_rs::constant_time::verify_slices_are_equal(&stored, &presented)
        .map_err(|_| invalid())?;
    let expires: time::OffsetDateTime = row.try_get("expires_at")?;
    let revoked: Option<time::OffsetDateTime> = row.try_get("revoked_at")?;
    if revoked.is_some() || expires <= time::OffsetDateTime::now_utc() {
        return Err(invalid());
    }
    let tenant: Uuid = row.try_get("tenant_id")?;
    // Revoked with the grant it was issued under (§6.2).
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let grant = match row.try_get::<Option<String>, _>("granted_by_claim")? {
        Some(claim) => authz::claim_grants_manager(&snap, tenant, &claim),
        None => row.try_get("binding_manager")?,
    };
    if !grant {
        return Err(invalid());
    }
    let cidrs: Vec<String> = row.try_get("allowed_cidrs")?;
    let allowed = from.is_some_and(|ip| {
        cidrs
            .iter()
            .filter_map(|c| c.parse::<ipnet::IpNet>().ok())
            .any(|n| n.contains(&ip))
    });
    if !allowed {
        return Err(ApiError::of(ProblemType::AddressNotAllowed));
    }
    rate_limit(state, parts, &format!("key:{}", token.id))?;
    sqlx::query("SELECT kura_scoped_token_touch($1)")
        .bind(token.id)
        .execute(&state.db)
        .await?;
    let route = parts
        .extensions
        .get::<axum::extract::MatchedPath>()
        .map_or("unmatched", axum::extract::MatchedPath::as_str);
    tracing::info!(target: "audit", event = "scoped_token_used", token = %token.id, %route, client = ?from);
    let prefix: String = row.try_get("path_prefix")?;
    Ok(KeyCaller {
        id: token.id,
        tenant,
        vault: row.try_get("vault_id")?,
        prefix_key: authz::fold(&prefix),
    })
}

impl Reader {
    /// The vault, readable at `min`; a token reads its own vault only, as a
    /// viewer, and another vault is `404` like any unreadable one (T4).
    pub async fn access(
        &self,
        state: &SharedState,
        v: Uuid,
        min: VaultRole,
    ) -> Result<ReadAccess, ApiError> {
        let snap = state
            .snapshot()
            .ok_or(ApiError::of(ProblemType::Unavailable))?;
        match self {
            Self::Device(d) => Ok(ReadAccess {
                access: authz::require_role(&state.db, &snap, &d.caller.principal, v, min).await?,
                prefix_key: String::new(),
                subject: Some(d.caller.principal.sub.clone()),
            }),
            Self::Key(k) => {
                if k.vault != v {
                    return Err(ApiError::of(ProblemType::NotFound));
                }
                let access = authz::key_access(&state.db, &snap, k.tenant, v)
                    .await?
                    .ok_or(ApiError::of(ProblemType::NotFound))?;
                if min > VaultRole::Viewer {
                    return Err(ApiError::with_detail(
                        ProblemType::InsufficientRole,
                        "scoped tokens are read only",
                    ));
                }
                Ok(ReadAccess {
                    access,
                    prefix_key: k.prefix_key.clone(),
                    subject: None,
                })
            }
        }
    }

    /// A signed-in device: its user and id; `None` for a scoped token.
    pub fn device(&self) -> Option<(&str, Uuid)> {
        match self {
            Self::Device(d) => Some((&d.caller.principal.sub, d.device)),
            Self::Key(_) => None,
        }
    }

    /// The subject for audit and transactions: the user, or the token.
    pub fn subject(&self) -> String {
        match self {
            Self::Device(d) => d.caller.principal.sub.clone(),
            Self::Key(k) => format!("key:{}", k.id),
        }
    }
}
