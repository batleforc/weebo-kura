//! Garbage collection and the `Delete` purge with real storage (RFC 0001
//! §6.9, decisions 24 and 33, T18). Opt-in: KURA_TEST_DATABASE_URL,
//! KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{CONFIG, Stored as S, problem};
use futures_util::TryStreamExt as _;
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use kura_server::maintenance::{BACKUP_LOCK, Report, Settings, pass};
use object_store::ObjectStoreExt as _;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup(test: &str) -> Option<S> {
    S::new(test, ("u1", "s1", &["maxime-members"]), "desk").await
}

impl S {
    async fn put_chunk(&self, data: &[u8]) -> String {
        self.e.put_chunk(&self.tok, &self.d, &self.v, data).await
    }

    /// One chunk's file: chunk, manifest; returns (blob, size).
    async fn put_file(&self, data: &[u8]) -> (String, u64) {
        self.e.upload(&self.tok, &self.d, &self.v, data).await
    }

    async fn put_blob(&self, b: &str, size: u64, chunk: &str) -> (StatusCode, Value) {
        self.e
            .put_blob((&self.tok, &self.d, &self.v), b, size, chunk)
            .await
    }

    async fn commit(&self, op: Value) -> Value {
        self.e.commit_head(&self.tok, &self.d, &self.v, op).await
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

async fn reconcile(s: &S, docs: &str) {
    let dir = std::env::temp_dir().join(format!("kura-t18-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("config.yaml"), docs).unwrap();
    let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
    kura_server::reconcile::reconcile(&s.e.t.app, &files, true)
        .await
        .unwrap();
}

async fn epoch(s: &S) -> Uuid {
    sqlx::query_scalar::<_, Uuid>("SELECT epoch FROM vault WHERE id = $1::uuid")
        .bind(&s.v)
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap()
}

async fn make_due(s: &S) {
    s.sql("UPDATE vault SET delete_after = now() - interval '1 second' WHERE delete_after IS NOT NULL")
        .await;
}

async fn purge_vault(s: &S) -> i64 {
    sqlx::query_scalar("SELECT kura_purge_vault($1::uuid, 1)")
        .bind(&s.v)
        .fetch_one(&s.e.t.app)
        .await
        .unwrap()
}

/// Removed and due again, then back after a first batch: every device must
/// resync without deleting (a new epoch), and no entry is left without its
/// head (review 5 H1). Returns the epoch once restored.
async fn restored_after_first_batch(s: &S, with: &str, without: &str) -> Uuid {
    let before = epoch(s).await;
    reconcile(s, without).await;
    make_due(s).await;
    for _ in 0..2 {
        assert_eq!(purge_vault(s).await, 1);
    }
    assert_eq!(s.count("entry").await, 0, "the entry left with its history");
    reconcile(s, with).await;
    assert_ne!(epoch(s).await, before, "devices get history_rewound");
    let headless: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM entry e WHERE NOT EXISTS (SELECT 1 FROM revision r WHERE r.id = e.head_revision)",
    )
    .fetch_one(&s.e.t.owner)
    .await
    .unwrap();
    assert_eq!(headless, 0);
    assert_eq!(purge_vault(s).await, -1, "no longer due");
    epoch(s).await
}

/// Removed again and due: purged, rows then objects, the vault row last;
/// this second cycle changes the epoch again from its first batch.
async fn purged_when_due(s: &S, without: &str, restored: Uuid) {
    reconcile(s, without).await;
    make_due(s).await;
    assert!(purge_vault(s).await >= 0);
    assert_ne!(
        epoch(s).await,
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
    reconcile(&s, &with).await;
    reconcile(&s, &without).await;
    let due: Option<time::OffsetDateTime> =
        sqlx::query_scalar("SELECT delete_after FROM vault WHERE id = $1::uuid")
            .bind(&s.v)
            .fetch_one(&s.e.t.owner)
            .await
            .unwrap();
    let days = (due.unwrap() - time::OffsetDateTime::now_utc()).whole_days();
    assert_eq!(days, 29, "30 days of grace");
    // The grace period passes, then the document comes back.
    make_due(&s).await;
    reconcile(&s, &with).await;
    let r = s.pass().await;
    assert_eq!(r.purged_vaults, 0, "{r:?}");
    assert_eq!((s.count("entry").await, s.objects().await), (1, 1));
    let restored = restored_after_first_batch(&s, &with, &without).await;
    purged_when_due(&s, &without, restored).await;
}

/// §10.1: the phase 6 metrics move with real traffic (PR 6.1).
#[tokio::test]
async fn storage_commit_and_resync_metrics_move() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (b, size) = s.put_file(&[7u8; 1000]).await;
    for path in ["a.md", "copy of a.md"] {
        s.commit(
            json!({ "op": "put", "entry": Uuid::now_v7(), "path": path, "blob": b, "size": size }),
        )
        .await;
    }
    let m = &s.e.state.metrics;
    assert_eq!(m.commit_duration_seconds.get_sample_count(), 2);
    kura_server::metrics::storage_once(&s.e.t.app, m)
        .await
        .unwrap();
    let stored = m.chunk_bytes_stored.with_label_values(&["maxime"]).get();
    assert!(stored >= 1000.0, "{stored}");
    let dedup = m.dedup_ratio.with_label_values(&["maxime"]).get();
    assert!(dedup > 1.5, "two files, one stored copy: {dedup}");
    assert_eq!(m.deletions_scheduled.get(), 0);
    let quota = m.quota_used_ratio.with_label_values(&["maxime"]).get();
    assert!(quota > 0.0 && quota < 0.01, "{quota}");
    let (st, _) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/changes?since=999", s.v),
            &s.tok,
            Some(&s.d),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::GONE);
    assert_eq!(
        m.resync_total.with_label_values(&["history_rewound"]).get(),
        1
    );
}

/// §10.3, PR 6.4: the bucket copy runs under the backup lock, copies only
/// what the second bucket lacks and names the point in time to recover
/// PostgreSQL to; the drill's check decrypts chunks and catches a bad one.
#[tokio::test]
async fn backup_copies_new_objects_and_the_drill_check_catches_corruption() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (b, size) = s.put_file(b"backed up").await;
    s.commit(
        json!({ "op": "put", "entry": Uuid::now_v7(), "path": "a.md", "blob": b, "size": size }),
    )
    .await;
    s.put_file(b"another file").await;
    // The second location: the same test bucket under a prefix of its own.
    let target = object_store::prefix::PrefixStore::new(
        s.st.objects(),
        format!("backup-{}", Uuid::new_v4()),
    );
    let first = kura_server::backup::copy(&s.e.state, &target)
        .await
        .unwrap();
    assert_eq!((first.objects, first.copied), (2, 2), "{first:?}");
    let manifest = object_store::path::Path::from(format!("backups/{}.json", first.recover_to));
    assert!(target.head(&manifest).await.is_ok());
    let keys = target
        .get(&object_store::path::Path::from(format!(
            "backups/{}.keys",
            first.recover_to
        )))
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&keys).lines().count(),
        2,
        "the key list of T"
    );
    let again = kura_server::backup::copy(&s.e.state, &target)
        .await
        .unwrap();
    assert_eq!((again.objects, again.copied), (2, 0), "keys are immutable");
    // Each completed copy is recorded, for the backup alert.
    kura_server::metrics::storage_once(&s.e.t.app, &s.e.state.metrics)
        .await
        .unwrap();
    assert!(
        s.e.state
            .metrics
            .backup_last_success_timestamp_seconds
            .get()
            > 0.0
    );
    // The serving role cannot sample other tenants' chunks.
    assert!(
        sqlx::query("SELECT kura_sample_chunks(1)")
            .execute(&s.e.t.app)
            .await
            .is_err()
    );
    // A maintenance pass holding the lock: the copy waits, then gives up.
    let mut held = s.e.t.owner.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock_shared($1)")
        .bind(BACKUP_LOCK)
        .execute(&mut *held)
        .await
        .unwrap();
    let busy =
        kura_server::backup::copy_waiting(&s.e.state, &target, std::time::Duration::ZERO).await;
    assert!(
        matches!(busy, Err(kura_server::backup::BackupError::Busy(_))),
        "{busy:?}"
    );
    sqlx::query("SELECT pg_advisory_unlock_all()")
        .execute(&mut *held)
        .await
        .unwrap();
    // The drill's check: every sampled chunk opens, until one is damaged.
    assert_eq!(
        kura_server::backup::check_chunks(&s.e.state, &s.e.t.owner, 10)
            .await
            .unwrap(),
        (2, 0)
    );
    let key: String = sqlx::query_scalar("SELECT object_key FROM chunk LIMIT 1")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    s.st.objects()
        .put(
            &object_store::path::Path::from(key.as_str()),
            b"\x01\x00garbage".to_vec().into(),
        )
        .await
        .unwrap();
    assert_eq!(
        kura_server::backup::check_chunks(&s.e.state, &s.e.t.owner, 10)
            .await
            .unwrap(),
        (2, 1)
    );
}

