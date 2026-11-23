//! RFC 0010: configuration shared across a member's profiles, plugins
//! installed on a vault, and the server's version on every answer. One test
//! per row of §7.1 that the server holds. Opt-in: as tests/tokens.rs.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use axum::http::StatusCode;
use base64::Engine as _;
use common::app::{CONFIG, Env, env_with, storage, vault_id};
use serde_json::{Value, json};
use uuid::Uuid;

struct S {
    e: Env,
    v: String,
}

struct D {
    tok: String,
    id: String,
}

async fn setup(test: &str) -> Option<S> {
    let st = storage(test).await?;
    let e = env_with(test, Some(st)).await?;
    let docs = CONFIG.replace(
        "  tenantRef: { name: maxime }\n  bindings:",
        "  tenantRef: { name: maxime }\n  configSync: { categories: [plugins, snippets], sharedByDefault: [snippets], vaultPlugins: true }\n  bindings:",
    );
    e.apply_config(&docs).await;
    let v = vault_id(&e, "knowledge").await;
    Some(S { e, v })
}

impl S {
    /// A device of `sub`; `owner` puts it in the tenant's owners (a manager).
    async fn device(&self, sub: &str, profile: &str, owner: bool) -> D {
        let group = if owner {
            "maxime-owners"
        } else {
            "maxime-members"
        };
        let tok = self.e.key.token(
            sub,
            &format!("s-{sub}-{profile}-{}", Uuid::new_v4()),
            &[group],
        );
        let (st, b) = self
            .e
            .call(
                "POST",
                "/v1/devices",
                &tok,
                None,
                Some(json!({ "name": format!("{sub}-{profile}"), "profile": profile })),
            )
            .await;
        assert_eq!(st, StatusCode::CREATED, "{b}");
        D {
            id: b["id"].as_str().unwrap().to_owned(),
            tok,
        }
    }

    async fn call(
        &self,
        d: &D,
        method: &str,
        uri: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.e
            .call(
                method,
                &format!("/v1/vaults/{}{uri}", self.v),
                &d.tok,
                Some(&d.id),
                body,
            )
            .await
    }

