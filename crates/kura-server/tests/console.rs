//! RFC 0003 phase 1, the server's half: `GET /me`, the bindings list, the
//! `web` device (one per Authentik session, signed out without a ban, never
//! the configuration space, purged after 30 days) and the console's files.
//! Opt-in: KURA_TEST_DATABASE_URL.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{Env, env, problem, vault_id};
use kura_server::app::app;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn web(e: &Env, token: &str) -> (StatusCode, Value) {
    e.call(
        "POST",
        "/v1/devices",
        token,
        None,
        Some(json!({ "name": "Firefox on Linux", "profile": "web" })),
    )
    .await
}

/// T11, `me_shows_only_own_tenants`: the caller's own tenants and roles,
/// before any device; nothing of another subject.
#[tokio::test]
async fn me_shows_only_own_tenants() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let admin = e
        .key
        .token("boss", "s1", &["maxime-admins", "kura-platform-admins"]);
    let (st, me) = e.call("GET", "/v1/me", &admin, None, None).await;
    assert_eq!(st, StatusCode::OK, "{me}");
    assert_eq!(me["subject"], "boss");
    assert_eq!(
        me["tenants"],
        json!([{ "slug": "maxime", "role": "admin" }])
    );
    assert_eq!(me["platformAdmin"], true);
    let other = e.key.token("u9", "s9", &["other-members"]);
    let (_, me) = e.call("GET", "/v1/me", &other, None, None).await;
    assert_eq!(
        me["tenants"],
        json!([{ "slug": "other", "role": "member" }])
    );
    assert_eq!(me["platformAdmin"], false);
    let nobody = e.key.token("u0", "s0", &[]);
    let (st, me) = e.call("GET", "/v1/me", &nobody, None, None).await;
    assert_eq!((st, &me["tenants"]), (StatusCode::OK, &json!([])));
    let (st, _) = e.call("GET", "/v1/me", "not-a-token", None, None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

/// T11, `bindings_need_manager` and `foreign_tenant_is_404`.
#[tokio::test]
async fn bindings_need_manager() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let admin = e.key.token("boss", "s1", &["maxime-admins"]);
    let ad = e.register(&admin, "desk").await;
    let (st, b) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/claim:maxime-owners"),
            &admin,
            Some(&ad),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let (st, list) = e
        .call(
            "GET",
            &format!("/v1/vaults/{v}/bindings"),
            &admin,
            Some(&ad),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{list}");
    assert_eq!(
        list,
        json!([
            { "subject": "claim:maxime-members", "role": "editor", "managedBy": "config", "widened": false },
            { "subject": "claim:maxime-owners", "role": "viewer", "managedBy": "api", "widened": false },
        ])
    );
    let member = e.key.token("u1", "s2", &["maxime-members"]);
    let md = e.register(&member, "laptop").await;
    let (st, b) = e
        .call(
            "GET",
            &format!("/v1/vaults/{v}/bindings"),
            &member,
            Some(&md),
            None,
        )
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role")
    );
    let secrets = vault_id(&e, "secrets").await;
    let (st, _) = e
        .call(
            "GET",
            &format!("/v1/vaults/{secrets}/bindings"),
            &admin,
            Some(&ad),
            None,
        )
        .await;
    assert_eq!(
        st,
        StatusCode::NOT_FOUND,
        "another tenant's vault looks missing"
    );
}

/// T12 and decision 24: one web device per Authentik session; signing out
/// refuses its id as unknown without banning the session, so signing in
/// again works and the session's other devices go on; a revoked one still
/// bans the session.
#[tokio::test]
async fn web_device_is_reused_per_sid() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let token = e.key.token("u1", "browser-session", &["maxime-members"]);
    let (st, first) = web(&e, &token).await;
    assert_eq!(st, StatusCode::CREATED, "{first}");
    assert_eq!(first["profile"], "web");
    let (st, again) = web(&e, &token).await;
    assert_eq!(
        (st, &again["id"]),
        (StatusCode::OK, &first["id"]),
        "same sid, same device"
    );
    let d = first["id"].as_str().unwrap().to_owned();
    // The CLI approved in the same Authentik session.
    let cli = e.register(&token, "laptop").await;

    // `sign_out_clears_storage_and_closes_the_device`, the server's half.
    let (st, _) = e
        .call(
            "POST",
            &format!("/v1/devices/{d}/sign-out"),
            &token,
            None,
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, b) = e.call("GET", "/v1/vaults", &token, Some(&d), None).await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_unknown")
    );
    let active: Option<bool> = sqlx::query_scalar("SELECT kura_device_active($1::uuid)")
        .bind(&d)
        .fetch_one(&e.t.app)
        .await
        .unwrap();
    assert_eq!(active, Some(false), "its sockets close");
    let (_, listed) = e.call("GET", "/v1/devices", &token, Some(&cli), None).await;
    assert!(
        listed
            .as_array()
            .unwrap()
            .iter()
            .all(|x| x["id"] != json!(d)),
        "not listed: {listed}"
    );
    let (st, _) = e.call("GET", "/v1/vaults", &token, Some(&cli), None).await;
    assert_eq!(st, StatusCode::OK, "the session is not banned");
    // `sign_in_works_after_sign_out_in_the_same_session`: a new device.
    let (st, next) = web(&e, &token).await;
    assert_eq!(st, StatusCode::CREATED, "{next}");
    assert_ne!(next["id"], first["id"]);

    // Only one's own web device signs out.
    let other = e.key.token("u2", "other-session", &["maxime-members"]);
    let next_id = next["id"].as_str().unwrap();
    let (st, _) = e
        .call(
            "POST",
            &format!("/v1/devices/{next_id}/sign-out"),
            &other,
            None,
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = e
        .call(
            "POST",
            &format!("/v1/devices/{cli}/sign-out"),
            &token,
            None,
            None,
        )
        .await;
    assert_eq!(
        st,
        StatusCode::NOT_FOUND,
        "a desktop device is revoked, not signed out"
    );

    // A revocation is still the hard cut of RFC 0001 decision 35.
    let (st, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{next_id}"),
            &token,
            Some(&cli),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, b) = web(&e, &token).await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );

    // Web devices idle 30 days are purged; others stay to 180.
    let fresh = e.key.token("u3", "third-session", &["maxime-members"]);
    let (_, w) = web(&e, &fresh).await;
    let desk = e.register(&fresh, "desk").await;
    sqlx::query("UPDATE device SET last_seen = now() - interval '31 days' WHERE user_sub = 'u3'")
        .execute(&e.t.owner)
        .await
        .unwrap();
    sqlx::query_scalar::<_, i64>("SELECT kura_device_purge()")
        .fetch_one(&e.t.app)
        .await
        .unwrap();
    let left: Vec<String> = sqlx::query_scalar("SELECT id::text FROM device WHERE user_sub = 'u3'")
        .fetch_all(&e.t.owner)
        .await
        .unwrap();
    assert_eq!(
        left,
        [desk],
        "the web device {} went, the desktop stayed",
        w["id"]
    );
}

