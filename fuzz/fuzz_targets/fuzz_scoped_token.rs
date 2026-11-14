//! Scoped tokens arrive in `Authorization` from anyone on the network
//! (RFC 0001 §6.2, T5). Properties: parsing never panics; an accepted token
//! is exactly the text its parts write back, so no second spelling of one
//! token exists (case, padding, separators).
#![no_main]

use kura_core::token::ScopedToken;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    if let Ok(t) = ScopedToken::parse(text) {
        assert_eq!(t.expose(), text);
    }
});
