//! Vault paths (RFC 0001 §6.3): parsed once, at every boundary, into a type
//! that cannot hold an invalid value. The server parses what clients send;
//! clients parse what the server sends before writing anything (T7, T14).

use std::fmt;

use unicode_normalization::{UnicodeNormalization as _, is_nfc};

/// The prefix of every path refusal while deserializing, so a server can
/// answer `invalid_path` rather than a generic parse error.
pub const INVALID_PATH: &str = "invalid path";

/// The longest text after a dot kept as an extension, and the longest
/// suffix, in bytes: a suffixed name always fits in 255 bytes.
const MAX_EXTENSION: usize = 16;
const MAX_SUFFIX: usize = 64;

/// Bytes per segment and per path (§6.3).
pub const MAX_SEGMENT: usize = 255;
pub const MAX_PATH: usize = 4096;

/// A relative, `/`-separated, NFC path in the note namespace.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VaultPath(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("the path is empty")]
    Empty,
    #[error("the path is longer than 4,096 bytes")]
    TooLong,
    #[error("a name is longer than 255 bytes")]
    SegmentTooLong,
    #[error("the path is absolute or has an empty name")]
    EmptySegment,
    #[error("\".\" and \"..\" are not allowed as a name")]
    DotSegment,
    #[error("a name starting with \".\" is not synced as a note")]
    Hidden,
    #[error("a name may not end with a dot or a space")]
    TrailingDotOrSpace,
    #[error("{0:?} is not allowed in a name")]
    Forbidden(char),
    #[error("control characters are not allowed in a name")]
    Control,
    #[error("the path is not in Unicode NFC form")]
    NotNfc,
    #[error("invisible characters are not allowed in a name")]
    Ignorable,
    #[error("a name like \"NAME~1\" can stand for another file on Windows")]
    ShortName,
}

impl VaultPath {
    /// Strict: refuses a path that is not already NFC. The wire carries NFC only.
    pub fn parse(s: &str) -> Result<Self, PathError> {
        if !is_nfc(s) {
            return Err(PathError::NotNfc);
        }
        check(s)?;
        Ok(Self(s.to_owned()))
    }

    /// A path read from a local file system: normalized to NFC first (macOS
    /// file names come decomposed), then checked like [`VaultPath::parse`].
    pub fn from_local(s: &str) -> Result<Self, PathError> {
        let nfc: String = s.nfc().collect();
        check(&nfc)?;
        Ok(Self(nfc))
    }

    /// Another path that only differs by case: the same file on a
    /// case-insensitive disk (macOS, Windows, Android).
    pub fn same_but_case(&self, other: &Self) -> bool {
        self != other && self.0.to_lowercase() == other.0.to_lowercase()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// The last segment.
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// The same directory with another last segment, checked.
    pub fn with_name(&self, name: &str) -> Result<Self, PathError> {
        match self.0.rsplit_once('/') {
            Some((dir, _)) => Self::from_local(&format!("{dir}/{name}")),
            None => Self::from_local(name),
        }
    }

    /// `notes/a.md` → `notes/a (suffix).md`, cut so the name stays within 255
    /// bytes. Used for conflict files and occupied paths (§6.5).
    pub fn suffixed(&self, suffix: &str) -> Result<Self, PathError> {
        let name = self.name();
        // A long text after the last dot is not an extension: keep it in the stem.
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() && e.len() <= MAX_EXTENSION => (s, Some(e)),
            _ => (name, None),
        };
        let mut clean: String = suffix
            .chars()
            .filter(|c| !ignorable(*c))
            .map(|c| if allowed_char(c) { c } else { '-' })
            .collect();
        // Bounded in bytes, so the name always fits (review 3.6 F4).
        let mut keep = clean.len().min(MAX_SUFFIX);
        while !clean.is_char_boundary(keep) {
            keep = keep.saturating_sub(1);
        }
        clean.truncate(keep);
        let tail = match ext {
            Some(e) => format!(" ({clean}).{e}"),
            None => format!(" ({clean})"),
        };
        let budget = MAX_SEGMENT.saturating_sub(tail.len());
        let mut cut = stem.len().min(budget);
        while !stem.is_char_boundary(cut) {
            cut = cut.saturating_sub(1);
        }
        let stem = stem
            .get(..cut)
            .unwrap_or_default()
            .trim_end_matches(['.', ' ']);
        self.with_name(&format!("{stem}{tail}"))
    }
}

