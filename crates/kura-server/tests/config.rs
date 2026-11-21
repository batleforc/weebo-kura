//! Obsidian configuration spaces (RFC 0001 §6.8, decision 34, the
//! configuration half of T14). Opt-in: as tests/tokens.rs.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use axum::body::Body;
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
    e.apply_config(&docs).await;
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
        self.e.upload(&d.tok, &d.id, &self.v, data).await
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

async fn seed_and_list(s: &S, a: &D, a_phone: &D, b: &D) -> String {
    let plugin = Uuid::now_v7().to_string();
    let (st, r) = s
        .put(
            a,
            &plugin,
            "plugins/dataview/main.js",
            "module.exports = 1;",
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let (st, _) = s
        .put(
            a,
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
            .put(a, &Uuid::now_v7().to_string(), path, "{}", None)
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
        s.paths(a, "config").await,
        ["plugins/dataview/main.js", "snippets/wide.css"]
    );
    assert!(
        s.paths(a, "notes").await.is_empty(),
        "never among the notes"
    );
    assert!(
        s.paths(a_phone, "config").await.is_empty(),
        "another profile"
    );
    assert!(s.paths(b, "config").await.is_empty(), "another member");
    assert!(s.paths(b, "notes").await.is_empty());
    plugin
}

async fn hidden_by_address(s: &S, a: &D, b: &D) {
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
    assert_eq!(get(a).await, StatusCode::OK);
    assert_eq!(get(b).await, StatusCode::NOT_FOUND);
    // RFC 0002 T2: nor in a batch, where it reads as an unknown hash.
    let batch = |d: &D| {
        let uri = format!("/v1/vaults/{}/chunks/batch", s.v);
        let (tok, id) = (d.tok.clone(), d.id.clone());
        let state = s.e.state.clone();
        let body = json!({ "chunks": [c.to_string()] }).to_string();
        async move {
            let req = Request::post(uri)
                .header("authorization", format!("Bearer {tok}"))
                .header("x-kura-device", id)
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap();
            let r = app(state).oneshot(req).await.unwrap();
            axum::body::to_bytes(r.into_body(), 1 << 20)
                .await
                .unwrap()
                .to_vec()
        }
    };
    assert_eq!(batch(a).await[32], 0, "the owner gets it");
    let theirs = batch(b).await;
    assert_eq!(theirs.len(), 37, "batch_hides_other_members_config");
    assert_eq!(theirs[32..], [1, 0, 0, 0, 0]);
}

async fn no_overwrite_by_id(s: &S, b: &D, plugin: &str) {
    // Another member cannot write over it by naming its id.
    let (bb, size) = s.upload(b, b"evil").await;
    for space in [Some("config"), None] {
        let (st, body) = s
            .commit(b, space, json!([{ "op": "put", "entry": plugin, "path": "plugins/dataview/main.js", "blob": bb, "size": size }]))
            .await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{space:?}: {body}");
    }
}

async fn scoped_token_sees_notes_only(s: &S, a: &D) {
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
        let (st, _) =
            s.e.call_peer(
                "10.1.1.1:1",
                "GET",
                &format!("/v1/vaults/{}/changes?since=0&{q}", s.v),
                key,
                None,
            )
            .await;
        assert_eq!(st, want, "{q}");
    }
    // The vault tells clients what syncs.
    let (_, vaults) =
        s.e.call("GET", "/v1/vaults", &a.tok, Some(&a.id), None)
            .await;
    assert_eq!(vaults[0]["configSync"], json!(["plugins", "snippets"]));
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
    let plugin = seed_and_list(&s, &a, &a_phone, &b).await;
    hidden_by_address(&s, &a, &b).await;
    no_overwrite_by_id(&s, &b, &plugin).await;
    scoped_token_sees_notes_only(&s, &a).await;
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

impl S {
    async fn get(&self, d: &D, uri: &str) -> (StatusCode, Value) {
        self.e
            .call(
                "GET",
                &format!("/v1/vaults/{}/{uri}", self.v),
                &d.tok,
                Some(&d.id),
                None,
            )
            .await
    }

    /// The `entries` items of a space, by path.
    async fn listed(&self, d: &D, query: &str) -> std::collections::BTreeMap<String, Value> {
        let (st, b) = self.get(d, &format!("entries?{query}")).await;
        assert_eq!(st, StatusCode::OK, "{query}: {b}");
        b["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i["path"].as_str().unwrap().to_owned(), i.clone()))
            .collect()
    }
}

async fn listed_by_profile(s: &S, desk: &D, web: &D) -> std::collections::BTreeMap<String, Value> {
    // Listed by profile, the hidden file without its hash.
    let items = s.listed(web, "space=config&profile=desktop").await;
    assert_eq!(items.len(), 3, "{items:?}");
    assert!(items["snippets/wide.css"]["blob"].is_string());
    assert!(items["plugins/dataview/manifest.json"]["blob"].is_string());
    assert!(items["plugins/dataview/data.json"].get("blob").is_none());
    assert!(
        s.listed(web, "space=config&profile=mobile")
            .await
            .is_empty()
    );
    for (q, want) in [
        ("entries?space=config", StatusCode::BAD_REQUEST),
        (
            "entries?space=config&profile=tablet",
            StatusCode::BAD_REQUEST,
        ),
        ("entries?space=config&profile=web", StatusCode::BAD_REQUEST),
        ("changes?since=0&space=config", StatusCode::FORBIDDEN),
    ] {
        assert_eq!(s.get(web, q).await.0, want, "{q}");
    }
    // T6: an Obsidian device names its own profile only.
    assert_eq!(
        s.listed(desk, "space=config&profile=desktop").await.len(),
        3
    );
    assert_eq!(
        s.get(desk, "entries?space=config&profile=mobile").await.0,
        StatusCode::BAD_REQUEST
    );
    items
}

async fn content_of_viewable_only(
    s: &S,
    (desk, web): (&D, &D),
    items: &std::collections::BTreeMap<String, Value>,
    (data_blob, data_chunk): (&str, &str),
) -> String {
    // T2: content of the viewable file only; the hidden one as a random hash.
    let snippet_blob = items["snippets/wide.css"]["blob"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        s.get(web, &format!("blobs/{snippet_blob}")).await.0,
        StatusCode::OK
    );
    let hidden = s.get(web, &format!("blobs/{data_blob}")).await;
    let random = s.get(web, &format!("blobs/{}", "ab".repeat(32))).await;
    assert_eq!(hidden, random, "a hidden file reads as an unknown hash");
    assert_eq!(hidden.0, StatusCode::NOT_FOUND);
    assert_eq!(
        s.get(web, &format!("chunks/{data_chunk}")).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        s.get(desk, &format!("blobs/{data_blob}")).await.0,
        StatusCode::OK
    );
    let (st, batch) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/blobs/batch", s.v),
            &web.tok,
            Some(&web.id),
            Some(json!({ "blobs": [data_blob, snippet_blob] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{batch}");
    // Missing, as an unknown hash; only the viewable file has a manifest.
    assert_eq!(batch["missing"], json!([data_blob]), "{batch}");
    assert_eq!(
        batch["manifests"][0]["hash"],
        json!(snippet_blob),
        "{batch}"
    );
    let req = Request::post(format!("/v1/vaults/{}/chunks/batch", s.v))
        .header("authorization", format!("Bearer {}", web.tok))
        .header("x-kura-device", &web.id)
        .header("content-type", "application/json")
        .body(Body::from(json!({ "chunks": [data_chunk] }).to_string()))
        .unwrap();
    let r = app(s.e.state.clone()).oneshot(req).await.unwrap();
    let records = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    assert_eq!(
        records[32..],
        [1, 0, 0, 0, 0],
        "a hidden chunk reads as missing"
    );
    snippet_blob
}

async fn hidden_history(s: &S, desk: &D, web: &D, snippet: &str, data: &str) {
    // History: the hidden file's revisions carry no hash for the console.
    let (_, revs) = s.get(web, &format!("entries/{data}/revisions")).await;
    assert!(
        revs["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i.get("blob").is_none()),
        "{revs}"
    );
    let (_, revs) = s.get(web, &format!("entries/{snippet}/revisions")).await;
    assert!(revs["items"][0]["blob"].is_string(), "{revs}");
    let (_, revs) = s.get(desk, &format!("entries/{data}/revisions")).await;
    assert!(
        revs["items"][0]["blob"].is_string(),
        "the owner's device keeps it"
    );
}

async fn console_writes_nothing(
    s: &S,
    (desk, web, phone): (&D, &D, &D),
    snippet: &str,
    data_blob: &str,
    size: u64,
) {
    // T3, T4: no content write from the console, not even a note on the hash.
    let note = json!([{ "op": "put", "entry": Uuid::now_v7().to_string(), "path": "leak.md", "blob": data_blob, "size": size }]);
    let (st, b) = s.commit(web, None, note).await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role"),
        "{b}"
    );
    let (_, revs) = s.get(desk, &format!("entries/{snippet}/revisions")).await;
    let first = revs["items"][0]["revision"].as_str().unwrap().to_owned();
    for d in [web, phone] {
        let (st, b) =
            s.e.call(
                "POST",
                &format!("/v1/vaults/{}/entries/{snippet}/restore", s.v),
                &d.tok,
                Some(&d.id),
                Some(json!({ "revision": first })),
            )
            .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "restore refused: {b}");
    }
    let chunk = ChunkHash::of(b"x").to_string();
    let req = Request::put(format!("/v1/vaults/{}/chunks/{chunk}", s.v))
        .header("authorization", format!("Bearer {}", web.tok))
        .header("x-kura-device", &web.id)
        .body(Body::from(b"x".to_vec()))
        .unwrap();
    assert_eq!(
        app(s.e.state.clone()).oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

async fn other_members_console(s: &S, snippet_blob: &str) {
    // T1: another member's console sees none of it.
    let other = s.device("u2", "web").await;
    assert!(
        s.listed(&other, "space=config&profile=desktop")
            .await
            .is_empty()
    );
    assert_eq!(
        s.get(&other, &format!("blobs/{snippet_blob}")).await.0,
        StatusCode::NOT_FOUND
    );
}

/// RFC 0008 T1, T2, T4, T6: the console lists its owner's configuration by
/// profile, reads what holds no secret, is never given a hidden file's hash
/// or content, and writes nothing.
#[tokio::test]
async fn a_web_device_lists_and_reads_only_what_holds_no_secret() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, web, phone) = (
        s.device("u1", "desktop").await,
        s.device("u1", "web").await,
        s.device("u1", "mobile").await,
    );
    let secret = format!("{{\"token\":\"s3cret-{}\"}}", Uuid::new_v4());
    let (snippet, data) = (Uuid::now_v7().to_string(), Uuid::now_v7().to_string());
    for (entry, path, text) in [
        (snippet.as_str(), "snippets/wide.css", ".x{}"),
        (data.as_str(), "plugins/dataview/data.json", secret.as_str()),
        (
            &Uuid::now_v7().to_string(),
            "plugins/dataview/manifest.json",
            "{\"name\":\"Dataview\"}",
        ),
    ] {
        let (st, r) = s.put(&desk, entry, path, text, None).await;
        assert_eq!(st, StatusCode::OK, "{path}: {r}");
    }
    let data_blob = blob_hash(secret.len() as u64, &[ChunkHash::of(secret.as_bytes())]).to_string();
    let data_chunk = ChunkHash::of(secret.as_bytes()).to_string();
    let items = listed_by_profile(&s, &desk, &web).await;
    let snippet_blob =
        content_of_viewable_only(&s, (&desk, &web), &items, (&data_blob, &data_chunk)).await;
    hidden_history(&s, &desk, &web, &snippet, &data).await;
    console_writes_nothing(
        &s,
        (&desk, &web, &phone),
        &snippet,
        &data_blob,
        secret.len() as u64,
    )
    .await;
    other_members_console(&s, &snippet_blob).await;
}

/// RFC 0008 §5.2: hidden revisions sharing a note's bytes never crowd the
/// note out of the 200-row window the content check reads.
#[tokio::test]
async fn a_note_sharing_bytes_with_hidden_configuration_stays_readable() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, web) = (s.device("u1", "desktop").await, s.device("u1", "web").await);
    let (b, size) = s.upload(&desk, b"{}").await;
    let ops: Vec<Value> = (0..201)
        .map(|i| json!({ "op": "put", "entry": Uuid::now_v7().to_string(), "path": format!("plugins/p{i}/data.json"), "blob": b, "size": size }))
        .collect();
    let (st, r) = s.commit(&desk, Some("config"), Value::Array(ops)).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let (st, r) = s
        .commit(&desk, None, json!([{ "op": "put", "entry": Uuid::now_v7().to_string(), "path": "n.md", "blob": b, "size": size }]))
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(s.get(&web, &format!("blobs/{b}")).await.0, StatusCode::OK);
}
