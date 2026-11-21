//! Inbound adapters of kura-operator: the push loop (the watches of the
//! three kinds → one push of the whole set → the statuses) and the
//! admission webhook.

pub mod pusher;
pub mod webhook;

/// Field manager of the status patches, meter and Event reporter name.
pub const FIELD_MANAGER: &str = "kura-operator";
/// Prefix of the kit's metrics: `<prefix>_reconcile_total`, `<prefix>_leader`…
pub const METRIC_PREFIX: &str = "kura_operator";
