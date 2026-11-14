//! A JSON body whose rejection is a problem document, not axum's plain text.

use axum::extract::rejection::{BytesRejection, FailedToBufferBody, JsonRejection};
use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use crate::problem::{ApiError, ProblemType};

pub struct Body<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for Body<T> {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(Self(v)),
            // Over `MAX_BODY_BYTES` (axum answers 400 there; §4.4 says 413).
            Err(JsonRejection::BytesRejection(BytesRejection::FailedToBufferBody(
                FailedToBufferBody::LengthLimitError(_),
            ))) => Err(ApiError::of(ProblemType::PayloadTooLarge)),
            Err(e) => Err(ApiError::with_detail(
                ProblemType::InvalidRequest,
                e.body_text(),
            )),
        }
    }
}

/// A raw body, bounded by the route's `DefaultBodyLimit`; over it is `413`.
pub struct Raw(pub bytes::Bytes);

impl<S: Send + Sync> FromRequest<S> for Raw {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match bytes::Bytes::from_request(req, state).await {
            Ok(b) => Ok(Self(b)),
            Err(BytesRejection::FailedToBufferBody(FailedToBufferBody::LengthLimitError(_))) => {
                Err(ApiError::of(ProblemType::PayloadTooLarge))
            }
            Err(e) => Err(ApiError::with_detail(
                ProblemType::InvalidRequest,
                e.body_text(),
            )),
        }
    }
}
