//! RFC 0003 phase 3: notification rules and signed webhook deliveries,
//! against a local https receiver (T1 to T6). Opt-in: KURA_TEST_DATABASE_URL,
//! KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test helpers fail the test on purpose; the receiver's offsets fit a request"
)]

mod common;

use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use common::app::{Env, env_custom, problem, storage, vault_id};
use kura_server::notifier::Notifier;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use uuid::Uuid;

/// One request the receiver got: its head (lowercased header names) and body.
#[derive(Clone, Debug)]
struct Got {
    head: String,
    body: Vec<u8>,
}

impl Got {
    fn header(&self, name: &str) -> Option<String> {
        self.head
            .lines()
            .find_map(|l| {
                l.split_once(':')
                    .filter(|(k, _)| k.trim().eq_ignore_ascii_case(name))
            })
            .map(|(_, v)| v.trim().to_owned())
    }
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

/// An https listener on 127.0.0.1 with a certificate made for the test; it
/// logs each request and answers `status`.
struct Receiver {
    url: String,
    ca: reqwest::Certificate,
    got: Arc<Mutex<Vec<Got>>>,
    status: Arc<Mutex<u16>>,
}

async fn receiver() -> Receiver {
    let key = rcgen::KeyPair::generate().unwrap();
    let params =
        rcgen::CertificateParams::new(vec!["localhost".into(), "127.0.0.1".into()]).unwrap();
    let cert = params.self_signed(&key).unwrap();
    let ca = reqwest::Certificate::from_der(cert.der()).unwrap();
    // The provider named: ring (sqlx) and aws-lc-rs both link here.
    let provider = Arc::new(tokio_rustls::rustls::crypto::aws_lc_rs::default_provider());
    let config = tokio_rustls::rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            tokio_rustls::rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into()),
        )
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "https://127.0.0.1:{}/hook",
        listener.local_addr().unwrap().port()
    );
    let got = Arc::new(Mutex::new(Vec::new()));
    let status = Arc::new(Mutex::new(200u16));
    let (g, st) = (got.clone(), status.clone());
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let (acceptor, g, st) = (acceptor.clone(), g.clone(), st.clone());
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let mut buf = Vec::new();
                let mut chunk = [0u8; 4096];
                // Head, then as many body bytes as content-length says.
                loop {
                    let n = tls.read(&mut chunk).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let head = text[..end].to_owned();
                        let len: usize = head
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if buf.len() >= end + 4 + len {
                            g.lock().unwrap().push(Got {
                                head,
                                body: buf[end + 4..end + 4 + len].to_vec(),
                            });
                            let code = *st.lock().unwrap();
                            let answer = format!(
                                "HTTP/1.1 {code} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                            );
                            drop(tls.write_all(answer.as_bytes()).await);
                            drop(tls.shutdown().await);
                            return;
                        }
                    }
                }
            });
        }
    });
    Receiver {
        url,
        ca,
        got,
        status,
    }
}

/// The app with notifications on, the receiver's CA trusted and `allowed`
/// opened (the receiver listens on loopback, which is refused by default).
async fn setup(test: &str, r: &Receiver, allowed: &[&str]) -> Option<Env> {
    let st = storage(test).await?;
    let allowed: Vec<ipnet::IpNet> = allowed.iter().map(|a| a.parse().unwrap()).collect();
    let client =
        kura_server::net::webhook_client(allowed.clone(), std::slice::from_ref(&r.ca)).unwrap();
    env_custom(test, Some(st), move |s| {
        s.with_notifier(Notifier::new(client, allowed))
    })
    .await
}

struct Admin {
    token: String,
    device: String,
}

impl Admin {
    async fn new(e: &Env, groups: &[&str]) -> Self {
        let token = e.key.token("boss", "sa", groups);
        let device = e.register(&token, "desk").await;
        Self { token, device }
    }
    async fn call(
        &self,
        e: &Env,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        e.call(
            method,
            &format!("/v1{path}"),
            &self.token,
            Some(&self.device),
            body,
        )
        .await
    }
}

