//! RFC 0005 phases 1 and 2: a `viewer` binding with folders reads only them,
//! on every read route, through the same filter as a scoped token; the
//! widest grant wins; folders are a viewer's only, in files, in the API and
//! in the database. Opt-in: KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR,
//! KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT.

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use axum::http::StatusCode;
use common::app::{Env, env_with, problem, storage, vault_id};
use kura_core::{ChunkHash, blob_hash};
use serde_json::{Value, json};
use uuid::Uuid;

/// The test configuration with a players' group, bound as RFC 0005 §4.1.
fn config(players: &str) -> String {
    common::app::CONFIG
        .replace(
            "member: [maxime-members] }",
            "member: [maxime-members, maxime-players] }",
        )
        .replace(
            "    - { role: editor, claimValue: maxime-members }",
            &format!("    - {{ role: editor, claimValue: maxime-members }}\n    - {players}"),
        )
}

async fn apply(e: &Env, docs: &str) -> kura_server::reconcile::Outcome {
    let dir = std::env::temp_dir().join(format!("kura-http-{}", e.t.name));
    std::fs::write(dir.join("config.yaml"), docs).unwrap();
    let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
    let outcome = kura_server::reconcile::reconcile(&e.t.app, &files, true)
        .await
        .unwrap();
    e.state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    outcome
}

struct Who {
    token: String,
    device: String,
}

async fn who(e: &Env, sub: &str, groups: &[&str]) -> Who {
    let token = e.key.token(sub, &format!("s-{sub}"), groups);
    let device = e.register(&token, sub).await;
    Who { token, device }
}

impl Who {
    async fn get(&self, e: &Env, uri: &str) -> (StatusCode, Value) {
        e.call("GET", uri, &self.token, Some(&self.device), None)
            .await
    }
    async fn post(&self, e: &Env, uri: &str, body: Value) -> (StatusCode, Value) {
        e.call("POST", uri, &self.token, Some(&self.device), Some(body))
            .await
    }
}

/// A one-chunk file, uploaded and committed by `w`; returns its entry id.
async fn file(e: &Env, w: &Who, v: &str, path: &str, data: &[u8]) -> String {
    let c = ChunkHash::of(data);
    let r = axum::http::Request::builder()
        .method("PUT")
        .uri(format!("/v1/vaults/{v}/chunks/{c}"))
        .header("authorization", format!("Bearer {}", w.token))
        .header("x-kura-device", &w.device)
        .body(axum::body::Body::from(data.to_vec()))
        .unwrap();
    let st = tower::ServiceExt::oneshot(kura_server::app::app(e.state.clone()), r)
        .await
        .unwrap()
        .status();
    assert_eq!(st, StatusCode::NO_CONTENT);
    let size = data.len() as u64;
    let b = blob_hash(size, &[c]).to_string();
    let (st, _) = e
        .call(
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{b}"),
            &w.token,
            Some(&w.device),
            Some(json!({ "size": size, "chunks": [c.to_string()] })),
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let entry = Uuid::now_v7().to_string();
    let op = json!({ "op": "put", "entry": entry, "path": path, "blob": b, "size": size });
    let (st, body) = w
        .post(
            e,
            &format!("/v1/vaults/{v}/commits"),
            json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op] }),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    entry
}

