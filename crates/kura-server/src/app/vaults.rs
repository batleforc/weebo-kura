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
            scopes: None,
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
        for a in authz::vault_accesses_in(&state.db, &snap, p, tenant, MAX_LISTED).await? {
            out.push(Vault::from(&a));
        }
    }
    Ok(Json(out))
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
    let (_, access) = manageable(&state, &caller, v).await?;
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
    tx.commit().await?;
    Ok(Json(
        rows.into_iter()
            .map(|(kind, subject, role, managed_by, paths)| BindingView {
                subject: format!("{kind}:{subject}"),
                role,
                managed_by,
                paths,
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
    tracing::info!(target: "audit", event = "binding_changed", sub = %caller.caller.principal.sub, vault = %v, binding = %b, role = role_name(body.role), folders = body.paths.as_ref().map_or(0, Vec::len));
    Ok(Json(BindingView {
        subject: b,
        role: role_name(body.role).into(),
        managed_by: "api".into(),
        paths: body.paths,
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
