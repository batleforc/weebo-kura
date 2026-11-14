//! Types and algorithms shared by kura-server and its clients (RFC 0001
//! §5.1): chunk addressing and content-defined chunking (§6.4). No I/O, no
//! panic on input: this crate also runs in WASM inside Obsidian (§6.11).

pub mod chunk;

pub use chunk::{ChunkHash, ChunkParams, ChunkSpan, blob_hash, chunks};
