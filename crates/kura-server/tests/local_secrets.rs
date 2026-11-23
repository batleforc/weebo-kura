//! A server without Vault (`localSecrets`): the bucket and master key read
//! from files, a chunk sealed with a locally wrapped data key and read back.
//! Opt-in: KURA_TEST_DATABASE_URL and KURA_TEST_S3_ENDPOINT.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{CONFIG, env_with, vault_id};
use kura_server::config::LocalSecrets;
use tower::ServiceExt;

#[tokio::test]
async fn a_server_without_vault_stores_and_reads_back() {
    let Ok(endpoint) = std::env::var("KURA_TEST_S3_ENDPOINT") else {
        eprintln!("SKIPPED: {} — KURA_TEST_S3_ENDPOINT unset", module_path!());
        return;
    };
    let dir = std::env::temp_dir().join(format!("kura-local-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (key, bucket) = (dir.join("master.key"), dir.join("bucket.json"));
    std::fs::write(&key, "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=\n").unwrap();
    std::fs::write(
        &bucket,
        format!(r#"{{"endpoint":"{endpoint}","bucket":"kura-test","accessKeyId":"kura-dev","secretAccessKey":"kura-dev-secret"}}"#),
    )
    .unwrap();
    let files = LocalSecrets {
        master_key_file: key.clone(),
        bucket_file: bucket,
    };
    let storage = Arc::new(kura_server::store::local_storage(&files).unwrap());
    let Some(e) = env_with(module_path!(), Some(storage)).await else {
        return;
    };
    e.apply_config(CONFIG).await;
    let v = vault_id(&e, "knowledge").await;
    let tok = e.key.token("u1", "s-local", &["maxime-members"]);
    let d = e.register(&tok, "desk").await;
    let data = b"stored without Vault";
    let (blob, size) = e.upload(&tok, &d, &v, data).await;
    let op = serde_json::json!({ "op": "put", "entry": uuid::Uuid::now_v7().to_string(), "path": "local.md", "blob": blob, "size": size });
    let body = serde_json::json!({ "idempotencyKey": "local-1", "operations": [op] });
    let (st, b) = e
        .call(
            "POST",
            &format!("/v1/vaults/{v}/commits"),
            &tok,
            Some(&d),
            Some(body),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let h = kura_core::ChunkHash::of(data);
    let req = Request::get(format!("/v1/vaults/{v}/chunks/{h}"))
        .header("authorization", format!("Bearer {tok}"))
        .header("x-kura-device", &d)
        .body(Body::empty())
        .unwrap();
    let r = kura_server::app::app(e.state.clone())
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let body = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
    assert_eq!(body.as_ref(), data);
    // A master key of the wrong length is refused at start.
    std::fs::write(&key, "c2hvcnQ=").unwrap();
    assert!(kura_server::store::local_storage(&files).is_err());
    drop(std::fs::remove_dir_all(dir));
}
