//! Chunks: addressed by the BLAKE3 hash of their plaintext, cut by FastCDC
//! (2020) with the vault's parameters, read from the server, never compiled in.

use std::fmt;
use std::str::FromStr;

/// The BLAKE3 hash of a chunk's plaintext; 64 lowercase hex characters on the wire.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkHash([u8; 32]);

impl ChunkHash {
    pub fn of(data: &[u8]) -> Self {
        Self(*blake3::hash(data).as_bytes())
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChunkError {
    #[error("a chunk hash is 64 lowercase hex characters")]
    Hash,
    #[error("chunking parameters out of bounds: {0}")]
    Params(&'static str),
}

fn nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => b.checked_sub(b'0'),
        b'a'..=b'f' => b.checked_sub(b'a').and_then(|v| v.checked_add(10)),
        _ => None,
    }
}

impl FromStr for ChunkHash {
    type Err = ChunkError;

    /// Lowercase only: one hash, one spelling, so a path can never alias another.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        if bytes.len() != 64 {
            return Err(ChunkError::Hash);
        }
        let mut out = [0u8; 32];
        let (pairs, _) = bytes.as_chunks::<2>();
        for (o, [h, l]) in out.iter_mut().zip(pairs) {
            let hi = nibble(*h).ok_or(ChunkError::Hash)?;
            let lo = nibble(*l).ok_or(ChunkError::Hash)?;
            *o = (hi << 4) | lo;
        }
        Ok(Self(out))
    }
}

impl fmt::Display for ChunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

impl serde::Serialize for ChunkHash {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for ChunkHash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = std::borrow::Cow::<str>::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl fmt::Debug for ChunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChunkHash({self})")
    }
}

/// FastCDC parameters, checked against the bounds the algorithm accepts
/// (its own checks are debug assertions only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkParams {
    min: usize,
    avg: usize,
    max: usize,
}

/// RFC 0001 §4.3: no chunk above 4 MiB.
pub const MAX_CHUNK: usize = 4 << 20;

impl ChunkParams {
    pub fn new(min: u64, avg: u64, max: u64) -> Result<Self, ChunkError> {
        use fastcdc::v2020::{AVERAGE_MAX, AVERAGE_MIN, MAXIMUM_MIN, MINIMUM_MAX, MINIMUM_MIN};
        let conv = |v: u64| usize::try_from(v).map_err(|_| ChunkError::Params("too large"));
        let (min, avg, max) = (conv(min)?, conv(avg)?, conv(max)?);
        if !(min < avg && avg < max) {
            return Err(ChunkError::Params("expected min < avg < max"));
        }
        if !(MINIMUM_MIN..=MINIMUM_MAX).contains(&min)
            || !(AVERAGE_MIN..=AVERAGE_MAX).contains(&avg)
        {
            return Err(ChunkError::Params("min 64 B to 1 MiB, avg 256 B to 4 MiB"));
        }
        if !(MAXIMUM_MIN..=MAX_CHUNK).contains(&max) {
            return Err(ChunkError::Params("max 1 KiB to 4 MiB"));
        }
        if min % 2 != 0 || avg % 2 != 0 || max % 2 != 0 {
            return Err(ChunkError::Params("sizes must be even"));
        }
        Ok(Self { min, avg, max })
    }

    pub const fn max(self) -> usize {
        self.max
    }
}

/// A blob's address: BLAKE3 over a domain tag, the file size and the ordered
/// chunk hashes, so the server can check a manifest without the file
/// (plan D39). Each chunk is itself checked against its own hash on upload.
pub fn blob_hash(size: u64, chunks: &[ChunkHash]) -> ChunkHash {
    let mut h = blake3::Hasher::new();
    h.update(b"weebo-kura blob v1");
    h.update(&size.to_be_bytes());
    for c in chunks {
        h.update(c.as_bytes());
    }
    ChunkHash(*h.finalize().as_bytes())
}

/// One chunk of a file: where it is and what it hashes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSpan {
    pub offset: usize,
    pub len: usize,
    pub hash: ChunkHash,
}

