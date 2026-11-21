//! Resource documents of weebo-kura (`KuraTenant`, `KuraVault`), their
//! defaults and `validate()`, shared by the server's file loader and the
//! future operator (RFC 0001 §4.1, decision 49).

pub mod names;
pub mod parse;
pub mod quantity;
pub mod reason;
pub mod role;
pub mod spec;
#[cfg(feature = "crd")]
pub mod status;
pub mod validate;

pub use names::{DnsLabel, TenantSlug};
pub use parse::{ConfigError, Resource, Source, parse_file};
pub use quantity::{Duration, Quantity};
pub use reason::ReasonCode;
pub use role::{TenantRole, VaultRole};
pub use spec::{
    Binding, Chunking, DeletionPolicy, Document, KuraTenant, KuraVault, TenantSpec, VaultKey,
    VaultSpec,
};
/// The custom resources the future operator serves, and kura-crdgen writes.
#[cfg(feature = "crd")]
pub use spec::{KuraTenantResource, KuraVaultResource};
pub use validate::{Config, ManagedBy, PreviousState, StoredVault, validate};
