//! Commits end to end on PostgreSQL (RFC 0001 §6.3, §6.5; T3, T17). Blobs
//! are inserted as rows: a commit only checks that they are stored.
//! Opt-in: KURA_TEST_DATABASE_URL.

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use axum::http::StatusCode;
use common::app::{Env, env, problem, vault_id};
use kura_core::{ChunkHash, blob_hash};
use serde_json::{Value, json};
use uuid::Uuid;

struct S {
    e: Env,
    tok: String,
    d: String,
    v: String,
}

async fn setup(test: &str) -> Option<S> {
    let e = env(test).await?;
    let tok = e.key.token("u1", "s1", &["maxime-members"]);
    let d = e.register(&tok, "desk:01").await;
    let v = vault_id(&e, "knowledge").await;
    Some(S { e, tok, d, v })
}

/// A second vault of the tenant, with user bindings only.
async fn extra_vault(e: &Env, name: &str, bindings: &[(&str, &str)]) -> String {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO vault (tenant_id, namespace, name, chunking_params, managed_by)
         SELECT tenant_id, namespace, $1, chunking_params, 'api' FROM vault WHERE name = 'knowledge' RETURNING id",
    )
    .bind(name)
    .fetch_one(&e.t.owner)
    .await
    .unwrap();
    for (sub, role) in bindings {
        sqlx::query(
            "INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by)
             SELECT tenant_id, id, 'user', $2, $3, 'api' FROM vault WHERE id = $1",
        )
        .bind(id)
        .bind(sub)
        .bind(role)
        .execute(&e.t.owner)
        .await
        .unwrap();
    }
    id.to_string()
}

/// Stores a one-chunk blob row for `content`; returns its address and size.
async fn blob(e: &Env, v: &str, content: &str) -> (String, u64) {
    let size = content.len() as u64;
    let b = blob_hash(size, &[ChunkHash::of(content.as_bytes())]);
    sqlx::query(
        "INSERT INTO blob (vault_id, tenant_id, hash, size) SELECT id, tenant_id, $2, $3 FROM vault WHERE id = $1::uuid
         ON CONFLICT DO NOTHING",
    )
    .bind(v)
    .bind(b.as_bytes().as_slice())
    .bind(i64::try_from(size).unwrap())
    .execute(&e.t.owner)
    .await
    .unwrap();
    (b.to_string(), size)
}

fn eid() -> String {
    Uuid::now_v7().to_string()
}

impl S {
    async fn commit(&self, ops: Value) -> (StatusCode, Value) {
        self.commit_as(
            &self.tok,
            &self.d,
            &self.v,
            &Uuid::new_v4().to_string(),
            ops,
        )
        .await
    }

    async fn commit_as(
        &self,
        tok: &str,
        d: &str,
        v: &str,
        key: &str,
        ops: Value,
    ) -> (StatusCode, Value) {
        self.e
            .call(
                "POST",
                &format!("/v1/vaults/{v}/commits"),
                tok,
                Some(d),
                Some(json!({ "idempotencyKey": key, "operations": ops })),
            )
            .await
    }

    async fn put(&self, entry: &str, path: &str, content: &str, base: Option<&str>) -> Value {
        let (h, size) = blob(&self.e, &self.v, content).await;
        let mut op = json!({ "op": "put", "entry": entry, "path": path, "blob": h, "size": size });
        if let Some(b) = base {
            op.as_object_mut().unwrap().insert("base".into(), json!(b));
        }
        let (s, b) = self.commit(json!([op])).await;
        assert_eq!(s, StatusCode::OK, "{b}");
        b.pointer("/results/0").unwrap().clone()
    }
}

fn outcome(r: &Value) -> (&str, &str) {
    (
        r.pointer("/outcome").and_then(Value::as_str).unwrap(),
        r.pointer("/head/path").and_then(Value::as_str).unwrap(),
    )
}

