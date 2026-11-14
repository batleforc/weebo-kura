//! Authentik's signing keys: fetched through OIDC discovery, cached (at most
//! 16), refetched on an unknown key id at most once a minute, persisted in
//! PostgreSQL so a replica started during an Authentik outage still verifies.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::Instant;

use jsonwebtoken::DecodingKey;
use jsonwebtoken::jwk::JwkSet;
use serde::Deserialize;
use serde_json::{Map, Value};
use sqlx::PgPool;

use super::{AuthError, PROVIDER_TIMEOUT};

pub const MAX_KEYS: usize = 16;
pub const REFETCH_INTERVAL: Duration = Duration::from_secs(60);
/// Age past which the cached set is refreshed on the next verification.
pub const MAX_AGE: Duration = Duration::from_secs(600);

/// The two endpoints read from the provider's discovery document.
#[derive(Debug, Clone, Deserialize)]
pub struct Discovery {
    #[serde(default)]
    issuer: Option<String>,
    jwks_uri: String,
    userinfo_endpoint: String,
}

/// An endpoint the server may send tokens to: https, or http on loopback
/// for local tests (review 4 L5).
fn safe_endpoint(url: &str) -> bool {
    url.parse::<axum::http::Uri>()
        .is_ok_and(|u| match (u.scheme_str(), u.host()) {
            (Some("https"), Some(_)) => true,
            (Some("http"), Some(h)) => matches!(h, "127.0.0.1" | "localhost" | "[::1]"),
            _ => false,
        })
}

/// Where keys and userinfo come from.
pub enum KeySource {
    /// Authentik, through `<issuer>/.well-known/openid-configuration`.
    Http {
        client: reqwest::Client,
        issuer: String,
        /// The `iss` the discovery document must name (`oidc.issuer`).
        expected: Option<String>,
        discovery: tokio::sync::OnceCell<Discovery>,
    },
    /// A fixed key set and userinfo answer, for tests.
    Fixed {
        jwks: std::sync::Mutex<JwkSet>,
        userinfo: Map<String, Value>,
    },
}

impl KeySource {
    pub fn http(issuer: &str) -> Result<Self, AuthError> {
        let client = reqwest::Client::builder()
            .timeout(PROVIDER_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AuthError::Provider(e.to_string()))?;
        Ok(Self::Http {
            client,
            issuer: issuer.to_owned(),
            expected: None,
            discovery: tokio::sync::OnceCell::new(),
        })
    }

    /// Refuses a discovery document that names another issuer.
    #[must_use]
    pub fn expecting(mut self, issuer: &str) -> Self {
        if let Self::Http { expected, .. } = &mut self {
            *expected = Some(issuer.to_owned());
        }
        self
    }
}

struct Cache {
    keys: HashMap<String, DecodingKey>,
    last_fetch: Option<Instant>,
}

pub struct Keys {
    source: KeySource,
    pool: Option<PgPool>,
    cache: tokio::sync::Mutex<Cache>,
    fetches: AtomicUsize,
}

fn provider<E: std::fmt::Display>(e: E) -> AuthError {
    AuthError::Provider(e.to_string())
}

/// Parses at most [`MAX_KEYS`] keys; one that does not parse is skipped.
fn decode_set(set: &JwkSet) -> HashMap<String, DecodingKey> {
    set.keys
        .iter()
        .filter_map(|jwk| Some((jwk.common.key_id.clone()?, DecodingKey::from_jwk(jwk).ok()?)))
        .take(MAX_KEYS)
        .collect()
}

impl Keys {
    pub fn new(source: KeySource, pool: Option<PgPool>) -> Self {
        Self {
            source,
            pool,
            cache: tokio::sync::Mutex::new(Cache {
                keys: HashMap::new(),
                last_fetch: None,
            }),
            fetches: AtomicUsize::new(0),
        }
    }

    /// Loads the keys persisted by any replica.
    pub async fn load_persisted(&self) -> Result<usize, sqlx::Error> {
        let Some(pool) = &self.pool else { return Ok(0) };
        let rows: Vec<Value> = sqlx::query_scalar("SELECT jwk FROM kura_jwks_load()")
            .fetch_all(pool)
            .await?;
        let set = JwkSet {
            keys: rows
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect(),
        };
        let keys = decode_set(&set);
        let n = keys.len();
        self.cache.lock().await.keys.extend(keys);
        Ok(n)
    }

