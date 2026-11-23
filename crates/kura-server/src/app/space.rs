//! Where entries live (RFC 0001 §6.8, decision 34; RFC 0010 §4.2): the
//! shared note namespace; the caller's own Obsidian configuration, per
//! device profile or shared between their profiles; or the plugin code
//! installed on the vault. The client names the space; the profile is the
//! device's as registered, the owner the caller, every path's home is the
//! server's decision, and row-level security keeps every other user's
//! configuration out of reach.

use crate::authz::VaultAccess;
use crate::db;
use crate::problem::{ApiError, ProblemType};

/// A resolved space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    /// `notes`, `config:desktop`, `config:mobile`, `config:shared` or
    /// `config:vault`.
    pub namespace: String,
    /// The configuration's owner; `None` for notes and the vault's plugins.
    pub owner: Option<String>,
    /// What decides the home of a configuration path.
    pub homes: Homes,
}

/// The vault's categories, the member's effective shared items and the
/// plugins installed on the vault (RFC 0010 §5.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Homes {
    pub categories: Vec<String>,
    pub shared: Vec<String>,
    pub vault_plugins: Vec<String>,
}

impl Space {
    pub fn notes() -> Self {
        Self {
            namespace: "notes".into(),
            owner: None,
            homes: Homes::default(),
        }
    }

    pub fn is_config(&self) -> bool {
        self.namespace != "notes"
    }

    /// The name a client gives this space in a request.
    pub fn asked(&self) -> Option<&'static str> {
        match self.namespace.as_str() {
            "notes" => None,
            "config:shared" => Some("config-shared"),
            "config:vault" => Some("config-vault"),
            _ => Some("config"),
        }
    }

    /// A path of this space may be written or served: any note; a
    /// configuration file whose home is this space, never the Kura plugin
    /// itself (§6.8); in the vault's space, the three code files of a plugin
    /// folder only (RFC 0010 T3).
    pub fn allows(&self, path: &str) -> bool {
        use kura_core::config::{home, synced, vault_code};
        let h = &self.homes;
        match self.namespace.as_str() {
            "notes" => true,
            "config:vault" => {
                synced(path, &h.categories)
                    && vault_code(path).is_some_and(|id| kura_api::spec::plugin_id_ok(&id))
            }
            "config:shared" => {
                home(path, &h.categories, &h.shared, &h.vault_plugins) == Some("shared")
            }
            _ => home(path, &h.categories, &h.shared, &h.vault_plugins) == Some("profile"),
        }
    }

    /// The space a path refused here belongs to, for the refusal's words.
    pub fn home_of(&self, path: &str) -> &'static str {
        let h = &self.homes;
        match kura_core::config::home(path, &h.categories, &h.shared, &h.vault_plugins) {
            Some("vault") => "config-vault",
            Some("shared") => "config-shared",
            Some(_) => "config",
            None => "no space: not a synced configuration category",
        }
    }
}

/// The effective shared set: the vault's defaults, each overridden by the
/// member's own choice (RFC 0010 §4.1).
pub fn effective(defaults: &[String], choices: &[(String, bool)]) -> Vec<String> {
    let mut out: Vec<String> = defaults
        .iter()
        .filter(|d| !choices.iter().any(|(i, s)| i == *d && !s))
        .cloned()
        .collect();
    for (item, shared) in choices {
        if *shared && !out.contains(item) {
            out.push(item.clone());
        }
    }
    out.sort();
    out
}

/// A vault manager, or a tenant admin (the check of `check_binding`).
pub fn manages(a: &VaultAccess) -> bool {
    a.role == kura_api::VaultRole::Manager
        || a.tenant_role
            .is_some_and(|r| r >= kura_api::TenantRole::Admin)
}