/// A connection holding the shared backup lock, and its backend pid.
async fn hold_backup_lock(db: &sqlx::PgPool) -> (sqlx::pool::PoolConnection<sqlx::Postgres>, i32) {
    let mut lock = db.acquire().await.unwrap();
    let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock_shared($1)")
        .bind(BACKUP_LOCK)
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    assert!(held);
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    (lock, pid)
}

/// Review 2026-11-15 #8 and #15: a vault restored while its objects are
/// purged. An object a device uploads again (written after the cutoff, or
/// named by a chunk row) is kept, and the purge does not finish. And once
/// the session holding the shared backup lock lost it, nothing is deleted.
#[tokio::test]
async fn purge_keeps_objects_uploaded_after_its_rows_went() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    s.put_file(b"uploaded again").await;
    let stray = object_store::path::Path::from(format!("{}/stray", s.v));
    s.st.objects()
        .put(&stray, b"sealed bytes".to_vec().into())
        .await
        .unwrap();
    let (mut lock, pid) = hold_backup_lock(&s.e.state.db).await;
    let v: Uuid = s.v.parse().unwrap();
    let objects = s.st.objects();
    let now = time::OffsetDateTime::now_utc();
    let before = now - time::Duration::hours(1);
    let after = now + time::Duration::hours(1);
    let purge =
        |cutoff| kura_server::maintenance::purge_objects(&s.e.state.db, pid, &*objects, v, cutoff);
    assert!(
        !purge(before).await.unwrap(),
        "both written after the cutoff"
    );
    assert_eq!(s.objects().await, 2);
    assert!(!purge(after).await.unwrap(), "the row's object is kept");
    assert_eq!(s.objects().await, 1, "only the stray object went");
    s.sql("DELETE FROM blob_chunk").await;
    s.sql("DELETE FROM chunk").await;
    // The lock is lost (a failover, an idle cutoff): nothing is deleted.
    sqlx::query("SELECT pg_advisory_unlock_all()")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert!(matches!(
        purge(after).await,
        Err(kura_server::maintenance::MaintenanceError::LockLost)
    ));
    assert_eq!(s.objects().await, 1);
    let _again: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock_shared($1)")
        .bind(BACKUP_LOCK)
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    assert!(purge(after).await.unwrap(), "nothing left");
    assert_eq!(s.objects().await, 0);
}

/// The purge lists the prefix as a stream, a page of BATCH keys at a time:
/// more keys than one page are all deleted, and the sibling prefix stays.
#[tokio::test]
async fn purge_walks_a_listing_longer_than_one_page() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let mem = object_store::memory::InMemory::new();
    let v: Uuid = s.v.parse().unwrap();
    let total = kura_server::maintenance::BATCH as usize * 2 + 37;
    for i in 0..total {
        let key = object_store::path::Path::from(format!("{v}/k{i}"));
        mem.put(&key, vec![1u8].into()).await.unwrap();
    }
    let other = object_store::path::Path::from(format!("{}/k", Uuid::new_v4()));
    mem.put(&other, vec![1u8].into()).await.unwrap();
    let (_lock, pid) = hold_backup_lock(&s.e.state.db).await;
    let cutoff = time::OffsetDateTime::now_utc() + time::Duration::hours(1);
    assert!(
        kura_server::maintenance::purge_objects(&s.e.state.db, pid, &mem, v, cutoff)
            .await
            .unwrap()
    );
    let left: Vec<_> = object_store::ObjectStore::list(&mem, None)
        .try_collect()
        .await
        .unwrap();
    assert_eq!(left.len(), 1, "only the other prefix remains");
}
