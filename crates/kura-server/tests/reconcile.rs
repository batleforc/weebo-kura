//! Reconciliation against PostgreSQL (RFC 0001 §4.3, §6.10, T12).

#![expect(clippy::unwrap_used, reason = "test helpers fail the test on purpose")]

mod common;

use std::sync::atomic::{AtomicU32, Ordering};

use common::test_db;
use kura_server::reconcile::files::{ConfigFiles, read_dir};
use kura_server::reconcile::snapshot::load_if_changed;
use kura_server::reconcile::{Outcome, reconcile};
use sqlx::Row;

fn tenant(name: &str, extra: &str) -> String {
    format!(
        "apiVersion: kura.weebo.io/v1alpha1\nkind: KuraTenant\nmetadata: {{ name: {name}{extra} }}\nspec:\n  claimMappings: {{ claim: groups, roles: {{ member: [g-{name}] }} }}\n  vaultNamespaces: [{name}]\n"
    )
}

fn tenant_delete(name: &str) -> String {
    tenant(name, "") + "  deletionPolicy: Delete\n"
}

fn vault(ns: &str, name: &str) -> String {
    format!(
        "apiVersion: kura.weebo.io/v1alpha1\nkind: KuraVault\nmetadata: {{ name: {name}, namespace: {ns} }}\nspec:\n  tenantRef: {{ name: {ns} }}\n  bindings:\n    - {{ role: editor, claimValue: g-{ns} }}\n    - {{ role: viewer, subject: sub-1 }}\n"
    )
}

static DIRS: AtomicU32 = AtomicU32::new(0);

/// Writes the documents to a fresh directory and reads it like the server.
fn files(docs: &[String]) -> ConfigFiles {
    let d = std::env::temp_dir().join(format!(
        "kura-reconcile-{}-{}",
        std::process::id(),
        DIRS.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("config.yaml"), docs.join("---\n")).unwrap();
    read_dir(&d).unwrap()
}

async fn apply(pool: &sqlx::PgPool, docs: &[String]) -> Outcome {
    reconcile(pool, &files(docs), true).await.unwrap()
}

async fn count_state(t: &common::TestDb) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM config_state")
        .fetch_one(&t.owner)
        .await
        .unwrap()
}

async fn tenant_row(t: &common::TestDb, slug: &str) -> (bool, Option<time::OffsetDateTime>) {
    let r = sqlx::query(
        "SELECT disabled_at IS NOT NULL AS d, delete_after FROM tenant WHERE slug = $1",
    )
    .bind(slug)
    .fetch_one(&t.owner)
    .await
    .unwrap();
    (r.get("d"), r.get("delete_after"))
}

#[tokio::test]
async fn invalid_document_keeps_last_config() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    assert!(matches!(
        apply(&t.app, &[tenant("a", ""), vault("a", "notes")]).await,
        Outcome::Applied { .. }
    ));
    let first = load_if_changed(&t.app, None).await.unwrap().unwrap();

    let bad = tenant("a", "").replace("member:", "membr:");
    let Outcome::Rejected(errors) = apply(&t.app, &[bad, vault("a", "notes")]).await else {
        panic!("accepted")
    };
    assert!(
        errors[0]
            .to_string()
            .starts_with("config.yaml#1 spec.claimMappings.roles"),
        "{errors:?}"
    );
    assert_eq!(count_state(&t).await, 1, "nothing written");
    assert!(
        load_if_changed(&t.app, Some(first.id))
            .await
            .unwrap()
            .is_none(),
        "the stored state did not move"
    );
    assert_eq!(first.tenant_ids.len(), 1);
    assert_eq!(first.config.vaults.len(), 1);
    let bindings: i64 = sqlx::query_scalar("SELECT count(*) FROM role_binding")
        .fetch_one(&t.owner)
        .await
        .unwrap();
    assert_eq!(bindings, 2);
}

