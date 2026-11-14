//! Garbage collection and the `Delete` purge with real storage (RFC 0001
//! §6.9, decisions 24 and 33, T18). Opt-in: KURA_TEST_DATABASE_URL,
//! KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{CONFIG, Env, env_with, problem, storage, vault_id};
use futures_util::TryStreamExt as _;
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use kura_server::maintenance::{BACKUP_LOCK, Report, Settings, pass};
use kura_server::store::Storage;
use object_store::ObjectStoreExt as _;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
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
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&tok, "desk").await;
    let v = vault_id(&e, "knowledge").await;
    Some(S { e, st, tok, d, v })
}

impl S {
    async fn put_chunk(&self, data: &[u8]) -> String {
        let h = ChunkHash::of(data).to_string();
        let req = Request::put(format!("/v1/vaults/{}/chunks/{h}", self.v))
            .header("authorization", format!("Bearer {}", self.tok))
            .header("x-kura-device", &self.d)
            .body(Body::from(data.to_vec()))
            .unwrap();
        let r = app(self.e.state.clone()).oneshot(req).await.unwrap();
        assert!(r.status().is_success(), "{}", r.status());
        h
    }

    /// One chunk's file: chunk, manifest; returns (blob, size).
    async fn put_file(&self, data: &[u8]) -> (String, u64) {
        let c = self.put_chunk(data).await;
        let size = data.len() as u64;
        let b = blob_hash(size, &[ChunkHash::of(data)]).to_string();
        let (st, body) = self.put_blob(&b, size, &c).await;
        assert_eq!(st, StatusCode::NO_CONTENT, "{body}");
        (b, size)
    }

    async fn put_blob(&self, b: &str, size: u64, chunk: &str) -> (StatusCode, Value) {
        self.e
            .call(
                "PUT",
                &format!("/v1/vaults/{}/blobs/{b}", self.v),
                &self.tok,
                Some(&self.d),
                Some(json!({ "size": size, "chunks": [chunk] })),
            )
            .await
    }

    async fn commit(&self, op: Value) -> Value {
        let (st, b) = self
            .e
            .call(
                "POST",
                &format!("/v1/vaults/{}/commits", self.v),
                &self.tok,
                Some(&self.d),
                Some(json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op] })),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{b}");
        b["results"][0]["head"].clone()
    }

    async fn objects(&self) -> usize {
        let prefix = object_store::path::Path::from(self.v.as_str());
        let all: Vec<_> = self
            .st
            .objects()
            .list(Some(&prefix))
            .try_collect()
            .await
            .unwrap();
        all.len()
    }

    async fn count(&self, table: &'static str) -> i64 {
        // The table names are literals of these tests.
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM {table} WHERE vault_id = $1::uuid"
        )))
        .bind(&self.v)
        .fetch_one(&self.e.t.owner)
        .await
        .unwrap()
    }

    async fn sql(&self, q: &'static str) {
        sqlx::query(q).execute(&self.e.t.owner).await.unwrap();
    }

    /// Rows older than the 24 hours garbage collection waits.
    async fn age(&self) {
        self.sql("UPDATE chunk SET last_touched = now() - interval '2 days'")
            .await;
        self.sql("UPDATE blob SET last_touched = now() - interval '2 days'")
            .await;
    }

    async fn pass(&self) -> Report {
        let fast = Settings {
            pause: std::time::Duration::ZERO,
            orphan_grace: time::Duration::ZERO,
        };
        pass(&self.e.state, fast, &CancellationToken::new())
            .await
            .unwrap()
            .expect("no other pass runs")
    }
}

/// §6.9: an unreferenced chunk goes, row then object; once its entry's last
/// revision with content is pruned, the blob and its chunk follow.
#[tokio::test]
async fn collection_removes_only_what_nothing_references() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (b, size) = s.put_file(b"referenced content").await;
    let entry = Uuid::now_v7();
    let head = s
        .commit(json!({ "op": "put", "entry": entry, "path": "a.md", "blob": b, "size": size }))
        .await;
    s.put_chunk(b"uploaded, never committed").await;
    assert_eq!(s.objects().await, 2);
    // Younger than 24 hours: kept.
    assert_eq!(s.pass().await.chunks, 0);
    s.age().await;
    let r = s.pass().await;
    assert_eq!((r.chunks, r.objects, r.blobs), (1, 1, 0), "{r:?}");
    assert_eq!(s.objects().await, 1);
    // Deleted, its content revision pruned by age: blob, chunk and object go.
    s.commit(json!({ "op": "delete", "entry": entry, "base": head["revision"] }))
        .await;
    s.sql(
        r#"UPDATE tenant SET retention_policy = '{"maxRevisionAge": "1d"}' WHERE slug = 'maxime'"#,
    )
    .await;
    s.sql("UPDATE revision SET created_at = now() - interval '2 days'")
        .await;
    s.age().await;
    let r = s.pass().await;
    assert_eq!(
        (r.revisions, r.blobs, r.chunks, r.objects),
        (1, 1, 1, 1),
        "{r:?}"
    );
    assert_eq!((s.objects().await, s.count("chunk").await), (0, 0));
}

