//! The Kura gateway (RFC 0009 §6.3): `PUT /v1/admin/documents` with the
//! operator's projected ServiceAccount token, re-read on every push.

pub mod http_gateway;

pub use http_gateway::{HttpGatewayFactory, HttpKuraGateway};
