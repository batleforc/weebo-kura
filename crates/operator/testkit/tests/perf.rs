//! What kura-operator costs to run (RFC 0009 §10.3, the kit's
//! `Budget::small_cluster`): its release binary against envtest, a stand-in
//! kura-server answering every push, a burst of 100 vaults then an edit of
//! each. A measurement: `task operator:perf` builds the binary and runs it.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

use std::path::Path;
use std::time::Duration;

use axum::routing::put;
use k8s_openapi::api::core::v1::ServiceAccount;
use kube::api::{Api, PostParams};
use kura_api::{KuraInstance, KuraTenantResource, KuraVaultResource};
use serde_json::{Value, json};
use weebo_kit_perf::workload::names;
use weebo_kit_perf::{BASELINE, Budget, FINAL, OperatorMetrics, PerfRun};

const VAULTS: usize = 100;

/// Every document of the push, `Ready` with its generation: what kura-server
/// answers when it applies a set.
async fn apply(axum::Json(body): axum::Json<Value>) -> axum::Json<Value> {
    let objects: Vec<Value> = body["documents"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .map(|d| {
            json!({
                "kind": d["kind"], "namespace": d["metadata"]["namespace"], "name": d["metadata"]["name"],
                "conditions": [{ "type": "Ready", "status": "True", "reason": "Reconciled", "message": "",
                                 "observedGeneration": d["metadata"]["generation"] }]
            })
        })
        .collect();
    axum::Json(json!({ "applied": true, "outcome": "applied", "objects": objects }))
}

fn ready(v: &KuraVaultResource) -> bool {
    v.status.as_ref().is_some_and(|s| {
        s.observed_generation == v.metadata.generation
            && weebo_kit_api::status::ready(&s.conditions).is_some_and(|c| c.status == "True")
    })
}

fn vault(name: &str, role: &str) -> KuraVaultResource {
    serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault",
        "metadata": { "name": name, "namespace": "default" },
        "spec": { "tenantRef": { "name": "perf" }, "bindings": [{ "role": role, "claimValue": "perf" }] }
    }))
    .expect("a vault")
}

#[tokio::test]
#[ignore = "a measurement on the release binary: task operator:perf"]
async fn kura_operator_fits_a_small_cluster() {
    let binary =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/release/kura-operator");
    assert!(
        binary.exists(),
        "build it first: cargo build --release -p kura-operator"
    );
    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    tokio::spawn(async move {
        drop(
            axum::serve(
                listener,
                axum::Router::new().route("/v1/admin/documents", put(apply)),
            )
            .await,
        )
    });
    let account: ServiceAccount =
        serde_json::from_value(json!({ "metadata": { "name": "kura-operator" } }))
            .expect("an account");
    Api::<ServiceAccount>::namespaced(client.clone(), "default")
        .create(&PostParams::default(), &account)
        .await
        .expect("the operator's ServiceAccount");
    let instance: KuraInstance = serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": "kura" },
        "spec": { "url": url, "auth": { "serviceAccountToken": { "audience": "kura" } } }
    }))
    .expect("an instance");
    Api::<KuraInstance>::all(client.clone())
        .create(&PostParams::default(), &instance)
        .await
        .expect("the instance");
    let tenant: KuraTenantResource = serde_json::from_value(json!({
        "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "perf" },
        "spec": { "vaultNamespaces": ["default"] }
    }))
    .expect("a tenant");
    Api::<KuraTenantResource>::all(client.clone())
        .create(&PostParams::default(), &tenant)
        .await
        .expect("the tenant");

    let mut command = tokio::process::Command::new(&binary);
    command.env("KURA_SERVICE_ACCOUNT", "kura-operator");
    let mut run = PerfRun::start(
        "kura-operator",
        &cluster,
        command,
        OperatorMetrics::new("kura_operator"),
    )
    .await;
    run.idle("startup", Duration::from_secs(20)).await;
    let vaults = run.workload(Api::<KuraVaultResource>::namespaced(
        client.clone(),
        "default",
    ));
    // A warm-up doing what the measured loads do (create, edit, delete 100
    // vaults): the baseline is measured with the allocator and the caches
    // already sized, as the kit's method says; a leak would still grow.
    let warm = names("warm", VAULTS);
    let warm_objects: Vec<KuraVaultResource> = warm.iter().map(|n| vault(n, "viewer")).collect();
    run.load("warm-up", VAULTS, async {
        vaults.create(&warm_objects).await;
        let took = vaults
            .wait_all(&warm, Duration::from_secs(120), ready)
            .await;
        vaults
            .patch_all(
                &warm,
                |_| json!({ "spec": { "bindings": [{ "role": "editor", "claimValue": "perf" }] } }),
            )
            .await;
        vaults
            .wait_all(&warm, Duration::from_secs(120), ready)
            .await;
        vaults.delete_all(&warm).await;
        vaults.wait_gone(&warm, Duration::from_secs(120)).await;
        took
    })
    .await;
    run.idle(BASELINE, Duration::from_secs(60)).await;

    let list = names("vault", VAULTS);
    let objects: Vec<KuraVaultResource> = list.iter().map(|n| vault(n, "viewer")).collect();
    run.load("burst", VAULTS, async {
        vaults.create(&objects).await;
        vaults
            .wait_all(&list, Duration::from_secs(120), ready)
            .await
    })
    .await;
    run.load("edit", VAULTS, async {
        vaults
            .patch_all(
                &list,
                |_| json!({ "spec": { "bindings": [{ "role": "editor", "claimValue": "perf" }] } }),
            )
            .await;
        vaults
            .wait_all(&list, Duration::from_secs(120), ready)
            .await
    })
    .await;
    // Back to the baseline's set, so the final idle compares like with like.
    run.load("delete", VAULTS, async {
        vaults.delete_all(&list).await;
        vaults.wait_gone(&list, Duration::from_secs(120)).await
    })
    .await;
    run.idle(FINAL, Duration::from_secs(60)).await;

    // Written to target/perf/kura-operator.{md,json}, printed, and checked.
    run.finish().await.assert_within(&Budget::small_cluster());
}
