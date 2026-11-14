//! Which files of `.obsidian` sync (RFC 0001 §6.8, decision 34): by
//! category, per user and per device profile. Paths are relative to
//! `.obsidian`. The server applies the same rule; clients filter with it so
//! they never offer a file the server refuses, and never the files that
//! must stay on the device: `community-plugins.json` (what is enabled),
//! `workspace*.json`, and the Kura plugin itself.

/// The category of a path under `.obsidian`, as `configSync` names it.
///
/// Without regard to case: on a case-insensitive file system `MAIN.JS` is
/// `main.js` and `WEEBO-KURA` is the Kura plugin (phase 8 review H2).
pub fn category(path: &str) -> Option<&'static str> {
    let path = path.to_lowercase();
    let (top, rest) = path.split_once('/').unwrap_or((&path, ""));
    match (top, rest) {
        ("plugins", r) => {
            let (id, file) = r.split_once('/')?;
            (id != "weebo-kura" && !file.is_empty()).then_some("plugins")
        }
        ("snippets", r) if !r.is_empty() => Some("snippets"),
        ("themes", r) if !r.is_empty() => Some("themes"),
        ("hotkeys.json", "") => Some("hotkeys"),
        _ => None,
    }
}

/// Whether a path syncs with these categories enabled.
pub fn synced(path: &str, categories: &[String]) -> bool {
    category(path).is_some_and(|c| categories.iter().any(|x| x == c))
}

/// The plugin a path belongs to (its id folded to lower case) and whether
/// it is its code (`main.js`, in any case).
pub fn plugin_of(path: &str) -> Option<(String, bool)> {
    let lower = path.to_lowercase();
    let rest = lower.strip_prefix("plugins/")?;
    let (id, file) = rest.split_once('/')?;
    Some((id.to_owned(), file == "main.js"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_never_include_what_stays_on_the_device() {
        let all = ["plugins", "snippets", "themes", "hotkeys"].map(String::from);
        for yes in [
            "plugins/dataview/main.js",
            "snippets/a.css",
            "themes/T/theme.css",
            "hotkeys.json",
        ] {
            assert!(synced(yes, &all), "{yes}");
        }
        for no in [
            "community-plugins.json",
            "workspace.json",
            "workspace-mobile.json",
            "plugins/weebo-kura/data.json",
            "plugins/x",
            "app.json",
        ] {
            assert!(!synced(no, &all), "{no}");
        }
        assert!(!synced("hotkeys.json", &["plugins".into()]));
        assert_eq!(plugin_of("plugins/dv/main.js"), Some(("dv".into(), true)));
        assert_eq!(plugin_of("Plugins/DV/MAIN.JS"), Some(("dv".into(), true)));
        assert_eq!(
            plugin_of("plugins/dv/manifest.json"),
            Some(("dv".into(), false))
        );
        assert!(!synced("plugins/WEEBO-KURA/data.json", &all));
        assert_eq!(plugin_of("snippets/a.css"), None);
    }
}