/// §6.2: the console never reads or writes an Obsidian configuration.
#[tokio::test]
async fn a_web_device_never_gets_the_configuration_space() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let docs = common::app::CONFIG.replace(
        "  tenantRef: { name: maxime }\n  bindings:",
        "  tenantRef: { name: maxime }\n  configSync: { categories: [snippets] }\n  bindings:",
    );
    let dir = std::env::temp_dir().join(format!("kura-http-{}", e.t.name));
    std::fs::write(dir.join("config.yaml"), docs).unwrap();
    let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
    kura_server::reconcile::reconcile(&e.t.app, &files, true)
        .await
        .unwrap();
    e.state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    let v = vault_id(&e, "knowledge").await;
    let token = e.key.token("u1", "s1", &["maxime-members"]);
    let (_, w) = web(&e, &token).await;
    let w = w["id"].as_str().unwrap();
    let desk = e.register(&token, "desk").await;
    let uri = format!("/v1/vaults/{v}/changes?since=0&space=config");
    let (st, b) = e.call("GET", &uri, &token, Some(w), None).await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role")
    );
    let (st, _) = e.call("GET", &uri, &token, Some(&desk), None).await;
    assert_eq!(st, StatusCode::OK);
}

/// T10 and §6.1: `app_sets_frame_ancestors_none`, `kura_sets_no_cookie`,
/// the shell for client routes, `config.json`, and nothing when it is off.
#[tokio::test]
async fn the_console_is_served_with_its_headers() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("kura-ui-{}", e.t.name));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><title>Kura</title>").unwrap();
    std::fs::write(dir.join("assets/app-1a2b.js"), "export {}").unwrap();
    let get = |state: std::sync::Arc<kura_server::app::AppState>, uri: &'static str| async move {
        let r = app(state)
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let (parts, body) = r.into_parts();
        let body = axum::body::to_bytes(body, 1 << 20).await.unwrap();
        (
            parts.status,
            parts.headers,
            String::from_utf8_lossy(&body).into_owned(),
        )
    };
    // Off: nothing under /app, and `/` is not redirected.
    let (st, _, _) = get(e.state.clone(), "/app/").await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    let ui = kura_server::app::ui::UiSettings::new(
        dir.clone(),
        "kura-web".into(),
        "https://auth.example.org/".into(),
        Some("https://auth.example.org/application/o/kura-web/".into()),
    )
    .unwrap();
    let state = kura_server::app::AppState::new(
        kura_server::metrics::Metrics::new().unwrap(),
        e.t.app.clone(),
        e.key.verifier(),
    )
    .with_ui(ui);
    let state = std::sync::Arc::new(state);

    let (st, h, body) = get(state.clone(), "/app/vaults/123").await;
    assert_eq!(st, StatusCode::OK, "a client route gets the shell");
    assert!(body.contains("<title>Kura</title>"));
    let csp = h["content-security-policy"].to_str().unwrap();
    assert!(
        csp.contains("frame-ancestors 'none'") && csp.contains("script-src 'self';"),
        "{csp}"
    );
    assert_eq!(h["referrer-policy"], "no-referrer");
    assert_eq!(h["x-content-type-options"], "nosniff");
    assert_eq!(h["cache-control"], "no-cache");
    assert!(h.get("set-cookie").is_none(), "kura_sets_no_cookie");
    let (st, h, _) = get(state.clone(), "/app/assets/app-1a2b.js").await;
    assert_eq!(st, StatusCode::OK);
    assert!(h["cache-control"].to_str().unwrap().contains("immutable"));
    let (st, _, body) = get(state.clone(), "/app/assets/gone-9z9z.js").await;
    assert_eq!(st, StatusCode::NOT_FOUND, "a stale asset is not the shell");
    assert!(!body.contains("<title>"));
    let (st, h, body) = get(state.clone(), "/app/config.json").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap(),
        json!({
            "issuer": "https://auth.example.org/",
            "discovery": "https://auth.example.org/application/o/kura-web/",
            "clientId": "kura-web",
        })
    );
    assert!(h.get("content-security-policy").is_some());
    let (st, h, _) = get(state, "/").await;
    assert!(st.is_redirection());
    assert_eq!(h["location"], "/app/");
}
