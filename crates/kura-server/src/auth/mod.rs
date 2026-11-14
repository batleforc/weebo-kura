//! Authentication (RFC 0001 §6.2): `verify_access_token` checks signature,
//! issuer, audience and expiry against Authentik's JWKS, with the algorithms
//! pinned in `server.yaml`, and refuses an ID token by requiring `scope`
//! (decision 50). Keys are cached (at most 16), refetched on an unknown key id
//! at most once a minute, and persisted in PostgreSQL.

pub mod keys;
pub mod principal;

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::config::{self, OidcConfig};
pub use keys::{KeySource, Keys};
pub use principal::{BearerToken, Principal};

/// A bearer longer than this is refused before any parsing.
pub const MAX_TOKEN_BYTES: usize = 16 << 10;
/// Userinfo answers cached at most (§6.2).
pub const USERINFO_CACHE: usize = 10_000;
/// Clock skew tolerated on `exp`.
const LEEWAY_SECS: u64 = 30;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid token: {0}")]
    Invalid(&'static str),
    #[error("invalid token: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("identity provider unreachable: {0}")]
    Provider(String),
}

impl From<AuthError> for crate::problem::ApiError {
    fn from(e: AuthError) -> Self {
        match e {
            AuthError::Provider(cause) => Self::Unavailable(cause),
            AuthError::Invalid(_) | AuthError::Jwt(_) => {
                tracing::warn!(error = %e, "token refused");
                Self::of(crate::problem::ProblemType::InvalidToken)
            }
        }
    }
}

fn algorithm(a: config::Algorithm) -> Algorithm {
    match a {
        config::Algorithm::RS256 => Algorithm::RS256,
        config::Algorithm::RS384 => Algorithm::RS384,
        config::Algorithm::RS512 => Algorithm::RS512,
        config::Algorithm::PS256 => Algorithm::PS256,
        config::Algorithm::PS384 => Algorithm::PS384,
        config::Algorithm::PS512 => Algorithm::PS512,
        config::Algorithm::ES256 => Algorithm::ES256,
        config::Algorithm::ES384 => Algorithm::ES384,
        config::Algorithm::EdDSA => Algorithm::EdDSA,
    }
}

/// sha256(token) -> (claims from userinfo, token expiry).
type UserinfoCache = HashMap<[u8; 32], (Map<String, Value>, u64)>;

pub struct Verifier {
    issuer: String,
    audiences: Vec<String>,
    algorithms: Vec<Algorithm>,
    pub platform_admin: config::ClaimMatch,
    pub keys: Keys,
    /// sha256(token) -> (claims from userinfo, token expiry).
    userinfo: Mutex<UserinfoCache>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Verifier {
    pub fn new(oidc: &OidcConfig, keys: Keys) -> Self {
        Self {
            issuer: oidc.issuer.clone(),
            audiences: oidc.audiences.clone(),
            algorithms: oidc.algorithms.iter().copied().map(algorithm).collect(),
            platform_admin: oidc.platform_admin.clone(),
            keys,
            userinfo: Mutex::new(HashMap::new()),
        }
    }

    /// The access token's claims, or a refusal. Never trusts the token header
    /// for the algorithm: it must be one of the pinned ones.
    pub async fn verify_access_token(&self, token: &str) -> Result<Principal, AuthError> {
        if token.len() > MAX_TOKEN_BYTES {
            return Err(AuthError::Invalid("too long"));
        }
        // `alg: none` has no `Algorithm` variant: decode_header refuses it.
        let header = decode_header(token)?;
        if !self.algorithms.contains(&header.alg) {
            return Err(AuthError::Invalid("algorithm not allowed"));
        }
        let kid = header.kid.ok_or(AuthError::Invalid("no key id"))?;
        let key = self
            .keys
            .get(&kid)
            .await?
            .ok_or(AuthError::Invalid("unknown key id"))?;
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&self.audiences);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = LEEWAY_SECS;
        let claims = decode::<Map<String, Value>>(token, &key, &validation)?.claims;
        // Only access tokens carry `scope` in Authentik (decision 50).
        if !claims.get("scope").is_some_and(Value::is_string) {
            return Err(AuthError::Invalid("no scope claim: not an access token"));
        }
        Principal::from_claims(claims).ok_or(AuthError::Invalid("no subject"))
    }

    /// Fills the claims the configuration reads (`needed`) from userinfo when
    /// the access token lacks them (`include_claims_in_id_token` off, §6.2).
    pub async fn with_claims(
        &self,
        mut principal: Principal,
        token: &BearerToken,
        needed: &[&str],
    ) -> Result<Principal, AuthError> {
        if needed.iter().all(|c| principal.claims.contains_key(*c)) {
            return Ok(principal);
        }
        let digest: [u8; 32] = Sha256::digest(token.expose().as_bytes()).into();
        let cached = self
            .userinfo
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&digest)
            .cloned();
        let info = match cached {
            Some((info, _)) => info,
            None => {
                let info = self.keys.userinfo(token.expose()).await?;
                self.cache_userinfo(digest, info.clone(), principal.exp);
                info
            }
        };
        for claim in needed {
            if let Some(v) = info.get(*claim) {
                principal
                    .claims
                    .entry((*claim).to_owned())
                    .or_insert_with(|| v.clone());
            }
        }
        Ok(principal)
    }

    fn cache_userinfo(&self, digest: [u8; 32], info: Map<String, Value>, exp: u64) {
        let mut cache = self.userinfo.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= USERINFO_CACHE {
            let t = now();
            cache.retain(|_, (_, e)| *e > t);
        }
        // Still full of live entries: this answer is simply not cached.
        if cache.len() < USERINFO_CACHE {
            cache.insert(digest, (info, exp));
        }
    }

    pub fn userinfo_cached(&self) -> usize {
        self.userinfo
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether the principal is a platform admin (`oidc.platformAdmin`).
    pub fn is_platform_admin(&self, p: &Principal) -> bool {
        p.claim_values(&self.platform_admin.claim)
            .iter()
            .any(|v| *v == self.platform_admin.value)
    }
}

/// Bound on how long a refetch may wait for Authentik (§6.11).
pub const PROVIDER_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(test)]
pub(crate) mod tests;
