//! The HTTP API end to end: real tokens, real PostgreSQL, the reconciled
//! configuration (RFC 0001 T2, devices; admin status). Opt-in like tests/db.rs.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{env, problem, vault_id};
use kura_server::app::app;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn revoked_device_is_refused() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let phone_tok = e.key.token("u1", "s-phone", &["maxime-members"]);
    let (laptop, phone) = (
        e.register(&tok, "laptop").await,
        e.register(&phone_tok, "phone").await,
    );
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{phone}"),
            &tok,
            Some(&laptop),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, b) = e
        .call("GET", "/v1/devices", &phone_tok, Some(&phone), None)
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );
    let (s, b) = e
        .call("GET", "/v1/devices", &tok, Some(&laptop), None)
        .await;
    assert_eq!(
        s,
        StatusCode::OK,
        "another device of the same user still works"
    );
    let revoked: Vec<bool> = b
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["revoked"].as_bool().unwrap())
        .collect();
    assert_eq!(revoked, [false, true]);
}

#[tokio::test]
async fn revoked_sid_cannot_register() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let stolen = e.key.token("u1", "old-session", &["maxime-members"]);
    let old_tab = e.register(&stolen, "old-tab").await;
    let fresh = e.key.token("u1", "new-session", &["maxime-members"]);
    let laptop = e.register(&fresh, "laptop").await;
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{old_tab}"),
            &fresh,
            Some(&laptop),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    // The holder of the stolen refresh token gets a new access token in the same session.
    let (s, b) = e
        .call(
            "POST",
            "/v1/devices",
            &stolen,
            None,
            Some(json!({ "name": "evil", "profile": "mobile" })),
        )
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );
    e.register(&fresh, "second").await;
}

/// D34 with Authentik's refresh: a refreshed token carries no `sid` but keeps
/// the session's `auth_time`, which then stands for the session.
#[tokio::test]
async fn refreshed_token_without_sid_follows_its_session() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let g = &["maxime-members"];
    let signed_in = e.key.token_with("u1", Some("s-a"), Some(1000), g);
    let laptop = e.register(&signed_in, "laptop").await;
    let refreshed = e.key.token_with("u1", None, Some(1000), g);
    let (s, _) = e
        .call("GET", "/v1/devices", &refreshed, Some(&laptop), None)
        .await;
    assert_eq!(s, StatusCode::OK, "a refreshed token keeps working");
    let other = e.key.token_with("u1", Some("s-b"), Some(2000), g);
    let phone = e.register(&other, "phone").await;
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{laptop}"),
            &other,
            Some(&phone),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    // The revoked session, refreshed, is refused on every device and cannot register.
    let (s, b) = e
        .call("GET", "/v1/devices", &refreshed, Some(&phone), None)
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );
    let (s, b) = e
        .call(
            "POST",
            "/v1/devices",
            &refreshed,
            None,
            Some(json!({ "name": "evil", "profile": "mobile" })),
        )
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );
    // The other session, refreshed, still works; a token naming no session does not.
    let other_refreshed = e.key.token_with("u1", None, Some(2000), g);
    let (s, _) = e
        .call("GET", "/v1/devices", &other_refreshed, Some(&phone), None)
        .await;
    assert_eq!(s, StatusCode::OK);
    let anonymous = e.key.token_with("u1", None, None, g);
    let (s, b) = e
        .call("GET", "/v1/devices", &anonymous, Some(&phone), None)
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/invalid_token")
    );
    // Another user's session that signed in at the same second is not affected.
    let u2 = e.key.token_with("u2", Some("s-c"), Some(1000), g);
    e.register(&u2, "u2-laptop").await;
}

