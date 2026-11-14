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
        Ok(Self {
            registry,
            ready,
            config_rejected_total,
            config_warnings,
            authz_denied_total,
            chunk_corrupt_total,
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