#[tokio::test]
async fn an_older_replica_never_overwrites_newer_state() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let gen1 = vec![1u8; 32];
    let gen2 = vec![2u8; 32];
    sqlx::query("SELECT kura_config_state_insert($1, '0.10.0', '{}', '{}')")
        .bind(&gen1)
        .execute(&t.app)
        .await
        .unwrap();
    let begin = |generation: Vec<u8>, version: &'static str, local: bool| {
        let pool = t.app.clone();
        async move {
            let mut tx = pool.begin().await.unwrap();
            let ok: bool = sqlx::query_scalar("SELECT kura_config_begin($1, $2, $3)")
                .bind(generation)
                .bind(version)
                .bind(local)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            tx.rollback().await.unwrap();
            ok
        }
    };
    assert!(
        !begin(gen2.clone(), "0.9.0", true).await,
        "0.9.0 is older than 0.10.0 (not as text)"
    );
    assert!(
        !begin(gen1.clone(), "0.11.0", true).await,
        "same generation: nothing to write"
    );
    assert!(
        !begin(gen2.clone(), "0.10.0", false).await,
        "same version, no local change: a lagging mount"
    );
    assert!(begin(gen2.clone(), "0.10.0", true).await);
    assert!(
        begin(gen2, "0.11.0", false).await,
        "a newer writer always may"
    );
}

#[tokio::test]
async fn orphan_disables_and_delete_schedules() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let force = ", annotations: { kura.weebo.io/force-orphan: \"true\" }";
    let forced = tenant("c", force) + "  deletionPolicy: Delete\n";
    apply(
        &t.app,
        &[
            tenant("a", ""),
            tenant("b", ""),
            forced.clone(),
            tenant_delete("d"),
        ],
    )
    .await;

    let Outcome::Applied { removed } =
        apply(&t.app, &[tenant("a", ""), forced, tenant_delete("d")]).await
    else {
        panic!()
    };
    assert_eq!(removed.len(), 1);
    assert_eq!(
        tenant_row(&t, "b").await,
        (true, None),
        "Orphan: disabled, kept, no date"
    );

    apply(&t.app, &[tenant("a", ""), tenant_delete("d")]).await;
    assert_eq!(
        tenant_row(&t, "c").await,
        (true, None),
        "force-orphan cancels the Delete cleanup"
    );

    apply(&t.app, &[tenant("a", "")]).await;
    let (disabled, after) = tenant_row(&t, "d").await;
    assert!(disabled);
    let days = (after.unwrap() - time::OffsetDateTime::now_utc()).whole_days();
    assert!((29..=30).contains(&days), "{days}");

    let snapshot = load_if_changed(&t.app, None).await.unwrap().unwrap();
    assert_eq!(
        snapshot.tenant_ids.keys().collect::<Vec<_>>(),
        ["a"],
        "disabled tenants grant nothing"
    );
    let reasons: Vec<&str> = snapshot
        .status
        .objects
        .iter()
        .map(|o| o.conditions[0].reason.as_str())
        .collect();
    assert_eq!(reasons, ["Reconciled", "DeletionScheduled"]);
}

#[tokio::test]
async fn restored_document_cancels_deletion() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    apply(&t.app, &[tenant("a", ""), tenant_delete("d")]).await;
    apply(&t.app, &[tenant("a", "")]).await;
    assert!(tenant_row(&t, "d").await.1.is_some());
    apply(&t.app, &[tenant("a", ""), tenant_delete("d")]).await;
    assert_eq!(tenant_row(&t, "d").await, (false, None));
}