fn paths(p: &Value) -> Vec<&str> {
    let mut out: Vec<&str> = p["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["path"].as_str().unwrap())
        .collect();
    out.sort_unstable();
    out
}

/// T1, T2, §6.P observations 2 and 5 in process: a restricted viewer sees
/// its folders on every read route, a hidden note answers as a missing one,
/// and the widest grant wins.
#[tokio::test]
async fn a_restricted_viewer_reads_its_folders_only() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    apply(
        &e,
        &config("{ role: viewer, claimValue: maxime-players, paths: [Sessions/, resumes/] }"),
    )
    .await;
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    file(&e, &gm, &v, "Sessions/s12.md", b"session twelve").await;
    file(&e, &gm, &v, "Resumes/s12.md", b"recap twelve").await;
    let secret = file(&e, &gm, &v, "MJ/secret.md", b"xyzzy the villain").await;
    let bob = who(&e, "bob", &["maxime-players"]).await;

    // GET /vaults shows the reach.
    let (_, vaults) = bob.get(&e, "/v1/vaults").await;
    let knowledge = vaults
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "knowledge")
        .unwrap();
    assert_eq!(
        (knowledge["role"].as_str(), &knowledge["paths"]),
        (Some("viewer"), &json!(["Sessions/", "resumes/"]))
    );

    let (_, c) = bob
        .get(&e, &format!("/v1/vaults/{v}/changes?since=0"))
        .await;
    assert_eq!(
        paths(&c),
        ["Resumes/s12.md", "Sessions/s12.md"],
        "folded folders"
    );
    let (_, en) = bob.get(&e, &format!("/v1/vaults/{v}/entries")).await;
    assert_eq!(paths(&en), ["Resumes/s12.md", "Sessions/s12.md"]);

    // A hidden note answers exactly as a missing one.
    let hidden = ChunkHash::of(b"xyzzy the villain");
    let hidden_blob = blob_hash(17, &[hidden]);
    let random = ChunkHash::of(b"nobody wrote this");
    for (uri, other) in [
        (
            format!("/v1/vaults/{v}/chunks/{hidden}"),
            format!("/v1/vaults/{v}/chunks/{random}"),
        ),
        (
            format!("/v1/vaults/{v}/blobs/{hidden_blob}"),
            format!("/v1/vaults/{v}/blobs/{random}"),
        ),
        (
            format!("/v1/vaults/{v}/entries/{secret}/revisions"),
            format!("/v1/vaults/{v}/entries/{}/revisions", Uuid::now_v7()),
        ),
    ] {
        let (a, ba) = bob.get(&e, &uri).await;
        let (b, bb) = bob.get(&e, &other).await;
        assert_eq!(a, StatusCode::NOT_FOUND, "{uri}");
        assert_eq!((a, &ba), (b, &bb), "{uri}: hidden answers as missing");
    }
    let (_, batch) = bob
        .post(
            &e,
            &format!("/v1/vaults/{v}/blobs/batch"),
            json!({ "blobs": [hidden_blob.to_string()] }),
        )
        .await;
    assert_eq!(batch["missing"], json!([hidden_blob.to_string()]));
    let visible = blob_hash(14, &[ChunkHash::of(b"session twelve")]);
    let (st, _) = bob
        .get(&e, &format!("/v1/vaults/{v}/blobs/{visible}"))
        .await;
    assert_eq!(st, StatusCode::OK);
    // A viewer never writes, folders or not.
    let (st, b) = bob
        .post(&e, &format!("/v1/vaults/{v}/commits"), json!({ "idempotencyKey": "k", "operations": [{ "op": "delete", "entry": secret, "base": Uuid::now_v7() }] }))
        .await;
    assert_eq!(
        (st, problem(&b)),
        (StatusCode::FORBIDDEN, "/v1/problems/insufficient_role")
    );

    // Observation 5: moved out of the folders, the note is gone with its history.
    let moved = file(&e, &gm, &v, "Sessions/s13.md", b"thirteen").await;
    let (_, h) = gm
        .get(&e, &format!("/v1/vaults/{v}/entries/{moved}/revisions"))
        .await;
    let base = h["items"][0]["revision"].as_str().unwrap().to_owned();
    let b13 = blob_hash(8, &[ChunkHash::of(b"thirteen")]).to_string();
    let op = json!({ "op": "put", "entry": moved, "path": "MJ/s13.md", "base": base, "blob": b13, "size": 8 });
    let (st, _) = gm
        .post(
            &e,
            &format!("/v1/vaults/{v}/commits"),
            json!({ "idempotencyKey": "mv", "operations": [op] }),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = bob
        .get(&e, &format!("/v1/vaults/{v}/entries/{moved}/revisions"))
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // The groups a binding may name, to whoever may bind; not to a viewer.
    let (_, gm_vaults) = gm.get(&e, "/v1/vaults").await;
    let mine = gm_vaults
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "knowledge")
        .unwrap();
    assert_eq!(
        mine["groups"],
        json!([
            "maxime-admins",
            "maxime-members",
            "maxime-owners",
            "maxime-players"
        ])
    );
    assert!(knowledge.get("groups").is_none());

    // The widest grant wins: a player who is also a member edits everything.
    let both = who(&e, "alice", &["maxime-players", "maxime-members"]).await;
    let (_, vaults) = both.get(&e, "/v1/vaults").await;
    let knowledge = vaults
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["name"] == "knowledge")
        .unwrap();
    assert_eq!(
        (knowledge["role"].as_str(), knowledge.get("paths")),
        (Some("editor"), None)
    );
    let (_, en) = both.get(&e, &format!("/v1/vaults/{v}/entries")).await;
    assert!(paths(&en).contains(&"MJ/secret.md"));
}

