//! RFC 0001 PR 2.6 (decision 52): deduplication and request counts of a real
//! vault under several chunking settings, with the chunker the clients use.
//! Reads the folder only, writes nothing, sends nothing. Notes only: a path
//! with a dot-segment (`.obsidian`, `.trash`) is skipped, as the notes
//! namespace refuses it.
//!
//! usage: cargo run --release -p kura-core --example chunk_stats -- <folder>

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use kura_core::{ChunkHash, ChunkParams, chunks};

/// Records and bytes one `chunks/batch` call carries (RFC 0002 §6.1).
const BATCH_RECORDS: usize = 1000;
const BATCH_BYTES: usize = 4 << 20;

const KIB: u64 = 1024;
/// Decision 52 first, then smaller settings to compare against.
const SETTINGS: &[(&str, u64, u64, u64)] = &[
    (
        "256K/1M/4M (decision 52)",
        256 * KIB,
        1024 * KIB,
        4096 * KIB,
    ),
    ("64K/256K/1M", 64 * KIB, 256 * KIB, 1024 * KIB),
    ("16K/64K/256K", 16 * KIB, 64 * KIB, 256 * KIB),
    ("4K/16K/64K", 4 * KIB, 16 * KIB, 64 * KIB),
];

fn files(root: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(root)? {
        let e = e?;
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let t = e.file_type()?;
        if t.is_dir() {
            files(&e.path(), out)?;
        } else if t.is_file() {
            out.push(e.path());
        }
    }
    Ok(())
}

/// Calls of `chunks/batch` to fetch `sizes` from nothing, packed greedily.
fn batches(sizes: &[usize]) -> usize {
    let (mut calls, mut n, mut bytes) = (0usize, 0usize, 0usize);
    for &s in sizes {
        if n > 0 && (n == BATCH_RECORDS || bytes.saturating_add(s) > BATCH_BYTES) {
            calls = calls.saturating_add(1);
            n = 0;
            bytes = 0;
        }
        n = n.saturating_add(1);
        bytes = bytes.saturating_add(s);
    }
    if n > 0 {
        calls = calls.saturating_add(1);
    }
    calls
}

fn pct(part: u64, whole: u64) -> String {
    if whole == 0 {
        return "-".into();
    }
    #[expect(clippy::cast_precision_loss, reason = "a printed ratio")]
    let r = part as f64 * 100.0 / whole as f64;
    format!("{r:.1}%")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args()
        .nth(1)
        .ok_or("usage: chunk_stats <folder>")?;
    let mut paths = Vec::new();
    files(Path::new(&root), &mut paths)?;
    let contents: Vec<Vec<u8>> = paths.iter().map(std::fs::read).collect::<Result<_, _>>()?;
    let total: u64 = contents.iter().map(|c| c.len() as u64).sum();
    println!(
        "folder: {} files, {} bytes (notes only, dot-paths skipped)",
        contents.len(),
        total
    );
    println!();
    println!(
        "{:<26} {:>8} {:>8} {:>13} {:>7} {:>11} {:>9}",
        "setting", "chunks", "unique", "unique bytes", "dedup", "chunk/file", "batches"
    );
    for (name, min, avg, max) in SETTINGS {
        let params = ChunkParams::new(*min, *avg, *max)?;
        let mut unique: HashMap<ChunkHash, usize> = HashMap::new();
        let mut count = 0usize;
        for data in &contents {
            for span in chunks(data, params) {
                let end = span.offset.saturating_add(span.len);
                let bytes = data.get(span.offset..end).ok_or("a chunk past its file")?;
                unique.insert(ChunkHash::of(bytes), span.len);
                count = count.saturating_add(1);
            }
        }
        let sizes: Vec<usize> = unique.values().copied().collect();
        let unique_bytes: u64 = sizes.iter().map(|s| *s as u64).sum();
        #[expect(clippy::cast_precision_loss, reason = "a printed average")]
        let per_file = count as f64 / contents.len().max(1) as f64;
        println!(
            "{:<26} {:>8} {:>8} {:>13} {:>7} {:>11.2} {:>9}",
            name,
            count,
            unique.len(),
            unique_bytes,
            pct(total.saturating_sub(unique_bytes), total),
            per_file,
            batches(&sizes),
        );
    }
    println!();
    println!(
        "dedup: bytes saved by identical chunks; batches: chunks/batch calls of a first attach."
    );
    Ok(())
}
