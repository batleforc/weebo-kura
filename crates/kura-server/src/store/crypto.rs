//! The stored chunk (RFC 0001 §6.4, §6.6): a format byte, a codec byte, a
//! random 96-bit nonce and the AES-256-GCM ciphertext of the plaintext, under
//! the vault's data key. The vault id, the chunk hash and both bytes are the
//! associated data, so a chunk cannot be replayed under another address,
//! vault or format. Object keys are keyed hashes: holding the bucket alone
//! does not tell whether a known file is stored.

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use aws_lc_rs::rand::{SecureRandom, SystemRandom};
use kura_core::ChunkHash;
use uuid::Uuid;
use zeroize::Zeroizing;

/// v1 of the stored chunk layout.
pub const FORMAT_V1: u8 = 1;
/// Codec 0: no compression (decision 38).
pub const CODEC_NONE: u8 = 0;
const HEADER: usize = 2;
const TAG: usize = 16;
/// Context of the object-key derivation; never reused for anything else.
const OBJECT_KEY_CONTEXT: &str = "weebo-kura 2026-11-14 object key v1";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("a data key is 32 bytes")]
    KeyLength,
    #[error("stored chunk is malformed")]
    Malformed,
    #[error("stored chunk format {0} or codec {1} is not readable by this release")]
    Unsupported(u8, u8),
    #[error("stored chunk failed authentication")]
    Corrupt,
    #[error("no randomness available")]
    Random,
}

/// A vault's plaintext data key. No `Debug` content, no `Display`, wiped on
/// drop (§6.11).
pub struct DataKey(Zeroizing<[u8; 32]>);

impl DataKey {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let array: [u8; 32] = bytes.try_into().map_err(|_| CryptoError::KeyLength)?;
        Ok(Self(Zeroizing::new(array)))
    }

    fn aead(&self) -> Result<LessSafeKey, CryptoError> {
        UnboundKey::new(&AES_256_GCM, self.0.as_slice())
            .map(LessSafeKey::new)
            .map_err(|_| CryptoError::KeyLength)
    }

    /// The bucket name of one upload of a chunk: a keyed hash of its address
    /// under a key derived from the data key, then a random suffix, so two
    /// uploads of one chunk never share an object (§6.6).
    pub fn object_key(&self, vault: Uuid, hash: &ChunkHash) -> Result<String, CryptoError> {
        let derived = Zeroizing::new(blake3::derive_key(OBJECT_KEY_CONTEXT, self.0.as_slice()));
        let tag = blake3::keyed_hash(&derived, hash.as_bytes());
        let mut suffix = [0u8; 8];
        SystemRandom::new()
            .fill(&mut suffix)
            .map_err(|_| CryptoError::Random)?;
        let suffix: String = suffix.iter().map(|b| format!("{b:02x}")).collect();
        Ok(format!("{vault}/{}-{suffix}", tag.to_hex()))
    }
}

impl std::fmt::Debug for DataKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataKey(<redacted>)")
    }
}

fn aad(vault: Uuid, hash: &ChunkHash, format: u8, codec: u8) -> [u8; 50] {
    let mut out = [0u8; 50];
    let (v, rest) = out.split_at_mut(16);
    v.copy_from_slice(vault.as_bytes());
    let (h, rest) = rest.split_at_mut(32);
    h.copy_from_slice(hash.as_bytes());
    if let [f, c] = rest {
        *f = format;
        *c = codec;
    }
    out
}

/// Encrypts a chunk's plaintext for storage, in one buffer.
pub fn seal(
    key: &DataKey,
    vault: Uuid,
    hash: &ChunkHash,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| CryptoError::Random)?;
    let start = HEADER.saturating_add(NONCE_LEN);
    let mut out = Vec::with_capacity(start.saturating_add(plaintext.len()).saturating_add(TAG));
    out.extend_from_slice(&[FORMAT_V1, CODEC_NONE]);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(plaintext);
    let body = out.get_mut(start..).ok_or(CryptoError::Malformed)?;
    let tag = key
        .aead()?
        .seal_in_place_separate_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(aad(vault, hash, FORMAT_V1, CODEC_NONE)),
            body,
        )
        .map_err(|_| CryptoError::Corrupt)?;
    out.extend_from_slice(tag.as_ref());
    Ok(out)
}

