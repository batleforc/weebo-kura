//! Shared harness for the tests that need PostgreSQL. Opt-in through
//! `KURA_TEST_DATABASE_URL` (a superuser URL, see `task dev:up`), with a loud
//! skip so `task test` runs without it (weebo-testing). Each test gets its own
//! database, owned by `kura_owner`, migrated, and a pool of `kura_app`.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    dead_code,
    reason = "test helpers fail the test on purpose; each test binary uses a subset"
)]

use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

pub mod app;

pub const ROLE_PASSWORD: &str = "kura-test";

pub struct TestDb {
    /// `kura_owner`, the migration owner (and definer of the functions).
    pub owner: PgPool,
    /// `kura_app`, the role the server connects with.
    pub app: PgPool,
    pub name: String,
    pub app_options: PgConnectOptions,
}

pub fn admin_url(test: &str) -> Option<String> {
    let url = std::env::var("KURA_TEST_DATABASE_URL").ok();
    if url.is_none() {
        eprintln!("SKIPPED: {test} — KURA_TEST_DATABASE_URL unset");
    }
    url
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh, migrated database, or `None` (after printing the skip).
pub async fn test_db(test: &str) -> Option<TestDb> {
    let url = admin_url(test)?;
    let admin_opts = PgConnectOptions::from_str(&url).unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(admin_opts.clone())
        .await
        .unwrap();
    let bootstrap = include_str!("../../sql/bootstrap-roles.sql");
    // Roles are cluster-wide and test binaries run in parallel: serialize.
    let mut tx = admin.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(4242)")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::raw_sql(bootstrap).execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'kura_owner') THEN CREATE ROLE kura_owner LOGIN; END IF; END $$;
         ALTER ROLE kura_owner PASSWORD '{ROLE_PASSWORD}';
         ALTER ROLE kura_app PASSWORD '{ROLE_PASSWORD}';"
    )))
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // Databases of test runs that have ended: a database per test would
    // otherwise pile up in a long-lived local cluster. Another binary may
    // drop the same one first: errors are ignored.
    #[cfg(target_os = "linux")]
    {
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT datname FROM pg_database WHERE datname LIKE 'kura\\_test\\_%'",
        )
        .fetch_all(&admin)
        .await
        .unwrap();
        for d in names {
            let pid = d.split('_').nth(2).unwrap_or("");
            if !pid.is_empty() && !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                drop(
                    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                        "DROP DATABASE IF EXISTS \"{d}\" WITH (FORCE)"
                    )))
                    .execute(&admin)
                    .await,
                );
            }
        }
    }

    let name = format!(
        "kura_test_{}_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
        test.rsplit("::")
            .next()
            .unwrap_or(test)
            .chars()
            .take(30)
            .collect::<String>()
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE \"{name}\" OWNER kura_owner"
    )))
    .execute(&admin)
    .await
    .unwrap();
    admin.close().await;

    let owner_options = admin_opts
        .clone()
        .username("kura_owner")
        .password(ROLE_PASSWORD)
        .database(&name);
    let owner = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(owner_options)
        .await
        .unwrap();
    kura_server::db::migrate(&owner).await.unwrap();
    let app_options = admin_opts
        .username("kura_app")
        .password(ROLE_PASSWORD)
        .database(&name);
    let app = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(app_options.clone())
        .await
        .unwrap();
    Some(TestDb {
        owner,
        app,
        name,
        app_options,
    })
}

impl TestDb {
    /// Inserts a tenant as the owner (the reconciler's path) and returns its id.
    pub async fn tenant(&self, slug: &str) -> uuid::Uuid {
        sqlx::query_scalar(
            "INSERT INTO tenant (slug, vault_namespaces) VALUES ($1, ARRAY[$1]) RETURNING id",
        )
        .bind(slug)
        .fetch_one(&self.owner)
        .await
        .unwrap()
    }

    pub async fn vault(&self, tenant: uuid::Uuid, namespace: &str, name: &str) -> uuid::Uuid {
        sqlx::query_scalar(
            "INSERT INTO vault (tenant_id, namespace, name, chunking_params, managed_by)
             VALUES ($1, $2, $3, jsonb_build_object('min', '256Ki', 'avg', '1Mi', 'max', '4Mi'), 'config') RETURNING id",
        )
        .bind(tenant)
        .bind(namespace)
        .bind(name)
        .fetch_one(&self.owner)
        .await
        .unwrap()
    }
}