#[tokio::test]
async fn fast_forward_rename_and_delete() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let a = eid();
    let r1 = s.put(&a, "notes/a.md", "one", None).await;
    assert_eq!(outcome(&r1), ("accepted", "notes/a.md"));
    assert_eq!(r1["head"]["seq"], 1);
    let rev1 = r1["head"]["revision"].as_str().unwrap();
    let r2 = s.put(&a, "notes/b.md", "two", Some(rev1)).await;
    assert_eq!(outcome(&r2), ("accepted", "notes/b.md"));
    assert_eq!(r2["head"]["seq"], 2);
    // The same content again, from any base, is a no-op.
    let r3 = s.put(&a, "notes/b.md", "two", Some(rev1)).await;
    assert_eq!(r3["head"]["seq"], 2);
    let rev2 = r2["head"]["revision"].as_str().unwrap();
    let (st, b) = s
        .commit(json!([{ "op": "delete", "entry": a, "base": rev2 }]))
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["results"][0]["head"]["deleted"], true);
    assert_eq!(b["headSeq"], 3);
    // The path is free again.
    let r4 = s.put(&eid(), "notes/B.md", "three", None).await;
    assert_eq!(outcome(&r4), ("accepted", "notes/B.md"));
    let (_, vaults) =
        s.e.call("GET", "/v1/vaults", &s.tok, Some(&s.d), None)
            .await;
    let k = vaults
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "knowledge")
        .unwrap();
    assert_eq!(k["headSeq"], 4);
    assert_eq!(k["chunking"]["max"], 4 << 20);
}

#[tokio::test]
async fn diverging_edits_become_a_conflict_file() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let a = eid();
    let r1 = s.put(&a, "Meeting.md", "base", None).await;
    let rev1 = r1["head"]["revision"].as_str().unwrap().to_owned();
    let r2 = s.put(&a, "Meeting.md", "edit on desk", Some(&rev1)).await;
    assert_eq!(r2["outcome"], "accepted");
    // A second device edited the same base.
    let r3 = s.put(&a, "Meeting.md", "edit on phone", Some(&rev1)).await;
    assert_eq!(outcome(&r3), ("conflicted", "Meeting.md"));
    assert_eq!(r3["head"]["revision"], r2["head"]["revision"], "head kept");
    let today = time::OffsetDateTime::now_utc().date();
    assert_eq!(
        r3["conflict"]["path"],
        format!("Meeting (conflict desk-01 {today}).md"),
        "the device name is cleaned into a valid name"
    );
    // Both versions are stored: nothing is lost.
    let blobs: Vec<Vec<u8>> = sqlx::query_scalar("SELECT r.blob_hash FROM entry e JOIN revision r ON r.id = e.head_revision WHERE NOT e.deleted")
        .fetch_all(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(blobs.len(), 2);
    // A second conflict on the same file gets its own name.
    let r4 = s.put(&a, "Meeting.md", "edit on tablet", Some(&rev1)).await;
    assert_eq!(
        r4["conflict"]["path"],
        format!("Meeting (conflict desk-01 {today}) (2).md")
    );
}

/// Review 4 H2: a push retried because its conflict file could not be
/// moved locally (an open note) lands on the same conflict file; a name long
/// enough to be shortened too (review 2026-11-15 #18).
#[tokio::test]
async fn the_same_content_pushed_again_reuses_its_conflict_file() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let long = format!("{}.md", "l".repeat(240));
    for name in ["Open.md", long.as_str()] {
        let stem = name.trim_end_matches(".md").get(..20).unwrap_or("Open");
        let a = eid();
        let base = s.put(&a, name, "base", None).await["head"]["revision"]
            .as_str()
            .unwrap()
            .to_owned();
        s.put(&a, name, "edited elsewhere", Some(&base)).await;
        let first = s.put(&a, name, "typed here", Some(&base)).await;
        let again = s.put(&a, name, "typed here", Some(&base)).await;
        assert_eq!(first["outcome"], "conflicted", "{name}");
        assert_eq!(
            again["conflict"]["entry"], first["conflict"]["entry"],
            "{name}"
        );
        let n: i64 =
            sqlx::query_scalar("SELECT count(*) FROM entry WHERE NOT deleted AND path LIKE $1")
                .bind(format!("{stem}%"))
                .fetch_one(&s.e.t.owner)
                .await
                .unwrap();
        assert_eq!(n, 2, "one note, one conflict file: {name}");
        // A new file pushed twice onto an occupied path, likewise.
        let c1 = s.put(&eid(), name, "another", None).await;
        let c2 = s.put(&eid(), name, "another", None).await;
        assert_eq!(c2["conflict"]["entry"], c1["conflict"]["entry"], "{name}");
    }
}