/// The member's choices on this vault, under their own subject.
pub async fn choices(
    pool: &sqlx::PgPool,
    a: &VaultAccess,
    sub: &str,
) -> Result<Vec<(String, bool)>, ApiError> {
    let mut tx = db::begin_tenant(pool, a.tenant, Some(sub)).await?;
    let rows = sqlx::query_as(
        "SELECT item, shared FROM config_share WHERE vault_id = $1 AND owner_sub = $2 ORDER BY item",
    )
    .bind(a.vault)
    .bind(sub)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// The plugins installed on the vault: a live code file in `config:vault`.
pub async fn vault_plugins(pool: &sqlx::PgPool, a: &VaultAccess) -> Result<Vec<String>, ApiError> {
    if !a.config.vault_plugins {
        return Ok(Vec::new());
    }
    let mut tx = db::begin_tenant(pool, a.tenant, None).await?;
    let ids = sqlx::query_scalar(
        "SELECT DISTINCT split_part(path_key, '/', 2) FROM entry
         WHERE vault_id = $1 AND namespace = 'config:vault' AND NOT deleted ORDER BY 1",
    )
    .bind(a.vault)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ids)
}

/// What decides the homes, for this member on this vault.
pub async fn homes(pool: &sqlx::PgPool, a: &VaultAccess, sub: &str) -> Result<Homes, ApiError> {
    Ok(Homes {
        categories: categories(a),
        shared: effective(&a.config.shared_by_default, &choices(pool, a, sub).await?),
        vault_plugins: vault_plugins(pool, a).await?,
    })
}

