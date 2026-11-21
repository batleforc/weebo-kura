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

use axum::http::StatusCode;
use common::app::{CONFIG, Stored as S, problem};
use kura_server::app::search::round;
use object_store::ObjectStoreExt as _;
use serde_json::{Value, json};
use uuid::Uuid;

async fn setup(test: &str) -> Option<S> {
    S::new(test, ("boss", "sb", &["maxime-admins"]), "desk").await
}

impl S {
    async fn file(&self, entry: &str, path: &str, text: &str, base: Option<&str>) -> Value {
        let data = text.as_bytes();
        let (b, size) = self.e.upload(&self.tok, &self.d, &self.v, data).await;
        let mut op = json!({ "op": "put", "entry": entry, "path": path, "blob": b, "size": size });
        if let Some(base) = base {
            op["base"] = json!(base);
        }
        self.commit(op).await
    }

    async fn commit(&self, op: Value) -> Value {
        self.e.commit_head(&self.tok, &self.d, &self.v, op).await
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
        self.e.apply_config(&docs).await;
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

    the_queries_fold_accents_and_stems(&s).await;

    a_scoped_token_searches_under_its_prefix(&s).await;

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

/// the queries fold accents and stems: one phase of the test below.
async fn the_queries_fold_accents_and_stems(s: &S) {
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
}

/// a scoped token searches under its prefix: one phase of the test below.
async fn a_scoped_token_searches_under_its_prefix(s: &S) {
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
    let (st, body) =
        s.e.call_peer(
            "10.1.2.3:4",
            "POST",
            &format!("/v1/vaults/{}/search", s.v),
            t["token"].as_str().unwrap(),
            Some(json!({ "query": "budget or running" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let paths: Vec<&str> = body["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["notes/running.md"]);
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
        // Not again at once: the job waits the 5-minute reclaim (review
        // 2026-11-15 #9), aged here.
        assert_eq!(
            s.index().await,
            kura_server::app::search::Round::default(),
            "attempt {attempt} waits"
        );
        sqlx::query("UPDATE index_job SET claimed_at = now() - interval '6 minutes' WHERE claimed_at IS NOT NULL")
            .execute(&s.e.t.owner)
            .await
            .unwrap();
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
    assert_eq!(
        s.e.state.metrics.index_failed.get(),
        1,
        "given up, and visible"
    );
}
