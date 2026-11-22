//! The push loop (RFC 0009 §5.3): any change to a `KuraTenant`, `KuraVault`
//! or `KuraInstance` wakes it; events within [`DEBOUNCE`] make one push of
//! the whole set, built from the caches — never before they have listed, so
//! a partial set is never sent (T10). The server's answer becomes each
//! resource's status, patched only when it changed: the patch wakes the
//! loop again, the next push is `unchanged`, and nothing more is written.

use std::fmt::Debug;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use futures::{FutureExt, Stream, StreamExt};
use kube::api::Api;
use kube::runtime::events::Recorder;
use kube::runtime::reflector::Store;
use kube::{Client, Resource, ResourceExt};
use kura_api::status::{BoundInstance, Status};
use kura_api::{KuraInstance, KuraTenantResource, KuraVaultResource, ReasonCode};
use kura_operator_application::ports::GatewayFactory;
use kura_operator_application::push::{self, Outcome};
use kura_operator_domain::Failure;
use kura_operator_domain::documents::{self, Document};
use kura_operator_domain::verdict::Verdict;
use serde::Serialize;
use serde::de::DeserializeOwned;
use weebo_kit_api::Condition;
use weebo_kit_runtime::Shutdown;
use weebo_kit_runtime::metrics::ReconcileMetrics;
use weebo_kit_runtime::status::StatusUpdate;

use crate::{FIELD_MANAGER, METRIC_PREFIX};

/// Events closer than this make one push.
pub const DEBOUNCE: Duration = Duration::from_secs(1);
/// A push at least this often, changes or not: the server's status moves too
/// (a quota passed, a deletion due).
pub const RESYNC: Duration = Duration::from_secs(300);
/// After a failed push, the next one.
pub const RETRY: Duration = Duration::from_secs(30);

/// The status every kind of this operator carries.
pub trait HasStatus {
    fn status(&self) -> Option<&Status>;
}

macro_rules! has_status {
    ($($k:ty),*) => {$(
        impl HasStatus for $k {
            fn status(&self) -> Option<&Status> {
                self.status.as_ref()
            }
        }
    )*};
}
has_status!(KuraTenantResource, KuraVaultResource, KuraInstance);

fn metrics() -> &'static ReconcileMetrics {
    static METRICS: OnceLock<ReconcileMetrics> = OnceLock::new();
    METRICS.get_or_init(|| ReconcileMetrics::new(FIELD_MANAGER, METRIC_PREFIX))
}

pub struct Pusher {
    pub client: Client,
    pub recorder: Recorder,
    pub gateways: Arc<dyn GatewayFactory>,
    pub tenants: Store<KuraTenantResource>,
    pub vaults: Store<KuraVaultResource>,
    pub instances: Store<KuraInstance>,
}

fn items<K>(store: &Store<K>) -> Vec<K>
where
    K: Resource<DynamicType = ()> + Clone + 'static,
{
    store.state().iter().map(|k| K::clone(k)).collect()
}

/// Whether publishing `verdict` would leave `obj`'s status as it is.
fn unchanged<K: Resource<DynamicType = ()> + HasStatus>(
    obj: &K,
    verdict: &Verdict,
    instance: Option<&BoundInstance>,
) -> bool {
    let status = obj.status();
    let previous = status.map(|s| s.conditions.as_slice()).unwrap_or_default();
    let generation = obj.meta().generation;
    let now =
        k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(k8s_openapi::jiff::Timestamp::now());
    let next = weebo_kit_runtime::conditions::conditions(
        previous,
        &verdict.ready,
        &verdict.advisories,
        generation,
        &now,
    );
    let key = |c: &Condition| {
        (
            c.type_.clone(),
            c.status.clone(),
            c.reason.clone(),
            c.message.clone(),
            c.observed_generation,
        )
    };
    let same_conditions =
        previous.len() == next.len() && previous.iter().map(key).eq(next.iter().map(key));
    let same_generation =
        verdict.ready.is_err() || status.and_then(|s| s.observed_generation) == generation;
    same_conditions && same_generation && status.and_then(|s| s.instance.as_ref()) == instance
}

impl Pusher {
    /// Until `shutdown`: a push at once, then on every change (debounced),
    /// every [`RESYNC`], or [`RETRY`] after a failure.
    pub async fn run(self, triggers: impl Stream<Item = ()> + Send, shutdown: Shutdown) {
        let mut triggers = std::pin::pin!(triggers);
        let synced = async {
            self.tenants.wait_until_ready().await.ok();
            self.vaults.wait_until_ready().await.ok();
            self.instances.wait_until_ready().await.ok();
        };
        tokio::select! {
            () = synced => {}
            () = shutdown.wait() => return,
        }
        loop {
            let ok = self.push_once().await;
            let wait = if ok { RESYNC } else { RETRY };
            tokio::select! {
                () = shutdown.wait() => return,
                () = tokio::time::sleep(wait) => {}
                next = triggers.next() => {
                    if next.is_none() {
                        return;
                    }
                    tokio::time::sleep(DEBOUNCE).await;
                    while let Some(Some(())) = triggers.next().now_or_never() {}
                }
            }
        }
    }

