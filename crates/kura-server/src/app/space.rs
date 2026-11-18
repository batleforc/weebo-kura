//! Where entries live (RFC 0001 §6.8, decision 34): the shared note
//! namespace, or the caller's own Obsidian configuration for its device's
//! profile. The client only asks for `config`; the profile is the device's
//! as registered, the owner the caller, and row-level security keeps every
//! other user's configuration out of reach.

use crate::authz::VaultAccess;
use crate::problem::{ApiError, ProblemType};
use kura_api::spec::ConfigCategory;

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

/// The space a request names (`space=config`, else notes), for this
/// device: `caller` is its subject and registered profile.
pub fn resolve(
    asked: Option<&str>,
    a: &VaultAccess,
    caller: Option<(&str, &str)>,
    usage: Use<'_>,
) -> Result<Space, ApiError> {
    match asked {
        None | Some("notes") => Ok(Space::notes()),
        Some("config") => {
            // A scoped token never reads configuration (decision 34).
            let Some((sub, device_profile)) = caller else {
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
            let named = match usage {
                Use::List { profile } => profile,
                Use::Changes | Use::Write => None,
            };
            if let Some(p) = named
                && !matches!(p, "desktop" | "mobile")
            {
                return Err(ApiError::with_detail(
                    ProblemType::InvalidRequest,
                    format!("profile: {p} is neither desktop nor mobile"),
                ));
            }
            let profile = if device_profile == "web" {
                // The console lists, by a profile it names; it never follows
                // or writes the configuration (RFC 0008 §5.1).
                match (usage, named) {
                    (Use::List { .. }, Some(p)) => p,
                    (Use::List { .. }, None) => {
                        return Err(ApiError::with_detail(
                            ProblemType::InvalidRequest,
                            "profile: the console names desktop or mobile",
                        ));
                    }
                    (Use::Changes | Use::Write, _) => {
                        return Err(ApiError::with_detail(
                            ProblemType::InsufficientRole,
                            "configuration is served to Obsidian devices only",
                        ));
                    }
                }
            } else {
                // An Obsidian device reads its own profile only.
                if named.is_some_and(|p| p != device_profile) {
                    return Err(ApiError::with_detail(
                        ProblemType::InvalidRequest,
                        "profile: a device reads its own profile only",
                    ));
                }
                device_profile
            };
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