    pub fn source(&self) -> &KeySource {
        &self.source
    }

    /// Number of times the provider was asked for keys.
    pub fn fetches(&self) -> usize {
        self.fetches.load(Ordering::Relaxed)
    }

    pub async fn get(&self, kid: &str) -> Result<Option<DecodingKey>, AuthError> {
        let mut cache = self.cache.lock().await;
        // A known key is served until the set is MAX_AGE old: then the next
        // call refetches, so a key Authentik retired stops verifying.
        let fresh = cache.last_fetch.is_some_and(|t| t.elapsed() < MAX_AGE);
        if let Some(k) = cache.keys.get(kid).filter(|_| fresh) {
            return Ok(Some(k.clone()));
        }
        if cache
            .last_fetch
            .is_some_and(|t| t.elapsed() < REFETCH_INTERVAL)
        {
            return Ok(None);
        }
        cache.last_fetch = Some(Instant::now());
        match self.fetch().await {
            Ok((set, raw)) => {
                cache.keys = decode_set(&set);
                if let Some(pool) = &self.pool {
                    // Best effort: verification works from memory either way.
                    if let Err(e) = sqlx::query("SELECT kura_jwks_replace($1)")
                        .bind(raw)
                        .execute(pool)
                        .await
                    {
                        tracing::warn!(error = %e, "JWKS not persisted");
                    }
                }
            }
            // Authentik unreachable: keys already cached keep working; an unknown key id is refused (§10.2).
            Err(e) => tracing::warn!(error = %e, "JWKS refetch failed"),
        }
        Ok(cache.keys.get(kid).cloned())
    }

    async fn discovery(&self) -> Result<Option<&Discovery>, AuthError> {
        let KeySource::Http {
            client,
            issuer,
            expected,
            discovery,
        } = &self.source
        else {
            return Ok(None);
        };
        let url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        discovery
            .get_or_try_init(|| async {
                client
                    .get(&url)
                    .send()
                    .await
                    .map_err(provider)?
                    .error_for_status()
                    .map_err(provider)?
                    .json::<Discovery>()
                    .await
                    .map_err(provider)
                    .and_then(|d| {
                        let same =
                            |a: &str, b: &str| a.trim_end_matches('/') == b.trim_end_matches('/');
                        if let Some(want) = expected
                            && !d.issuer.as_deref().is_some_and(|got| same(got, want))
                        {
                            return Err(AuthError::Provider(
                                "discovery names another issuer".into(),
                            ));
                        }
                        if !safe_endpoint(&d.jwks_uri) || !safe_endpoint(&d.userinfo_endpoint) {
                            return Err(AuthError::Provider(
                                "discovery endpoints must be https".into(),
                            ));
                        }
                        Ok(d)
                    })
            })
            .await
            .map(Some)
    }

    async fn fetch(&self) -> Result<(JwkSet, Value), AuthError> {
        self.fetches.fetch_add(1, Ordering::Relaxed);
        let raw: Value = match &self.source {
            KeySource::Fixed { jwks, .. } => serde_json::to_value(
                &*jwks
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )
            .map_err(provider)?,
            KeySource::Http { client, .. } => {
                let Some(d) = self.discovery().await? else {
                    return Err(AuthError::Invalid("no discovery"));
                };
                client
                    .get(&d.jwks_uri)
                    .send()
                    .await
                    .map_err(provider)?
                    .error_for_status()
                    .map_err(provider)?
                    .json()
                    .await
                    .map_err(provider)?
            }
        };
        let set: JwkSet = serde_json::from_value(raw.clone()).map_err(provider)?;
        let keys = raw.get("keys").cloned().unwrap_or(Value::Array(Vec::new()));
        Ok((set, keys))
    }

    /// The userinfo answer for this token.
    pub async fn userinfo(&self, token: &str) -> Result<Map<String, Value>, AuthError> {
        match &self.source {
            KeySource::Fixed { userinfo, .. } => Ok(userinfo.clone()),
            KeySource::Http { client, .. } => {
                let Some(d) = self.discovery().await? else {
                    return Err(AuthError::Invalid("no discovery"));
                };
                client
                    .get(&d.userinfo_endpoint)
                    .bearer_auth(token)
                    .send()
                    .await
                    .map_err(provider)?
                    .error_for_status()
                    .map_err(provider)?
                    .json()
                    .await
                    .map_err(provider)
            }
        }
    }
}
