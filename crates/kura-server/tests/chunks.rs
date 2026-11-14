//! Chunks and blobs end to end: PostgreSQL, Vault Transit and an S3 store
//! (RFC 0001 T6, T8, T9). Opt-in: KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR,
//! KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT (bucket `kura-test`, keys
//! `kura-dev` / `kura-dev-secret`, see `task dev:s3`).

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{Env, env_with, problem, storage, vault_id};
use futures_util::TryStreamExt as _;
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use kura_server::store::Storage;
use object_store::ObjectStoreExt as _;
use serde_json::json;
use tower::ServiceExt;

async fn setup(test: &str) -> Option<(Env, Arc<Storage>, String, String, String)> {
    let s = storage(test).await?;
    let e = env_with(test, Some(s.clone())).await?;
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&tok, "laptop").await;
    let v = vault_id(&e, "knowledge").await;
    Some((e, s, tok, d, v))
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
    // And it reads back decrypted, immutable.
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
    let bodies: Vec<Vec<u8>> = (0u8..6).map(|i| vec![i; 300 << 10]).collect();
    let calls = bodies.into_iter().map(|b| {
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