/// T3: folders are a viewer's only, in the database, the API and the files;
/// a change of folders in the files keeps the binding's id; a restricted
/// claim the tenant maps to admin is warned about.
#[tokio::test]
async fn folders_are_a_viewers_only() {
    let Some(e) = common::app::env(module_path!()).await else {
        return;
    };
    apply(
        &e,
        &config("{ role: viewer, claimValue: maxime-players, paths: [Sessions/] }"),
    )
    .await;
    let v = vault_id(&e, "knowledge").await;
    let row = || async {
        sqlx::query_as::<_, (Uuid, Option<Vec<String>>)>(
            "SELECT id, paths FROM role_binding WHERE subject = 'maxime-players'",
        )
        .fetch_one(&e.t.owner)
        .await
        .unwrap()
    };
    let (id, p) = row().await;
    assert_eq!(p, Some(vec!["Sessions/".to_owned()]));
    apply(
        &e,
        &config("{ role: viewer, claimValue: maxime-players, paths: [Sessions/, Resumes/] }"),
    )
    .await;
    assert_eq!(
        row().await,
        (
            id,
            Some(vec!["Sessions/".to_owned(), "Resumes/".to_owned()])
        ),
        "same id"
    );
    // The database refuses a writer with folders, and an empty list.
    for bad in [
        "UPDATE role_binding SET role = 'editor' WHERE id = $1",
        "UPDATE role_binding SET paths = '{}' WHERE id = $1",
    ] {
        let r = sqlx::query(bad).bind(id).execute(&e.t.owner).await;
        assert!(
            r.unwrap_err()
                .to_string()
                .contains("role_binding_paths_check"),
            "{bad}"
        );
    }
    // A file asking a writer for folders is refused whole; the last
    // configuration stays.
    let outcome = apply(
        &e,
        &config("{ role: editor, claimValue: maxime-players, paths: [Sessions/] }"),
    )
    .await;
    assert!(
        !matches!(outcome, kura_server::reconcile::Outcome::Applied { .. }),
        "{outcome:?}"
    );
    assert_eq!(
        row().await.1,
        Some(vec!["Sessions/".to_owned(), "Resumes/".to_owned()])
    );

    // The API, for an admin.
    let admin = who(&e, "boss", &["maxime-admins", "kura-platform-admins"]).await;
    let put = |b: Value| {
        let (t, d) = (admin.token.clone(), admin.device.clone());
        let e = &e;
        let v = v.clone();
        async move {
            e.call(
                "PUT",
                &format!("/v1/vaults/{v}/bindings/claim:maxime-owners"),
                &t,
                Some(&d),
                Some(b),
            )
            .await
        }
    };
    for (body, why) in [
        (
            json!({ "role": "editor", "paths": ["Sessions/"] }),
            "only a viewer",
        ),
        (json!({ "role": "viewer", "paths": [] }), "1 to 16"),
        (json!({ "role": "viewer", "paths": ["../x/"] }), "folder"),
    ] {
        let (st, b) = put(body).await;
        assert_eq!(
            (st, problem(&b)),
            (StatusCode::BAD_REQUEST, "/v1/problems/invalid_request"),
            "{why}"
        );
        assert!(b["detail"].as_str().unwrap().contains(why), "{b}");
    }
    let (st, b) = put(json!({ "role": "viewer", "paths": ["Notes/"] })).await;
    assert_eq!((st, &b["paths"]), (StatusCode::OK, &json!(["Notes/"])));
    let (st, b) = put(json!({ "role": "editor" })).await;
    assert_eq!(
        (st, b.get("paths")),
        (StatusCode::OK, None),
        "raising the role clears the folders"
    );
    let (_, list) = admin.get(&e, &format!("/v1/vaults/{v}/bindings")).await;
    let players = list
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["subject"] == "claim:maxime-players")
        .unwrap();
    assert_eq!(players["paths"], json!(["Sessions/", "Resumes/"]));

    // §4.3's warning: folders on a claim the tenant maps to admin.
    apply(
        &e,
        &config("{ role: viewer, claimValue: maxime-admins, paths: [Sessions/] }"),
    )
    .await;
    let (_, status) = admin.get(&e, "/v1/admin/status").await;
    let vault = status["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["name"] == "knowledge")
        .unwrap();
    assert_eq!(
        vault["conditions"][1]["type"], "RestrictedBindingWidened",
        "{vault}"
    );
    assert!(e.state.snapshot().unwrap().status.warnings() >= 1);
}

