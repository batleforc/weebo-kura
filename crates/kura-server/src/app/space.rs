//! Where entries live (RFC 0001 §6.8, decision 34): the shared note
//! namespace, or the caller's own Obsidian configuration for its device's
//! profile. The client only asks for `config`; the profile is the device's
//! as registered, the owner the caller, and row-level security keeps every
//! other user's configuration out of reach.

use kura_api::spec::ConfigCategory;
use uuid::Uuid;

use super::SharedState;
use crate::authz::VaultAccess;
use crate::db;
use crate::problem::{ApiError, ProblemType};

/// A resolved space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Space {
    /// `notes`, `config:desktop` or `config:mobile`.
    pub namespace: String,
    /// The configuration's owner; `None` for notes.
    pub owner: Option<String>,
    pub categories: Vec<ConfigCategory>,
}

impl Space {
    pub fn notes() -> Self {
        Self {
            namespace: "notes".into(),
            owner: None,
            categories: Vec::new(),
        }
    }

    pub fn is_config(&self) -> bool {
        self.owner.is_some()
    }

    /// A path of this space may be written: any note, or a configuration
    /// file of a synced category, never the Kura plugin itself (§6.8).
    pub fn allows(&self, path: &str) -> bool {
        !self.is_config() || category(path).is_some_and(|c| self.categories.contains(&c))
    }
}

/// The category of a path under `.obsidian`.
/// Without regard to case: on a case-insensitive file system `Plugins/X/MAIN.JS`
/// is the same file (phase 8 review H2).
pub fn category(path: &str) -> Option<ConfigCategory> {
    let path = path.to_lowercase();
    let (top, rest) = path.split_once('/').unwrap_or((&path, ""));
    match (top, rest) {
        ("plugins", r) => {
            let (id, file) = r.split_once('/')?;
            (id != "weebo-kura" && !file.is_empty()).then_some(ConfigCategory::Plugins)
        }
        ("snippets", r) if !r.is_empty() => Some(ConfigCategory::Snippets),
        ("themes", r) if !r.is_empty() => Some(ConfigCategory::Themes),
        ("hotkeys.json", "") => Some(ConfigCategory::Hotkeys),
        _ => None,
    }
}

/// The space a request names (`space=config`, else notes), for this device.
pub async fn resolve(
    state: &SharedState,
    asked: Option<&str>,
    a: &VaultAccess,
    caller: Option<(&str, Uuid)>,
) -> Result<Space, ApiError> {
    match asked {
        None | Some("notes") => Ok(Space::notes()),
        Some("config") => {
            // A scoped token never reads configuration (decision 34).
            let Some((sub, device)) = caller else {
                return Err(ApiError::with_detail(
                    ProblemType::InsufficientRole,
                    "configuration is served to its owner's devices only",
                ));
            };
            if a.config.is_empty() {
                return Err(ApiError::with_detail(
                    ProblemType::InvalidRequest,
                    "this vault does not sync the Obsidian configuration",
                ));
            }
            let mut tx = db::begin_subject(&state.db, sub).await?;
            let profile: String = sqlx::query_scalar("SELECT profile FROM device WHERE id = $1")
                .bind(device)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(ApiError::of(ProblemType::DeviceUnknown))?;
            tx.commit().await?;
            // The console never holds an Obsidian configuration (RFC 0003 §6.2).
            if profile == "web" {
                return Err(ApiError::with_detail(
                    ProblemType::InsufficientRole,
                    "configuration is served to Obsidian devices only",
                ));
            }
            Ok(Space {
                namespace: format!("config:{profile}"),
                owner: Some(sub.to_owned()),
                categories: a.config.clone(),
            })
        }
        Some(other) => Err(ApiError::with_detail(
            ProblemType::InvalidRequest,
            format!("space: {other} is neither notes nor config"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_and_the_kura_plugin() {
        assert_eq!(
            category("plugins/dataview/main.js"),
            Some(ConfigCategory::Plugins)
        );
        assert_eq!(
            category("snippets/wide.css"),
            Some(ConfigCategory::Snippets)
        );
        assert_eq!(
            category("themes/Minimal/theme.css"),
            Some(ConfigCategory::Themes)
        );
        assert_eq!(category("hotkeys.json"), Some(ConfigCategory::Hotkeys));
        assert_eq!(
            category("Plugins/DataView/MAIN.JS"),
            Some(ConfigCategory::Plugins)
        );
        for never in [
            "plugins/weebo-kura/data.json",
            "plugins/x",
            "community-plugins.json",
            "workspace.json",
            "app.json",
            "snippets",
            "hotkeys.json/x",
            "plugins/WEEBO-KURA/data.json",
            "Plugins/Weebo-Kura/main.js",
        ] {
            assert_eq!(category(never), None, "{never}");
        }
        let s = Space {
            namespace: "config:desktop".into(),
            owner: Some("u".into()),
            categories: vec![ConfigCategory::Snippets],
        };
        assert!(s.allows("snippets/a.css") && !s.allows("plugins/x/main.js"));
        assert!(Space::notes().allows("anything.md"));
    }
}
