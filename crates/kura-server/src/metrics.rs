//! Prometheus metrics, served on their own port (RFC 0001 §4.4, §10.1).
//! Labels are limited to `tenant`, `vault` and `outcome`.

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use prometheus::{
    Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec,
    IntGauge, Opts, Registry, TextEncoder,
};

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
    /// Writes asked by scoped tokens, by `outcome`: `applied`, `refused`
    /// (folder or possession), `disabled` (tenant switch) (RFC 0004 §10.1).
    pub token_writes_total: IntCounterVec,
    /// Three-way merges attempted, by `outcome`: `merged` or `conflict` (§10.1).
    pub merge_total: IntCounterVec,
    /// Items of the batch routes (RFC 0002), by `route` (`blobs`, `chunks`)
    /// and `outcome`: `found` (fetched and sent) or `missing` (nothing fetched).
    pub batch_items_total: IntCounterVec,
    /// Body size of each `chunks/batch` answer (RFC 0002 §10.1).
    pub batch_response_bytes: Histogram,
    /// Vault epochs regenerated after a promotion or a restore (§10.1 alert).
    pub vault_epoch_changes_total: IntCounter,
    /// Rows and objects removed by maintenance, by `outcome`: `revision`,
    /// `entry`, `blob`, `chunk`, `object`, `orphan`, `purged` (§6.9).
    pub maintenance_deleted_total: IntCounterVec,
    /// Tenants purged by `Delete` whose Transit key awaits an operator (§10.1).
    pub tenants_awaiting_key_destruction: IntGauge,
    /// Vaults whose maintenance failed in a pass (§6.9).
    pub maintenance_failures_total: IntCounter,
    /// Commit handling time, lock wait and merges included (§10.3 budget).
    pub commit_duration_seconds: Histogram,
    /// Round trip of this replica's own heartbeat through `NOTIFY` (§10.1).
    pub notify_lag_seconds: Gauge,
    /// `pg_notification_queue_usage()`, from 0 to 1 (§10.2).
    pub notify_queue_usage: Gauge,
    /// Open notification sockets on this replica.
    pub ws_connections: IntGauge,
    /// Vault (Transit) calls, by `outcome`: `ok` or `error`.
    pub vault_call_duration_seconds: HistogramVec,
    /// `410` answers sent, by `outcome`: `cursor_expired`, `history_rewound`.
    pub resync_total: IntCounterVec,
    /// Encrypted bytes stored, by `tenant`.
    pub chunk_bytes_stored: GaugeVec,
    /// Bytes of current files over bytes stored, by `tenant`.
    pub dedup_ratio: GaugeVec,
    /// Bytes stored over the quota, by `tenant` (absent without a quota).
    pub quota_used_ratio: GaugeVec,
    /// `503` answers from a failed dependency; the log line names it.
    pub unavailable_total: IntCounter,
    /// Tenants and vaults removed with `Delete`, in their grace period.
    pub deletions_scheduled: IntGauge,
    /// When the last bucket copy finished (0: never), for its alert.
    pub backup_last_success_timestamp_seconds: Gauge,
    /// When the storage gauges were last refreshed, so stale ones alert.
    pub storage_refreshed_timestamp_seconds: Gauge,
    /// Search index jobs waiting (§10.1).
    pub index_queue_depth: IntGauge,
    /// Search index jobs given up after five attempts: those notes are out of
    /// search until their next edit (review 2026-11-15 #9).
    pub index_failed: IntGauge,
    /// When the indexer last finished a round: frozen while it is stuck or
    /// failing, which the queue depth alone cannot show (review 2026-11-15 #20).
    pub index_round_timestamp_seconds: Gauge,
    /// When a maintenance pass last ran garbage collection: frozen while the
    /// backup lock keeps it skipped, or passes fail.
    pub maintenance_gc_timestamp_seconds: Gauge,
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
        let token_writes_total = IntCounterVec::new(
            Opts::new("token_writes_total", "Writes asked by scoped tokens"),
            &["outcome"],
        )?;
        registry.register(Box::new(token_writes_total.clone()))?;
        let merge_total = IntCounterVec::new(
            Opts::new("merge_total", "Three-way merges attempted"),
            &["outcome"],
        )?;
        registry.register(Box::new(merge_total.clone()))?;
        let batch_items_total = IntCounterVec::new(
            Opts::new("batch_items_total", "Items asked of the batch routes"),
            &["route", "outcome"],
        )?;
        registry.register(Box::new(batch_items_total.clone()))?;
        let batch_response_bytes = Histogram::with_opts(
            HistogramOpts::new("batch_response_bytes", "Body size of chunks/batch answers")
                .buckets(vec![
                    4096.0,
                    65536.0,
                    262_144.0,
                    1_048_576.0,
                    2_097_152.0,
                    4_194_304.0,
                    4_500_000.0,
                ]),
        )?;
        registry.register(Box::new(batch_response_bytes.clone()))?;
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
        let commit_duration_seconds = Histogram::with_opts(
            HistogramOpts::new("commit_duration_seconds", "Commit handling time")
                .buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.2, 0.5, 1.0, 2.5, 5.0]),
        )?;
        registry.register(Box::new(commit_duration_seconds.clone()))?;
        let notify_lag_seconds = Gauge::new(
            "notify_lag_seconds",
            "Round trip of this replica's heartbeat through NOTIFY",
        )?;
        registry.register(Box::new(notify_lag_seconds.clone()))?;
        let notify_queue_usage = Gauge::new(
            "notify_queue_usage",
            "pg_notification_queue_usage(), from 0 to 1",
        )?;
        registry.register(Box::new(notify_queue_usage.clone()))?;
        let ws_connections = IntGauge::new("ws_connections", "Open notification sockets")?;
        registry.register(Box::new(ws_connections.clone()))?;
        let vault_call_duration_seconds = HistogramVec::new(
            HistogramOpts::new("vault_call_duration_seconds", "Vault Transit calls"),
            &["outcome"],
        )?;
        registry.register(Box::new(vault_call_duration_seconds.clone()))?;
        let resync_total = IntCounterVec::new(
            Opts::new("resync_total", "410 answers sent, by problem type"),
            &["outcome"],
        )?;
        registry.register(Box::new(resync_total.clone()))?;
        let chunk_bytes_stored = GaugeVec::new(
            Opts::new("chunk_bytes_stored", "Encrypted bytes stored"),
            &["tenant"],
        )?;
        registry.register(Box::new(chunk_bytes_stored.clone()))?;
        let dedup_ratio = GaugeVec::new(
            Opts::new("dedup_ratio", "Bytes of current files over bytes stored"),
            &["tenant"],
        )?;
        registry.register(Box::new(dedup_ratio.clone()))?;
        let quota_used_ratio = GaugeVec::new(
            Opts::new("quota_used_ratio", "Bytes stored over the quota"),
            &["tenant"],
        )?;
        registry.register(Box::new(quota_used_ratio.clone()))?;
        let unavailable_total =
            IntCounter::new("unavailable_total", "503 answers from a failed dependency")?;
        registry.register(Box::new(unavailable_total.clone()))?;
        let deletions_scheduled = IntGauge::new(
            "deletions_scheduled",
            "Tenants and vaults removed with Delete, in their grace period",
        )?;
        registry.register(Box::new(deletions_scheduled.clone()))?;
        let backup_last_success_timestamp_seconds = Gauge::new(
            "backup_last_success_timestamp_seconds",
            "When the last bucket copy finished, unix seconds; 0 if never",
        )?;
        registry.register(Box::new(backup_last_success_timestamp_seconds.clone()))?;
        let storage_refreshed_timestamp_seconds = Gauge::new(
            "storage_refreshed_timestamp_seconds",
            "When the storage gauges were last refreshed, unix seconds",
        )?;
        registry.register(Box::new(storage_refreshed_timestamp_seconds.clone()))?;
        let index_queue_depth = IntGauge::new("index_queue_depth", "Search index jobs waiting")?;
        registry.register(Box::new(index_queue_depth.clone()))?;
        let index_failed = IntGauge::new(
            "index_failed",
            "Search index jobs given up after five attempts",
        )?;
        registry.register(Box::new(index_failed.clone()))?;
        let index_round_timestamp_seconds = Gauge::new(
            "index_round_timestamp_seconds",
            "When the search indexer last finished a round, unix seconds",
        )?;
        registry.register(Box::new(index_round_timestamp_seconds.clone()))?;
        let maintenance_gc_timestamp_seconds = Gauge::new(
            "maintenance_gc_timestamp_seconds",
            "When a maintenance pass last ran garbage collection, unix seconds",
        )?;
        registry.register(Box::new(maintenance_gc_timestamp_seconds.clone()))?;
        Ok(Self {
            registry,
            ready,
            config_rejected_total,
            config_warnings,
            authz_denied_total,
            chunk_corrupt_total,
            commits_total,
            token_writes_total,
            merge_total,
            batch_items_total,
            batch_response_bytes,
            vault_epoch_changes_total,
            maintenance_deleted_total,
            tenants_awaiting_key_destruction,
            maintenance_failures_total,
            commit_duration_seconds,
            notify_lag_seconds,
            notify_queue_usage,
            ws_connections,
            vault_call_duration_seconds,
            resync_total,
            chunk_bytes_stored,
            dedup_ratio,
            quota_used_ratio,
            unavailable_total,
            deletions_scheduled,
            backup_last_success_timestamp_seconds,
            storage_refreshed_timestamp_seconds,
            index_queue_depth,
            index_failed,
            index_round_timestamp_seconds,
            maintenance_gc_timestamp_seconds,
        })
    }
}