#[tokio::test]
async fn section_6_5_rows_without_merge() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    // Rename on one side, edit on the other: both apply.
    let a = eid();
    let rev1 = s.put(&a, "x.md", "v1", None).await["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let renamed = s.put(&a, "y.md", "v1", Some(&rev1)).await;
    assert_eq!(outcome(&renamed), ("accepted", "y.md"));
    let edited = s.put(&a, "x.md", "v2", Some(&rev1)).await;
    assert_eq!(
        outcome(&edited),
        ("accepted", "y.md"),
        "the edit keeps the server's rename"
    );
    // Delete against an entry edited since: dropped.
    let (_, b) = s
        .commit(json!([{ "op": "delete", "entry": a, "base": rev1 }]))
        .await;
    assert_eq!(b["results"][0]["outcome"], "conflicted");
    assert_eq!(b["results"][0]["head"]["deleted"], false);
    // Edit against a deleted entry: the edit wins and restores it.
    let head = edited["head"]["revision"].as_str().unwrap().to_owned();
    s.commit(json!([{ "op": "delete", "entry": a, "base": head }]))
        .await;
    let restored = s.put(&a, "y.md", "v3", Some(&head)).await;
    assert_eq!(outcome(&restored), ("accepted", "y.md"));
    assert_eq!(restored["head"]["deleted"], false);
    // Same path created on two devices: identical content is unified…
    let b1 = eid();
    s.put(&b1, "same.md", "hello", None).await;
    let unified = s.put(&eid(), "SAME.md", "hello", None).await;
    assert_eq!(outcome(&unified), ("accepted", "same.md"));
    assert_eq!(unified["head"]["entry"], b1.as_str());
    // …other content becomes a conflict file under the pushed id.
    let c = eid();
    let other = s.put(&c, "same.md", "different", None).await;
    assert_eq!(other["outcome"], "conflicted");
    assert_eq!(other["conflict"]["entry"], c.as_str());
    // A rename onto an occupied path gets a suffix.
    let d = eid();
    let dr = s.put(&d, "d.md", "d", None).await["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let moved = s.put(&d, "y.md", "d", Some(&dr)).await;
    assert_eq!(outcome(&moved), ("accepted", "y (2).md"));
}

#[tokio::test]
async fn a_crowded_name_still_gets_a_unique_one() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    for name in [
        "x.md", "x (2).md", "x (3).md", "x (4).md", "x (5).md", "x (6).md", "x (7).md", "x (8).md",
    ] {
        s.put(&eid(), name, name, None).await;
    }
    // The last numbered suffix is tried too (review 2026-11-15 #18).
    let n9 = eid();
    let rev = s.put(&n9, "n.md", "n", None).await["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let moved = s.put(&n9, "x.md", "n", Some(&rev)).await;
    assert_eq!(moved["head"]["path"], "x (9).md", "{moved}");
    let a = eid();
    let r = s.put(&a, "x.md", "one more", None).await;
    assert_eq!(r["outcome"], "conflicted", "{r}");
    let path = r["conflict"]["path"].as_str().unwrap();
    assert!(path.starts_with("x (conflict desk-01 "), "{path}");
    // Past the numbered suffixes, the entry id makes the name unique.
    let d = eid();
    let rev = s.put(&d, "d.md", "d", None).await["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let moved = s.put(&d, "x.md", "d", Some(&rev)).await;
    let tail = &d.replace('-', "")[20..];
    assert_eq!(moved["head"]["path"], format!("x ({tail}).md"));
}

#[tokio::test]
async fn a_path_freed_and_taken_in_one_commit() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let a = eid();
    let rev = s.put(&a, "p.md", "a", None).await["head"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let (ha, sa) = blob(&s.e, &s.v, "a").await;
    let (hb, sb) = blob(&s.e, &s.v, "b").await;
    let (st, b) = s
        .commit(json!([
            { "op": "put", "entry": a, "path": "q.md", "base": rev, "blob": ha, "size": sa },
            { "op": "put", "entry": eid(), "path": "p.md", "blob": hb, "size": sb },
        ]))
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(outcome(&b["results"][0]), ("accepted", "q.md"));
    assert_eq!(outcome(&b["results"][1]), ("accepted", "p.md"));
}

#[tokio::test]
async fn viewer_cannot_commit() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let archive = extra_vault(&s.e, "archive", &[("v1", "viewer")]).await;
    let tok = s.e.key.token("v1", "sv", &["maxime-members"]);
    let d = s.e.register(&tok, "viewer-phone").await;
    let (h, size) = blob(&s.e, &archive, "x").await;
    let (st, b) = s
        .commit_as(
            &tok,
            &d,
            &archive,
            "k",
            json!([{ "op": "put", "entry": eid(), "path": "a.md", "blob": h, "size": size }]),
        )
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role")
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM revision")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

#[tokio::test]
async fn commit_with_foreign_entry_id_is_refused() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let archive = extra_vault(&s.e, "archive", &[("u1", "editor")]).await;
    let a = eid();
    let r = s.put(&a, "a.md", "secret", None).await;
    let rev = r["head"]["revision"].as_str().unwrap();
    // The same entry id and its revision, pushed to another vault.
    let (h, size) = blob(&s.e, &archive, "overwrite").await;
    let (st, b) = s
        .commit_as(&s.tok, &s.d, &archive, "k", json!([{ "op": "put", "entry": a, "path": "a.md", "base": rev, "blob": h, "size": size }]))
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request"),
        "{b}"
    );
    let (st, b) = s
        .commit_as(
            &s.tok,
            &s.d,
            &archive,
            "k2",
            json!([{ "op": "delete", "entry": a, "base": rev }]),
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{b}");
    // The original entry is untouched.
    let seq: i64 = sqlx::query_scalar(
        "SELECT last_seq FROM entry WHERE id = $1::uuid AND vault_id = $2::uuid",
    )
    .bind(&a)
    .bind(&s.v)
    .fetch_one(&s.e.t.owner)
    .await
    .unwrap();
    assert_eq!(seq, 1);
}

#[tokio::test]
async fn idempotency_key_reuse_different_body_is_422() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (h, size) = blob(&s.e, &s.v, "x").await;
    let a = eid();
    let ops = json!([{ "op": "put", "entry": a, "path": "a.md", "blob": h, "size": size }]);
    let (st1, b1) = s.commit_as(&s.tok, &s.d, &s.v, "k1", ops.clone()).await;
    assert_eq!(st1, StatusCode::OK, "{b1}");
    // A retry returns the stored result and writes nothing.
    let (st2, b2) = s.commit_as(&s.tok, &s.d, &s.v, "k1", ops).await;
    assert_eq!((st2, &b2), (StatusCode::OK, &b1));
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM revision")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(n, 1);
    let other = json!([{ "op": "put", "entry": eid(), "path": "b.md", "blob": h, "size": size }]);
    let (st3, b3) = s.commit_as(&s.tok, &s.d, &s.v, "k1", other.clone()).await;
    assert_eq!(
        (st3, problem(&b3)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/idempotency_conflict"
        )
    );
    // The key is scoped to the device: another device may use it.
    let d2 = s.e.register(&s.tok, "phone").await;
    let (st4, _) = s.commit_as(&s.tok, &d2, &s.v, "k1", other).await;
    assert_eq!(st4, StatusCode::OK);
}

