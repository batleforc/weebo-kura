//! RFC 0006: where each device stands on a vault. Written from the device's
//! own pulls and commits; read back by role (§7.2, T1–T5). Opt-in:
//! KURA_TEST_DATABASE_URL, KURA_TEST_VAULT_ADDR, KURA_TEST_VAULT_TOKEN,
//! KURA_TEST_S3_ENDPOINT.

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use axum::http::StatusCode;
use common::app::{Env, env, env_with, problem, storage, vault_id};
use kura_core::{ChunkHash, blob_hash};
use serde_json::{Value, json};
use uuid::Uuid;

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
    async fn call(
        &self,
        e: &Env,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        e.call(method, uri, &self.token, Some(&self.device), body)
            .await
    }
    /// A full pass of the notes, from 0.
    async fn pull(&self, e: &Env, v: &str) {
        let (st, b) = self
            .call(e, "GET", &format!("/v1/vaults/{v}/changes?since=0"), None)
            .await;
        assert_eq!(st, StatusCode::OK, "{b}");
        assert!(b.get("next").is_none(), "one page");
    }
    async fn sync(&self, e: &Env, v: &str) -> (StatusCode, Value) {
        self.call(e, "GET", &format!("/v1/vaults/{v}/sync"), None)
            .await
    }
}

/// A one-chunk file, uploaded and committed by `w`.
async fn file(e: &Env, w: &Who, v: &str, path: &str, data: &[u8]) {
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
    let (st, _) = w
        .call(
            e,
            "PUT",
            &format!("/v1/vaults/{v}/blobs/{b}"),
            Some(json!({ "size": size, "chunks": [c.to_string()] })),
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let op = json!({ "op": "put", "entry": Uuid::now_v7().to_string(), "path": path, "blob": b, "size": size });
    let (st, body) = w
        .call(
            e,
            "POST",
            &format!("/v1/vaults/{v}/commits"),
            Some(json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{body}");
}

fn row<'a>(list: &'a Value, device: &str) -> &'a Value {
    list.as_array()
        .unwrap()
        .iter()
        .find(|r| r["device"] == device)
        .unwrap_or(&Value::Null)
}

/// §6.P observations 1 and 2, T1: a pass records the head, a commit records
/// the push, "behind" follows the vault's head; a member sees their own
/// devices only, a tenant admin sees everyone's with the owner.
#[tokio::test]
async fn a_pass_records_where_the_device_stands() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    let bob = who(&e, "bob", &["maxime-members"]).await;
    let carol = who(&e, "carol", &["maxime-members"]).await;
    file(&e, &gm, &v, "a.md", b"one").await;

    let (st, list) = bob.sync(&e, &v).await;
    assert_eq!(st, StatusCode::OK, "{list}");
    assert_eq!(list, json!([]), "nothing before a pass");

    bob.pull(&e, &v).await;
    let (_, list) = bob.sync(&e, &v).await;
    let mine = row(&list, &bob.device);
    assert_eq!(mine["behind"], 0, "{list}");
    assert_eq!(mine["resync"], false);
    assert!(mine["caughtUpAt"].is_string());
    assert!(mine.get("owner").is_none(), "a member is not told owners");

    file(&e, &gm, &v, "b.md", b"two").await;
    file(&e, &gm, &v, "c.md", b"three").await;
    let (_, list) = bob.sync(&e, &v).await;
    assert_eq!(row(&list, &bob.device)["behind"], 2, "{list}");
    assert_eq!(list.as_array().unwrap().len(), 1, "T1: only bob's own");

    // T1: carol reads the vault, has no device on it, sees nobody.
    let (st, list) = carol.sync(&e, &v).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(list, json!([]));

    // A tenant admin sees every device, owners named, the push recorded.
    let (_, list) = gm.sync(&e, &v).await;
    assert_eq!(row(&list, &bob.device)["owner"], "bob", "{list}");
    let own = row(&list, &gm.device);
    assert!(own["pushedAt"].is_string(), "{list}");
    assert!(own.get("caughtUpAt").is_none(), "pushed, never pulled");
    assert!(own.get("owner").is_none(), "one's own device is not named");

    // The Devices page: the vaults each own device caught up on.
    let (_, devices) = bob.call(&e, "GET", "/v1/devices", None).await;
    let d = devices
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == bob.device.as_str())
        .unwrap();
    assert_eq!(d["vaults"][0]["vault"], v.as_str(), "{devices}");

    // History lost since: "full resync needed", no count.
    sqlx::query("UPDATE vault SET epoch = gen_random_uuid() WHERE id = $1::uuid")
        .bind(&v)
        .execute(&e.t.owner)
        .await
        .unwrap();
    let (_, list) = bob.sync(&e, &v).await;
    let mine = row(&list, &bob.device);
    assert_eq!(mine["resync"], true, "{list}");
    assert!(mine.get("behind").is_none());
}

/// §6.P observation 4, T2 and T3: a stranger, and another tenant's member,
/// get the 404 of a vault that does not exist.
#[tokio::test]
async fn a_stranger_gets_404() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let other = who(&e, "olga", &["other-members"]).await;
    let (st, b) = other.sync(&e, &v).await;
    assert_eq!(st, StatusCode::NOT_FOUND, "{b}");
    let (st, _) = other.sync(&e, &Uuid::new_v4().to_string()).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

/// §6.P observation 5: a scoped token is refused on the route before any
/// vault is looked at.
#[tokio::test]
async fn a_scoped_token_is_refused() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let (st, b) = e
        .call(
            "GET",
            &format!("/v1/vaults/{v}/sync"),
            &format!("{}example", kura_core::token::PREFIX),
            None,
            None,
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{b}");
    assert!(problem(&b).ends_with("insufficient_role"), "{b}");
}

