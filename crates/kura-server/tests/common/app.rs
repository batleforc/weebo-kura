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

/// A person with a token and a registered device.
pub struct Who {
    pub token: String,
    pub device: String,
}

pub async fn who(e: &Env, sub: &str, groups: &[&str]) -> Who {
    let token = e.key.token(sub, &format!("s-{sub}"), groups);
    let device = e.register(&token, sub).await;
    Who { token, device }
}

impl Who {
    pub async fn call(
        &self,
        e: &Env,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        e.call(method, uri, &self.token, Some(&self.device), body)
            .await
    }
    pub async fn get(&self, e: &Env, uri: &str) -> (StatusCode, Value) {
        self.call(e, "GET", uri, None).await
    }
    pub async fn post(&self, e: &Env, uri: &str, body: Value) -> (StatusCode, Value) {
        self.call(e, "POST", uri, Some(body)).await
    }
    /// A commit of `ops` under a fresh idempotency key.
    pub async fn commit(&self, e: &Env, v: &str, ops: Value) -> (StatusCode, Value) {
        self.post(
            e,
            &format!("/v1/vaults/{v}/commits"),
            json!({ "idempotencyKey": uuid::Uuid::new_v4().to_string(), "operations": ops }),
        )
        .await
    }
}

/// A one-chunk file, uploaded and committed by `w`; returns its entry id.
pub async fn file(e: &Env, w: &Who, v: &str, path: &str, data: &[u8]) -> String {
    let c = kura_core::ChunkHash::of(data);
    let r = Request::builder()
        .method("PUT")
        .uri(format!("/v1/vaults/{v}/chunks/{c}"))
        .header("authorization", format!("Bearer {}", w.token))
        .header("x-kura-device", &w.device)
        .body(Body::from(data.to_vec()))
        .unwrap();
    let st = app(e.state.clone()).oneshot(r).await.unwrap().status();
    assert_eq!(st, StatusCode::NO_CONTENT);
    let size = data.len() as u64;
    let b = kura_core::blob_hash(size, &[c]).to_string();
    let (st, _) = w
        .call(
            e,
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{b}"),
            Some(json!({ "size": size, "chunks": [c.to_string()] })),
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let entry = uuid::Uuid::now_v7().to_string();
    let op = json!({ "op": "put", "entry": entry, "path": path, "blob": b, "size": size });
    let (st, body) = w.commit(e, v, json!([op])).await;
    assert_eq!(st, StatusCode::OK, "{body}");
    entry
}

impl Env {
    /// Replaces the configuration with `docs`, reconciles it and reloads the snapshot.
    pub async fn apply_config(&self, docs: &str) -> kura_server::reconcile::Outcome {
        let dir = std::env::temp_dir().join(format!("kura-http-{}", self.t.name));
        std::fs::write(dir.join("config.yaml"), docs).unwrap();
        let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
        let outcome = kura_server::reconcile::reconcile(&self.t.app, &files, true)
            .await
            .unwrap();
        self.state.set_snapshot(
            kura_server::reconcile::snapshot::load_if_changed(&self.t.app, None)
                .await
                .unwrap()
                .unwrap(),
        );
        outcome
    }
}

impl Env {
    /// Uploads one chunk as the device and returns its hash.
    pub async fn put_chunk(&self, tok: &str, d: &str, v: &str, data: &[u8]) -> String {
        let h = kura_core::ChunkHash::of(data).to_string();
        let req = Request::put(format!("/v1/vaults/{v}/chunks/{h}"))
            .header("authorization", format!("Bearer {tok}"))
            .header("x-kura-device", d)
            .body(Body::from(data.to_vec()))
            .unwrap();
        let r = app(self.state.clone()).oneshot(req).await.unwrap();
        assert!(r.status().is_success(), "{}", r.status());
        h
    }

    pub async fn put_blob(
        &self,
        (tok, d, v): (&str, &str, &str),
        b: &str,
        size: u64,
        chunk: &str,
    ) -> (StatusCode, Value) {
        self.call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{b}"),
            tok,
            Some(d),
            Some(json!({ "size": size, "chunks": [chunk] })),
        )
        .await
    }

    /// One chunk's file: chunk, manifest; returns (blob, size).
    pub async fn upload(&self, tok: &str, d: &str, v: &str, data: &[u8]) -> (String, u64) {
        let c = self.put_chunk(tok, d, v, data).await;
        let size = data.len() as u64;
        let b = kura_core::blob_hash(size, &[kura_core::ChunkHash::of(data)]).to_string();
        let (st, body) = self.put_blob((tok, d, v), &b, size, &c).await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{body}");
        (b, size)
    }

    /// Commits one operation; returns the head of its result.
    pub async fn commit_head(&self, tok: &str, d: &str, v: &str, op: Value) -> Value {
        let (st, b) = self
            .call(
                "POST",
                &format!("/v1/vaults/{v}/commits"),
                tok,
                Some(d),
                Some(json!({ "idempotencyKey": uuid::Uuid::new_v4().to_string(), "operations": [op] })),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{b}");
        b["results"][0]["head"].clone()
    }
}

/// An environment with storage and one registered device on `knowledge`.
pub struct Stored {
    pub e: Env,
    pub st: Arc<Storage>,
    pub tok: String,
    pub d: String,
    pub v: String,
}

impl Stored {
    pub async fn new(
        test: &str,
        (sub, sid, groups): (&str, &str, &[&str]),
        device: &str,
    ) -> Option<Self> {
        let st = storage(test).await?;
        let e = env_with(test, Some(st.clone())).await?;
        let tok = e.key.token(sub, sid, groups);
        let d = e.register(&tok, device).await;
        let v = vault_id(&e, "knowledge").await;
        Some(Self { e, st, tok, d, v })
    }
}

/// [`env_with`] on a fresh object store.
pub async fn env_stored(test: &str) -> Option<Env> {
    let st = storage(test).await?;
    env_with(test, Some(st)).await
}

/// Logs of the test, captured until the guard drops.
pub fn capture_logs() -> (Captured, tracing::subscriber::DefaultGuard) {
    let captured = Captured::default();
    let sink = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("debug,sqlx=off")
        .with_writer(move || sink.clone())
        .with_ansi(false)
        .finish();
    (captured, tracing::subscriber::set_default(subscriber))
}

impl Env {
    /// A token for `sub` in session `sid`, and the id of a device registered with it.
    pub async fn login(
        &self,
        (sub, sid): (&str, &str),
        groups: &[&str],
        device: &str,
    ) -> (String, String) {
        let tok = self.key.token(sub, sid, groups);
        let d = self.register(&tok, device).await;
        (tok, d)
    }

    /// A blob row for `text` (no chunks: nothing reads it); returns (hash, size).
    pub async fn blob_row(&self, v: &str, text: &str) -> (String, u64) {
        let size = text.len() as u64;
        let b = kura_core::blob_hash(size, &[kura_core::ChunkHash::of(text.as_bytes())]);
        sqlx::query("INSERT INTO blob (vault_id, tenant_id, hash, size) SELECT id, tenant_id, $2, $3 FROM vault WHERE id = $1::uuid ON CONFLICT DO NOTHING")
            .bind(v)
            .bind(b.as_bytes().as_slice())
            .bind(i64::try_from(size).unwrap())
            .execute(&self.t.owner)
            .await
            .unwrap();
        (b.to_string(), size)
    }
}

/// An environment (no storage) with a member's device on `knowledge`.
pub struct Basic {
    pub e: Env,
    pub tok: String,
    pub d: String,
    pub v: String,
}

impl Basic {
    pub async fn new(test: &str, device: &str) -> Option<Self> {
        let e = env(test).await?;
        let (tok, d) = e.login(("u1", "s1"), &["maxime-members"], device).await;
        let v = vault_id(&e, "knowledge").await;
        Some(Self { e, tok, d, v })
    }
}

impl Env {
    /// [`Env::call`] as `peer` (`ip:port`), with a bearer `token` and no device.
    pub async fn call_peer(
        &self,
        peer: &str,
        method: &str,
        uri: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {token}"));
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        };
        let mut req = req.unwrap();
        req.extensions_mut().insert(axum::extract::ConnectInfo(
            peer.parse::<std::net::SocketAddr>().unwrap(),
        ));
        let r = app(self.state.clone()).oneshot(req).await.unwrap();
        let status = r.status();
        let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}
