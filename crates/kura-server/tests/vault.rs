//! Vault KV credentials (RFC 0001 decision 48), against a Vault in dev mode.
//! Opt-in: `KURA_TEST_VAULT_ADDR` and `KURA_TEST_VAULT_TOKEN` (see `task dev:up`).

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

use kura_server::config::{TransitConfig, VaultConfig, VaultKubernetesAuth, VaultSecrets};
use kura_server::vault::Vault;
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};

fn env() -> Option<(String, String)> {
    let addr = std::env::var("KURA_TEST_VAULT_ADDR").ok();
    let token = std::env::var("KURA_TEST_VAULT_TOKEN").ok();
    let (Some(a), Some(t)) = (addr, token) else {
        eprintln!(
            "SKIPPED: {} — KURA_TEST_VAULT_ADDR or KURA_TEST_VAULT_TOKEN unset",
            module_path!()
        );
        return None;
    };
    Some((a, t))
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

/// T18: with `deploy/vault/kura-server.hcl`, the server creates and unwraps
/// data keys, and can neither delete a tenant's Transit key nor make it
/// deletable (decision 33).
#[tokio::test]
async fn server_policy_cannot_delete_key() {
    let Some((addr, token)) = env() else { return };
    let mount = format!("mp{}", std::process::id());
    let a = admin(&addr, &token, &mount).await;
    let transit = format!("{mount}-transit");
    vaultrs::transit::key::create(&a, &transit, "kura-maxime", None)
        .await
        .unwrap();
    let policy = include_str!("../../../deploy/vault/kura-server.hcl")
        .replace("\"mv/", &format!("\"{mount}/"))
        .replace("\"transit/", &format!("\"{transit}/"));
    let name = format!("kura-server-{mount}");
    vaultrs::sys::policy::set(&a, &name, &policy).await.unwrap();
    let issued = vaultrs::token::new(
        &a,
        Some(
            &mut vaultrs::api::token::requests::CreateTokenRequestBuilder::default()
                .policies(vec![name])
                .no_default_policy(true),
        ),
    )
    .await
    .unwrap();
    let server = Vault::connect_with(&config(&addr, &mount), Some(issued.client_token.clone()))
        .await
        .unwrap();
    let (plain, wrapped) = server.new_data_key("kura-maxime").await.unwrap();
    assert_eq!(
        *server
            .unwrap_data_key("kura-maxime", &wrapped)
            .await
            .unwrap(),
        *plain
    );
    let http = reqwest::Client::new();
    let call = |method: reqwest::Method, path: String, body: Option<serde_json::Value>| {
        let mut r = http
            .request(method, format!("{addr}/v1/{path}"))
            .header("x-vault-token", &issued.client_token);
        if let Some(b) = body {
            r = r.json(&b);
        }
        r.send()
    };
    let config_key = call(
        reqwest::Method::POST,
        format!("{transit}/keys/kura-maxime/config"),
        Some(serde_json::json!({ "deletion_allowed": true })),
    )
    .await
    .unwrap();
    assert_eq!(
        config_key.status().as_u16(),
        403,
        "making the key deletable"
    );
    let delete = call(
        reqwest::Method::DELETE,
        format!("{transit}/keys/kura-maxime"),
        None,
    )
    .await
    .unwrap();
    assert_eq!(delete.status().as_u16(), 403, "deleting the key");
    let trim = call(
        reqwest::Method::POST,
        format!("{transit}/keys/kura-maxime/trim"),
        Some(serde_json::json!({ "min_available_version": 1 })),
    )
    .await
    .unwrap();
    assert_eq!(trim.status().as_u16(), 403, "trimming old key versions");
    // The key is still there for the admin.
    assert!(
        vaultrs::transit::key::read(&a, &transit, "kura-maxime")
            .await
            .is_ok()
    );
}