/// A verifier with no key: every token is refused. For tests that do not authenticate.
pub fn no_key_verifier() -> kura_server::auth::Verifier {
    use kura_server::auth::{KeySource, Keys, Verifier};
    use kura_server::config::{Algorithm, ClaimMatch, OidcConfig};
    let oidc = OidcConfig {
        issuer: "https://auth.example.org/".into(),
        discovery: None,
        audiences: vec!["kura-cli".into()],
        algorithms: vec![Algorithm::EdDSA],
        platform_admin: ClaimMatch {
            claim: "groups".into(),
            value: "admins".into(),
        },
    };
    let source = KeySource::Fixed {
        jwks: std::sync::Mutex::new(jsonwebtoken::jwk::JwkSet { keys: Vec::new() }),
        userinfo: serde_json::Map::new(),
    };
    Verifier::new(&oidc, Keys::new(source, None))
}

pub mod tokens {
    //! Signing keys generated per run (nothing committed) and a verifier for them.

    use aws_lc_rs::rand::SystemRandom;
    use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
    use serde_json::{Value, json};

    pub const ISSUER: &str = "https://auth.example.org/application/o/kura/";

    pub struct TestKey {
        encoding: EncodingKey,
        jwk: Value,
    }

    impl TestKey {
        pub fn new() -> Self {
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
            let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
            let x = URL_SAFE_NO_PAD.encode(pair.public_key().as_ref());
            Self {
                encoding: EncodingKey::from_ed_der(pkcs8.as_ref()),
                jwk: json!({ "kty": "OKP", "crv": "Ed25519", "x": x, "kid": "k1", "alg": "EdDSA" }),
            }
        }

        /// An access token for `sub` in session `sid`, carrying `groups`; its
        /// `auth_time` is derived from `sid`, so one session has one.
        pub fn token(&self, sub: &str, sid: &str, groups: &[&str]) -> String {
            let auth_time = sid
                .bytes()
                .fold(1_700_000_000_i64, |a, b| {
                    a.wrapping_mul(31).wrapping_add(i64::from(b))
                })
                .rem_euclid(1 << 31);
            self.token_with(sub, Some(sid), Some(auth_time), groups)
        }

        /// [`Self::token`] carrying the person's `name` claim (RFC 0006 D13).
        pub fn token_named(&self, sub: &str, groups: &[&str], name: &str) -> String {
            self.token_extra(
                sub,
                Some(&format!("s-{sub}")),
                Some(1_700_000_000),
                groups,
                &json!({ "name": name }),
            )
        }

        /// The session of a refreshed Authentik token: maybe no `sid`.
        pub fn token_with(
            &self,
            sub: &str,
            sid: Option<&str>,
            auth_time: Option<i64>,
            groups: &[&str],
        ) -> String {
            self.token_extra(sub, sid, auth_time, groups, &json!({}))
        }

        fn token_extra(
            &self,
            sub: &str,
            sid: Option<&str>,
            auth_time: Option<i64>,
            groups: &[&str],
            extra: &Value,
        ) -> String {
            let exp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .saturating_add(3600);
            let mut claims = json!({ "iss": ISSUER, "aud": "kura-cli", "sub": sub, "exp": exp,
                                 "scope": "openid profile", "groups": groups });
            if let Some(s) = sid {
                claims["sid"] = json!(s);
            }
            if let Some(t) = auth_time {
                claims["auth_time"] = json!(t);
            }
            for (k, v) in extra.as_object().into_iter().flatten() {
                claims[k] = v.clone();
            }
            let mut header = Header::new(Algorithm::EdDSA);
            header.kid = Some("k1".into());
            encode(&header, &claims, &self.encoding).unwrap()
        }