#[tokio::test]
async fn invalid_input_fails_the_whole_commit() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (h, size) = blob(&s.e, &s.v, "x").await;
    for (path, kind) in [
        ("../etc/passwd", "/v1/problems/invalid_path"),
        (".obsidian/plugins/x/main.js", "/v1/problems/invalid_path"),
        ("Cafe\u{301}.md", "/v1/problems/invalid_path"),
    ] {
        let (st, b) = s
            .commit(json!([{ "op": "put", "entry": eid(), "path": path, "blob": h, "size": size }]))
            .await;
        assert_eq!(
            (st, problem(&b)),
            (StatusCode::UNPROCESSABLE_ENTITY, kind),
            "{path}"
        );
    }
    let missing = ChunkHash::of(b"never uploaded").to_string();
    let (st, b) = s
        .commit(json!([
            { "op": "put", "entry": eid(), "path": "ok.md", "blob": h, "size": size },
            { "op": "put", "entry": eid(), "path": "b.md", "blob": missing, "size": 1 },
        ]))
        .await;
    assert_eq!(
        (st, problem(&b)),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "/v1/problems/hash_mismatch"
        )
    );
    let (st, _) = s
        .commit(
            json!([{ "op": "put", "entry": eid(), "path": "c.md", "blob": h, "size": size + 1 }]),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let a = eid();
    let (st, _) = s
        .commit(json!([
            { "op": "put", "entry": a, "path": "d.md", "blob": h, "size": size },
            { "op": "put", "entry": a, "path": "e.md", "blob": h, "size": size },
        ]))
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = s.commit(json!([])).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = s.commit(json!([{ "op": "put", "entry": Uuid::new_v4(), "path": "f.md", "blob": h, "size": size }])).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "entry ids are UUIDv7");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM revision")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(n, 0, "nothing was written");
}

