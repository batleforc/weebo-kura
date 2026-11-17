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
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 2);
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
        code(commands::sync_cmd(&a, Some("knowledge"), false, false).await),
        0
    );
    assert_eq!(code(commands::sync_cmd(&b, None, false, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    let (la, lb) = (files(&fa), files(&fb));
    assert_eq!(la, lb);
    assert_eq!(la.len(), 3, "{la:?}");
    assert!(
        la.iter().any(|(_, t)| t.contains("from a"))
            && la.iter().any(|(_, t)| t.contains("from b"))
    );

    // A name that cannot sync: the rest syncs, exit 4; a dry run sends nothing.
    std::fs::write(fa.join("Q3: budget.md"), "x").unwrap();
    assert_eq!(code(commands::sync_cmd(&a, None, true, false).await), 4);
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 4);
    assert_eq!(
        code(commands::sync_cmd(&a, Some("other"), false, false).await),
        3
    );
    assert_eq!(code(commands::status(&a).await), 0);
    assert_eq!(code(commands::devices(&a, None).await), 0);
    // §13: the migration check counts the name that cannot sync, then passes
    // once the folder and the server hold the same notes.
    assert_eq!(code(commands::verify(&a, "knowledge").await), 1);
    std::fs::remove_file(fa.join("Q3: budget.md")).unwrap();
    assert_eq!(code(commands::verify(&a, "knowledge").await), 0);
    std::fs::write(fa.join("late.md"), "not pushed").unwrap();
    assert_eq!(code(commands::verify(&a, "knowledge").await), 1);
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    assert_eq!(code(commands::verify(&a, "knowledge").await), 0);
    assert_eq!(code(commands::verify(&a, "other").await), 3);

    // Revoking phone-b from its own owner's other session is refused by name
    // (u1 cannot see u2's devices); u2 revokes it, then it must sign in (2).
    let revoke = |name: &str| Some(DeviceAction::Revoke { name: name.into() });
    assert_eq!(code(commands::devices(&a, revoke("phone-b")).await), 3);
    assert_eq!(code(commands::devices(&b, revoke("phone-b")).await), 0);
    assert_eq!(code(commands::sync_cmd(&b, None, false, false).await), 2);
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
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
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
            assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
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

/// §6.P observation 6 and §14 "horizon and full resync": a device away past
/// `deletedEntryTtl` gets 410, resyncs, and a note deleted meanwhile is gone
/// everywhere (in its `.trash` here); its own edit to that note survives as a
/// new file; history and restore work through the CLI.
#[tokio::test]
async fn a_long_absence_resyncs_and_history_restores() {
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
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();
    let (a, b) = (
        Ctx {
            home: dir("resync-home-a"),
            access_token: Some(e.key.token("u1", "ra", &["maxime-members"])),
        },
        Ctx {
            home: dir("resync-home-b"),
            access_token: Some(e.key.token("u1", "rb", &["maxime-members"])),
        },
    );
    for (ctx, name) in [(&a, "desk"), (&b, "laptop")] {
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
    let (fa, fb) = (dir("resync-a"), dir("resync-b"));
    for i in 0..10 {
        std::fs::write(fa.join(format!("n{i}.md")), format!("note {i}\n")).unwrap();
    }
    std::fs::write(fa.join("gone.md"), "deleted elsewhere\n").unwrap();
    std::fs::write(fa.join("edited.md"), "v1\n").unwrap();
    assert_eq!(
        code(commands::attach(&a, "knowledge", fa.clone(), true).await),
        0
    );
    assert_eq!(
        code(commands::attach(&b, "knowledge", fb.clone(), true).await),
        0
    );
    // b goes away; a deletes two notes, one of which b edits meanwhile.
    std::fs::remove_file(fa.join("gone.md")).unwrap();
    std::fs::remove_file(fa.join("edited.md")).unwrap();
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    std::fs::write(fa.join("later.md"), "written after\n").unwrap();
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    std::fs::write(fb.join("edited.md"), "v2, written while away\n").unwrap();
    // 100 days later, retention purges the deletions.
    sqlx::query("UPDATE revision SET created_at = now() - interval '100 days'")
        .execute(&e.t.owner)
        .await
        .unwrap();
    let fast = kura_server::maintenance::Settings {
        pause: std::time::Duration::ZERO,
        orphan_grace: time::Duration::hours(24),
    };
    let r =
        kura_server::maintenance::pass(&e.state, fast, &tokio_util::sync::CancellationToken::new())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(r.entries, 2, "{r:?}");
    // One of twelve files to trash: under the guard, no question asked.
    assert_eq!(code(commands::sync_cmd(&b, None, false, false).await), 0);
    assert!(!fb.join("gone.md").exists());
    assert_eq!(
        std::fs::read_to_string(fb.join(".trash/gone.md")).unwrap(),
        "deleted elsewhere\n"
    );
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    assert_eq!(files(&fa), files(&fb));
    assert!(files(&fa).contains(&("edited.md".into(), "v2, written while away\n".into())));
    assert!(files(&fa).contains(&("later.md".into(), "written after\n".into())));

    // History and restore of one note.
    std::fs::write(fa.join("n0.md"), "note 0, edited\n").unwrap();
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    assert_eq!(code(commands::history(&a, "knowledge", "n0.md").await), 0);
    assert_eq!(code(commands::history(&a, "knowledge", "nope.md").await), 3);
    let v = uuid::Uuid::parse_str(&common::app::vault_id(&e, "knowledge").await).unwrap();
    let first: uuid::Uuid = sqlx::query_scalar(
        "SELECT r.id FROM revision r JOIN entry n ON n.id = r.entry_id WHERE r.vault_id = $1 AND n.path = 'n0.md' ORDER BY r.seq LIMIT 1",
    )
    .bind(v)
    .fetch_one(&e.t.owner)
    .await
    .unwrap();
    assert_eq!(
        code(commands::restore(&a, "knowledge", "n0.md", first).await),
        0
    );
    assert_eq!(
        std::fs::read_to_string(fa.join("n0.md")).unwrap(),
        "note 0\n"
    );
}

/// Decision 32: the server is restored from a backup older than what the
/// devices hold (`history_rewound`). Nothing local is deleted: an entry the
/// server lost is pushed again, and a version it lost comes back, as a
/// conflict file beside the older head when the server cannot tell they
/// follow each other.
#[tokio::test]
async fn a_rewound_server_gets_everything_back() {
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
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();
    let (a, b) = (
        Ctx {
            home: dir("rewind-home-a"),
            access_token: Some(e.key.token("u1", "wa", &["maxime-members"])),
        },
        Ctx {
            home: dir("rewind-home-b"),
            access_token: Some(e.key.token("u1", "wb", &["maxime-members"])),
        },
    );
    for (ctx, name) in [(&a, "desk"), (&b, "laptop")] {
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
    let (fa, fb) = (dir("rewind-a"), dir("rewind-b"));
    std::fs::write(fa.join("n.md"), "v1\n").unwrap();
    std::fs::write(fa.join("old.md"), "kept\n").unwrap();
    assert_eq!(
        code(commands::attach(&a, "knowledge", fa.clone(), true).await),
        0
    );
    // What the backup holds: seq 1 and 2.
    std::fs::write(fa.join("n.md"), "v2\n").unwrap();
    std::fs::write(fa.join("m.md"), "new\n").unwrap();
    std::fs::remove_file(fa.join("old.md")).unwrap();
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    assert_eq!(
        code(commands::attach(&b, "knowledge", fb.clone(), true).await),
        0
    );
    assert_eq!(files(&fa), files(&fb));
    // Restore: the commits after seq 2 are lost, then the runbook's step.
    let v = uuid::Uuid::parse_str(&common::app::vault_id(&e, "knowledge").await).unwrap();
    let mut tx = e.t.owner.begin().await.unwrap();
    for q in [
        "DELETE FROM revision WHERE vault_id = $1 AND seq > 2",
        "DELETE FROM entry e WHERE vault_id = $1 AND NOT EXISTS (SELECT 1 FROM revision r WHERE r.vault_id = e.vault_id AND r.entry_id = e.id)",
        "UPDATE entry e SET (head_revision, last_seq, deleted, path, path_key) = (SELECT r.id, r.seq, r.deleted, r.path, lower(r.path) FROM revision r WHERE r.vault_id = e.vault_id AND r.entry_id = e.id ORDER BY r.seq DESC LIMIT 1) WHERE vault_id = $1",
        "UPDATE vault SET head_seq = 2 WHERE id = $1",
    ] {
        sqlx::query(q).bind(v).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    assert_eq!(kura_server::db::epoch_reset(&e.t.owner).await.unwrap(), 2);
    // Each device: nothing deleted, everything it held is on the server again.
    assert_eq!(code(commands::sync_cmd(&b, None, false, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&b, None, false, false).await), 0);
    let (la, lb) = (files(&fa), files(&fb));
    assert_eq!(la, lb);
    let texts: Vec<&str> = la.iter().map(|(_, t)| t.as_str()).collect();
    for t in ["v2\n", "new\n"] {
        assert!(texts.contains(&t), "{t:?} lost: {la:?}");
    }
    assert!(
        !fb.join(".trash").exists() && !fa.join(".trash").exists(),
        "nothing trashed"
    );
}

/// §6.2 "refresh once, then sign in", within one run: an access token that
/// expires during a long command (found by perf/attach.sh at 10,000 notes)
/// is renewed from the stored refresh token, rotated, and the request sent
/// again; a fixed `KURA_ACCESS_TOKEN` is never renewed.
#[tokio::test]
async fn an_expired_access_token_is_renewed_mid_run() {
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::get;
    use kura_cli::api::{Api, Renew, Token};

    // A server accepting only the renewed token.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new().route(
        "/v1/devices",
        get(|h: HeaderMap| async move {
            if h.get("authorization").and_then(|v| v.to_str().ok()) == Some("Bearer renewed") {
                (StatusCode::OK, axum::Json(serde_json::json!([])))
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(
                        serde_json::json!({ "type": "/v1/problems/invalid_token", "status": 401 }),
                    ),
                )
            }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, router).await });
    let issuer = fake_issuer("renewed".into()).await;
    let home = dir("home-renew");
    kura_cli::config::set_refresh_token(&home, Some("r1")).unwrap();
    let api = Api {
        http: kura_cli::api::http().unwrap(),
        server: server.clone(),
        token: Token::new("expired".into()),
        device: None,
        renew: Some(Renew {
            issuer,
            client_id: "kura-cli".into(),
            home: home.clone(),
        }),
    };
    api.devices().await.unwrap();
    assert_eq!(api.token.get(), "renewed");
    assert_eq!(
        kura_cli::config::refresh_token(&home).unwrap().as_deref(),
        Some("r2"),
        "the rotated refresh token is stored"
    );
    let fixed = Api {
        token: Token::new("expired".into()),
        renew: None,
        ..api
    };
    assert_eq!(fixed.devices().await.unwrap_err().kind(), "invalid_token");
}

/// §4.5 `kura token create`: --cidr is required (exit 3, before any call);
/// issued, listed, revoked.
#[tokio::test]
async fn the_cli_issues_lists_and_revokes_scoped_tokens() {
    use kura_cli::commands::TokenAction;
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
        home: dir("token-home"),
        access_token: Some(e.key.token("boss", "tb", &["maxime-admins"])),
    };
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();
    let r = commands::login(&a, server, issuer, "kura-cli".into(), Some("admin".into())).await;
    assert_eq!(code(r), 0);
    let create = |cidr: Vec<String>| TokenAction::Create {
        vault: "knowledge".into(),
        name: Some("agent".into()),
        read: vec!["notes/".into()],
        write: vec![],
        prefix: Some("agents/".into()),
        expires: "30d".into(),
        cidr,
    };
    assert_eq!(code(commands::token(&a, create(vec![])).await), 3);
    // Search is opt-in per tenant (decision 39): refused here, exit 3.
    assert_eq!(code(commands::search(&a, "knowledge", "budget").await), 3);
    assert_eq!(
        code(commands::token(&a, create(vec!["127.0.0.0/8".into()])).await),
        0
    );
    assert_eq!(
        code(
            commands::token(
                &a,
                TokenAction::List {
                    vault: "knowledge".into()
                }
            )
            .await
        ),
        0
    );
    let id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM scoped_token LIMIT 1")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    let revoke = TokenAction::Revoke {
        vault: "knowledge".into(),
        id,
    };
    let stored: (String, serde_json::Value) =
        sqlx::query_as("SELECT name, scopes FROM scoped_token WHERE id = $1")
            .bind(id)
            .fetch_one(&e.t.owner)
            .await
            .unwrap();
    assert_eq!(stored.0, "agent");
    assert_eq!(
        stored.1,
        serde_json::json!([{ "path": "notes/", "mode": "read" }, { "path": "agents/", "mode": "read" }]),
        "--prefix is one more --read"
    );
    // RFC 0004 §4.5: the two refusals a manager meets first, both exit 3.
    let write = |folder: &str| TokenAction::Create {
        vault: "knowledge".into(),
        name: None,
        read: vec![],
        write: vec![folder.into()],
        prefix: None,
        expires: "30d".into(),
        cidr: vec!["127.0.0.0/8".into()],
    };
    assert_eq!(
        code(commands::token(&a, write("/")).await),
        3,
        "whole-vault write"
    );
    assert_eq!(
        code(commands::token(&a, write("Resumes/")).await),
        3,
        "tenant switch off"
    );
    assert_eq!(code(commands::token(&a, revoke).await), 0);
    let revoked: bool = sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM scoped_token")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert!(revoked);
}

/// §6.8 with the CLI: the desktop configuration of `.obsidian` travels
/// between one user's devices, in its categories only; another member gets
/// none of it; what stays on a device (enabled plugins, the workspace) never
/// leaves it.
#[tokio::test]
async fn the_cli_syncs_one_users_configuration_only() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let docs = common::app::CONFIG.replace(
        "  tenantRef: { name: maxime }\n  bindings:",
        "  tenantRef: { name: maxime }\n  configSync: { categories: [plugins, snippets] }\n  bindings:",
    );
    let cfgdir = dir("config-docs");
    std::fs::write(cfgdir.join("config.yaml"), docs).unwrap();
    let files = kura_server::reconcile::files::read_dir(&cfgdir).unwrap();
    kura_server::reconcile::reconcile(&e.t.app, &files, true)
        .await
        .unwrap();
    e.state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let router = app(e.state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();
    let ctx = |home: &str, sub: &str, sid: &str| Ctx {
        home: dir(home),
        access_token: Some(e.key.token(sub, sid, &["maxime-members"])),
    };
    let (a, a2, other) = (
        ctx("cfg-home-a", "u1", "ca"),
        ctx("cfg-home-a2", "u1", "ca2"),
        ctx("cfg-home-o", "u2", "co"),
    );
    for (c, n) in [(&a, "desk"), (&a2, "laptop"), (&other, "theirs")] {
        let r = commands::login(
            c,
            server.clone(),
            issuer.clone(),
            "kura-cli".into(),
            Some(n.into()),
        )
        .await;
        assert_eq!(code(r), 0);
    }
    let (fa, fa2, fo) = (dir("cfg-a"), dir("cfg-a2"), dir("cfg-o"));
    let obs = fa.join(".obsidian");
    std::fs::create_dir_all(obs.join("plugins/dataview")).unwrap();
    std::fs::create_dir_all(obs.join("snippets")).unwrap();
    std::fs::write(obs.join("plugins/dataview/main.js"), "module.exports = {};").unwrap();
    std::fs::write(obs.join("snippets/wide.css"), ".x{}").unwrap();
    std::fs::write(obs.join("community-plugins.json"), "[\"dataview\"]").unwrap();
    std::fs::write(obs.join("workspace.json"), "{}").unwrap();
    std::fs::write(obs.join("hotkeys.json"), "{}").unwrap();
    std::fs::write(fa.join("note.md"), "shared\n").unwrap();
    for f in [&fa2, &fo] {
        std::fs::create_dir_all(f.join(".obsidian")).unwrap();
    }
    assert_eq!(
        code(commands::attach(&a, "knowledge", fa.clone(), true).await),
        0
    );
    assert_eq!(
        code(commands::attach(&a2, "knowledge", fa2.clone(), true).await),
        0
    );
    assert_eq!(
        code(commands::attach(&other, "knowledge", fo.clone(), true).await),
        0
    );
    let got = |f: &std::path::Path, p: &str| f.join(".obsidian").join(p).exists();
    assert!(got(&fa2, "plugins/dataview/main.js") && got(&fa2, "snippets/wide.css"));
    for stays in ["community-plugins.json", "workspace.json", "hotkeys.json"] {
        assert!(!got(&fa2, stays), "{stays} left its device");
    }
    for p in ["plugins/dataview/main.js", "snippets/wide.css"] {
        assert!(!got(&fo, p), "{p} reached another member");
    }
    assert!(fo.join("note.md").exists(), "notes are shared as before");
    // A category dropped: its files are forgotten, never deleted elsewhere.
    let only_plugins = common::app::CONFIG.replace(
        "  tenantRef: { name: maxime }\n  bindings:",
        "  tenantRef: { name: maxime }\n  configSync: { categories: [plugins] }\n  bindings:",
    );
    std::fs::write(cfgdir.join("config.yaml"), only_plugins).unwrap();
    let files = kura_server::reconcile::files::read_dir(&cfgdir).unwrap();
    kura_server::reconcile::reconcile(&e.t.app, &files, true)
        .await
        .unwrap();
    e.state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    assert_eq!(code(commands::sync_cmd(&a, None, false, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&a2, None, false, false).await), 0);
    assert!(
        got(&fa2, "snippets/wide.css"),
        "a dropped category deletes nothing"
    );
    let state = std::fs::read_to_string(fa.join(".kura/config-state.json")).unwrap();
    assert!(!state.contains("snippets/wide.css"), "forgotten here");
}

/// RFC 0004 §6.P, in process: a bot holding a scoped token and no login
/// attaches and gets its readable folders only (4), pushes a recap under its
/// write folder (1), and is refused by its own client, before anything is
/// sent, when a file leaves that folder (5).
#[tokio::test]
async fn a_scoped_token_syncs_its_folders_and_refuses_the_rest() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let docs = common::app::CONFIG.replacen(
        "  vaultNamespaces: [maxime]",
        "  vaultNamespaces: [maxime]\n  scopedTokens: { write: true }",
        1,
    );
    let cfgdir = dir("token-docs");
    std::fs::write(cfgdir.join("config.yaml"), docs).unwrap();
    let docs = kura_server::reconcile::files::read_dir(&cfgdir).unwrap();
    kura_server::reconcile::reconcile(&e.t.app, &docs, true)
        .await
        .unwrap();
    e.state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    let router = app(e.state.clone()).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });

    // The game master: an admin on a device, with the campaign's notes.
    let admin = e.key.token("boss", "gm", &["maxime-admins"]);
    let gm = Ctx {
        home: dir("gm-home"),
        access_token: Some(admin.clone()),
    };
    let issuer = "https://auth.example.org/application/o/kura/".to_owned();
    let r = commands::login(
        &gm,
        server.clone(),
        issuer,
        "kura-cli".into(),
        Some("gm".into()),
    )
    .await;
    assert_eq!(code(r), 0);
    let gm_dir = dir("gm-vault");
    for (path, text) in [
        ("Sessions/1.md", "session one\n"),
        ("MJ/secret.md", "the villain\n"),
    ] {
        std::fs::create_dir_all(gm_dir.join(path).parent().unwrap()).unwrap();
        std::fs::write(gm_dir.join(path), text).unwrap();
    }
    assert_eq!(
        code(commands::attach(&gm, "knowledge", gm_dir.clone(), true).await),
        0
    );

    let device = kura_cli::config::Config::load(&gm.home)
        .unwrap()
        .unwrap()
        .device
        .unwrap();
    let v = common::app::vault_id(&e, "knowledge").await;
    let (status, issued) = e
        .call(
            "POST",
            &format!("/v1/vaults/{v}/tokens"),
            &admin,
            Some(&device.to_string()),
            Some(serde_json::json!({
                "name": "discord-recap",
                "scopes": [{ "path": "Sessions/", "mode": "read" }, { "path": "Resumes/", "mode": "write" }],
                "expiresInDays": 30,
                "cidrs": ["127.0.0.0/8"],
            })),
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::CREATED, "{issued}");

    // The bot: a token and the server's address, nothing else.
    let bot = Ctx {
        home: dir("bot-home"),
        access_token: Some(issued["token"].as_str().unwrap().to_owned()),
    };
    kura_cli::config::Config {
        server,
        ..Default::default()
    }
    .save(&bot.home)
    .unwrap();
    let bot_dir = dir("bot-vault");
    assert_eq!(
        code(commands::attach(&bot, "knowledge", bot_dir.clone(), true).await),
        0
    );
    assert_eq!(
        files(&bot_dir),
        [("Sessions/1.md".to_owned(), "session one\n".to_owned())],
        "observation 4: nothing of MJ/"
    );
    assert!(
        kura_cli::config::Config::load(&bot.home)
            .unwrap()
            .unwrap()
            .device
            .is_none(),
        "a token registers no device"
    );
    let on_disk = std::fs::read_to_string(bot.home.join("config.json")).unwrap();
    assert!(!on_disk.contains("kura_"), "the token is never written");

    // Observation 1: a recap under the write folder reaches the game master.
    std::fs::create_dir_all(bot_dir.join("Resumes")).unwrap();
    std::fs::write(bot_dir.join("Resumes/1.md"), "recap one\n").unwrap();
    assert_eq!(code(commands::sync_cmd(&bot, None, false, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&gm, None, false, false).await), 0);
    assert_eq!(
        std::fs::read_to_string(gm_dir.join("Resumes/1.md")).unwrap(),
        "recap one\n"
    );
    let by: String =
        sqlx::query_scalar("SELECT user_sub FROM revision WHERE path = 'Resumes/1.md'")
            .fetch_one(&e.t.owner)
            .await
            .unwrap();
    assert_eq!(by, format!("key:{}", issued["id"].as_str().unwrap()));

    // Observation 5: moved out of the write folder, the client refuses
    // before sending; the server counted no refusal and nothing moved.
    let head = || async {
        sqlx::query_scalar::<_, i64>("SELECT head_seq FROM vault WHERE name = 'knowledge'")
            .fetch_one(&e.t.owner)
            .await
            .unwrap()
    };
    let before = head().await;
    std::fs::create_dir_all(bot_dir.join("MJ")).unwrap();
    std::fs::rename(bot_dir.join("Resumes/1.md"), bot_dir.join("MJ/1.md")).unwrap();
    let refused = commands::sync_cmd(&bot, None, false, false).await;
    assert_eq!(code(refused), 3);
    assert_eq!(head().await, before);
    assert_eq!(
        e.state
            .metrics
            .token_writes_total
            .with_label_values(&["refused"])
            .get(),
        0
    );
    // A file it only reads, edited: refused the same way.
    std::fs::rename(bot_dir.join("MJ/1.md"), bot_dir.join("Resumes/1.md")).unwrap();
    std::fs::write(bot_dir.join("Sessions/1.md"), "rewritten by the bot\n").unwrap();
    assert_eq!(code(commands::sync_cmd(&bot, None, false, false).await), 3);
    assert_eq!(head().await, before);
    // A refused push still pulls (code review of 2026-11-17, #1): the game
    // master's new session reaches the bot whose own edit stays local.
    std::fs::write(gm_dir.join("Sessions/2.md"), "session two\n").unwrap();
    assert_eq!(code(commands::sync_cmd(&gm, None, false, false).await), 0);
    assert_eq!(code(commands::sync_cmd(&bot, None, false, false).await), 3);
    assert_eq!(
        std::fs::read_to_string(bot_dir.join("Sessions/2.md")).unwrap(),
        "session two\n"
    );
    assert_eq!(
        std::fs::read_to_string(bot_dir.join("Sessions/1.md")).unwrap(),
        "rewritten by the bot\n",
        "the local edit is kept"
    );
}
