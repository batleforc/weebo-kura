//! Scoped tokens (RFC 0001 §6.2, §4.4): issued, listed and revoked by a
//! vault manager or tenant admin. Bound to addresses, expiring within 90
//! days, limited to 1 to 16 folder rules, each `read` or `write`
//! (RFC 0004); the secret is returned once and stored as its SHA-256.

use std::net::IpAddr;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kura_api::VaultRole;
use kura_core::VaultPath;
use kura_core::token::{SECRET_BYTES, ScopedToken};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sqlx::Row as _;
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::device::DeviceCaller;
use crate::authz::{self, PathScope, VaultAccess};
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};

/// The longest life of a token (§6.2).
pub const MAX_DAYS: u32 = 90;
/// Addresses per token.
pub const MAX_CIDRS: usize = 32;
/// Folder rules per token (RFC 0004 decision 8).
pub const MAX_SCOPES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScopeMode {
    Read,
    /// Also reads that folder, and nothing else (decision 9).
    Write,
}

/// One folder rule: the folder and everything under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeRule {
    /// A folder ending with `/`; `""` is the whole vault, `read` only.
    pub path: String,
    pub mode: ScopeMode,
}

/// The scope a list of rules means; `None` when it means none (T7).
pub fn scope_of(rules: &[ScopeRule]) -> Option<PathScope> {
    PathScope::from_rules(
        rules
            .iter()
            .map(|r| (r.path.as_str(), r.mode == ScopeMode::Write)),
    )
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateToken {
    /// 1 to 64 bytes, shown in lists and conflict file names; absent:
    /// `key-` and the first 8 characters of the id.
    #[serde(default)]
    pub name: Option<String>,
    /// One `read` rule, as before RFC 0004; refused together with `scopes`.
    /// Neither: the whole vault, read only.
    #[serde(default)]
    pub prefix: Option<String>,
    /// 1 to 16 folder rules.
    #[serde(default)]
    pub scopes: Option<Vec<ScopeRule>>,
    /// 1 to 90.
    pub expires_in_days: u32,
    /// Client addresses allowed, at least one; never `0.0.0.0/0` or `::/0`.
    pub cidrs: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenView {
    pub id: Uuid,
    pub name: String,
    pub scopes: Vec<ScopeRule>,
    /// As before RFC 0004: the folder of a token with one `read` rule, `/`
    /// (which matches no path) for any other.
    pub prefix: String,
    /// `write` when a rule is.
    pub mode: String,
    pub cidrs: Vec<String>,
    /// RFC 3339.
    pub expires_at: String,
    pub created_by: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssuedToken {
    /// `kura_<id>_<secret>`: shown once, never stored in clear.
    pub token: String,
    #[serde(flatten)]
    pub view: TokenView,
}

fn rfc3339(t: time::OffsetDateTime) -> Result<String, ApiError> {
    t.format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| ApiError::Unavailable(e.to_string()))
}

fn invalid(detail: impl Into<String>) -> ApiError {
    ApiError::with_detail(ProblemType::InvalidRequest, detail)
}

/// `""`, or a valid vault path followed by `/`.
fn check_prefix(p: &str) -> Result<(), ApiError> {
    if p.is_empty() {
        return Ok(());
    }
    let folder = p
        .strip_suffix('/')
        .ok_or_else(|| invalid("prefix: a folder ending with /"))?;
    VaultPath::parse(folder).map_err(|e| invalid(format!("prefix: {e}")))?;
    Ok(())
}

/// The rules a creation request means (RFC 0004 §4.2, §4.3).
fn check_scopes(
    prefix: Option<String>,
    scopes: Option<Vec<ScopeRule>>,
) -> Result<Vec<ScopeRule>, ApiError> {
    let rules = match (prefix, scopes) {
        (Some(_), Some(_)) => return Err(invalid("prefix and scopes: give one of them")),
        (prefix, None) => vec![ScopeRule {
            path: prefix.unwrap_or_default(),
            mode: ScopeMode::Read,
        }],
        (None, Some(rules)) => rules,
    };
    if rules.is_empty() || rules.len() > MAX_SCOPES {
        return Err(invalid("scopes: 1 to 16 folder rules"));
    }
    let mut seen = std::collections::HashSet::new();
    for r in &rules {
        check_prefix(&r.path)?;
        if r.path.is_empty() && r.mode == ScopeMode::Write {
            return Err(invalid(
                "scopes: write needs a folder; a token never writes the whole vault",
            ));
        }
        if !seen.insert(authz::fold(&r.path)) {
            return Err(invalid(format!(
                "scopes: {:?} is given twice",
                r.path.as_str()
            )));
        }
    }
    Ok(rules)
}

/// Trimmed, 1 to 64 bytes, usable in a file name (it suffixes conflict
/// files); absent: named after the id.
fn check_name(name: Option<&str>, id: Uuid) -> Result<String, ApiError> {
    let Some(name) = name.map(str::trim) else {
        return Ok(format!(
            "key-{}",
            id.simple().to_string().get(..8).unwrap_or("")
        ));
    };
    // It suffixes conflict files, like a device name: it must make a valid
    // file name there.
    let fits = VaultPath::parse("x.md")
        .and_then(|p| p.suffixed(&format!("conflict {name} 2026-11-17")))
        .is_ok();
    // A `/` would pass as a path and put the conflict file in a folder; a
    // control character has no place in a file name.
    if name.is_empty()
        || name.len() > 64
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
        || !fits
    {
        return Err(invalid("name: 1 to 64 bytes, usable in a file name"));
    }
    Ok(name.to_owned())
}

/// Parsed and normalized; refuses an empty list and any `/0` (§6.2).
fn check_cidrs(list: &[String]) -> Result<Vec<String>, ApiError> {
    if list.is_empty() || list.len() > MAX_CIDRS {
        return Err(invalid(
            "cidrs: 1 to 32 address ranges; a scoped token is always bound to addresses",
        ));
    }
    list.iter()
        .map(|c| {
            let net = c
                .parse::<ipnet::IpNet>()
                .or_else(|_| c.parse::<IpAddr>().map(ipnet::IpNet::from))
                .map_err(|_| invalid(format!("cidrs: {c} is not an address range")))?;
            if net.prefix_len() == 0 {
                return Err(invalid(format!("cidrs: {c} allows every address")));
            }
            Ok(net.trunc().to_string())
        })
        .collect()
}

async fn manage(
    state: &SharedState,
    caller: &DeviceCaller,
    v: Uuid,
) -> Result<VaultAccess, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    authz::require_role(
        &state.db,
        &snap,
        &caller.caller.principal,
        v,
        VaultRole::Manager,
    )
    .await
}

fn view(r: &sqlx::postgres::PgRow) -> Result<TokenView, ApiError> {
    let last: Option<time::OffsetDateTime> = r.try_get("last_used_at")?;
    let revoked: Option<time::OffsetDateTime> = r.try_get("revoked_at")?;
    Ok(TokenView {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        scopes: serde_json::from_value(r.try_get("scopes")?)
            .map_err(|e| ApiError::Unavailable(format!("stored scopes refused: {e}")))?,
        prefix: r.try_get("path_prefix")?,
        mode: r.try_get("mode")?,
        cidrs: r.try_get("cidrs")?,
        expires_at: rfc3339(r.try_get("expires_at")?)?,
        created_by: r.try_get("created_by")?,
        last_used_at: last.map(rfc3339).transpose()?,
        revoked: revoked.is_some(),
    })
}

/// Issues a token for this vault; the secret is in this answer only.
#[utoipa::path(
    post,
    path = "/vaults/{v}/tokens",
    tag = "tokens",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = CreateToken,
    responses(
        (status = 201, description = "The token, shown once", body = IssuedToken),
        (status = 400, description = "No address range, a /0 range, a bad prefix, rule, name or expiry", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Below manager", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "token_write_disabled: a write rule, and the tenant does not allow tokens to write", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn create_token(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
    Body(body): Body<CreateToken>,
) -> Result<(StatusCode, Json<IssuedToken>), ApiError> {
    let a = manage(&state, &caller, v).await?;
    let rules = check_scopes(body.prefix, body.scopes)?;
    let scope = scope_of(&rules).ok_or_else(|| invalid("scopes: 1 to 16 folder rules"))?;
    let cidrs = check_cidrs(&body.cidrs)?;
    if !(1..=MAX_DAYS).contains(&body.expires_in_days) {
        return Err(invalid("expiresInDays: 1 to 90"));
    }
    let p = &caller.caller.principal;
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    // The grant the token lives by: the tenant role's claim value, else the
    // binding that makes the caller a manager (§6.2).
    if scope.can_write() && !authz::token_write_allowed(&snap, a.tenant) {
        return Err(ApiError::with_detail(
            ProblemType::TokenWriteDisabled,
            "this tenant does not allow tokens to write (scopedTokens.write)",
        ));
    }
    let claim = authz::tenant_grant_claim(&snap, a.tenant, p);
    let mut tx = db::begin_tenant(&state.db, a.tenant, Some(&p.sub)).await?;
    let binding: Option<Uuid> = match &claim {
        Some(_) => None,
        None => {
            let values: Vec<&str> = a
                .claim
                .as_deref()
                .map(|c| p.claim_values(c))
                .unwrap_or_default();
            sqlx::query_scalar(
                "SELECT id FROM role_binding WHERE vault_id = $1 AND role = 'manager'
                   AND ((subject_kind = 'user' AND subject = $2) OR (subject_kind = 'claim' AND subject = ANY($3)))
                 LIMIT 1",
            )
            .bind(v)
            .bind(&p.sub)
            .bind(&values)
            .fetch_optional(&mut *tx)
            .await?
        }
    };
    if claim.is_none() && binding.is_none() {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "manager required",
        ));
    }
    let mut secret = [0u8; SECRET_BYTES];
    aws_lc_rs::rand::fill(&mut secret)
        .map_err(|_| ApiError::Unavailable("no randomness".into()))?;
    let token = ScopedToken::new(Uuid::now_v7(), secret);
    let name = check_name(body.name.as_deref(), token.id)?;
    // What a binary from before RFC 0004 reads: the folder of a lone read
    // rule, else `/`, which no path starts with (T8).
    let legacy = match rules.as_slice() {
        [only] if only.mode == ScopeMode::Read => only.path.as_str(),
        _ => "/",
    };
    let mode = if scope.can_write() { "write" } else { "read" };
    let hash: [u8; 32] = Sha256::digest(token.secret()).into();
    let row = sqlx::query(
        "INSERT INTO scoped_token (id, tenant_id, vault_id, path_prefix, allowed_cidrs, expires_at, token_hash, created_by, granted_by_binding, granted_by_claim, name, scopes, mode)
         VALUES ($1, $2, $3, $4, $5::cidr[], now() + make_interval(days => $6), $7, $8, $9, $10, $11, $12, $13)
         RETURNING id, name, scopes, path_prefix, mode, allowed_cidrs::text[] AS cidrs, expires_at, created_by, last_used_at, revoked_at",
    )
    .bind(token.id)
    .bind(a.tenant)
    .bind(v)
    .bind(legacy)
    .bind(&cidrs)
    .bind(i32::try_from(body.expires_in_days).unwrap_or(1))
    .bind(hash.as_slice())
    .bind(&p.sub)
    .bind(binding)
    .bind(&claim)
    .bind(&name)
    .bind(sqlx::types::Json(&rules))
    .bind(mode)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(target: "audit", event = "scoped_token_created", sub = %p.sub, vault = %v, token = %token.id, rules = rules.len(), write = scope.can_write());
    Ok((
        StatusCode::CREATED,
        Json(IssuedToken {
            token: token.expose(),
            view: view(&row)?,
        }),
    ))
}

