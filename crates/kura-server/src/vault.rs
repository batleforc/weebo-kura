//! HashiCorp Vault (RFC 0001 §6.6, decision 48): the server logs in with
//! Kubernetes auth and reads the database and bucket credentials from KV v2,
//! re-reading them so a rotation needs no restart. Transit arrives in PR 2.3.

use std::path::Path;
use std::time::Duration;

use serde::Deserialize;
use vaultrs::client::{Client as _, VaultClient, VaultClientSettingsBuilder};

use zeroize::Zeroizing;

use crate::config::VaultConfig;

/// Outbound calls to Vault wait at most this long (§6.11).
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// Where Kubernetes mounts the pod's service account token.
pub const SERVICE_ACCOUNT_TOKEN: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
/// Local runs only: a Vault token instead of Kubernetes auth.
pub const TOKEN_ENV: &str = "KURA_VAULT_TOKEN";

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("vault address `{0}` is not an http(s) URL")]
    Address(String),
    #[error("vault: {0}")]
    Client(String),
    #[error("cannot read the service account token: {0}")]
    ServiceAccount(std::io::Error),
    #[error("`{0}` is not a KV v2 path `<mount>/data/<path>`")]
    Path(String),
}

fn decode_b64(s: &str) -> Result<Zeroizing<Vec<u8>>, VaultError> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map(Zeroizing::new)
        .map_err(|_| VaultError::Client("Transit returned invalid base64".into()))
}

fn client_err<E: std::fmt::Display>(e: E) -> VaultError {
    VaultError::Client(e.to_string())
}

/// The database secret: `{"url": "postgres://…"}`.
#[derive(Clone, Deserialize)]
pub struct DatabaseSecret {
    pub url: String,
}

/// The bucket secret.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketSecret {
    pub endpoint: String,
    pub bucket: String,
    #[serde(default = "default_region")]
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

fn default_region() -> String {
    "us-east-1".into()
}

// Secrets: no Display, a redacted Debug (§6.11).
impl std::fmt::Debug for DatabaseSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseSecret(<redacted>)")
    }
}

impl std::fmt::Debug for BucketSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "BucketSecret {{ endpoint: {}, bucket: {}, <credentials redacted> }}",
            self.endpoint, self.bucket
        )
    }
}

/// `mv/data/kura/database` → (`mv`, `kura/database`), as RFC 0001 §4.1 writes it.
pub fn split_kv2(path: &str) -> Result<(&str, &str), VaultError> {
    let bad = || VaultError::Path(path.to_owned());
    let (mount, rest) = path.split_once('/').ok_or_else(bad)?;
    let secret = rest.strip_prefix("data/").ok_or_else(bad)?;
    if mount.is_empty() || secret.is_empty() {
        return Err(bad());
    }
    Ok((mount, secret))
}

pub struct Vault {
    config: VaultConfig,
    /// A fixed token (local runs, tests); `None`: Kubernetes auth.
    token: Option<String>,
    client: tokio::sync::RwLock<VaultClient>,
    /// `kura_vault_call_duration_seconds`, once the server registers it.
    calls: std::sync::OnceLock<prometheus::HistogramVec>,
}

impl Vault {
    /// Connects and logs in: with `KURA_VAULT_TOKEN` when set (local runs),
    /// else with the pod's service account through Kubernetes auth.
    pub async fn connect(config: &VaultConfig) -> Result<Self, VaultError> {
        let token = std::env::var(TOKEN_ENV).ok().filter(|t| !t.is_empty());
        Self::connect_with(config, token).await
    }

    /// [`Vault::connect`] with an explicit token, or `None` for Kubernetes auth.
    pub async fn connect_with(
        config: &VaultConfig,
        token: Option<String>,
    ) -> Result<Self, VaultError> {
        let parsed: Result<axum::http::Uri, _> = config.address.parse();
        if !parsed
            .is_ok_and(|u| matches!(u.scheme_str(), Some("http" | "https")) && u.host().is_some())
        {
            // vaultrs' builder panics on an invalid URL: check it first.
            return Err(VaultError::Address(config.address.clone()));
        }
        let settings = VaultClientSettingsBuilder::default()
            .address(&config.address)
            .timeout(Some(TIMEOUT))
            .build()
            .map_err(client_err)?;
        let client = VaultClient::new(settings).map_err(client_err)?;
        let vault = Self {
            config: config.clone(),
            token,
            client: tokio::sync::RwLock::new(client),
            calls: std::sync::OnceLock::new(),
        };
        vault.login().await?;
        Ok(vault)
    }

    /// (Re)authenticates; called at start and before each credential refresh,
    /// so an expired login token never outlives a tick.
    pub async fn login(&self) -> Result<(), VaultError> {
        let token = if let Some(t) = &self.token {
            t.clone()
        } else {
            let jwt = tokio::fs::read_to_string(Path::new(SERVICE_ACCOUNT_TOKEN))
                .await
                .map_err(VaultError::ServiceAccount)?;
            let client = self.client.read().await;
            vaultrs::auth::kubernetes::login(
                &*client,
                &self.config.kubernetes_auth.mount,
                &self.config.kubernetes_auth.role,
                jwt.trim(),
            )
            .await
            .map_err(client_err)?
            .client_token
        };
        self.client.write().await.set_token(&token);
        Ok(())
    }

    async fn read<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, VaultError> {
        let (mount, secret) = split_kv2(path)?;
        let client = self.client.read().await;
        vaultrs::kv2::read(&*client, mount, secret)
            .await
            .map_err(client_err)
    }