/// Refreshes the storage gauges per tenant once a minute, until `shutdown`.
pub async fn refresh_storage(
    pool: sqlx::PgPool,
    metrics: Metrics,
    shutdown: tokio_util::sync::CancellationToken,
) {
    loop {
        if let Err(e) = storage_once(&pool, &metrics).await {
            tracing::warn!(error = %e, "storage metrics not refreshed");
        }
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
        }
    }
}

/// One refresh of `chunk_bytes_stored`, `dedup_ratio`, `quota_used_ratio`,
/// `deletions_scheduled` and `tenants_awaiting_key_destruction`.
pub async fn storage_once(pool: &sqlx::PgPool, m: &Metrics) -> Result<(), sqlx::Error> {
    use sqlx::Row as _;
    let rows = sqlx::query("SELECT slug, stored, logical, quota FROM kura_tenant_storage()")
        .fetch_all(pool)
        .await?;
    let (scheduled, awaiting, backup): (i64, i64, Option<time::OffsetDateTime>) =
        sqlx::query_as(
            "SELECT kura_deletions_scheduled(), kura_tenants_awaiting_key_destruction(), kura_backup_last()",
        )
        .fetch_one(pool)
        .await?;
    m.deletions_scheduled.set(scheduled);
    m.tenants_awaiting_key_destruction.set(awaiting);
    #[expect(clippy::cast_precision_loss, reason = "unix seconds in a float gauge")]
    m.backup_last_success_timestamp_seconds
        .set(backup.map_or(0.0, |t| t.unix_timestamp() as f64));
    for gauge in [&m.chunk_bytes_stored, &m.dedup_ratio, &m.quota_used_ratio] {
        gauge.reset();
    }
    for r in &rows {
        let slug: String = r.try_get("slug")?;
        #[expect(
            clippy::cast_precision_loss,
            reason = "ratios of byte counts; a float is the metric's type"
        )]
        let (stored, logical, quota) = (
            r.try_get::<i64, _>("stored")? as f64,
            r.try_get::<i64, _>("logical")? as f64,
            r.try_get::<Option<i64>, _>("quota")?.map(|q| q as f64),
        );
        m.chunk_bytes_stored.with_label_values(&[&slug]).set(stored);
        if stored > 0.0 {
            m.dedup_ratio
                .with_label_values(&[&slug])
                .set(logical / stored);
        }
        if let Some(q) = quota.filter(|q| *q > 0.0) {
            m.quota_used_ratio
                .with_label_values(&[&slug])
                .set(stored / q);
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "unix seconds in a float gauge")]
    m.storage_refreshed_timestamp_seconds
        .set(time::OffsetDateTime::now_utc().unix_timestamp() as f64);
    Ok(())
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
