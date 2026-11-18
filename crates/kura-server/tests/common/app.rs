//! The application wired to a test database, the reconciled configuration
//! of `CONFIG`, and keys generated for the run.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use kura_server::app::{AppState, app};
use kura_server::config::{TransitConfig, VaultConfig, VaultKubernetesAuth, VaultSecrets};
use kura_server::metrics::Metrics;
use kura_server::store::Storage;
use kura_server::store::keys::DataKeys;
use kura_server::vault::{BucketSecret, Vault};
use serde_json::{Value, json};
use tower::ServiceExt;
use vaultrs::client::{VaultClient, VaultClientSettingsBuilder};

use super::tokens::TestKey;
use super::{TestDb, test_db};

pub const CONFIG: &str = r#"
apiVersion: kura.weebo.io/v1alpha1
kind: KuraTenant
metadata: { name: maxime }
spec:
  claimMappings:
    claim: groups
    roles: { owner: [maxime-owners], admin: [maxime-admins], member: [maxime-members] }
  vaultNamespaces: [maxime]
  quota: { storage: 1Mi, maxFileSize: 2Mi }
---
apiVersion: kura.weebo.io/v1alpha1
kind: KuraTenant
metadata: { name: other }
spec:
  claimMappings: { claim: groups, roles: { member: [other-members] } }
  vaultNamespaces: [other]
---
apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata: { name: knowledge, namespace: maxime }
spec:
  tenantRef: { name: maxime }
  bindings:
    - { role: editor, claimValue: maxime-members }
---
apiVersion: kura.weebo.io/v1alpha1
kind: KuraVault
metadata: { name: secrets, namespace: other }
spec:
  tenantRef: { name: other }
"#;

/// Object storage and data keys on the test Vault (or OpenBao) and S3 store.
/// Opt-in: KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT
/// (bucket `kura-test`, keys `kura-dev` / `kura-dev-secret`, see `task dev:s3`).
pub async fn storage(test: &str) -> Option<Arc<Storage>> {
    let vars = [
        "KURA_TEST_VAULT_ADDR",
        "KURA_TEST_VAULT_TOKEN",
        "KURA_TEST_S3_ENDPOINT",
    ]
    .map(|v| std::env::var(v).ok());
    let [Some(addr), Some(token), Some(endpoint)] = vars else {
        eprintln!(
            "SKIPPED: {test} — KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN or KURA_TEST_S3_ENDPOINT unset"
        );
        return None;
    };
    let mount = format!(
        "transit-chunks-{}-{}",
        std::process::id(),
        test.rsplit("::").next().unwrap_or(test).len()
    );
    let admin = VaultClient::new(
        VaultClientSettingsBuilder::default()
            .address(&addr)
            .token(&token)
            .build()
            .ok()?,
    )
    .ok()?;
    // Idempotent per process: tests of this binary share the mount.
    drop(vaultrs::sys::mount::enable(&admin, &mount, "transit", None).await);
    drop(vaultrs::transit::key::create(&admin, &mount, "kura-maxime", None).await);
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
    let vault = Arc::new(Vault::connect_with(&config, Some(token)).await.ok()?);
    let bucket = BucketSecret {
        endpoint,
        bucket: "kura-test".into(),
        region: "us-east-1".into(),
        access_key_id: "kura-dev".into(),
        secret_access_key: "kura-dev-secret".into(),
    };
    Some(Arc::new(Storage::new(
        kura_server::store::s3(&bucket).ok()?,
        DataKeys::new(vault),
    )))
}

pub struct Env {
    pub t: TestDb,
    pub key: TestKey,
    pub state: Arc<AppState>,
}

pub async fn env(test: &str) -> Option<Env> {
    env_with(test, None).await
}

/// [`env`] with object storage and data keys wired in.
pub async fn env_with(
    test: &str,
    storage: Option<Arc<kura_server::store::Storage>>,
) -> Option<Env> {
    env_custom(test, storage, |s| s).await
}

/// [`env_with`], with a last change to the state (a notifier, RFC 0003).
pub async fn env_custom(
    test: &str,
    storage: Option<Arc<kura_server::store::Storage>>,
    custom: impl FnOnce(AppState) -> AppState,
) -> Option<Env> {
    let t = test_db(test).await?;
    let dir = std::env::temp_dir().join(format!("kura-http-{}", t.name));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), CONFIG).unwrap();
    let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
    let outcome = kura_server::reconcile::reconcile(&t.app, &files, true)
        .await
        .unwrap();
    assert!(
        matches!(outcome, kura_server::reconcile::Outcome::Applied { .. }),
        "{outcome:?}"
    );
    let key = TestKey::new();
    let mut state = AppState::new(Metrics::new().unwrap(), t.app.clone(), key.verifier());
    if let Some(s) = storage {
        state = state.with_storage(s);
    }
    let state = Arc::new(custom(state));
    state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    Some(Env { t, key, state })
}

impl Env {
    pub async fn call(
        &self,
        method: &str,
        uri: &str,
        token: &str,
        device: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        if let Some(d) = device {
            req = req.header("x-kura-device", d);
        }
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        };
        let r = app(self.state.clone()).oneshot(req.unwrap()).await.unwrap();
        let status = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// Registers a device and returns its id.
    pub async fn register(&self, token: &str, name: &str) -> String {
        let (s, b) = self
            .call(
                "POST",
                "/v1/devices",
                token,
                None,
                Some(json!({ "name": name, "profile": "desktop" })),
            )
            .await;
        assert_eq!(s, StatusCode::CREATED, "{b}");
        b["id"].as_str().unwrap().to_owned()
    }
}

pub fn problem(b: &Value) -> &str {
    b["type"].as_str().unwrap_or("")
}

pub async fn vault_id(e: &Env, name: &str) -> String {
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM vault WHERE name = $1")
        .bind(name)
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    id.to_string()
}

/// A `MakeWriter` into a shared buffer.
#[derive(Clone, Default)]
pub struct Captured(pub Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
