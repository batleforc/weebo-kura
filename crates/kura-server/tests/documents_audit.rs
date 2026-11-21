//! T7 (RFC 0009 §7.1): every push is one audit line naming the operator's
//! subject, never a token. Its own binary: a tracing subscriber set per test
//! races with the tests beside it.

mod common;

use axum::http::StatusCode;
use common::app::{capture_logs, env_custom};
use common::tokens::{OPERATOR, OperatorKey};
use serde_json::json;

#[tokio::test]
async fn every_push_is_an_audit_line() {
    let key = OperatorKey::new();
    let verifier = key.verifier();
    let Some(e) = env_custom(module_path!(), None, move |s| s.with_documents(verifier)).await
    else {
        return;
    };
    let (captured, _guard) = capture_logs();
    let token = key.operator();
    // The harness's tenant `other` and its vault left out: one removal of each.
    let set = json!([
        { "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "maxime" },
          "spec": { "vaultNamespaces": ["maxime"] } },
        { "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault", "metadata": { "name": "knowledge", "namespace": "maxime" },
          "spec": { "tenantRef": { "name": "maxime" } } },
    ]);
    let (s, b) = e
        .call(
            "PUT",
            "/v1/admin/documents",
            &token,
            None,
            Some(json!({ "documents": set })),
        )
        .await;
    assert_eq!(
        (s, b["outcome"].as_str()),
        (StatusCode::OK, Some("applied")),
        "{b}"
    );
    let bad = json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "solo" }, "spec": { "nope": 1 } });
    let (s, _) = e
        .call(
            "PUT",
            "/v1/admin/documents",
            &token,
            None,
            Some(json!({ "documents": [bad] })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    let applied = logs
        .lines()
        .find(|l| l.contains("documents_pushed"))
        .unwrap_or_else(|| panic!("{logs}"));
    assert!(
        applied.contains("audit") && applied.contains(OPERATOR),
        "{applied}"
    );
    assert!(applied.contains("removed=2"), "{applied}");
    let removed = logs
        .lines()
        .filter(|l| l.contains("configuration object removed"))
        .count();
    assert_eq!(removed, 2, "{logs}");
    let rejected = logs
        .lines()
        .find(|l| l.contains("documents_rejected"))
        .unwrap();
    assert!(rejected.contains(OPERATOR), "{rejected}");
    assert!(!logs.contains(&token), "the token never reaches a log");
}
