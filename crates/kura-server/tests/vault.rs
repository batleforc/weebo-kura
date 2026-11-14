//! Vault KV credentials (RFC 0001 decision 48), against a Vault in dev mode.
//! Opt-in: `KURA_TEST_VAULT_ADDR` and `KURA_TEST_VAULT_TOKEN` (see `task dev:up`).

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

use kura_server::config::{TransitConfig, VaultConfig, VaultKubernetesAuth, VaultSecrets};
use kura_server::vault::Vault;
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};

fn env() -> Option<(String, String)> {
    let addr = std::env::var("KURA_TEST_VAULT_ADDR").ok();
    let token = std::env::var("KURA_TEST_VAULT_TOKEN").ok();
    match (addr, token) {
        (Some(a), Some(t)) => Some((a, t)),
        _ => {
            eprintln!(
                "SKIPPED: {} — KURA_TEST_VAULT_ADDR or KURA_TEST_VAULT_TOKEN unset",
                module_path!()
            );
            None
        }
    }
}

fn config(addr: &str, mount: &str) -> VaultConfig {
    VaultConfig {
        address: addr.into(),
        kubernetes_auth: VaultKubernetesAuth {
            mount: "kubernetes".into(),
            role: "kura".into(),
        },
        secrets: VaultSecrets {
            database: format!("{mount}/data/kura/database"),
            bucket: format!("{mount}/data/kura/bucket"),
        },
        transit: TransitConfig {
            mount: format!("{mount}-transit"),
        },
    }
}

/// A fresh KV v2 mount per test, so tests do not share secrets.
async fn admin(addr: &str, token: &str, mount: &str) -> VaultClient {
    let settings = VaultClientSettingsBuilder::default()
        .address(addr)
        .token(token)
        .build()
        .unwrap();
    let client = VaultClient::new(settings).unwrap();
    vaultrs::sys::mount::enable(&client, mount, "kv-v2", None)
        .await
        .unwrap();
    vaultrs::sys::mount::enable(&client, &format!("{mount}-transit"), "transit", None)
        .await
        .unwrap();
    client
}

#[tokio::test]
async fn reads_database_and_bucket_credentials_from_kv() {
    let Some((addr, token)) = env() else { return };
    let mount = format!("mv{}", std::process::id());
    let a = admin(&addr, &token, &mount).await;
    vaultrs::kv2::set(
        &a,
        &mount,
        "kura/database",
        &serde_json::json!({ "url": "postgres://kura_app:s3cret@db/kura" }),
    )
    .await
    .unwrap();
    vaultrs::kv2::set(
        &a,
        &mount,
        "kura/bucket",
        &serde_json::json!({ "endpoint": "http://s3:9000", "bucket": "kura", "accessKeyId": "AK", "secretAccessKey": "SK" }),
    )
    .await
    .unwrap();
    let vault = Vault::connect_with(&config(&addr, &mount), Some(token.clone()))
        .await
        .unwrap();
    assert_eq!(
        vault.database().await.unwrap().url,
        "postgres://kura_app:s3cret@db/kura"
    );
    let bucket = vault.bucket().await.unwrap();
    assert_eq!(
        (bucket.bucket.as_str(), bucket.region.as_str()),
        ("kura", "us-east-1")
    );
    // Rotation: the next read sees the new version.
    vaultrs::kv2::set(
        &a,
        &mount,
        "kura/database",
        &serde_json::json!({ "url": "postgres://kura_app:rotated@db/kura" }),
    )
    .await
    .unwrap();
    assert_eq!(
        vault.database().await.unwrap().url,
        "postgres://kura_app:rotated@db/kura"
    );
}

#[tokio::test]
async fn refuses_a_bad_address_and_a_bad_token() {
    let Some((addr, _)) = env() else { return };
    assert!(
        Vault::connect_with(&config("not a url", "mv"), Some("t".into()))
            .await
            .is_err()
    );
    let vault = Vault::connect_with(&config(&addr, "mv"), Some("wrong-token".into()))
        .await
        .unwrap();
    assert!(
        vault.database().await.is_err(),
        "a wrong token reads nothing"
    );
    // Without a token and outside Kubernetes, there is no service account to log in with.
    assert!(
        Vault::connect_with(&config(&addr, "mv"), None)
            .await
            .is_err()
    );
}
