//! T1: forged or foreign tokens are refused (RFC 0001 §7.1).

use std::sync::Mutex;

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::{Value, json};

use super::*;

pub const ISSUER: &str = "https://auth.example.org/application/o/kura/";

/// A signing key generated for this run, and its public JWK.
pub struct TestKey {
    pub kid: String,
    encoding: EncodingKey,
    pub jwk: Value,
}

impl TestKey {
    pub fn new(kid: &str) -> Self {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let x = URL_SAFE_NO_PAD.encode(pair.public_key().as_ref());
        Self {
            kid: kid.into(),
            encoding: EncodingKey::from_ed_der(pkcs8.as_ref()),
            jwk: json!({ "kty": "OKP", "crv": "Ed25519", "x": x, "kid": kid, "alg": "EdDSA", "use": "sig" }),
        }
    }

    pub fn sign(&self, claims: &Value) -> String {
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.kid.clone());
        encode(&header, claims, &self.encoding).unwrap()
    }
}

pub fn access_claims(sub: &str) -> Value {
    json!({
        "iss": ISSUER, "aud": "kura-cli", "sub": sub, "sid": "session-1",
        "exp": now().saturating_add(3600), "scope": "openid profile offline_access",
        "groups": ["kura-maxime"],
    })
}

pub fn oidc() -> OidcConfig {
    OidcConfig {
        issuer: ISSUER.into(),
        audiences: vec!["kura-plugin".into(), "kura-cli".into()],
        algorithms: vec![config::Algorithm::EdDSA],
        platform_admin: config::ClaimMatch {
            claim: "groups".into(),
            value: "kura-platform-admins".into(),
        },
    }
}

pub fn verifier(keys: &[&TestKey], userinfo: Value) -> Verifier {
    let set: JwkSet = serde_json::from_value(
        json!({ "keys": keys.iter().map(|k| k.jwk.clone()).collect::<Vec<_>>() }),
    )
    .unwrap();
    let Value::Object(userinfo) = userinfo else {
        panic!()
    };
    Verifier::new(
        &oidc(),
        Keys::new(
            KeySource::Fixed {
                jwks: Mutex::new(set),
                userinfo,
            },
            None,
        ),
    )
}

async fn refused(v: &Verifier, token: &str) -> String {
    v.verify_access_token(token).await.unwrap_err().to_string()
}

#[tokio::test]
async fn accepts_a_valid_access_token() {
    let k = TestKey::new("k1");
    let v = verifier(&[&k], json!({}));
    let p = v
        .verify_access_token(&k.sign(&access_claims("u1")))
        .await
        .unwrap();
    assert_eq!(p.sub, "u1");
    assert_eq!(p.sid.as_deref(), Some("session-1"));
    assert_eq!(p.claim_values("groups"), ["kura-maxime"]);
    assert!(!v.is_platform_admin(&p));
}

#[tokio::test]
async fn rejects_token_for_other_audience() {
    let k = TestKey::new("k1");
    let v = verifier(&[&k], json!({}));
    let mut c = access_claims("u1");
    c["aud"] = json!("another-app");
    assert!(refused(&v, &k.sign(&c)).await.contains("InvalidAudience"));
    let mut c = access_claims("u1");
    c["iss"] = json!("https://evil.example.org/");
    assert!(refused(&v, &k.sign(&c)).await.contains("InvalidIssuer"));
}

#[tokio::test]
async fn rejects_alg_none() {
    let k = TestKey::new("k1");
    let v = verifier(&[&k], json!({}));
    let enc = |v: &Value| URL_SAFE_NO_PAD.encode(serde_json::to_vec(v).unwrap());
    let token = format!(
        "{}.{}.",
        enc(&json!({"alg": "none", "kid": "k1"})),
        enc(&access_claims("u1"))
    );
    assert!(refused(&v, &token).await.starts_with("invalid token"));
    // An algorithm outside the pinned list, even a real one, is refused too (key confusion).
    let token = format!(
        "{}.{}.c2ln",
        enc(&json!({"alg": "HS256", "kid": "k1"})),
        enc(&access_claims("u1"))
    );
    assert_eq!(
        refused(&v, &token).await,
        "invalid token: algorithm not allowed"
    );
}

#[tokio::test]
async fn rejects_id_token_as_access_token() {
    let k = TestKey::new("k1");
    let v = verifier(&[&k], json!({}));
    let mut c = access_claims("u1");
    c.as_object_mut().unwrap().remove("scope");
    c["nonce"] = json!("n");
    assert_eq!(
        refused(&v, &k.sign(&c)).await,
        "invalid token: no scope claim: not an access token"
    );
}

#[tokio::test]
async fn rejects_expired_token() {
    let k = TestKey::new("k1");
    let v = verifier(&[&k], json!({}));
    let mut c = access_claims("u1");
    c["exp"] = json!(now().saturating_sub(3600));
    assert!(refused(&v, &k.sign(&c)).await.contains("ExpiredSignature"));
}