/// Decision 24, both interleavings: a manifest being stored holds its
/// chunks FOR KEY SHARE, so collection skips them; a collection that locked
/// a chunk first makes the manifest wait, then ask for a re-upload.
#[tokio::test]
async fn a_blob_put_and_collection_never_interleave() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let data = b"raced chunk";
    let c = s.put_chunk(data).await;
    let raw = ChunkHash::of(data).as_bytes().to_vec();
    s.age().await;
    // A blob PUT in flight: collection skips the chunk.
    let mut held = s.e.t.owner.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM chunk WHERE hash = $1 FOR KEY SHARE")
        .bind(&raw)
        .execute(&mut *held)
        .await
        .unwrap();
    let keys: Vec<String> = sqlx::query_scalar("SELECT kura_gc_chunks($1::uuid, 1000)")
        .bind(&s.v)
        .fetch_all(&s.e.t.app)
        .await
        .unwrap();
    assert!(keys.is_empty(), "a chunk held by a blob PUT is skipped");
    held.commit().await.unwrap();
    // Collection first: the blob PUT waits on the row, then finds it gone.
    let mut gc = s.e.t.owner.begin().await.unwrap();
    sqlx::query("SELECT 1 FROM chunk WHERE hash = $1 FOR UPDATE")
        .bind(&raw)
        .execute(&mut *gc)
        .await
        .unwrap();
    let size = data.len() as u64;
    let b = blob_hash(size, &[ChunkHash::of(data)]).to_string();
    let put = {
        let (e, tok, d, v, b, c) = (
            s.e.state.clone(),
            s.tok.clone(),
            s.d.clone(),
            s.v.clone(),
            b.clone(),
            c.clone(),
        );
        tokio::spawn(async move {
            let req = Request::put(format!("/v1/vaults/{v}/blobs/{b}"))
                .header("authorization", format!("Bearer {tok}"))
                .header("x-kura-device", d)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "size": size, "chunks": [c] }).to_string(),
                ))
                .unwrap();
            let r = app(e).oneshot(req).await.unwrap();
            let st = r.status();
            let body = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
            (
                st,
                serde_json::from_slice::<Value>(&body).unwrap_or(Value::Null),
            )
        })
    };
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(!put.is_finished(), "the blob PUT waits for the collection");
    sqlx::query("DELETE FROM chunk WHERE hash = $1")
        .bind(&raw)
        .execute(&mut *gc)
        .await
        .unwrap();
    gc.commit().await.unwrap();
    let (st, body) = put.await.unwrap();
    assert_eq!(
        (st, problem(&body)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/hash_mismatch"
        ),
        "{body}"
    );
    // The client uploads the chunk again, then the manifest is stored.
    s.put_chunk(data).await;
    let (st, body) = s.put_blob(&b, size, &c).await;
    assert_eq!(st, StatusCode::NO_CONTENT, "{body}");
}

/// §6.9: an object with no row is swept once old enough; nothing is deleted
/// while the backup job holds its lock.
#[tokio::test]
async fn orphans_are_swept_and_the_backup_lock_pauses_deletion() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (b, size) = s.put_file(b"kept content").await;
    s.commit(
        json!({ "op": "put", "entry": Uuid::now_v7(), "path": "a.md", "blob": b, "size": size }),
    )
    .await;
    s.put_chunk(b"unreferenced").await;
    let orphan = object_store::path::Path::from(format!("{}/left-by-a-crash", s.v));
    s.st.objects()
        .put(&orphan, b"sealed bytes".to_vec().into())
        .await
        .unwrap();
    s.age().await;
    assert_eq!(s.objects().await, 3);
    let mut backup = s.e.t.owner.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(BACKUP_LOCK)
        .execute(&mut *backup)
        .await
        .unwrap();
    let r = s.pass().await;
    assert!(r.gc_skipped, "{r:?}");
    assert_eq!((s.objects().await, s.count("chunk").await), (3, 2));
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(BACKUP_LOCK)
        .execute(&mut *backup)
        .await
        .unwrap();
    let r = s.pass().await;
    assert_eq!((r.chunks, r.orphans), (1, 1), "{r:?}");
    assert_eq!(
        s.objects().await,
        1,
        "only the referenced chunk's object is left"
    );
}

