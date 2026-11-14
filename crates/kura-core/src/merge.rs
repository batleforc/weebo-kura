//! Three-way line merge of text files (RFC 0001 §6.5, decision 43): `.md`
//! and `.txt` only, UTF-8 only, at most 4 MiB per side (§7.3). Anything else,
//! and any overlapping edit, is a conflict: both versions are then kept.

use crate::path::VaultPath;

/// Bytes per side (§7.3).
pub const MAX_MERGE: usize = 4 << 20;
/// Lines per side: `diffy`'s diff is quadratic in lines when the sides share
/// few, about 0.1 s at 4,000 lines a side; 20,000 bounds a merge to seconds
/// (review 4 H1, plan D54).
pub const MAX_MERGE_LINES: usize = 20_000;

/// Only line-oriented text merges safely; `.canvas` and other JSON would not
/// parse after a textual merge (§6.5).
pub fn mergeable(path: &VaultPath) -> bool {
    path.name().rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && (ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("txt"))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Merge {
    /// Both edits apply: the merged content.
    Clean(Vec<u8>),
    /// Overlapping edits, binary content or too large.
    Conflict,
}

/// Merges `ours` and `theirs`, both edited from `base`.
pub fn merge3(base: &[u8], ours: &[u8], theirs: &[u8]) -> Merge {
    if [base, ours, theirs]
        .iter()
        .any(|s| s.len() > MAX_MERGE || s.iter().filter(|b| **b == b'\n').count() > MAX_MERGE_LINES)
    {
        return Merge::Conflict;
    }
    let (Ok(b), Ok(o), Ok(t)) = (
        std::str::from_utf8(base),
        std::str::from_utf8(ours),
        std::str::from_utf8(theirs),
    ) else {
        return Merge::Conflict;
    };
    match diffy::merge(b, o, t) {
        Ok(m) => Merge::Clean(m.into_bytes()),
        Err(_) => Merge::Conflict,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_of_different_lines_merge() {
        let base = b"# Plan\none\ntwo\nthree\n";
        let ours = b"# Plan\none, edited here\ntwo\nthree\n";
        let theirs = b"# Plan\none\ntwo\nthree, edited there\n";
        assert_eq!(
            merge3(base, ours, theirs),
            Merge::Clean(b"# Plan\none, edited here\ntwo\nthree, edited there\n".to_vec())
        );
    }

    #[test]
    fn overlapping_binary_or_large_content_conflicts() {
        assert_eq!(merge3(b"a\n", b"b\n", b"c\n"), Merge::Conflict);
        assert_eq!(merge3(b"a\n", &[0xff, b'\n'], b"a\nb\n"), Merge::Conflict);
        let big = vec![b'x'; MAX_MERGE + 1];
        assert_eq!(merge3(b"", &big, b"y"), Merge::Conflict);
        let many = "l\n".repeat(MAX_MERGE_LINES + 1);
        assert_eq!(merge3(b"", many.as_bytes(), b"y\n"), Merge::Conflict);
        // The same edit on both sides is clean.
        assert_eq!(
            merge3(b"a\n", b"b\n", b"b\n"),
            Merge::Clean(b"b\n".to_vec())
        );
    }

    #[test]
    fn only_markdown_and_text_merge() {
        let m = |p: &str| mergeable(&VaultPath::parse(p).unwrap());
        assert!(m("notes/a.md") && m("A.MD") && m("todo.txt"));
        assert!(!m("board.canvas") && !m("data.json") && !m("README") && !m("x.md.bak"));
    }
}
