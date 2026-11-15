//! Authorization from the reconciled configuration (RFC 0001 §4.2, §6.2).
//! Reads only the stored snapshot, never this replica's files (decision 31).

use kura_api::TenantRole;
use uuid::Uuid;

use crate::auth::Principal;
use crate::reconcile::snapshot::Snapshot;

/// The caller's role in one enabled tenant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub slug: String,
    pub tenant: Uuid,
    pub role: TenantRole,
}

/// Every enabled tenant the caller belongs to, with the highest role its
/// claims map to. A subject bound to one of the tenant's file-declared vaults
/// is a `member` of that tenant.
pub fn memberships(snapshot: &Snapshot, p: &Principal) -> Vec<Membership> {
    let mut out = Vec::new();
    for (slug, t) in &snapshot.config.tenants {
        let Some(&tenant) = snapshot.tenant_ids.get(slug.as_str()) else {
            continue;
        };
        let mut role: Option<TenantRole> = None;
        if let Some(m) = &t.doc.spec.claim_mappings {
            let values = p.claim_values(&m.claim);
            let has = |list: &[String]| list.iter().any(|v| values.contains(&v.as_str()));
            for (r, list) in [
                (TenantRole::Owner, &m.roles.owner),
                (TenantRole::Admin, &m.roles.admin),
                (TenantRole::Member, &m.roles.member),
            ] {
                if has(list) {
                    role = role.max(Some(r));
                }
            }
        }
        if role.is_none() {
            let bound = snapshot.config.vaults.values().any(|v| {
                v.doc.spec.tenant_ref.name == slug.as_str()
                    && v.doc
                        .spec
                        .bindings
                        .iter()
                        .any(|b| b.subject.as_deref() == Some(p.sub.as_str()))
            });
            if bound {
                role = Some(TenantRole::Member);
            }
        }
        if let Some(role) = role {
            out.push(Membership {
                slug: slug.to_string(),
                tenant,
                role,
            });
        }
    }
    out
}

use kura_api::VaultRole;
use sqlx::Row as _;

use crate::db;
use crate::problem::{ApiError, ProblemType};

/// The tenants a device of the caller is linked to: its memberships, and
/// every tenant where a vault binding (from the files or the API, to its
/// subject or to one of its claim values) opens a vault to it. Without the
/// latter, a user who reaches a vault only through a binding could not
/// register a device, and the tenant's admins would not see or revoke it
/// (review 2026-11-15 #14).
pub async fn device_tenants(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    p: &Principal,
) -> Result<Vec<Uuid>, ApiError> {
    let mut out: Vec<Uuid> = memberships(snapshot, p).iter().map(|m| m.tenant).collect();
    for (slug, &tenant) in &snapshot.tenant_ids {
        if out.contains(&tenant) {
            continue;
        }
        let values: Vec<&str> = snapshot
            .config
            .tenants
            .get(slug.as_str())
            .and_then(|t| t.doc.spec.claim_mappings.as_ref())
            .map(|m| p.claim_values(&m.claim))
            .unwrap_or_default();
        let mut tx = db::begin_tenant(pool, tenant, Some(&p.sub)).await?;
        let bound: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM role_binding b JOIN vault v ON v.id = b.vault_id
                 WHERE v.disabled_at IS NULL
                   AND ((b.subject_kind = 'user' AND b.subject = $1)
                        OR (b.subject_kind = 'claim' AND b.subject = ANY($2))))",
        )
        .bind(&p.sub)
        .bind(&values)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        if bound {
            out.push(tenant);
        }
    }
    Ok(out)
}

/// What a tenant role grants on every vault of the tenant (§6.2): `owner`
/// and `admin` open every vault as a manager; `member` only through bindings.
pub const fn tenant_vault_role(role: TenantRole) -> Option<VaultRole> {
    match role {
        TenantRole::Owner | TenantRole::Admin => Some(VaultRole::Manager),
        TenantRole::Member => None,
    }
}

