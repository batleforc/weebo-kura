//! RFC 0007 phase 1: a folder is an entry with no content. Created, unified
//! and deleted by commits; sent only to a client that asks for folders; under
//! the same scope as notes (T1, T2). Opt-in: KURA_TEST_DATABASE_URL,
//! KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN, KURA_TEST_S3_ENDPOINT.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use axum::http::StatusCode;
use common::app::{Env, Who, env_stored, file, problem, vault_id, who};
use serde_json::{Value, json};
use uuid::Uuid;

impl Who {
    async fn folder(&self, e: &Env, v: &str, path: &str) -> (StatusCode, Value) {
        let entry = Uuid::now_v7().to_string();
        self.commit(
            e,
            v,
            json!([{ "op": "folder", "entry": entry, "path": path }]),
        )
        .await
    }
    /// Paths of a full pass, folders marked with a trailing `/`.
    async fn paths(&self, e: &Env, v: &str, folders: bool) -> Vec<String> {
        let (st, b) = self
            .call(
                e,
                "GET",
                &format!("/v1/vaults/{v}/changes?since=0&folders={folders}"),
                None,
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{b}");
        let mut out: Vec<String> = b["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["deleted"] == false)
            .map(|i| {
                let p = i["path"].as_str().unwrap().to_owned();
                if i["folder"] == true {
                    format!("{p}/")
                } else {
                    p
                }
            })
            .collect();
        out.sort();
        out
    }
}

/// §4.2: created, unified with the same folder made elsewhere, refused where
/// a file holds the name, deleted; never sent to a client that did not ask.
#[tokio::test]
async fn a_folder_is_an_entry_older_clients_never_see() {
    let Some(e) = env_stored(module_path!()).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let a = who(&e, "a", &["maxime-members"]).await;
    let b = who(&e, "b", &["maxime-members"]).await;

    let (st, r) = a.folder(&e, &v, "JDR/Empty").await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let made = &r["results"][0]["head"];
    assert_eq!(
        (&made["folder"], &made["size"]),
        (&json!(true), &json!(0)),
        "{r}"
    );
    assert!(made.get("blob").is_none());

    // The same folder from another device: one entry for both.
    let (_, r2) = b.folder(&e, &v, "JDR/Empty").await;
    assert_eq!(r2["results"][0]["outcome"], "accepted");
    assert_eq!(r2["results"][0]["head"]["entry"], made["entry"], "{r2}");

    file(&e, &a, &v, "JDR/Taken", b"a file").await;
    let (_, r3) = a.folder(&e, &v, "JDR/Taken").await;
    assert_eq!(r3["results"][0]["outcome"], "conflicted", "{r3}");

    assert_eq!(b.paths(&e, &v, true).await, ["JDR/Empty/", "JDR/Taken"]);
    // An older client: the folder is left out.
    assert_eq!(b.paths(&e, &v, false).await, ["JDR/Taken"]);

    // A folder is not a file, and a file not a folder.
    let entry = made["entry"].as_str().unwrap();
    let (st, body) = a
        .commit(&e, &v, json!([{ "op": "put", "entry": entry, "base": made["revision"], "path": "JDR/Empty", "blob": "0".repeat(64), "size": 0 }]))
        .await;
    assert_ne!(st, StatusCode::OK, "{body}");

    let (st, _) = a
        .commit(
            &e,
            &v,
            json!([{ "op": "delete", "entry": entry, "base": made["revision"] }]),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(b.paths(&e, &v, true).await, ["JDR/Taken"]);
}

/// T1 and T2: a restricted viewer sees its own root folder, even empty, and
/// nothing outside; a token writes a folder only where it writes notes.
#[tokio::test]
async fn folders_follow_the_scope_of_notes() {
    let Some(e) = env_stored(module_path!()).await else {
        return;
    };
    let docs = common::app::CONFIG
        .replace(
            "member: [maxime-members] }",
            "member: [maxime-members, maxime-players] }",
        )
        .replace(
            "    - { role: editor, claimValue: maxime-members }",
            "    - { role: editor, claimValue: maxime-members }\n    - { role: viewer, claimValue: maxime-players, paths: [Sessions/] }",
        )
        .replace(
            "metadata: { name: knowledge, namespace: maxime }\nspec:\n  tenantRef: { name: maxime }\n",
            "metadata: { name: knowledge, namespace: maxime }\nspec:\n  tenantRef: { name: maxime }\n  configSync: { categories: [snippets] }\n",
        );
    e.apply_config(&docs).await;
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    for p in ["Sessions", "Sessions/Prep", "MJ", "MJ/Secret"] {
        let (st, r) = gm.folder(&e, &v, p).await;
        assert_eq!(st, StatusCode::OK, "{r}");
    }
    let pia = who(&e, "pia", &["maxime-players"]).await;
    assert_eq!(
        pia.paths(&e, &v, true).await,
        ["Sessions/", "Sessions/Prep/"]
    );
    let (st, b) = pia
        .call(
            &e,
            "GET",
            &format!("/v1/vaults/{v}/entries?folders=true"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert!(!b.to_string().contains("MJ"), "{b}");

    // A viewer writes nothing, folders included.
    let (st, _) = pia.folder(&e, &v, "Sessions/New").await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    // The configuration space holds files only.
    let (st, b) = gm
        .call(
            &e,
            "POST",
            &format!("/v1/vaults/{v}/commits"),
            Some(json!({ "idempotencyKey": "k1", "space": "config", "operations": [{ "op": "folder", "entry": Uuid::now_v7().to_string(), "path": "snippets" }] })),
        )
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert!(problem(&b).ends_with("invalid_path"), "{b}");
}
