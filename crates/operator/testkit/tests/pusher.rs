//! The push loop and the webhook against a real `kube-apiserver` + `etcd`
//! (envtest), Kura swapped for the in-memory one (RFC 0009 §14): statuses
//! and Events from a push, a refusal named on every resource, no push
//! without a tenant (T10), and the webhook's refusals (T2).

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers fail the test on purpose"
)]

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use k8s_openapi::api::core::v1::{Event, Namespace};
use kube::api::{Api, ListParams, Patch, PatchParams, PostParams};
use kube::core::DynamicObject;
use kube::runtime::events::{Recorder, Reporter};
use kube::{Client, Resource};
use kura_api::{KuraInstance, KuraTenantResource, KuraVaultResource, ReasonCode};
use kura_operator_adapters_inbound::FIELD_MANAGER;
use kura_operator_adapters_inbound::pusher::Pusher;
use kura_operator_adapters_inbound::webhook::KuraAdmission;
use kura_operator_testkit::{FakeGateway, StaticGatewayFactory};
use serde_json::json;
use weebo_kit_runtime::admission::Admission;
use weebo_kit_runtime::shared::SharedWatches;
use weebo_kit_runtime::{Health, Shutdown, WatchScope};

const TIMEOUT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(200);

async fn create<K>(api: &Api<K>, value: serde_json::Value) -> K
where
    K: Resource + Clone + std::fmt::Debug + serde::de::DeserializeOwned + serde::Serialize,
{
    let obj: K = serde_json::from_value(value).expect("a valid object");
    api.create(&PostParams::default(), &obj)
        .await
        .expect("created")
}

