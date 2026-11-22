//! kura-operator's identity (RFC 0009 §6.2, §7.2): a projected ServiceAccount
//! token verified against the cluster's issuer, `RS256` only, `iss` and `aud`
//! exact, `sub` equal to the one configured. Its own keys and its own caller
//! type: a user token never reaches the documents route, and this token never
//! reaches a user route (T1).

use jsonwebtoken::{Algorithm, Validation, decode, decode_header};
use serde::Deserialize;

use super::{AuthError, Keys, MAX_TOKEN_BYTES};
use crate::config::OperatorIdentity;

/// Clock skew tolerated on `exp`.
const LEEWAY_SECS: u64 = 30;

/// The verified operator: only its subject, for the audit line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorCaller {
    pub subject: String,
}

/// Why a token is not the operator's.
#[derive(Debug, thiserror::Error)]
pub enum OperatorError {
    /// Not a valid token of the issuer for this audience: 401.
    #[error(transparent)]
    Token(#[from] AuthError),
    /// A valid token of another workload: 403.
    #[error("token of another subject")]
    OtherSubject,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
}

pub struct OperatorVerifier {
    issuer: String,
    audience: String,
    subject: String,
    pub keys: Keys,
}

impl OperatorVerifier {
    /// `keys` comes from the cluster issuer (`KeySource::cluster`), never
    /// persisted beside Authentik's.
    pub fn new(identity: &OperatorIdentity, keys: Keys) -> Self {
        Self {
            issuer: identity.issuer.clone(),
            audience: identity.audience.clone(),
            subject: identity.subject.clone(),
            keys,
        }
    }

