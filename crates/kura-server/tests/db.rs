//! Row-level security and roles (RFC 0001 T4, decision 26), against a real
//! PostgreSQL. Opt-in: `task test:db`, or `KURA_TEST_DATABASE_URL`.

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use common::test_db;
use kura_server::db;
use sqlx::Row;
use uuid::Uuid;

/// Tables of the public schema without forced row-level security.
async fn tables_without_forced_rls(pool: &sqlx::PgPool) -> (usize, Vec<String>) {
    let rows = sqlx::query(
        "SELECT c.relname, c.relrowsecurity AND c.relforcerowsecurity
         FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname = 'public' AND c.relkind IN ('r', 'p') AND c.relname <> '_sqlx_migrations'",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let missing = rows
        .iter()
        .filter(|r| !r.get::<bool, _>(1))
        .map(|r| r.get(0))
        .collect();
    (rows.len(), missing)
}

#[tokio::test]
async fn every_table_forces_rls() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let (count, missing) = tables_without_forced_rls(&t.owner).await;
    assert!(count >= 6, "expected the phase 1 tables, got {count}");
    assert!(
        missing.is_empty(),
        "tables without forced row-level security: {missing:?}"
    );
    // The gate can fail: a table that forgets it is reported.
    sqlx::query("CREATE TABLE gate_probe (id int)")
        .execute(&t.owner)
        .await
        .unwrap();
    assert_eq!(tables_without_forced_rls(&t.owner).await.1, ["gate_probe"]);
}

#[tokio::test]
async fn app_role_is_not_owner() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let r = sqlx::query("SELECT rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user")
        .fetch_one(&t.app)
        .await
        .unwrap();
    assert!(!r.get::<bool, _>(0), "superuser bypasses RLS");
    assert!(!r.get::<bool, _>(1), "BYPASSRLS");
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_tables WHERE schemaname = 'public' AND tableowner = current_user",
    )
    .fetch_one(&t.app)
    .await
    .unwrap();
    assert_eq!(owned, 0);
    for (setting, expected) in [
        ("lock_timeout", "5s"),
        ("statement_timeout", "5s"),
        ("idle_in_transaction_session_timeout", "10s"),
    ] {
        let v: String = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SHOW {setting}")))
            .fetch_one(&t.app)
            .await
            .unwrap();
        assert_eq!(v, expected, "{setting}");
    }
}

#[tokio::test]
async fn rls_hides_other_tenant() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let (a, b) = (t.tenant("a").await, t.tenant("b").await);
    let va = t.vault(a, "a", "notes").await;
    t.vault(b, "b", "notes").await;

    let mut tx = db::begin_tenant(&t.app, a, None).await.unwrap();
    let vaults: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM vault")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(vaults, vec![va]);
    let tenants: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM tenant")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(tenants, vec![a]);
    // Writing into the other tenant is refused by WITH CHECK.
    let e = sqlx::query("INSERT INTO vault (tenant_id, namespace, name, chunking_params, managed_by) VALUES ($1, 'b', 'x', '{}', 'api')")
        .bind(b)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("row-level security"), "{e}");
}

#[tokio::test]
async fn unset_tenant_setting_returns_nothing() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let a = t.tenant("a").await;
    t.vault(a, "a", "notes").await;
    sqlx::query("INSERT INTO device (user_sub, session_sid, name, profile) VALUES ('u1', 's1', 'laptop', 'desktop')")
        .execute(&t.owner)
        .await
        .unwrap();
    for table in ["tenant", "vault", "role_binding", "device"] {
        let n: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(&t.app)
                .await
                .unwrap();
        assert_eq!(n, 0, "{table} without a setting");
    }
}

#[tokio::test]
async fn pooled_connection_does_not_leak_tenant() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let a = t.tenant("a").await;
    t.vault(a, "a", "notes").await;
    let one = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(t.app_options.clone())
        .await
        .unwrap();
    let mut tx = db::begin_tenant(&one, a, Some("u1")).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM vault")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(n, 1);
    tx.commit().await.unwrap();
    // Same physical connection, next request without a tenant.
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM vault")
        .fetch_one(&one)
        .await
        .unwrap();
    assert_eq!(n, 0, "the tenant setting outlived its transaction");
    let s: Option<String> = sqlx::query_scalar("SELECT kura_current_subject()")
        .fetch_one(&one)
        .await
        .unwrap();
    assert_eq!(s, None);
}

