//! The verified caller, and the bearer token as a secret.

use serde_json::{Map, Value};

/// The bearer token. No `Display`, a redacted `Debug` (§6.11).
pub struct BearerToken(String);

impl BearerToken {
    pub fn new(token: String) -> Self {
        Self(token)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for BearerToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BearerToken(<redacted>)")
    }
}

#[derive(Debug, Clone)]
pub struct Principal {
    /// OIDC `sub`.
    pub sub: String,
    /// Authentik session id, bound to devices (decision 35).
    pub sid: Option<String>,
    /// `exp`, seconds since the epoch.
    pub exp: u64,
    pub claims: Map<String, Value>,
}

impl Principal {
    pub fn from_claims(claims: Map<String, Value>) -> Option<Self> {
        let sub = claims
            .get("sub")?
            .as_str()
            .filter(|s| !s.is_empty())?
            .to_owned();
        let sid = claims.get("sid").and_then(Value::as_str).map(str::to_owned);
        let exp = claims.get("exp").and_then(Value::as_u64).unwrap_or(0);
        Some(Self {
            sub,
            sid,
            exp,
            claims,
        })
    }

    /// A claim as a list of strings: `"g"` and `["g", "h"]` both work.
    pub fn claim_values(&self, claim: &str) -> Vec<&str> {
        match self.claims.get(claim) {
            Some(Value::String(s)) => vec![s.as_str()],
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        }
    }
}