/// A vault the caller may open, and with which rights.
#[derive(Debug, Clone)]
pub struct VaultAccess {
    pub vault: Uuid,
    pub tenant: Uuid,
    pub tenant_slug: String,
    /// The tenant's mapped claim, which `claim` bindings match against.
    pub claim: Option<String>,
    pub namespace: String,
    pub name: String,
    pub managed_by: String,
    pub tenant_role: Option<TenantRole>,
    /// The highest of the tenant role's grant and every matching binding (§4.2).
    pub role: VaultRole,
    /// Fixed at creation; clients read it from `GET /vaults` (§6.4).
    pub chunking: kura_api::Chunking,
    pub epoch: Uuid,
    pub head_seq: i64,
    /// The `configSync` categories; empty: `.obsidian` does not sync (§6.8).
    pub config: Vec<kura_api::spec::ConfigCategory>,
}

fn config_of(row: &sqlx::postgres::PgRow) -> Result<Vec<kura_api::spec::ConfigCategory>, ApiError> {
    let raw: Option<serde_json::Value> = row.try_get("config_sync")?;
    Ok(raw
        .and_then(|v| serde_json::from_value::<kura_api::spec::ConfigSync>(v).ok())
        .map(|c| c.categories)
        .unwrap_or_default())
}

pub fn parse_role(s: &str) -> Option<VaultRole> {
    match s {
        "viewer" => Some(VaultRole::Viewer),
        "editor" => Some(VaultRole::Editor),
        "manager" => Some(VaultRole::Manager),
        _ => None,
    }
}

/// The caller's access to a vault of a known tenant, or `None` when the vault
/// is missing, disabled, or unreadable to the caller.
pub async fn vault_access_in(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    p: &Principal,
    tenant: Uuid,
    vault: Uuid,
) -> Result<Option<VaultAccess>, ApiError> {
    Ok(vault_accesses(pool, snapshot, p, tenant, Some(vault), 1)
        .await?
        .into_iter()
        .next())
}

/// Every enabled vault of a tenant the caller reads, by namespace and name, at
/// most `limit`: two queries in one transaction, whatever the count (review R6).
pub async fn vault_accesses_in(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    p: &Principal,
    tenant: Uuid,
    limit: i64,
) -> Result<Vec<VaultAccess>, ApiError> {
    vault_accesses(pool, snapshot, p, tenant, None, limit).await
}

