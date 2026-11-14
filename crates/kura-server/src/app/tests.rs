use super::*;
use axum::body::Body;
use axum::http::StatusCode;
use tower::ServiceExt;

fn state() -> SharedState {
    Arc::new(AppState::new(
        Metrics::new().unwrap(),
        lazy_pool(),
        crate::auth::tests::verifier(&[], serde_json::json!({})),
    ))
}

pub fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://nobody@127.0.0.1:1/none")
        .unwrap()
}

async fn get(app: axum::Router, uri: &str) -> Response {
    app.oneshot(axum::http::Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
async fn healthz_always_answers_and_readyz_follows_the_config() {
    let s = state();
    assert_eq!(
        get(app(s.clone()), "/healthz").await.status(),
        StatusCode::OK
    );
    let r = get(app(s.clone()), "/readyz").await;
    assert_eq!(
        r.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "unready until a configuration is loaded"
    );
    assert_eq!(
        r.headers()[header::CONTENT_TYPE],
        "application/problem+json"
    );
    s.set_ready(true);
    assert_eq!(get(app(s), "/readyz").await.status(), StatusCode::OK);
}

#[tokio::test]
async fn scalar_is_served_with_its_own_csp() {
    let r = get(app(state()), "/scalar").await;
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(
        r.headers()[header::CONTENT_SECURITY_POLICY],
        crate::scalar::API_DOCS_CSP
    );
    let r = get(app(state()), "/healthz").await;
    assert!(
        r.headers().get(header::CONTENT_SECURITY_POLICY).is_none(),
        "only /scalar"
    );
}

#[test]
fn known_clients_below_their_minimum_are_too_old() {
    assert!(client_too_old("kura-cli/0.0.9"));
    assert!(client_too_old("kura-plugin/0.0.1"));
    assert!(
        client_too_old("kura-cli/garbage"),
        "a known client with an unreadable version"
    );
    assert!(client_too_old("kura-cli/1.2"));
    assert!(client_too_old("kura-cli/1.2.3.4"));
    assert!(!client_too_old("kura-cli/0.1.0"));
    assert!(!client_too_old("kura-cli/0.1.0-rc.1"));
    assert!(!client_too_old("kura-plugin/2.0.0+build.5"));
    assert!(
        !client_too_old("curl/8.0.0"),
        "unknown clients are not gated"
    );
    assert!(!client_too_old("no-slash"));
}

mod admin_status {
    use super::*;
    use crate::auth::tests::{TestKey, access_claims, verifier};

    fn state_with(key: &TestKey) -> SharedState {
        Arc::new(AppState::new(
            Metrics::new().unwrap(),
            lazy_pool(),
            verifier(&[key], serde_json::json!({})),
        ))
    }

    async fn call(state: SharedState, token: Option<String>) -> Response {
        let mut req = axum::http::Request::get("/v1/admin/status");
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        app(state)
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn requires_a_token_and_the_platform_admin_claim() {
        let k = TestKey::new("k1");
        let r = call(state_with(&k), None).await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        let r = call(state_with(&k), Some("garbage".into())).await;
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        let r = call(state_with(&k), Some(k.sign(&access_claims("u1")))).await;
        assert_eq!(
            r.status(),
            StatusCode::UNAUTHORIZED,
            "no X-Kura-Device: device_unknown"
        );
        let body = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("device_unknown"));
    }

    #[tokio::test]
    async fn a_client_below_its_minimum_is_told_to_upgrade() {
        let k = TestKey::new("k1");
        let req =
            axum::http::Request::get("/v1/admin/status").header(CLIENT_HEADER, "kura-cli/0.0.1");
        let r = app(state_with(&k))
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UPGRADE_REQUIRED);
    }
}
