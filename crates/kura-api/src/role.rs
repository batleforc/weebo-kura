//! Roles at tenant and vault level (RFC 0001 §6.2).

use serde::{Deserialize, Serialize};

/// A role granted on a whole tenant, mapped from an OIDC claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TenantRole {
    Member,
    Admin,
    Owner,
}

/// A role granted on one vault, by a binding. Ordered: a higher role
/// includes every right of a lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "crd", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum VaultRole {
    Viewer,
    Editor,
    Manager,
}