        pub fn verifier(&self) -> kura_server::auth::Verifier {
            use kura_server::auth::{KeySource, Keys, Verifier};
            use kura_server::config::{Algorithm, ClaimMatch, OidcConfig};
            let oidc = OidcConfig {
                issuer: ISSUER.into(),
                discovery: None,
                audiences: vec!["kura-cli".into()],
                algorithms: vec![Algorithm::EdDSA],
                platform_admin: ClaimMatch {
                    claim: "groups".into(),
                    value: "kura-platform-admins".into(),
                },
            };
            let set = serde_json::from_value(json!({ "keys": [self.jwk.clone()] })).unwrap();
            Verifier::new(
                &oidc,
                Keys::new(
                    KeySource::Fixed {
                        jwks: std::sync::Mutex::new(set),
                        userinfo: serde_json::Map::new(),
                    },
                    None,
                ),
            )
        }
    }

    /// The cluster's service-account issuer and kura-operator's identity
    /// (RFC 0009), with an RSA key generated for the run.
    pub const SA_ISSUER: &str = "https://kubernetes.default.svc.cluster.local";
    pub const OPERATOR: &str = "system:serviceaccount:kura-operator:kura-operator";

    pub struct OperatorKey {
        encoding: EncodingKey,
        jwk: Value,
    }

    /// The `RSAPrivateKey` inside a PKCS#8 `PrivateKeyInfo`: SEQUENCE {
    /// INTEGER, SEQUENCE (algorithm), OCTET STRING (the key) }.
    fn pkcs1_of(der: &[u8]) -> &[u8] {
        fn element(d: &[u8]) -> (usize, usize) {
            match d[1] {
                n if n < 0x80 => (2, usize::from(n)),
                n => {
                    let bytes = usize::from(n & 0x7f);
                    let len = d[2..2 + bytes]
                        .iter()
                        .fold(0, |a, b| (a << 8) | usize::from(*b));
                    (2 + bytes, len)
                }
            }
        }
        let (outer, _) = element(der);
        let mut rest = &der[outer..];
        for _ in 0..2 {
            let (h, l) = element(rest);
            rest = &rest[h.saturating_add(l)..];
        }
        assert_eq!(rest[0], 0x04);
        let (h, l) = element(rest);
        &rest[h..h.saturating_add(l)]
    }

    impl OperatorKey {
        pub fn new() -> Self {
            use aws_lc_rs::encoding::AsDer;
            let pair = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
            let pkcs8 = pair.as_der().unwrap();
            let public = pair.public_key();
            let n = URL_SAFE_NO_PAD.encode(public.modulus().big_endian_without_leading_zero());
            let e = URL_SAFE_NO_PAD.encode(public.exponent().big_endian_without_leading_zero());
            Self {
                encoding: EncodingKey::from_rsa_der(pkcs1_of(pkcs8.as_ref())),
                jwk: json!({ "kty": "RSA", "n": n, "e": e, "kid": "sa1", "alg": "RS256" }),
            }
        }

        /// A projected token of `sub` for `aud`, valid `ttl` seconds (negative: expired).
        pub fn token(&self, sub: &str, aud: &str, ttl: i64) -> String {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let exp = now.saturating_add_signed(ttl);
            let claims =
                json!({ "iss": SA_ISSUER, "aud": [aud], "sub": sub, "exp": exp, "iat": now });
            let mut header = Header::new(Algorithm::RS256);
            header.kid = Some("sa1".into());
            encode(&header, &claims, &self.encoding).unwrap()
        }

        /// kura-operator's token, as the kubelet projects it.
        pub fn operator(&self) -> String {
            self.token(OPERATOR, "kura", 3600)
        }

        pub fn verifier(&self) -> kura_server::auth::OperatorVerifier {
            use kura_server::auth::{KeySource, Keys, OperatorVerifier};
            let identity = kura_server::config::OperatorIdentity {
                issuer: SA_ISSUER.into(),
                audience: "kura".into(),
                subject: OPERATOR.into(),
                ca_file: None,
                token_file: None,
            };
            let set = serde_json::from_value(json!({ "keys": [self.jwk.clone()] })).unwrap();
            OperatorVerifier::new(
                &identity,
                Keys::new(
                    KeySource::Fixed {
                        jwks: std::sync::Mutex::new(set),
                        userinfo: serde_json::Map::new(),
                    },
                    None,
                ),
            )
        }
    }
}