/// RFC 0005 §14, the gate: a restricted viewer walks every vault route of
/// the API with the ids of a hidden note; nothing of it comes back. A route
/// added later that reads content without `ReadAccess` fails here.
#[tokio::test]
async fn no_route_shows_a_restricted_viewer_a_hidden_note() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    apply(
        &e,
        &config("{ role: viewer, claimValue: maxime-players, paths: [Sessions/] }"),
    )
    .await;
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    file(&e, &gm, &v, "Sessions/s12.md", b"session twelve").await;
    let secret = file(&e, &gm, &v, "MJ/secret.md", b"xyzzy the villain").await;
    let chunk = ChunkHash::of(b"xyzzy the villain");
    let blob = blob_hash(17, &[chunk]);
    let bob = who(&e, "bob", &["maxime-players"]).await;
    // What bob does not already know: the content and the hidden path. The
    // ids and hashes he sent may come back (a batch's `missing` lists them,
    // as for a random hash).
    let needles = ["xyzzy".to_owned(), "villain".to_owned(), "MJ/".to_owned()];
    let leaks = |body: &[u8]| {
        let text = String::from_utf8_lossy(body);
        needles.iter().any(|n| text.contains(n.as_str()))
    };
    let mut walked = 0;
    for (method, route, _) in kura_server::limits::LIMITS {
        if !route.starts_with("/v1/vaults/{v}") || route.ends_with("/events") {
            continue;
        }
        let uri = route
            .replace("{v}", &v)
            .replace("{e}", &secret)
            .replace("{hash}", &blob.to_string())
            .replace("{b}", "claim:maxime-players")
            .replace("{t}", &Uuid::now_v7().to_string());
        let body = match *route {
            "/v1/vaults/{v}/blobs/batch" => json!({ "blobs": [blob.to_string()] }),
            "/v1/vaults/{v}/chunks/batch" => json!({ "chunks": [chunk.to_string()] }),
            "/v1/vaults/{v}/chunks/missing" => json!({ "hashes": [chunk.to_string()] }),
            "/v1/vaults/{v}/search" => json!({ "query": "xyzzy" }),
            _ => json!({}),
        };
        let bytes = if *method == "GET" {
            Vec::new()
        } else {
            body.to_string().into_bytes()
        };
        let request = axum::http::Request::builder()
            .method(*method)
            .uri(&uri)
            .header("authorization", format!("Bearer {}", bob.token))
            .header("x-kura-device", &bob.device)
            .header("content-type", "application/json")
            .body(axum::body::Body::from(bytes))
            .unwrap();
        let answer = tower::ServiceExt::oneshot(kura_server::app::app(e.state.clone()), request)
            .await
            .unwrap();
        let status = answer.status();
        let got = axum::body::to_bytes(answer.into_body(), 16 << 20)
            .await
            .unwrap();
        assert!(
            !leaks(&got),
            "{method} {route} ({status}) shows the hidden note: {}",
            String::from_utf8_lossy(&got)
        );
        walked += 1;
    }
    // The chunk route's own id is the chunk hash, not the blob's.
    let (st, b) = bob.get(&e, &format!("/v1/vaults/{v}/chunks/{chunk}")).await;
    assert!(st == StatusCode::NOT_FOUND && !leaks(b.to_string().as_bytes()));
    assert!(walked >= 15, "walked {walked} routes");
}
