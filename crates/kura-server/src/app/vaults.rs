//! `/v1/vaults` and vault bindings (RFC 0001 §4.4, §6.2). Objects declared in
//! files are read-only here; API-made vaults and bindings are `managed_by =
//! api`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use kura_api::{DnsLabel, TenantRole, VaultRole};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::device::DeviceCaller;
use crate::authz::{self, BindingSubject, VaultAccess, role_name};
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};
use crate::reconcile::snapshot::Snapshot;

/// Vaults listed at most per call (§4.4, keyset paging arrives with the sync API).
pub const MAX_LISTED: i64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    pub id: Uuid,
    pub tenant: String,
    pub namespace: String,
    pub name: String,
    /// Your effective role: `viewer`, `editor` or `manager`.
    pub role: String,
    /// `config` (read-only here) or `api`.
    pub managed_by: String,
    /// FastCDC parameters in bytes; clients never use a compiled-in value (§6.4).
    pub chunking: ChunkingView,
    /// Send back with `changes?since=` (§6.3).
    pub epoch: Uuid,
    pub head_seq: u64,
    /// When `headSeq` was written, RFC 3339 (RFC 0005 §4.5, "updated");
    /// absent before the first commit or once that revision is purged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    /// The Obsidian configuration categories that sync (§6.8); empty: none.
    pub config_sync: Vec<String>,
    /// For a restricted viewer: the folders they read (RFC 0005 §4.2);
    /// absent: the whole vault.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    /// For a scoped token: its own folder rules (RFC 0004), so its client
    /// refuses a change outside them before sending it. Absent for a device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<super::tokens::ScopeRule>>,
    /// For whoever may bind on this vault (its manager, a tenant admin): the
    /// claim values the tenant maps, the only groups a binding may name
    /// (RFC 0003 §4.5, Add access).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<String>>,
    /// For a scoped token: its own name, which its client shows (RFC 0004 §4.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
pub struct ChunkingView {
    pub min: u64,
    pub avg: u64,
    pub max: u64,
}

