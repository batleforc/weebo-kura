//! The `kura-operator` binary itself against envtest, as it runs in a pod
//! (RFC 0009 §14): `main`'s wiring, the Lease, the push through the real
//! HTTP gateway (TokenRequest, the CA of `tls.caSecretRef`, HTTPS) to a
//! stand-in kura-server, the webhook served over TLS and called by
//! kube-apiserver itself, the metrics, and a clean exit on SIGTERM.
//! `task operator:test` builds the binary before the tests.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::http::HeaderMap;
use axum::routing::put;
use base64::Engine;
use k8s_openapi::api::admissionregistration::v1::ValidatingWebhookConfiguration;
use k8s_openapi::api::coordination::v1::Lease;
use k8s_openapi::api::core::v1::{Namespace, Secret, ServiceAccount};
use kube::api::{Api, PostParams};
use kura_api::{KuraInstance, KuraTenantResource, KuraVaultResource};
use serde_json::{Value, json};
use weebo_kit_runtime::webhook_server::{MissingCertificate, WebhookServerConfig, spawn_with};
use weebo_kit_runtime::{Health, Shutdown};

const TIMEOUT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(250);

/// The binary `task operator:test` builds and names in `KURA_OPERATOR_BIN`
/// (cargo's own path for it: the build directory's layout varies).
fn binary() -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os("KURA_OPERATOR_BIN")
            .expect("KURA_OPERATOR_BIN names the kura-operator binary: run task operator:test"),
    );
    assert!(path.exists(), "{} does not exist", path.display());
    path
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port()
}

async fn create<K>(api: &Api<K>, value: Value) -> K
where
    K: kube::Resource + Clone + std::fmt::Debug + serde::de::DeserializeOwned + serde::Serialize,
{
    let obj: K = serde_json::from_value(value).expect("a valid object");
    api.create(&PostParams::default(), &obj)
        .await
        .expect("created")
}