    async fn put(&self, d: &D, space: &str, path: &str, text: &str) -> (StatusCode, Value) {
        let (blob, size) = self.e.upload(&d.tok, &d.id, &self.v, text.as_bytes()).await;
        let op = json!({ "op": "put", "entry": Uuid::now_v7().to_string(), "path": path, "blob": blob, "size": size });
        let body = json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op], "space": space });
        self.call(d, "POST", "/commits", Some(body)).await
    }

    async fn paths(&self, d: &D, space: &str) -> Vec<String> {
        let (st, b) = self
            .call(d, "GET", &format!("/changes?since=0&space={space}"), None)
            .await;
        assert_eq!(st, StatusCode::OK, "{space}: {b}");
        let mut p: Vec<String> = b["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["path"].as_str().unwrap().to_owned())
            .collect();
        p.sort();
        p
    }

    async fn install(
        &self,
        d: &D,
        id: &str,
        target: &str,
        manifest_id: &str,
    ) -> (StatusCode, Value) {
        let b64 = base64::engine::general_purpose::STANDARD;
        let manifest = format!(r#"{{"id":"{manifest_id}","name":"Kanban","version":"2.0.51"}}"#);
        let body = json!({
            "manifest": b64.encode(manifest),
            "main": b64.encode("module.exports = class {};"),
            "styles": b64.encode(".k{}"),
        });
        self.call(
            d,
            "PUT",
            &format!("/config/plugins/{id}?target={target}"),
            Some(body),
        )
        .await
    }
}

fn kind(b: &Value) -> &str {
    b["type"].as_str().unwrap_or_default()
}

/// T1, T6: a shared item lives in the member's `config:shared`, read by both
/// their profiles, refused in the profile copy, hidden from another member;
/// choices are the member's own.
#[tokio::test]
async fn shared_config_hidden_from_other_member() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (desk, phone, bob) = (
        s.device("alice", "desktop", false).await,
        s.device("alice", "mobile", false).await,
        s.device("bob", "desktop", false).await,
    );
    // snippets are shared by default: their home is config-shared.
    let (st, b) = s.put(&desk, "config", "snippets/reading.css", ".r{}").await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    assert!(
        kind(&b).ends_with("invalid_path")
            && b["detail"].as_str().unwrap().contains("config-shared"),
        "{b}"
    );
    let (st, b) = s
        .put(&desk, "config-shared", "snippets/reading.css", ".r{}")
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(
        s.paths(&phone, "config-shared").await,
        ["snippets/reading.css"]
    );
    // A plugin is per profile until shared.
    let (st, b) = s
        .put(&desk, "config-shared", "plugins/dataview/data.json", "{}")
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    let (st, b) = s
        .put(&desk, "config", "plugins/dataview/data.json", "{}")
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert!(s.paths(&phone, "config").await.is_empty());
    let (st, b) = s
        .call(
            &phone,
            "PUT",
            "/config/shares/plugin:dataview",
            Some(json!({ "shared": true })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["shared"], json!(["plugin:dataview", "snippets"]));
    // Shared now: the profile copy is kept but not served.
    assert!(s.paths(&desk, "config").await.is_empty());
    let (st, b) = s
        .put(
            &phone,
            "config-shared",
            "plugins/dataview/data.json",
            "{\"a\":1}",
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(
        s.paths(&desk, "config-shared").await,
        ["plugins/dataview/data.json", "snippets/reading.css"]
    );
    // bob sees none of it, and none of alice's choices.
    assert!(s.paths(&bob, "config-shared").await.is_empty());
    let (st, b) = s.call(&bob, "GET", "/config/shares", None).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["choices"], json!({}));
    assert_eq!(b["shared"], json!(["snippets"]));
    // A choice turned off goes back to the default with DELETE.
    let (st, b) = s
        .call(
            &desk,
            "PUT",
            "/config/shares/snippets",
            Some(json!({ "shared": false })),
        )
        .await;
    assert_eq!(
        (st, b["shared"].clone()),
        (StatusCode::OK, json!(["plugin:dataview"]))
    );
    let (st, b) = s
        .call(&desk, "DELETE", "/config/shares/snippets", None)
        .await;
    assert_eq!(
        (st, b["shared"].clone()),
        (StatusCode::OK, json!(["plugin:dataview", "snippets"]))
    );
    // GET /v1/vaults names the effective set and the switch.
    let (st, list) =
        s.e.call("GET", "/v1/vaults", &desk.tok, Some(&desk.id), None)
            .await;
    assert_eq!(st, StatusCode::OK);
    let mine = list
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == json!(s.v))
        .unwrap();
    assert_eq!(mine["configShared"], json!(["plugin:dataview", "snippets"]));
    assert_eq!(mine["vaultPlugins"], json!(true));
    // Items outside the grammar or the vault's categories.
    for bad in ["plugin:weebo-kura", "plugin:Bad", "hotkeys", "plugins"] {
        let (st, b) = s
            .call(
                &desk,
                "PUT",
                &format!("/config/shares/{bad}"),
                Some(json!({ "shared": true })),
            )
            .await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{bad}: {b}");
    }
}

