//! kura-server binary: argument parsing, logging, signals. Everything else
//! lives in the library.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context as _;
use kura_server::{app, config::ServerConfig, metrics::Metrics};
use tokio_util::sync::CancellationToken;

const USAGE: &str = "usage: kura-server [--config <server.yaml>] | kura-server dump-spec | kura-server migrate | kura-server reset-epochs | kura-server maintain [--config <server.yaml>]";

/// Database URL until phase 2 reads the credentials from Vault KV (plan D15).
const DATABASE_URL_ENV: &str = "KURA_DATABASE_URL";
/// The directory of resource documents (RFC 0001 §4.1).
const CONFIG_DIR_ENV: &str = "KURA_CONFIG_DIR";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["dump-spec"] => dump_spec(),
        ["migrate"] => migrate().await,
        ["reset-epochs"] => reset_epochs().await,
        ["maintain"] => maintain(PathBuf::from("/etc/kura/server.yaml")).await,
        ["maintain", "--config", path] => maintain(PathBuf::from(path)).await,
        [] => run(PathBuf::from("/etc/kura/server.yaml")).await,
        ["--config", path] => run(PathBuf::from(path)).await,
        _ => Err(anyhow::anyhow!(USAGE)),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kura-server: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn dump_spec() -> anyhow::Result<()> {
    println!("{}", app::openapi_spec().to_pretty_json()?);
    Ok(())
}

/// Applies the migrations; run as the migration owner by a job before each rollout (§13).
async fn migrate() -> anyhow::Result<()> {
    // The job runs as the migration owner, whose credentials are not the
    // server's: always from the environment.
    let url = std::env::var(DATABASE_URL_ENV)
        .with_context(|| format!("{DATABASE_URL_ENV} is not set"))?;
    let pool = kura_server::db::connect(&url, 1, None).await?;
    kura_server::db::migrate(&pool).await?;
    println!("migrations applied");
    Ok(())
}

/// The restore runbook's step (decision 32): every vault gets a new epoch,
/// so each device resyncs and pushes back what the restore lost, deleting
/// nothing. Run as the migration owner, like `migrate`.
async fn reset_epochs() -> anyhow::Result<()> {
    let url = std::env::var(DATABASE_URL_ENV)
        .with_context(|| format!("{DATABASE_URL_ENV} is not set"))?;
    let pool = kura_server::db::connect(&url, 1, None).await?;
    kura_server::db::check_schema(&pool).await?;
    let n = kura_server::db::epoch_reset(&pool).await?;
    println!("{n} vault epochs regenerated");
    Ok(())
}

/// Vault, the database and object storage, as `run` and `maintain` share them.
async fn connect(
    config: &ServerConfig,
) -> anyhow::Result<(
    app::AppState,
    Option<std::sync::Arc<kura_server::vault::Vault>>,
    String,
)> {
    // Credentials from Vault KV when `vault` is configured (decision 48);
    // KURA_DATABASE_URL overrides it for local runs.
    let vault = match &config.vault {
        Some(v) => Some(std::sync::Arc::new(
            kura_server::vault::Vault::connect(v).await?,
        )),
        None => None,
    };
    let url = match (std::env::var(DATABASE_URL_ENV).ok(), &vault) {
        (Some(url), _) => url,
        (None, Some(v)) => v.database().await?.url,
        (None, None) => {
            anyhow::bail!("set {DATABASE_URL_ENV}, or configure `vault` in server.yaml")
        }
    };
    let metrics = Metrics::new()?;
    let pool =
        kura_server::db::connect(&url, 16, Some(metrics.vault_epoch_changes_total.clone())).await?;
    kura_server::db::check_schema(&pool).await?;
    let keys = kura_server::auth::Keys::new(
        kura_server::auth::KeySource::http(
            config
                .oidc
                .discovery
                .as_deref()
                .unwrap_or(&config.oidc.issuer),
        )?
        .expecting(&config.oidc.issuer),
        Some(pool.clone()),
    );
    let verifier = kura_server::auth::Verifier::new(&config.oidc, keys);
    let http = app::HttpSettings {
        allowed_origins: config.cors.allowed_origins.clone(),
        trusted_proxies: config.trusted_proxies.clone(),
    };
    let mut state = app::AppState::new(metrics, pool, verifier).with_http(http);
    if let Some(v) = &vault {
        // Object storage and data keys need Vault: the bucket secret and Transit (§6.6).
        let objects = kura_server::store::s3(&v.bucket().await?)?;
        let keys = kura_server::store::keys::DataKeys::new(v.clone());
        state = state.with_storage(std::sync::Arc::new(kura_server::store::Storage::new(
            objects, keys,
        )));
    }
    Ok((state, vault, url))
}

/// One maintenance pass now, outside the window (§6.9): retention, garbage
/// collection, the `Delete` purge. Waits for none: another pass running, or
/// the backup lock, skip it.
async fn maintain(config_path: PathBuf) -> anyhow::Result<()> {
    let config = ServerConfig::load(&config_path)?;
    let (state, _, _) = connect(&config).await?;
    let shutdown = CancellationToken::new();
    tokio::spawn(stop_on_signal(shutdown.clone()));
    match kura_server::maintenance::pass(
        &state,
        kura_server::maintenance::Settings::default(),
        &shutdown,
    )
    .await?
    {
        Some(r) => println!("{r:?}"),
        None => println!("another maintenance pass is running"),
    }
    Ok(())
}

async fn run(config_path: PathBuf) -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = ServerConfig::load(&config_path)?;
    let api = tokio::net::TcpListener::bind(config.listen.api)
        .await
        .context("binding the API port")?;
    let metrics_listener = tokio::net::TcpListener::bind(config.listen.metrics)
        .await
        .context("binding the metrics port")?;
    let (state, vault, url) = connect(&config).await?;
    let config_dir = std::env::var_os(CONFIG_DIR_ENV)
        .map(PathBuf::from)
        .with_context(|| format!("{CONFIG_DIR_ENV} is not set"))?;
    let persisted = state.verifier.keys.load_persisted().await?;
    tracing::info!(keys = persisted, "persisted signing keys loaded");
    let state = std::sync::Arc::new(state);
    let shutdown = CancellationToken::new();
    tokio::spawn(stop_on_signal(shutdown.clone()));
    let mut background = kura_server::background(&state, config_dir, &shutdown);
    background.push(Box::pin(kura_server::maintenance::run(
        state.clone(),
        config.maintenance.window,
        shutdown.clone(),
    )));
    if let Some(v) = vault {
        let every = std::time::Duration::from_secs(300);
        background.push(Box::pin(kura_server::vault::refresh_credentials(
            v,
            state.db.clone(),
            state.storage.clone(),
            url,
            every,
            shutdown.clone(),
        )));
    }
    tracing::info!(api = %config.listen.api, metrics = %config.listen.metrics, "kura-server started");
    kura_server::serve(api, metrics_listener, state, background, shutdown).await?;
    tracing::info!("kura-server stopped");
    Ok(())
}

async fn stop_on_signal(shutdown: CancellationToken) {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "SIGTERM handler unavailable, only Ctrl-C stops the server");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        r = ctrl_c => if let Err(e) = r { tracing::warn!(error = %e, "Ctrl-C handler failed") },
        () = term => {}
    }
    tracing::info!("shutdown requested");
    shutdown.cancel();
}
