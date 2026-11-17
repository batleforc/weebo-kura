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
    // Downgraded then raised again: the token stays cut (review 2026-11-15 #19).
    for role in ["viewer", "manager"] {
        let (st, b) =
            s.e.call(
                "PUT",
                &format!("/v1/vaults/{}/bindings/user:u2", s.v),
                &s.admin,
                Some(&s.ad),
                Some(json!({ "role": role })),
            )
            .await;
        assert!(st.is_success(), "{b}");
        assert_eq!(
            s.read(&key, "/changes?since=0", INSIDE).await.0,
            StatusCode::UNAUTHORIZED,
            "after {role}"
        );
    }
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
    // A token issued under the binding as it is now dies with its deletion.
    let (st, b) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/tokens", s.v),
            &mgr,
            Some(&md),
            Some(json!({ "expiresInDays": 7, "cidrs": ["10.42.0.0/16"] })),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "no longer a manager: {b}");
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

// ---- RFC 0004: several folders, and write access ----

impl S {
    /// Sets `scopedTokens.write` on the tenant, as a reconciled document.
    async fn tenant_write(&self, on: bool) {
        let config = common::app::CONFIG.replacen(
            "  vaultNamespaces: [maxime]",
            &format!("  vaultNamespaces: [maxime]\n  scopedTokens: {{ write: {on} }}"),
            1,
        );
        let dir = std::env::temp_dir().join(format!("kura-http-{}", self.e.t.name));
        std::fs::write(dir.join("config.yaml"), config).unwrap();
        let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
        kura_server::reconcile::reconcile(&self.e.t.app, &files, true)
            .await
            .unwrap();
        self.e.state.set_snapshot(
            kura_server::reconcile::snapshot::load_if_changed(&self.e.t.app, None)
                .await
                .unwrap()
                .unwrap(),
        );
    }

    /// A token with these rules: its secret and its id.
    async fn scoped(&self, name: &str, scopes: Value) -> (String, Uuid) {
        let (st, b) = self
            .issue(json!({ "name": name, "scopes": scopes, "expiresInDays": 30, "cidrs": ["10.42.0.0/16"] }))
            .await;
        assert_eq!(st, StatusCode::CREATED, "{b}");
        (
            b["token"].as_str().unwrap().to_owned(),
            Uuid::parse_str(b["id"].as_str().unwrap()).unwrap(),
        )
    }

    async fn as_key(
        &self,
        key: &str,
        method: &str,
        path: &str,
        body: Option<Body>,
    ) -> (StatusCode, Value) {
        let uri = format!("/v1/vaults/{}{path}", self.v);
        let (st, b) = send(&self.e, method, &uri, key, None, body, INSIDE).await;
        (st, json_of(&b))
    }

    async fn key_commit(&self, key: &str, idem: &str, op: Value) -> (StatusCode, Value) {
        let body = json!({ "idempotencyKey": idem, "operations": [op] });
        self.as_key(key, "POST", "/commits", Some(Body::from(body.to_string())))
            .await
    }