#[tokio::test]
async fn rejects_a_key_the_provider_does_not_publish_and_refetches_once_a_minute() {
    let k = TestKey::new("k1");
    let forger = TestKey::new("k2");
    let v = verifier(&[&k], json!({}));
    assert_eq!(
        refused(&v, &forger.sign(&access_claims("u1"))).await,
        "invalid token: unknown key id"
    );
    assert_eq!(
        refused(&v, &forger.sign(&access_claims("u1"))).await,
        "invalid token: unknown key id"
    );
    assert_eq!(
        v.keys.fetches(),
        1,
        "an unknown key id refetches at most once a minute"
    );
    // Same key id, other key: the signature fails.
    let impostor = TestKey::new("k1");
    assert!(
        refused(&v, &impostor.sign(&access_claims("u1")))
            .await
            .contains("InvalidSignature")
    );
    assert!(
        refused(&v, &"x".repeat(MAX_TOKEN_BYTES + 1))
            .await
            .contains("too long")
    );
}

#[tokio::test]
async fn missing_role_claims_come_from_userinfo_and_are_cached() {
    let k = TestKey::new("k1");
    let v = verifier(
        &[&k],
        json!({ "groups": ["kura-platform-admins"], "email": "x@example.org" }),
    );
    let mut c = access_claims("u1");
    c.as_object_mut().unwrap().remove("groups");
    let token = k.sign(&c);
    let p = v.verify_access_token(&token).await.unwrap();
    let bearer = BearerToken::new(token);
    let p = v.with_claims(p, &bearer, &["groups"]).await.unwrap();
    assert!(v.is_platform_admin(&p));
    assert!(!p.claims.contains_key("email"), "only the claims asked for");
    assert_eq!(v.userinfo_cached(), 1);
    assert_eq!(format!("{bearer:?}"), "BearerToken(<redacted>)");
}

/// The HTTP source against a local stand-in for Authentik: discovery, then
/// JWKS and userinfo from the URLs it names.
#[tokio::test]
async fn keys_and_userinfo_come_through_oidc_discovery() {
    use axum::routing::get;
    let k = TestKey::new("k1");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let jwks = json!({ "keys": [k.jwk.clone()] });
    let discovery = json!({ "jwks_uri": format!("{base}/jwks/"), "userinfo_endpoint": format!("{base}/userinfo/") });
    let app = axum::Router::new()
        .route(
            "/application/o/kura/.well-known/openid-configuration",
            get(move || async move { axum::Json(discovery) }),
        )
        .route("/jwks/", get(move || async move { axum::Json(jwks) }))
        .route(
            "/userinfo/",
            get(|h: axum::http::HeaderMap| async move {
                let bearer = h.get("authorization").is_some();
                axum::Json(json!({ "groups": ["kura-platform-admins"], "bearer": bearer }))
            }),
        );
    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    let issuer = format!("{base}/application/o/kura/");
    let keys = Keys::new(KeySource::http(&issuer).unwrap(), None);
    let v = Verifier::new(
        &OidcConfig {
            issuer: issuer.clone(),
            ..oidc()
        },
        keys,
    );
    let mut claims = access_claims("u1");
    claims["iss"] = json!(issuer);
    claims.as_object_mut().unwrap().remove("groups");
    let token = k.sign(&claims);
    let p = v.verify_access_token(&token).await.unwrap();
    assert_eq!(v.keys.fetches(), 1);
    let p = v
        .with_claims(p, &BearerToken::new(token), &["groups"])
        .await
        .unwrap();
    assert!(v.is_platform_admin(&p));

    // The provider gone: cached keys still verify, an unknown key id is refused.
    server.abort();
    let again = k.sign(&claims);
    assert!(v.verify_access_token(&again).await.is_ok());
    assert!(KeySource::http("https://unreachable.invalid/").is_ok());
    let dead = Keys::new(KeySource::http("http://127.0.0.1:1/").unwrap(), None);
    assert!(dead.get("k1").await.unwrap().is_none());
    assert!(dead.userinfo("t").await.is_err());
}

/// A key the provider stopped publishing stops verifying once the cached set
/// is older than `MAX_AGE`, without waiting for an unknown key id.
#[tokio::test(start_paused = true)]
async fn a_retired_key_stops_verifying_after_max_age() {
    let k = TestKey::new("k1");
    let v = verifier(&[&k], json!({}));
    assert!(
        v.verify_access_token(&k.sign(&access_claims("u1")))
            .await
            .is_ok()
    );
    let KeySource::Fixed { jwks, .. } = v.keys.source() else {
        panic!()
    };
    jwks.lock().unwrap().keys.clear();
    assert!(
        v.verify_access_token(&k.sign(&access_claims("u1")))
            .await
            .is_ok(),
        "still fresh"
    );
    tokio::time::advance(keys::MAX_AGE + std::time::Duration::from_secs(1)).await;
    assert_eq!(
        refused(&v, &k.sign(&access_claims("u1"))).await,
        "invalid token: unknown key id"
    );
}
