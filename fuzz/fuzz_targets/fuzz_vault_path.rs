//! Vault paths come from every client and, for each client, from the server
//! (RFC 0001 §6.3, T7, T14). Properties: parsing never panics; an accepted
//! path has no `.`/`..`/dot-leading segment and re-parses to itself; the
//! local (NFC-normalizing) parse accepts every strict path unchanged; a
//! suffixed path is valid and keeps its names within 255 bytes.
#![no_main]

use kura_core::VaultPath;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (String, String)| {
    let (text, suffix) = input;
    if let Ok(p) = VaultPath::parse(&text) {
        assert_eq!(p.as_str(), text);
        assert!(p.segments().all(|s| !s.is_empty() && !s.starts_with('.')));
        assert_eq!(VaultPath::from_local(&text).ok().as_ref(), Some(&p));
    }
    if let Ok(p) = VaultPath::from_local(&text) {
        assert_eq!(VaultPath::parse(p.as_str()).ok().as_ref(), Some(&p));
        if let Ok(s) = p.suffixed(&suffix) {
            assert!(s.name().len() <= kura_core::path::MAX_SEGMENT);
            assert_eq!(VaultPath::parse(s.as_str()).ok().as_ref(), Some(&s));
        }
    }
});
