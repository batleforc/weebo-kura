//! What the logs say about notification rules (RFC 0003 T4, T13, §4.3), in
//! one test: a tracing subscriber set per test races with the callsite
//! caches of tests running beside it, so this binary holds a single test.
//! Opt-in: KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR,
//! KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT.

mod common;

use axum::http::StatusCode;
use common::app::{Captured, env_custom, storage};
use kura_server::notifier::Notifier;
use serde_json::json;

/// T13 (`rule_changes_are_audited`): creating, changing, testing and
/// deleting a rule each leave one audit line with the rule's id. T4
/// (`logs_have_no_webhook_url`): no line holds the URL's path, the signing
/// secret or the `authorization` value. §4.3: with notifications off, the
/// startup count of stored rules is one `WARN` line.
#[tokio::test]
async fn rule_changes_are_audited() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let allowed: Vec<ipnet::IpNet> = vec!["127.0.0.1/32".parse().unwrap()];
    let client = kura_server::net::webhook_client(allowed.clone(), &[]).unwrap();
    let Some(e) = env_custom(module_path!(), Some(st), move |s| {
        s.with_notifier(Notifier::new(client, allowed))
    })
    .await
    else {
        return;
    };
    let captured = Captured::default();
    let sink = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("debug,sqlx=off")
        .with_writer(move || sink.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let token = e.key.token("boss", "sa", &["maxime-admins"]);
    let device = e.register(&token, "desk").await;
    let call = |method: &'static str, path: String, body: Option<serde_json::Value>| {
        let (token, device) = (token.clone(), device.clone());
        let e = &e;
        async move { e.call(method, &path, &token, Some(&device), body).await }
    };
    // Nothing listens on the port: the test's delivery fails, which is fine.
    let url = "https://127.0.0.1:9/hook-zq8path";
    let authorization = "Bearer zq8authorization";
    let (s, created) = call(
        "POST",
        "/v1/tenants/maxime/notifications".into(),
        Some(json!({ "name": "hub", "events": ["token.created"],
            "channel": { "type": "webhook", "url": url, "authorization": authorization } })),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap().to_owned();
    let secret = created["secret"].as_str().unwrap().to_owned();
    let rule = format!("/v1/tenants/maxime/notifications/{id}");
    let (s, changed) = call(
        "PUT",
        rule.clone(),
        Some(json!({ "name": "hub2", "rotateSecret": true })),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{changed}");
    let rotated = changed["secret"].as_str().unwrap().to_owned();
    let (s, tested) = call("POST", format!("{rule}/test"), None).await;
    assert_eq!(s, StatusCode::OK, "{tested}");

    // §4.3: the count the server logs at startup with notifications off.
    assert_eq!(
        kura_server::notifier::warn_rules_while_off(&e.t.app)
            .await
            .unwrap(),
        1
    );
    let (s, _) = call("DELETE", rule, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(
        kura_server::notifier::warn_rules_while_off(&e.t.app)
            .await
            .unwrap(),
        0,
        "no rule, no warning"
    );

    let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for event in [
        "notification_rule_created",
        "notification_rule_changed",
        "notification_rule_tested",
        "notification_rule_deleted",
    ] {
        let lines: Vec<&str> = logs
            .lines()
            .filter(|l| l.contains(&format!(r#"event="{event}""#)))
            .collect();
        assert_eq!(lines.len(), 1, "{event}: {logs}");
        assert!(lines[0].contains(&format!("rule={id}")), "{}", lines[0]);
    }
    let warnings: Vec<&str> = logs
        .lines()
        .filter(|l| l.contains("WARN") && l.contains("notifications.enabled is false"))
        .collect();
    assert_eq!(warnings.len(), 1, "{logs}");
    assert!(warnings[0].contains("rules=1"), "{}", warnings[0]);
    for leaked in [
        "hook-zq8path",
        "zq8authorization",
        secret.trim_start_matches("whsec_"),
        rotated.trim_start_matches("whsec_"),
    ] {
        assert!(!logs.contains(leaked), "{leaked} in the logs: {logs}");
    }
}
