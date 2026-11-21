//! The admission webhook (RFC 0009 §4.3): what Kubernetes can refuse before
//! anything reaches Kura. A `KuraVault` in a namespace its tenant does not
//! list, or naming no tenant, and a second `KuraInstance`. The tenant is
//! read live, not from a cache that may not have seen it yet; the server
//! re-validates the whole set regardless (T4), so this is a convenience
//! that fails closed, never the control.

use std::sync::Arc;

use axum::Router;
use kube::Client;
use kube::api::{Api, ListParams};
use kube::core::DynamicObject;
use kura_api::{KuraInstance, KuraTenantResource, KuraVaultResource, ReasonCode};
use weebo_kit_runtime::Health;
use weebo_kit_runtime::admission::{Admission, Denial, typed};

use crate::{FIELD_MANAGER, METRIC_PREFIX};

pub struct KuraAdmission {
    pub client: Client,
}

/// The rule for a vault, given its tenant as read.
pub fn vault_allowed(
    namespace: &str,
    tenant_name: &str,
    tenant: Option<&KuraTenantResource>,
) -> Result<(), Denial<ReasonCode>> {
    let Some(tenant) = tenant else {
        return Err(Denial::new(
            ReasonCode::TenantNotFound,
            format!("TenantNotFound: tenant `{tenant_name}` does not exist"),
        ));
    };
    if !tenant
        .spec
        .vault_namespaces
        .iter()
        .any(|ns| ns.as_str() == namespace)
    {
        return Err(Denial::new(
            ReasonCode::VaultNamespaceNotAllowed,
            format!(
                "VaultNamespaceNotAllowed: namespace `{namespace}` is not in vaultNamespaces of tenant `{tenant_name}`"
            ),
        ));
    }
    Ok(())
}

#[async_trait::async_trait]
impl Admission for KuraAdmission {
    type Reason = ReasonCode;

    async fn admit(
        &self,
        kind: &str,
        namespace: &str,
        object: &DynamicObject,
    ) -> Result<(), Denial<ReasonCode>> {
        let invalid = |m: String| Denial::new(ReasonCode::InvalidDocument, m);
        match kind {
            "KuraTenant" => Ok(()),
            "KuraVault" => {
                let vault: KuraVaultResource = typed(object).map_err(|e| invalid(e.to_string()))?;
                let name = &vault.spec.tenant_ref.name;
                let tenant = Api::<KuraTenantResource>::all(self.client.clone())
                    .get_opt(name)
                    .await
                    .map_err(|e| invalid(format!("tenant `{name}` could not be read: {e}")))?;
                vault_allowed(namespace, name, tenant.as_ref())
            }
            "KuraInstance" => {
                let this = object.metadata.name.clone().unwrap_or_default();
                let others: Vec<String> = Api::<KuraInstance>::all(self.client.clone())
                    .list(&ListParams::default())
                    .await
                    .map_err(|e| invalid(format!("instances could not be listed: {e}")))?
                    .items
                    .into_iter()
                    .filter_map(|i| i.metadata.name)
                    .filter(|n| *n != this)
                    .collect();
                if others.is_empty() {
                    Ok(())
                } else {
                    Err(invalid(format!(
                        "one KuraInstance per cluster: {} already names the server",
                        others.join(", ")
                    )))
                }
            }
            other => Err(invalid(format!(
                "kind {other} is not governed by kura-operator"
            ))),
        }
    }

    fn undecidable(&self) -> ReasonCode {
        ReasonCode::InvalidDocument
    }
}

/// `/validate` and the kit's probes.
pub fn router(client: Client, health: Arc<Health>) -> Router {
    weebo_kit_runtime::admission::router(
        Arc::new(KuraAdmission { client }),
        health,
        FIELD_MANAGER,
        METRIC_PREFIX,
    )
}

#[cfg(test)]
mod tests {
    use kura_api::TenantSpec;

    use super::*;

    #[test]
    fn a_vault_needs_its_tenant_and_a_listed_namespace() {
        let mut tenant = KuraTenantResource::new("family", TenantSpec::default());
        tenant.spec.vault_namespaces =
            vec![kura_api::DnsLabel::try_from("family".to_string()).unwrap()];
        assert!(vault_allowed("family", "family", Some(&tenant)).is_ok());
        let refused = vault_allowed("team-b", "family", Some(&tenant)).unwrap_err();
        assert_eq!(refused.reason, ReasonCode::VaultNamespaceNotAllowed);
        assert_eq!(
            refused.message,
            "VaultNamespaceNotAllowed: namespace `team-b` is not in vaultNamespaces of tenant `family`"
        );
        assert_eq!(
            vault_allowed("family", "ghost", None).unwrap_err().reason,
            ReasonCode::TenantNotFound
        );
    }
}
