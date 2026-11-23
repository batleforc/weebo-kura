//! RFC 0010 §4.4: each member's choice of what crosses their profiles, and
//! the install route that writes a plugin's three code files into one
//! configuration space as one commit (so the console never commits itself).

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use base64::Engine as _;
use kura_api::VaultRole;
use kura_core::wire::Operation;
use kura_core::{EntryId, VaultPath};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::SharedState;
use super::body::Body;
use super::device::DeviceCaller;
use super::space::{self, Space};
use crate::authz::{self, VaultAccess};
use crate::db;
use crate::problem::{ApiError, Problem, ProblemType};

/// Choices one member may hold on one vault (§7.5).
pub const MAX_CHOICES: i64 = 200;
/// A plugin's `manifest.json` at most (§4.3).
const MAX_MANIFEST: usize = 64 << 10;
/// The install body: three files, base64 in JSON.
pub const MAX_INSTALL_BODY: usize = 48 << 20;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Shares {
    /// The vault's `sharedByDefault`.
    pub defaults: Vec<String>,
    /// Your own choices, by item; an absent item follows the default.
    pub choices: BTreeMap<String, bool>,
    /// What is shared for you: the defaults overridden by your choices.
    pub shared: Vec<String>,
    /// The plugins installed on the vault by its managers.
    pub vault_plugins: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PutShare {
    pub shared: bool,
}

/// A configuration space's member on this vault: any role, never a
/// restricted viewer (their scope holds no configuration, T7).
async fn member(
    state: &SharedState,
    caller: &DeviceCaller,
    v: Uuid,
) -> Result<VaultAccess, ApiError> {
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let a = authz::require_role(
        &state.db,
        &snap,
        &caller.caller.principal,
        v,
        VaultRole::Viewer,
    )
    .await?;
    if a.paths.is_some() {
        return Err(ApiError::with_detail(
            ProblemType::InsufficientRole,
            "a restricted viewer has no configuration",
        ));
    }
    if a.config.categories.is_empty() {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            "this vault does not sync the Obsidian configuration",
        ));
    }
    Ok(a)
}

fn item_ok(a: &VaultAccess, item: &str) -> Result<(), ApiError> {
    match kura_api::spec::ConfigSync::item_category(item) {
        Some(c) if a.config.categories.contains(&c) => Ok(()),
        Some(_) => Err(ApiError::with_detail(
            ProblemType::InvalidPath,
            format!("{item}: its category does not sync on this vault"),
        )),
        None => Err(ApiError::with_detail(
            ProblemType::InvalidPath,
            format!("{item} is neither plugin:<id> nor snippets, themes, hotkeys"),
        )),
    }
}

async fn shares_of(state: &SharedState, a: &VaultAccess, sub: &str) -> Result<Shares, ApiError> {
    let choices = space::choices(&state.db, a, sub).await?;
    Ok(Shares {
        defaults: a.config.shared_by_default.clone(),
        shared: space::effective(&a.config.shared_by_default, &choices),
        choices: choices.into_iter().collect(),
        vault_plugins: space::vault_plugins(&state.db, a).await?,
    })
}

