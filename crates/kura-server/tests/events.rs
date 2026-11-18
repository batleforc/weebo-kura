//! WebSocket tickets and notifications (RFC 0001 §6.3, decision 36, T11),
//! over real TCP, with each replica's `LISTEN` task running. Opt-in:
//! KURA_TEST_DATABASE_URL.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use common::app::{Env, env, vault_id};
use futures_util::StreamExt as _;
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::{AppState, app};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Serves `state` on a local port with its `LISTEN` task; returns the address.
async fn replica(state: Arc<AppState>, stop: &CancellationToken) -> String {
    tokio::spawn(kura_server::notify::listen(
        state.db.clone(),
        state.clone(),
        stop.clone(),
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = app(state).into_make_service_with_connect_info::<SocketAddr>();
    tokio::spawn(async move { axum::serve(listener, router).await });
    // Let the LISTEN connection come up.
    tokio::time::sleep(Duration::from_millis(500)).await;
    format!("127.0.0.1:{}", addr.port())
}

struct S {
    e: Env,
    tok: String,
    d: String,
    v: String,
    addr: String,
    stop: CancellationToken,
}

async fn setup(test: &str) -> Option<S> {
    let e = env(test).await?;
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&tok, "desk").await;
    let v = vault_id(&e, "knowledge").await;
    let stop = CancellationToken::new();
    let addr = replica(e.state.clone(), &stop).await;
    Some(S {
        e,
        tok,
        d,
        v,
        addr,
        stop,
    })
}

impl S {
    async fn ticket(&self, d: &str) -> String {
        let (st, b) = self
            .e
            .call(
                "POST",
                &format!("/v1/vaults/{}/events/ticket", self.v),
                &self.tok,
                Some(d),
                None,
            )
            .await;
        assert_eq!(st.as_u16(), 200, "{b}");
        assert_eq!(b["expiresIn"], 30);
        b["ticket"].as_str().unwrap().to_owned()
    }

    async fn open(&self, addr: &str, vault: &str, ticket: &str) -> Socket {
        let url = format!("ws://{addr}/v1/vaults/{vault}/events?ticket={ticket}");
        tokio_tungstenite::connect_async(url).await.unwrap().0
    }

    async fn commit(&self, path: &str) {
        let size = path.len() as u64;
        let b = blob_hash(size, &[ChunkHash::of(path.as_bytes())]);
        sqlx::query("INSERT INTO blob (vault_id, tenant_id, hash, size) SELECT id, tenant_id, $2, $3 FROM vault WHERE id = $1::uuid ON CONFLICT DO NOTHING")
            .bind(&self.v)
            .bind(b.as_bytes().as_slice())
            .bind(i64::try_from(size).unwrap())
            .execute(&self.e.t.owner)
            .await
            .unwrap();
        let op = json!({ "op": "put", "entry": Uuid::now_v7(), "path": path, "blob": b.to_string(), "size": size });
        let (st, body) = self
            .e
            .call(
                "POST",
                &format!("/v1/vaults/{}/commits", self.v),
                &self.tok,
                Some(&self.d),
                Some(json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op] })),
            )
            .await;
        assert_eq!(st.as_u16(), 200, "{body}");
    }
}

/// The next notice that is not a ping, or the close code.
async fn next(ws: &mut Socket) -> Result<Value, u16> {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(8), ws.next())
            .await
            .expect("nothing within 8 s");
        match m {
            Some(Ok(Message::Text(t))) => {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["type"] != "ping" {
                    return Ok(v);
                }
            }
            Some(Ok(Message::Close(Some(f)))) => return Err(u16::from(f.code)),
            Some(Ok(Message::Close(None))) | None => return Err(1005),
            Some(Ok(_)) => {}
            Some(Err(e)) => panic!("{e}"),
        }
    }
}

#[tokio::test]
async fn a_commit_reaches_the_socket() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let t = s.ticket(&s.d).await;
    let mut ws = s.open(&s.addr, &s.v, &t).await;
    s.commit("a.md").await;
    assert_eq!(
        next(&mut ws).await,
        Ok(json!({ "type": "change", "seq": 1 }))
    );
    assert_eq!(s.e.state.metrics.ws_connections.get(), 1);
    drop(ws);
    for _ in 0..50 {
        if s.e.state.metrics.ws_connections.get() == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(
        s.e.state.metrics.ws_connections.get(),
        0,
        "the seat is released"
    );
    s.stop.cancel();
}

#[tokio::test]
async fn ticket_reuse_is_refused() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let t = s.ticket(&s.d).await;
    let _first = s.open(&s.addr, &s.v, &t).await;
    let mut second = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut second).await, Err(4003));
    let mut forged = s.open(&s.addr, &s.v, "not-a-ticket").await;
    assert_eq!(next(&mut forged).await, Err(4003));
    s.stop.cancel();
}

#[tokio::test]
async fn ticket_for_other_vault_is_refused() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let t = s.ticket(&s.d).await;
    let other = vault_id(&s.e, "secrets").await;
    let mut ws = s.open(&s.addr, &other, &t).await;
    assert_eq!(next(&mut ws).await, Err(4004));
    // Consumed anyway: not reusable on its own vault either.
    let mut again = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut again).await, Err(4003));
    s.stop.cancel();
}