async fn vault_accesses(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    p: &Principal,
    tenant: Uuid,
    only: Option<Uuid>,
    limit: i64,
) -> Result<Vec<VaultAccess>, ApiError> {
    let Some((slug, _)) = snapshot.tenant_ids.iter().find(|(_, id)| **id == tenant) else {
        return Ok(Vec::new());
    };
    let claim = snapshot
        .config
        .tenants
        .get(slug.as_str())
        .and_then(|t| t.doc.spec.claim_mappings.as_ref())
        .map(|m| m.claim.clone());
    let tenant_role = memberships(snapshot, p)
        .into_iter()
        .find(|m| m.tenant == tenant)
        .map(|m| m.role);
    let values = claim
        .as_deref()
        .map(|c| p.claim_values(c))
        .unwrap_or_default();
    let every = tenant_role.and_then(tenant_vault_role).is_some();
    let mut tx = db::begin_tenant(pool, tenant, Some(&p.sub)).await?;
    // Only the vaults the caller may open, before the limit (review
    // 2026-11-15 #19): the role is settled below.
    let rows = sqlx::query(
        "SELECT id, namespace, name, managed_by, chunking_params, epoch, head_seq, config_sync FROM vault
         WHERE disabled_at IS NULL AND ($1::uuid IS NULL OR id = $1)
           AND ($3 OR EXISTS (SELECT 1 FROM role_binding b WHERE b.vault_id = vault.id
                AND ((b.subject_kind = 'user' AND b.subject = $4) OR (b.subject_kind = 'claim' AND b.subject = ANY($5)))))
         ORDER BY namespace, name LIMIT $2",
    )
    .bind(only)
    .bind(limit)
    .bind(every)
    .bind(&p.sub)
    .bind(&values)
    .fetch_all(&mut *tx)
    .await?;
    let ids: Vec<Uuid> = rows
        .iter()
        .map(|r| r.try_get("id"))
        .collect::<Result<_, _>>()?;
    let bindings = sqlx::query(
        "SELECT vault_id, subject_kind, subject, role FROM role_binding WHERE vault_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut out = Vec::new();
    for row in &rows {
        let vault: Uuid = row.try_get("id")?;
        let mut role = tenant_role.and_then(tenant_vault_role);
        for b in &bindings {
            let (v, kind, subject, r): (Uuid, String, String, String) = (
                b.try_get("vault_id")?,
                b.try_get("subject_kind")?,
                b.try_get("subject")?,
                b.try_get("role")?,
            );
            let matches = v == vault
                && match kind.as_str() {
                    "claim" => values.contains(&subject.as_str()),
                    "user" => subject == p.sub,
                    _ => false,
                };
            if matches {
                role = role.max(parse_role(&r));
            }
        }
        let Some(role) = role else { continue };
        out.push(VaultAccess {
            vault,
            tenant,
            tenant_slug: slug.clone(),
            claim: claim.clone(),
            namespace: row.try_get("namespace")?,
            name: row.try_get("name")?,
            managed_by: row.try_get("managed_by")?,
            tenant_role,
            role,
            chunking: serde_json::from_value(row.try_get("chunking_params")?)
                .map_err(|e| ApiError::Unavailable(format!("stored chunking refused: {e}")))?,
            epoch: row.try_get("epoch")?,
            head_seq: row.try_get("head_seq")?,
            config: config_of(row)?,
        });
    }
    Ok(out)
}

/// `authz::require_role`: checked on the vault before any work. A vault the
/// caller cannot read answers `404`, exactly as a missing one (T4).
pub async fn require_role(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    p: &Principal,
    vault: Uuid,
    min: VaultRole,
) -> Result<VaultAccess, ApiError> {
    let tenant = db::vault_tenant(pool, vault).await?;
    let access = match tenant {
        Some(t) => vault_access_in(pool, snapshot, p, t, vault).await?,
        None => None,
    };
    let access = access.ok_or(ApiError::of(ProblemType::NotFound))?;
    if access.role < min {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            format!(
                "{} required, you are {}",
                role_name(min),
                role_name(access.role)
            ),
        ));
    }
    Ok(access)
}

pub const fn role_name(r: VaultRole) -> &'static str {
    match r {
        VaultRole::Viewer => "viewer",
        VaultRole::Editor => "editor",
        VaultRole::Manager => "manager",
    }
}

/// A binding a manager or admin asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingSubject {
    /// A value of the tenant's mapped claim.
    Claim(String),
    /// An OIDC `sub`.
    User(String),
}

impl BindingSubject {
    /// `claim:<value>` or `user:<sub>`, the `{b}` of `/vaults/{v}/bindings/{b}`.
    pub fn parse(s: &str) -> Option<Self> {
        let (kind, value) = s.split_once(':')?;
        if value.is_empty() || value.len() > 256 {
            return None;
        }
        match kind {
            "claim" => Some(Self::Claim(value.to_owned())),
            "user" => Some(Self::User(value.to_owned())),
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Claim(_) => "claim",
            Self::User(_) => "user",
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::Claim(v) | Self::User(v) => v,
        }
    }
}

/// `authz::check_binding` (§6.2, T15): only a vault `manager` or tenant
/// `admin` binds; only a claim value from the tenant's own `claimMappings`, or
/// the `sub` of a known tenant member; never a role above the caller's own.
pub async fn check_binding(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    access: &VaultAccess,
    subject: &BindingSubject,
    role: VaultRole,
) -> Result<(), ApiError> {
    let can_manage = access.role == VaultRole::Manager
        || access.tenant_role.is_some_and(|r| r >= TenantRole::Admin);
    if !can_manage {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "manager required",
        ));
    }
    if role > access.role {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "cannot grant a role above your own",
        ));
    }
    let known = match subject {
        BindingSubject::Claim(value) => snapshot
            .config
            .tenants
            .get(access.tenant_slug.as_str())
            .and_then(|t| t.doc.spec.claim_mappings.as_ref())
            .is_some_and(|m| {
                [&m.roles.owner, &m.roles.admin, &m.roles.member]
                    .iter()
                    .any(|l| l.contains(value))
            }),
        BindingSubject::User(sub) => {
            let mut tx = db::begin_tenant(pool, access.tenant, None).await?;
            let known: bool = sqlx::query_scalar("SELECT kura_tenant_has_member($1)")
                .bind(sub)
                .fetch_one(&mut *tx)
                .await?;
            tx.commit().await?;
            known
        }
    };
    if !known {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "not a claim value or member of this tenant",
        ));
    }
    Ok(())
}

