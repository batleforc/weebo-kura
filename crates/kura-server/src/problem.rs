//! Errors at the HTTP boundary: `application/problem+json` (RFC 9457) with
//! the stable `type`s of RFC 0001 §4.4. Mapped once, here; internal detail
//! goes to the log, never to the response.

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// Every problem `type` of RFC 0001 §4.4, stable across releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemType {
    /// A body or parameter that does not parse (plan D26).
    InvalidRequest,
    InvalidToken,
    DeviceRevoked,
    DeviceUnknown,
    InsufficientRole,
    AddressNotAllowed,
    NotFound,
    SearchDisabled,
    /// RFC 0004 §4.3: a `write` rule in a tenant that did not opt in.
    TokenWriteDisabled,
    CursorExpired,
    HistoryRewound,
    PayloadTooLarge,
    InvalidPath,
    HashMismatch,
    IdempotencyConflict,
    QuotaExceeded,
    ClientTooOld,
    RateLimited,
    Unavailable,
}

impl ProblemType {
    /// The answers that refuse a caller: each one is an audit event (T10).
    pub const fn is_denial(self) -> bool {
        matches!(
            self,
            Self::InvalidToken
                | Self::DeviceRevoked
                | Self::DeviceUnknown
                | Self::InsufficientRole
                | Self::AddressNotAllowed
                | Self::NotFound
        )
    }

    pub const ALL: [Self; 19] = [
        Self::InvalidRequest,
        Self::InvalidToken,
        Self::DeviceRevoked,
        Self::DeviceUnknown,
        Self::InsufficientRole,
        Self::AddressNotAllowed,
        Self::NotFound,
        Self::SearchDisabled,
        Self::TokenWriteDisabled,
        Self::CursorExpired,
        Self::HistoryRewound,
        Self::PayloadTooLarge,
        Self::InvalidPath,
        Self::HashMismatch,
        Self::IdempotencyConflict,
        Self::QuotaExceeded,
        Self::ClientTooOld,
        Self::RateLimited,
        Self::Unavailable,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::InvalidToken => "invalid_token",
            Self::DeviceRevoked => "device_revoked",
            Self::DeviceUnknown => "device_unknown",
            Self::InsufficientRole => "insufficient_role",
            Self::AddressNotAllowed => "address_not_allowed",
            Self::NotFound => "not_found",
            Self::SearchDisabled => "search_disabled",
            Self::TokenWriteDisabled => "token_write_disabled",
            Self::CursorExpired => "cursor_expired",
            Self::HistoryRewound => "history_rewound",
            Self::PayloadTooLarge => "payload_too_large",
            Self::InvalidPath => "invalid_path",
            Self::HashMismatch => "hash_mismatch",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::QuotaExceeded => "quota_exceeded",
            Self::ClientTooOld => "client_too_old",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
        }
    }

    pub const fn status(self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::BAD_REQUEST,
            Self::InvalidToken | Self::DeviceRevoked | Self::DeviceUnknown => {
                StatusCode::UNAUTHORIZED
            }
            Self::InsufficientRole | Self::AddressNotAllowed => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::SearchDisabled | Self::TokenWriteDisabled => StatusCode::CONFLICT,
            Self::CursorExpired | Self::HistoryRewound => StatusCode::GONE,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::InvalidPath
            | Self::HashMismatch
            | Self::IdempotencyConflict
            | Self::QuotaExceeded => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ClientTooOld => StatusCode::UPGRADE_REQUIRED,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

/// The RFC 9457 body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, ToSchema)]
pub struct Problem {
    /// Relative URI under `/v1/problems/`, stable across releases.
    #[serde(rename = "type")]
    pub type_: String,
    pub title: String,
    pub status: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// An error a handler returns. Every variant maps to one problem type.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0:?}")]
    Problem(ProblemType, Option<String>),
    /// Retried later by the client (`Retry-After`, seconds).
    #[error("rate limited, retry after {0} s")]
    RateLimited(u64),
    /// A dependency failed; the detail is logged, never sent.
    #[error("dependency failure: {0}")]
    Unavailable(String),
}

impl ApiError {
    pub fn of(t: ProblemType) -> Self {
        Self::Problem(t, None)
    }

    pub fn with_detail(t: ProblemType, detail: impl Into<String>) -> Self {
        Self::Problem(t, Some(detail.into()))
    }

    pub fn problem_type(&self) -> ProblemType {
        match self {
            Self::Problem(t, _) => *t,
            Self::RateLimited(_) => ProblemType::RateLimited,
            Self::Unavailable(_) => ProblemType::Unavailable,
        }
    }
}

/// Back-off a client is asked to wait after a dependency failure.
const UNAVAILABLE_RETRY_AFTER: u64 = 5;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let t = self.problem_type();
        let (detail, retry_after) = match self {
            Self::Problem(_, detail) => (detail, None),
            Self::RateLimited(secs) => (None, Some(secs)),
            Self::Unavailable(cause) => {
                tracing::warn!(error = %cause, "dependency failure");
                (None, Some(UNAVAILABLE_RETRY_AFTER))
            }
        };
        problem_response(t, detail, retry_after)
    }
}

pub fn problem_response(
    t: ProblemType,
    detail: Option<String>,
    retry_after: Option<u64>,
) -> Response {
    let status = t.status();
    let body = Problem {
        type_: format!("/v1/problems/{}", t.name()),
        title: t.name().replace('_', " "),
        status: status.as_u16(),
        detail,
    };
    let mut response = (status, Json(body)).into_response();
    // Read by the audit middleware, which logs every denial once (§10.1).
    response.extensions_mut().insert(t);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/problem+json"),
    );
    if let Some(secs) = retry_after {
        headers.insert(header::RETRY_AFTER, HeaderValue::from(secs));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn body(r: Response) -> Problem {
        let bytes = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn every_type_renders_its_status_and_uri() {
        for t in ProblemType::ALL {
            let r = ApiError::of(t).into_response();
            assert_eq!(r.status(), t.status());
            assert_eq!(
                r.headers()[header::CONTENT_TYPE],
                "application/problem+json"
            );
            let p = body(r).await;
            assert_eq!(p.type_, format!("/v1/problems/{}", t.name()));
            assert_eq!(p.status, t.status().as_u16());
        }
    }

    #[tokio::test]
    async fn unavailable_and_rate_limited_carry_retry_after_and_no_detail() {
        let r = ApiError::Unavailable("connection refused to 10.0.0.1:5432".into()).into_response();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(r.headers()[header::RETRY_AFTER], "5");
        let p = body(r).await;
        assert_eq!(p.detail, None, "no internal detail leaks");
        let r = ApiError::RateLimited(7).into_response();
        assert_eq!(r.headers()[header::RETRY_AFTER], "7");
        let r =
            ApiError::with_detail(ProblemType::InsufficientRole, "editor required").into_response();
        assert_eq!(body(r).await.detail.as_deref(), Some("editor required"));
    }
}