    pub async fn database(&self) -> Result<DatabaseSecret, VaultError> {
        self.read(&self.config.secrets.database).await
    }

    pub async fn bucket(&self) -> Result<BucketSecret, VaultError> {
        self.read(&self.config.secrets.bucket).await
    }

    /// Times every Transit call into `h`, by outcome (§10.1).
    pub fn measure(&self, h: prometheus::HistogramVec) {
        drop(self.calls.set(h));
    }

    fn timed<T, E>(&self, start: std::time::Instant, r: Result<T, E>) -> Result<T, E> {
        if let Some(h) = self.calls.get() {
            let outcome = if r.is_ok() { "ok" } else { "error" };
            h.with_label_values(&[outcome])
                .observe(start.elapsed().as_secs_f64());
        }
        r
    }

    /// A new data key from Transit: `(plaintext, wrapped)`. The server's
    /// policy allows only `datakey`, `decrypt` and `rewrap` (§6.6).
    pub async fn new_data_key(
        &self,
        transit_key: &str,
    ) -> Result<(Zeroizing<Vec<u8>>, String), VaultError> {
        use vaultrs::api::transit::requests::DataKeyType;
        let client = self.client.read().await;
        let start = std::time::Instant::now();
        let r = vaultrs::transit::generate::data_key(
            &*client,
            &self.config.transit.mount,
            transit_key,
            DataKeyType::Plaintext,
            None,
        )
        .await;
        let r = self.timed(start, r).map_err(client_err)?;
        let plain = r
            .plaintext
            .ok_or_else(|| VaultError::Client("Transit returned no plaintext".into()))?;
        Ok((decode_b64(&plain)?, r.ciphertext))
    }

    /// Unwraps a stored data key.
    pub async fn unwrap_data_key(
        &self,
        transit_key: &str,
        wrapped: &str,
    ) -> Result<Zeroizing<Vec<u8>>, VaultError> {
        let client = self.client.read().await;
        let start = std::time::Instant::now();
        let r = vaultrs::transit::data::decrypt(
            &*client,
            &self.config.transit.mount,
            transit_key,
            wrapped,
            None,
        )
        .await;
        let r = self.timed(start, r).map_err(client_err)?;
        decode_b64(&r.plaintext)
    }
}

/// Re-reads the database and bucket credentials every `every`, until
/// `shutdown`; a change reaches the pool's new connections and a new bucket
/// client at once (rotation, decision 48).
pub async fn refresh_credentials(
    vault: std::sync::Arc<Vault>,
    pool: sqlx::PgPool,
    storage: Option<std::sync::Arc<crate::store::Storage>>,
    mut current: String,
    every: Duration,
    shutdown: tokio_util::sync::CancellationToken,
) {
    let mut bucket: Option<BucketSecret> = None;
    loop {
        tokio::select! {
            () = shutdown.cancelled() => return,
            () = tokio::time::sleep(every) => {}
        }
        rotate_database(&vault, &pool, &mut current).await;
        if let Some(storage) = &storage {
            rotate_bucket(&vault, storage, &mut bucket).await;
        }
    }
}

/// Re-reads the database secret; a new URL reaches the pool's new connections.
async fn rotate_database(vault: &Vault, pool: &sqlx::PgPool, current: &mut String) {
    let fresh = async {
        vault.login().await?;
        vault.database().await
    };
    let secret = match fresh.await {
        Ok(secret) => secret,
        Err(e) => {
            tracing::warn!(error = %e, "database credentials not refreshed, the current ones stay");
            return;
        }
    };
    if secret.url == *current {
        return;
    }
    match secret.url.parse::<sqlx::postgres::PgConnectOptions>() {
        Ok(options) => {
            pool.set_connect_options(options);
            *current = secret.url;
            tracing::info!("database credentials rotated; new connections use them");
        }
        Err(e) => {
            tracing::warn!(error = %e, "rotated database URL does not parse, the previous one stays");
        }
    }
}

/// Re-reads the bucket secret; a change builds a new bucket client.
async fn rotate_bucket(
    vault: &Vault,
    storage: &crate::store::Storage,
    bucket: &mut Option<BucketSecret>,
) {
    let Ok(fresh) = vault.bucket().await.inspect_err(|e| {
        tracing::warn!(error = %e, "bucket credentials not refreshed, the current ones stay");
    }) else {
        return;
    };
    if bucket.as_ref() == Some(&fresh) {
        return;
    }
    let Ok(objects) = crate::store::s3(&fresh).inspect_err(|e| {
        tracing::warn!(error = %e, "rotated bucket secret unusable, the previous one stays");
    }) else {
        return;
    };
    // The first read only records what start-up already uses.
    if bucket.is_some() {
        storage.set_objects(objects);
        tracing::info!("bucket credentials rotated");
    }
    *bucket = Some(fresh);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv2_paths_split_into_mount_and_secret() {
        assert_eq!(
            split_kv2("mv/data/kura/database").unwrap(),
            ("mv", "kura/database")
        );
        for bad in ["mv", "mv/kura/database", "/data/x", "mv/data/"] {
            assert!(split_kv2(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn secrets_never_print() {
        let s = DatabaseSecret {
            url: "postgres://u:hunter2@db/kura".into(),
        };
        assert!(!format!("{s:?}").contains("hunter2"));
        let b = BucketSecret {
            endpoint: "http://minio:9000".into(),
            bucket: "kura".into(),
            region: default_region(),
            access_key_id: "AKID".into(),
            secret_access_key: "hunter2".into(),
        };
        assert!(!format!("{b:?}").contains("hunter2") && !format!("{b:?}").contains("AKID"));
    }
}
