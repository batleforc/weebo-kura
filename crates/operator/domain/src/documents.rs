//! The resources as the documents the server reads (RFC 0009 §4.2, "What is
//! sent"): `apiVersion`, `kind`, `metadata.{name, namespace, labels,
//! annotations, generation}` and `spec`. Kubernetes' own fields are dropped:
//! the server refuses unknown fields, as it does in a file.

use std::collections::BTreeMap;

use kube::{Resource, ResourceExt};
use kura_api::{KuraTenantResource, KuraVaultResource};
use serde::Serialize;
use serde_json::{Value, json};

/// Written by `kubectl apply`, never part of the document.
const LAST_APPLIED: &str = "kubectl.kubernetes.io/last-applied-configuration";

/// A resource and the document it becomes, under the name the server knows
/// it by (`kura_api::pushed_name`).
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub name: String,
    pub body: Value,
}

fn document<K, S>(resource: &K, spec: &S) -> Option<Document>
where
    K: Resource<DynamicType = ()>,
    S: Serialize,
{
    let annotations: BTreeMap<&String, &String> = resource
        .annotations()
        .iter()
        .filter(|(k, _)| k.as_str() != LAST_APPLIED)
        .collect();
    let mut metadata = serde_json::Map::new();
    metadata.insert("name".into(), json!(resource.name_any()));
    if let Some(ns) = resource.namespace() {
        metadata.insert("namespace".into(), json!(ns));
    }
    if !resource.labels().is_empty() {
        metadata.insert("labels".into(), json!(resource.labels()));
    }
    if !annotations.is_empty() {
        metadata.insert("annotations".into(), json!(annotations));
    }
    if let Some(generation) = resource.meta().generation {
        metadata.insert("generation".into(), json!(generation));
    }
    let body = json!({
        "apiVersion": K::api_version(&()),
        "kind": K::kind(&()),
        "metadata": metadata,
        "spec": serde_json::to_value(spec).ok()?,
    });
    // A resource the server could not name is still sent: the server's
    // refusal then names it in the status.
    let name = kura_api::pushed_name(&body).unwrap_or_else(|_| {
        format!(
            "{}/{}",
            K::kind(&()),
            resource
                .namespace()
                .map_or(resource.name_any(), |ns| format!(
                    "{ns}/{}",
                    resource.name_any()
                ))
        )
    });
    Some(Document { name, body })
}

/// The whole set, sorted by name: equal resources give an equal set.
pub fn set(tenants: &[KuraTenantResource], vaults: &[KuraVaultResource]) -> Vec<Document> {
    let mut docs: Vec<Document> = tenants
        .iter()
        .filter(|t| t.meta().deletion_timestamp.is_none())
        .filter_map(|t| document(t, &t.spec))
        .chain(
            vaults
                .iter()
                .filter(|v| v.meta().deletion_timestamp.is_none())
                .filter_map(|v| document(v, &v.spec)),
        )
        .collect();
    docs.sort_by(|a, b| a.name.cmp(&b.name));
    docs
}

/// The name of a resource as the server knows it, for the status.
pub fn name_of<K: Resource<DynamicType = ()>>(resource: &K) -> String {
    match resource.namespace() {
        Some(ns) => format!("{}/{ns}/{}", K::kind(&()), resource.name_any()),
        None => format!("{}/{}", K::kind(&()), resource.name_any()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kube::api::ObjectMeta;
    use kura_api::{TenantSpec, VaultSpec};
    use weebo_kit_api::ObjectRef;

    use super::*;

    fn tenant(name: &str) -> KuraTenantResource {
        let mut t = KuraTenantResource::new(name, TenantSpec::default());
        t.metadata = ObjectMeta {
            name: Some(name.into()),
            generation: Some(4),
            uid: Some("u-1".into()),
            resource_version: Some("123".into()),
            annotations: Some(BTreeMap::from([
                (LAST_APPLIED.to_string(), "{}".to_string()),
                ("kura.weebo.io/force-orphan".to_string(), "true".to_string()),
            ])),
            ..ObjectMeta::default()
        };
        t
    }

    fn vault(ns: &str, name: &str) -> KuraVaultResource {
        let mut v = KuraVaultResource::new(
            name,
            VaultSpec {
                tenant_ref: ObjectRef { name: ns.into() },
                chunking: None,
                config_sync: None,
                bindings: Vec::new(),
                deletion_policy: kura_api::DeletionPolicy::Orphan,
            },
        );
        v.metadata.namespace = Some(ns.into());
        v
    }

    #[test]
    fn kubernetes_fields_are_dropped_and_the_generation_kept() {
        let docs = set(&[tenant("maxime")], &[]);
        let body = &docs[0].body;
        assert_eq!(docs[0].name, "KuraTenant/maxime");
        assert_eq!(body["apiVersion"], "kura.weebo.io/v1alpha1");
        assert_eq!(body["metadata"]["generation"], 4);
        assert_eq!(
            body["metadata"]["annotations"],
            json!({ "kura.weebo.io/force-orphan": "true" })
        );
        assert!(body["metadata"].get("uid").is_none());
        assert!(body["metadata"].get("resourceVersion").is_none());
        assert!(body.get("status").is_none());
        // What the server parses: a document of the file loader.
        let text = serde_json::to_string(body).unwrap();
        let parsed = kura_api::parse_file(&docs[0].name, &text).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn the_set_is_sorted_and_skips_resources_being_deleted() {
        let mut gone = vault("maxime", "old");
        gone.metadata.deletion_timestamp = Some(k8s_time());
        let docs = set(
            &[tenant("zeta"), tenant("alpha")],
            &[vault("maxime", "notes"), gone],
        );
        let names: Vec<&str> = docs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "KuraTenant/alpha",
                "KuraTenant/zeta",
                "KuraVault/maxime/notes"
            ]
        );
        assert_eq!(name_of(&vault("maxime", "notes")), "KuraVault/maxime/notes");
    }

    fn k8s_time() -> k8s_openapi::apimachinery::pkg::apis::meta::v1::Time {
        serde_json::from_value(json!("2026-11-22T00:00:00Z")).unwrap()
    }
}