    /// Uploads `data` with the token (chunk, then manifest) and commits it.
    async fn key_put(
        &self,
        key: &str,
        entry: &str,
        path: &str,
        data: &[u8],
        base: Option<&str>,
    ) -> (StatusCode, Value) {
        let c = ChunkHash::of(data);
        let (st, b) = self
            .as_key(
                key,
                "PUT",
                &format!("/chunks/{c}"),
                Some(Body::from(data.to_vec())),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{b}");
        let size = data.len() as u64;
        let blob = blob_hash(size, &[c]);
        let manifest = json!({ "size": size, "chunks": [c.to_string()] });
        let (st, b) = self
            .as_key(
                key,
                "PUT",
                &format!("/blobs/{blob}"),
                Some(Body::from(manifest.to_string())),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{b}");
        let mut op = json!({ "op": "put", "entry": entry, "path": path, "blob": blob.to_string(), "size": size });
        if let Some(base) = base {
            op["base"] = json!(base);
        }
        self.key_commit(key, &Uuid::new_v4().to_string(), op).await
    }

    async fn head_seq(&self) -> i64 {
        sqlx::query_scalar("SELECT head_seq FROM vault WHERE id = $1::uuid")
            .bind(&self.v)
            .fetch_one(&self.e.t.owner)
            .await
            .unwrap()
    }

    fn token_writes(&self, outcome: &str) -> u64 {
        self.e
            .state
            .metrics
            .token_writes_total
            .with_label_values(&[outcome])
            .get()
    }
}

fn outside_folders(st: StatusCode, b: &Value) -> bool {
    st == StatusCode::FORBIDDEN
        && problem(b) == "/v1/problems/insufficient_role"
        && b["detail"]
            .as_str()
            .is_some_and(|d| d.ends_with("outside the token's write folders"))
}

fn not_uploaded(st: StatusCode, b: &Value) -> bool {
    st == StatusCode::UNPROCESSABLE_ENTITY && b["detail"] == "content not uploaded by this token"
}

fn paths(p: &Value) -> Vec<&str> {
    let mut out: Vec<&str> = p["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    out.sort_unstable();
    out
}

/// RFC 0004 phase 1: a token with several read folders sees them, and only
/// them, on every read site; the rules are validated; what an older binary
/// would read of such a token matches no path (T8).
#[tokio::test]
async fn multi_folder_token_reads_its_folders_only() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (sess, pnj, mj) = (
        Uuid::now_v7().to_string(),
        Uuid::now_v7().to_string(),
        Uuid::now_v7().to_string(),
    );
    s.file(&sess, "Sessions/1.md", b"session one", None).await;
    s.file(&pnj, "PNJ/elf.md", b"an elf", None).await;
    s.file(&mj, "MJ/secret.md", b"the villain", None).await;

    // §4.3 through the API; the unit test has every row.
    for (body, why) in [
        (
            json!({ "scopes": [], "expiresInDays": 30, "cidrs": ["10.0.0.0/8"] }),
            "empty list",
        ),
        (
            json!({ "scopes": [{ "path": "", "mode": "write" }], "expiresInDays": 30, "cidrs": ["10.0.0.0/8"] }),
            "whole-vault write",
        ),
        (
            json!({ "prefix": "a/", "scopes": [{ "path": "a/", "mode": "read" }], "expiresInDays": 30, "cidrs": ["10.0.0.0/8"] }),
            "prefix and scopes",
        ),
        (
            json!({ "scopes": [{ "path": "a/", "mode": "admin" }], "expiresInDays": 30, "cidrs": ["10.0.0.0/8"] }),
            "unknown mode",
        ),
    ] {
        let (st, b) = s.issue(body).await;
        assert_eq!(
            (st, problem(&b)),
            (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request"),
            "{why}"
        );
    }
    let (st, b) = s
        .issue(json!({ "scopes": [{ "path": "Resumes/", "mode": "write" }], "expiresInDays": 30, "cidrs": ["10.0.0.0/8"] }))
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::CONFLICT, "/v1/problems/token_write_disabled"),
        "the tenant did not opt in"
    );

    let (key, id) = s
        .scoped(
            "reader",
            json!([{ "path": "Sessions/", "mode": "read" }, { "path": "pnj/", "mode": "read" }]),
        )
        .await;
    let (_, p) = s.as_key(&key, "GET", "/changes?since=0", None).await;
    assert_eq!(paths(&p), ["PNJ/elf.md", "Sessions/1.md"]);
    let (_, p) = s.as_key(&key, "GET", "/entries", None).await;
    assert_eq!(paths(&p), ["PNJ/elf.md", "Sessions/1.md"]);
    let (st, _) = s
        .as_key(&key, "GET", &format!("/entries/{mj}/revisions"), None)
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = s
        .as_key(&key, "GET", &format!("/entries/{pnj}/revisions"), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let mut asked = Vec::new();
    for (data, ok) in [
        (&b"session one"[..], true),
        (b"an elf", true),
        (b"the villain", false),
    ] {
        let c = ChunkHash::of(data);
        let blob = blob_hash(data.len() as u64, &[c]);
        let want = if ok {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        };
        let (st, _) = s.as_key(&key, "GET", &format!("/chunks/{c}"), None).await;
        assert_eq!(st, want, "chunk, visible: {ok}");
        let (st, _) = s.as_key(&key, "GET", &format!("/blobs/{blob}"), None).await;
        assert_eq!(st, want, "blob, visible: {ok}");
        asked.push(blob.to_string());
    }
    let (st, b) = s
        .as_key(
            &key,
            "POST",
            "/blobs/batch",
            Some(Body::from(json!({ "blobs": asked }).to_string())),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(
        b["missing"].as_array().unwrap().len(),
        1,
        "the batch hides the same blob: {b}"
    );

    // The list shows the name and the rules, never a secret.
    let (_, list) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/tokens", s.v),
            &s.admin,
            Some(&s.ad),
            None,
        )
        .await;
    let shown = list
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id.to_string())
        .unwrap();
    assert_eq!(
        (shown["name"].as_str(), shown["mode"].as_str()),
        (Some("reader"), Some("read"))
    );
    assert_eq!(
        shown["scopes"][1],
        json!({ "path": "pnj/", "mode": "read" })
    );

    // T8, `legacy_prefix_matches_no_path`: several rules store `/`, which no
    // path starts with; one read rule keeps its folder for an older binary.
    assert_eq!(shown["prefix"], "/");
    let under: i64 = sqlx::query_scalar("SELECT count(*) FROM entry WHERE path_key LIKE '/%'")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(under, 0);
    let single = s.key("Sessions/").await;
    let stored: (String, Value) =
        sqlx::query_as("SELECT path_prefix, scopes FROM scoped_token WHERE id = $1")
            .bind(Uuid::parse_str(&single[5..37]).unwrap())
            .fetch_one(&s.e.t.owner)
            .await
            .unwrap();
    assert_eq!(
        stored,
        (
            "Sessions/".to_owned(),
            json!([{ "path": "Sessions/", "mode": "read" }])
        )
    );
    // A row inserted by a binary from before the migration gets its one rule.
    let old: (Value, String) = sqlx::query_as(
        "INSERT INTO scoped_token (id, tenant_id, vault_id, path_prefix, allowed_cidrs, expires_at, token_hash, created_by)
         SELECT gen_random_uuid(), tenant_id, id, 'old/', '{10.0.0.0/8}', now() + interval '1 day', sha256('x'), 'boss' FROM vault WHERE id = $1::uuid
         RETURNING scopes, name",
    )
    .bind(&s.v)
    .fetch_one(&s.e.t.owner)
    .await
    .unwrap();
    assert_eq!(old.0, json!([{ "path": "old/", "mode": "read" }]));
    assert!(old.1.starts_with("key-"));

    // T7: rules that no longer decode refuse the token; they never widen it.
    sqlx::query("UPDATE scoped_token SET scopes = '[{\"path\": \"Sessions/\", \"mode\": \"all\"}]' WHERE id = $1")
        .bind(id)
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let (st, b) = s.as_key(&key, "GET", "/changes?since=0", None).await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/invalid_token")
    );
}

