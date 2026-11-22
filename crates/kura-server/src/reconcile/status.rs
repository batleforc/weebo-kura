//! Per-object status in the kit's shape (§4.3): `observedGeneration` (the
//! pushed `metadata.generation`, else a hash of the document) and conditions, `Ready` first, with reasons
//! from Kura's catalogue.

use kura_api::ReasonCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use weebo_kit_api::Condition;

use super::Removed;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectStatus {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    pub name: String,
    /// `file#index`; absent for an object no longer declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredStatus {
    pub objects: Vec<ObjectStatus>,
}

impl StoredStatus {
    /// Objects whose `Ready` is not `True`, or that carry an advisory
    /// condition: the `kura_config_warnings` gauge.
    pub fn warnings(&self) -> usize {
        self.objects
            .iter()
            .filter(|o| {
                weebo_kit_api::status::ready(&o.conditions).is_none_or(|c| c.status != "True")
                    || o.conditions
                        .iter()
                        .any(|c| c.type_ != Condition::READY && c.status == "True")
            })
            .count()
    }
}

/// The document's `metadata.generation` when kura-operator sent it (RFC 0009
/// §4.2), else a hash of the document as parsed, positive, stable for equal
/// documents.
fn observed_generation<S: Serialize>(doc: &kura_api::Document<S>) -> Option<i64> {
    if let Some(generation) = doc.metadata.generation {
        return Some(generation);
    }
    let bytes = serde_json::to_vec(doc).ok()?;
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let mut first = [0u8; 8];
    first.copy_from_slice(digest.get(..8)?);
    Some(i64::from_be_bytes(first) & i64::MAX)
}

fn ready(status: bool, reason: ReasonCode, message: String) -> Condition {
    Condition {
        type_: Condition::READY.into(),
        status: if status { "True" } else { "False" }.into(),
        reason: reason.as_str().into(),
        message,
        last_transition_time: None,
        observed_generation: None,
    }
}

/// RFC 0005 §4.3: a restricted binding whose claim value the tenant maps to
/// `owner` or `admin`, who open the whole vault anyway.
fn widened(
    config: &kura_api::Config,
    v: &kura_api::validate::Sourced<kura_api::spec::KuraVault>,
) -> Option<Condition> {
    let mapping = config
        .tenants
        .get(v.doc.spec.tenant_ref.name.as_str())?
        .doc
        .spec
        .claim_mappings
        .as_ref()?;
    let wide: Vec<String> = v
        .doc
        .spec
        .bindings
        .iter()
        .filter(|b| b.paths.is_some())
        .filter_map(|b| b.claim_value.as_ref())
        .filter(|c| mapping.roles.owner.contains(c) || mapping.roles.admin.contains(c))
        .cloned()
        .collect();
    (!wide.is_empty()).then(|| Condition {
        type_: ReasonCode::RestrictedBindingWidened.as_str().into(),
        status: "True".into(),
        reason: ReasonCode::RestrictedBindingWidened.as_str().into(),
        message: format!(
            "{} lists folders but the tenant maps it to owner or admin: the whole vault is open to it",
            wide.join(", ")
        ),
        last_transition_time: None,
        observed_generation: None,
    })
}

pub fn build(config: &kura_api::Config, removed: &[Removed]) -> StoredStatus {
    let mut objects = Vec::new();
    for (slug, t) in &config.tenants {
        let generation = observed_generation(&t.doc);
        objects.push(ObjectStatus {
            kind: kura_api::spec::KIND_TENANT.into(),
            namespace: None,
            name: slug.to_string(),
            source: Some(t.source.to_string()),
            observed_generation: generation,
            conditions: vec![Condition {
                observed_generation: generation,
                ..ready(true, ReasonCode::Reconciled, String::new())
            }],
        });
    }
    for (key, v) in &config.vaults {
        let generation = observed_generation(&v.doc);
        let mut conditions = vec![Condition {
            observed_generation: generation,
            ..ready(true, ReasonCode::Reconciled, String::new())
        }];
        if let Some(c) = widened(config, v) {
            conditions.push(c);
        }
        objects.push(ObjectStatus {
            kind: kura_api::spec::KIND_VAULT.into(),
            namespace: Some(key.namespace.to_string()),
            name: key.name.to_string(),
            source: Some(v.source.to_string()),
            observed_generation: generation,
            conditions,
        });
    }
    for r in removed {
        let condition = match r.delete_after {
            Some(when) => ready(
                false,
                ReasonCode::DeletionScheduled,
                format!(
                    "removed from the files with deletionPolicy Delete; deleted after {when} unless the document returns"
                ),
            ),
            None => ready(
                false,
                ReasonCode::Orphaned,
                "removed from the files; disabled, data kept".into(),
            ),
        };
        let kind = if r.kind == "tenant" {
            kura_api::spec::KIND_TENANT
        } else {
            kura_api::spec::KIND_VAULT
        };
        objects.push(ObjectStatus {
            kind: kind.into(),
            namespace: r.namespace.clone(),
            name: r.name.clone(),
            source: None,
            observed_generation: None,
            conditions: vec![condition],
        });
    }
    StoredStatus { objects }
}
