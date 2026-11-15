//! Scoped tokens end to end (RFC 0001 §6.2; T5, T16, the token half of
//! T10). Requests carry a client address, as through the ingress. Opt-in:
//! KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN,
//! KURA_TEST_S3_ENDPOINT.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use common::app::{Env, env_with, problem, storage, vault_id};
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const INSIDE: &str = "10.42.1.2:50000";
const OUTSIDE: &str = "192.168.1.9:50000";

struct S {
    e: Env,
    admin: String,
    ad: String,
    v: String,
}

async fn setup(test: &str) -> Option<S> {
    let st = storage(test).await?;
    let e = env_with(test, Some(st)).await?;
    let admin = e.key.token("boss", "sb", &["maxime-admins"]);
    let ad = e.register(&admin, "admin-desk").await;
    let v = vault_id(&e, "knowledge").await;
    Some(S { e, admin, ad, v })
}

/// A request from `from`, with an optional bearer and device.
async fn send(
    e: &Env,
    method: &str,
    uri: &str,
    bearer: &str,
    device: Option<&str>,
    body: Option<Body>,
    from: &str,
) -> (StatusCode, Vec<u8>) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {bearer}"))
        .header("content-type", "application/json");
    if let Some(d) = device {
        req = req.header("x-kura-device", d);
    }
    let mut req = req.body(body.unwrap_or_else(Body::empty)).unwrap();
    req.extensions_mut()
        .insert(ConnectInfo(from.parse::<SocketAddr>().unwrap()));
    let r = app(e.state.clone()).oneshot(req).await.unwrap();
    let status = r.status();
    let bytes = axum::body::to_bytes(r.into_body(), 8 << 20).await.unwrap();
    (status, bytes.to_vec())
}

fn json_of(b: &[u8]) -> Value {
    serde_json::from_slice(b).unwrap_or(Value::Null)
}

impl S {
    async fn issue(&self, body: Value) -> (StatusCode, Value) {
        let (st, b) = self
            .e
            .call(
                "POST",
                &format!("/v1/vaults/{}/tokens", self.v),
                &self.admin,
                Some(&self.ad),
                Some(body),
            )
            .await;
        (st, b)
    }

    async fn key(&self, prefix: &str) -> String {
        let (st, b) = self
            .issue(json!({ "prefix": prefix, "expiresInDays": 30, "cidrs": ["10.42.0.0/16"] }))
            .await;
        assert_eq!(st, StatusCode::CREATED, "{b}");
        b["token"].as_str().unwrap().to_owned()
    }

    async fn read(&self, key: &str, path: &str, from: &str) -> (StatusCode, Value) {
        let (st, b) = send(
            &self.e,
            "GET",
            &format!("/v1/vaults/{}{path}", self.v),
            key,
            None,
            None,
            from,
        )
        .await;
        (st, json_of(&b))
    }

    /// A file of one chunk, uploaded and committed by the admin.
    async fn file(&self, entry: &str, path: &str, data: &[u8], base: Option<&str>) -> Value {
        let c = ChunkHash::of(data).to_string();
        let (st, _) = send(
            &self.e,
            "PUT",
            &format!("/v1/vaults/{}/chunks/{c}", self.v),
            &self.admin,
            Some(&self.ad),
            Some(Body::from(data.to_vec())),
            INSIDE,
        )
        .await;
        assert!(st.is_success(), "{st}");
        let size = data.len() as u64;
        let b = blob_hash(size, &[ChunkHash::of(data)]).to_string();
        let (st, body) = self
            .e
            .call(
                "PUT",
                &format!("/v1/vaults/{}/blobs/{b}", self.v),
                &self.admin,
                Some(&self.ad),
                Some(json!({ "size": size, "chunks": [c] })),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{body}");
        let mut op = json!({ "op": "put", "entry": entry, "path": path, "blob": b, "size": size });
        if let Some(base) = base {
            op["base"] = json!(base);
        }
        let (st, body) = self
            .e
            .call(
                "POST",
                &format!("/v1/vaults/{}/commits", self.v),
                &self.admin,
                Some(&self.ad),
                Some(json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op] })),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{body}");
        body["results"][0]["head"].clone()
    }

    async fn last_used(&self) -> Option<time::OffsetDateTime> {
        sqlx::query_scalar("SELECT max(last_used_at) FROM scoped_token")
            .fetch_one(&self.e.t.owner)
            .await
            .unwrap()
    }
}

