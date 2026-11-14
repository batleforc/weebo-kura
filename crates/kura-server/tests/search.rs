//! Opt-in search end to end (RFC 0001 §6.7, decision 39): off by default,
//! backfilled when enabled, French and English with accents folded, file
//! names by trigram, prefix-bound for tokens, dropped when disabled; a job
//! that keeps failing is kept after 5 attempts. Opt-in: as tests/tokens.rs.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use common::app::{CONFIG, Env, env_with, problem, storage, vault_id};
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use kura_server::app::search::round;
use kura_server::store::Storage;
use object_store::ObjectStoreExt as _;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

struct S {
    e: Env,
    st: Arc<Storage>,
    tok: String,
    d: String,
    v: String,
}

async fn setup(test: &str) -> Option<S> {
    let st = storage(test).await?;
    let e = env_with(test, Some(st.clone())).await?;
    let tok = e.key.token("boss", "sb", &["maxime-admins"]);
    let d = e.register(&tok, "desk").await;
    let v = vault_id(&e, "knowledge").await;
    Some(S { e, st, tok, d, v })
}

impl S {
    async fn file(&self, entry: &str, path: &str, text: &str, base: Option<&str>) -> Value {
        let data = text.as_bytes();
        let c = ChunkHash::of(data).to_string();
        let req = Request::put(format!("/v1/vaults/{}/chunks/{c}", self.v))
            .header("authorization", format!("Bearer {}", self.tok))
            .header("x-kura-device", &self.d)
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
                &self.tok,
                Some(&self.d),
                Some(json!({ "size": size, "chunks": [c] })),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{body}");
        let mut op = json!({ "op": "put", "entry": entry, "path": path, "blob": b, "size": size });
        if let Some(base) = base {
            op["base"] = json!(base);
        }
        self.commit(op).await
    }

    async fn commit(&self, op: Value) -> Value {
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
        assert_eq!(st, StatusCode::OK, "{body}");
        body["results"][0]["head"].clone()
    }

    async fn search(&self, query: &str) -> (StatusCode, Value) {
        self.e
            .call(
                "POST",
                &format!("/v1/vaults/{}/search", self.v),
                &self.tok,
                Some(&self.d),
                Some(json!({ "query": query })),
            )
            .await
    }

