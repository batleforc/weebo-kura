//! Folder scopes (RFC 0004 §5.1): the rules a scoped token carries, and the
//! folders a restricted viewer reads (RFC 0005). One comparison form for the
//! server, the CLI and the fuzz build: full Unicode case folding, then NFC.

use serde::{Deserialize, Serialize};

use crate::VaultPath;

/// Folder rules per token (RFC 0004 decision 8).
pub const MAX_SCOPES: usize = 16;

/// Full Unicode case folding then NFC: the one comparison form of paths and
/// token prefixes (§6.3), so a prefix check and a uniqueness check agree.
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization as _;
    caseless::default_case_fold_str(s).nfc().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum ScopeMode {
    Read,
    /// Also reads that folder, and nothing else (decision 9).
    Write,
}

/// One folder rule: the folder and everything under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeRule {
    /// A folder ending with `/`; `""` is the whole vault, `read` only.
    pub path: String,
    pub mode: ScopeMode,
}

/// Why a list of rules is refused (§4.3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    #[error("a folder ending with /")]
    NotAFolder,
    #[error("{0}")]
    Path(crate::PathError),
    #[error("prefix and scopes: give one of them")]
    Both,
    #[error("1 to 16 folder rules")]
    Count,
    #[error("write needs a folder; a token never writes the whole vault")]
    WholeVaultWrite,
    #[error("{0:?} is given twice")]
    Twice(String),
}

/// `""`, or a valid vault path followed by `/`.
pub fn check_folder(p: &str) -> Result<(), ScopeError> {
    if p.is_empty() {
        return Ok(());
    }
    let folder = p.strip_suffix('/').ok_or(ScopeError::NotAFolder)?;
    VaultPath::parse(folder).map_err(ScopeError::Path)?;
    Ok(())
}

/// The rules a token creation means (RFC 0004 §4.2, §4.3): `prefix` is one
/// `read` rule; nothing at all is the whole vault, read only.
pub fn check_rules(
    prefix: Option<String>,
    scopes: Option<Vec<ScopeRule>>,
) -> Result<Vec<ScopeRule>, ScopeError> {
    let rules = match (prefix, scopes) {
        (Some(_), Some(_)) => return Err(ScopeError::Both),
        (prefix, None) => vec![ScopeRule {
            path: prefix.unwrap_or_default(),
            mode: ScopeMode::Read,
        }],
        (None, Some(rules)) => rules,
    };
    if rules.is_empty() || rules.len() > MAX_SCOPES {
        return Err(ScopeError::Count);
    }
    let mut seen = std::collections::HashSet::new();
    for r in &rules {
        check_folder(&r.path)?;
        if r.path.is_empty() && r.mode == ScopeMode::Write {
            return Err(ScopeError::WholeVaultWrite);
        }
        if !seen.insert(fold(&r.path)) {
            return Err(ScopeError::Twice(r.path.clone()));
        }
    }
    Ok(rules)
}

/// The scope a list of rules means; `None` when it means none (T7).
pub fn scope_of(rules: &[ScopeRule]) -> Option<PathScope> {
    PathScope::from_rules(
        rules
            .iter()
            .map(|r| (r.path.as_str(), r.mode == ScopeMode::Write)),
    )
}

/// Where a caller may read and write inside a vault (RFC 0004 §5.1): folded
/// folder prefixes. "The whole vault" is `read: None`, never an empty list,
/// so a scope that lost its rules sees nothing (T7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathScope {
    read: Option<Vec<String>>,
    /// Each is also readable (decision 9).
    write: Vec<String>,
}

impl PathScope {
    /// Every path, read only: a signed-in device, or a token without rules.
    pub fn whole() -> Self {
        Self {
            read: None,
            write: Vec::new(),
        }
    }

    /// From `(path, writes)` rules, where `""` is the whole vault. `None`
    /// for no rule at all, or a whole-vault write: neither is a scope.
    pub fn from_rules<'a>(rules: impl IntoIterator<Item = (&'a str, bool)>) -> Option<Self> {
        let (mut whole, mut read, mut write, mut any) = (false, Vec::new(), Vec::new(), false);
        for (path, writes) in rules {
            any = true;
            match (path.is_empty(), writes) {
                (true, true) => return None,
                (true, false) => whole = true,
                (false, true) => write.push(fold(path)),
                (false, false) => read.push(fold(path)),
            }
        }
        any.then_some(Self {
            read: (!whole).then_some(read),
            write,
        })
    }

    /// §6.2, T16: whether a path is under a readable folder.
    pub fn sees(&self, path: &str) -> bool {
        let Some(read) = &self.read else { return true };
        let key = fold(path);
        read.iter().chain(&self.write).any(|p| key.starts_with(p))
    }

    /// Whether a path is under a `write` folder; reading grants no write.
    pub fn writes(&self, path: &str) -> bool {
        let key = fold(path);
        self.write.iter().any(|p| key.starts_with(p))
    }

    pub fn can_write(&self) -> bool {
        !self.write.is_empty()
    }

    /// The readable folders as SQL patterns for `path_key LIKE ANY($n)`
    /// (backslash is `LIKE`'s default escape), wildcards escaped. No
    /// pattern matches no row.
    pub fn like_patterns(&self) -> Vec<String> {
        let Some(read) = &self.read else {
            return vec!["%".to_owned()];
        };
        read.iter()
            .chain(&self.write)
            .map(|prefix| {
                let mut out = String::with_capacity(prefix.len().saturating_add(1));
                for c in prefix.chars() {
                    if matches!(c, '\\' | '%' | '_') {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('%');
                out
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_reads_and_writes_under_its_folders_only() {
        let s = PathScope::from_rules([("Sessions/", false), ("Résumés/", true)]).unwrap();
        assert!(s.sees("sessions/2026/a.md"), "folded");
        assert!(
            s.sees("RÉSUMÉS/a.md") && s.writes("Re\u{301}sume\u{301}s/a.md"),
            "NFC"
        );
        assert!(!s.writes("Sessions/a.md"), "read grants no write");
        for outside in [
            "MJ/secret.md",
            "Sessions2/a.md",
            "Sessions",
            "a/Sessions/b.md",
        ] {
            assert!(!s.sees(outside) && !s.writes(outside), "{outside}");
        }
        // The whole vault readable, one folder writable.
        let s = PathScope::from_rules([("", false), ("Out/", true)]).unwrap();
        assert!(s.sees("MJ/secret.md") && !s.writes("MJ/secret.md") && s.writes("out/x.md"));
        assert_eq!(s.like_patterns(), ["%"]);
        assert!(PathScope::whole().sees("x.md") && !PathScope::whole().can_write());
    }

    /// RFC 0004 T7: nothing decodes to "everything" by accident.
    #[test]
    fn empty_scope_sees_nothing() {
        assert_eq!(PathScope::from_rules([]), None);
        assert_eq!(
            PathScope::from_rules([("", true)]),
            None,
            "whole-vault write"
        );
        // Write rules only: the read list is empty, not absent.
        let s = PathScope::from_rules([("Out/", true)]).unwrap();
        assert!(!s.sees("x.md") && s.sees("Out/x.md"));
        assert_eq!(s.like_patterns(), ["out/%"]);
    }

    #[test]
    fn like_patterns_escape_wildcards() {
        let s = PathScope::from_rules([("a_b%c\\d/", false)]).unwrap();
        assert_eq!(s.like_patterns(), ["a\\_b\\%c\\\\d/%"]);
    }
}