/// T5 and the token half of T10: a token reads its vault from its
/// addresses, is refused elsewhere, expired, revoked or tampered with, and
/// never writes; each refusal leaves `last_used_at` alone.
#[tokio::test]
async fn token_outside_cidr_is_refused_and_revoked_or_expired_tokens_are_401() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    // Closed defaults (§6.2).
    for (body, why) in [
        (json!({ "expiresInDays": 30, "cidrs": [] }), "no address"),
        (
            json!({ "expiresInDays": 30, "cidrs": ["0.0.0.0/0"] }),
            "every address",
        ),
        (
            json!({ "expiresInDays": 91, "cidrs": ["10.0.0.0/8"] }),
            "past 90 days",
        ),
        (
            json!({ "expiresInDays": 30, "cidrs": ["10.0.0.0/8"], "prefix": "agents" }),
            "prefix without /",
        ),
    ] {
        let (st, b) = s.issue(body).await;
        assert_eq!(
            (st, problem(&b)),
            (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request"),
            "{why}"
        );
    }
    let member = s.e.key.token("u1", "s1", &["maxime-members"]);
    let md = s.e.register(&member, "desk").await;
    let (st, _) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/tokens", s.v),
            &member,
            Some(&md),
            Some(json!({ "expiresInDays": 30, "cidrs": ["10.0.0.0/8"] })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "an editor cannot issue tokens");

    let key = s.key("").await;
    let (st, list) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/tokens", s.v),
            &s.admin,
            Some(&s.ad),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        !list.to_string().contains(&key[38..]),
        "the secret is never listed"
    );
    assert_eq!(list[0]["cidrs"], json!(["10.42.0.0/16"]));

    // From outside its addresses: 403, nothing touched (§6.P observation 3).
    let (st, b) = send(&s.e, "GET", "/v1/vaults", &key, None, None, OUTSIDE).await;
    assert_eq!(
        (st, problem(&json_of(&b))),
        (StatusCode::FORBIDDEN, "/v1/problems/address_not_allowed")
    );
    assert_eq!(s.last_used().await, None);
    let (st, b) = send(&s.e, "GET", "/v1/vaults", &key, None, None, INSIDE).await;
    assert_eq!(st, StatusCode::OK);
    let vaults = json_of(&b);
    assert_eq!(
        vaults.as_array().unwrap().len(),
        1,
        "its one vault: {vaults}"
    );
    assert!(s.last_used().await.is_some());
    let (st, _) = s.read(&key, "/changes?since=0", INSIDE).await;
    assert_eq!(st, StatusCode::OK);
    // Another vault, even one the creator manages: 404.
    let other = vault_id(&s.e, "secrets").await;
    let (st, _) = send(
        &s.e,
        "GET",
        &format!("/v1/vaults/{other}/changes?since=0"),
        &key,
        None,
        None,
        INSIDE,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    // Read only: a write route refuses it.
    let commit = json!({ "idempotencyKey": "k", "operations": [{ "op": "delete", "entry": Uuid::now_v7(), "base": Uuid::now_v7() }] });
    let (st, b) = send(
        &s.e,
        "POST",
        &format!("/v1/vaults/{}/commits", s.v),
        &key,
        None,
        Some(Body::from(commit.to_string())),
        INSIDE,
    )
    .await;
    assert_eq!(
        (st, problem(&json_of(&b))),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role"),
        "read_token_cannot_commit"
    );
    // A tampered secret, then expiry, then revocation: 401.
    let tampered = format!(
        "{}{}",
        &key[..key.len() - 1],
        if key.ends_with('0') { '1' } else { '0' }
    );
    let (st, _) = s.read(&tampered, "/changes?since=0", INSIDE).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    sqlx::query("UPDATE scoped_token SET expires_at = now() - interval '1 second'")
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let (st, b) = s.read(&key, "/changes?since=0", INSIDE).await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/invalid_token"),
        "expired_token_is_401"
    );
    let fresh = s.key("").await;
    let id = Uuid::parse_str(&fresh[5..37]).unwrap();
    let (st, _) =
        s.e.call(
            "DELETE",
            &format!("/v1/vaults/{}/tokens/{id}", s.v),
            &s.admin,
            Some(&s.ad),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = s.read(&fresh, "/changes?since=0", INSIDE).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED, "revoked_token_is_401");
}

/// §6.2: a token issued through a binding dies with the binding.
#[tokio::test]
async fn a_token_is_revoked_with_the_binding_that_allowed_it() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    // A member, known to the tenant by its device, made manager by binding.
    let mgr = s.e.key.token("u2", "sm", &["maxime-members"]);
    let md = s.e.register(&mgr, "mgr-desk").await;
    let (st, b) =
        s.e.call(
            "PUT",
            &format!("/v1/vaults/{}/bindings/user:u2", s.v),
            &s.admin,
            Some(&s.ad),
            Some(json!({ "role": "manager" })),
        )
        .await;
    assert!(st.is_success(), "{b}");
    let (st, b) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/tokens", s.v),
            &mgr,
            Some(&md),
            Some(json!({ "expiresInDays": 7, "cidrs": ["10.42.0.0/16"] })),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{b}");
    let key = b["token"].as_str().unwrap().to_owned();
    assert_eq!(
        s.read(&key, "/changes?since=0", INSIDE).await.0,
        StatusCode::OK
    );
    let (st, _) =
        s.e.call(
            "DELETE",
            &format!("/v1/vaults/{}/bindings/user:u2", s.v),
            &s.admin,
            Some(&s.ad),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    assert_eq!(
        s.read(&key, "/changes?since=0", INSIDE).await.0,
        StatusCode::UNAUTHORIZED
    );
}

