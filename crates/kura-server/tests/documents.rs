//! `PUT /v1/admin/documents` against PostgreSQL (RFC 0009 §6.2, §14): a push
//! applies like files, and only kura-operator's identity may push.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers fail the test on purpose"
)]

mod common;

use axum::http::StatusCode;
use common::app::{Env, env_custom};
use common::tokens::{OPERATOR, OperatorKey};
use serde_json::{Value, json};

const ROUTE: &str = "/v1/admin/documents";

fn tenant(name: &str) -> Value {
    json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant",
        "metadata": { "name": name, "generation": 1 },
        "spec": {
            "claimMappings": { "claim": "groups", "roles": { "owner": [format!("{name}-owners")], "member": [format!("{name}-members")] } },
            "vaultNamespaces": [name]
        }
    })
}

fn vault(ns: &str, name: &str, generation: i64) -> Value {
    json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault",
        "metadata": { "name": name, "namespace": ns, "generation": generation },
        "spec": { "tenantRef": { "name": ns }, "bindings": [{ "role": "editor", "claimValue": format!("{ns}-members") }] }
    })
}

/// The documents of the harness's `CONFIG`, as kura-operator sends them.
fn base() -> Vec<Value> {
    vec![
        tenant("maxime"),
        tenant("other"),
        vault("maxime", "knowledge", 1),
        vault("other", "secrets", 1),
    ]
}

async fn operator_env(test: &str, key: &OperatorKey) -> Option<Env> {
    let verifier = key.verifier();
    env_custom(test, None, move |s| s.with_documents(verifier)).await
}

async fn push(e: &Env, token: &str, documents: Vec<Value>) -> (StatusCode, Value) {
    e.call(
        "PUT",
        ROUTE,
        token,
        None,
        Some(json!({ "documents": documents })),
    )
    .await
}

async fn stored_generation(e: &Env) -> Vec<u8> {
    sqlx::query_scalar("SELECT generation FROM config_state ORDER BY id DESC LIMIT 1")
        .fetch_one(&e.t.owner)
        .await
        .unwrap()
}

fn object<'a>(body: &'a Value, kind: &str, name: &str) -> &'a Value {
    body["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["kind"] == kind && o["name"] == name)
        .unwrap_or_else(|| panic!("{kind} {name} in {body}"))
}

fn pushes(e: &Env, outcome: &str) -> u64 {
    e.state
        .metrics
        .documents_pushes_total
        .with_label_values(&[outcome])
        .get()
}

