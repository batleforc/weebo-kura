//! A commit body is the largest structured input a client sends (RFC 0001
//! §6.3, T17). Properties: parsing never panics, and an accepted body
//! serializes back to one that parses to the same value.
#![no_main]

use kura_core::wire::CommitRequest;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(c) = serde_json::from_slice::<CommitRequest>(data) {
        let again = serde_json::to_vec(&c).expect("a parsed body serializes");
        assert_eq!(serde_json::from_slice::<CommitRequest>(&again).ok().as_ref(), Some(&c));
    }
});