/// Your choices of what crosses your desktop and mobile devices on a vault.
#[utoipa::path(
    get,
    path = "/vaults/{v}/config/shares",
    tag = "config",
    params(("v" = Uuid, Path, description = "Vault id"), ("X-Kura-Device" = Uuid, Header, description = "This device")),
    responses(
        (status = 200, description = "Defaults, your choices, the effective set, the vault's plugins", body = Shares),
        (status = 403, description = "A restricted viewer", body = Problem, content_type = "application/problem+json"),
        (status = 404, description = "No such vault", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn get_shares(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path(v): Path<Uuid>,
) -> Result<Json<Shares>, ApiError> {
    let a = member(&state, &caller, v).await?;
    Ok(Json(
        shares_of(&state, &a, &caller.caller.principal.sub).await?,
    ))
}

/// Share an item between your profiles, or keep it per profile, overriding
/// the vault's default for you.
#[utoipa::path(
    put,
    path = "/vaults/{v}/config/shares/{item}",
    tag = "config",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("item" = String, Path, description = "`plugin:<id>`, `snippets`, `themes` or `hotkeys`"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    request_body = PutShare,
    responses(
        (status = 200, description = "Your choices now", body = Shares),
        (status = 400, description = "An item not well formed, or more than 200 choices", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "A restricted viewer", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn put_share(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, item)): Path<(Uuid, String)>,
    Body(body): Body<PutShare>,
) -> Result<Json<Shares>, ApiError> {
    let a = member(&state, &caller, v).await?;
    item_ok(&a, &item)?;
    let sub = &caller.caller.principal.sub;
    set_choice(&state, &a, sub, &item, body.shared).await?;
    tracing::debug!(vault = %v, %item, shared = body.shared, "share choice set");
    Ok(Json(shares_of(&state, &a, sub).await?))
}

async fn set_choice(
    state: &SharedState,
    a: &VaultAccess,
    sub: &str,
    item: &str,
    shared: bool,
) -> Result<(), ApiError> {
    let mut tx = db::begin_tenant(&state.db, a.tenant, Some(sub)).await?;
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM config_share WHERE vault_id = $1 AND owner_sub = $2 AND item <> $3",
    )
    .bind(a.vault)
    .bind(sub)
    .bind(item)
    .fetch_one(&mut *tx)
    .await?;
    if held >= MAX_CHOICES {
        return Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            format!("at most {MAX_CHOICES} choices per vault"),
        ));
    }
    sqlx::query(
        "INSERT INTO config_share (tenant_id, vault_id, owner_sub, item, shared) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (vault_id, owner_sub, item) DO UPDATE SET shared = EXCLUDED.shared",
    )
    .bind(a.tenant)
    .bind(a.vault)
    .bind(sub)
    .bind(item)
    .bind(shared)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Back to the vault's default for this item.
#[utoipa::path(
    delete,
    path = "/vaults/{v}/config/shares/{item}",
    tag = "config",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("item" = String, Path, description = "`plugin:<id>`, `snippets`, `themes` or `hotkeys`"),
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    responses(
        (status = 200, description = "Your choices now", body = Shares),
        (status = 400, description = "An item not well formed", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn delete_share(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, item)): Path<(Uuid, String)>,
) -> Result<Json<Shares>, ApiError> {
    let a = member(&state, &caller, v).await?;
    item_ok(&a, &item)?;
    let sub = &caller.caller.principal.sub;
    let mut tx = db::begin_tenant(&state.db, a.tenant, Some(sub)).await?;
    sqlx::query("DELETE FROM config_share WHERE vault_id = $1 AND owner_sub = $2 AND item = $3")
        .bind(v)
        .bind(sub)
        .bind(&item)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(shares_of(&state, &a, sub).await?))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct InstallQuery {
    /// `vault` (a manager's Obsidian device only), `shared`, `desktop` or
    /// `mobile` (the caller's own configuration).
    pub target: String,
}

/// A plugin's three code files, base64.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InstallPlugin {
    /// `manifest.json`, base64.
    pub manifest: String,
    /// `main.js`, base64.
    pub main: String,
    /// `styles.css`, base64, when the plugin has one.
    #[serde(default)]
    pub styles: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub name: String,
    pub size: u64,
    /// Hex SHA-256 of the file.
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub id: String,
    /// The manifest's `name`, else the id.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `config-vault`, `config-shared` or `config`.
    pub space: String,
    pub files: Vec<InstalledFile>,
}

fn invalid(detail: impl Into<String>) -> ApiError {
    ApiError::with_detail(ProblemType::InvalidPath, detail)
}