/// A push applies like files: the new vault is stored with its pushed
/// generation, the same set again is `unchanged`, an invalid set leaves the
/// stored one, and a document left out follows its `deletionPolicy`.
#[tokio::test]
async fn a_push_applies_like_files() {
    let key = OperatorKey::new();
    let Some(e) = operator_env(module_path!(), &key).await else {
        return;
    };
    let token = key.operator();
    let mut set = base();
    set.push(vault("maxime", "recipes", 7));
    let (s, b) = push(&e, &token, set.clone()).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["applied"], true);
    assert_eq!(b["outcome"], "applied");
    let recipes = object(&b, "KuraVault", "recipes");
    assert_eq!(recipes["observedGeneration"], 7);
    assert_eq!(recipes["source"], "KuraVault/maxime/recipes#1");
    assert_eq!(recipes["conditions"][0]["reason"], "Reconciled");
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM vault WHERE namespace = 'maxime' AND name = 'recipes' AND disabled_at IS NULL",
    )
    .fetch_one(&e.t.owner)
    .await
    .unwrap();
    assert_eq!(rows, 1);

    // The same set in another order: nothing written.
    set.reverse();
    let before = stored_generation(&e).await;
    let (s, b) = push(&e, &token, set.clone()).await;
    assert_eq!(
        (s, b["outcome"].as_str()),
        (StatusCode::OK, Some("unchanged"))
    );
    assert_eq!(stored_generation(&e).await, before);

    // A vault outside its tenant's namespaces: the whole set is refused, the
    // error names it, the stored generation stays.
    let mut bad = set.clone();
    bad.push(vault("other", "intruder", 1));
    bad.last_mut().unwrap()["spec"]["tenantRef"]["name"] = json!("maxime");
    let (s, b) = push(&e, &token, bad).await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(
        (b["applied"].as_bool(), b["outcome"].as_str()),
        (Some(false), Some("rejected"))
    );
    assert_eq!(
        b["errors"][0]["location"]["file"], "KuraVault/other/intruder",
        "{b}"
    );
    assert_eq!(stored_generation(&e).await, before);
    assert_eq!(b["rejection"].as_array().unwrap().len(), 1, "{b}");

    // An unknown field is refused as in a file (`deny_unknown_fields`).
    let mut unknown = set.clone();
    unknown[0]["spec"]["quotaa"] = json!({});
    let (_, b) = push(&e, &token, unknown).await;
    assert_eq!(b["outcome"], "rejected", "{b}");

    // `recipes` left out (policy Orphan): disabled, data kept, reversible.
    let without: Vec<Value> = set
        .iter()
        .filter(|d| d["metadata"]["name"] != "recipes")
        .cloned()
        .collect();
    let (_, b) = push(&e, &token, without).await;
    assert_eq!(b["outcome"], "applied", "{b}");
    assert_eq!(
        object(&b, "KuraVault", "recipes")["conditions"][0]["reason"],
        "Orphaned"
    );
    let (_, b) = push(&e, &token, set).await;
    assert_eq!(
        object(&b, "KuraVault", "recipes")["conditions"][0]["reason"],
        "Reconciled"
    );
    assert_eq!(pushes(&e, "applied"), 3);
    assert_eq!(pushes(&e, "unchanged"), 1);
    assert_eq!(pushes(&e, "rejected"), 2);
}

/// T4: with the webhook bypassed (down, or a resource applied before it
/// existed), the server still refuses a vault outside its tenant's
/// namespaces: the webhook is a convenience, the server the control.
#[tokio::test]
async fn server_rejects_what_the_webhook_would() {
    let key = OperatorKey::new();
    let Some(e) = operator_env(module_path!(), &key).await else {
        return;
    };
    let before = stored_generation(&e).await;
    let mut set = base();
    let mut intruder = vault("team-b", "recipes", 1);
    intruder["spec"]["tenantRef"]["name"] = json!("maxime");
    set.push(intruder);
    let (s, b) = push(&e, &key.operator(), set).await;
    assert_eq!(
        (s, b["outcome"].as_str()),
        (StatusCode::OK, Some("rejected")),
        "{b}"
    );
    assert_eq!(
        b["errors"][0]["location"]["file"], "KuraVault/team-b/recipes",
        "{b}"
    );
    assert_eq!(stored_generation(&e).await, before);
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM vault WHERE namespace = 'team-b'")
        .fetch_one(&e.t.owner)
        .await
        .unwrap();
    assert_eq!(stored, 0);
}

/// T6: a set pushed while a newer server version wrote last is not applied
/// (`kura_config_begin`), so a stale writer never reverts a newer one.
#[tokio::test]
async fn older_writer_refused() {
    let key = OperatorKey::new();
    let Some(e) = operator_env(module_path!(), &key).await else {
        return;
    };
    sqlx::query("UPDATE config_state SET writer_version = '99.0.0'")
        .execute(&e.t.owner)
        .await
        .unwrap();
    let before = stored_generation(&e).await;
    let mut set = base();
    set.push(vault("maxime", "recipes", 1));
    let (s, b) = push(&e, &key.operator(), set).await;
    assert_eq!(
        (s, b["applied"].as_bool()),
        (StatusCode::OK, Some(false)),
        "{b}"
    );
    assert_eq!(stored_generation(&e).await, before);
}