#[tokio::test]
async fn devices_are_isolated_by_subject() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    for sub in ["u1", "u2"] {
        sqlx::query("INSERT INTO device (user_sub, session_sid, name, profile) VALUES ($1, 's', 'laptop', 'desktop')")
            .bind(sub)
            .execute(&t.owner)
            .await
            .unwrap();
    }
    let mut tx = db::begin_subject(&t.app, "u1").await.unwrap();
    let subs: Vec<String> = sqlx::query_scalar("SELECT user_sub FROM device")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(subs, ["u1"]);
    let e = sqlx::query("INSERT INTO device (user_sub, session_sid, name, profile) VALUES ('u2', 's', 'x', 'mobile')")
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("row-level security"), "{e}");
}

/// The named SECURITY DEFINER functions, one by one (T4 residual).
#[tokio::test]
async fn definer_functions_return_only_what_they_name() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let a = t.tenant("a").await;
    let v = t.vault(a, "a", "notes").await;
    assert_eq!(db::vault_tenant(&t.app, v).await.unwrap(), Some(a));
    assert_eq!(
        db::vault_tenant(&t.app, Uuid::from_u128(1)).await.unwrap(),
        None
    );

    // JWKS: replace, load, purge a key no longer published; never readable directly.
    let keys = serde_json::json!([{"kid": "k1", "kty": "RSA"}, {"kid": "k2", "kty": "RSA"}]);
    sqlx::query("SELECT kura_jwks_replace($1)")
        .bind(&keys)
        .execute(&t.app)
        .await
        .unwrap();
    sqlx::query("SELECT kura_jwks_replace($1)")
        .bind(serde_json::json!([{"kid": "k2", "kty": "RSA"}]))
        .execute(&t.app)
        .await
        .unwrap();
    let kids: Vec<String> = sqlx::query_scalar("SELECT kid FROM kura_jwks_load()")
        .fetch_all(&t.app)
        .await
        .unwrap();
    assert_eq!(kids, ["k2"]);
    let e = sqlx::query("SELECT * FROM jwks_key")
        .fetch_all(&t.app)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("permission denied"), "{e}");
    // kura_app reads config_state but can neither lock nor write it.
    let e = sqlx::query("SELECT * FROM config_state FOR UPDATE")
        .fetch_all(&t.app)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("permission denied"), "{e}");
}

#[tokio::test]
async fn schema_check_refuses_an_older_schema_only() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    db::check_schema(&t.app).await.unwrap();
    // A newer schema (a migration this binary does not know) is accepted.
    sqlx::query("INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) VALUES (99999999999999, 'future', true, '\\x00', 0)")
        .execute(&t.owner)
        .await
        .unwrap();
    db::check_schema(&t.app).await.unwrap();
    // A migration of this binary missing is refused, even below the newest
    // applied one (here the future one above).
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 1")
        .execute(&t.owner)
        .await
        .unwrap();
    let e = db::check_schema(&t.app).await.unwrap_err();
    assert!(
        matches!(e, db::DbError::SchemaTooOld { missing: 1, .. }),
        "{e}"
    );
}

/// Keys fetched by one replica are usable by another started while Authentik
/// is down (§6.2).
#[tokio::test]
async fn jwks_survive_a_restart_without_the_provider() {
    use kura_server::auth::{KeySource, Keys};
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    // A public key only: 32 arbitrary bytes, nothing secret.
    let jwk = serde_json::json!({ "kty": "OKP", "crv": "Ed25519", "kid": "k1", "alg": "EdDSA",
                                  "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo" });
    let set: jsonwebtoken::jwk::JwkSet =
        serde_json::from_value(serde_json::json!({ "keys": [jwk] })).unwrap();
    let fixed = |set| KeySource::Fixed {
        jwks: std::sync::Mutex::new(set),
        userinfo: serde_json::Map::new(),
    };
    let first = Keys::new(fixed(set), Some(t.app.clone()));
    assert!(first.get("k1").await.unwrap().is_some());

    // Restarted while Authentik is unreachable.
    let restarted = Keys::new(
        KeySource::http("http://127.0.0.1:1/").unwrap(),
        Some(t.app.clone()),
    );
    assert_eq!(restarted.load_persisted().await.unwrap(), 1);
    assert!(
        restarted.get("k1").await.unwrap().is_some(),
        "served from the persisted copy"
    );
}

/// A chunk row can only carry its vault's own tenant (phase 2 review P6).
#[tokio::test]
async fn a_chunk_cannot_claim_another_tenant() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let (a, b) = (t.tenant("a").await, t.tenant("b").await);
    let va = t.vault(a, "a", "notes").await;
    let e = sqlx::query("INSERT INTO chunk (vault_id, tenant_id, hash, size, object_key) VALUES ($1, $2, $3, 1, 'k')")
        .bind(va)
        .bind(b)
        .bind(vec![0u8; 32])
        .execute(&t.owner)
        .await
        .unwrap_err();
    assert!(e.to_string().contains("foreign key"), "{e}");
}