impl From<&VaultAccess> for Vault {
    fn from(a: &VaultAccess) -> Self {
        Self {
            id: a.vault,
            tenant: a.tenant_slug.clone(),
            namespace: a.namespace.clone(),
            name: a.name.clone(),
            role: role_name(a.role).into(),
            managed_by: a.managed_by.clone(),
            chunking: ChunkingView {
                min: a.chunking.min.bytes(),
                avg: a.chunking.avg.bytes(),
                max: a.chunking.max.bytes(),
            },
            epoch: a.epoch,
            config_sync: a
                .config
                .iter()
                .filter_map(|c| serde_json::to_value(c).ok())
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            head_seq: u64::try_from(a.head_seq).unwrap_or_default(),
            updated_at: None,
            scopes: None,
            groups: None,
            token_name: None,
            paths: a.paths.clone(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateVault {
    /// The tenant's slug.
    pub tenant: String,
    /// An RFC 1123 label, unique in the tenant.
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutBinding {
    /// `viewer`, `editor` or `manager`, at most your own role.
    #[schema(value_type = String, example = "editor")]
    pub role: VaultRole,
    /// `viewer` only: 1 to 16 folders ending with `/`; absent: the whole
    /// vault (RFC 0005).
    #[serde(default)]
    pub paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BindingView {
    /// `claim:<value>` or `user:<sub>`.
    pub subject: String,
    pub role: String,
    pub managed_by: String,
    /// The folders a restricted viewer reads; absent: the whole vault.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
    /// RFC 0005 §4.3: folders listed, but the tenant maps this group to
    /// `owner` or `admin`, so the whole vault is open to it anyway.
    #[serde(default)]
    pub widened: bool,
    /// RFC 0005 §4.3: the bound folders under which no note exists yet
    /// (filled by the listing only).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub empty_folders: Vec<String>,
}

/// RFC 0005 §4.3: a restricted claim binding whose value the tenant maps to
/// `owner` or `admin` (the `RestrictedBindingWidened` rule, for every source).
fn widened(
    snap: &Snapshot,
    access: &VaultAccess,
    subject: &str,
    paths: Option<&Vec<String>>,
) -> bool {
    let Some(value) = subject.strip_prefix("claim:") else {
        return false;
    };
    paths.is_some()
        && snap
            .config
            .tenants
            .get(access.tenant_slug.as_str())
            .and_then(|t| t.doc.spec.claim_mappings.as_ref())
            .is_some_and(|m| {
                m.roles
                    .owner
                    .iter()
                    .chain(&m.roles.admin)
                    .any(|c| c == value)
            })
}

/// The `LIKE` pattern of one bound folder, as the read routes apply it.
fn folder_pattern(folder: &str) -> Option<String> {
    authz::PathScope::from_rules([(folder, false)])?
        .like_patterns()
        .into_iter()
        .next()
}

fn snapshot(state: &SharedState) -> Result<std::sync::Arc<Snapshot>, ApiError> {
    state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))
}

/// The vaults you can read.
#[utoipa::path(
    get,
    path = "/vaults",
    tag = "vaults",
    params(("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Readable vaults", body = Vec<Vault>),
        (status = 401, description = "Invalid token or device", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn list_vaults(
    State(state): State<SharedState>,
    reader: super::reader::Reader,
) -> Result<Json<Vec<Vault>>, ApiError> {
    let snap = snapshot(&state)?;
    let caller = match reader {
        super::reader::Reader::Device(d) => d,
        // A scoped token sees its one vault.
        super::reader::Reader::Key(k) => {
            let a =
                authz::key_access(&state.db, &snap, k.tenant, k.vault, VaultRole::Viewer).await?;
            return Ok(Json(
                a.iter()
                    .map(|a| Vault {
                        scopes: Some(k.rules.clone()),
                        token_name: Some(k.name.clone()),
                        ..Vault::from(a)
                    })
                    .collect(),
            ));
        }
    };
    let p = &caller.caller.principal;
    let mut out = Vec::new();
    // Every enabled tenant: a binding may open a vault without any tenant role.
    for &tenant in snap.tenant_ids.values() {
        let accesses = authz::vault_accesses_in(&state.db, &snap, p, tenant, MAX_LISTED).await?;
        let updated = updated_at(&state.db, tenant, &p.sub, &accesses).await?;
        for a in accesses {
            let binds = a.role == VaultRole::Manager
                || a.tenant_role.is_some_and(|r| r >= TenantRole::Admin);
            let groups = binds
                .then(|| {
                    snap.config
                        .tenants
                        .get(a.tenant_slug.as_str())
                        .and_then(|t| t.doc.spec.claim_mappings.as_ref())
                        .map(|m| {
                            let mut all: Vec<String> = m
                                .roles
                                .owner
                                .iter()
                                .chain(&m.roles.admin)
                                .chain(&m.roles.member)
                                .cloned()
                                .collect();
                            all.sort();
                            all.dedup();
                            all
                        })
                })
                .flatten();
            out.push(Vault {
                groups,
                updated_at: updated.get(&a.vault).cloned(),
                ..Vault::from(&a)
            });
        }
    }
    Ok(Json(out))
}

/// When each vault's head revision was written, in one query per tenant.
/// A restricted viewer already sees `headSeq` move (§4.2); the time of
/// that move says no more about the hidden folders.
async fn updated_at(
    pool: &sqlx::PgPool,
    tenant: Uuid,
    sub: &str,
    accesses: &[VaultAccess],
) -> Result<std::collections::HashMap<Uuid, String>, ApiError> {
    use sqlx::Row as _;
    let (ids, seqs): (Vec<Uuid>, Vec<i64>) = accesses
        .iter()
        .filter(|a| a.head_seq > 0)
        .map(|a| (a.vault, a.head_seq))
        .unzip();
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut tx = db::begin_tenant(pool, tenant, Some(sub)).await?;
    let rows = sqlx::query(
        "SELECT r.vault_id, r.created_at FROM revision r
           JOIN unnest($1::uuid[], $2::bigint[]) AS h(id, seq) ON r.vault_id = h.id AND r.seq = h.seq",
    )
    .bind(&ids)
    .bind(&seqs)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    rows.iter()
        .map(|r| {
            let at: time::OffsetDateTime = r.try_get("created_at")?;
            let at = at
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|e| ApiError::Unavailable(e.to_string()))?;
            Ok((r.try_get("vault_id")?, at))
        })
        .collect()
}

/// Create an API-managed vault (tenant admin).
#[utoipa::path(
    post,
    path = "/vaults",
    tag = "vaults",
    params(("X-Kura-Device" = Uuid, Header, description = "This device")),
    request_body = CreateVault,
    responses(
        (status = 201, description = "Created", body = Vault),
        (status = 400, description = "Invalid name, or the name is taken", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "Not a tenant admin", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such tenant", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn create_vault(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Body(body): Body<CreateVault>,
) -> Result<(StatusCode, Json<Vault>), ApiError> {
    let snap = snapshot(&state)?;
    let p = &caller.caller.principal;
    let name = DnsLabel::try_from(body.name.as_str())
        .map_err(|e| ApiError::with_detail(ProblemType::InvalidRequest, e.to_string()))?;
    // Unknown and foreign tenants answer alike.
    let membership = authz::memberships(&snap, p)
        .into_iter()
        .find(|m| m.slug == body.tenant)
        .ok_or(ApiError::of(ProblemType::NotFound))?;
    if membership.role < TenantRole::Admin {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "tenant admin required",
        ));
    }
    let mut tx = db::begin_tenant(&state.db, membership.tenant, Some(&p.sub)).await?;
    let chunking = serde_json::to_value(kura_api::Chunking::DEFAULT).unwrap_or_default();
    let id: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO vault (tenant_id, namespace, name, chunking_params, managed_by) VALUES ($1, $2, $3, $4, 'api')
         ON CONFLICT (namespace, name) DO NOTHING RETURNING id",
    )
    .bind(membership.tenant)
    .bind(&membership.slug)
    .bind(name.as_str())
    .bind(chunking)
    .fetch_optional(&mut *tx)
    .await?;
    let id = id.ok_or(ApiError::with_detail(
        ProblemType::InvalidRequest,
        "a vault with this name exists",
    ))?;
    crate::notifier::record(
        &state,
        &mut tx,
        crate::notifier::Fact {
            tenant: membership.tenant,
            tenant_slug: &membership.slug,
            vault: Some((id, name.as_str())),
            kind: "vault.created",
            title: format!("Vault {} created", name.as_str()),
            message: format!("Tenant {}: a new vault.", membership.slug),
            data: serde_json::json!({}),
            dedupe: None,
        },
    )
    .await?;
    tx.commit().await?;
    tracing::info!(target: "audit", event = "vault_created", sub = %p.sub, vault = %id);
    let access = authz::vault_access_in(&state.db, &snap, p, membership.tenant, id)
        .await?
        .ok_or(ApiError::of(ProblemType::NotFound))?;
    Ok((StatusCode::CREATED, Json(Vault::from(&access))))
}

async fn manageable(
    state: &SharedState,
    caller: &DeviceCaller,
    v: Uuid,
) -> Result<(std::sync::Arc<Snapshot>, VaultAccess), ApiError> {
    let snap = snapshot(state)?;
    let access = authz::require_role(
        &state.db,
        &snap,
        &caller.caller.principal,
        v,
        VaultRole::Viewer,
    )
    .await?;
    Ok((snap, access))
}

/// Delete an API-managed vault (tenant admin): disabled now, purged after 30 days.
#[utoipa::path(
    delete,
    path = "/vaults/{v}",
    tag = "vaults",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 204, description = "Disabled, deletion scheduled"),
        (status = 403, description = "Not a tenant admin, or declared in configuration", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn delete_vault(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let (_, access) = manageable(&state, &caller, v).await?;
    if !access.tenant_role.is_some_and(|r| r >= TenantRole::Admin) {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "tenant admin required",
        ));
    }
    if access.managed_by != "api" {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "declared in configuration: remove its document instead",
        ));
    }
    let mut tx = db::begin_tenant(&state.db, access.tenant, None).await?;
    sqlx::query("UPDATE vault SET disabled_at = now(), delete_after = now() + interval '30 days' WHERE id = $1")
        .bind(v)
        .execute(&mut *tx)
        .await?;
    crate::notifier::record(
        &state,
        &mut tx,
        crate::notifier::Fact::vault(
            &access,
            "vault.deleted",
            format!("Vault {} deleted", access.name),
            format!(
                "Tenant {}: disabled now, purged after 30 days.",
                access.tenant_slug
            ),
            serde_json::json!({}),
        ),
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(target: "audit", event = "vault_deleted", sub = %caller.caller.principal.sub, vault = %v);
    Ok(StatusCode::NO_CONTENT)
}