/// T10 (server half): an empty set is refused, never read as removing every
/// tenant.
#[tokio::test]
async fn empty_set_refused() {
    let key = OperatorKey::new();
    let Some(e) = operator_env(module_path!(), &key).await else {
        return;
    };
    let before = stored_generation(&e).await;
    let (s, b) = push(&e, &key.operator(), Vec::new()).await;
    assert_eq!(
        (s, b["outcome"].as_str()),
        (StatusCode::OK, Some("rejected")),
        "{b}"
    );
    assert_eq!(stored_generation(&e).await, before);
}

/// T1: no user and no other workload writes the documents. A platform
/// admin's Authentik token, another ServiceAccount, the wrong audience, an
/// expired token: refused, nothing stored.
#[tokio::test]
async fn documents_refuses_other_identities() {
    let key = OperatorKey::new();
    let Some(e) = operator_env(module_path!(), &key).await else {
        return;
    };
    let before = stored_generation(&e).await;
    let admin = e.key.token("root", "s1", &["kura-platform-admins"]);
    let cases = [
        (admin, StatusCode::UNAUTHORIZED, "invalid_token"),
        (
            key.token("system:serviceaccount:default:default", "kura", 3600),
            StatusCode::FORBIDDEN,
            "insufficient_role",
        ),
        (
            key.token(&format!("{OPERATOR}-x"), "kura", 3600),
            StatusCode::FORBIDDEN,
            "insufficient_role",
        ),
        (
            key.token(OPERATOR, "kubernetes", 3600),
            StatusCode::UNAUTHORIZED,
            "invalid_token",
        ),
        (
            key.token(OPERATOR, "kura", -120),
            StatusCode::UNAUTHORIZED,
            "invalid_token",
        ),
        (String::new(), StatusCode::UNAUTHORIZED, "invalid_token"),
    ];
    for (token, status, problem) in cases {
        let (s, b) = push(&e, &token, base()).await;
        assert_eq!(s, status, "{b}");
        assert_eq!(b["type"], format!("/v1/problems/{problem}"));
    }
    assert_eq!(stored_generation(&e).await, before);
    assert_eq!(pushes(&e, "refused"), 6);
    // And the operator's token opens no user route.
    let (s, _) = e
        .call("GET", "/v1/vaults", &key.operator(), Some("x"), None)
        .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

/// `files` mode: refused with 409 before the token is read, whoever asks.
#[tokio::test]
async fn files_mode_refuses_every_push() {
    let key = OperatorKey::new();
    let Some(e) = env_custom(module_path!(), None, |s| s).await else {
        return;
    };
    for token in [key.operator(), String::new()] {
        let (s, b) = push(&e, &token, base()).await;
        assert_eq!(s, StatusCode::CONFLICT, "{b}");
        assert_eq!(b["type"], "/v1/problems/documents_source_files");
    }
}

/// T5: the body and the count are bounded, and so is the rate.
#[tokio::test]
async fn documents_body_and_count_limits() {
    let key = OperatorKey::new();
    let Some(e) = operator_env(module_path!(), &key).await else {
        return;
    };
    let token = key.operator();
    let mut huge = base();
    huge[0]["metadata"]["annotations"] = json!({ "x": "a".repeat(4 << 20) });
    let (s, _) = push(&e, &token, huge).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
    let many: Vec<Value> = (0..2_001).map(|i| tenant(&format!("t{i}"))).collect();
    let (s, b) = push(&e, &token, many).await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE, "{b}");
    let (s, b) = push(
        &e,
        &token,
        vec![json!({ "kind": "Secret", "metadata": { "name": "x" } })],
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    // 3 calls so far; the limit is 30 a minute.
    for _ in 3..30 {
        let (s, _) = push(&e, &token, base()).await;
        assert_eq!(s, StatusCode::OK);
    }
    let (s, _) = push(&e, &token, base()).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
}
