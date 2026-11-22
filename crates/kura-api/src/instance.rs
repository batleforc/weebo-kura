//! `KuraInstance`: the Kura server the operator hands the documents to, and
//! how it proves who it is (RFC 0009 §4.1). Cluster-scoped, one per server.

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use weebo_kit_api::TlsOptions;

/// The Kura server the operator pushes `KuraTenant` and `KuraVault` to.
/// Cluster-scoped: one server serves the cluster. `Ready` tells whether the
/// server answers and accepts the operator's identity.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[kube(
    group = "kura.weebo.io",
    version = "v1alpha1",
    kind = "KuraInstance",
    plural = "kurainstances",
    doc = "A Kura server the operator hands the KuraTenant and KuraVault documents to.",
    status = "crate::status::Status",
    printcolumn = r#"{"name":"URL","type":"string","jsonPath":".spec.url"}"#,
    printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].status"}"#,
    printcolumn = r#"{"name":"Reason","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].reason"}"#
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KuraInstanceSpec {
    /// Base URL of the Kura server's API, such as `http://kura.kura.svc:8080`.
    /// Use `https` for anything that leaves the node.
    #[schemars(regex(pattern = r"^https?://[^/?#\s]+(/[^?#\s]*)?$"))]
    pub url: String,
    /// How the operator verifies the server's TLS certificate (`https` only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<TlsOptions>,
    /// How the operator proves who it is to the server. Required: the server
    /// accepts the documents from one configured identity only.
    pub auth: InstanceAuth,
}

/// The operator's identity towards Kura.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstanceAuth {
    /// A projected ServiceAccount token, rotated by the kubelet, that the
    /// server verifies against the cluster's OIDC issuer. No secret at rest.
    pub service_account_token: ServiceAccountToken,
}

/// The projected token the operator presents.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServiceAccountToken {
    /// The token's audience, equal to the server's `documents.operator.audience`.
    #[schemars(length(min = 1, max = 253))]
    pub audience: String,
}

#[cfg(test)]
mod tests {
    use kube::CustomResourceExt;

    use super::*;

    #[test]
    fn the_instance_is_cluster_scoped_and_requires_auth() {
        let crd = KuraInstance::crd();
        assert_eq!(crd.spec.scope, "Cluster");
        let schema = serde_json::to_value(&crd.spec.versions[0].schema).unwrap();
        let required = &schema["openAPIV3Schema"]["properties"]["spec"]["required"];
        assert_eq!(required, &serde_json::json!(["auth", "url"]));
    }

    #[test]
    fn a_spec_without_auth_is_refused() {
        let err = serde_json::from_value::<KuraInstanceSpec>(
            serde_json::json!({ "url": "http://kura:8080" }),
        );
        assert!(err.is_err());
    }
}
