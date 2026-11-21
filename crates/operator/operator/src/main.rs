//! The `kura-operator` binary (RFC 0009): the kit's runtime (telemetry,
//! health, leader election, the webhook server), the shared watches of
//! `KuraTenant`, `KuraVault` and `KuraInstance`, and the push loop on the
//! leader only.

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::Client;
use kube::runtime::events::{Recorder, Reporter};
use kura_api::{KuraInstance, KuraTenantResource, KuraVaultResource};
use kura_operator_adapters_inbound::pusher::Pusher;
use kura_operator_adapters_inbound::{FIELD_MANAGER, METRIC_PREFIX, webhook};
use kura_operator_adapters_outbound::HttpGatewayFactory;
use weebo_kit_runtime::shared::{LISTED_TIMEOUT, SharedWatches};
use weebo_kit_runtime::{Health, Shutdown, WatchScope, leader, telemetry, webhook_server};

/// Keep it across releases: old and new pods of a rollout compete for it.
const DEFAULT_LEASE_NAME: &str = "kura-operator-leader";
/// The operator's own ServiceAccount, whose tokens it mints for Kura (the
/// chart sets it from `spec.serviceAccountName`); its namespace is the pod's.
const SERVICE_ACCOUNT_ENV: &str = "KURA_SERVICE_ACCOUNT";
const NAMESPACE_ENV: &str = "POD_NAMESPACE";

fn main() -> anyhow::Result<()> {
    weebo_kit_runtime::runtime::build()?.block_on(operator())
}

async fn operator() -> anyhow::Result<()> {
    let telemetry = telemetry::init(FIELD_MANAGER)?;
    let result = match std::env::args().nth(1).as_deref() {
        // What the image's smoke run calls: no cluster needed.
        Some("--version") => {
            println!("{FIELD_MANAGER} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(other) => Err(anyhow::anyhow!("unknown command {other:?}")),
        None => run(&telemetry).await,
    };
    if let Err(err) = &result {
        tracing::error!(error = %err, "operator failed");
    }
    telemetry.shutdown();
    result
}

async fn run(telemetry: &telemetry::Telemetry) -> anyhow::Result<()> {
    // kube's rustls and reqwest's are both linked: rustls then picks no
    // default provider. An `Err` only means one is installed already.
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::debug!("a rustls crypto provider was already installed");
    }
    telemetry.serve_metrics().await?;
    leader::register_leader_gauge(FIELD_MANAGER, METRIC_PREFIX);

    let client = Client::try_default().await?;
    let scope = WatchScope::from_env()?;
    // Tenants and the instance are cluster-scoped, and a set is whole or
    // nothing: a namespace-limited operator would push a partial set.
    anyhow::ensure!(
        scope.cluster_kinds() && scope.watched_namespace().is_none() && scope.selector().is_none(),
        "kura-operator watches every KuraTenant, KuraVault and KuraInstance of the cluster: leave the watch namespace and label selector unset"
    );
    let shutdown = Shutdown::on_termination()?;
    let health = Arc::new(Health::default());

    let mut watches = SharedWatches::default();
    let tenants = watches.spawn(
        scope.api::<KuraTenantResource>(client.clone()),
        &scope,
        &health,
        "KuraTenant watch",
    );
    let vaults = watches.spawn(
        scope.api::<KuraVaultResource>(client.clone()),
        &scope,
        &health,
        "KuraVault watch",
    );
    let instances = watches.spawn(
        scope.api::<KuraInstance>(client.clone()),
        &scope,
        &health,
        "KuraInstance watch",
    );
    let touched = |w: Option<futures::stream::BoxStream<'static, ()>>| {
        w.unwrap_or_else(|| futures::stream::empty().boxed())
    };
    let triggers = futures::stream::select_all([
        touched(
            watches
                .get::<KuraTenantResource>()
                .map(|w| w.touched().map(drop).boxed()),
        ),
        touched(
            watches
                .get::<KuraVaultResource>()
                .map(|w| w.touched().map(drop).boxed()),
        ),
        touched(
            watches
                .get::<KuraInstance>()
                .map(|w| w.touched().map(drop).boxed()),
        ),
    ]);

    let holder_id = std::env::var("HOSTNAME").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    let service_account = std::env::var(SERVICE_ACCOUNT_ENV)
        .map_err(|_| anyhow::anyhow!("{SERVICE_ACCOUNT_ENV} is not set"))?;
    let namespace =
        std::env::var(NAMESPACE_ENV).map_err(|_| anyhow::anyhow!("{NAMESPACE_ENV} is not set"))?;
    let pusher = Pusher {
        client: client.clone(),
        recorder: Recorder::new(
            client.clone(),
            Reporter {
                controller: FIELD_MANAGER.into(),
                instance: Some(holder_id.clone()),
            },
        ),
        gateways: Arc::new(HttpGatewayFactory::new(
            client.clone(),
            namespace,
            service_account,
        )),
        tenants,
        vaults,
        instances,
    };

    // The webhook runs on every replica, leader or not: a leader-only
    // webhook would turn each failover into refused applies.
    let server = webhook_server::spawn(
        webhook::router(client.clone(), health.clone()),
        health,
        shutdown.clone(),
    )
    .await?;

    // One pusher at a time: the kit's Lease.
    let campaign = async {
        watches.wait_listed(LISTED_TIMEOUT).await;
        leader::acquire_leadership(&client, holder_id, DEFAULT_LEASE_NAME).await
    };
    let leadership = tokio::select! {
        leadership = campaign => leadership,
        () = shutdown.wait() => {
            server.stopped().await;
            return Ok(());
        }
    };
    tracing::info!("pushing the documents");
    pusher.run(triggers, shutdown.clone()).await;
    leadership.release().await;
    if tokio::time::timeout(Duration::from_secs(30), server.stopped())
        .await
        .is_err()
    {
        tracing::warn!("the webhook server was still draining after 30 s; exiting anyway");
    }
    Ok(())
}
