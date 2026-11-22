//! What the use case needs from the outside: a gateway to the Kura server
//! a `KuraInstance` names, built per push so a rotated token or an edited
//! URL takes effect on the next one.

use std::sync::Arc;

use kura_api::{KuraInstance, ReasonCode};
use kura_operator_domain::Failure;
use kura_operator_domain::verdict::PushResult;
use serde_json::Value;

/// A failed push.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GatewayError {
    /// No answer: network error, timeout, 408, 429, 5xx.
    #[error("kura-server unreachable: {0}")]
    Unreachable(String),
    /// 401 or 403: the server does not take the operator's identity.
    #[error("kura-server refused the operator's identity: {0}")]
    Unauthorized(String),
    /// 409: the server reads its documents from files.
    #[error(
        "kura-server reads its documents from files (documents.source: files); set documents.source: operator to hand them to the operator"
    )]
    SourceIsFiles,
    /// Any other answer, such as a 400 naming a document the server cannot name.
    #[error("kura-server refused the push: {0}")]
    Refused(String),
}

impl GatewayError {
    pub fn failure(&self) -> Failure {
        let reason = match self {
            Self::Unreachable(_) => ReasonCode::InstanceUnreachable,
            Self::Unauthorized(_) => ReasonCode::InstanceUnauthorized,
            Self::SourceIsFiles => ReasonCode::SourceIsFiles,
            Self::Refused(_) => ReasonCode::SetRejected,
        };
        Failure::new(reason, self.to_string())
    }
}

/// One Kura server.
#[async_trait::async_trait]
pub trait KuraGateway: Send + Sync {
    /// `PUT /v1/admin/documents` with every document.
    async fn push(&self, documents: &[Value]) -> Result<PushResult, GatewayError>;
}

/// The gateway of an instance; `Err` when its spec cannot give one (an
/// unusable URL, a CA that cannot be read).
#[async_trait::async_trait]
pub trait GatewayFactory: Send + Sync {
    async fn gateway_for(&self, instance: &KuraInstance) -> Result<Arc<dyn KuraGateway>, Failure>;
}
