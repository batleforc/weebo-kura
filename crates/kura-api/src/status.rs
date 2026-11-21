//! The status of `KuraTenant` and `KuraVault` as custom resources: the kit's
//! shape, a `Ready` condition carrying one of [`crate::ReasonCode`]. The
//! server reports the same facts for documents read from files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use weebo_kit_api::{BoundInstance, Condition};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// The `metadata.generation` the last reconcile applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,
    /// The `KuraInstance` the operator pushed this object to (RFC 0009 §6.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<BoundInstance>,
    /// `Ready`, with one of Kura's reason codes.
    #[serde(default)]
    pub conditions: Vec<Condition>,
}
