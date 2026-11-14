//! kura-server: the sync backend of weebo-kura (RFC 0001).

pub mod app;
pub mod auth;
pub mod authz;
pub mod backup;
pub mod config;
pub mod db;
pub mod limits;
pub mod maintenance;
pub mod metrics;
pub mod net;
pub mod notify;
pub mod problem;
pub mod reconcile;
pub mod scalar;
pub mod store;
pub mod vault;

use tokio::net::TcpListener;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// A background task (reconciler, cache refresh) run beside the servers; it
/// must return once the shutdown token is cancelled.
pub type Background = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

/// The background tasks of a running server: the reconciler of
/// `config_dir` and the authorization cache refresh.
pub fn background(
    state: &app::SharedState,
    config_dir: std::path::PathBuf,
    shutdown: &CancellationToken,
) -> Vec<Background> {
    vec![
        Box::pin(reconcile::run(
            state.db.clone(),
            config_dir,
            state.clone(),
            shutdown.clone(),
        )),
        Box::pin(reconcile::snapshot::run(
            state.db.clone(),
            state.clone(),
            shutdown.clone(),
        )),
        Box::pin(app::devices::purge_loop(state.db.clone(), shutdown.clone())),
        Box::pin(metrics::refresh_storage(
            state.db.clone(),
            state.metrics.clone(),
            shutdown.clone(),
        )),
        Box::pin(notify::listen(
            state.db.clone(),
            state.clone(),
            shutdown.clone(),
        )),
    ]
}

/// Serves the API and the metrics on their listeners, with the background
/// tasks, until `shutdown` is cancelled, then drains them. Returns the first
/// error of either server.
pub async fn serve(
    api: TcpListener,
    metrics: TcpListener,
    state: app::SharedState,
    background: Vec<Background>,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    let mut tasks = JoinSet::new();
    for task in background {
        tasks.spawn(async move {
            task.await;
            Ok(())
        });
    }
    let metrics_router = metrics::router(state.metrics.clone());
    let api_router = app::app(state);
    let token = shutdown.clone();
    tasks.spawn(async move {
        axum::serve(
            api,
            api_router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(token.cancelled_owned())
        .await
    });
    let token = shutdown.clone();
    tasks.spawn(async move {
        axum::serve(metrics, metrics_router)
            .with_graceful_shutdown(token.cancelled_owned())
            .await
    });
    let mut result = Ok(());
    while let Some(joined) = tasks.join_next().await {
        let outcome = joined.map_err(std::io::Error::other).and_then(|r| r);
        if let Err(e) = outcome {
            // One server down takes the other with it: the process restarts whole.
            shutdown.cancel();
            if result.is_ok() {
                result = Err(e);
            }
        }
    }
    result
}