/// Decrypts a stored chunk and checks it is the chunk of this address.
pub fn open(
    key: &DataKey,
    vault: Uuid,
    hash: &ChunkHash,
    stored: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let (header, rest) = stored
        .split_at_checked(HEADER)
        .ok_or(CryptoError::Malformed)?;
    let (format, codec) = match header {
        [f, c] => (*f, *c),
        _ => return Err(CryptoError::Malformed),
    };
    if format != FORMAT_V1 || codec != CODEC_NONE {
        return Err(CryptoError::Unsupported(format, codec));
    }
    let (nonce, body) = rest
        .split_at_checked(NONCE_LEN)
        .ok_or(CryptoError::Malformed)?;
    if body.len() < TAG {
        return Err(CryptoError::Malformed);
    }
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| CryptoError::Malformed)?;
    let mut buf = body.to_vec();
    let len = key
        .aead()?
        .open_in_place(nonce, Aad::from(aad(vault, hash, format, codec)), &mut buf)
        .map_err(|_| CryptoError::Corrupt)?
        .len();
    buf.truncate(len);
    // AES-GCM authenticates the bytes; the address must also be their hash.
    if ChunkHash::of(&buf) != *hash {
        return Err(CryptoError::Corrupt);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(b: u8) -> DataKey {
        DataKey::from_bytes(&[b; 32]).unwrap()
    }

    #[test]
    fn seals_and_opens_and_hides_the_plaintext() {
        let (k, v) = (key(1), Uuid::from_u128(7));
        let data = b"# A note\nwith a secret line".repeat(20);
        let h = ChunkHash::of(&data);
        let stored = seal(&k, v, &h, &data).unwrap();
        assert_eq!(stored.get(..2), Some(&[FORMAT_V1, CODEC_NONE][..]));
        assert!(
            !stored.windows(13).any(|w| w == b"a secret line"),
            "T8: no plaintext at rest"
        );
        assert_eq!(open(&k, v, &h, &stored).unwrap(), data);
        assert_ne!(seal(&k, v, &h, &data).unwrap(), stored, "random nonce");
    }

    #[test]
    fn a_chunk_opens_only_at_its_own_address_vault_and_key() {
        let (k, v) = (key(1), Uuid::from_u128(7));
        let data = b"content".to_vec();
        let h = ChunkHash::of(&data);
        let stored = seal(&k, v, &h, &data).unwrap();
        assert_eq!(open(&key(2), v, &h, &stored), Err(CryptoError::Corrupt));
        assert_eq!(
            open(&k, Uuid::from_u128(8), &h, &stored),
            Err(CryptoError::Corrupt)
        );
        assert_eq!(
            open(&k, v, &ChunkHash::of(b"other"), &stored),
            Err(CryptoError::Corrupt)
        );
        let mut flipped = stored.clone();
        if let Some(b) = flipped.last_mut() {
            *b ^= 1;
        }
        assert_eq!(open(&k, v, &h, &flipped), Err(CryptoError::Corrupt));
        let mut codec = stored.clone();
        codec[1] = 1;
        assert_eq!(open(&k, v, &h, &codec), Err(CryptoError::Unsupported(1, 1)));
        assert_eq!(open(&k, v, &h, &stored[..20]), Err(CryptoError::Malformed));
        assert_eq!(open(&k, v, &h, &[]), Err(CryptoError::Malformed));
        assert!(DataKey::from_bytes(&[0; 31]).is_err());
        assert_eq!(format!("{:?}", key(9)), "DataKey(<redacted>)");
    }

    #[test]
    fn object_keys_are_keyed_and_unique_per_upload() {
        let (k, v, h) = (key(1), Uuid::from_u128(7), ChunkHash::of(b"x"));
        let (a, b) = (k.object_key(v, &h).unwrap(), k.object_key(v, &h).unwrap());
        assert_ne!(a, b, "a random suffix per upload");
        let prefix = |s: &str| s.rsplit_once('-').map(|(p, _)| p.to_owned()).unwrap();
        assert_eq!(prefix(&a), prefix(&b));
        assert!(!a.contains(&h.to_string()), "the address does not appear");
        assert_ne!(
            prefix(&a),
            prefix(&key(2).object_key(v, &h).unwrap()),
            "keyed by the vault's key"
        );
    }
}