    async fn paths(&self, query: &str) -> Vec<String> {
        let (st, b) = self.search(query).await;
        assert_eq!(st, StatusCode::OK, "{b}");
        b["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["path"].as_str().unwrap().to_owned())
            .collect()
    }

    /// The configuration with search on or off for tenant maxime, applied
    /// and loaded as the server would.
    async fn set_search(&self, on: bool) {
        let docs = if on {
            CONFIG.replace(
                "quota: { storage: 1Mi, maxFileSize: 2Mi }",
                "quota: { storage: 1Mi, maxFileSize: 2Mi }\n  search: { enabled: true }",
            )
        } else {
            CONFIG.to_owned()
        };
        let dir = std::env::temp_dir().join(format!("kura-search-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.yaml"), docs).unwrap();
        let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
        kura_server::reconcile::reconcile(&self.e.t.app, &files, true)
            .await
            .unwrap();
        let snap = kura_server::reconcile::snapshot::load_if_changed(&self.e.t.app, None)
            .await
            .unwrap()
            .unwrap();
        self.e.state.set_snapshot(snap);
    }

    async fn index(&self) -> kura_server::app::search::Round {
        round(&self.e.state).await.unwrap()
    }
}

#[tokio::test]
async fn search_is_opt_in_indexes_markdown_and_follows_edits() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    // Written before search is on: picked up by the backfill.
    let budget = Uuid::now_v7().to_string();
    let first = s
        .file(
            &budget,
            "finance/budget.md",
            "Le budget prévisionnel du troisième trimestre est approuvé.\n",
            None,
        )
        .await;
    s.file(
        &Uuid::now_v7().to_string(),
        "image.canvas",
        "{\"nodes\": [\"budget\"]}",
        None,
    )
    .await;
    let (st, b) = s.search("budget").await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::CONFLICT, "/v1/problems/search_disabled")
    );

    s.set_search(true).await;
    let r = s.index().await;
    assert_eq!((r.queued, r.indexed), (1, 1), "{r:?}: Markdown only");
    // Written after: queued by its own commit.
    s.file(
        &Uuid::now_v7().to_string(),
        "notes/running.md",
        "She keeps running every morning.\n",
        None,
    )
    .await;
    s.file(
        &Uuid::now_v7().to_string(),
        "rapports/rapport-annuel.md",
        "Sans le mot.\n",
        None,
    )
    .await;
    assert_eq!(s.index().await.indexed, 2);

    assert_eq!(
        s.paths("previsionnel").await,
        ["finance/budget.md"],
        "accents folded, French stems"
    );
    assert_eq!(s.paths("run").await, ["notes/running.md"], "English stems");
    assert_eq!(
        s.paths("rapport-annuel").await,
        ["rapports/rapport-annuel.md"],
        "file names by trigram"
    );
    let (_, b) = s.search("approuvé").await;
    assert!(
        b["results"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("approuvé"),
        "{b}"
    );
    let (st, b) = s.search("x").await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request")
    );

    // A scoped token searches under its prefix only (T16).
    let (st, t) =
        s.e.call(
            "POST",
            &format!("/v1/vaults/{}/tokens", s.v),
            &s.tok,
            Some(&s.d),
            Some(json!({ "prefix": "notes/", "expiresInDays": 1, "cidrs": ["10.0.0.0/8"] })),
        )
        .await;
    assert_eq!(st, StatusCode::CREATED, "{t}");
    let mut req = Request::post(format!("/v1/vaults/{}/search", s.v))
        .header(
            "authorization",
            format!("Bearer {}", t["token"].as_str().unwrap()),
        )
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "query": "budget or running" }).to_string(),
        ))
        .unwrap();
    req.extensions_mut()
        .insert(ConnectInfo("10.1.2.3:4".parse::<SocketAddr>().unwrap()));
    let r = app(s.e.state.clone()).oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body: Value =
        serde_json::from_slice(&axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap())
            .unwrap();
    let paths: Vec<&str> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["notes/running.md"]);

    // An edit replaces the document; a deletion drops it.
    let h = s
        .file(
            &budget,
            "finance/budget.md",
            "Le budget est rejeté.\n",
            first["revision"].as_str(),
        )
        .await;
    s.index().await;
    assert!(s.paths("previsionnel").await.is_empty());
    assert_eq!(s.paths("rejeté").await, ["finance/budget.md"]);
    // The serving role cannot write words for an entry no job claims (review 7 L4).
    sqlx::query("SELECT kura_index_set($1::uuid, $2::uuid, $3::uuid, 'injected words')")
        .bind(&s.v)
        .bind(&budget)
        .bind(h["revision"].as_str().unwrap())
        .execute(&s.e.t.app)
        .await
        .unwrap();
    assert!(s.paths("injected").await.is_empty());
    // Renamed away from Markdown: its words leave the index (review 7 M3).
    let h = s
        .file(
            &budget,
            "finance/budget.txt",
            "Le budget est rejeté.\n",
            h["revision"].as_str(),
        )
        .await;
    s.index().await;
    assert!(s.paths("rejeté").await.is_empty());
    s.commit(json!({ "op": "delete", "entry": budget, "base": h["revision"] }))
        .await;
    s.index().await;

    // Disabled: the index goes (decision 39).
    s.set_search(false).await;
    let r = s.index().await;
    assert_eq!(r.dropped, 2, "{r:?}");
    let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM search_doc")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(docs, 0);
    assert_eq!(s.search("running").await.0, StatusCode::CONFLICT);
}

/// §6.7: a job that cannot be indexed is retried, then kept as failed after
/// 5 attempts; the queue depth counts only pending ones.
#[tokio::test]
async fn a_failing_index_job_is_kept_after_five_attempts() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    s.set_search(true).await;
    s.index().await;
    s.file(
        &Uuid::now_v7().to_string(),
        "broken.md",
        "its object will be gone\n",
        None,
    )
    .await;
    // The chunk's object disappears: the note cannot be read.
    let key: String = sqlx::query_scalar("SELECT object_key FROM chunk LIMIT 1")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    s.st.objects()
        .delete(&object_store::path::Path::from(key.as_str()))
        .await
        .unwrap();
    for attempt in 1..=5 {
        let r = s.index().await;
        assert_eq!(r.failed, 1, "attempt {attempt}: {r:?}");
    }
    assert_eq!(
        s.index().await,
        kura_server::app::search::Round::default(),
        "kept, not retried"
    );
    let failed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM index_job WHERE failed_at IS NOT NULL")
            .fetch_one(&s.e.t.owner)
            .await
            .unwrap();
    assert_eq!(failed, 1);
    assert_eq!(s.e.state.metrics.index_queue_depth.get(), 0);
}
