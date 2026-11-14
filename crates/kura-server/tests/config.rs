//! Obsidian configuration spaces (RFC 0001 §6.8, decision 34, the
//! configuration half of T14). Opt-in: as tests/tokens.rs.

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
use common::app::{CONFIG, Env, env_with, problem, storage, vault_id};
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

struct S {
    e: Env,
    v: String,
}

/// One device: its token and id.
struct D {
    tok: String,
    id: String,
}

async fn setup(test: &str) -> Option<S> {
    let st = storage(test).await?;
    let e = env_with(test, Some(st)).await?;
    // knowledge syncs plugins and snippets, not hotkeys.
    let docs = CONFIG.replace(
        "  tenantRef: { name: maxime }\n  bindings:",
        "  tenantRef: { name: maxime }\n  configSync: { categories: [plugins, snippets] }\n  bindings:",
    );
    let dir = std::env::temp_dir().join(format!("kura-config-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
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
    Some(S { e, v })
}

impl S {
    async fn device(&self, sub: &str, profile: &str) -> D {
        let tok = self.e.key.token(
            sub,
            &format!("s-{sub}-{profile}-{}", Uuid::new_v4()),
            &["maxime-members"],
        );
        let (st, b) = self
            .e
            .call(
                "POST",
                "/v1/devices",
                &tok,
                None,
                Some(json!({ "name": format!("{sub}-{profile}"), "profile": profile })),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED, "{b}");
        D {
            id: b["id"].as_str().unwrap().to_owned(),
            tok,
        }
    }

    async fn upload(&self, d: &D, data: &[u8]) -> (String, u64) {
        let c = ChunkHash::of(data).to_string();
        let req = Request::put(format!("/v1/vaults/{}/chunks/{c}", self.v))
            .header("authorization", format!("Bearer {}", d.tok))
            .header("x-kura-device", &d.id)
            .body(Body::from(data.to_vec()))
            .unwrap();
        assert!(
            app(self.e.state.clone())
                .oneshot(req)
                .await
                .unwrap()
                .status()
                .is_success()
        );
        let size = data.len() as u64;
        let b = blob_hash(size, &[ChunkHash::of(data)]).to_string();
        let (st, body) = self
            .e
            .call(
                "PUT",
                &format!("/v1/vaults/{}/blobs/{b}", self.v),
                &d.tok,
                Some(&d.id),
                Some(json!({ "size": size, "chunks": [c] })),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{body}");
        (b, size)
    }

    async fn commit(&self, d: &D, space: Option<&str>, ops: Value) -> (StatusCode, Value) {
        let mut body = json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": ops });
        if let Some(s) = space {
            body["space"] = json!(s);
        }
        self.e
            .call(
                "POST",
                &format!("/v1/vaults/{}/commits", self.v),
                &d.tok,
                Some(&d.id),
                Some(body),
            )
            .await
    }

    /// Puts `text` at `path` in the configuration space; returns the result.
    async fn put(
        &self,
        d: &D,
        entry: &str,
        path: &str,
        text: &str,
        base: Option<&str>,
    ) -> (StatusCode, Value) {
        let (b, size) = self.upload(d, text.as_bytes()).await;
        let mut op = json!({ "op": "put", "entry": entry, "path": path, "blob": b, "size": size });
        if let Some(base) = base {
            op["base"] = json!(base);
        }
        self.commit(d, Some("config"), json!([op])).await
    }

    async fn paths(&self, d: &D, space: &str) -> Vec<String> {
        let (st, b) = self
            .e
            .call(
                "GET",
                &format!("/v1/vaults/{}/changes?since=0&space={space}", self.v),
                &d.tok,
                Some(&d.id),
                None,
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{b}");
        let mut p: Vec<String> = b["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["path"].as_str().unwrap().to_owned())
            .collect();
        p.sort();
        p
    }
}

/// T14: a configuration space is its owner's, for one profile; another
/// member, a scoped token and the other profile see nothing of it, not even
/// its content by address; categories and the Kura plugin are enforced.
#[tokio::test]
async fn config_namespace_hidden_from_other_member() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (a, a_phone, b) = (
        s.device("u1", "desktop").await,
        s.device("u1", "mobile").await,
        s.device("u2", "desktop").await,
    );
    let plugin = Uuid::now_v7().to_string();
    let (st, r) = s
        .put(
            &a,
            &plugin,
            "plugins/dataview/main.js",
            "module.exports = 1;",
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let (st, _) = s
        .put(
            &a,
            &Uuid::now_v7().to_string(),
            "snippets/wide.css",
            ".x{}",
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    for path in [
        "hotkeys.json",
        "plugins/weebo-kura/main.js",
        "community-plugins.json",
        "workspace.json",
    ] {
        let (st, b) = s
            .put(&a, &Uuid::now_v7().to_string(), path, "{}", None)
            .await;
        assert_eq!(
            (st, problem(&b)),
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                "/v1/problems/invalid_path"
            ),
            "{path}"
        );
    }
    assert_eq!(
        s.paths(&a, "config").await,
        ["plugins/dataview/main.js", "snippets/wide.css"]
    );
    assert!(
        s.paths(&a, "notes").await.is_empty(),
        "never among the notes"
    );
    assert!(
        s.paths(&a_phone, "config").await.is_empty(),
        "another profile"
    );
    assert!(s.paths(&b, "config").await.is_empty(), "another member");
    assert!(s.paths(&b, "notes").await.is_empty());
    // Not by content address either.
    let c = ChunkHash::of(b"module.exports = 1;");
    let get = |d: &D| {
        let uri = format!("/v1/vaults/{}/chunks/{c}", s.v);
        let (tok, id) = (d.tok.clone(), d.id.clone());
        let state = s.e.state.clone();
        async move {
            let req = Request::get(uri)
                .header("authorization", format!("Bearer {tok}"))
                .header("x-kura-device", id)
                .body(Body::empty())
                .unwrap();
            app(state).oneshot(req).await.unwrap().status()
        }
    };
    assert_eq!(get(&a).await, StatusCode::OK);
    assert_eq!(get(&b).await, StatusCode::NOT_FOUND);
    // Another member cannot write over it by naming its id.
    let (bb, size) = s.upload(&b, b"evil").await;
    for space in [Some("config"), None] {
        let (st, body) = s
            .commit(&b, space, json!([{ "op": "put", "entry": plugin, "path": "plugins/dataview/main.js", "blob": bb, "size": size }]))
            .await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{space:?}: {body}");
    }
    // A scoped token: notes only, never configuration.
    let admin = s.e.key.token("boss", "sb", &["maxime-admins"]);
    let ad = s.e.register(&admin, "admin-desk").await;
    let (_, t) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/tokens", s.v),
            &admin,
            Some(&ad),
            Some(json!({ "expiresInDays": 1, "cidrs": ["10.0.0.0/8"] })),
        )
        .await;
    let key = t["token"].as_str().unwrap();
    for (q, want) in [
        ("space=config", StatusCode::FORBIDDEN),
        ("space=notes", StatusCode::OK),
    ] {
        let mut req = Request::get(format!("/v1/vaults/{}/changes?since=0&{q}", s.v))
            .header("authorization", format!("Bearer {key}"))
            .body(Body::empty())
            .unwrap();
        req.extensions_mut()
            .insert(ConnectInfo("10.1.1.1:1".parse::<SocketAddr>().unwrap()));
        assert_eq!(
            app(s.e.state.clone()).oneshot(req).await.unwrap().status(),
            want,
            "{q}"
        );
    }
    // The vault tells clients what syncs.
    let (_, vaults) =
        s.e.call("GET", "/v1/vaults", &a.tok, Some(&a.id), None)
            .await;
    assert_eq!(vaults[0]["configSync"], json!(["plugins", "snippets"]));
}

/// §6.5: configuration never conflicts; the last write wins, the earlier
/// version stays in history, and a new id on a held path writes onto it.
#[tokio::test]
async fn configuration_writes_last_wins_without_conflict_files() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, laptop) = (
        s.device("u1", "desktop").await,
        s.device("u1", "desktop").await,
    );
    let e = Uuid::now_v7().to_string();
    let (_, r) = s.put(&desk, &e, "snippets/a.css", "one", None).await;
    let first = r["results"][0]["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let (_, r) = s
        .put(&desk, &e, "snippets/a.css", "two", Some(&first))
        .await;
    assert_eq!(r["results"][0]["outcome"], "accepted");
    // The laptop still on the first version: its edit wins, no conflict file.
    let (st, r) = s
        .put(&laptop, &e, "snippets/a.css", "three", Some(&first))
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["results"][0]["outcome"], "accepted");
    assert!(r["results"][0].get("conflict").is_none());
    // A new id for the same path writes onto that entry.
    let (_, r) = s
        .put(
            &laptop,
            &Uuid::now_v7().to_string(),
            "snippets/a.css",
            "four",
            None,
        )
        .await;
    assert_eq!(r["results"][0]["head"]["entry"], e.as_str());
    assert_eq!(s.paths(&desk, "config").await, ["snippets/a.css"]);
    let (_, revs) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/entries/{e}/revisions", s.v),
            &desk.tok,
            Some(&desk.id),
            None,
        )
        .await;
    assert_eq!(
        revs["items"].as_array().unwrap().len(),
        4,
        "history kept: {revs}"
    );
    // The overwritten version is restorable by its owner (phase 8 review H1).
    let (st, r) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/entries/{e}/restore", s.v),
            &desk.tok,
            Some(&desk.id),
            Some(json!({ "revision": first })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["head"]["path"], "snippets/a.css");
    let other = s.device("u2", "desktop").await;
    let (st, _) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/entries/{e}/restore", s.v),
            &other.tok,
            Some(&other.id),
            Some(json!({ "revision": first })),
        )
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND, "nobody else's");
}
