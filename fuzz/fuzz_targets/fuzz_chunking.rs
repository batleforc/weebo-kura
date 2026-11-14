//! Chunk hashes come from URLs and chunk parameters from configuration; the
//! chunker runs in WASM, where a panic kills sync (RFC 0001 §6.11).
//! Properties: a hash parses back to itself; any accepted parameters cut any
//! input into chunks that reassemble it, each within `max`.
#![no_main]

use kura_core::{ChunkHash, ChunkParams, chunks};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: (String, u32, u32, u32, Vec<u8>)| {
    let (text, min, avg, max, data) = input;
    if let Ok(h) = text.parse::<ChunkHash>() {
        assert_eq!(h.to_string(), text);
    }
    // Raw values first (refusal must not panic), then values folded into the
    // accepted ranges, so the chunker itself is exercised most of the time.
    let _ = ChunkParams::new(min.into(), avg.into(), max.into());
    let min = 64 + u64::from(min % 4096) * 2;
    let avg = min + 2 + u64::from(avg % 8192) * 2;
    let max = (avg + 2 + u64::from(max % 16384) * 2).max(1024);
    let Ok(params) = ChunkParams::new(min, avg, max) else {
        return;
    };
    let mut rebuilt = Vec::with_capacity(data.len());
    for span in chunks(&data, params) {
        assert!(span.len <= params.max());
        let part = &data[span.offset..span.offset + span.len];
        assert_eq!(ChunkHash::of(part), span.hash);
        rebuilt.extend_from_slice(part);
    }
    assert_eq!(rebuilt, data);
});