/// RFC 0004 T1, T5, T9, T10 and the tenant switch: a token writes under its
/// `write` folders and nowhere else, whatever names the outside entry; what
/// it wrote is ordinary, attributed history.
#[tokio::test]
async fn token_cannot_write_outside() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    s.tenant_write(true).await;
    let (sess, mj) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    s.file(&sess, "Sessions/1.md", b"session one", None).await;
    let secret = s.file(&mj, "MJ/secret.md", b"the villain", None).await;
    let (key, id) = s
        .scoped("discord-recap", json!([{ "path": "Sessions/", "mode": "read" }, { "path": "Resumes/", "mode": "write" }]))
        .await;

    // The success: a recap, attributed to the key, with the token as device.
    let recap = Uuid::now_v7().to_string();
    let (st, b) = s
        .key_put(&key, &recap, "Resumes/1.md", b"recap one", None)
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let head = b["results"][0]["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let by: (String, Uuid) =
        sqlx::query_as("SELECT user_sub, device_id FROM revision WHERE id = $1::uuid")
            .bind(&head)
            .fetch_one(&s.e.t.owner)
            .await
            .unwrap();
    assert_eq!(
        by,
        (format!("key:{id}"), id),
        "token_revision_names_the_key"
    );
    assert_eq!(s.token_writes("applied"), 1);
    // It reads back what it wrote: `write` reads its own folder.
    let (_, p) = s.as_key(&key, "GET", "/changes?since=0", None).await;
    assert_eq!(paths(&p), ["Resumes/1.md", "Sessions/1.md"]);

    // An idempotent retry, by token id, is the stored answer.
    let op = json!({ "op": "delete", "entry": recap, "base": head });
    let (st, first) = s.key_commit(&key, "retry-1", op.clone()).await;
    assert_eq!(st, StatusCode::OK, "{first}");
    let (st, again) = s.key_commit(&key, "retry-1", op).await;
    assert_eq!((st, &again), (StatusCode::OK, &first));
    // T5: the deletion is a revision; the content before it is still history.
    let (_, r) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/entries/{recap}/revisions", s.v),
            &s.admin,
            Some(&s.ad),
            None,
        )
        .await;
    assert_eq!(
        r["items"].as_array().unwrap().len(),
        2,
        "token_delete_is_restorable: {r}"
    );

    // T1, four ways out, and the read-only folder. Nothing moves.
    let live = Uuid::now_v7().to_string();
    let (st, b) = s
        .key_put(&key, &live, "Resumes/2.md", b"recap two", None)
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let live_head = b["results"][0]["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = (s.head_seq().await, s.token_writes("refused"));
    let secret_rev = secret["revision"].as_str().unwrap();
    for (entry, path, base, why) in [
        (
            Uuid::now_v7().to_string(),
            "MJ/x.md",
            None,
            "a new path outside",
        ),
        (
            live.clone(),
            "MJ/2.md",
            Some(live_head.as_str()),
            "a rename out",
        ),
        (
            mj.clone(),
            "Resumes/stolen.md",
            Some(secret_rev),
            "an outside entry moved in by its id",
        ),
        (
            Uuid::now_v7().to_string(),
            "Sessions/x.md",
            None,
            "a folder it only reads",
        ),
        (
            Uuid::now_v7().to_string(),
            "resumes2/x.md",
            None,
            "a sibling of its folder",
        ),
    ] {
        let (st, b) = s.key_put(&key, &entry, path, b"recap two", base).await;
        assert!(outside_folders(st, &b), "{why}: {st} {b}");
    }
    let (st, b) = s
        .key_commit(
            &key,
            "del-mj",
            json!({ "op": "delete", "entry": mj, "base": secret_rev }),
        )
        .await;
    assert!(outside_folders(st, &b), "delete by id: {st} {b}");
    // One operation outside refuses the commit whole.
    let both = json!({ "idempotencyKey": "both", "operations": [
        { "op": "delete", "entry": live, "base": live_head },
        { "op": "delete", "entry": mj, "base": secret_rev },
    ]});
    let (st, b) = s
        .as_key(&key, "POST", "/commits", Some(Body::from(both.to_string())))
        .await;
    assert!(outside_folders(st, &b), "{st} {b}");
    assert_eq!(s.head_seq().await, before.0, "nothing was applied");
    assert_eq!(s.token_writes("refused"), before.1 + 7);
    let still: String = sqlx::query_scalar("SELECT path FROM entry WHERE id = $1::uuid")
        .bind(&mj)
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(still, "MJ/secret.md");

    // A token never restores, and never reaches another vault.
    let (st, _) = s
        .as_key(
            &key,
            "POST",
            &format!("/entries/{live}/restore"),
            Some(Body::from("{}")),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let other = vault_id(&s.e, "secrets").await;
    let (st, _) = send(
        &s.e,
        "POST",
        &format!("/v1/vaults/{other}/commits"),
        &key,
        None,
        Some(Body::from("{}")),
        INSIDE,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // The switch off: writes stop, reads go on, nothing is deleted.
    s.tenant_write(false).await;
    let (st, b) = s
        .key_commit(
            &key,
            "off",
            json!({ "op": "delete", "entry": live, "base": live_head }),
        )
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role"),
        "{b}"
    );
    assert_eq!(s.token_writes("disabled"), 1);
    // §4.3: the operator sees it, on the tenant, in the admin status.
    let root =
        s.e.key
            .token("root", "sr", &["maxime-owners", "kura-platform-admins"]);
    let rd = s.e.register(&root, "console").await;
    let (_, status) =
        s.e.call("GET", "/v1/admin/status", &root, Some(&rd), None)
            .await;
    let tenant = status["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["name"] == "maxime")
        .unwrap();
    assert_eq!(
        tenant["conditions"][1]["type"], "TokenWriteDisabled",
        "{tenant}"
    );
    let (st, _) = s.as_key(&key, "GET", "/changes?since=0", None).await;
    assert_eq!(st, StatusCode::OK);
    s.tenant_write(true).await;

    // T10, `revoked_grant_cuts_write_token`: issued through the admin claim
    // value, the token dies when the tenant stops mapping it.
    sqlx::query("UPDATE scoped_token SET granted_by_claim = 'no-longer-mapped' WHERE id = $1")
        .bind(id)
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let (st, b) = s
        .key_commit(
            &key,
            "cut",
            json!({ "op": "delete", "entry": live, "base": live_head }),
        )
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::UNAUTHORIZED, "/v1/problems/invalid_token")
    );
}

