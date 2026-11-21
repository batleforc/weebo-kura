//! kura-operator's test harness: the kit's envtest control plane with
//! Kura's CRDs from `deploy/crd/`, and a gateway factory serving the
//! in-memory Kura of `kura-operator-application`.

use std::path::Path;
use std::sync::Arc;

use kura_api::KuraInstance;
use kura_operator_application::ports::{GatewayFactory, KuraGateway};
pub use kura_operator_application::test_support::FakeGateway;
use kura_operator_domain::Failure;
pub use weebo_kit_testkit::envtest::EnvTestCluster;

/// A control plane with every CRD of `deploy/crd/` installed.
pub async fn start() -> EnvTestCluster {
    EnvTestCluster::start(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../deploy/crd")).await
}

/// Every instance is served by the same in-memory Kura.
pub struct StaticGatewayFactory(pub FakeGateway);

#[async_trait::async_trait]
impl GatewayFactory for StaticGatewayFactory {
    async fn gateway_for(&self, _: &KuraInstance) -> Result<Arc<dyn KuraGateway>, Failure> {
        Ok(Arc::new(self.0.clone()))
    }
}