/// `Ready`'s (status, reason, message) of `name`, once it has one.
async fn ready<K>(api: &Api<K>, name: &str, want: (&str, ReasonCode)) -> String
where
    K: Resource + Clone + std::fmt::Debug + serde::de::DeserializeOwned + serde::Serialize,
{
    tokio::time::timeout(TIMEOUT, async {
        loop {
            let value = serde_json::to_value(api.get(name).await.expect("readable")).expect("json");
            let conditions: Vec<weebo_kit_api::Condition> =
                serde_json::from_value(value["status"]["conditions"].clone()).unwrap_or_default();
            if let Some(c) = weebo_kit_api::status::ready(&conditions)
                && c.status == want.0
                && c.reason == want.1.as_str()
            {
                return c.message.clone();
            }
            tokio::time::sleep(POLL).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{name} never reached {want:?}"))
}

fn start_pusher(client: &Client, gateway: FakeGateway) -> tokio::task::JoinHandle<()> {
    let scope = WatchScope::cluster();
    let health = Arc::new(Health::default());
    let mut watches = SharedWatches::default();
    let tenants = watches.spawn(
        Api::<KuraTenantResource>::all(client.clone()),
        &scope,
        &health,
        "t",
    );
    let vaults = watches.spawn(
        Api::<KuraVaultResource>::all(client.clone()),
        &scope,
        &health,
        "v",
    );
    let instances = watches.spawn(
        Api::<KuraInstance>::all(client.clone()),
        &scope,
        &health,
        "i",
    );
    let triggers = futures::stream::select_all([
        watches
            .get::<KuraTenantResource>()
            .expect("watch")
            .touched()
            .map(drop)
            .boxed(),
        watches
            .get::<KuraVaultResource>()
            .expect("watch")
            .touched()
            .map(drop)
            .boxed(),
        watches
            .get::<KuraInstance>()
            .expect("watch")
            .touched()
            .map(drop)
            .boxed(),
    ]);
    let pusher = Pusher {
        client: client.clone(),
        recorder: Recorder::new(
            client.clone(),
            Reporter {
                controller: FIELD_MANAGER.into(),
                instance: None,
            },
        ),
        gateways: Arc::new(StaticGatewayFactory(gateway)),
        tenants,
        vaults,
        instances,
    };
    tokio::spawn(async move {
        // The watches live as long as the loop.
        let _watches = watches;
        pusher.run(triggers, Shutdown::never()).await;
    })
}

#[tokio::test]
async fn a_push_becomes_every_resources_status() {
    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();
    let instances: Api<KuraInstance> = Api::all(client.clone());
    let tenants: Api<KuraTenantResource> = Api::all(client.clone());
    let vaults: Api<KuraVaultResource> = Api::namespaced(client.clone(), "family");
    create(
        &Api::<Namespace>::all(client.clone()),
        json!({ "metadata": { "name": "family" } }),
    )
    .await;
    create(
        &instances,
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": "kura" },
                "spec": { "url": "http://kura.kura.svc:8080", "auth": { "serviceAccountToken": { "audience": "kura" } } } }),
    )
    .await;

    let gateway = FakeGateway::default();
    let pusher = start_pusher(&client, gateway.clone());

    // T10 (operator half): no tenant, nothing pushed; the instance says so.
    let message = ready(&instances, "kura", ("True", ReasonCode::Reconciled)).await;
    assert_eq!(message, "no KuraTenant to push");
    assert!(gateway.pushes().is_empty());

    create(
        &tenants,
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "family" },
                "spec": { "vaultNamespaces": ["family"] } }),
    )
    .await;
    create(
        &vaults,
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault", "metadata": { "name": "recipes", "namespace": "family" },
                "spec": { "tenantRef": { "name": "family" } } }),
    )
    .await;
    ready(&tenants, "family", ("True", ReasonCode::Reconciled)).await;
    ready(&vaults, "recipes", ("True", ReasonCode::Reconciled)).await;
    let vault = vaults.get("recipes").await.expect("readable");
    let status = vault.status.expect("a status");
    assert_eq!(status.observed_generation, vault.metadata.generation);
    assert_eq!(status.instance.expect("bound").name, "kura");
    let last = gateway.pushes().last().cloned().expect("a push");
    let names: Vec<&str> = last
        .iter()
        .filter_map(|d| d["metadata"]["name"].as_str())
        .collect();
    assert_eq!(names, ["family", "recipes"]);
    assert!(
        last[1]["metadata"].get("uid").is_none(),
        "Kubernetes' fields are dropped"
    );

    // The status patches woke the loop once more, and then it went quiet.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let settled = gateway.pushes().len();
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(
        gateway.pushes().len(),
        settled,
        "status patches must not loop"
    );

    // The server refuses the vault: the vault shows why, the tenant names it.
    gateway.refuse("KuraVault/family/recipes", "spec.chunking: min 64 to 1Mi");
    vaults
        .patch(
            "recipes",
            &PatchParams::default(),
            &Patch::Merge(
                json!({ "spec": { "bindings": [{ "role": "viewer", "subject": "sub-1" }] } }),
            ),
        )
        .await
        .expect("patched");
    let why = ready(&vaults, "recipes", ("False", ReasonCode::InvalidDocument)).await;
    assert!(why.contains("spec.chunking"), "{why}");
    let named = ready(&tenants, "family", ("False", ReasonCode::SetRejected)).await;
    assert!(
        named.contains("KuraVault/family/recipes was refused"),
        "{named}"
    );
    let events: Api<Event> = Api::namespaced(client.clone(), "family");
    let warned = tokio::time::timeout(TIMEOUT, async {
        loop {
            let found = events
                .list(&ListParams::default().fields("involvedObject.name=recipes"))
                .await
                .expect("events")
                .items
                .into_iter()
                .any(|e| {
                    e.type_.as_deref() == Some("Warning")
                        && e.reason.as_deref() == Some("InvalidDocument")
                });
            if found {
                return;
            }
            tokio::time::sleep(POLL).await;
        }
    })
    .await;
    assert!(warned.is_ok(), "a Warning Event on the refused vault");
    pusher.abort();
}