/// The vault's categories as `configSync` names them.
pub fn categories(a: &VaultAccess) -> Vec<String> {
    a.config
        .categories
        .iter()
        .filter_map(|c| serde_json::to_value(c).ok())
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

/// What a request does in the space it names (RFC 0008 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use<'a> {
    /// `GET entries`: the console may list, naming a profile.
    List { profile: Option<&'a str> },
    /// `GET changes`: a device's sync only.
    Changes,
    /// A commit: a device's sync only.
    Write,
}

/// [`resolve`], with the homes loaded for a configuration space.
pub async fn resolve_loaded(
    pool: &sqlx::PgPool,
    asked: Option<&str>,
    a: &VaultAccess,
    caller: Option<(&str, &str)>,
    usage: Use<'_>,
) -> Result<Space, ApiError> {
    let mut space = resolve(asked, a, caller, usage)?;
    if let (true, Some((sub, _))) = (space.is_config(), caller) {
        space.homes = homes(pool, a, sub).await?;
    }
    Ok(space)
}

fn refuse(kind: ProblemType, detail: impl Into<String>) -> ApiError {
    ApiError::with_detail(kind, detail)
}

/// The space a request names (`space=config`, `config-shared`,
/// `config-vault`, else notes), for this device: `caller` is its subject and
/// registered profile. The homes are left empty: see [`resolve_loaded`].
pub fn resolve(
    asked: Option<&str>,
    a: &VaultAccess,
    caller: Option<(&str, &str)>,
    usage: Use<'_>,
) -> Result<Space, ApiError> {
    let name = match asked {
        None | Some("notes") => return Ok(Space::notes()),
        Some(n @ ("config" | "config-shared" | "config-vault")) => n,
        Some(other) => {
            return Err(refuse(
                ProblemType::InvalidRequest,
                format!("space: {other} is neither notes, config, config-shared nor config-vault"),
            ));
        }
    };
    // A scoped token never reads configuration (decision 34).
    let Some((sub, device_profile)) = caller else {
        return Err(refuse(
            ProblemType::InsufficientRole,
            "configuration is served to its owner's devices only",
        ));
    };
    if a.config.categories.is_empty() {
        return Err(refuse(
            ProblemType::InvalidRequest,
            "this vault does not sync the Obsidian configuration",
        ));
    }
    let named = match usage {
        Use::List { profile } => profile,
        Use::Changes | Use::Write => None,
    };
    let web = device_profile == "web";
    if name != "config" {
        // RFC 0010: a restricted viewer's scope never holds configuration.
        if a.paths.is_some() {
            return Err(refuse(
                ProblemType::InsufficientRole,
                "a restricted viewer has no configuration",
            ));
        }
        if named.is_some() {
            return Err(refuse(
                ProblemType::InvalidRequest,
                format!("profile: the {name} space names no profile"),
            ));
        }
        if web && !matches!(usage, Use::List { .. }) {
            return Err(refuse(
                ProblemType::InsufficientRole,
                "configuration is served to Obsidian devices only",
            ));
        }
        if name == "config-shared" {
            return Ok(Space {
                namespace: "config:shared".into(),
                owner: Some(sub.to_owned()),
                homes: Homes::default(),
            });
        }
        if !a.config.vault_plugins {
            return Err(refuse(
                ProblemType::InvalidRequest,
                "this vault does not take plugins",
            ));
        }
        if usage == Use::Write && !manages(a) {
            return Err(refuse(
                ProblemType::InsufficientRole,
                "installing a plugin on the vault needs the manager role",
            ));
        }
        return Ok(Space {
            namespace: "config:vault".into(),
            owner: None,
            homes: Homes::default(),
        });
    }
    if let Some(p) = named
        && !matches!(p, "desktop" | "mobile")
    {
        return Err(refuse(
            ProblemType::InvalidRequest,
            format!("profile: {p} is neither desktop nor mobile"),
        ));
    }
    let profile = if web {
        // The console lists, by a profile it names; it never follows or
        // writes the configuration (RFC 0008 §5.1).
        match (usage, named) {
            (Use::List { .. }, Some(p)) => p,
            (Use::List { .. }, None) => {
                return Err(refuse(
                    ProblemType::InvalidRequest,
                    "profile: the console names desktop or mobile",
                ));
            }
            (Use::Changes | Use::Write, _) => {
                return Err(refuse(
                    ProblemType::InsufficientRole,
                    "configuration is served to Obsidian devices only",
                ));
            }
        }
    } else {
        // An Obsidian device reads its own profile only.
        if named.is_some_and(|p| p != device_profile) {
            return Err(refuse(
                ProblemType::InvalidRequest,
                "profile: a device reads its own profile only",
            ));
        }
        device_profile
    };
    Ok(Space {
        namespace: format!("config:{profile}"),
        owner: Some(sub.to_owned()),
        homes: Homes::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn space(namespace: &str, shared: &[&str], vault: &[&str]) -> Space {
        Space {
            namespace: namespace.into(),
            owner: (namespace != "config:vault").then(|| "u".into()),
            homes: Homes {
                categories: ["plugins", "snippets", "hotkeys"]
                    .map(String::from)
                    .to_vec(),
                shared: shared.iter().map(|s| (*s).to_owned()).collect(),
                vault_plugins: vault.iter().map(|s| (*s).to_owned()).collect(),
            },
        }
    }

    #[test]
    fn each_path_has_one_home() {
        let shared = ["snippets", "plugin:dv"];
        let vault = ["ex"];
        let profile = space("config:desktop", &shared, &vault);
        let mine = space("config:shared", &shared, &vault);
        let theirs = space("config:vault", &shared, &vault);
        for (path, home) in [
            ("snippets/a.css", "config-shared"),
            ("plugins/dv/data.json", "config-shared"),
            ("plugins/ex/main.js", "config-vault"),
            ("plugins/ex/data.json", "config"),
            ("hotkeys.json", "config"),
        ] {
            assert_eq!(profile.home_of(path), home, "{path}");
            for (s, name) in [(&profile, "config"), (&mine, "config-shared")] {
                assert_eq!(s.allows(path), home == name, "{name} {path}");
            }
        }
        // The vault's space takes any plugin's three code files, nothing else.
        assert!(
            theirs.allows("plugins/new/manifest.json") && theirs.allows("plugins/ex/styles.css")
        );
        for no in [
            "plugins/ex/data.json",
            "snippets/a.css",
            "plugins/weebo-kura/main.js",
            "themes/t/theme.css",
        ] {
            assert!(!theirs.allows(no), "{no}");
        }
        assert!(
            !profile.allows("themes/t/theme.css") && !profile.allows("plugins/weebo-kura/main.js")
        );
        assert!(Space::notes().allows("anything.md"));
    }

    #[test]
    fn a_choice_overrides_the_default() {
        let defaults = ["snippets".to_owned(), "plugin:dv".to_owned()];
        let choices = [
            ("plugin:dv".to_owned(), false),
            ("plugin:cal".to_owned(), true),
        ];
        assert_eq!(effective(&defaults, &choices), ["plugin:cal", "snippets"]);
        assert_eq!(effective(&[], &[]), Vec::<String>::new());
    }
}