fn rule(url: &str, events: &[&str], vault: Option<&str>) -> Value {
    json!({ "name": "hub", "events": events, "vault": vault, "channel": { "type": "webhook", "url": url, "authorization": "Bearer test-key" } })
}

/// Claims and delivers what is due, as the worker does, until nothing is.
async fn drain(e: &Env) {
    loop {
        let due: Vec<(Uuid, Uuid, String)> =
            sqlx::query_as("SELECT id, tenant_id, slug FROM kura_notification_claim(16)")
                .fetch_all(&e.t.app)
                .await
                .unwrap();
        if due.is_empty() {
            return;
        }
        for (id, tenant, slug) in due {
            kura_server::notifier::deliver(&e.state, id, tenant, &slug)
                .await
                .unwrap();
        }
    }
}

/// §6.P observation 4, T3, T4: a test is signed, carries the rule's
/// `authorization`, and nothing of the URL or secrets is shown again.
#[tokio::test]
async fn a_rule_signs_its_deliveries_and_never_shows_its_secrets() {
    let r = receiver().await;
    let Some(e) = setup(module_path!(), &r, &["127.0.0.1/32"]).await else {
        return;
    };
    let admin = Admin::new(&e, &["maxime-admins"]).await;
    let (st, created) = admin
        .call(
            &e,
            "POST",
            "/tenants/maxime/notifications",
            Some(rule(&r.url, &["token.created"], None)),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{created}");
    let secret = created["secret"].as_str().unwrap().to_owned();
    assert!(secret.starts_with("whsec_"));
    assert_eq!(created["host"], "127.0.0.1");
    let id = created["id"].as_str().unwrap().to_owned();

    let (st, test) = admin
        .call(
            &e,
            "POST",
            &format!("/tenants/maxime/notifications/{id}/test"),
            None,
        )
        .await;
    assert_eq!(
        (st, &test["result"]),
        (StatusCode::OK, &json!("200")),
        "{test}"
    );
    let got = r.got.lock().unwrap().clone();
    assert_eq!(got.len(), 1);
    let ping = &got[0];
    assert_eq!(ping.header("x-kura-event").as_deref(), Some("ping"));
    assert_eq!(
        ping.header("authorization").as_deref(),
        Some("Bearer test-key")
    );
    // T3: HMAC-SHA256 of "timestamp.body" with the secret's bytes.
    let raw = base64::Engine::decode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        secret.trim_start_matches("whsec_"),
    )
    .unwrap();
    let ts: i64 = ping.header("x-kura-timestamp").unwrap().parse().unwrap();
    assert_eq!(
        ping.header("x-kura-signature").unwrap(),
        kura_server::notifier::signature(&raw, ts, &ping.body)
    );
    assert_eq!(ping.json()["type"], "ping");
    assert!(ping.header("x-kura-delivery").is_some());

    // T4: the list, the database and the deliveries hold no URL path, secret or key.
    let (_, list) = admin
        .call(&e, "GET", "/tenants/maxime/notifications", None)
        .await;
    let text = list.to_string();
    for leaked in ["/hook", "test-key", &secret] {
        assert!(!text.contains(leaked), "{leaked} in {text}");
    }
    assert_eq!(list[0]["hasAuthorization"], true);
    assert_eq!(list[0]["last"]["result"], "200");
    let sealed: Vec<u8> = sqlx::query_scalar("SELECT sealed_url FROM notification_rule")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert!(!sealed.windows(4).any(|w| w == b"hook"), "sealed at rest");
    let (_, deliveries) = admin
        .call(
            &e,
            "GET",
            &format!("/tenants/maxime/notifications/{id}/deliveries"),
            None,
        )
        .await;
    assert_eq!(deliveries[0]["event"], "ping");
    assert_eq!(deliveries[0]["status"], "delivered");

    // A rotated secret is shown once more, and signs from then on.
    let (st, rotated) = admin
        .call(
            &e,
            "PUT",
            &format!("/tenants/maxime/notifications/{id}"),
            Some(json!({ "rotateSecret": true })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_ne!(rotated["secret"], json!(secret));
    let (_, again) = admin
        .call(
            &e,
            "PUT",
            &format!("/tenants/maxime/notifications/{id}"),
            Some(json!({ "name": "hub2" })),
        )
        .await;
    assert!(again.get("secret").is_none());
}

/// T1, §6.P observation 6: every refused target gets one message, at
/// creation and at sending; a refused address is never connected to.
#[tokio::test]
async fn no_webhook_reaches_an_address_the_operator_did_not_allow() {
    let r = receiver().await;
    let Some(e) = setup(module_path!(), &r, &[]).await else {
        return;
    };
    let admin = Admin::new(&e, &["maxime-admins"]).await;
    let port = r
        .url
        .split(':')
        .nth(2)
        .unwrap()
        .split('/')
        .next()
        .unwrap()
        .to_owned();
    let mut messages = std::collections::HashSet::new();
    for url in [
        r.url.clone(),
        format!("https://localhost:{port}/hook"),
        "https://10.0.0.5/hook".into(),
        "https://169.254.169.254/latest/meta-data".into(),
        "https://[::ffff:127.0.0.1]/".into(),
        "http://example.org/hook".into(),
        "https://user:pw@example.org/hook".into(),
        "https://does-not-resolve.invalid/hook".into(),
    ] {
        let (st, b) = admin
            .call(
                &e,
                "POST",
                "/tenants/maxime/notifications",
                Some(rule(&url, &["token.created"], None)),
            )
            .await;
        assert_eq!(
            (st, problem(&b)),
            (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request"),
            "{url}"
        );
        messages.insert(b["detail"].as_str().unwrap().to_owned());
    }
    assert_eq!(
        messages.len(),
        1,
        "one message for every case: {messages:?}"
    );

    // A row written past the API: the delivery is refused, nothing connects.
    let tenant: Uuid = sqlx::query_scalar("SELECT id FROM tenant WHERE slug = 'maxime'")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    let id = Uuid::now_v7();
    let key = e
        .state
        .storage
        .as_ref()
        .unwrap()
        .keys
        .notifications(&e.t.app, tenant, "maxime")
        .await
        .unwrap();
    let seal = |f: &str, v: &[u8]| {
        kura_server::store::crypto::seal_value(&key, &kura_server::notifier::context(id, f), v)
            .unwrap()
    };
    sqlx::query(
        "INSERT INTO notification_rule (id, tenant_id, name, events, host, sealed_url, sealed_secret, created_by)
         VALUES ($1, $2, 'past', '{ping}', '127.0.0.1', $3, $4, 'test')",
    )
    .bind(id)
    .bind(tenant)
    .bind(seal("url", r.url.as_bytes()))
    .bind(seal("secret", b"0123456789abcdef0123456789abcdef"))
    .execute(&e.t.owner)
    .await
    .unwrap();
    let (st, test) = admin
        .call(
            &e,
            "POST",
            &format!("/tenants/maxime/notifications/{id}/test"),
            None,
        )
        .await;
    assert_eq!(
        (st, &test["result"]),
        (StatusCode::OK, &json!("address_refused")),
        "{test}"
    );
    assert!(
        r.got.lock().unwrap().is_empty(),
        "the refused connection was never made"
    );
}

/// T5, T6, §6.P observations 5 and 7: an event is recorded only when a rule
/// listens; payloads carry ids and names, never a subject, a path or a
/// device name; retries back off; 410 and 20 failures switch a rule off.
#[tokio::test]
async fn events_reach_their_rules_with_ids_only_and_retries_back_off() {
    let r = receiver().await;
    let Some(e) = setup(module_path!(), &r, &["127.0.0.1/32"]).await else {
        return;
    };
    let admin = Admin::new(&e, &["maxime-admins"]).await;
    let events = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM notification_event")
            .fetch_one(&e.t.owner)
            .await
            .unwrap()
    };
    // No rule: a fact records nothing (T6).
    let member = e.key.token("u1", "s1", &["maxime-members"]);
    let phone = e.register(&member, "alice's phone").await;
    assert_eq!(events().await, 0);

    let (st, b) = admin
        .call(
            &e,
            "POST",
            "/tenants/maxime/notifications",
            Some(rule(&r.url, &["device.revoked", "vault.changed"], None)),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{b}");
    let rule_id = b["id"].as_str().unwrap().to_owned();
    let (st, _) = e
        .call(
            "DELETE",
            &format!("/v1/devices/{phone}"),
            &member,
            Some(&phone),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    drain(&e).await;
    let got = r.got.lock().unwrap().clone();
    let revoked = got
        .iter()
        .find(|g| g.header("x-kura-event").as_deref() == Some("device.revoked"))
        .unwrap();
    let body = revoked.json();
    assert_eq!(body["data"]["device"]["id"], json!(phone));
    let flat = body.to_string();
    for personal in ["u1", "alice's phone", "\"sub\"", "\"path\""] {
        assert!(!flat.contains(personal), "{personal} in {flat}");
    }

    // vault.changed: one event a window, counting the facts in it (T6).
    let tenant: Uuid = sqlx::query_scalar("SELECT id FROM tenant WHERE slug = 'maxime'")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    for _ in 0..3 {
        let mut tx = kura_server::db::begin_tenant(&e.t.app, tenant, None)
            .await
            .unwrap();
        kura_server::notifier::insert(
            &mut tx,
            kura_server::notifier::Fact {
                tenant,
                tenant_slug: "maxime",
                vault: None,
                kind: "vault.changed",
                title: "t".into(),
                message: "m".into(),
                data: json!({}),
                dedupe: Some("vault.changed:x:1".into()),
            },
            None,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let changed: Vec<Value> =
        sqlx::query_scalar("SELECT payload FROM notification_event WHERE type = 'vault.changed'")
            .fetch_all(&e.t.owner)
            .await
            .unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0]["data"]["count"], 3);
    let deliveries: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM notification_delivery d JOIN notification_event ev ON ev.id = d.event_id WHERE ev.type = 'vault.changed'",
    )
    .fetch_one(&e.t.owner)
    .await
    .unwrap();
    assert_eq!(deliveries, 1, "one delivery for the window");

    // 500: retried a minute later (§6.P observation 7).
    *r.status.lock().unwrap() = 500;
    let (_, test) = admin
        .call(
            &e,
            "POST",
            &format!("/tenants/maxime/notifications/{rule_id}/test"),
            None,
        )
        .await;
    assert_eq!(test["result"], "status_5xx");
    let (attempts, status, wait): (i32, String, f64) = sqlx::query_as(
        "SELECT d.attempts, d.status, extract(epoch FROM d.next_attempt_at - now())::float8 FROM notification_delivery d
         JOIN notification_event ev ON ev.id = d.event_id WHERE ev.type = 'ping'",
    )
    .fetch_one(&e.t.owner)
    .await
    .unwrap();
    assert_eq!((attempts, status.as_str()), (1, "pending"));
    assert!((50.0..70.0).contains(&wait), "{wait}");

    // The 20th failure in a row switches the rule off; a re-enable forgets them.
    sqlx::query("UPDATE notification_rule SET consecutive_failures = 19")
        .execute(&e.t.owner)
        .await
        .unwrap();
    drop(
        admin
            .call(
                &e,
                "POST",
                &format!("/tenants/maxime/notifications/{rule_id}/test"),
                None,
            )
            .await,
    );
    let (_, list) = admin
        .call(&e, "GET", "/tenants/maxime/notifications", None)
        .await;
    assert_eq!(
        (
            list[0]["enabled"].clone(),
            list[0]["disabledReason"].clone()
        ),
        (json!(false), json!("failures: status_5xx"))
    );
    let (_, back) = admin
        .call(
            &e,
            "PUT",
            &format!("/tenants/maxime/notifications/{rule_id}"),
            Some(json!({ "enabled": true })),
        )
        .await;
    assert_eq!(
        (back["enabled"].clone(), back["consecutiveFailures"].clone()),
        (json!(true), json!(0))
    );
    // 410 Gone switches it off at once.
    *r.status.lock().unwrap() = 410;
    drop(
        admin
            .call(
                &e,
                "POST",
                &format!("/tenants/maxime/notifications/{rule_id}/test"),
                None,
            )
            .await,
    );
    let (_, list) = admin
        .call(&e, "GET", "/tenants/maxime/notifications", None)
        .await;
    assert_eq!(list[0]["disabledReason"], "gone");
}

/// T5 and §4.3: a vault manager subscribes one vault of theirs to vault
/// events, sees no other rule, and every route is 409 with notifications off.
#[tokio::test]
async fn a_manager_sees_their_vaults_only_and_off_means_409() {
    let r = receiver().await;
    let Some(e) = setup(module_path!(), &r, &["127.0.0.1/32"]).await else {
        return;
    };
    let admin = Admin::new(&e, &["maxime-admins"]).await;
    let (st, _) = admin
        .call(
            &e,
            "POST",
            "/tenants/maxime/notifications",
            Some(rule(&r.url, &["quota.high"], None)),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED);
    let v = vault_id(&e, "knowledge").await;
    let (st, _) = admin
        .call(
            &e,
            "PUT",
            &format!("/vaults/{v}/bindings/claim:maxime-members"),
            Some(json!({ "role": "manager" })),
        )
        .await;
    assert!(st == StatusCode::OK || st == StatusCode::FORBIDDEN);
    sqlx::query("UPDATE role_binding SET role = 'manager' WHERE subject = 'maxime-members'")
        .execute(&e.t.owner)
        .await
        .unwrap();
    let m_token = e.key.token("m1", "sm", &["maxime-members"]);
    let manager = Admin {
        device: e.register(&m_token, "m").await,
        token: m_token,
    };
    let (_, list) = manager
        .call(&e, "GET", "/tenants/maxime/notifications", None)
        .await;
    assert_eq!(list, json!([]), "the tenant rule is not theirs");
    for (body, why) in [
        (rule(&r.url, &["vault.changed"], None), "no vault"),
        (rule(&r.url, &["device.revoked"], Some(&v)), "tenant event"),
    ] {
        let (st, b) = manager
            .call(&e, "POST", "/tenants/maxime/notifications", Some(body))
            .await;
        assert_eq!(
            (st, problem(&b)),
            (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role"),
            "{why}"
        );
    }
    let (st, b) = manager
        .call(
            &e,
            "POST",
            "/tenants/maxime/notifications",
            Some(rule(&r.url, &["vault.changed", "token.expiring"], Some(&v))),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{b}");
    let (_, list) = manager
        .call(&e, "GET", "/tenants/maxime/notifications", None)
        .await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    let (st, _) = admin
        .call(&e, "GET", "/tenants/other/notifications", None)
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "another tenant looks missing");
    let (st, b) = admin
        .call(
            &e,
            "POST",
            "/tenants/maxime/notifications",
            Some(rule(&r.url, &["nope"], None)),
        )
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request")
    );

    // Off: 409 on every route (§4.3).
    let Some(off) = common::app::env_with(&format!("{}-off", module_path!()), None).await else {
        return;
    };
    let token = off.key.token("boss", "s", &["maxime-admins"]);
    let d = off.register(&token, "d").await;
    let (st, b) = off
        .call(
            "GET",
            "/v1/tenants/maxime/notifications",
            &token,
            Some(&d),
            None,
        )
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::CONFLICT, "/v1/problems/notifications_disabled")
    );
}
