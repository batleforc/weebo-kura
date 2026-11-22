//! The operator's token for Kura (RFC 0009, plan D18): minted by a real
//! kube-apiserver through the TokenRequest API, for its own ServiceAccount,
//! with the audience the `KuraInstance` names, and sent as the bearer.

#![expect(clippy::expect_used, reason = "test helpers fail the test on purpose")]

use std::sync::{Arc, Mutex};

use axum::http::HeaderMap;
use axum::routing::put;
use base64::Engine;
use k8s_openapi::api::core::v1::ServiceAccount;
use kube::api::{Api, PostParams};
use kura_api::KuraInstance;
use kura_operator_adapters_outbound::HttpGatewayFactory;
use kura_operator_application::ports::GatewayFactory;
use serde_json::{Value, json};

/// The claims of a JWT, unverified: what kura-server would verify.
fn claims(token: &str) -> Value {
    let payload = token.split('.').nth(1).expect("a JWT");
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("base64url");
    serde_json::from_slice(&bytes).expect("JSON claims")
}

#[tokio::test]
async fn the_token_names_the_operator_and_the_instances_audience() {
    drop(rustls::crypto::ring::default_provider().install_default());
    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();
    let accounts: Api<ServiceAccount> = Api::namespaced(client.clone(), "default");
    let account: ServiceAccount =
        serde_json::from_value(json!({ "metadata": { "name": "kura-operator" } }))
            .expect("an account");
    accounts
        .create(&PostParams::default(), &account)
        .await
        .expect("created");

    // A stand-in kura-server recording the bearer it receives.
    let seen = Arc::new(Mutex::new(None::<String>));
    let record = seen.clone();
    let app = axum::Router::new().route(
        "/v1/admin/documents",
        put(move |h: HeaderMap| {
            let record = record.clone();
            async move {
                let bearer = h
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer "))
                    .map(str::to_owned);
                *record.lock().expect("lock") = bearer;
                axum::Json(json!({ "applied": true, "outcome": "applied", "objects": [] }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    let server = tokio::spawn(async move { drop(axum::serve(listener, app).await) });

    let instance: KuraInstance = serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": "kura" },
        "spec": { "url": url, "auth": { "serviceAccountToken": { "audience": "kura" } } }
    }))
    .expect("an instance");
    let factory = HttpGatewayFactory::new(client, "default".into(), "kura-operator".into());
    let gateway = factory.gateway_for(&instance).await.expect("a gateway");
    let answer = gateway.push(&[]).await.expect("pushed");
    assert_eq!(answer.outcome, "applied");
    let token = seen.lock().expect("lock").clone().expect("a bearer");
    let c = claims(&token);
    assert_eq!(c["sub"], "system:serviceaccount:default:kura-operator");
    assert_eq!(c["aud"], json!(["kura"]));

    // Another audience: another token, never the cached one.
    let mut other = instance.clone();
    other.spec.auth.service_account_token.audience = "kura-staging".into();
    factory
        .gateway_for(&other)
        .await
        .expect("a gateway")
        .push(&[])
        .await
        .expect("pushed");
    let token = seen.lock().expect("lock").clone().expect("a bearer");
    assert_eq!(claims(&token)["aud"], json!(["kura-staging"]));

    // An account that does not exist: no token, InstanceUnauthorized.
    let missing = HttpGatewayFactory::new(cluster.client(), "default".into(), "nobody".into());
    let refused = missing.gateway_for(&instance).await.err().expect("refused");
    assert_eq!(refused.reason, kura_api::ReasonCode::InstanceUnauthorized);
    server.abort();
}

/// `tls`: a CA Secret without the named key, or that does not exist, is
/// `InstanceUnreachable`; `insecureSkipVerify` still builds a gateway.
#[tokio::test]
async fn the_ca_secret_must_hold_the_named_key() {
    drop(rustls::crypto::ring::default_provider().install_default());
    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();
    let account: ServiceAccount =
        serde_json::from_value(json!({ "metadata": { "name": "kura-operator" } }))
            .expect("an account");
    Api::<ServiceAccount>::namespaced(client.clone(), "default")
        .create(&PostParams::default(), &account)
        .await
        .expect("created");
    let secret: k8s_openapi::api::core::v1::Secret = serde_json::from_value(
        json!({ "metadata": { "name": "kura-ca" }, "stringData": { "other": "x" } }),
    )
    .expect("a secret");
    Api::<k8s_openapi::api::core::v1::Secret>::namespaced(client.clone(), "default")
        .create(&PostParams::default(), &secret)
        .await
        .expect("created");
    let factory = HttpGatewayFactory::new(client, "default".into(), "kura-operator".into());
    let with_tls = |tls: Value| -> KuraInstance {
        serde_json::from_value(json!({
            "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": "kura" },
            "spec": { "url": "https://kura:8443", "auth": { "serviceAccountToken": { "audience": "kura" } }, "tls": tls }
        }))
        .expect("an instance")
    };
    for (secret, key) in [("kura-ca", "ca.crt"), ("missing", "ca.crt")] {
        let refused = factory
            .gateway_for(&with_tls(
                json!({ "caSecretRef": { "namespace": "default", "name": secret, "key": key } }),
            ))
            .await
            .err()
            .expect("refused");
        assert_eq!(refused.reason, kura_api::ReasonCode::InstanceUnreachable);
        assert!(
            refused.message.contains("tls.caSecretRef"),
            "{}",
            refused.message
        );
    }
    assert!(
        factory
            .gateway_for(&with_tls(json!({ "insecureSkipVerify": true })))
            .await
            .is_ok()
    );
}