/// T2: the webhook refuses a vault outside its tenant's namespaces, or
/// naming no tenant, and a second instance — reading the tenant live.
#[tokio::test]
async fn webhook_refuses_unlisted_namespace() {
    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();
    create(
        &Api::<KuraTenantResource>::all(client.clone()),
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "family" },
                "spec": { "vaultNamespaces": ["family"] } }),
    )
    .await;
    let admission = KuraAdmission {
        client: client.clone(),
    };
    let vault = |tenant: &str| -> DynamicObject {
        serde_json::from_value(json!({
            "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault",
            "metadata": { "name": "recipes" }, "spec": { "tenantRef": { "name": tenant } }
        }))
        .expect("a vault")
    };
    assert!(
        admission
            .admit("KuraVault", "family", &vault("family"))
            .await
            .is_ok()
    );
    let refused = admission
        .admit("KuraVault", "team-b", &vault("family"))
        .await
        .expect_err("refused");
    assert_eq!(refused.reason, ReasonCode::VaultNamespaceNotAllowed);
    let ghost = admission
        .admit("KuraVault", "family", &vault("ghost"))
        .await
        .expect_err("refused");
    assert_eq!(ghost.reason, ReasonCode::TenantNotFound);

    let instance = |name: &str| -> DynamicObject {
        serde_json::from_value(json!({
            "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": name },
            "spec": { "url": "http://kura:8080", "auth": { "serviceAccountToken": { "audience": "kura" } } }
        }))
        .expect("an instance")
    };
    assert!(
        admission
            .admit("KuraInstance", "", &instance("kura"))
            .await
            .is_ok()
    );
    create(
        &Api::<KuraInstance>::all(client.clone()),
        serde_json::to_value(instance("kura")).expect("json"),
    )
    .await;
    assert!(
        admission
            .admit("KuraInstance", "", &instance("kura"))
            .await
            .is_ok(),
        "the same one, updated"
    );
    assert!(
        admission
            .admit("KuraInstance", "", &instance("other"))
            .await
            .is_err()
    );
    assert!(
        admission
            .admit("ConfigMap", "family", &instance("x"))
            .await
            .is_err(),
        "fails closed"
    );
}

/// No `KuraInstance`, then two: nothing is pushed and every resource names
/// why (`InstanceNotFound`); a vault being deleted is no longer published.
#[tokio::test]
async fn without_exactly_one_instance_nothing_is_pushed() {
    let cluster = kura_operator_testkit::start().await;
    let client = cluster.client();
    let instances: Api<KuraInstance> = Api::all(client.clone());
    let tenants: Api<KuraTenantResource> = Api::all(client.clone());
    create(
        &tenants,
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "family" },
                "spec": { "vaultNamespaces": ["family"] } }),
    )
    .await;
    let gateway = FakeGateway::default();
    let pusher = start_pusher(&client, gateway.clone());

    let why = ready(&tenants, "family", ("False", ReasonCode::InstanceNotFound)).await;
    assert_eq!(why, "no KuraInstance names the Kura server");

    let instance = |name: &str| {
        json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraInstance", "metadata": { "name": name },
                "spec": { "url": "http://kura:8080", "auth": { "serviceAccountToken": { "audience": "kura" } } } })
    };
    create(&instances, instance("kura")).await;
    ready(&tenants, "family", ("True", ReasonCode::Reconciled)).await;
    create(&instances, instance("other")).await;
    let why = ready(&tenants, "family", ("False", ReasonCode::InstanceNotFound)).await;
    assert_eq!(why, "2 KuraInstances: one server per cluster");
    assert_eq!(
        ready(&instances, "other", ("False", ReasonCode::InstanceNotFound)).await,
        why
    );
    assert_eq!(
        gateway.pushes().len(),
        1,
        "only while there was one instance"
    );
    pusher.abort();
}

/// The webhook's other answers: a tenant is always admitted, a vault that
/// does not parse is `InvalidDocument`, and so is what it cannot decide.
#[tokio::test]
async fn webhook_admits_tenants_and_refuses_what_it_cannot_read() {
    let cluster = kura_operator_testkit::start().await;
    let admission = KuraAdmission {
        client: cluster.client(),
    };
    let object = |value: serde_json::Value| -> DynamicObject {
        serde_json::from_value(value).expect("an object")
    };
    assert!(
        admission
            .admit(
                "KuraTenant",
                "",
                &object(
                    json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant",
                                "metadata": { "name": "family" }, "spec": {} })
                )
            )
            .await
            .is_ok()
    );
    let unreadable = admission
        .admit(
            "KuraVault",
            "family",
            &object(
                json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraVault",
                            "metadata": { "name": "recipes" }, "spec": { "tenantRef": 7 } }),
            ),
        )
        .await
        .expect_err("refused");
    assert_eq!(unreadable.reason, ReasonCode::InvalidDocument);
    assert_eq!(admission.undecidable(), ReasonCode::InvalidDocument);
}