/// RFC 0004 T2, T3, T6: a hash is not a capability. A write token names
/// only content it can read or uploaded itself, and learns nothing about
/// the rest: stored outside its folders or not, the answer is the same.
#[tokio::test]
async fn token_cannot_launder_content() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    s.tenant_write(true).await;
    let mj = Uuid::now_v7().to_string();
    s.file(&mj, "MJ/secret.md", b"the villain", None).await;
    let (key, id) = s
        .scoped("bot", json!([{ "path": "Resumes/", "mode": "write" }]))
        .await;
    let secret = ChunkHash::of(b"the villain");
    let secret_blob = blob_hash(11, &[secret]);
    let nowhere = ChunkHash::of(b"stored nowhere");
    let touched = |h: ChunkHash| {
        let owner = s.e.t.owner.clone();
        async move {
            sqlx::query_scalar::<_, time::OffsetDateTime>(
                "SELECT last_touched FROM chunk WHERE hash = $1",
            )
            .bind(h.as_bytes().as_slice())
            .fetch_one(&owner)
            .await
            .unwrap()
        }
    };
    let before = touched(secret).await;

    // `missing_is_blind_outside_scope` (T3): both hashes are "missing".
    let ask = json!({ "hashes": [secret.to_string(), nowhere.to_string()] });
    let (st, b) = s
        .as_key(
            &key,
            "POST",
            "/chunks/missing",
            Some(Body::from(ask.to_string())),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["missing"].as_array().unwrap().len(), 2, "{b}");
    assert_eq!(
        touched(secret).await,
        before,
        "an unknown chunk is not kept from collection"
    );

    // `token_cannot_launder_chunk` (T2): a manifest naming the outside chunk
    // is refused exactly as one naming a chunk stored nowhere.
    let mut answers = Vec::new();
    for (chunk, size) in [(secret, 11u64), (nowhere, 14)] {
        let blob = blob_hash(size, &[chunk]);
        let manifest = json!({ "size": size, "chunks": [chunk.to_string()] });
        let (st, b) = s
            .as_key(
                &key,
                "PUT",
                &format!("/blobs/{blob}"),
                Some(Body::from(manifest.to_string())),
            )
            .await;
        assert!(not_uploaded(st, &b), "{st} {b}");
        answers.push((st, b["type"].clone(), b["detail"].clone()));
    }
    assert_eq!(answers[0], answers[1]);
    // `token_cannot_launder_blob` (T2): nor a commit naming the outside blob.
    let stolen = Uuid::now_v7().to_string();
    let op = json!({ "op": "put", "entry": stolen, "path": "Resumes/stolen.md", "blob": secret_blob.to_string(), "size": 11 });
    let (st, b) = s.key_commit(&key, "steal", op).await;
    assert!(not_uploaded(st, &b), "{st} {b}");
    let (st, _) = s
        .as_key(&key, "GET", &format!("/chunks/{secret}"), None)
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(s.token_writes("refused"), 3);

    // Sending the bytes is the proof: the same content, uploaded by the token.
    let (st, b) = s
        .key_put(&key, &stolen, "Resumes/same.md", b"the villain", None)
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let (_, b) = s
        .as_key(
            &key,
            "POST",
            "/chunks/missing",
            Some(Body::from(ask.to_string())),
        )
        .await;
    assert_eq!(b["missing"], json!([nowhere.to_string()]));

    // `token_upload_is_purged` (T6): remembered 24 hours, gone with the token.
    let rows = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM token_upload WHERE token_id = $1")
            .bind(id)
            .fetch_one(&s.e.t.owner)
            .await
            .unwrap()
    };
    assert_eq!(rows().await, 2, "one chunk, one blob");
    sqlx::query(
        "UPDATE token_upload SET created_at = now() - interval '25 hours' WHERE kind = 'chunk'",
    )
    .execute(&s.e.t.owner)
    .await
    .unwrap();
    let purged: i64 = sqlx::query_scalar("SELECT kura_token_upload_purge()")
        .fetch_one(&s.e.t.app)
        .await
        .unwrap();
    assert_eq!((purged, rows().await), (1, 1));
    sqlx::query("DELETE FROM scoped_token WHERE id = $1")
        .bind(id)
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(rows().await, 0);
}

