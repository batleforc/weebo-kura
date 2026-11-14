//! The three-way merge runs on content any editor controls (RFC 0001 §6.5,
//! §7.3). Properties: it never panics; an unchanged side yields the other
//! side; identical edits yield that edit.
#![no_main]

use kura_core::merge::{Merge, merge3};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (String, String, String)| {
    let (base, ours, theirs) = input;
    let (b, o, t) = (base.as_bytes(), ours.as_bytes(), theirs.as_bytes());
    let _ = merge3(b, o, t);
    assert_eq!(merge3(b, b, t), Merge::Clean(t.to_vec()));
    assert_eq!(merge3(b, o, b), Merge::Clean(o.to_vec()));
    assert_eq!(merge3(b, o, o), Merge::Clean(o.to_vec()));
});
