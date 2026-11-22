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
    /// Kubernetes' `metadata.generation`, sent by kura-operator and reported
    /// back as `observedGeneration` (RFC 0009 §4.2); files leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<i64>,
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

/// What removing the document does to the object it declared.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
pub enum DeletionPolicy {
    /// Removing the document disables the object and keeps its data.
    #[default]
    Orphan,
    /// Removing the document disables the object and deletes it after 30 days.
    Delete,
}

/// A tenant: who belongs to it, through which OIDC claim, and the limits its
/// vaults share. Cluster-scoped; its `metadata.name` is the tenant's slug,
/// used in API paths.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "crd",
    derive(kube::CustomResource),
    kube(
        group = "kura.weebo.io",
        version = "v1alpha1",
        kind = "KuraTenant",
        plural = "kuratenants",
        root = "KuraTenantResource",
        doc = "A Kura tenant: its members, its quota and the namespaces its vaults live in.",
        status = "crate::status::Status",
        printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].status"}"#,
        printcolumn = r#"{"name":"Reason","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].reason"}"#
    )
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantSpec {
    /// The OIDC claim whose values grant the tenant roles. Absent or empty:
    /// the tenant has no members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_mappings: Option<ClaimMappings>,
    /// The namespaces allowed to declare a `KuraVault` for this tenant.
    /// Absent: no namespace may.
    #[serde(default)]
    pub vault_namespaces: Vec<DnsLabel>,
    /// How long history is kept, applied by the nightly maintenance pass.
    /// Absent: every revision is kept, and deleted entries are purged after 90d.
    #[serde(default)]
    pub retention: Retention,
    /// Storage limits shared by the tenant's vaults. Absent fields: no limit.
    #[serde(default)]
    pub quota: Quota,
    /// Full-text search over the tenant's notes.
    #[serde(default)]
    pub search: Search,
    /// What the tenant's scoped tokens may do beyond reading.
    #[serde(default)]
    pub scoped_tokens: ScopedTokens,
    /// What removing this document does: `Orphan` (default) disables the
    /// tenant and keeps its data; `Delete` also deletes it after 30 days,
    /// cancelled if the document comes back.
    #[serde(default)]
    pub deletion_policy: DeletionPolicy,
}

/// The claim read from the caller's token, and the values granting each role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaimMappings {
    /// The token claim to read, such as `groups`: a string or a list of strings.
    pub claim: String,
    /// The claim values granting each tenant role.
    #[serde(default)]
    pub roles: TenantRoles,
}

/// Claim values granting each tenant role. An unknown role is refused.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TenantRoles {
    /// Values granting `owner`: every right on the tenant and its vaults.
    #[serde(default)]
    pub owner: Vec<String>,
    /// Values granting `admin`: create and delete vaults, manage bindings,
    /// scoped tokens and notification rules.
    #[serde(default)]
    pub admin: Vec<String>,
    /// Values granting `member`: the vaults a binding opens to them.
    #[serde(default)]
    pub member: Vec<String>,
}

/// Pruning policy, applied by the maintenance pass. Absent fields keep every
/// revision; deleted entries are purged after 90d unless `deletedEntryTtl` says otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retention {
    /// Revisions older than this are pruned, an entry's head never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_revision_age: Option<Duration>,
    /// At most this many revisions are kept per entry, the newest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_revisions_per_entry: Option<u32>,
    /// A deleted entry stays restorable this long, then is purged. Absent: 90d.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_entry_ttl: Option<Duration>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quota {
    /// Encrypted bytes the tenant may store, all vaults together. An upload
    /// past it is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<Quantity>,
    /// The largest file a device may sync; larger ones stay local and are
    /// reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<Quantity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Search {
    /// Absent: false. The index is plaintext, so indexing is opt-in.
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopedTokens {
    /// Absent: false. Tokens of the tenant may carry `write` rules
    /// (RFC 0004 §4.1); set back to false, their writes stop at once.
    #[serde(default)]
    pub write: bool,
}

/// A vault: the tenant it belongs to, how its files are cut into chunks,
/// what of `.obsidian` syncs, and who may open it. Namespaced: the namespace
/// must be one of its tenant's `vaultNamespaces`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[cfg_attr(
    feature = "crd",
    derive(kube::CustomResource),
    kube(
        group = "kura.weebo.io",
        version = "v1alpha1",
        kind = "KuraVault",
        plural = "kuravaults",
        namespaced,
        root = "KuraVaultResource",
        doc = "A Kura vault: an Obsidian vault synced through Kura, and who may open it.",
        status = "crate::status::Status",
        printcolumn = r#"{"name":"Tenant","type":"string","jsonPath":".spec.tenantRef.name"}"#,
        printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].status"}"#,
        printcolumn = r#"{"name":"Reason","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].reason"}"#
    )
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VaultSpec {
    /// The `KuraTenant` this vault belongs to.
    #[cfg_attr(
        feature = "crd",
        schemars(extend("x-weebo-link" = { "to": [{ "group": "kura.weebo.io", "kind": "KuraTenant" }] }))
    )]
    pub tenant_ref: ObjectRef,
    /// Content-defined chunking sizes. Absent: the value stored when the
    /// vault was created (256Ki, 1Mi, 4Mi by default); fixed once set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chunking: Option<Chunking>,
    /// Which parts of the `.obsidian` folder sync, per user and per device
    /// profile, never between members. Absent: the folder does not sync.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_sync: Option<ConfigSync>,
    /// Vault roles granted to claim values or subjects. Absent: only tenant
    /// `owner` and `admin` can open the vault.
    #[serde(default)]
    pub bindings: Vec<Binding>,
    /// What removing this document does: `Orphan` (default) disables the
    /// vault and keeps its data; `Delete` also deletes it after 30 days,
    /// cancelled if the document comes back.
    #[serde(default)]
    pub deletion_policy: DeletionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Chunking {
    /// Smallest chunk, except a file's last.
    pub min: Quantity,
    /// Average chunk the cut points aim at.
    pub avg: Quantity,
    /// Largest chunk, at most 4Mi (a mobile client's body).
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
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigSync {
    /// The parts of `.obsidian` that sync: `plugins`, `snippets`, `themes`,
    /// `hotkeys`.
    #[serde(default)]
    pub categories: Vec<ConfigCategory>,
}

/// A part of the `.obsidian` folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum ConfigCategory {
    Plugins,
    Snippets,
    Themes,
    Hotkeys,
}

/// A vault role bound to a claim value or to one OIDC subject, exactly one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
// The rules `validate()` applies to files, as CEL for the apiserver.
#[cfg_attr(
    feature = "crd",
    schemars(extend("x-kubernetes-validations" = [
        { "rule": "has(self.claimValue) != has(self.subject)", "message": "a binding names exactly one of claimValue and subject" },
        { "rule": "!has(self.paths) || self.role == 'viewer'", "message": "paths restrict a viewer only" }
    ]))
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    /// The vault role granted: `viewer` reads, `editor` also writes,
    /// `manager` also manages bindings and scoped tokens.
    pub role: VaultRole,
    /// A value of the tenant's mapped claim, such as a group name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_value: Option<String>,
    /// One OIDC subject (`sub`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    /// `viewer` only: 1 to 16 folders, each ending with `/`, that the holder
    /// reads; absent: the whole vault (RFC 0005 §4.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
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
