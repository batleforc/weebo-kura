//! The server-side three-way merge (RFC 0001 §6.5, phase 4) end to end:
//! content uploaded, merged from storage, the result readable. Opt-in:
//! PostgreSQL, Vault (or OpenBao) and S3, as `tests/chunks.rs`.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app::{Env, env_with, storage, vault_id};
use kura_core::{ChunkHash, blob_hash};
use kura_server::app::app;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

struct S {
    e: Env,
    tok: String,
    v: String,
}

async fn setup(test: &str) -> Option<S> {
    let st = storage(test).await?;
    let e = env_with(test, Some(st)).await?;
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let v = vault_id(&e, "knowledge").await;
    Some(S { e, tok, v })
}

impl S {
    async fn raw(&self, method: &str, uri: &str, d: &str, body: Vec<u8>) -> (StatusCode, Vec<u8>) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {}", self.tok))
            .header("x-kura-device", d)
            .body(Body::from(body))
            .unwrap();
        let r = app(self.e.state.clone()).oneshot(req).await.unwrap();
        let status = r.status();
        (
            status,
            axum::body::to_bytes(r.into_body(), 8 << 20)
                .await
                .unwrap()
                .to_vec(),
        )
    }

    /// Uploads `content` as one chunk and its manifest; returns (blob, size).
    async fn upload(&self, d: &str, content: &str) -> (String, u64) {
        let h = ChunkHash::of(content.as_bytes());
        let (st, _) = self
            .raw(
                "PUT",
                &format!("/v1/vaults/{}/chunks/{h}", self.v),
                d,
                content.as_bytes().to_vec(),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        let size = content.len() as u64;
        let b = blob_hash(size, &[h]);
        let (st, _) = self
            .e
            .call(
                "PUT",
                &format!("/v1/vaults/{}/blobs/{b}", self.v),
                &self.tok,
                Some(d),
                Some(json!({ "size": size, "chunks": [h] })),
            )
            .await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        (b.to_string(), size)
    }

    async fn put(
        &self,
        d: &str,
        entry: &str,
        path: &str,
        content: &str,
        base: Option<&str>,
    ) -> Value {
        let (blob, size) = self.upload(d, content).await;
        let mut op =
            json!({ "op": "put", "entry": entry, "path": path, "blob": blob, "size": size });
        if let Some(b) = base {
            op["base"] = json!(b);
        }
        let (st, b) = self
            .e
            .call(
                "POST",
                &format!("/v1/vaults/{}/commits", self.v),
                &self.tok,
                Some(d),
                Some(json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op] })),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{b}");
        b["results"][0].clone()
    }

    /// The stored content of a blob, as a client downloads it.
    async fn content(&self, d: &str, blob: &str) -> String {
        let (_, m) = self
            .e
            .call(
                "GET",
                &format!("/v1/vaults/{}/blobs/{blob}", self.v),
                &self.tok,
                Some(d),
                None,
            )
            .await;
        let mut out = Vec::new();
        for c in m["chunks"].as_array().unwrap() {
            let (st, part) = self
                .raw(
                    "GET",
                    &format!("/v1/vaults/{}/chunks/{}", self.v, c.as_str().unwrap()),
                    d,
                    Vec::new(),
                )
                .await;
            assert_eq!(st, StatusCode::OK);
            out.extend(part);
        }
        String::from_utf8(out).unwrap()
    }
}

fn rev(r: &Value) -> String {
    r["head"]["revision"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn edits_of_different_lines_merge() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, phone) = (
        s.e.register(&s.tok, "desk").await,
        s.e.register(&s.tok, "phone").await,
    );
    let a = Uuid::now_v7().to_string();
    let base = rev(&s.put(&desk, &a, "plan.md", "one\ntwo\nthree\n", None).await);
    assert_eq!(
        s.put(&desk, &a, "plan.md", "one, desk\ntwo\nthree\n", Some(&base))
            .await["outcome"],
        "accepted"
    );
    let r = s
        .put(
            &phone,
            &a,
            "plan.md",
            "one\ntwo\nthree, phone\n",
            Some(&base),
        )
        .await;
    assert_eq!(r["outcome"], "merged", "{r}");
    assert!(r.get("conflict").is_none());
    let merged = s.content(&phone, r["head"]["blob"].as_str().unwrap()).await;
    assert_eq!(merged, "one, desk\ntwo\nthree, phone\n");
    assert_eq!(
        s.e.state
            .metrics
            .merge_total
            .with_label_values(&["merged"])
            .get(),
        1
    );
    // The same edit again, later: already in the head, a no-op.
    let again = s
        .put(
            &phone,
            &a,
            "plan.md",
            "one\ntwo\nthree, phone\n",
            Some(&base),
        )
        .await;
    assert_eq!(again["head"]["revision"], r["head"]["revision"]);
}