/// T18 and decision 33: a vault removed with `Delete` keeps everything for
/// its grace period; its document returning cancels the deletion; once due,
/// its rows and objects are purged, the vault row last.
#[tokio::test]
async fn restored_document_cancels_deletion() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (b, size) = s.put_file(b"precious").await;
    s.commit(
        json!({ "op": "put", "entry": Uuid::now_v7(), "path": "a.md", "blob": b, "size": size }),
    )
    .await;
    let with = CONFIG.replace(
        "  tenantRef: { name: maxime }\n",
        "  tenantRef: { name: maxime }\n  deletionPolicy: Delete\n",
    );
    let without: String = with
        .split("\n---\n")
        .filter(|d| !d.contains("name: knowledge"))
        .collect::<Vec<_>>()
        .join("\n---\n");
    let reconcile = |docs: String| {
        let pool = s.e.t.app.clone();
        async move {
            let dir = std::env::temp_dir().join(format!("kura-t18-{}", Uuid::new_v4()));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("config.yaml"), docs).unwrap();
            let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
            kura_server::reconcile::reconcile(&pool, &files, true)
                .await
                .unwrap()
        }
    };
    reconcile(with.clone()).await;
    reconcile(without.clone()).await;
    let due: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT delete_after FROM vault WHERE id = $1::uuid")
            .bind(&s.v)
            .fetch_one(&s.e.t.owner)
            .await
            .unwrap();
    let days = (due.unwrap() - time::OffsetDateTime::now_utc()).whole_days();
    assert_eq!(days, 29, "30 days of grace");
    // The grace period passes, then the document comes back.
    s.sql("UPDATE vault SET delete_after = now() - interval '1 second' WHERE delete_after IS NOT NULL")
        .await;
    reconcile(with.clone()).await;
    let r = s.pass().await;
    assert_eq!(r.purged_vaults, 0, "{r:?}");
    assert_eq!((s.count("entry").await, s.objects().await), (1, 1));
    // Removed and due again, then back after a first batch: every device
    // must resync without deleting (a new epoch), and no entry is left
    // without its head (review 5 H1).
    let epoch = |s: &S| {
        let (pool, v) = (s.e.t.owner.clone(), s.v.clone());
        async move {
            sqlx::query_scalar::<_, Uuid>("SELECT epoch FROM vault WHERE id = $1::uuid")
                .bind(v)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let before = epoch(&s).await;
    reconcile(without.clone()).await;
    s.sql("UPDATE vault SET delete_after = now() - interval '1 second' WHERE delete_after IS NOT NULL")
        .await;
    for _ in 0..2 {
        let n: i64 = sqlx::query_scalar("SELECT kura_purge_vault($1::uuid, 1)")
            .bind(&s.v)
            .fetch_one(&s.e.t.app)
            .await
            .unwrap();
        assert_eq!(n, 1);
    }
    assert_eq!(s.count("entry").await, 0, "the entry left with its history");
    reconcile(with.clone()).await;
    assert_ne!(epoch(&s).await, before, "devices get history_rewound");
    let headless: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM entry e WHERE NOT EXISTS (SELECT 1 FROM revision r WHERE r.id = e.head_revision)",
    )
    .fetch_one(&s.e.t.owner)
    .await
    .unwrap();
    assert_eq!(headless, 0);
    let n: i64 = sqlx::query_scalar("SELECT kura_purge_vault($1::uuid, 1)")
        .bind(&s.v)
        .fetch_one(&s.e.t.app)
        .await
        .unwrap();
    assert_eq!(n, -1, "no longer due");
    // Removed again and due: purged, rows then objects, the vault row last;
    // this second cycle changes the epoch again from its first batch.
    let restored = epoch(&s).await;
    reconcile(without).await;
    s.sql("UPDATE vault SET delete_after = now() - interval '1 second' WHERE delete_after IS NOT NULL")
        .await;
    let n: i64 = sqlx::query_scalar("SELECT kura_purge_vault($1::uuid, 1)")
        .bind(&s.v)
        .fetch_one(&s.e.t.app)
        .await
        .unwrap();
    assert!(n >= 0);
    assert_ne!(
        epoch(&s).await,
        restored,
        "a second cycle changes the epoch too"
    );
    let r = s.pass().await;
    assert_eq!(r.purged_vaults, 1, "{r:?}");
    for t in ["entry", "revision", "blob", "chunk", "commit_record"] {
        assert_eq!(s.count(t).await, 0, "{t}");
    }
    assert_eq!(s.objects().await, 0);
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM vault WHERE id = $1::uuid")
        .bind(&s.v)
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
