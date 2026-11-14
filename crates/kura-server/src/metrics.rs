//! Prometheus metrics, served on their own port (RFC 0001 §4.4, §10.1).
//! Labels are limited to `tenant`, `vault` and `outcome`.

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use prometheus::{Encoder, IntCounter, IntCounterVec, IntGauge, Opts, Registry, TextEncoder};

#[derive(Clone)]
pub struct Metrics {
    pub registry: Registry,
    /// 1 while the configuration is loaded and the server serves traffic.
    pub ready: IntGauge,
    /// Configuration changes rejected by validation (§4.3).
    pub config_rejected_total: IntCounter,
    /// Objects of the stored configuration whose `Ready` is not `True`.
    pub config_warnings: IntGauge,
    /// Authorization denials, by `outcome` (§10.1).
    pub authz_denied_total: IntCounterVec,
    /// Stored chunks that failed their check on read (§10.1).
    pub chunk_corrupt_total: IntCounter,
    /// Commits that wrote at least one revision (§10.1, §6.P observation 1).
    pub commits_total: IntCounter,
    /// Three-way merges attempted, by `outcome`: `merged` or `conflict` (§10.1).
    pub merge_total: IntCounterVec,
    /// Vault epochs regenerated after a promotion or a restore (§10.1 alert).
    pub vault_epoch_changes_total: IntCounter,
    /// Rows and objects removed by maintenance, by `outcome`: `revision`,
    /// `entry`, `blob`, `chunk`, `object`, `orphan`, `purged` (§6.9).
    pub maintenance_deleted_total: IntCounterVec,
    /// Tenants purged by `Delete` whose Transit key awaits an operator (§10.1).
    pub tenants_awaiting_key_destruction: IntGauge,
    /// Vaults whose maintenance failed in a pass (§6.9).
    pub maintenance_failures_total: IntCounter,
}

impl Metrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new_custom(Some("kura".into()), None)?;
        let ready = IntGauge::new("ready", "1 while the configuration is loaded")?;
        let config_rejected_total =
            IntCounter::new("config_rejected_total", "Rejected configuration changes")?;
        let config_warnings = IntGauge::new("config_warnings", "Configuration objects not Ready")?;
        registry.register(Box::new(ready.clone()))?;
        registry.register(Box::new(config_rejected_total.clone()))?;
        registry.register(Box::new(config_warnings.clone()))?;
        let authz_denied_total = IntCounterVec::new(
            Opts::new("authz_denied_total", "Authorization denials"),
            &["outcome"],
        )?;
        registry.register(Box::new(authz_denied_total.clone()))?;
        let chunk_corrupt_total =
            IntCounter::new("chunk_corrupt_total", "Stored chunks failing their check")?;
        registry.register(Box::new(chunk_corrupt_total.clone()))?;
        let commits_total = IntCounter::new("commits_total", "Commits that wrote revisions")?;
        registry.register(Box::new(commits_total.clone()))?;
        let merge_total = IntCounterVec::new(
            Opts::new("merge_total", "Three-way merges attempted"),
            &["outcome"],
        )?;
        registry.register(Box::new(merge_total.clone()))?;
        let vault_epoch_changes_total = IntCounter::new(
            "vault_epoch_changes_total",
            "Vault epochs regenerated after a promotion or a restore",
        )?;
        registry.register(Box::new(vault_epoch_changes_total.clone()))?;
        let maintenance_deleted_total = IntCounterVec::new(
            Opts::new(
                "maintenance_deleted_total",
                "Rows and objects removed by maintenance",
            ),
            &["outcome"],
        )?;
        registry.register(Box::new(maintenance_deleted_total.clone()))?;
        let tenants_awaiting_key_destruction = IntGauge::new(
            "tenants_awaiting_key_destruction",
            "Tenants purged by Delete whose Transit key awaits an operator",
        )?;
        registry.register(Box::new(tenants_awaiting_key_destruction.clone()))?;
        let maintenance_failures_total = IntCounter::new(
            "maintenance_failures_total",
            "Vaults whose maintenance failed in a pass",
        )?;
        registry.register(Box::new(maintenance_failures_total.clone()))?;
        Ok(Self {
            registry,
            ready,
            config_rejected_total,
            config_warnings,
            authz_denied_total,
            chunk_corrupt_total,
            commits_total,
            merge_total,
            vault_epoch_changes_total,
            maintenance_deleted_total,
            tenants_awaiting_key_destruction,
            maintenance_failures_total,
        })
    }
}

async fn render(State(metrics): State<Metrics>) -> impl IntoResponse {
    let mut buf = Vec::new();
    match TextEncoder::new().encode(&metrics.registry.gather(), &mut buf) {
        Ok(()) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, prometheus::TEXT_FORMAT)],
            buf,
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "metrics encoding failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub fn router(metrics: Metrics) -> Router {
    Router::new()
        .route("/metrics", get(render))
        .with_state(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn metrics_are_exported_with_the_kura_prefix() {
        let m = Metrics::new().unwrap();
        m.ready.set(1);
        let r = router(m)
            .oneshot(
                axum::http::Request::get("/metrics")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let text = axum::body::to_bytes(r.into_body(), 1 << 16).await.unwrap();
        assert!(String::from_utf8_lossy(&text).contains("kura_ready 1"));
    }
}
