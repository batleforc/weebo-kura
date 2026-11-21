//! Chunks and blobs end to end: PostgreSQL, Vault Transit and an S3 store
//! (RFC 0001 T6, T8, T9). Opt-in: KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR,
//! KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT (bucket `kura-test`, keys
//! `kura-dev` / `kura-dev-secret`, see `task dev:s3`).

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{CONFIG, Env, Stored, problem};
use futures_util::TryStreamExt as _;
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use kura_server::store::Storage;
use object_store::ObjectStoreExt as _;
use serde_json::json;
use tower::ServiceExt;

async fn setup(test: &str) -> Option<(Env, Arc<Storage>, String, String, String)> {
    let s = Stored::new(test, ("u1", "s1", &["maxime-members"]), "laptop").await?;
    Some((s.e, s.st, s.tok, s.d, s.v))
}

async fn put(
    e: &Env,
    tok: &str,
    d: &str,
    v: &str,
    hash: &str,
    body: Vec<u8>,
) -> (StatusCode, serde_json::Value) {
    let req = Request::put(format!("/v1/vaults/{v}/chunks/{hash}"))
        .header("authorization", format!("Bearer {tok}"))
        .header("x-kura-device", d)
        .body(Body::from(body))
        .unwrap();
    let r = app(e.state.clone()).oneshot(req).await.unwrap();
    let status = r.status();
    let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Commits a file with this manifest: content is served once an entry names
/// it (phase 8: never by address alone).
async fn commit_blob(e: &Env, tok: &str, d: &str, v: &str, blob: &str, size: u64) {
    let op = json!({ "op": "put", "entry": uuid::Uuid::now_v7(), "path": format!("{blob}.md"), "blob": blob, "size": size });
    let (st, b) = e
        .call(
            "POST",
            &format!("/v1/vaults/{v}/commits"),
            tok,
            Some(d),
            Some(json!({ "idempotencyKey": uuid::Uuid::new_v4().to_string(), "operations": [op] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
}

/// Raw objects stored under the vault's prefix.
async fn objects(s: &Storage, v: &str) -> Vec<Vec<u8>> {
    let prefix = object_store::path::Path::from(v);
    let metas: Vec<_> = s.objects().list(Some(&prefix)).try_collect().await.unwrap();
    let mut out = Vec::new();
    for m in metas {
        out.push(
            s.objects()
                .get(&m.location)
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .to_vec(),
        );
    }
    out
}

#[tokio::test]
async fn hash_mismatch_is_422() {
    let Some((e, s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    let wrong = ChunkHash::of(b"something else").to_string();
    let (status, b) = put(&e, &tok, &d, &v, &wrong, b"the real bytes".to_vec()).await;
    assert_eq!(
        (status, problem(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/hash_mismatch"
        )
    );
    assert!(objects(&s, &v).await.is_empty(), "no object written");
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn bucket_object_is_not_plaintext() {
    let Some((e, s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    let note = b"# Diary\nThe safe code is 4512, never write it down.\n".repeat(30);
    let hash = ChunkHash::of(&note).to_string();
    let (status, _) = put(&e, &tok, &d, &v, &hash, note.clone()).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let stored = objects(&s, &v).await;
    assert_eq!(stored.len(), 1);
    assert!(
        !stored[0].windows(14).any(|w| w == b"safe code is 4"),
        "plaintext at rest"
    );
    let key: String = sqlx::query_scalar("SELECT object_key FROM chunk")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert!(
        !key.contains(&hash),
        "the object key does not reveal the content hash"
    );
    // And it reads back decrypted, immutable, once a file names it.
    let size = note.len() as u64;
    let blob = blob_hash(size, &[ChunkHash::of(&note)]).to_string();
    let (st, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{blob}"),
            &tok,
            Some(&d),
            Some(json!({ "size": size, "chunks": [hash] })),
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    commit_blob(&e, &tok, &d, &v, &blob, size).await;
    let (status, b) = e
        .call(
            "GET",
            &format!("/v1/vaults/{v}/chunks/{hash}"),
            &tok,
            Some(&d),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    let req = Request::get(format!("/v1/vaults/{v}/chunks/{hash}"))
        .header("authorization", format!("Bearer {tok}"))
        .header("x-kura-device", &d)
        .body(Body::empty())
        .unwrap();
    let r = app(e.state.clone()).oneshot(req).await.unwrap();
    assert_eq!(
        r.headers()["cache-control"],
        "private, immutable, max-age=31536000"
    );
    let bytes = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    assert_eq!(bytes.as_ref(), note.as_slice());
}

#[tokio::test]
async fn oversized_chunk_is_413() {
    let Some((e, s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    let big = vec![7u8; (4 << 20) + 1];
    let hash = ChunkHash::of(&big).to_string();
    let (status, b) = put(&e, &tok, &d, &v, &hash, big).await;
    assert_eq!(
        (status, problem(&b)),
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "/v1/problems/payload_too_large"
        )
    );
    assert!(objects(&s, &v).await.is_empty());
}

#[tokio::test]
async fn upload_resumes_from_missing_chunks_and_manifests_check_out() {
    let Some((e, _s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    let (a, b) = (b"first chunk".to_vec(), b"second chunk".to_vec());
    let (ha, hb) = (ChunkHash::of(&a), ChunkHash::of(&b));
    let ask = json!({ "hashes": [ha.to_string(), hb.to_string()] });
    let (_, r) = e
        .call(
            "POST",
            &format!("/v1/vaults/{v}/chunks/missing"),
            &tok,
            Some(&d),
            Some(ask.clone()),
        )
        .await;
    assert_eq!(r["missing"].as_array().unwrap().len(), 2);
    put(&e, &tok, &d, &v, &ha.to_string(), a.clone()).await;
    let (_, r) = e
        .call(
            "POST",
            &format!("/v1/vaults/{v}/chunks/missing"),
            &tok,
            Some(&d),
            Some(ask),
        )
        .await;
    assert_eq!(
        r["missing"],
        json!([hb.to_string()]),
        "only what is missing is re-sent"
    );
    // A full batch of 1,000 hashes (about 67 KB) fits the route's body limit;
    // the 64 KiB default refused it with 413 (found by perf/attach.sh).
    let many: Vec<String> = (0..1000u32)
        .map(|i| ChunkHash::of(&i.to_le_bytes()).to_string())
        .collect();
    let (st, r) = e
        .call(
            "POST",
            &format!("/v1/vaults/{v}/chunks/missing"),
            &tok,
            Some(&d),
            Some(json!({ "hashes": many })),
        )
        .await;
    assert_eq!(st.as_u16(), 200, "{r}");
    assert_eq!(r["missing"].as_array().unwrap().len(), 1000);
    // A manifest naming a missing chunk is refused; complete, it is stored.
    let size = (a.len() + b.len()) as u64;
    let bh = blob_hash(size, &[ha, hb]).to_string();
    let manifest = json!({ "size": size, "chunks": [ha.to_string(), hb.to_string()] });
    let (s1, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{bh}"),
            &tok,
            Some(&d),
            Some(manifest.clone()),
        )
        .await;
    assert_eq!(s1, StatusCode::UNPROCESSABLE_ENTITY);
    put(&e, &tok, &d, &v, &hb.to_string(), b.clone()).await;
    let (s2, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{bh}"),
            &tok,
            Some(&d),
            Some(manifest.clone()),
        )
        .await;
    assert_eq!(s2, StatusCode::NO_CONTENT);
    commit_blob(&e, &tok, &d, &v, &bh, size).await;
    let (s3, got) = e
        .call(
            "GET",
            &format!("/v1/vaults/{v}/blobs/{bh}"),
            &tok,
            Some(&d),
            None,
        )
        .await;
    assert_eq!((s3, got), (StatusCode::OK, manifest.clone()));
    // A manifest under the wrong address, or with a wrong size.
    let lie = json!({ "size": size + 1, "chunks": [ha.to_string(), hb.to_string()] });
    let (s4, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{bh}"),
            &tok,
            Some(&d),
            Some(lie),
        )
        .await;
    assert_eq!(s4, StatusCode::UNPROCESSABLE_ENTITY);
    // A second upload of a present chunk writes nothing new.
    let (s5, _) = put(&e, &tok, &d, &v, &ha.to_string(), a).await;
    assert_eq!(s5, StatusCode::NO_CONTENT);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert_eq!(rows, 2);
}

#[tokio::test]
async fn quota_and_roles_bound_uploads() {
    let Some((e, _s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    // The tenant's quota is 1 MiB of stored chunks, files at most 2 MiB.
    let one = vec![1u8; 600 << 10];
    let two = vec![2u8; 600 << 10];
    let (s1, _) = put(&e, &tok, &d, &v, &ChunkHash::of(&one).to_string(), one).await;
    assert_eq!(s1, StatusCode::NO_CONTENT);
    let (s2, b) = put(&e, &tok, &d, &v, &ChunkHash::of(&two).to_string(), two).await;
    assert_eq!(
        (s2, problem(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/quota_exceeded"
        )
    );
    let huge = json!({ "size": 3u64 << 20, "chunks": [] });
    let h = blob_hash(3 << 20, &[]).to_string();
    let (s3, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{h}"),
            &tok,
            Some(&d),
            Some(huge),
        )
        .await;
    assert_eq!(s3, StatusCode::PAYLOAD_TOO_LARGE, "over maxFileSize");
    // A tenant admin without a binding is manager; a stranger sees 404.
    let other = e.key.token("o1", "so", &["other-members"]);
    let od = e.register(&other, "x").await;
    let (s4, _) = put(
        &e,
        &other,
        &od,
        &v,
        &ChunkHash::of(b"z").to_string(),
        b"z".to_vec(),
    )
    .await;
    assert_eq!(s4, StatusCode::NOT_FOUND);
}

/// Concurrent uploads cannot overshoot the quota together (review P3).
#[tokio::test]
async fn concurrent_uploads_respect_the_quota() {
    let Some((e, _s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    let calls = (0u8..6).map(|i| vec![i; 300 << 10]).map(|b| {
        let h = ChunkHash::of(&b).to_string();
        let (e, tok, d, v) = (&e, &tok, &d, &v);
        async move { put(e, tok, d, v, &h, b).await.0 }
    });
    let statuses = futures_util::future::join_all(calls).await;
    let stored: i64 = sqlx::query_scalar("SELECT coalesce(sum(size), 0)::bigint FROM chunk")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert!(
        stored <= 1 << 20,
        "{stored} bytes stored over a 1 MiB quota: {statuses:?}"
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::NO_CONTENT)
            .count(),
        3,
        "{statuses:?}"
    );
}

/// `POST` of a JSON body; the raw answer.
async fn post_raw(e: &Env, tok: &str, d: &str, uri: &str, body: Vec<u8>) -> (StatusCode, Vec<u8>) {
    let req = Request::post(uri)
        .header("authorization", format!("Bearer {tok}"))
        .header("x-kura-device", d)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let r = app(e.state.clone()).oneshot(req).await.unwrap();
    let status = r.status();
    let bytes = axum::body::to_bytes(r.into_body(), 8 << 20).await.unwrap();
    (status, bytes.to_vec())
}

/// `chunks/batch` records: (hash, Some(bytes) or None for missing).
fn records(mut b: &[u8]) -> Vec<(String, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let hash = ChunkHash::from_bytes(b[..32].try_into().unwrap()).to_string();
        let status = b[32];
        let len = u32::from_be_bytes(b[33..37].try_into().unwrap()) as usize;
        let data = b[37..37 + len].to_vec();
        out.push((hash, (status == 0).then_some(data)));
        b = &b[37 + len..];
    }
    out
}

/// Uploads `data` as one chunk and a one-chunk file, committed; its blob hash.
async fn stored_file(e: &Env, tok: &str, d: &str, v: &str, data: &[u8]) -> String {
    let h = ChunkHash::of(data);
    put(e, tok, d, v, &h.to_string(), data.to_vec()).await;
    let blob = blob_hash(data.len() as u64, &[h]).to_string();
    let (st, b) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{blob}"),
            tok,
            Some(d),
            Some(json!({ "size": data.len(), "chunks": [h.to_string()] })),
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT, "{b}");
    commit_blob(e, tok, d, v, &blob, data.len() as u64).await;
    blob
}

/// RFC 0002: order kept, duplicates dropped, the 4 MiB cut, and an unknown
/// chunk, an uncommitted one and a malformed request each answered as
/// single reads would (T3), with the limits of §7.5 (T4).
#[tokio::test]
async fn batches_answer_in_order_within_their_budget() {
    let Some((e, _s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    // Room for 5 MiB of chunks, to reach the 4 MiB cut.
    let docs = CONFIG.replace(
        "quota: { storage: 1Mi, maxFileSize: 2Mi }",
        "quota: { storage: 16Mi, maxFileSize: 4Mi }",
    );
    assert_ne!(docs, CONFIG);
    e.apply_config(&docs).await;
    let (a, b) = (b"alpha".to_vec(), b"bravo".to_vec());
    let (big, bigger) = (vec![1u8; 3 << 20], vec![2u8; 2 << 20]);
    let blob_a = stored_file(&e, &tok, &d, &v, &a).await;
    stored_file(&e, &tok, &d, &v, &b).await;
    stored_file(&e, &tok, &d, &v, &big).await;
    stored_file(&e, &tok, &d, &v, &bigger).await;
    // Uploaded but named by no entry: served by neither route.
    let orphan = b"never committed".to_vec();
    put(
        &e,
        &tok,
        &d,
        &v,
        &ChunkHash::of(&orphan).to_string(),
        orphan.clone(),
    )
    .await;
    let h = |x: &[u8]| ChunkHash::of(x).to_string();
    let unknown = h(b"nobody has this");
    let uri = format!("/v1/vaults/{v}/chunks/batch");
    let ask = |list: Vec<String>| serde_json::to_vec(&json!({ "chunks": list })).unwrap();

    let (st, body) = post_raw(
        &e,
        &tok,
        &d,
        &uri,
        ask(vec![h(&a), unknown.clone(), h(&a), h(&orphan), h(&b)]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let got = records(&body);
    assert_eq!(
        got,
        vec![
            (h(&a), Some(a.clone())),
            (unknown.clone(), None),
            (h(&orphan), None),
            (h(&b), Some(b.clone())),
        ]
    );
    // An unknown hash and an uncommitted one: the same bytes but the hash.
    // Records 2 and 3 start at 42 and 79 (after a 37-byte header and 5 bytes).
    assert_eq!(body[42 + 32..42 + 37], body[79 + 32..79 + 37]);
    assert_eq!(body[42 + 32..42 + 37], [1, 0, 0, 0, 0]);

    // 3 MiB then 2 MiB: the second would pass 4 MiB, so it is left out.
    let (_, body) = post_raw(&e, &tok, &d, &uri, ask(vec![h(&big), h(&bigger)])).await;
    assert_eq!(records(&body), vec![(h(&big), Some(big.clone()))]);
    let (_, body) = post_raw(&e, &tok, &d, &uri, ask(vec![h(&bigger)])).await;
    assert_eq!(records(&body), vec![(h(&bigger), Some(bigger.clone()))]);

    // Manifests: found ones in order, the rest missing.
    let (st, body) = post_raw(
        &e,
        &tok,
        &d,
        &format!("/v1/vaults/{v}/blobs/batch"),
        serde_json::to_vec(&json!({ "blobs": [blob_a, unknown] })).unwrap(),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let r: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(r["manifests"][0]["hash"], blob_a);
    assert_eq!(r["manifests"][0]["chunks"], json!([h(&a)]));
    assert_eq!(r["missing"], json!([unknown]));

    // Malformed: none, 1,001, a bad hash; a body past 128 KiB.
    for bad in [vec![], vec![h(&a); 1001], vec!["ABC".to_owned()]] {
        let (st, body) = post_raw(&e, &tok, &d, &uri, ask(bad)).await;
        assert_eq!(
            st,
            StatusCode::BAD_REQUEST,
            "{}",
            String::from_utf8_lossy(&body)
        );
    }
    let (st, _) = post_raw(&e, &tok, &d, &uri, vec![b' '; 129 << 10]).await;
    assert_eq!(st, StatusCode::PAYLOAD_TOO_LARGE);

    // Every batch slot of the replica taken: 429 before any work, never a
    // 503 that the dependency alert would count.
    let held = e.state.batches.try_acquire_many(8).unwrap();
    let unavailable = e.state.metrics.unavailable_total.get();
    let (st, _) = post_raw(&e, &tok, &d, &uri, ask(vec![h(&a)])).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(e.state.metrics.unavailable_total.get(), unavailable);
    drop(held);
    let found = e
        .state
        .metrics
        .batch_items_total
        .with_label_values(&["chunks", "found"])
        .get();
    assert_eq!(found, 4, "only what was sent was fetched");

    // 60 calls a minute per device.
    let mut last = StatusCode::OK;
    for _ in 0..60 {
        last = post_raw(&e, &tok, &d, &uri, ask(vec![unknown.clone()]))
            .await
            .0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

/// Review 2026-11-15 #3: when the commit itself fails, the row may or may
/// not have landed, so the object is left for the orphan sweep instead of
/// deleted (a deleted object under a landed row loses the version for good).
/// A refusal before the commit still deletes it.
#[tokio::test]
async fn a_failed_commit_leaves_the_object_for_the_sweep() {
    let Some((e, s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    // Fails at COMMIT, after the insert ran: the ambiguous case.
    for sql in [
        "CREATE FUNCTION fail_at_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'commit lost'; END $$",
        "CREATE CONSTRAINT TRIGGER fail_at_commit AFTER INSERT ON chunk DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_at_commit()",
    ] {
        sqlx::query(sql).execute(&e.t.owner).await.unwrap();
    }
    let data = b"a chunk whose commit fails".to_vec();
    let (status, _) = put(&e, &tok, &d, &v, &ChunkHash::of(&data).to_string(), data).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        objects(&s, &v).await.len(),
        1,
        "the object is kept for the sweep"
    );
    sqlx::query("DROP TRIGGER fail_at_commit ON chunk")
        .execute(&e.t.owner)
        .await
        .unwrap();
}

/// RFC 0002 phase 3, T7: `chunks/upload` checks every record against its
/// address before storing any; `blobs/upload` stores every manifest or none.
#[tokio::test]
async fn upload_batch_rejects_a_wrong_hash() {
    let Some((e, s, tok, d, v)) = setup(module_path!()).await else {
        return;
    };
    let uri = |route: &str| format!("/v1/vaults/{v}/{route}");
    let (a, b) = (b"first chunk".to_vec(), b"second chunk".to_vec());
    let (ha, hb) = (ChunkHash::of(&a), ChunkHash::of(&b));
    let body = |list: &[(ChunkHash, Option<&[u8]>)]| {
        let mut out = Vec::new();
        for (h, data) in list {
            kura_core::wire::write_record(&mut out, h, *data);
        }
        out
    };

    // One record under another's address: refused, nothing stored.
    let wrong = body(&[(ha, Some(&a)), (hb, Some(&a))]);
    let (st, err) = post_raw(&e, &tok, &d, &uri("chunks/upload"), wrong).await;
    let err: serde_json::Value = serde_json::from_slice(&err).unwrap();
    assert_eq!(
        (st, problem(&err)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/hash_mismatch"
        )
    );
    assert!(
        objects(&s, &v).await.is_empty(),
        "a refused batch stored nothing"
    );

    // Malformed: no record, a missing-status record, a record cut short.
    for bad in [
        Vec::new(),
        body(&[(ha, None)]),
        body(&[(ha, Some(&a))])[..40].to_vec(),
    ] {
        let (st, _) = post_raw(&e, &tok, &d, &uri("chunks/upload"), bad).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }

    // Both right, one twice: stored once each, and no longer missing.
    let good = body(&[(ha, Some(&a)), (hb, Some(&b)), (ha, Some(&a))]);
    let (st, _) = post_raw(&e, &tok, &d, &uri("chunks/upload"), good).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    assert_eq!(objects(&s, &v).await.len(), 2);
    let (_, m) = e
        .call(
            "POST",
            &uri("chunks/missing"),
            &tok,
            Some(&d),
            Some(json!({ "hashes": [ha, hb] })),
        )
        .await;
    assert_eq!(m["missing"], json!([]));

    manifests_are_stored_all_or_none(&e, &tok, &d, &v, &a, &b).await;

    // A stranger to the vault sees no upload route either.
    let other = e.key.token("o1", "so", &["other-members"]);
    let od = e.register(&other, "x").await;
    let (st, _) = post_raw(
        &e,
        &other,
        &od,
        &uri("chunks/upload"),
        body(&[(ha, Some(&a))]),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

/// manifests are stored all or none: one phase of the test below.
async fn manifests_are_stored_all_or_none(
    e: &Env,
    tok: &str,
    d: &str,
    v: &str,
    a: &[u8],
    b: &[u8],
) {
    let (ha, hb) = (ChunkHash::of(a), ChunkHash::of(b));
    let uri = |route: &str| format!("/v1/vaults/{v}/{route}");
    // Manifests: one names a chunk never stored, so neither is kept.
    let size = |d: &[u8]| d.len() as u64;
    let ghost = ChunkHash::of(b"never uploaded");
    let fine = json!({ "hash": blob_hash(size(a), &[ha]), "size": size(a), "chunks": [ha] });
    let broken = json!({ "hash": blob_hash(14, &[ghost]), "size": 14, "chunks": [ghost] });
    let blobs = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM blob WHERE vault_id = $1::uuid")
            .bind(v)
            .fetch_one(&e.t.owner)
            .await
            .unwrap()
    };
    let (st, _) = e
        .call(
            "POST",
            &uri("blobs/upload"),
            tok,
            Some(d),
            Some(json!({ "manifests": [fine, broken] })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(blobs().await, 0, "all or nothing");
    let lying = json!({ "hash": blob_hash(99, &[ha]), "size": 1, "chunks": [ha] });
    let (st, _) = e
        .call(
            "POST",
            &uri("blobs/upload"),
            tok,
            Some(d),
            Some(json!({ "manifests": [lying] })),
        )
        .await;
    assert_eq!(
        st,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a manifest not at its address"
    );
    let both = json!({ "hash": blob_hash(size(b), &[hb]), "size": size(b), "chunks": [hb] });
    let (st, _) = e
        .call(
            "POST",
            &uri("blobs/upload"),
            tok,
            Some(d),
            Some(json!({ "manifests": [fine, both] })),
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    assert_eq!(blobs().await, 2);
}