fn subject(b: &str) -> Result<BindingSubject, ApiError> {
    BindingSubject::parse(b).ok_or(ApiError::with_detail(
        ProblemType::InvalidRequest,
        "binding is claim:<value> or user:<sub>",
    ))
}

/// `subject_kind, subject, role, managed_by, paths` of `role_binding`.
type BindingRow = (String, String, String, String, Option<Vec<String>>);

/// The vault's bindings, from files and from the API (RFC 0003 §4.4): vault
/// manager or tenant admin.
#[utoipa::path(
    get,
    path = "/vaults/{v}/bindings",
    tag = "vaults",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Every binding, by subject", body = Vec<BindingView>),
        (status = 403, description = "Below manager", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn list_bindings(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
) -> Result<Json<Vec<BindingView>>, ApiError> {
    let (snap, access) = manageable(&state, &caller, v).await?;
    let can_manage = access.role == VaultRole::Manager
        || access.tenant_role.is_some_and(|r| r >= TenantRole::Admin);
    if !can_manage {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "manager required",
        ));
    }
    let mut tx = db::begin_tenant(&state.db, access.tenant, None).await?;
    let rows: Vec<BindingRow> = sqlx::query_as(
        "SELECT subject_kind, subject, role, managed_by, paths FROM role_binding
         WHERE vault_id = $1 ORDER BY subject_kind, subject LIMIT 1000",
    )
    .bind(v)
    .fetch_all(&mut *tx)
    .await?;
    // RFC 0005 §4.3 "0 notes": the bound folders' patterns that match no
    // note, in one query; EXISTS stops at the first note found.
    let mut patterns: Vec<String> = rows
        .iter()
        .filter_map(|r| r.4.as_ref())
        .flatten()
        .filter_map(|f| folder_pattern(f))
        .collect();
    patterns.sort_unstable();
    patterns.dedup();
    let empty: Vec<String> = if patterns.is_empty() {
        Vec::new()
    } else {
        sqlx::query_scalar(
            "SELECT p FROM unnest($2::text[]) AS p WHERE NOT EXISTS (SELECT 1 FROM entry e
             WHERE e.vault_id = $1 AND e.namespace = 'notes' AND NOT e.deleted AND e.path_key LIKE p)",
        )
        .bind(v)
        .bind(&patterns)
        .fetch_all(&mut *tx)
        .await?
    };
    tx.commit().await?;
    Ok(Json(
        rows.into_iter()
            .map(|(kind, subject, role, managed_by, paths)| {
                let subject = format!("{kind}:{subject}");
                let empty_folders = paths
                    .iter()
                    .flatten()
                    .filter(|f| folder_pattern(f).is_some_and(|p| empty.contains(&p)))
                    .cloned()
                    .collect();
                BindingView {
                    widened: widened(&snap, &access, &subject, paths.as_ref()),
                    subject,
                    role,
                    managed_by,
                    paths,
                    empty_folders,
                }
            })
            .collect(),
    ))
}