/// Full Unicode case folding then NFC: the one comparison form of paths and
/// token prefixes (§6.3), so a prefix check and a uniqueness check agree.
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    caseless::default_case_fold_str(s).nfc().collect()
}

/// `authz::visible_under_prefix` (§6.2, T16): whether a path is under a
/// token's folded prefix; an empty prefix sees everything.
pub fn visible_under_prefix(path: &str, prefix_key: &str) -> bool {
    prefix_key.is_empty() || fold(path).starts_with(prefix_key)
}

/// A folded prefix as a SQL `LIKE` pattern on `path_key`, wildcards escaped.
pub fn like_prefix(prefix_key: &str) -> String {
    let mut out = String::with_capacity(prefix_key.len().saturating_add(1));
    for c in prefix_key.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

/// A claim value of the caller that the tenant maps to `owner` or `admin`:
/// what a token issued through the tenant role depends on.
pub fn tenant_grant_claim(snapshot: &Snapshot, tenant: Uuid, p: &Principal) -> Option<String> {
    let (slug, _) = snapshot.tenant_ids.iter().find(|(_, id)| **id == tenant)?;
    let m = snapshot
        .config
        .tenants
        .get(slug.as_str())?
        .doc
        .spec
        .claim_mappings
        .as_ref()?;
    let values = p.claim_values(&m.claim);
    m.roles
        .owner
        .iter()
        .chain(&m.roles.admin)
        .find(|v| values.contains(&v.as_str()))
        .cloned()
}

/// Whether the tenant still maps this claim value to `owner` or `admin`.
pub fn claim_grants_manager(snapshot: &Snapshot, tenant: Uuid, value: &str) -> bool {
    let Some((slug, _)) = snapshot.tenant_ids.iter().find(|(_, id)| **id == tenant) else {
        return false;
    };
    snapshot
        .config
        .tenants
        .get(slug.as_str())
        .and_then(|t| t.doc.spec.claim_mappings.as_ref())
        .is_some_and(|m| {
            m.roles
                .owner
                .iter()
                .chain(&m.roles.admin)
                .any(|v| v == value)
        })
}

/// A scoped token's access to its vault: always `viewer` (read only, §6.2).
pub async fn key_access(
    pool: &sqlx::PgPool,
    snapshot: &Snapshot,
    tenant: Uuid,
    vault: Uuid,
) -> Result<Option<VaultAccess>, ApiError> {
    let Some((slug, _)) = snapshot.tenant_ids.iter().find(|(_, id)| **id == tenant) else {
        return Ok(None);
    };
    let mut tx = db::begin_tenant(pool, tenant, None).await?;
    let row = sqlx::query(
        "SELECT namespace, name, managed_by, chunking_params, epoch, head_seq, config_sync FROM vault WHERE id = $1 AND disabled_at IS NULL",
    )
    .bind(vault)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    let Some(row) = row else { return Ok(None) };
    Ok(Some(VaultAccess {
        vault,
        tenant,
        tenant_slug: slug.clone(),
        claim: None,
        namespace: row.try_get("namespace")?,
        name: row.try_get("name")?,
        managed_by: row.try_get("managed_by")?,
        tenant_role: None,
        role: VaultRole::Viewer,
        chunking: serde_json::from_value(row.try_get("chunking_params")?)
            .map_err(|e| ApiError::Unavailable(format!("stored chunking refused: {e}")))?,
        epoch: row.try_get("epoch")?,
        head_seq: row.try_get("head_seq")?,
        config: config_of(&row)?,
    }))
}
