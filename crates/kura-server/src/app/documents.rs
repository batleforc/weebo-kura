//! `PUT /v1/admin/documents`: the whole set of `KuraTenant` and `KuraVault`
//! documents, pushed by kura-operator and applied by the reconciler that
//! applies files (RFC 0009 §4.4, §5.2). Checked in this order: the source,
//! the identity, the rate, the body; nothing is parsed for a caller refused.

use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::header;
use axum::http::request::Parts;
use kura_api::ConfigError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use super::SharedState;
use super::admin::AdminStatus;
use super::body::Body;
use crate::auth::{OperatorCaller, OperatorError};
use crate::problem::{ApiError, Problem, ProblemType};
use crate::reconcile::files::ConfigFiles;
use crate::reconcile::{self, Outcome};

/// The body limit of the route (RFC 0009 §7.3).
pub const MAX_BODY_BYTES: usize = 4 << 20;
/// Documents in one push at most.
pub const MAX_DOCUMENTS: usize = 2_000;
const ROUTE: &str = "/v1/admin/documents";

/// The operator, verified: refused with 409 in `files` mode before the token
/// is read, then 401 or 403, then 429 (per subject).
pub struct Operator(pub OperatorCaller);

impl FromRequestParts<SharedState> for Operator {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        let refused = |e: ApiError| {
            state
                .metrics
                .documents_pushes_total
                .with_label_values(&["refused"])
                .inc();
            e
        };
        let Some(verifier) = &state.documents else {
            return Err(refused(ApiError::of(ProblemType::DocumentsSourceFiles)));
        };
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| refused(ApiError::of(ProblemType::InvalidToken)))?;
        let caller = match verifier.verify(token.trim()).await {
            Ok(c) => c,
            Err(OperatorError::Token(e)) => return Err(refused(e.into())),
            Err(OperatorError::OtherSubject) => {
                return Err(refused(ApiError::of(ProblemType::InsufficientRole)));
            }
        };
        state
            .limits
            .check(&caller.subject, "PUT", ROUTE)
            .map_err(|wait| refused(ApiError::RateLimited(wait)))?;
        Ok(Self(caller))
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DocumentsBody {
    /// Every `KuraTenant` and `KuraVault`: `apiVersion`, `kind`, `metadata`
    /// (`name`, `namespace`, `labels`, `annotations`, `generation`) and `spec`.
    #[schema(value_type = Vec<Object>)]
    pub documents: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DocumentsResult {
    /// Whether this push changed the stored configuration.
    pub applied: bool,
    /// `applied`, `unchanged` or `rejected`.
    pub outcome: String,
    /// Why the set was refused, each with the document it names
    /// (`location.file` is `KuraTenant/<name>` or `KuraVault/<namespace>/<name>`).
    #[schema(value_type = Vec<Object>)]
    pub errors: Vec<ConfigError>,
    /// The stored configuration after this push: the last valid one when refused.
    #[serde(flatten)]
    pub status: AdminStatus,
}

/// The pushed documents as the reconciler's input: one "file" per document,
/// in JSON (which the YAML parser reads), so every rule applied to files
/// applies here, `deny_unknown_fields` included.
pub fn config_files(documents: &[Value]) -> Result<ConfigFiles, ApiError> {
    if documents.len() > MAX_DOCUMENTS {
        return Err(ApiError::with_detail(
            ProblemType::PayloadTooLarge,
            format!("at most {MAX_DOCUMENTS} documents"),
        ));
    }
    let mut files = Vec::with_capacity(documents.len());
    for (i, doc) in documents.iter().enumerate() {
        let name = kura_api::pushed_name(doc).map_err(|e| {
            ApiError::with_detail(ProblemType::InvalidRequest, format!("documents[{i}]: {e}"))
        })?;
        let content = serde_json::to_string(doc)
            .map_err(|e| ApiError::with_detail(ProblemType::InvalidRequest, e.to_string()))?;
        files.push((name, content));
    }
    Ok(ConfigFiles::new(files))
}

/// Replace the whole set of documents.
#[utoipa::path(
    put,
    path = "/admin/documents",
    tag = "admin",
    request_body = DocumentsBody,
    responses(
        (status = 200, description = "Applied, unchanged or refused as a whole; the status of every object", body = DocumentsResult),
        (status = 400, description = "A document that names no kind or no valid name", body = Problem, content_type = "application/problem+json"),
        (status = 401, description = "Not a token of the cluster's issuer for this audience", body = Problem, content_type = "application/problem+json"),
        (status = 403, description = "The token of another workload than the configured operator", body = Problem, content_type = "application/problem+json"),
        (status = 409, description = "documents.source is files", body = Problem, content_type = "application/problem+json"),
        (status = 413, description = "Over 4 MiB or 2,000 documents", body = Problem, content_type = "application/problem+json"),
        (status = 429, description = "Over 30 a minute", body = Problem, content_type = "application/problem+json"),
    ),
    security(("bearer_token" = [])),
)]
pub async fn put_documents(
    State(state): State<SharedState>,
    Operator(caller): Operator,
    Body(body): Body<DocumentsBody>,
) -> Result<Json<DocumentsResult>, ApiError> {
    let files = config_files(&body.documents)?;
    let generation: String = files
        .generation
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    // A push is always this writer's own change (kura_config_begin).
    let outcome = reconcile::reconcile(&state.db, &files, true)
        .await
        .map_err(|e| ApiError::Unavailable(e.to_string()))?;
    let (label, errors) = match outcome {
        Outcome::Applied { removed } => {
            state.set_rejection(None);
            tracing::info!(
                target: "audit",
                event = "documents_pushed",
                subject = %caller.subject,
                generation,
                documents = files.files.len(),
                removed = removed.len(),
                "documents applied"
            );
            reconcile::log_applied(&removed);
            // The answer carries the status just written, not the one this
            // replica's cache still holds.
            let current = state.snapshot().map(|s| s.id);
            match reconcile::snapshot::load_if_changed(&state.db, current).await {
                Ok(Some(snapshot)) => state.set_snapshot(snapshot),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "status not reloaded after a push"),
            }
            ("applied", Vec::new())
        }
        Outcome::NotWritten => {
            state.set_rejection(None);
            ("unchanged", Vec::new())
        }
        Outcome::Rejected(errors) => {
            state.metrics.config_rejected_total.inc();
            tracing::warn!(
                target: "audit",
                event = "documents_rejected",
                subject = %caller.subject,
                generation,
                errors = errors.len(),
                "documents rejected, the last valid configuration stays"
            );
            state.set_rejection(Some(errors.clone()));
            ("rejected", errors)
        }
    };
    state
        .metrics
        .documents_pushes_total
        .with_label_values(&[label])
        .inc();
    Ok(Json(DocumentsResult {
        applied: label == "applied",
        outcome: label.into(),
        errors,
        status: super::admin::current(&state).await?,
    }))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn documents_are_named_by_kind_namespace_and_name() {
        let tenant = json!({ "apiVersion": "kura.weebo.io/v1alpha1", "kind": "KuraTenant", "metadata": { "name": "maxime" }, "spec": {} });
        let vault = json!({ "kind": "KuraVault", "metadata": { "name": "notes", "namespace": "maxime" }, "spec": {} });
        assert_eq!(kura_api::pushed_name(&tenant).unwrap(), "KuraTenant/maxime");
        assert_eq!(
            kura_api::pushed_name(&vault).unwrap(),
            "KuraVault/maxime/notes"
        );
        for bad in [
            json!({ "kind": "KuraVault", "metadata": { "name": "notes" } }),
            json!({ "kind": "ConfigMap", "metadata": { "name": "x" } }),
            json!({ "kind": "KuraTenant", "metadata": { "name": "a/b\nc" } }),
            json!({ "kind": "KuraTenant" }),
            json!("KuraTenant"),
        ] {
            assert!(kura_api::pushed_name(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_same_set_in_another_order_is_the_same_generation() {
        let a = json!({ "kind": "KuraTenant", "metadata": { "name": "a" }, "spec": {} });
        let b = json!({ "kind": "KuraTenant", "metadata": { "name": "b" }, "spec": {} });
        let one = config_files(&[a.clone(), b.clone()]).unwrap();
        let two = config_files(&[b, a]).unwrap();
        assert_eq!(one, two);
        let many = vec![json!({}); MAX_DOCUMENTS + 1];
        assert_eq!(
            config_files(&many).unwrap_err().problem_type(),
            ProblemType::PayloadTooLarge
        );
    }
}