/// Bind a role on a vault (vault manager or tenant admin, §6.2).
#[utoipa::path(
    put,
    path = "/vaults/{v}/bindings/{b}",
    tag = "vaults",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("b" = String, Path, description = "claim:<value> or user:<sub>"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    request_body = PutBinding,
    responses(
        (status = 200, description = "Bound", body = BindingView),
        (status = 403, description = "Not allowed to grant this", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn put_binding(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, b)): Path<(Uuid, String)>,
    Body(body): Body<PutBinding>,
) -> Result<Json<BindingView>, ApiError> {
    let (snap, access) = manageable(&state, &caller, v).await?;
    let subject = subject(&b)?;
    // RFC 0005 §4.3: folders on a viewer only, each a vault folder.
    if let Some(paths) = &body.paths {
        kura_api::validate::check_paths(body.role, paths).map_err(|m| {
            ApiError::with_detail(ProblemType::InvalidRequest, format!("paths: {m}"))
        })?;
        for p in paths {
            super::tokens::check_folder(p)?;
        }
    }
    authz::check_binding(&state.db, &snap, &access, &subject, body.role).await?;
    let mut tx = db::begin_tenant(&state.db, access.tenant, None).await?;
    // `paths` is written with the role, so raising a role clears it in the
    // same statement (the CHECK refuses a writer with folders).
    let written: Option<String> = sqlx::query_scalar(
        "INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by, paths) VALUES ($1, $2, $3, $4, $5, 'api', $6)
         ON CONFLICT (vault_id, subject_kind, subject) DO UPDATE SET role = EXCLUDED.role, paths = EXCLUDED.paths WHERE role_binding.managed_by = 'api'
         RETURNING role",
    )
    .bind(access.tenant)
    .bind(v)
    .bind(subject.kind())
    .bind(subject.value())
    .bind(role_name(body.role))
    .bind(&body.paths)
    .fetch_optional(&mut *tx)
    .await?;
    if written.is_none() {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "declared in configuration: read-only here",
        ));
    }
    crate::notifier::record(
        &state,
        &mut tx,
        crate::notifier::Fact::vault(
            &access,
            "binding.changed",
            format!("Access to {} changed", access.name),
            format!("Tenant {}: a {} binding of the vault is now {}.", access.tenant_slug, subject.kind(), role_name(body.role)),
            // The kind and the role: never the subject itself (T5).
            serde_json::json!({ "binding": { "kind": subject.kind(), "role": role_name(body.role), "folders": body.paths.as_ref().map(Vec::len) } }),
        ),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(target: "audit", event = "binding_changed", sub = %caller.caller.principal.sub, vault = %v, binding = %b, role = role_name(body.role), restricted = body.paths.is_some(), folders = body.paths.as_ref().map_or(0, Vec::len));
    Ok(Json(BindingView {
        widened: widened(&snap, &access, &b, body.paths.as_ref()),
        subject: b,
        role: role_name(body.role).into(),
        managed_by: "api".into(),
        paths: body.paths,
        empty_folders: Vec::new(),
    }))
}

