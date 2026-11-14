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

        /// An access token for `sub` in session `sid`, carrying `groups`.
        pub fn token(&self, sub: &str, sid: &str, groups: &[&str]) -> String {
            let exp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .saturating_add(3600);
            let claims = json!({ "iss": ISSUER, "aud": "kura-cli", "sub": sub, "sid": sid, "exp": exp,
                                 "scope": "openid profile", "groups": groups });
            let mut header = Header::new(Algorithm::EdDSA);
            header.kid = Some("k1".into());
            encode(&header, &claims, &self.encoding).unwrap()
        }

        pub fn verifier(&self) -> kura_server::auth::Verifier {
            use kura_server::auth::{KeySource, Keys, Verifier};
            use kura_server::config::{Algorithm, ClaimMatch, OidcConfig};
            let oidc = OidcConfig {
                issuer: ISSUER.into(),
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
}
