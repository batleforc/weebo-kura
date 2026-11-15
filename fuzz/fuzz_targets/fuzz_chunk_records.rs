//! `chunks/batch` answers come from the server into every client (RFC 0002
//! T6). Properties: parsing never panics; every record's bytes are at most a
//! chunk; the records written back are the body, byte for byte, so nothing
//! is skipped or invented.
#![no_main]

use kura_core::wire::parse_records;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|body: &[u8]| {
    if let Ok(records) = parse_records(body) {
        let mut back = Vec::with_capacity(body.len());
        for (hash, data) in records {
            back.extend_from_slice(hash.as_bytes());
            back.push(if data.is_some() { 0 } else { 1 });
            let data = data.unwrap_or_default();
            assert!(data.len() <= kura_core::chunk::MAX_CHUNK);
            back.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
            back.extend_from_slice(data);
        }
        assert_eq!(back, body);
    }
});