#[tokio::test]
async fn expired_ticket_is_refused() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let t = s.ticket(&s.d).await;
    sqlx::query("UPDATE ws_ticket SET expires_at = now() - interval '1 second'")
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let mut ws = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut ws).await, Err(4003));
    s.stop.cancel();
}

#[tokio::test]
async fn sockets_close_at_token_expiry_on_revocation_and_past_four() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    // The token behind a ticket expires: the socket closes then.
    let t = s.ticket(&s.d).await;
    sqlx::query("UPDATE ws_ticket SET token_expires_at = now() + interval '1 second'")
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let mut ws = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut ws).await, Err(4001));
    // Four sockets per device; a fifth is refused.
    let mut open = Vec::new();
    for _ in 0..4 {
        let t = s.ticket(&s.d).await;
        open.push(s.open(&s.addr, &s.v, &t).await);
    }
    let t = s.ticket(&s.d).await;
    let mut fifth = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut fifth).await, Err(4008));
    // Revoking the device closes its sockets.
    let other = s.e.register(&s.tok, "phone").await;
    let (st, _) =
        s.e.call(
            "DELETE",
            &format!("/v1/devices/{}", s.d),
            &s.tok,
            Some(&other),
            None,
        )
        .await;
    assert_eq!(st.as_u16(), 204);
    assert_eq!(next(&mut open[0]).await, Err(4002));
    s.stop.cancel();
}

/// Plan V4: an open socket re-checks the vault role on every ping (the first
/// at once), so a caller who lost access stops hearing about the vault.
#[tokio::test]
async fn a_socket_closes_when_its_caller_loses_the_vault() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    // Still a reader: the socket stays open and serves its notices.
    let t = s.ticket(&s.d).await;
    let mut kept = s.open(&s.addr, &s.v, &t).await;
    // The groups that granted the role are gone from the ticket's claims, as
    // when the mapping or binding behind them no longer matches.
    let t = s.ticket(&s.d).await;
    sqlx::query("UPDATE ws_ticket SET claims = jsonb_set(claims, '{groups}', '[]')")
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let mut ws = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut ws).await, Err(4005));
    // A ticket with no claims at all is spent.
    let t = s.ticket(&s.d).await;
    sqlx::query("UPDATE ws_ticket SET claims = '{}'")
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let mut empty = s.open(&s.addr, &s.v, &t).await;
    assert_eq!(next(&mut empty).await, Err(4003));
    s.commit("kept.md").await;
    assert_eq!(next(&mut kept).await.unwrap()["type"], "change");
    s.stop.cancel();
}

/// A commit served by one replica reaches a socket held by another, through
/// PostgreSQL's `NOTIFY`, never through the replicas themselves.
#[tokio::test]
async fn a_notification_crosses_replicas() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let b = Arc::new(AppState::new(
        kura_server::metrics::Metrics::new().unwrap(),
        s.e.t.app.clone(),
        s.e.key.verifier(),
    ));
    b.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&s.e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    let addr_b = replica(b, &s.stop).await;
    let t = s.ticket(&s.d).await;
    let mut ws = s.open(&addr_b, &s.v, &t).await;
    s.commit("crossing.md").await;
    assert_eq!(
        next(&mut ws).await,
        Ok(json!({ "type": "change", "seq": 1 }))
    );
    s.stop.cancel();
}

/// Review 4 M1: the ticket route takes no token, so it is limited per client
/// address before any database work: random tickets end in 429.
#[tokio::test]
async fn a_ticket_flood_is_limited_per_address() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let mut limited = false;
    for i in 0..70 {
        let url = format!("ws://{}/v1/vaults/{}/events?ticket=forged{i}", s.addr, s.v);
        if let Err(tokio_tungstenite::tungstenite::Error::Http(r)) =
            tokio_tungstenite::connect_async(url).await
        {
            assert_eq!(r.status().as_u16(), 429);
            limited = true;
            break;
        }
    }
    assert!(limited, "70 attempts were not limited");
    s.stop.cancel();
}

/// RFC 0005 T5, existing behaviour pinned: a viewer restricted to
/// `Sessions/` holds a socket, and a change under a hidden folder reaches it
/// as a `seq` only, never a path.
#[tokio::test]
async fn notice_has_no_path() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    sqlx::query(
        "INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by, paths)
         SELECT tenant_id, id, 'user', 'bob', 'viewer', 'api', ARRAY['Sessions/'] FROM vault WHERE id = $1::uuid",
    )
    .bind(&s.v)
    .execute(&s.e.t.owner)
    .await
    .unwrap();
    let bob = s.e.key.token("bob", "s-bob", &[]);
    let bob_device = s.e.register(&bob, "bob").await;
    let (st, b) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/events/ticket", s.v),
            &bob,
            Some(&bob_device),
            None,
        )
        .await;
    assert_eq!(st.as_u16(), 200, "{b}");
    let mut ws = s.open(&s.addr, &s.v, b["ticket"].as_str().unwrap()).await;
    s.commit("MJ/secret-villain.md").await;
    let notice = next(&mut ws).await.unwrap();
    assert_eq!(notice, json!({ "type": "change", "seq": 1 }));
    assert!(!notice.to_string().contains("MJ"), "{notice}");
    s.stop.cancel();
}