#[tokio::test]
async fn concurrent_commits_never_skip_a_seq() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let s = std::sync::Arc::new(s);
    let mut tasks = Vec::new();
    for i in 0..16 {
        let s = s.clone();
        tasks.push(tokio::spawn(async move {
            let (h, size) = blob(&s.e, &s.v, &format!("content {i}")).await;
            let ops: Vec<Value> = (0..5)
                .map(|j| json!({ "op": "put", "entry": eid(), "path": format!("c/{i}-{j}.md"), "blob": h, "size": size }))
                .collect();
            let (st, b) = s.commit(json!(ops)).await;
            assert_eq!(st, StatusCode::OK, "{b}");
        }));
    }
    for t in tasks {
        t.await.unwrap();
    }
    let seqs: Vec<i64> = sqlx::query_scalar("SELECT seq FROM revision ORDER BY seq")
        .fetch_all(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(seqs, (1..=80).collect::<Vec<i64>>());
    let head: i64 = sqlx::query_scalar("SELECT head_seq FROM vault WHERE name = 'knowledge'")
        .fetch_one(&s.e.t.owner)
        .await
        .unwrap();
    assert_eq!(head, 80);
}

#[tokio::test]
async fn old_commit_records_are_purged() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (h, size) = blob(&s.e, &s.v, "x").await;
    s.commit(json!([{ "op": "put", "entry": eid(), "path": "a.md", "blob": h, "size": size }]))
        .await;
    sqlx::query("UPDATE commit_record SET created_at = now() - interval '8 days'")
        .execute(&s.e.t.owner)
        .await
        .unwrap();
    let purged: i64 = sqlx::query_scalar("SELECT kura_commit_purge()")
        .fetch_one(&s.e.t.app)
        .await
        .unwrap();
    assert_eq!(purged, 1);
}

impl S {
    async fn changes(&self, query: &str) -> (StatusCode, Value) {
        self.e
            .call(
                "GET",
                &format!("/v1/vaults/{}/changes?{query}", self.v),
                &self.tok,
                Some(&self.d),
                None,
            )
            .await
    }
}