/// Unicode default-ignorable code points: invisible, and dropped by some
/// file systems (HFS+), so `\u{200C}.obsidian` would become `.obsidian` (T14).
fn ignorable(c: char) -> bool {
    matches!(
        u32::from(c),
        0xAD | 0x34F
            | 0x61C
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0xFFF0..=0xFFF8
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
}

/// `OBSIDI~1`: an 8.3 short name, which NTFS may resolve to another file
/// (`.obsidian`), as git refuses `git~1` (T14).
fn short_name(seg: &str) -> bool {
    let base = seg.split('.').next().unwrap_or(seg);
    match base.rsplit_once('~') {
        Some((head, digits)) => {
            !head.is_empty()
                && head.chars().count() <= 6
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

fn allowed_char(c: char) -> bool {
    !c.is_control() && !matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*' | '/')
}

fn check(s: &str) -> Result<(), PathError> {
    if s.is_empty() {
        return Err(PathError::Empty);
    }
    if s.len() > MAX_PATH {
        return Err(PathError::TooLong);
    }
    for seg in s.split('/') {
        if seg.is_empty() {
            return Err(PathError::EmptySegment);
        }
        if seg.len() > MAX_SEGMENT {
            return Err(PathError::SegmentTooLong);
        }
        if seg == "." || seg == ".." {
            return Err(PathError::DotSegment);
        }
        // Note namespace: dot-folders travel only through the configuration
        // namespace, never as notes (decision 28).
        if seg.starts_with('.') {
            return Err(PathError::Hidden);
        }
        if seg.ends_with(['.', ' ']) {
            return Err(PathError::TrailingDotOrSpace);
        }
        if short_name(seg) {
            return Err(PathError::ShortName);
        }
        for c in seg.chars() {
            if ignorable(c) {
                return Err(PathError::Ignorable);
            }
            if c.is_control() {
                return Err(PathError::Control);
            }
            if !allowed_char(c) {
                return Err(PathError::Forbidden(c));
            }
        }
    }
    Ok(())
}

impl fmt::Display for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for VaultPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VaultPath({:?})", self.0)
    }
}

impl serde::Serialize for VaultPath {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for VaultPath {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).map_err(|e| serde::de::Error::custom(format!("{INVALID_PATH}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotdot_is_rejected() {
        for bad in ["..", "../a.md", "a/../b.md", "a/./b.md", ".", "a/.."] {
            assert_eq!(
                VaultPath::parse(bad).unwrap_err(),
                PathError::DotSegment,
                "{bad}"
            );
        }
        assert_eq!(
            VaultPath::parse("/etc/passwd").unwrap_err(),
            PathError::EmptySegment
        );
        assert_eq!(
            VaultPath::parse("a//b").unwrap_err(),
            PathError::EmptySegment
        );
        assert_eq!(VaultPath::parse("a/").unwrap_err(), PathError::EmptySegment);
        assert_eq!(
            VaultPath::parse("a\\..\\b").unwrap_err(),
            PathError::Forbidden('\\')
        );
    }

    #[test]
    fn dot_path_in_note_namespace_is_rejected() {
        for bad in [
            ".obsidian/plugins/x/main.js",
            ".git/config",
            "notes/.trash/a.md",
            ".hidden",
        ] {
            assert_eq!(
                VaultPath::parse(bad).unwrap_err(),
                PathError::Hidden,
                "{bad}"
            );
        }
        assert!(VaultPath::parse("notes/a.b.md").is_ok());
    }

    #[test]
    fn names_follow_the_rules() {
        assert!(VaultPath::parse("Projets/Q3 budget.md").is_ok());
        assert!(VaultPath::parse("Été/café.md").is_ok());
        for (bad, err) in [
            ("", PathError::Empty),
            ("a.", PathError::TrailingDotOrSpace),
            ("a /b", PathError::TrailingDotOrSpace),
            ("Q3: budget.md", PathError::Forbidden(':')),
            ("what?.md", PathError::Forbidden('?')),
            ("a\u{7}.md", PathError::Control),
            ("a\nb", PathError::Control),
        ] {
            assert_eq!(VaultPath::parse(bad).unwrap_err(), err, "{bad:?}");
        }
        assert_eq!(
            VaultPath::parse(&"a".repeat(256)).unwrap_err(),
            PathError::SegmentTooLong
        );
        assert!(VaultPath::parse(&"a".repeat(255)).is_ok());
        let long = vec!["a".repeat(200); 21].join("/");
        assert_eq!(VaultPath::parse(&long).unwrap_err(), PathError::TooLong);
    }

    #[test]
    fn nfc_is_required_on_the_wire_and_applied_locally() {
        let decomposed = "Cafe\u{301}.md";
        assert_eq!(VaultPath::parse(decomposed).unwrap_err(), PathError::NotNfc);
        let p = VaultPath::from_local(decomposed).unwrap();
        assert_eq!(p.as_str(), "Café.md");
        assert_eq!(VaultPath::parse(p.as_str()).unwrap(), p);
    }

    #[test]
    fn suffixes_stay_valid_and_bounded() {
        let p = VaultPath::parse("notes/Meeting.md").unwrap();
        assert_eq!(
            p.suffixed("conflict desk:01 2026-11-14").unwrap().as_str(),
            "notes/Meeting (conflict desk-01 2026-11-14).md"
        );
        assert_eq!(
            VaultPath::parse("README")
                .unwrap()
                .suffixed("2")
                .unwrap()
                .as_str(),
            "README (2)"
        );
        assert_eq!(VaultPath::parse(".md").unwrap_err(), PathError::Hidden);
        let long = VaultPath::parse(&format!("{}é.md", "a".repeat(250))).unwrap();
        let s = long.suffixed("conflict pixel 2026-11-14").unwrap();
        assert!(s.name().len() <= MAX_SEGMENT);
        assert!(s.name().ends_with(" (conflict pixel 2026-11-14).md"));
        assert_eq!(p.name(), "Meeting.md");
        assert_eq!(p.segments().count(), 2);
    }

    #[test]
    fn invisible_and_short_names_cannot_stand_for_a_dot_folder() {
        for bad in [
            "\u{200C}.obsidian/plugins/x/main.js",
            "a\u{FEFF}b.md",
            "x\u{200B}/y.md",
            "a\u{E0001}.md",
        ] {
            assert_eq!(
                VaultPath::parse(bad).unwrap_err(),
                PathError::Ignorable,
                "{bad:?}"
            );
        }
        for bad in ["OBSIDI~1/plugins/x/main.js", "GIT~1", "notes/PROGRA~12.TXT"] {
            assert_eq!(
                VaultPath::parse(bad).unwrap_err(),
                PathError::ShortName,
                "{bad}"
            );
        }
        for ok in ["a~b.md", "~1.md", "notes~draft/x.md", "toolongname~1.md"] {
            assert!(VaultPath::parse(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn suffixes_always_fit() {
        let long_ext = VaultPath::parse(&format!("v1.{}", "x".repeat(250))).unwrap();
        let device = "\u{1F600}".repeat(64);
        for p in [&long_ext, &VaultPath::parse(&"é".repeat(127)).unwrap()] {
            let s = p
                .suffixed(&format!("conflict {device} 2026-11-14"))
                .unwrap();
            assert!(s.name().len() <= MAX_SEGMENT, "{}", s.name().len());
        }
        assert!(
            VaultPath::parse("a.md")
                .unwrap()
                .suffixed("x\u{200B}y")
                .unwrap()
                .as_str()
                == "a (xy).md"
        );
    }

    #[test]
    fn serde_parses_strictly() {
        let p: VaultPath = serde_json::from_str("\"a/b.md\"").unwrap();
        assert_eq!(serde_json::to_string(&p).unwrap(), "\"a/b.md\"");
        assert!(serde_json::from_str::<VaultPath>("\"../x\"").is_err());
        assert_eq!(format!("{p:?}"), "VaultPath(\"a/b.md\")");
    }
}