    pub async fn verify(&self, token: &str) -> Result<OperatorCaller, OperatorError> {
        if token.len() > MAX_TOKEN_BYTES {
            return Err(AuthError::Invalid("too long").into());
        }
        let header = decode_header(token).map_err(AuthError::from)?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthError::Invalid("algorithm not allowed").into());
        }
        let kid = header.kid.ok_or(AuthError::Invalid("no key id"))?;
        let key = self
            .keys
            .get(&kid)
            .await?
            .ok_or(AuthError::Invalid("unknown key id"))?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = LEEWAY_SECS;
        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(AuthError::from)?
            .claims;
        // Byte for byte: no prefix, no case folding, no wildcard.
        if claims.sub != self.subject {
            return Err(OperatorError::OtherSubject);
        }
        Ok(OperatorCaller {
            subject: claims.sub,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;
    use crate::auth::tests::RsaTestKey as TestKey;
    use crate::auth::{KeySource, Keys};

    const ISSUER: &str = "https://kubernetes.default.svc.cluster.local";
    const SUBJECT: &str = "system:serviceaccount:kura-operator:kura-operator";

    fn verifier(key: &TestKey) -> OperatorVerifier {
        let identity = OperatorIdentity {
            issuer: ISSUER.into(),
            audience: "kura".into(),
            subject: SUBJECT.into(),
            ca_file: None,
            token_file: None,
        };
        let source = KeySource::Fixed {
            jwks: std::sync::Mutex::new(key.jwks()),
            userinfo: serde_json::Map::new(),
        };
        OperatorVerifier::new(&identity, Keys::new(source, None))
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn claims(sub: &str, aud: &str, exp: u64) -> serde_json::Value {
        json!({ "iss": ISSUER, "aud": [aud], "sub": sub, "exp": exp, "iat": now() })
    }

    #[tokio::test]
    async fn only_the_configured_service_account_passes() {
        let key = TestKey::new("k1");
        let v = verifier(&key);
        let ok = key.sign(&claims(SUBJECT, "kura", now() + 600));
        assert_eq!(v.verify(&ok).await.unwrap().subject, SUBJECT);

        let other = key.sign(&claims(
            "system:serviceaccount:default:default",
            "kura",
            now() + 600,
        ));
        assert!(matches!(
            v.verify(&other).await,
            Err(OperatorError::OtherSubject)
        ));
        // A prefix of the subject is not the subject.
        let prefix = key.sign(&claims(&format!("{SUBJECT}x"), "kura", now() + 600));
        assert!(matches!(
            v.verify(&prefix).await,
            Err(OperatorError::OtherSubject)
        ));
    }

    #[tokio::test]
    async fn audience_issuer_expiry_and_algorithm_are_checked() {
        let key = TestKey::new("k1");
        let v = verifier(&key);
        let refused = |t: String| {
            let v = &v;
            async move { matches!(v.verify(&t).await, Err(OperatorError::Token(_))) }
        };
        assert!(refused(key.sign(&claims(SUBJECT, "kura-cli", now() + 600))).await);
        assert!(refused(key.sign(&claims(SUBJECT, "kura", now() - 120))).await);
        let mut other_issuer = claims(SUBJECT, "kura", now() + 600);
        other_issuer["iss"] = json!("https://auth.example.org/application/o/kura/");
        assert!(refused(key.sign(&other_issuer)).await);
        let no_sub = json!({ "iss": ISSUER, "aud": ["kura"], "exp": now() + 600 });
        assert!(refused(key.sign(&no_sub)).await);
        // HS256 signed with a guessable secret, and `alg: none`.
        let hs = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(Algorithm::HS256),
            &claims(SUBJECT, "kura", now() + 600),
            &jsonwebtoken::EncodingKey::from_secret(b"kura").unwrap(),
        )
        .unwrap();
        assert!(refused(hs).await);
        assert!(refused("eyJhbGciOiJub25lIn0.e30.".into()).await);
        assert!(refused("x".repeat(MAX_TOKEN_BYTES + 1)).await);
    }

    /// A stand-in for the cluster's issuer: discovery and keys for an
    /// authenticated ServiceAccount only, no userinfo endpoint. T9: a key set
    /// behind a redirect is not followed.
    #[tokio::test]
    async fn keys_come_from_the_cluster_issuer_with_a_bearer_and_no_redirect() {
        use axum::http::{HeaderMap, StatusCode};
        use axum::response::IntoResponse;
        use axum::routing::get;
        let key = TestKey::new("k1");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let jwks = json!({ "keys": [key.jwk.clone()] });
        let authed = |h: &HeaderMap| {
            h.get("authorization")
                .is_some_and(|v| v == "Bearer kura-server-token")
        };
        let discovery = {
            let base = base.clone();
            move |h: HeaderMap| async move {
                if !authed(&h) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                axum::Json(json!({ "issuer": base, "jwks_uri": format!("{base}/openid/v1/jwks") }))
                    .into_response()
            }
        };
        let app = axum::Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route(
                "/openid/v1/jwks",
                get(move |h: HeaderMap| async move {
                    if !authed(&h) {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    axum::Json(jwks).into_response()
                }),
            )
            .route(
                "/moved/.well-known/openid-configuration",
                get(|| async {
                    axum::response::Redirect::temporary("/.well-known/openid-configuration")
                }),
            );
        let server = tokio::spawn(async move { axum::serve(listener, app).await });

        let dir = std::env::temp_dir().join(format!("kura-operator-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let token_file = dir.join("token");
        std::fs::write(&token_file, "kura-server-token\n").unwrap();
        let identity = |issuer: &str| OperatorIdentity {
            issuer: issuer.into(),
            audience: "kura".into(),
            subject: SUBJECT.into(),
            ca_file: None,
            token_file: Some(token_file.clone()),
        };
        let with_bearer = |issuer: &str| {
            let source = KeySource::cluster(issuer, &[], Some(token_file.clone())).unwrap();
            OperatorVerifier::new(&identity(issuer), Keys::new(source, None))
        };
        let mut c = claims(SUBJECT, "kura", now() + 600);
        c["iss"] = json!(base);
        let token = key.sign(&c);
        assert_eq!(
            with_bearer(&base).verify(&token).await.unwrap().subject,
            SUBJECT
        );

        // Without kura-server's own token the issuer answers 401: no key.
        let anonymous = OperatorVerifier::new(
            &identity(&base),
            Keys::new(KeySource::http(&base).unwrap(), None),
        );
        assert!(matches!(
            anonymous.verify(&token).await,
            Err(OperatorError::Token(_))
        ));

        // A redirect is not followed.
        let moved = format!("{base}/moved");
        let mut c = claims(SUBJECT, "kura", now() + 600);
        c["iss"] = json!(moved);
        assert!(matches!(
            with_bearer(&moved).verify(&key.sign(&c)).await,
            Err(OperatorError::Token(_))
        ));
        server.abort();
    }
}