/// T3, T4, T5, T13, T14: only a manager's Obsidian device writes the vault's
/// plugins, their three code files only, with the manifest naming the id.
#[tokio::test]
async fn vault_plugins_are_code_only_from_a_manager() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let (mgr, mgr_web, bob, bob_web) = (
        s.device("alice", "desktop", true).await,
        s.device("alice", "web", true).await,
        s.device("bob", "desktop", false).await,
        s.device("bob", "web", false).await,
    );
    // member_cannot_write_vault_space
    let (st, b) = s.put(&bob, "config-vault", "plugins/ex/main.js", "x").await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{b}");
    assert!(b["detail"].as_str().unwrap().contains("manager"), "{b}");
    // vault_space_refuses_data_json
    let (st, b) = s
        .put(&mgr, "config-vault", "plugins/ex/data.json", "{}")
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    let (st, b) = s.put(&mgr, "config-vault", "plugins/ex/main.js", "x").await;
    assert_eq!(st, StatusCode::OK, "{b}");
    // console_cannot_install_on_vault, install_refuses_mismatched_id
    let (st, b) = s.install(&mgr_web, "kanban", "vault", "kanban").await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{b}");
    let (st, b) = s.install(&bob, "kanban", "vault", "kanban").await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{b}");
    for (id, manifest) in [
        ("kanban", "other"),
        ("kanban", "weebo-kura"),
        ("weebo-kura", "weebo-kura"),
    ] {
        let (st, b) = s.install(&mgr, id, "vault", manifest).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{id}/{manifest}: {b}");
    }
    let (st, b) = s.install(&mgr, "kanban", "vault", "kanban").await;
    assert_eq!(st, StatusCode::OK, "{b}");
    assert_eq!(b["files"].as_array().unwrap().len(), 3);
    assert_eq!(b["version"], "2.0.51");
    // Every member's device reads them; the web device lists only.
    assert_eq!(
        s.paths(&bob, "config-vault").await,
        [
            "plugins/ex/main.js",
            "plugins/kanban/main.js",
            "plugins/kanban/manifest.json",
            "plugins/kanban/styles.css"
        ]
    );
    let (st, _) = s
        .call(&bob_web, "GET", "/changes?since=0&space=config-vault", None)
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, b) = s
        .call(&bob_web, "GET", "/entries?space=config-vault", None)
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    // A vault plugin's code has the vault as its home; its settings stay personal.
    let (st, b) = s
        .put(&bob, "config", "plugins/kanban/main.js", "mine")
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{b}");
    let (st, b) = s
        .put(&bob, "config", "plugins/kanban/data.json", "{}")
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    // The console installs for its member: shared, and the choice follows.
    let (st, b) = s.install(&bob_web, "calendar", "shared", "calendar").await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let (_, shares) = s.call(&bob, "GET", "/config/shares", None).await;
    assert!(
        shares["shared"]
            .as_array()
            .unwrap()
            .contains(&json!("plugin:calendar")),
        "{shares}"
    );
    assert_eq!(shares["vaultPlugins"], json!(["ex", "kanban"]));
    assert!(
        s.paths(&bob, "config-shared")
            .await
            .contains(&"plugins/calendar/main.js".to_owned())
    );
    // A device installs in its own profile, never another's.
    let (st, b) = s.install(&bob, "tasks", "mobile", "tasks").await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{b}");
}

/// T8, T7: the web device lists the configuration spaces, never follows or
/// writes them; a vault without the switch has no vault space.
#[tokio::test]
async fn web_device_lists_config_spaces_only() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let web = s.device("alice", "web", false).await;
    let (st, _) = s
        .call(&web, "GET", "/changes?since=0&space=config-shared", None)
        .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, b) = s
        .call(&web, "GET", "/entries?space=config-shared", None)
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    // The blob is a desktop's: the console uploads nothing either.
    let desk = s.device("alice", "desktop", false).await;
    let (blob, size) = s.e.upload(&desk.tok, &desk.id, &s.v, b".a{}").await;
    let op = json!({ "op": "put", "entry": Uuid::now_v7().to_string(), "path": "snippets/a.css", "blob": blob, "size": size });
    let body = json!({ "idempotencyKey": Uuid::new_v4().to_string(), "operations": [op], "space": "config-shared" });
    let (st, b) = s.call(&web, "POST", "/commits", Some(body)).await;
    assert_eq!(st, StatusCode::FORBIDDEN, "{b}");
    let off = CONFIG.replace(
        "  tenantRef: { name: maxime }\n  bindings:",
        "  tenantRef: { name: maxime }\n  configSync: { categories: [plugins] }\n  bindings:",
    );
    s.e.apply_config(&off).await;
    let desk = s.device("alice", "desktop", true).await;
    let (st, b) = s
        .call(&desk, "GET", "/changes?since=0&space=config-vault", None)
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{b}");
    assert!(
        b["detail"]
            .as_str()
            .unwrap()
            .contains("does not take plugins"),
        "{b}"
    );
}

/// RFC 0010 §4.2: every answer names the server's version.
#[tokio::test]
async fn every_answer_names_the_server_version() {
    let Some(s) = setup(module_path!()).await else {
        return;
    };
    let d = s.device("alice", "desktop", false).await;
    let req = axum::http::Request::builder()
        .uri("/v1/vaults")
        .header("authorization", format!("Bearer {}", d.tok))
        .header("x-kura-device", &d.id)
        .body(axum::body::Body::empty())
        .unwrap();
    let r = tower::ServiceExt::oneshot(kura_server::app::app(s.e.state.clone()), req)
        .await
        .unwrap();
    assert_eq!(
        r.headers()["x-kura-server"],
        concat!("kura-server/", env!("CARGO_PKG_VERSION"))
    );
}
