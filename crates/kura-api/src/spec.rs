//! The `KuraTenant` and `KuraVault` documents (RFC 0001 §4.1), in the
//! conventions of weebo-operator-kit: group `kura.weebo.io`, version
//! `v1alpha1`, camelCase, unknown fields refused, defaults on the closed side.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use weebo_kit_api::ObjectRef;

use crate::names::DnsLabel;
use crate::quantity::{Duration, Quantity};
use crate::role::VaultRole;

pub const API_VERSION: &str = "kura.weebo.io/v1alpha1";
pub const KIND_TENANT: &str = "KuraTenant";
pub const KIND_VAULT: &str = "KuraVault";
/// The kit's annotations, under Kura's domain (`kura.weebo.io/force-orphan`).
pub const ANNOTATIONS: weebo_kit_api::Annotations =
    weebo_kit_api::Annotations::new("kura.weebo.io");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Metadata {
    pub name: DnsLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<DnsLabel>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

impl Metadata {
    /// `kura.weebo.io/force-orphan: "true"`: released without any cleanup.
    pub fn force_orphan(&self) -> bool {
        self.annotations
            .get(&ANNOTATIONS.key(weebo_kit_api::annotations::FORCE_ORPHAN))
            .is_some_and(|v| v == "true")
    }
}

/// One resource document: `apiVersion` and `kind` are checked by the parser
/// and not stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document<S> {
    pub metadata: Metadata,
    pub spec: S,
}

pub type KuraTenant = Document<TenantSpec>;
pub type KuraVault = Document<VaultSpec>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeletionPolicy {
    /// Removing the document disables the object and keeps its data.
    #[default]
    Orphan,
    /// Removing the document disables the object and deletes it after 30 days.
    Delete,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantSpec {
    /// Absent or empty: the tenant has no members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_mappings: Option<ClaimMappings>,
    /// Absent: no namespace may declare a vault for the tenant.
    #[serde(default)]
    pub vault_namespaces: Vec<DnsLabel>,
    #[serde(default)]
    pub retention: Retention,
    #[serde(default)]
    pub quota: Quota,
    #[serde(default)]
    pub search: Search,
    #[serde(default)]
    pub deletion_policy: DeletionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimMappings {
    pub claim: String,
    #[serde(default)]
    pub roles: TenantRoles,
}

/// Claim values granting each tenant role. An unknown role is refused.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantRoles {
    #[serde(default)]
    pub owner: Vec<String>,
    #[serde(default)]
    pub admin: Vec<String>,
    #[serde(default)]
    pub member: Vec<String>,
}

/// Pruning policy, applied in phase 5. Absent fields keep every revision.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retention {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_revision_age: Option<Duration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_revisions_per_entry: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_entry_ttl: Option<Duration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quota {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<Quantity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Search {
    /// Absent: false. The index is plaintext, so indexing is opt-in.
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultSpec {
    pub tenant_ref: ObjectRef,
    /// Absent: the value stored when the vault was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunking: Option<Chunking>,
    /// Absent: the `.obsidian` folder does not sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_sync: Option<ConfigSync>,
    /// Absent: only tenant `owner` and `admin` can open the vault.
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub deletion_policy: DeletionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Chunking {
    pub min: Quantity,
    pub avg: Quantity,
    pub max: Quantity,
}

impl Chunking {
    /// Largest chunk a mobile client can send in one body (§4.3, §6.8).
    pub const MAX_CHUNK: Quantity = Quantity::from_bytes(4 << 20);
    /// The built-in default, applied when a vault is created without one.
    pub const DEFAULT: Self = Self {
        min: Quantity::from_bytes(256 << 10),
        avg: Quantity::from_bytes(1 << 20),
        max: Quantity::from_bytes(4 << 20),
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigSync {
    #[serde(default)]
    pub categories: Vec<ConfigCategory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConfigCategory {
    Plugins,
    Snippets,
    Themes,
    Hotkeys,
}

/// A vault role bound to a claim value or to one OIDC subject, exactly one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub role: VaultRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
}

/// A vault's identity: its namespace and name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VaultKey {
    pub namespace: DnsLabel,
    pub name: DnsLabel,
}

impl std::fmt::Display for VaultKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}