#[tokio::test]
async fn stored_state_feeds_validation() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    apply(
        &t.app,
        &[
            tenant("a", ""),
            tenant("b", ""),
            tenant("c", ""),
            vault("a", "notes"),
        ],
    )
    .await;
    // Mass removal is judged against the stored state, not memory.
    assert!(matches!(
        apply(&t.app, &[tenant("a", ""), vault("a", "notes")]).await,
        Outcome::Rejected(_)
    ));
    // Changing a vault's chunking after creation.
    let changed = vault("a", "notes") + "  chunking: { min: 128Ki, avg: 1Mi, max: 2Mi }\n";
    let Outcome::Rejected(e) = apply(
        &t.app,
        &[tenant("a", ""), tenant("b", ""), tenant("c", ""), changed],
    )
    .await
    else {
        panic!()
    };
    assert!(e[0].to_string().contains("created with"), "{e:?}");
    // A vault created through the API cannot be declared by a file.
    let tenant_a: uuid::Uuid = sqlx::query_scalar("SELECT id FROM tenant WHERE slug = 'a'")
        .fetch_one(&t.owner)
        .await
        .unwrap();
    t.vault(tenant_a, "a", "api-made").await;
    sqlx::query("UPDATE vault SET managed_by = 'api' WHERE name = 'api-made'")
        .execute(&t.owner)
        .await
        .unwrap();
    let docs = [
        tenant("a", ""),
        tenant("b", ""),
        tenant("c", ""),
        vault("a", "notes"),
        vault("a", "api-made"),
    ];
    let Outcome::Rejected(e) = apply(&t.app, &docs).await else {
        panic!()
    };
    assert!(
        e[0].to_string().contains("managed through the API"),
        "{e:?}"
    );
}

#[tokio::test]
async fn reapplying_keeps_api_bindings_and_skips_unchanged_files() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let docs = [tenant("a", ""), vault("a", "notes")];
    apply(&t.app, &docs).await;
    sqlx::query(
        "INSERT INTO role_binding (tenant_id, vault_id, subject_kind, subject, role, managed_by)
         SELECT tenant_id, id, 'user', 'sub-api', 'viewer', 'api' FROM vault",
    )
    .execute(&t.owner)
    .await
    .unwrap();
    let ids = |pool: sqlx::PgPool| async move {
        sqlx::query_scalar::<_, String>(
            "SELECT subject || '=' || id::text FROM role_binding WHERE managed_by = 'config' ORDER BY subject",
        )
        .fetch_all(&pool)
        .await
        .unwrap()
    };
    let before = ids(t.owner.clone()).await;
    let edited = [
        tenant("a", ""),
        vault("a", "notes").replace("role: editor", "role: manager"),
    ];
    assert!(matches!(
        apply(&t.app, &edited).await,
        Outcome::Applied { .. }
    ));
    let roles: Vec<String> =
        sqlx::query_scalar("SELECT role || ':' || managed_by FROM role_binding ORDER BY subject")
            .fetch_all(&t.owner)
            .await
            .unwrap();
    assert_eq!(roles, ["manager:config", "viewer:config", "viewer:api"]);
    // A re-applied binding keeps its row, so a scoped token granted through it
    // survives any configuration change (review 2026-11-15 #2).
    assert_eq!(
        ids(t.owner.clone()).await,
        before,
        "same ids after a role change"
    );
    let dropped = [
        tenant("a", ""),
        vault("a", "notes").replace("    - { role: viewer, subject: sub-1 }\n", ""),
    ];
    apply(&t.app, &dropped).await;
    let left: Vec<String> = sqlx::query_scalar(
        "SELECT subject || ':' || managed_by FROM role_binding ORDER BY subject",
    )
    .fetch_all(&t.owner)
    .await
    .unwrap();
    assert_eq!(
        left,
        ["g-a:config", "sub-api:api"],
        "an undeclared binding is deleted"
    );
    assert_eq!(
        ids(t.owner.clone()).await,
        before[..1],
        "the other keeps its id"
    );
    // Same files again: same generation, nothing written.
    assert!(matches!(apply(&t.app, &dropped).await, Outcome::NotWritten));
    assert_eq!(
        count_state(&t).await,
        3,
        "three distinct generations applied"
    );
}