#[tokio::test]
async fn overlapping_and_non_text_edits_conflict() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, phone) = (
        s.e.register(&s.tok, "desk").await,
        s.e.register(&s.tok, "phone").await,
    );
    for (path, base_text, desk_text, phone_text) in [
        ("same.md", "line\n", "desk line\n", "phone line\n"),
        (
            "board.canvas",
            "{\n\"a\": 1,\n\"b\": 2\n}\n",
            "{\n\"a\": 10,\n\"b\": 2\n}\n",
            "{\n\"a\": 1,\n\"b\": 20\n}\n",
        ),
    ] {
        let a = Uuid::now_v7().to_string();
        let base = rev(&s.put(&desk, &a, path, base_text, None).await);
        s.put(&desk, &a, path, desk_text, Some(&base)).await;
        let r = s.put(&phone, &a, path, phone_text, Some(&base)).await;
        assert_eq!(r["outcome"], "conflicted", "{path}: {r}");
        assert_eq!(
            s.content(&phone, r["conflict"]["blob"].as_str().unwrap())
                .await,
            phone_text
        );
    }
}

#[tokio::test]
async fn a_rename_on_one_side_and_edits_on_both_merge() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, phone) = (
        s.e.register(&s.tok, "desk").await,
        s.e.register(&s.tok, "phone").await,
    );
    let a = Uuid::now_v7().to_string();
    let base = rev(&s.put(&desk, &a, "a.md", "one\ntwo\nthree\n", None).await);
    s.put(
        &desk,
        &a,
        "renamed.md",
        "one, desk\ntwo\nthree\n",
        Some(&base),
    )
    .await;
    let r = s
        .put(&phone, &a, "a.md", "one\ntwo\nthree, phone\n", Some(&base))
        .await;
    assert_eq!(
        (r["outcome"].as_str(), r["head"]["path"].as_str()),
        (Some("merged"), Some("renamed.md")),
        "{r}"
    );
}

/// Eight devices change their own line of one note at once: every change
/// lands, merged into the head or kept in a conflict file; none is lost,
/// whatever the interleaving of prepare and lock.
#[tokio::test]
async fn concurrent_line_edits_lose_nothing() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let s = std::sync::Arc::new(s);
    let desk = s.e.register(&s.tok, "desk").await;
    let lines: Vec<String> = (0..8).map(|i| format!("line {i}")).collect();
    let base_text = lines.join("\n") + "\n";
    let a = Uuid::now_v7().to_string();
    let base = rev(&s.put(&desk, &a, "shared.md", &base_text, None).await);
    let mut tasks = Vec::new();
    for i in 0..8 {
        let (s, a, base, lines) = (s.clone(), a.clone(), base.clone(), lines.clone());
        tasks.push(tokio::spawn(async move {
            let d = s.e.register(&s.tok, &format!("dev{i}")).await;
            let mut mine = lines.clone();
            mine[i] = format!("line {i}, edited by dev{i}");
            let r = s
                .put(&d, &a, "shared.md", &(mine.join("\n") + "\n"), Some(&base))
                .await;
            (d, r)
        }));
    }
    let mut texts = Vec::new();
    let mut last = None;
    for t in tasks {
        let (d, r) = t.await.unwrap();
        if let Some(c) = r.get("conflict") {
            texts.push(s.content(&d, c["blob"].as_str().unwrap()).await);
        }
        last = Some(d);
    }
    let d = last.unwrap();
    let (_, page) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/changes?since=0", s.v),
            &s.tok,
            Some(&d),
            None,
        )
        .await;
    let head = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|it| it["entry"] == a.as_str())
        .unwrap()
        .clone();
    texts.push(s.content(&d, head["blob"].as_str().unwrap()).await);
    for i in 0..8 {
        let edit = format!("line {i}, edited by dev{i}");
        assert!(
            texts.iter().any(|t| t.contains(&edit)),
            "lost: {edit}\n{texts:#?}"
        );
    }
    assert!(
        s.e.state
            .metrics
            .merge_total
            .with_label_values(&["merged"])
            .get()
            >= 1
    );
}
