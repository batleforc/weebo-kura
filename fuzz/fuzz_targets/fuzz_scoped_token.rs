//! Scoped tokens arrive in `Authorization` from anyone on the network
//! (RFC 0001 §6.2, T5), and their folder rules from a manager (RFC 0004 §7.3).
//! Properties: parsing a token never panics and an accepted one is exactly
//! the text its parts write back; checking rules never panics; accepted rules
//! always make a scope; that scope writes only where it reads, never writes
//! the whole vault, and its SQL patterns match exactly the paths it sees.
#![no_main]

use kura_core::scope::{ScopeMode, ScopeRule, check_rules, fold, scope_of};
use kura_core::token::ScopedToken;
use libfuzzer_sys::arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

/// `LIKE` on a pattern of `like_patterns`: an escaped prefix then `%`.
fn like(pattern: &str, key: &str) -> bool {
    let Some(body) = pattern.strip_suffix('%') else { return false };
    let mut prefix = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        prefix.push(if c == '\\' { chars.next().unwrap_or('\\') } else { c });
    }
    key.starts_with(&prefix)
}

fuzz_target!(|data: &[u8]| {
    let mut u = Unstructured::new(data);
    let text: &str = Arbitrary::arbitrary(&mut u).unwrap_or_default();
    if let Ok(t) = ScopedToken::parse(text) {
        assert_eq!(t.expose(), text);
    }
    let prefix: Option<String> = Arbitrary::arbitrary(&mut u).unwrap_or_default();
    let raw: Option<Vec<(String, bool)>> = Arbitrary::arbitrary(&mut u).unwrap_or_default();
    let scopes = raw.map(|rules| {
        rules
            .into_iter()
            .map(|(path, write)| ScopeRule { path, mode: if write { ScopeMode::Write } else { ScopeMode::Read } })
            .collect()
    });
    let paths: Vec<String> = Arbitrary::arbitrary(&mut u).unwrap_or_default();
    let Ok(rules) = check_rules(prefix, scopes) else { return };
    let scope = scope_of(&rules).expect("accepted rules make a scope");
    let any_write = rules.iter().any(|r| r.mode == ScopeMode::Write);
    assert_eq!(scope.can_write(), any_write);
    let patterns = scope.like_patterns();
    for p in paths.iter().chain(rules.iter().map(|r| &r.path)) {
        if scope.writes(p) {
            assert!(scope.sees(p), "writes {p:?} without seeing it");
        }
        assert_eq!(patterns.iter().any(|pat| like(pat, &fold(p))), scope.sees(p), "{p:?} {patterns:?}");
    }
    // A write rule never opens the whole vault to writing.
    assert!(!scope.writes("x.md") || rules.iter().any(|r| r.mode == ScopeMode::Write && fold("x.md").starts_with(&fold(&r.path))));
});