#[tokio::test]
async fn changes_page_by_seq_and_refuse_a_lost_history() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (h, size) = blob(&s.e, &s.v, "x").await;
    // 1,001 entries in two commits; the first holds the 1,000-operation maximum.
    let ops: Vec<Value> = (0..1000)
        .map(|i| json!({ "op": "put", "entry": eid(), "path": format!("n/{i}.md"), "blob": h, "size": size }))
        .collect();
    let (st, b) = s.commit(json!(ops)).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let last = eid();
    s.put(&last, "z.md", "x", None).await;
    let (st, p1) = s.changes("since=0").await;
    assert_eq!(st, StatusCode::OK, "{p1}");
    assert_eq!(p1["items"].as_array().unwrap().len(), 1000);
    assert_eq!(p1["headSeq"], 1001);
    assert_eq!(p1["next"], "1000");
    let epoch = p1["epoch"].as_str().unwrap().to_owned();
    let (_, p2) = s.changes(&format!("since=1000&epoch={epoch}")).await;
    assert_eq!(p2["items"].as_array().unwrap().len(), 1);
    assert_eq!(p2["items"][0]["entry"], last.as_str());
    assert!(p2.get("next").is_none());
    // One item per entry: its current state, even after several revisions.
    let rev = p2["items"][0]["revision"].as_str().unwrap().to_owned();
    s.put(&last, "z2.md", "x", Some(&rev)).await;
    let (_, p3) = s.changes(&format!("since=1001&epoch={epoch}")).await;
    assert_eq!(p3["items"].as_array().unwrap().len(), 1);
    assert_eq!(p3["items"][0]["path"], "z2.md");
    // A cursor from another epoch, or above the head: the server lost history.
    let (st, b) = s
        .changes(&format!("since=5&epoch={}", Uuid::new_v4()))
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::GONE, "/v1/problems/history_rewound")
    );
    let (st, b) = s.changes("since=99999").await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::GONE, "/v1/problems/history_rewound")
    );
    let (st, _) = s.changes("since=-1").await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    // entries: live ones, by id.
    let (st, en) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/entries", s.v),
            &s.tok,
            Some(&s.d),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{en}");
    assert_eq!(en["items"].as_array().unwrap().len(), 1000);
    let after = en["next"].as_str().unwrap();
    let (_, en2) =
        s.e.call(
            "GET",
            &format!("/v1/vaults/{}/entries?after={after}", s.v),
            &s.tok,
            Some(&s.d),
            None,
        )
        .await;
    assert_eq!(en2["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn a_pull_during_commits_never_passes_an_unseen_change() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let s = std::sync::Arc::new(s);
    let writers: Vec<_> = (0..12)
        .map(|i| {
            let s = s.clone();
            tokio::spawn(async move {
                let (h, size) = blob(&s.e, &s.v, &format!("w{i}")).await;
                for j in 0..4 {
                    let (st, b) = s
                        .commit(json!([{ "op": "put", "entry": eid(), "path": format!("w/{i}-{j}.md"), "blob": h, "size": size }]))
                        .await;
                    assert_eq!(st, StatusCode::OK, "{b}");
                }
            })
        })
        .collect();
    // A device pulling by cursor while the writers run.
    let mut seen = std::collections::HashSet::new();
    let mut cursor = 0u64;
    let pull = async |cursor: &mut u64, seen: &mut std::collections::HashSet<String>| loop {
        let (st, b) = s.changes(&format!("since={cursor}")).await;
        assert_eq!(st, StatusCode::OK, "{b}");
        for it in b["items"].as_array().unwrap() {
            seen.insert(it["entry"].as_str().unwrap().to_owned());
            *cursor = it["seq"].as_u64().unwrap();
        }
        if b.get("next").is_none() {
            break;
        }
    };
    while !writers.iter().all(tokio::task::JoinHandle::is_finished) {
        pull(&mut cursor, &mut seen).await;
    }
    for w in writers {
        w.await.unwrap();
    }
    pull(&mut cursor, &mut seen).await;
    assert_eq!(
        seen.len(),
        48,
        "every change reached the device exactly by its cursor"
    );
    assert_eq!(cursor, 48);
}