/// Polls `check` until it gives a value, or fails the test naming `what`.
async fn eventually<T, F, Fut>(what: &str, mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    tokio::time::timeout(TIMEOUT, async {
        loop {
            if let Some(v) = check().await {
                return v;
            }
            tokio::time::sleep(POLL).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("never: {what}"))
}

/// What kura-server answers when it applies a set: every document `Ready`.
async fn apply(pushes: Arc<AtomicUsize>, headers: HeaderMap, body: Value) -> axum::Json<Value> {
    assert!(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("Bearer ")),
        "a bearer on every push"
    );
    pushes.fetch_add(1, Ordering::SeqCst);
    let objects: Vec<Value> = body["documents"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|d| {
            json!({
                "kind": d["kind"], "namespace": d["metadata"]["namespace"], "name": d["metadata"]["name"],
                "conditions": [{ "type": "Ready", "status": "True", "reason": "Reconciled", "message": "",
                                 "observedGeneration": d["metadata"]["generation"] }]
            })
        })
        .collect();
    axum::Json(json!({ "applied": true, "outcome": "applied", "objects": objects }))
}

#[tokio::test]
async fn the_binary_pushes_over_tls_and_its_webhook_guards_the_apiserver() {
    drop(rustls::crypto::ring::default_provider().install_default());
    let binary = binary();

    // --version and an unknown command need no cluster.
    let version = tokio::process::Command::new(&binary)
        .arg("--version")
        .output()
        .await
        .expect("ran");
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("kura-operator "));
    let unknown = tokio::process::Command::new(&binary)
        .arg("--nope")
        .output()
        .await
        .expect("ran");
    assert!(!unknown.status.success());

    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("binary-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a work dir");
    let kubeconfig = dir.join("kubeconfig.yaml");
    cluster.write_kubeconfig(&kubeconfig);

    // One self-signed certificate for 127.0.0.1: the stand-in kura-server's
    // and the webhook's, trusted through caSecretRef and caBundle.
    let key = rcgen::KeyPair::generate().expect("a key");
    let cert = rcgen::CertificateParams::new(vec!["127.0.0.1".into(), "localhost".into()])
        .expect("params")
        .self_signed(&key)
        .expect("a certificate");
    let (cert_file, key_file) = (dir.join("tls.crt"), dir.join("tls.key"));
    std::fs::write(&cert_file, cert.pem()).expect("cert written");
    std::fs::write(&key_file, key.serialize_pem()).expect("key written");

    // The stand-in kura-server, over HTTPS.
    let pushes = Arc::new(AtomicUsize::new(0));
    let counted = pushes.clone();
    let kura_port = free_port();
    let _kura = spawn_with(
        WebhookServerConfig {
            addr: SocketAddr::from(([127, 0, 0, 1], kura_port)),
            cert_file: cert_file.clone(),
            key_file: key_file.clone(),
            drain: Duration::ZERO,
            missing_certificate: MissingCertificate::Fail,
        },
        axum::Router::new().route(
            "/v1/admin/documents",
            put(move |h: HeaderMap, axum::Json(b): axum::Json<Value>| apply(counted.clone(), h, b)),
        ),
        Arc::new(Health::default()),
        Shutdown::never(),
    )
    .await
    .expect("the stand-in kura-server");

    for ns in ["family", "team-b"] {
        create(
            &Api::<Namespace>::all(client.clone()),
            json!({ "metadata": { "name": ns } }),
        )
        .await;
    }
    create(
        &Api::<ServiceAccount>::namespaced(client.clone(), "default"),
        json!({ "metadata": { "name": "kura-operator" } }),
    )
    .await;
    create(
        &Api::<Secret>::namespaced(client.clone(), "default"),
        json!({ "metadata": { "name": "kura-ca" }, "stringData": { "ca.crt": cert.pem() } }),
    )
    .await;
    let instances: Api<KuraInstance> = Api::all(client.clone());
    create(
        &instances,
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": "kura" },
                "spec": { "url": format!("https://127.0.0.1:{kura_port}"),
                          "auth": { "serviceAccountToken": { "audience": "kura" } },
                          "tls": { "caSecretRef": { "namespace": "default", "name": "kura-ca", "key": "ca.crt" } } } }),
    )
    .await;
    create(
        &Api::<KuraTenantResource>::all(client.clone()),
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "family" },
                "spec": { "vaultNamespaces": ["family"] } }),
    )
    .await;
    let vaults: Api<KuraVaultResource> = Api::namespaced(client.clone(), "family");
    create(
        &vaults,
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault", "metadata": { "name": "recipes", "namespace": "family" },
                "spec": { "tenantRef": { "name": "family" } } }),
    )
    .await;

    // A missing ServiceAccount variable: the binary refuses to start.
    let refused = tokio::process::Command::new(&binary)
        .env("KUBECONFIG", &kubeconfig)
        .env("POD_NAMESPACE", "default")
        .env_remove("KURA_SERVICE_ACCOUNT")
        .env("WEBHOOK_TLS_CERT_FILE", dir.join("none"))
        .output()
        .await
        .expect("ran");
    assert!(!refused.status.success());

    let webhook_port = free_port();
    let metrics_port = free_port();
    let log = std::fs::File::create(dir.join("operator.log")).expect("a log file");
    let mut operator = tokio::process::Command::new(&binary)
        .env("KUBECONFIG", &kubeconfig)
        .env("POD_NAMESPACE", "default")
        .env("HOSTNAME", "kura-operator-e2e")
        .env("KURA_SERVICE_ACCOUNT", "kura-operator")
        .env("WEBHOOK_PORT", webhook_port.to_string())
        .env("WEBHOOK_TLS_CERT_FILE", &cert_file)
        .env("WEBHOOK_TLS_KEY_FILE", &key_file)
        .env("WEEBO_SHUTDOWN_DRAIN_SECONDS", "0")
        .env("WEEBO_METRICS_ADDR", format!("127.0.0.1:{metrics_port}"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().expect("log handle"))
        .stderr(log)
        .kill_on_drop(true)
        .spawn()
        .expect("the operator starts");

    // The push: the vault and the instance turn Ready from kura-server's answer.
    eventually("the vault Ready from the push", || async {
        let v = vaults.get("recipes").await.ok()?;
        let s = v.status?;
        (s.observed_generation == v.metadata.generation
            && weebo_kit_api::status::ready(&s.conditions).is_some_and(|c| c.status == "True"))
        .then_some(())
    })
    .await;
    assert!(pushes.load(Ordering::SeqCst) >= 1);
    let lease = Api::<Lease>::namespaced(client.clone(), "default")
        .get("kura-operator-leader")
        .await
        .expect("the Lease");
    assert_eq!(
        lease.spec.and_then(|s| s.holder_identity).as_deref(),
        Some("kura-operator-e2e")
    );
    let metrics_url = format!("http://127.0.0.1:{metrics_port}/metrics");
    eventually("the leader gauge at 1", || async {
        let body = reqwest::get(&metrics_url).await.ok()?.text().await.ok()?;
        body.lines()
            .any(|l| l.starts_with("kura_operator_leader") && l.ends_with(" 1"))
            .then_some(())
    })
    .await;

    // The webhook, registered with kube-apiserver: the apiserver asks it.
    let ca_bundle = base64::engine::general_purpose::STANDARD.encode(cert.pem());
    create(
        &Api::<ValidatingWebhookConfiguration>::all(client.clone()),
        json!({ "metadata": { "name": "kura-operator" },
                "webhooks": [{ "name": "validate.kura.weebo.io", "admissionReviewVersions": ["v1"],
                               "sideEffects": "None", "failurePolicy": "Fail", "timeoutSeconds": 5,
                               "clientConfig": { "url": format!("https://127.0.0.1:{webhook_port}/validate"), "caBundle": ca_bundle },
                               "rules": [{ "apiGroups": ["kura.weebo.io"], "apiVersions": ["v1alpha1"],
                                           "operations": ["CREATE", "UPDATE"], "resources": ["kuravaults", "kurainstances"] }] }] }),
    )
    .await;
    let dry_run = PostParams {
        dry_run: true,
        ..PostParams::default()
    };
    let outsider: KuraVaultResource = serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault", "metadata": { "name": "notes", "namespace": "team-b" },
        "spec": { "tenantRef": { "name": "family" } }
    }))
    .expect("a vault");
    let team_b: Api<KuraVaultResource> = Api::namespaced(client.clone(), "team-b");
    // The configuration takes a moment to reach the apiserver's admission chain.
    let refusal = eventually(
        "the webhook refusing a vault outside its tenant",
        || async {
            team_b
                .create(&dry_run, &outsider)
                .await
                .err()
                .map(|e| e.to_string())
        },
    )
    .await;
    assert!(refusal.contains("VaultNamespaceNotAllowed"), "{refusal}");
    let second: KuraInstance = serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": "other" },
        "spec": { "url": "http://kura:8080", "auth": { "serviceAccountToken": { "audience": "kura" } } }
    }))
    .expect("an instance");
    let twice = instances
        .create(&dry_run, &second)
        .await
        .expect_err("one KuraInstance per cluster")
        .to_string();
    assert!(twice.contains("one KuraInstance per cluster"), "{twice}");
    let listed: KuraVaultResource = serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault", "metadata": { "name": "notes", "namespace": "family" },
        "spec": { "tenantRef": { "name": "family" } }
    }))
    .expect("a vault");
    vaults
        .create(&dry_run, &listed)
        .await
        .expect("a vault in a listed namespace is admitted");

    // SIGTERM: the Lease released, the webhook drained, exit 0.
    let pid = operator.id().expect("running");
    let sent = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .expect("kill ran");
    assert!(sent.success());
    let status = tokio::time::timeout(TIMEOUT, operator.wait())
        .await
        .expect("exits within the timeout")
        .expect("an exit status");
    assert!(
        status.success(),
        "{status}; log: {}",
        std::fs::read_to_string(dir.join("operator.log")).unwrap_or_default()
    );
}