/// The manifest's `name` and `version`, once its `id` is the route's (T14).
fn read_manifest(id: &str, raw: &[u8]) -> Result<(String, Option<String>), ApiError> {
    if raw.len() > MAX_MANIFEST {
        return Err(invalid("manifest.json is over 64 KiB"));
    }
    let m: serde_json::Value =
        serde_json::from_slice(raw).map_err(|e| invalid(format!("manifest.json: {e}")))?;
    let text = |k: &str| {
        m.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    match text("id") {
        Some(i) if i == id => {}
        Some(i) => return Err(invalid(format!("manifest.json names {i}, not {id}"))),
        None => return Err(invalid("manifest.json has no id")),
    }
    Ok((
        text("name").unwrap_or_else(|| id.to_owned()),
        text("version"),
    ))
}

/// Install a plugin's code, written disabled on the devices that receive it:
/// on the vault (a manager, from an Obsidian device or the CLI) or in your
/// own configuration (the console too).
#[utoipa::path(
    put,
    path = "/vaults/{v}/config/plugins/{id}",
    tag = "config",
    params(
        ("v" = Uuid, Path, description = "Vault id"),
        ("id" = String, Path, description = "The plugin's id, as its manifest names it"),
        InstallQuery,
        ("X-Kura-Device" = Uuid, Header, description = "This device"),
    ),
    request_body = InstallPlugin,
    responses(
        (status = 200, description = "Installed, as one commit", body = Installed),
        (status = 400, description = "A manifest not JSON or naming another id, a missing file, a vault without plugins", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "On the vault: not a manager, or the console", body = Problem, content_type = "application/problem+json"),
        (status = 413, description = "A file over the tenant's maxFileSize", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn install_plugin(
    State(state): State<SharedState>,
    caller: DeviceCaller,
    Path((v, id)): Path<(Uuid, String)>,
    Query(q): Query<InstallQuery>,
    Body(body): Body<InstallPlugin>,
) -> Result<(StatusCode, Json<Installed>), ApiError> {
    let a = member(&state, &caller, v).await?;
    if !kura_api::spec::plugin_id_ok(&id) {
        return Err(invalid(format!(
            "{id} is not a plugin id this vault may carry"
        )));
    }
    let sub = caller.caller.principal.sub.clone();
    let item = format!("plugin:{id}");
    let refuse = |kind, detail: &str| Err(ApiError::with_detail(kind, detail.to_owned()));
    let (asked, namespace) = match q.target.as_str() {
        // The console installs for its member only (decision 12, T13).
        "vault" if caller.is_web() => {
            return refuse(
                ProblemType::InsufficientRole,
                "the console installs a plugin for you only",
            );
        }
        "vault" if !a.config.vault_plugins => {
            return refuse(
                ProblemType::InvalidRequest,
                "this vault does not take plugins",
            );
        }
        "vault" if !space::manages(&a) => {
            return refuse(
                ProblemType::InsufficientRole,
                "installing a plugin on the vault needs the manager role",
            );
        }
        "vault" => ("config-vault", "config:vault".to_owned()),
        "shared" => ("config-shared", "config:shared".to_owned()),
        // A device writes its own profile's copy; the console names one.
        p @ ("desktop" | "mobile") if caller.is_web() || p == caller.profile => {
            ("config", format!("config:{p}"))
        }
        "desktop" | "mobile" => {
            return refuse(
                ProblemType::InvalidRequest,
                "a device installs into its own profile, shared, or the vault",
            );
        }
        other => {
            return Err(ApiError::with_detail(
                ProblemType::InvalidRequest,
                format!("target: {other} is neither vault, shared, desktop nor mobile"),
            ));
        }
    };
    if a.role < VaultRole::Editor {
        // Writing configuration is a device's sync right (Editor).
        return refuse(ProblemType::InsufficientRole, "editor required");
    }
    let mut space = Space {
        owner: (asked != "config-vault").then(|| sub.clone()),
        namespace,
        homes: space::Homes::default(),
    };
    space.homes = space::homes(&state.db, &a, &sub).await?;
    // The choice follows the target: shared lands where it is served.
    let choice = match asked {
        "config-shared" => Some(true),
        "config" if space.homes.shared.contains(&item) => Some(false),
        _ => None,
    };
    match choice {
        Some(true) => space.homes.shared.push(item.clone()),
        Some(false) => space.homes.shared.retain(|s| *s != item),
        None => {}
    }

    let b64 = base64::engine::general_purpose::STANDARD;
    let decode = |name: &str, s: &str| {
        b64.decode(s)
            .map_err(|e| ApiError::with_detail(ProblemType::InvalidRequest, format!("{name}: {e}")))
    };
    let manifest = decode("manifest", &body.manifest)?;
    let (name, version) = read_manifest(&id, &manifest)?;
    let mut files = vec![
        ("manifest.json", manifest),
        ("main.js", decode("main", &body.main)?),
    ];
    if let Some(s) = &body.styles {
        files.push(("styles.css", decode("styles", s)?));
    }
    let snap = state
        .snapshot()
        .ok_or(ApiError::of(ProblemType::Unavailable))?;
    let max_file = snap
        .config
        .tenants
        .get(a.tenant_slug.as_str())
        .and_then(|t| t.doc.spec.quota.max_file_size);
    if files
        .iter()
        .any(|(_, d)| max_file.is_some_and(|m| d.len() as u64 > m.bytes()))
    {
        return Err(ApiError::of(ProblemType::PayloadTooLarge));
    }
    let mut operations = Vec::new();
    let mut out = Vec::new();
    for (file, data) in files {
        let path = VaultPath::parse(&format!("plugins/{id}/{file}"))
            .map_err(|e| invalid(e.to_string()))?;
        if !space.allows(path.as_str()) {
            return Err(invalid(format!(
                "{path} belongs to {}",
                space.home_of(path.as_str())
            )));
        }
        let size = data.len() as u64;
        let sha256 = sha2::Sha256::digest(&data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let blob = super::chunks::store_content(&state, &a, v, data).await?;
        let entry = EntryId::from_uuid(Uuid::now_v7())
            .ok_or_else(|| ApiError::Unavailable("a new entry id".into()))?;
        operations.push(Operation::Put {
            entry,
            path,
            base: None,
            blob,
            size,
        });
        out.push(InstalledFile {
            name: file.to_owned(),
            size,
            sha256,
        });
    }
    if let Some(shared) = choice {
        set_choice(&state, &a, &sub, &item, shared).await?;
    }
    super::commits::commit_for(&state, &a, caller.device, &sub, &space, operations).await?;
    Ok((
        StatusCode::OK,
        Json(Installed {
            id,
            name,
            version,
            space: asked.to_owned(),
            files: out,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manifest_names_the_routes_plugin() {
        let ok = br#"{"id":"kanban","name":"Kanban","version":"2.0.51"}"#;
        assert_eq!(
            read_manifest("kanban", ok).unwrap(),
            ("Kanban".to_owned(), Some("2.0.51".to_owned()))
        );
        assert!(read_manifest("other", ok).is_err());
        assert!(read_manifest("kanban", b"not json").is_err());
        assert!(read_manifest("kanban", br#"{"name":"x"}"#).is_err());
        assert!(read_manifest("kanban", &vec![b' '; MAX_MANIFEST + 1]).is_err());
    }
}