#[tokio::test]
async fn device_ownership_and_limits() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let u1 = e.key.token("u1", "s1", &["maxime-members"]);
    let u2 = e.key.token("u2", "s2", &["maxime-members"]);
    let d1 = e.register(&u1, "laptop").await;
    let d2 = e.register(&u2, "laptop").await;
    // Another subject's device is unknown to me, as header and as target.
    let (s, b) = e.call("GET", "/v1/devices", &u2, Some(&d1), None).await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_unknown")
    );
    let (s, b) = e
        .call("DELETE", &format!("/v1/devices/{d1}"), &u2, Some(&d2), None)
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::NOT_FOUND, "/v1/problems/not_found")
    );
    let (s, b) = e
        .call("GET", "/v1/devices", &u1, Some("not-a-uuid"), None)
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_unknown")
    );
    // Not a member of any tenant.
    let stranger = e.key.token("u3", "s3", &["nobody"]);
    let (s, _) = e
        .call(
            "POST",
            "/v1/devices",
            &stranger,
            None,
            Some(json!({ "name": "x", "profile": "desktop" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // A bad body is a problem document too.
    let (s, b) = e
        .call(
            "POST",
            "/v1/devices",
            &u1,
            None,
            Some(json!({ "name": "x", "profile": "watch" })),
        )
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request")
    );
    let (s, _) = e
        .call(
            "POST",
            "/v1/devices",
            &u1,
            None,
            Some(json!({ "name": "", "profile": "desktop" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // At most 20 live devices (inserted directly: registration is limited to 10 a minute).
    sqlx::query(
        "INSERT INTO device (user_sub, session_sid, name, profile)
         SELECT 'u1', 's1', 'd' || i, 'desktop' FROM generate_series(1, 19) i",
    )
    .execute(&e.t.owner)
    .await
    .unwrap();
    let (s, b) = e
        .call(
            "POST",
            "/v1/devices",
            &u1,
            None,
            Some(json!({ "name": "21st", "profile": "desktop" })),
        )
        .await;
    assert_eq!(
        (s, problem(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/quota_exceeded"
        )
    );
}

#[tokio::test]
async fn a_tenant_admin_sees_and_revokes_the_tenants_devices_only() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let member = e.key.token("u1", "s1", &["maxime-members"]);
    let admin = e.key.token("a1", "sa", &["maxime-admins"]);
    let foreign = e.key.token("o1", "so", &["other-members"]);
    let dm = e.register(&member, "laptop").await;
    let da = e.register(&admin, "admin-laptop").await;
    let df = e.register(&foreign, "other-laptop").await;
    let (_, b) = e.call("GET", "/v1/devices", &admin, Some(&da), None).await;
    let owners: Vec<&str> = b
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["owner"].as_str().unwrap_or("me"))
        .collect();
    assert_eq!(
        owners,
        ["me", "u1"],
        "the other tenant's device is not listed: {b}"
    );
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{df}"),
            &admin,
            Some(&da),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{dm}"),
            &admin,
            Some(&da),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = e.call("GET", "/v1/devices", &member, Some(&dm), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_status_is_for_platform_admins() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let member = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&member, "laptop").await;
    let (s, _) = e
        .call("GET", "/v1/admin/status", &member, Some(&d), None)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let root = e
        .key
        .token("root", "sr", &["maxime-owners", "kura-platform-admins"]);
    let dr = e.register(&root, "console").await;
    let (s, b) = e
        .call("GET", "/v1/admin/status", &root, Some(&dr), None)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["objects"].as_array().unwrap().len(), 4);
    assert_eq!(b["objects"][0]["conditions"][0]["reason"], "Reconciled");
    assert_eq!(b["generation"].as_str().unwrap().len(), 64);
}

#[tokio::test]
async fn idle_devices_are_purged_but_revoked_sessions_are_remembered() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let idle = e.register(&tok, "idle").await;
    let old = e.register(&tok, "old").await;
    let laptop = e.register(&tok, "laptop").await;
    e.call(
        "DELETE",
        &format!("/v1/devices/{old}"),
        &tok,
        Some(&laptop),
        None,
    )
    .await;
    sqlx::query(
        "UPDATE device SET last_seen = now() - interval '181 days' WHERE id::text = ANY($1)",
    )
    .bind(vec![idle.clone(), old.clone()])
    .execute(&e.t.owner)
    .await
    .unwrap();
    let purged: i64 = sqlx::query_scalar("SELECT kura_device_purge()")
        .fetch_one(&e.t.app)
        .await
        .unwrap();
    assert_eq!(purged, 1);
    let (s, b) = e.call("GET", "/v1/devices", &tok, Some(&idle), None).await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_unknown"),
        "re-register silently"
    );
    let (s, b) = e.call("GET", "/v1/devices", &tok, Some(&old), None).await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );
}

#[tokio::test]
async fn foreign_vault_is_404() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let member = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&member, "laptop").await;
    let (s, b) = e.call("GET", "/v1/vaults", &member, Some(&d), None).await;
    assert_eq!(s, StatusCode::OK);
    let names: Vec<(&str, &str)> = b
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v["name"].as_str().unwrap(), v["role"].as_str().unwrap()))
        .collect();
    assert_eq!(
        names,
        [("knowledge", "editor")],
        "only the vault a binding opens"
    );

    let foreign = vault_id(&e, "secrets").await;
    let missing = uuid::Uuid::from_u128(42).to_string();
    let put = Some(json!({ "role": "viewer" }));
    let a = e
        .call(
            "PUT",
            &format!("/v1/vaults/{foreign}/bindings/user:u1"),
            &member,
            Some(&d),
            put.clone(),
        )
        .await;
    let m = e
        .call(
            "PUT",
            &format!("/v1/vaults/{missing}/bindings/user:u1"),
            &member,
            Some(&d),
            put,
        )
        .await;
    assert_eq!(a.0, StatusCode::NOT_FOUND);
    assert_eq!(a, m, "byte-identical to a vault that does not exist");
    let a = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{foreign}"),
            &member,
            Some(&d),
            None,
        )
        .await;
    let m = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{missing}"),
            &member,
            Some(&d),
            None,
        )
        .await;
    assert_eq!((a.0, a.1.clone()), (m.0, m.1));
    assert_eq!(a.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn manager_cannot_bind_foreign_group() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let admin = e.key.token("a1", "sa", &["maxime-admins"]);
    let da = e.register(&admin, "admin").await;
    let manager = e.key.token("m1", "sm", &["maxime-members"]);
    let dm = e.register(&manager, "manager").await;
    let v = vault_id(&e, "knowledge").await;
    let (s, b) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/user:m1"),
            &admin,
            Some(&da),
            Some(json!({ "role": "manager" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    // A group of another tenant, an unknown subject: refused.
    let (s, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/claim:other-members"),
            &manager,
            Some(&dm),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/user:stranger"),
            &manager,
            Some(&dm),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // The tenant's own group is fine; a file-declared binding is read-only.
    let (s, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/claim:maxime-admins"),
            &manager,
            Some(&dm),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let (s, b) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/claim:maxime-members"),
            &manager,
            Some(&dm),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "{b}");
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{v}/bindings/claim:maxime-members"),
            &manager,
            Some(&dm),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{v}/bindings/claim:maxime-admins"),
            &manager,
            Some(&dm),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/nonsense"),
            &manager,
            Some(&dm),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn manager_cannot_grant_above_own_role() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    // An editor (through the file binding) is below manager: it binds nothing.
    let editor = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&editor, "laptop").await;
    let other = e.key.token("u2", "s2", &["maxime-members"]);
    e.register(&other, "laptop").await;
    let v = vault_id(&e, "knowledge").await;
    for role in ["manager", "editor", "viewer"] {
        let (s, b) = e
            .call(
                "PUT",
                &format!("/v1/vaults/{v}/bindings/user:u2"),
                &editor,
                Some(&d),
                Some(json!({ "role": role })),
            )
            .await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role}: {b}");
    }
    let denied: i64 = e
        .state
        .metrics
        .authz_denied_total
        .with_label_values(&["insufficient_role"])
        .get()
        .try_into()
        .unwrap();
    assert_eq!(denied, 3);
}

#[tokio::test]
async fn tenant_admins_manage_api_vaults_only() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let admin = e.key.token("a1", "sa", &["maxime-admins"]);
    let da = e.register(&admin, "admin").await;
    let member = e.key.token("u1", "s1", &["maxime-members"]);
    let dm = e.register(&member, "laptop").await;
    let (s, b) = e
        .call(
            "POST",
            "/v1/vaults",
            &admin,
            Some(&da),
            Some(json!({ "tenant": "maxime", "name": "drafts" })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(
        (
            b["namespace"].as_str(),
            b["managedBy"].as_str(),
            b["role"].as_str()
        ),
        (Some("maxime"), Some("api"), Some("manager"))
    );
    let drafts = b["id"].as_str().unwrap().to_owned();
    let (s, _) = e
        .call(
            "POST",
            "/v1/vaults",
            &admin,
            Some(&da),
            Some(json!({ "tenant": "maxime", "name": "drafts" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "name taken");
    let (s, _) = e
        .call(
            "POST",
            "/v1/vaults",
            &admin,
            Some(&da),
            Some(json!({ "tenant": "other", "name": "x" })),
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "foreign tenant");
    let (s, _) = e
        .call(
            "POST",
            "/v1/vaults",
            &member,
            Some(&dm),
            Some(json!({ "tenant": "maxime", "name": "x" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = e
        .call(
            "POST",
            "/v1/vaults",
            &admin,
            Some(&da),
            Some(json!({ "tenant": "maxime", "name": "Bad Name" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let knowledge = vault_id(&e, "knowledge").await;
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{knowledge}"),
            &admin,
            Some(&da),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "declared in configuration");
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{drafts}"),
            &admin,
            Some(&da),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/vaults/{drafts}"),
            &admin,
            Some(&da),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND, "disabled vaults are gone");
}

#[tokio::test]
async fn rate_limits_answer_429_with_retry_after() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&tok, "laptop").await;
    for _ in 0..60 {
        let (s, _) = e.call("GET", "/v1/vaults", &tok, Some(&d), None).await;
        assert_eq!(s, StatusCode::OK);
    }
    let req = Request::get("/v1/vaults")
        .header("authorization", format!("Bearer {tok}"))
        .header("x-kura-device", &d);
    let r = app(e.state.clone())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::TOO_MANY_REQUESTS);
    let wait: u64 = r.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((1..=60).contains(&wait));
    // Another route keeps its own budget.
    let (s, _) = e.call("GET", "/v1/devices", &tok, Some(&d), None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn cors_allows_the_obsidian_origins_only_and_bodies_are_bounded() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let preflight = |origin: &str| {
        Request::builder()
            .method("OPTIONS")
            .uri("/v1/vaults")
            .header("origin", origin)
            .header("access-control-request-method", "GET")
            .header(
                "access-control-request-headers",
                "authorization,x-kura-device",
            )
            .body(Body::empty())
            .unwrap()
    };
    let r = app(e.state.clone())
        .oneshot(preflight("app://obsidian.md"))
        .await
        .unwrap();
    assert_eq!(
        r.headers()["access-control-allow-origin"],
        "app://obsidian.md"
    );
    assert!(
        r.headers()
            .get("access-control-allow-credentials")
            .is_none()
    );
    let r = app(e.state.clone())
        .oneshot(preflight("https://evil.example.org"))
        .await
        .unwrap();
    assert!(r.headers().get("access-control-allow-origin").is_none());

    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let big = json!({ "name": "x".repeat(70_000), "profile": "desktop" });
    let (s, b) = e.call("POST", "/v1/devices", &tok, None, Some(big)).await;
    assert_eq!(
        (s, problem(&b)),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "/v1/problems/payload_too_large"
        )
    );
}

/// Gate: every route of the spec has a rate limit in `limits::LIMITS`.
#[test]
fn every_route_has_a_limit() {
    let spec = serde_json::to_value(kura_server::app::openapi_spec()).unwrap();
    let mut missing = Vec::new();
    for (path, item) in spec["paths"].as_object().unwrap() {
        if !path.starts_with("/v1/") {
            continue;
        }
        for method in item.as_object().unwrap().keys() {
            let m = method.to_uppercase();
            if kura_server::limits::limit_of(&m, path).is_none() {
                missing.push(format!("{m} {path}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "routes without a rate limit: {missing:?}"
    );
}

/// T2: once a device is revoked, its session cannot borrow another device id
/// of the same user, even with fresh access tokens from that session.
#[tokio::test]
async fn revoked_session_cannot_use_another_device() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let stolen = e.key.token("u1", "stolen-session", &["maxime-members"]);
    let phone = e.register(&stolen, "phone").await;
    let laptop_session = e.key.token("u1", "laptop-session", &["maxime-members"]);
    let laptop = e.register(&laptop_session, "laptop").await;
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{phone}"),
            &laptop_session,
            Some(&laptop),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, b) = e
        .call("GET", "/v1/devices", &stolen, Some(&laptop), None)
        .await;
    assert_eq!(
        (s, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/device_revoked")
    );
    let (s, _) = e
        .call("GET", "/v1/devices", &laptop_session, Some(&laptop), None)
        .await;
    assert_eq!(s, StatusCode::OK, "the laptop's own session is unaffected");
}

/// Device links follow memberships: a user who left a tenant drops out of
/// its admin's view, and a revoked device does not make a "known member".
#[tokio::test]
async fn device_links_follow_current_memberships() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let admin = e.key.token("a1", "sa", &["maxime-admins"]);
    let da = e.register(&admin, "admin").await;
    let member = e
        .key
        .token("u1", "s1", &["maxime-members", "other-members"]);
    let dm = e.register(&member, "laptop").await;
    // u1 leaves maxime; their next request (a minute later) refreshes the links.
    sqlx::query("UPDATE device SET last_seen = now() - interval '2 minutes'")
        .execute(&e.t.owner)
        .await
        .unwrap();
    let left = e.key.token("u1", "s1", &["other-members"]);
    let (s, _) = e.call("GET", "/v1/devices", &left, Some(&dm), None).await;
    assert_eq!(s, StatusCode::OK);
    let (_, b) = e.call("GET", "/v1/devices", &admin, Some(&da), None).await;
    assert_eq!(
        b.as_array().unwrap().len(),
        1,
        "only the admin's own device: {b}"
    );
    let (s, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{dm}"),
            &admin,
            Some(&da),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let v = vault_id(&e, "knowledge").await;
    let (s, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/bindings/user:u1"),
            &admin,
            Some(&da),
            Some(json!({ "role": "viewer" })),
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "no longer a known member");
}

/// A quota lowered below what is stored shows as an advisory condition (§4.3).
#[tokio::test]
async fn quota_below_usage_is_reported() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let tenant: uuid::Uuid = sqlx::query_scalar("SELECT id FROM tenant WHERE slug = 'maxime'")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    let v: uuid::Uuid = vault_id(&e, "knowledge").await.parse().unwrap();
    // 2 MiB stored against the 1 MiB quota of the test configuration.
    sqlx::query(
        "INSERT INTO chunk (vault_id, tenant_id, hash, size, object_key)
         SELECT $1, $2, decode(lpad(to_hex(i), 64, '0'), 'hex'), 1048576, 'k' || i FROM generate_series(1, 2) i",
    )
    .bind(v)
    .bind(tenant)
    .execute(&e.t.owner)
    .await
    .unwrap();
    let root_member = e
        .key
        .token("root", "sr", &["maxime-owners", "kura-platform-admins"]);
    let dr = e.register(&root_member, "console").await;
    let (s, b) = e
        .call("GET", "/v1/admin/status", &root_member, Some(&dr), None)
        .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    let maxime = b["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["name"] == "maxime")
        .unwrap();
    assert_eq!(
        maxime["conditions"][1]["type"], "QuotaBelowUsage",
        "{maxime}"
    );
}