/// The background tasks: a fresh install is unready until the reconciler has
/// written a configuration and the cache has read it back (decision 31).
#[tokio::test]
async fn the_server_becomes_ready_once_a_configuration_is_stored() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("kura-run-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tenants.yaml"), tenant("a", "")).unwrap();
    let state = std::sync::Arc::new(kura_server::app::AppState::new(
        kura_server::metrics::Metrics::new().unwrap(),
        t.app.clone(),
        common::no_key_verifier(),
    ));
    assert!(!state.is_ready());
    let token = tokio_util::sync::CancellationToken::new();
    let tasks: Vec<_> = kura_server::background(&state, Some(dir), &token)
        .into_iter()
        .map(tokio::spawn)
        .collect();
    for _ in 0..100 {
        if state.is_ready() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(state.is_ready());
    assert_eq!(state.snapshot().unwrap().tenant_ids.len(), 1);
    token.cancel();
    for task in tasks {
        task.await.unwrap();
    }
}

/// A tick that fails (database error, lock timeout) keeps the change pending:
/// the next tick still applies it.
#[tokio::test]
async fn a_failed_tick_is_retried_as_a_local_change() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("kura-retry-{}", t.name));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("tenants.yaml"), tenant("a", "")).unwrap();
    apply(&t.app, &[tenant("a", "")]).await;
    // The next change cannot be written for a while.
    std::fs::write(
        dir.join("tenants.yaml"),
        tenant("a", "") + "---\n" + &tenant("b", ""),
    )
    .unwrap();
    sqlx::query("REVOKE EXECUTE ON FUNCTION kura_config_begin(bytea, text, boolean) FROM kura_app")
        .execute(&t.owner)
        .await
        .unwrap();
    let state = std::sync::Arc::new(kura_server::app::AppState::new(
        kura_server::metrics::Metrics::new().unwrap(),
        t.app.clone(),
        common::no_key_verifier(),
    ));
    let token = tokio_util::sync::CancellationToken::new();
    let poll = std::time::Duration::from_millis(100);
    let task = tokio::spawn(kura_server::reconcile::run_every(
        t.app.clone(),
        dir,
        state,
        token.clone(),
        poll,
    ));
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert_eq!(count_state(&t).await, 1, "nothing written while failing");
    sqlx::query("GRANT EXECUTE ON FUNCTION kura_config_begin(bytea, text, boolean) TO kura_app")
        .execute(&t.owner)
        .await
        .unwrap();
    for _ in 0..50 {
        if count_state(&t).await == 2 {
            break;
        }
        tokio::time::sleep(poll).await;
    }
    assert_eq!(count_state(&t).await, 2, "the pending change was applied");
    token.cancel();
    task.await.unwrap();
}

/// A configuration written elsewhere reaches the snapshot through the
/// `NOTIFY`, well before the 5 s poll.
#[tokio::test]
async fn a_config_change_is_seen_before_the_poll() {
    let Some(t) = test_db(module_path!()).await else {
        return;
    };
    let state = std::sync::Arc::new(kura_server::app::AppState::new(
        kura_server::metrics::Metrics::new().unwrap(),
        t.app.clone(),
        common::no_key_verifier(),
    ));
    let token = tokio_util::sync::CancellationToken::new();
    let tasks = [
        tokio::spawn(kura_server::reconcile::snapshot::run(
            t.app.clone(),
            state.clone(),
            token.clone(),
        )),
        tokio::spawn(kura_server::notify::listen(
            t.app.clone(),
            state.clone(),
            token.clone(),
        )),
    ];
    // The loader has polled once (nothing stored) and sleeps; LISTEN is up.
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    assert!(state.snapshot().is_none());
    let started = std::time::Instant::now();
    apply(&t.app, &[tenant("a", ""), vault("a", "notes")]).await;
    while state.snapshot().is_none() && started.elapsed() < std::time::Duration::from_secs(4) {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(state.snapshot().is_some(), "not loaded within 4 s");
    token.cancel();
    for task in tasks {
        task.await.unwrap();
    }
}