    /// One push and its statuses; `false` when it should be retried soon.
    pub async fn push_once(&self) -> bool {
        let started = Instant::now();
        let tenants = items(&self.tenants);
        let vaults = items(&self.vaults);
        let instances = items(&self.instances);
        let docs = documents::set(&tenants, &vaults);
        let (outcome, bound) = match instances.as_slice() {
            [instance] if !tenants.is_empty() => (
                push::push(self.gateways.as_ref(), instance, &docs).await,
                Some(BoundInstance::cluster(instance.name_any())),
            ),
            [instance] => (
                no_tenant(&docs),
                Some(BoundInstance::cluster(instance.name_any())),
            ),
            [] => (
                all_failed(&docs, "no KuraInstance names the Kura server"),
                None,
            ),
            several => (
                all_failed(
                    &docs,
                    &format!("{} KuraInstances: one server per cluster", several.len()),
                ),
                None,
            ),
        };
        metrics().record("Set", outcome.reason(), started.elapsed());
        tracing::info!(documents = docs.len(), outcome = %outcome.summary, "set pushed");
        let mut ok = outcome.instance.ready.is_ok();
        for t in &tenants {
            ok &= self
                .publish_named(Api::all(self.client.clone()), t, &outcome, bound.as_ref())
                .await;
        }
        for v in &vaults {
            let api = Api::namespaced(self.client.clone(), &v.namespace().unwrap_or_default());
            ok &= self.publish_named(api, v, &outcome, bound.as_ref()).await;
        }
        for i in &instances {
            ok &= self
                .publish(Api::all(self.client.clone()), i, &outcome.instance, None)
                .await;
        }
        ok
    }

    async fn publish_named<K>(
        &self,
        api: Api<K>,
        obj: &K,
        outcome: &Outcome,
        instance: Option<&BoundInstance>,
    ) -> bool
    where
        K: Resource<DynamicType = ()> + HasStatus + Clone + Debug + DeserializeOwned + Serialize,
    {
        let name = documents::name_of(obj);
        match outcome.resources.iter().find(|(n, _)| *n == name) {
            Some((_, verdict)) => self.publish(api, obj, verdict, instance).await,
            // Being deleted: no longer part of the set.
            None => true,
        }
    }

    async fn publish<K>(
        &self,
        api: Api<K>,
        obj: &K,
        verdict: &Verdict,
        instance: Option<&BoundInstance>,
    ) -> bool
    where
        K: Resource<DynamicType = ()> + HasStatus + Clone + Debug + DeserializeOwned + Serialize,
    {
        if unchanged(obj, verdict, instance) {
            return true;
        }
        let mut fields = serde_json::Map::new();
        if let Some(i) = instance {
            fields.insert(
                "instance".into(),
                serde_json::to_value(i).unwrap_or_default(),
            );
        }
        let previous = obj
            .status()
            .map(|s| s.conditions.as_slice())
            .unwrap_or_default();
        let update = StatusUpdate {
            ready: &verdict.ready,
            advisories: &verdict.advisories,
            fields,
        };
        match weebo_kit_runtime::status::publish(
            &api,
            &self.recorder,
            FIELD_MANAGER,
            obj,
            previous,
            update,
        )
        .await
        {
            Ok(_) => true,
            Err(e) => {
                tracing::warn!(error = %e, object = %obj.name_any(), "status not published");
                false
            }
        }
    }
}

fn all_failed(docs: &[Document], message: &str) -> Outcome {
    let failure = Failure::new(ReasonCode::InstanceNotFound, message);
    Outcome {
        instance: Verdict::failed(failure.clone()),
        resources: docs
            .iter()
            .map(|d| (d.name.clone(), Verdict::failed(failure.clone())))
            .collect(),
        summary: ReasonCode::InstanceNotFound.as_str().into(),
    }
}

/// No `KuraTenant` at all: nothing is pushed (the server refuses an empty
/// set, and would never read it as a removal); vaults name the cause.
fn no_tenant(docs: &[Document]) -> Outcome {
    let failure = Failure::new(
        ReasonCode::TenantNotFound,
        "no KuraTenant exists: nothing is pushed",
    );
    Outcome {
        instance: Verdict {
            ready: Ok("no KuraTenant to push".into()),
            advisories: Vec::new(),
        },
        resources: docs
            .iter()
            .map(|d| (d.name.clone(), Verdict::failed(failure.clone())))
            .collect(),
        summary: "empty".into(),
    }
}
