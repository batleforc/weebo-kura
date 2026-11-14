//! The `kura` commands over real HTTP against the application (RFC 0001
//! §4.5): login, attach, sync with a conflict, status, devices, revocation,
//! and the exit codes. Opt-in: PostgreSQL, Vault (or OpenBao) and S3, as
//! `tests/chunks.rs`.

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use common::app::{env_with, storage};
use kura_cli::commands::{self, Ctx, DeviceAction};
use kura_server::app::app;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kura-cli-{name}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&d));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn code(r: Result<u8, commands::Fail>) -> u8 {
    match r {
        Ok(c) => c,
        Err(commands::Fail(c, msg)) => {
            eprintln!("({c}) {msg}");
            c
        }
    }
}

fn files(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                stack.push(e.path());
            } else {
                let rel = e
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.push((rel, std::fs::read_to_string(e.path()).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn the_cli_syncs_two_devices_and_maps_exit_codes() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let router = app(e.state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });
    let a = Ctx {
        home: dir("home-a"),
        access_token: Some(e.key.token("u1", "sa", &["maxime-members"])),
    };
    let b = Ctx {
        home: dir("home-b"),
        access_token: Some(e.key.token("u2", "sb", &["maxime-members"])),
    };
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();

    // Before any login: sign in first (2).
    assert_eq!(code(commands::sync_cmd(&a, None, false).await), 2);
    for (ctx, name) in [(&a, "desk-a"), (&b, "phone-b")] {
        let r = commands::login(
            ctx,
            server.clone(),
            issuer.clone(),
            "kura-cli".into(),
            Some(name.into()),
        )
        .await;
        assert_eq!(code(r), 0);
    }
    let bad = commands::login(
        &a,
        server.clone(),
        issuer.clone(),
        "kura-cli".into(),
        Some("bad:name".into()),
    )
    .await;
    assert_eq!(code(bad), 3, "a device name must be a valid file name");

    let (fa, fb) = (dir("vault-a"), dir("vault-b"));
    std::fs::create_dir_all(fa.join("notes")).unwrap();
    std::fs::write(fa.join("notes/hello.md"), "# Hello\n").unwrap();
    std::fs::write(fa.join("plan.md"), "plan\n").unwrap();
    assert_eq!(
        code(commands::attach(&a, "knowledge", fa.clone(), true).await),
        0
    );
    assert_eq!(
        code(commands::attach(&b, "knowledge", fb.clone(), true).await),
        0
    );
    assert_eq!(files(&fa), files(&fb), "b downloaded a's files");
    assert_eq!(
        code(commands::attach(&a, "nope", dir("nope"), true).await),
        3
    );

    // The same paragraph edited on both: a conflict file, both texts kept.
    std::fs::write(fa.join("notes/hello.md"), "# Hello\nfrom a\n").unwrap();
    std::fs::write(fb.join("notes/hello.md"), "# Hello\nfrom b\n").unwrap();
    assert_eq!(
        code(commands::sync_cmd(&a, Some("knowledge"), false).await),
        0
    );
    assert_eq!(code(commands::sync_cmd(&b, None, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&a, None, false).await), 0);
    let (la, lb) = (files(&fa), files(&fb));
    assert_eq!(la, lb);
    assert_eq!(la.len(), 3, "{la:?}");
    assert!(
        la.iter().any(|(_, t)| t.contains("from a"))
            && la.iter().any(|(_, t)| t.contains("from b"))
    );

    // A name that cannot sync: the rest syncs, exit 4; a dry run sends nothing.
    std::fs::write(fa.join("Q3: budget.md"), "x").unwrap();
    assert_eq!(code(commands::sync_cmd(&a, None, true).await), 4);
    assert_eq!(code(commands::sync_cmd(&a, None, false).await), 4);
    assert_eq!(code(commands::sync_cmd(&a, Some("other"), false).await), 3);
    assert_eq!(code(commands::status(&a).await), 0);
    assert_eq!(code(commands::devices(&a, None).await), 0);

    // Revoking phone-b from its own owner's other session is refused by name
    // (u1 cannot see u2's devices); u2 revokes it, then it must sign in (2).
    let revoke = |name: &str| Some(DeviceAction::Revoke { name: name.into() });
    assert_eq!(code(commands::devices(&a, revoke("phone-b")).await), 3);
    assert_eq!(code(commands::devices(&b, revoke("phone-b")).await), 0);
    assert_eq!(code(commands::sync_cmd(&b, None, false).await), 2);
}

/// A fake identity provider: discovery, the device code grant (pending once),
/// and refresh with rotation; any other refresh token is `invalid_grant`.
async fn fake_issuer(access: String) -> String {
    use axum::extract::State;
    use axum::routing::{get, post};
    use axum::{Form, Json};
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let polls = Arc::new(Mutex::new(0u32));
    let discovery = json!({ "token_endpoint": format!("{base}/token"), "device_authorization_endpoint": format!("{base}/device") });
    let router = axum::Router::new()
        .route("/.well-known/openid-configuration", get(move || async move { Json(discovery) }))
        .route(
            "/device",
            post(|| async {
                Json(json!({ "device_code": "dc", "user_code": "WDJB-MJHT", "verification_uri": "https://auth/device",
                    "verification_uri_complete": "https://auth/device?code=WDJB-MJHT", "expires_in": 30, "interval": 1 }))
            }),
        )
        .route(
            "/token",
            post(move |State(polls): State<Arc<Mutex<u32>>>, Form(f): Form<HashMap<String, String>>| {
                let access = access.clone();
                async move {
                    let tokens = |r: &str| (axum::http::StatusCode::OK, Json(json!({ "access_token": access, "refresh_token": r })));
                    let error = |e: &str| (axum::http::StatusCode::BAD_REQUEST, Json(json!({ "error": e })));
                    match (f.get("grant_type").map(String::as_str), f.get("refresh_token").map(String::as_str)) {
                        (Some("urn:ietf:params:oauth:grant-type:device_code"), _) => {
                            let mut n = polls.lock().unwrap();
                            *n = n.saturating_add(1);
                            if *n == 1 { error("authorization_pending") } else { tokens("r1") }
                        }
                        (Some("refresh_token"), Some("r1")) => tokens("r2"),
                        _ => error("invalid_grant"),
                    }
                }
            }),
        )
        .with_state(polls);
    tokio::spawn(async move { axum::serve(listener, router).await });
    base
}

#[tokio::test]
async fn sign_in_by_device_code_and_refresh_with_rotation() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let router = app(e.state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });
    let issuer = fake_issuer(e.key.token("u1", "s1", &["maxime-members"])).await;
    let ctx = Ctx {
        home: dir("home-oidc"),
        access_token: None,
    };
    let r = commands::login(&ctx, server, issuer, "kura-cli".into(), Some("desk".into())).await;
    assert_eq!(code(r), 0);
    assert_eq!(
        kura_cli::config::refresh_token(&ctx.home)
            .unwrap()
            .as_deref(),
        Some("r1")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(ctx.home.join("credentials.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "the refresh token is owner-only");
    }
    // Each run refreshes; the rotated token is stored before use. (`status`
    // with no vault attached does not reach the server.)
    assert_eq!(code(commands::status(&ctx).await), 0);
    assert_eq!(code(commands::devices(&ctx, None).await), 0);
    assert_eq!(
        kura_cli::config::refresh_token(&ctx.home)
            .unwrap()
            .as_deref(),
        Some("r2")
    );
    // r2 is unknown to the provider: invalid_grant means sign in again (2).
    assert_eq!(code(commands::devices(&ctx, None).await), 2);
}

/// §6.P observation 1, in process: `kura watch` on b receives a's change
/// within 5 s through the socket (no timer), and pushes b's own edit once quiet.
#[tokio::test]
async fn watch_pulls_on_notification_and_pushes_when_quiet() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let stop = tokio_util::sync::CancellationToken::new();
    tokio::spawn(kura_server::notify::listen(
        e.state.db.clone(),
        e.state.clone(),
        stop.clone(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let router = app(e.state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let a = Ctx {
        home: dir("watch-home-a"),
        access_token: Some(e.key.token("u1", "wa", &["maxime-members"])),
    };
    let b = Ctx {
        home: dir("watch-home-b"),
        access_token: Some(e.key.token("u2", "wb", &["maxime-members"])),
    };
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();
    let (fa, fb) = (dir("watch-a"), dir("watch-b"));
    for (ctx, name, folder) in [(&a, "wa", &fa), (&b, "wb", &fb)] {
        assert_eq!(
            code(
                commands::login(
                    ctx,
                    server.clone(),
                    issuer.clone(),
                    "kura-cli".into(),
                    Some(name.into())
                )
                .await
            ),
            0
        );
        assert_eq!(
            code(commands::attach(ctx, "knowledge", folder.clone(), true).await),
            0
        );
    }
    let watching = stop.clone();
    let b_home = b.home.clone();
    let b_token = b.access_token.clone();
    let watcher = tokio::spawn(async move {
        let b = Ctx {
            home: b_home,
            access_token: b_token,
        };
        commands::watch(&b, Some("knowledge"), watching.cancelled())
            .await
            .map_err(|f| f.1)
    });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    std::fs::write(fa.join("from-a.md"), "hello b\n").unwrap();
    let pushed = tokio::time::Instant::now();
    assert_eq!(code(commands::sync_cmd(&a, None, false).await), 0);
    let arrived = async {
        while !fb.join("from-a.md").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), arrived)
        .await
        .expect("a's change reached b within 5 s");
    assert!(pushed.elapsed() < std::time::Duration::from_secs(5));
    // b edits; watch pushes once quiet (5 s) and a pulls it.
    std::fs::write(fb.join("from-b.md"), "hello a\n").unwrap();
    let back = async {
        loop {
            assert_eq!(code(commands::sync_cmd(&a, None, false).await), 0);
            if fa.join("from-b.md").exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(20), back)
        .await
        .expect("b's edit was pushed by watch");
    stop.cancel();
    assert_eq!(watcher.await.unwrap(), Ok(0));
}