/// T5: a restricted viewer's "behind" counts only its folders; question 3:
/// removing a person's binding removes their devices from the list.
#[tokio::test]
async fn behind_counts_only_the_device_scope() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    // RFC 0005's players: a viewer of Sessions/ only.
    let docs = common::app::CONFIG
        .replace(
            "member: [maxime-members] }",
            "member: [maxime-members, maxime-players] }",
        )
        .replace(
            "    - { role: editor, claimValue: maxime-members }",
            "    - { role: editor, claimValue: maxime-members }\n    - { role: viewer, claimValue: maxime-players, paths: [Sessions/] }",
        );
    let dir = std::env::temp_dir().join(format!("kura-http-{}", e.t.name));
    std::fs::write(dir.join("config.yaml"), docs).unwrap();
    let files = kura_server::reconcile::files::read_dir(&dir).unwrap();
    kura_server::reconcile::reconcile(&e.t.app, &files, true)
        .await
        .unwrap();
    e.state.set_snapshot(
        kura_server::reconcile::snapshot::load_if_changed(&e.t.app, None)
            .await
            .unwrap()
            .unwrap(),
    );
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    let pia = who(&e, "pia", &["maxime-players"]).await;
    pia.pull(&e, &v).await;
    file(&e, &gm, &v, "MJ/secret.md", b"hidden").await;
    file(&e, &gm, &v, "Sessions/s1.md", b"seen").await;
    let (_, list) = gm.sync(&e, &v).await;
    assert_eq!(row(&list, &pia.device)["behind"], 1, "{list}");

    let (st, b) = gm
        .call(
            &e,
            "PUT",
            &format!("/v1/vaults/{v}/bindings/user:pia"),
            Some(json!({ "role": "viewer", "paths": ["Sessions/"] })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let (st, _) = gm
        .call(
            &e,
            "DELETE",
            &format!("/v1/vaults/{v}/bindings/user:pia"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (_, list) = gm.sync(&e, &v).await;
    assert!(row(&list, &pia.device).is_null(), "{list}");
}

/// Phase 3 (question 2): a device reports its last run as four counts, read
/// back in the vault's list; anything but four bounded counts is refused, and
/// a stranger's report is a 404 like the rest.
#[tokio::test]
async fn a_device_reports_its_last_run_as_counts() {
    let Some(e) = env(module_path!()).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    let bob = who(&e, "bob", &["maxime-members"]).await;
    let uri = format!("/v1/vaults/{v}/sync/report");
    let counts = json!({ "pending": 3, "conflicts": 1, "unsyncable": 2, "failed": 0 });
    let (st, b) = bob.call(&e, "PUT", &uri, Some(counts)).await;
    assert_eq!(st, StatusCode::NO_CONTENT, "{b}");
    let (_, list) = gm.sync(&e, &v).await;
    let r = &row(&list, &bob.device)["report"];
    assert_eq!(
        (
            &r["pending"],
            &r["conflicts"],
            &r["unsyncable"],
            &r["failed"]
        ),
        (&json!(3), &json!(1), &json!(2), &json!(0)),
        "{list}"
    );
    assert!(r["at"].is_string());

    // Paths are not counts: an unknown field is refused, as is a huge count.
    let (st, b) = bob
        .call(&e, "PUT", &uri, Some(json!({ "pending": 0, "conflicts": 0, "unsyncable": 0, "failed": 0, "paths": ["a.md"] })))
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{b}");
    let (st, _) = bob
        .call(
            &e,
            "PUT",
            &uri,
            Some(json!({ "pending": 2_000_000, "conflicts": 0, "unsyncable": 0, "failed": 0 })),
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    let olga = who(&e, "olga", &["other-members"]).await;
    let (st, _) = olga
        .call(
            &e,
            "PUT",
            &uri,
            Some(json!({ "pending": 0, "conflicts": 0, "unsyncable": 0, "failed": 0 })),
        )
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = e
        .call(
            "PUT",
            &uri,
            &format!("{}example", kura_core::token::PREFIX),
            None,
            Some(json!({})),
        )
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
}

/// RFC 0006 D13: a manager reads an owner by name, taken from the owner's own
/// token; a member is never told it.
#[tokio::test]
async fn an_owner_is_named_by_their_name_claim() {
    let Some(st) = storage(module_path!()).await else {
        return;
    };
    let Some(e) = env_with(module_path!(), Some(st)).await else {
        return;
    };
    let v = vault_id(&e, "knowledge").await;
    let gm = who(&e, "gm", &["maxime-admins"]).await;
    let token = e
        .key
        .token_named("h4sh3d-sub", &["maxime-members"], "Bob Martin");
    let device = e.register(&token, "laptop").await;
    let bob = Who { token, device };
    bob.pull(&e, &v).await;
    let (_, list) = gm.sync(&e, &v).await;
    let r = row(&list, &bob.device);
    assert_eq!(
        (&r["owner"], &r["ownerName"]),
        (&json!("h4sh3d-sub"), &json!("Bob Martin")),
        "{list}"
    );
    let (_, devices) = gm.call(&e, "GET", "/v1/devices", None).await;
    let d = devices
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["id"] == bob.device.as_str())
        .unwrap();
    assert_eq!(d["ownerName"], "Bob Martin", "{devices}");
    let (_, mine) = bob.sync(&e, &v).await;
    assert!(row(&mine, &bob.device).get("ownerName").is_none(), "{mine}");
}