/// T16: a token under `agents/` lists, reads history of, and downloads only
/// what is under it, chunks included; an entry renamed out disappears with
/// its history, one renamed in shows only its revisions made inside.
#[tokio::test]
async fn prefix_token_cannot_fetch_outside_chunk_or_read_renamed_history() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (inside, outside, moved_out, moved_in) = (
        Uuid::now_v7().to_string(),
        Uuid::now_v7().to_string(),
        Uuid::now_v7().to_string(),
        Uuid::now_v7().to_string(),
    );
    s.file(&inside, "agents/a.md", b"inside", None).await;
    s.file(&outside, "private/b.md", b"outside", None).await;
    let h = s.file(&moved_out, "agents/x.md", b"was inside", None).await;
    s.file(
        &moved_out,
        "private/x.md",
        b"now outside",
        Some(h["revision"].as_str().unwrap()),
    )
    .await;
    let h = s
        .file(&moved_in, "private/y.md", b"was outside", None)
        .await;
    s.file(
        &moved_in,
        "Agents/y.md",
        b"now inside",
        Some(h["revision"].as_str().unwrap()),
    )
    .await;
    let key = s.key("agents/").await;

    let (st, p) = s.read(&key, "/changes?since=0", INSIDE).await;
    assert_eq!(st, StatusCode::OK);
    let mut paths: Vec<&str> = p["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    paths.sort_unstable();
    assert_eq!(paths, ["Agents/y.md", "agents/a.md"], "case-folded prefix");
    let (_, p) = s.read(&key, "/entries", INSIDE).await;
    assert_eq!(p["items"].as_array().unwrap().len(), 2);

    // History: renamed out, gone with its history; renamed in, inside revisions only.
    let (st, _) = s
        .read(&key, &format!("/entries/{moved_out}/revisions"), INSIDE)
        .await;
    assert_eq!(
        st,
        StatusCode::NOT_FOUND,
        "prefix_token_cannot_read_renamed_history"
    );
    let (st, r) = s
        .read(&key, &format!("/entries/{moved_in}/revisions"), INSIDE)
        .await;
    assert_eq!(st, StatusCode::OK);
    let revs: Vec<&str> = r["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    assert_eq!(revs, ["Agents/y.md"]);

    // Content: inside served, outside 404, chunks and blobs alike.
    for (data, ok) in [
        (&b"inside"[..], true),
        (b"outside", false),
        (b"was outside", false),
        (b"was inside", false),
        (b"now inside", true),
    ] {
        let c = ChunkHash::of(data);
        let blob = blob_hash(data.len() as u64, &[c]);
        let (st, _) = s.read(&key, &format!("/chunks/{c}"), INSIDE).await;
        let (sb, _) = s.read(&key, &format!("/blobs/{blob}"), INSIDE).await;
        let want = if ok {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        };
        assert_eq!((st, sb), (want, want), "{}", String::from_utf8_lossy(data));
    }
    // RFC 0002 T1: the same answers in one batch, outside mixed with inside.
    let all: Vec<&[u8]> = vec![
        b"inside",
        b"outside",
        b"was outside",
        b"was inside",
        b"now inside",
    ];
    let chunks: Vec<String> = all.iter().map(|d| ChunkHash::of(d).to_string()).collect();
    let blobs: Vec<String> = all
        .iter()
        .map(|d| blob_hash(d.len() as u64, &[ChunkHash::of(d)]).to_string())
        .collect();
    let (st, body) = send(
        &s.e,
        "POST",
        &format!("/v1/vaults/{}/chunks/batch", s.v),
        &key,
        None,
        Some(Body::from(json!({ "chunks": chunks }).to_string())),
        INSIDE,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let mut found = Vec::new();
    let mut rest = body.as_slice();
    while rest.len() >= 37 {
        let len = u32::from_be_bytes(rest[33..37].try_into().unwrap()) as usize;
        if rest[32] == 0 {
            found.push(rest[37..37 + len].to_vec());
        }
        rest = &rest[37 + len..];
    }
    assert_eq!(
        found,
        [b"inside".to_vec(), b"now inside".to_vec()],
        "batch_respects_token_prefix"
    );
    let (st, body) = send(
        &s.e,
        "POST",
        &format!("/v1/vaults/{}/blobs/batch", s.v),
        &key,
        None,
        Some(Body::from(json!({ "blobs": blobs }).to_string())),
        INSIDE,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let r = json_of(&body);
    assert_eq!(r["manifests"].as_array().unwrap().len(), 2);
    assert_eq!(r["missing"].as_array().unwrap().len(), 3);
    // The whole-vault view of a device is untouched.
    let c = ChunkHash::of(b"outside");
    let (st, _) = send(
        &s.e,
        "GET",
        &format!("/v1/vaults/{}/chunks/{c}", s.v),
        &s.admin,
        Some(&s.ad),
        None,
        INSIDE,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}
