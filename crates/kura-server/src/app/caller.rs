//! The authenticated caller, extracted from `Authorization: Bearer`.

use axum::extract::FromRequestParts;
use axum::http::header;
use axum::http::request::Parts;

use super::SharedState;
use crate::auth::{BearerToken, Principal};
use crate::problem::{ApiError, ProblemType};

pub struct Caller {
    pub principal: Principal,
    pub token: BearerToken,
}

/// Every claim the stored configuration reads: tenant claim mappings and the
/// platform admin claim. Missing ones are asked of userinfo (§6.2).
fn needed_claims(state: &SharedState) -> Vec<String> {
    let mut claims = vec![state.verifier.platform_admin.claim.clone()];
    if let Some(snapshot) = state.snapshot() {
        for t in snapshot.config.tenants.values() {
            if let Some(m) = &t.doc.spec.claim_mappings {
                claims.push(m.claim.clone());
            }
        }
    }
    claims.sort();
    claims.dedup();
    claims
}

impl FromRequestParts<SharedState> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or(ApiError::of(ProblemType::InvalidToken))?;
        let token = BearerToken::new(token.trim().to_owned());
        let principal = state.verifier.verify_access_token(token.expose()).await?;
        let needed = needed_claims(state);
        let needed: Vec<&str> = needed.iter().map(String::as_str).collect();
        let principal = state
            .verifier
            .with_claims(principal, &token, &needed)
            .await?;
        Ok(Self { principal, token })
    }
}