/// Remove an API-managed binding (vault manager or tenant admin).
#[utoipa::path(
    delete,
    path = "/vaults/{v}/bindings/{b}",
    tag = "vaults",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("b" = String, Path, description = "claim:<value> or user:<sub>"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    responses(
        (status = 204, description = "Removed"),
        (status = 403, description = "Not allowed to remove this", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault or binding", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn delete_binding(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, b)): Path<(Uuid, String)>,
) -> Result<StatusCode, ApiError> {
    let (_, access) = manageable(&state, &caller, v).await?;
    let subject = subject(&b)?;
    let can_manage = access.role == VaultRole::Manager
        || access.tenant_role.is_some_and(|r| r >= TenantRole::Admin);
    if !can_manage {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "manager required",
        ));
    }
    let mut tx = db::begin_tenant(&state.db, access.tenant, None).await?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT role, managed_by FROM role_binding WHERE vault_id = $1 AND subject_kind = $2 AND subject = $3")
            .bind(v)
            .bind(subject.kind())
            .bind(subject.value())
            .fetch_optional(&mut *tx)
            .await?;
    let (role, managed_by) = row.ok_or(ApiError::of(ProblemType::NotFound))?;
    if managed_by != "api" {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "declared in configuration: read-only here",
        ));
    }
    if authz::parse_role(&role).is_none_or(|r| r > access.role) {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "cannot remove a role above your own",
        ));
    }
    sqlx::query(
        "DELETE FROM role_binding WHERE vault_id = $1 AND subject_kind = $2 AND subject = $3",
    )
    .bind(v)
    .bind(subject.kind())
    .bind(subject.value())
    .execute(&mut *tx)
    .await?;
    crate::notifier::record(
        &state,
        &mut tx,
        crate::notifier::Fact::vault(
            &access,
            "binding.removed",
            format!("Access to {} removed", access.name),
            format!(
                "Tenant {}: a {} binding of the vault was removed.",
                access.tenant_slug,
                subject.kind()
            ),
            serde_json::json!({ "binding": { "kind": subject.kind() } }),
        ),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(target: "audit", event = "binding_removed", sub = %caller.caller.principal.sub, vault = %v, binding = %b);
    Ok(StatusCode::NO_CONTENT)
}
