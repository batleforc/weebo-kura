//! Types and algorithms shared by kura-server and its clients (RFC 0001
//! §5.1): chunk addressing and content-defined chunking (§6.4), vault paths and
//! the sync protocol's wire types (§6.3). No I/O, no
//! panic on input: this crate also runs in WASM inside Obsidian (§6.11).

pub mod chunk;
#[cfg(feature = "merge")]
pub mod merge;
pub mod path;
pub mod sync;
pub mod token;
pub mod wire;

pub use chunk::{ChunkHash, ChunkParams, ChunkSpan, blob_hash, chunks};
pub use path::{PathError, VaultPath};
pub use wire::EntryId;
