//! The scoped token's text form (RFC 0001 §6.2): `kura_<id>_<secret>`, the
//! id a UUID in 32 lowercase hex digits, the secret 256 random bits in 64.
//! Parsed once, here, by the server and the CLI alike; the server keeps
//! only the secret's SHA-256.

use std::fmt;

use uuid::Uuid;

pub const PREFIX: &str = "kura_";
/// Bytes of a secret: 256 bits (§6.2).
pub const SECRET_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("not a kura scoped token")]
    Malformed,
}

/// A parsed token. `Debug` never shows the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct ScopedToken {
    pub id: Uuid,
    secret: [u8; SECRET_BYTES],
}

impl fmt::Debug for ScopedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScopedToken({}, <redacted>)", self.id)
    }
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b.wrapping_sub(b'0')),
        b'a'..=b'f' => Some(b.wrapping_sub(b'a').wrapping_add(10)),
        _ => None,
    }
}

impl ScopedToken {
    pub fn new(id: Uuid, secret: [u8; SECRET_BYTES]) -> Self {
        Self { id, secret }
    }

    /// Strict: the prefix, 32 then 64 lowercase hex digits, one `_` between.
    pub fn parse(s: &str) -> Result<Self, TokenError> {
        let rest = s.strip_prefix(PREFIX).ok_or(TokenError::Malformed)?;
        let (id, secret) = rest.split_once('_').ok_or(TokenError::Malformed)?;
        if id.len() != 32 || secret.len() != 2 * SECRET_BYTES {
            return Err(TokenError::Malformed);
        }
        if !id.bytes().all(|b| hex_digit(b).is_some()) {
            return Err(TokenError::Malformed);
        }
        let id = Uuid::try_parse(id).map_err(|_| TokenError::Malformed)?;
        let mut out = [0u8; SECRET_BYTES];
        let (pairs, _) = secret.as_bytes().as_chunks::<2>();
        for (o, [hi, lo]) in out.iter_mut().zip(pairs) {
            let (hi, lo) = (
                hex_digit(*hi).ok_or(TokenError::Malformed)?,
                hex_digit(*lo).ok_or(TokenError::Malformed)?,
            );
            *o = hi.wrapping_mul(16).wrapping_add(lo);
        }
        Ok(Self { id, secret: out })
    }

    pub fn secret(&self) -> &[u8; SECRET_BYTES] {
        &self.secret
    }

    /// The text to hand over once; never logged.
    pub fn expose(&self) -> String {
        let secret: String = self.secret.iter().map(|b| format!("{b:02x}")).collect();
        format!("{PREFIX}{}_{secret}", self.id.simple())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_round_trip_and_parse_strictly() {
        let t = ScopedToken::new(Uuid::from_u128(0x0193_a9f0), [0xab; SECRET_BYTES]);
        let text = t.expose();
        assert_eq!(text.len(), 5 + 32 + 1 + 64);
        assert_eq!(ScopedToken::parse(&text), Ok(t.clone()));
        assert!(!format!("{t:?}").contains("abab"), "Debug hides the secret");
        for bad in [
            String::new(),
            "kura_".into(),
            text.replacen("kura_", "kurb_", 1),
            text.to_uppercase(),
            format!("{text}0"),
            text.replacen('_', "-", 2),
            text.replace(&t.id.simple().to_string(), &t.id.hyphenated().to_string()),
            format!("{}g", &text[..text.len() - 1]),
            format!("{}é", &text[..text.len() - 2]),
        ] {
            assert_eq!(
                ScopedToken::parse(&bad),
                Err(TokenError::Malformed),
                "{bad}"
            );
        }
    }
}
