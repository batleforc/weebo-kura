//! Data keys against PostgreSQL and Vault (RFC 0001 §6.6, decision 29).
//! Opt-in: KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN.

mod common;

use std::sync::Arc;

use common::test_db;
use kura_core::ChunkHash;
use kura_server::config::{TransitConfig, VaultConfig, VaultKubernetesAuth, VaultSecrets};
use kura_server::store::crypto::{open, seal};
use kura_server::store::keys::DataKeys;
use kura_server::vault::Vault;
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};

async fn vault(test: &str) -> Option<Arc<Vault>> {
    let (Ok(addr), Ok(token)) = (
        std::env::var("KURA_TEST_VAULT_ADDR"),
        std::env::var("KURA_TEST_VAULT_TOKEN"),
    ) else {
        eprintln!("SKIPPED: {test} — KURA_TEST_VAULT_ADDR or KURA_TEST_VAULT_TOKEN unset");
        return None;
    };
    let mount = format!("transit-{}-{}", std::process::id(), test.len());
    let settings = VaultClientSettingsBuilder::default()
        .address(&addr)
        .token(&token)
        .build()
        .ok()?;
    let admin = VaultClient::new(settings).ok()?;
    vaultrs::sys::mount::enable(&admin, &mount, "transit", None)
        .await
        .ok()?;
    // The operator's step (decision 33): one Transit key per tenant.
    vaultrs::transit::key::create(&admin, &mount, "kura-a", None)
        .await
        .ok()?;
    let config = VaultConfig {
        address: addr,
        kubernetes_auth: VaultKubernetesAuth {
            mount: "kubernetes".into(),
            role: "kura".into(),
        },
        secrets: VaultSecrets {
            database: "kv/data/x".into(),
            bucket: "kv/data/y".into(),
        },
        transit: TransitConfig { mount },
    };
    Some(Arc::new(
        Vault::connect_with(&config, Some(token)).await.ok()?,
    ))
}

/// Two replicas creating one vault's data key keep one (§14).
#[tokio::test]
async fn two_replicas_creating_one_data_key_keep_one() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let Some(v) = vault(module_path!()).await else {
        return;
    };
    let tenant = t.tenant("a").await;
    let id = t.vault(tenant, "a", "notes").await;
    let metrics = kura_server::metrics::Metrics::new().unwrap();
    v.measure(metrics.vault_call_duration_seconds.clone());
    let (r1, r2) = (DataKeys::new(v.clone()), DataKeys::new(v.clone()));
    let (k1, k2) = tokio::join!(
        r1.get(&t.app, tenant, "a", id),
        r2.get(&t.app, tenant, "a", id)
    );
    let (k1, k2) = (k1.expect("replica 1"), k2.expect("replica 2"));
    let data = b"one key for both".to_vec();
    let h = ChunkHash::of(&data);
    let sealed = seal(&k1, id, &h, &data).expect("seal");
    assert_eq!(
        open(&k2, id, &h, &sealed).expect("the other replica opens it"),
        data
    );
    let wrapped: Option<String> = sqlx::query_scalar("SELECT wrapped_dek FROM vault WHERE id = $1")
        .bind(id)
        .fetch_one(&t.owner)
        .await
        .expect("row");
    assert!(
        wrapped.is_some_and(|w| w.starts_with("vault:v1:")),
        "stored wrapped only"
    );
    // A restarted replica unwraps the same key.
    let r3 = DataKeys::new(v);
    let k3 = r3.get(&t.app, tenant, "a", id).await.expect("replica 3");
    assert_eq!(
        open(&k3, id, &h, &sealed).expect("same key after restart"),
        data
    );
    assert_eq!(r3.cached_keys(), 1);
    // Every Transit call is timed (§10.1): two data keys made, then unwraps.
    let calls = metrics
        .vault_call_duration_seconds
        .with_label_values(&["ok"])
        .get_sample_count();
    assert!(calls >= 3, "{calls}");
}
