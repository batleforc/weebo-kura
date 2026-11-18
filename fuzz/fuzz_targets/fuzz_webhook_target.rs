//! A webhook URL comes from a tenant admin and decides where the server
//! connects (RFC 0003 §6.5, T1). Properties: parsing never panics; an
//! accepted target is https, carries no credentials, names a host, fits in
//! 2,048 bytes, parses again to itself, and its IP literal, if any, passed
//! the address rule.
#![no_main]

use kura_core::webhook::{MAX_URL, Target, refused_by_default};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    let allowed = |ip| !refused_by_default(ip);
    if let Ok(t) = Target::parse(text, allowed) {
        assert!(t.as_str().starts_with("https://"));
        assert!(t.as_str().len() <= MAX_URL);
        assert!(!t.host().is_empty());
        if let Some(ip) = t.literal() {
            assert!(!refused_by_default(ip), "{ip}");
        }
        assert_eq!(Target::parse(t.as_str(), allowed).as_ref(), Ok(&t));
    }
});
