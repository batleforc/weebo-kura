//! The use case of kura-operator (RFC 0009 §5.1): push the whole set to the
//! server and say what every resource shows. Ports only; the HTTP client
//! and the Kubernetes side are adapters.

pub mod ports;
pub mod push;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