/// The vault's tokens, newest first; never a secret.
#[utoipa::path(
    get,
    path = "/vaults/{v}/tokens",
    tag = "tokens",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "At most 1,000 tokens", body = [TokenView]),
        (status = 403, description = "Below manager", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn list_tokens(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
) -> Result<Json<Vec<TokenView>>, ApiError> {
    let a = manage(&state, &caller, v).await?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let rows = sqlx::query(
        "SELECT id, name, scopes, path_prefix, mode, allowed_cidrs::text[] AS cidrs, expires_at, created_by, last_used_at, revoked_at
         FROM scoped_token WHERE vault_id = $1 ORDER BY created_at DESC LIMIT 1000",
    )
    .bind(v)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    rows.iter().map(view).collect::<Result<_, _>>().map(Json)
}

/// Revokes a token at once, on every replica (the next use is refused).
#[utoipa::path(
    delete,
    path = "/vaults/{v}/tokens/{t}",
    tag = "tokens",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("t" = Uuid, Path, description = "Token id"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    responses(
        (status = 204, description = "Revoked"),
        (status = 403, description = "Below manager", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault or token", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn revoke_token(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, t)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let a = manage(&state, &caller, v).await?;
    let mut tx = db::begin_tenant(&state.db, a.tenant, None).await?;
    let done = sqlx::query(
        "UPDATE scoped_token SET revoked_at = coalesce(revoked_at, now()) WHERE id = $1 AND vault_id = $2",
    )
    .bind(t)
    .bind(v)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    if done == 0 {
        return Err(ApiError::of(ProblemType::NotFound));
    }
    tracing::info!(target: "audit", event = "scoped_token_revoked", sub = %caller.caller.principal.sub, vault = %v, token = %t);
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_and_ranges_are_checked() {
        assert!(check_prefix("").is_ok());
        assert!(check_prefix("agents/").is_ok());
        assert!(check_prefix("a/b/").is_ok());
        for bad in ["agents", "/", "../x/", ".obsidian/", "a//"] {
            assert!(check_prefix(bad).is_err(), "{bad}");
        }
        assert_eq!(
            check_cidrs(&["10.42.1.7/16".into(), "2001:db8::1".into()]).unwrap(),
            ["10.42.0.0/16", "2001:db8::1/128"]
        );
        for bad in [
            vec![],
            vec!["0.0.0.0/0".into()],
            vec!["::/0".into()],
            vec!["nope".into()],
        ] {
            assert!(check_cidrs(&bad).is_err(), "{bad:?}");
        }
    }

    fn rule(path: &str, mode: ScopeMode) -> ScopeRule {
        ScopeRule {
            path: path.into(),
            mode,
        }
    }

    /// Every row of RFC 0004 §4.3 that needs no tenant.
    #[test]
    fn scopes_are_checked() {
        use ScopeMode::{Read, Write};
        // Neither: the whole vault, read only. `prefix`: one read rule.
        assert_eq!(check_scopes(None, None).unwrap(), [rule("", Read)]);
        assert_eq!(
            check_scopes(Some("a/".into()), None).unwrap(),
            [rule("a/", Read)]
        );
        let ok = vec![rule("", Read), rule("Out/", Write)];
        assert_eq!(check_scopes(None, Some(ok.clone())).unwrap(), ok);
        let many = |n: usize| (0..n).map(|i| rule(&format!("f{i}/"), Read)).collect();
        assert!(check_scopes(None, Some(many(16))).is_ok());
        for (bad, why) in [
            (Some(vec![]), "an empty list never reads as everything"),
            (Some(many(17)), "more than 16"),
            (Some(vec![rule("a", Read)]), "no trailing /"),
            (Some(vec![rule("../a/", Read)]), "not a vault path"),
            (Some(vec![rule("", Write)]), "whole-vault write"),
            (
                Some(vec![rule("A/", Read), rule("a/", Write)]),
                "equal once folded",
            ),
        ] {
            assert!(check_scopes(None, bad).is_err(), "{why}");
        }
        assert!(
            check_scopes(Some("a/".into()), Some(vec![rule("a/", Read)])).is_err(),
            "prefix and scopes"
        );
        let unknown: Result<ScopeRule, _> =
            serde_json::from_str(r#"{"path":"a/","mode":"read","deny":true}"#);
        assert!(unknown.is_err(), "unknown field");
    }

    #[test]
    fn names_are_checked() {
        let id = Uuid::from_u128(0x019a_3f00_0000_7000_8000_0000_0000_0001);
        assert_eq!(check_name(None, id).unwrap(), "key-019a3f00");
        assert_eq!(
            check_name(Some(" discord-recap "), id).unwrap(),
            "discord-recap"
        );
        for bad in ["", "  ", "a/b", "a\nb", &"x".repeat(65)] {
            assert!(check_name(Some(bad), id).is_err(), "{bad:?}");
        }
    }
}
