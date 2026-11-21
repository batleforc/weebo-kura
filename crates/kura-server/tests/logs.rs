//! What the logs say, in one test: a tracing subscriber set per test races
//! with the callsite caches of tests running beside it, so this binary holds
//! a single test.

mod common;

use axum::http::StatusCode;
use common::app::{capture_logs, env, vault_id};

/// T19 (logging half): logs carry the route template, never the path, which
/// holds vault ids and, later, note paths. T10 (denial half): every refusal is
/// one audit event and one `kura_authz_denied_total` count.
#[tokio::test]
async fn logs_name_routes_and_audit_every_denial() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let (captured, _guard) = capture_logs();
    let member = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&member, "laptop").await;
    let secret = vault_id(&e, "secrets").await;
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{secret}"),
            &member,
            Some(&d),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = e
        .call("GET", "/v1/admin/status", &member, Some(&d), None)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e.call("GET", "/v1/vaults", "forged", Some(&d), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // T19 on the sync routes: the cursor in `?since=` stays out of the logs.
    let knowledge = vault_id(&e, "knowledge").await;
    let (s, _) = e
        .call(
            "GET",
            &format!(
                "/v1/vaults/{knowledge}/changes?since=0&epoch=8f3e2b6a-91c4-4f7d-b2a5-7c1d9e4f6a83"
            ),
            &member,
            Some(&d),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::GONE);
    // Nor does a WebSocket ticket in `?ticket=`.
    let (_, _) = e
        .call(
            "GET",
            &format!("/v1/vaults/{knowledge}/events?ticket=Zq9ticketSecretValue"),
            &member,
            Some(&d),
            None,
        )
        .await;

    let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains(r#"route="/v1/vaults/{v}""#), "{logs}");
    assert!(
        !logs.contains(&secret),
        "the vault id leaked into the logs: {logs}"
    );
    assert!(!logs.contains(&member), "the token leaked");
    assert!(logs.contains(r#"route="/v1/vaults/{v}/changes""#), "{logs}");
    assert!(
        !logs.contains("8f3e2b6a"),
        "the query string leaked: {logs}"
    );
    assert!(
        !logs.contains("Zq9ticketSecretValue"),
        "the ticket leaked: {logs}"
    );
    assert!(!logs.contains(&knowledge), "the vault id leaked");
    let denials: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains(r#"event="authz_denied""#))
        .collect();
    assert_eq!(denials.len(), 3, "{logs}");
    for (line, problem) in denials
        .iter()
        .zip(["not_found", "insufficient_role", "invalid_token"])
    {
        assert!(line.contains(&format!(r#"problem="{problem}""#)), "{line}");
    }
    let m = &e.state.metrics.authz_denied_total;
    for problem in ["not_found", "insufficient_role", "invalid_token"] {
        assert_eq!(m.with_label_values(&[problem]).get(), 1, "{problem}");
    }
}
