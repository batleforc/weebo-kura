//! What kura-operator decides without I/O (RFC 0009 §6.3): the set of
//! documents built from the resources, and what each resource shows from
//! the server's answer. No rule of Kura's lives here: the server judges.

pub mod documents;
pub mod verdict;

pub use kura_api::ReasonCode;

/// A `Ready: False` cause, carrying one of Kura's reason codes.
pub type Failure = weebo_kit_domain::Failure<ReasonCode>;
/// A non-blocking condition shown next to `Ready: True`.
pub type Advisory = weebo_kit_domain::Advisory<ReasonCode>;