/// Cuts `data` into content-defined chunks. An empty file has no chunk.
pub fn chunks(data: &[u8], params: ChunkParams) -> Vec<ChunkSpan> {
    fastcdc::v2020::FastCDC::new(data, params.min, params.avg, params.max)
        .filter_map(|c| {
            let end = c.offset.checked_add(c.length)?;
            Some(ChunkSpan {
                offset: c.offset,
                len: c.length,
                hash: ChunkHash::of(data.get(c.offset..end)?),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc_default() -> ChunkParams {
        ChunkParams::new(256 << 10, 1 << 20, 4 << 20).unwrap()
    }

    /// Deterministic pseudo-random bytes (xorshift), no dependency.
    fn noise(len: usize, mut seed: u64) -> Vec<u8> {
        (0..len)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed & 0xff) as u8
            })
            .collect()
    }

    #[test]
    fn hashes_round_trip_in_lowercase_hex_only() {
        let h = ChunkHash::of(b"hello");
        let s = h.to_string();
        assert_eq!(s.len(), 64);
        assert_eq!(s.parse::<ChunkHash>().unwrap(), h);
        assert!(s.to_uppercase().parse::<ChunkHash>().is_err());
        for bad in [
            "",
            "00",
            &"g".repeat(64),
            &"0".repeat(63),
            &"0".repeat(65),
            &"é".repeat(32),
        ] {
            assert!(bad.parse::<ChunkHash>().is_err(), "{bad}");
        }
    }

    #[test]
    fn reassembled_chunks_equal_the_input() {
        let data = noise(9 << 20, 7);
        let spans = chunks(&data, rfc_default());
        assert!(spans.len() > 2);
        let mut rebuilt = Vec::new();
        let mut expected = 0;
        for s in &spans {
            assert_eq!(s.offset, expected);
            assert!(s.len <= 4 << 20);
            let part = &data[s.offset..s.offset + s.len];
            assert_eq!(ChunkHash::of(part), s.hash);
            rebuilt.extend_from_slice(part);
            expected += s.len;
        }
        assert_eq!(rebuilt, data);
        assert!(chunks(&[], rfc_default()).is_empty());
        assert_eq!(
            chunks(b"a small note", rfc_default()).len(),
            1,
            "most notes are one chunk"
        );
    }

    #[test]
    fn an_edit_in_the_middle_keeps_most_chunks() {
        let data = noise(12 << 20, 3);
        let mut edited = data.clone();
        edited.splice(6 << 20..6 << 20, b"an inserted paragraph".iter().copied());
        let a: std::collections::HashSet<_> = chunks(&data, rfc_default())
            .iter()
            .map(|s| s.hash)
            .collect();
        let b: Vec<_> = chunks(&edited, rfc_default())
            .iter()
            .map(|s| s.hash)
            .collect();
        let shared = b.iter().filter(|h| a.contains(h)).count();
        assert!(
            shared + 2 >= b.len(),
            "only the chunks around the edit change: {shared}/{}",
            b.len()
        );
    }

    #[test]
    fn blob_hashes_bind_size_and_order() {
        let (a, b) = (ChunkHash::of(b"a"), ChunkHash::of(b"b"));
        assert_ne!(blob_hash(2, &[a, b]), blob_hash(2, &[b, a]));
        assert_ne!(blob_hash(2, &[a, b]), blob_hash(3, &[a, b]));
        assert_eq!(blob_hash(0, &[]), blob_hash(0, &[]));
    }

    #[test]
    fn params_outside_the_algorithms_bounds_are_refused() {
        assert!(ChunkParams::new(0, 1 << 20, 4 << 20).is_err());
        assert!(
            ChunkParams::new(256 << 10, 1 << 20, 8 << 20).is_err(),
            "above 4 MiB"
        );
        assert!(ChunkParams::new(1 << 20, 1 << 20, 4 << 20).is_err());
        assert!(
            ChunkParams::new(2 << 20, 3 << 20, 4 << 20).is_err(),
            "min above 1 MiB"
        );
        assert!(ChunkParams::new(101, 1 << 20, 4 << 20).is_err(), "odd");
        assert!(ChunkParams::new(u64::MAX, 1 << 20, 4 << 20).is_err());
        assert_eq!(rfc_default().max(), 4 << 20);
    }
}