/// RFC 0004 T4, T6: a write token passes authorization on the read routes
/// and the four write routes, and on no other route of the API; its writes
/// count against its own limit.
#[tokio::test]
async fn only_writer_routes_accept_tokens() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    s.tenant_write(true).await;
    let (key, _) = s
        .scoped("bot", json!([{ "path": "Resumes/", "mode": "write" }]))
        .await;
    let (id, hash) = (Uuid::now_v7().to_string(), ChunkHash::of(b"x").to_string());
    let mut open = Vec::new();
    for (method, route, _) in kura_server::limits::LIMITS {
        // Opened with a ticket in the query, never a bearer; the ticket
        // route below refuses the token.
        if *route == "/v1/vaults/{v}/events" {
            continue;
        }
        let uri = route
            .replace("{v}", &s.v)
            .replace("{hash}", &hash)
            .replace("{d}", &id)
            .replace("{b}", &id)
            .replace("{e}", &id)
            .replace("{t}", &id);
        let (st, b) = send(
            &s.e,
            method,
            &uri,
            &key,
            None,
            Some(Body::from("{}")),
            INSIDE,
        )
        .await;
        let refused = st == StatusCode::UNAUTHORIZED
            || (st == StatusCode::FORBIDDEN
                && json_of(&b)["detail"] == "scoped tokens are read only");
        if !refused {
            open.push(format!("{method} {route}"));
        }
    }
    open.sort();
    let mut want = vec![
        // `Reader`.
        "GET /v1/vaults",
        "GET /v1/vaults/{v}/blobs/{hash}",
        "GET /v1/vaults/{v}/changes",
        "GET /v1/vaults/{v}/chunks/{hash}",
        "GET /v1/vaults/{v}/entries",
        "GET /v1/vaults/{v}/entries/{e}/revisions",
        "POST /v1/vaults/{v}/blobs/batch",
        "POST /v1/vaults/{v}/chunks/batch",
        "POST /v1/vaults/{v}/search",
        // `Writer`: the four routes of RFC 0004 §4.4.
        "POST /v1/vaults/{v}/chunks/missing",
        "POST /v1/vaults/{v}/commits",
        "PUT /v1/vaults/{v}/blobs/{hash}",
        "PUT /v1/vaults/{v}/chunks/{hash}",
    ];
    want.sort_unstable();
    assert_eq!(open, want);

    // `token_writes_are_rate_limited`: the commit route's 120 a minute.
    let mut last = StatusCode::OK;
    for _ in 0..121 {
        (last, _) = s
            .as_key(&key, "POST", "/commits", Some(Body::from("{}")))
            .await;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}
