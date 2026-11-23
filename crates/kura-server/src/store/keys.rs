//! One data key per vault (RFC 0001 §6.6, decision 29): created by Transit,
//! stored only wrapped, written once with `UPDATE … WHERE wrapped_dek IS
//! NULL`, then read back; no chunk is encrypted under a key that was not read
//! back, so two replicas racing keep one key. Plaintext keys are cached at
//! most 1,000, for 10 minutes: the one deliberate fail-open of a Vault outage
//! (§10.2).
//!
//! Without Vault (`localSecrets`), a master key read from a file wraps each
//! data key instead of Transit: AES-256-GCM, the tenant's key name as
//! associated data, stored as `kura-local:v1:<base64>`. A key wrapped in one
//! mode never opens in the other: switching modes is not supported.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::time::Instant;
use uuid::Uuid;

use super::crypto::DataKey;
use crate::db;
use crate::problem::ApiError;
use crate::vault::Vault;

pub const CACHE_SIZE: usize = 1000;
pub const CACHE_TTL: Duration = Duration::from_secs(600);

/// The tenant's Transit key, created by an operator (decision 33).
pub fn transit_key_name(tenant_slug: &str) -> String {
    format!("kura-{tenant_slug}")
}

type Cache = HashMap<Uuid, (Arc<DataKey>, Instant)>;

/// Where a wrapped data key lives. The ids never collide: random UUIDs.
#[derive(Clone, Copy)]
enum Slot {
    Vault,
    Notifications,
}

impl Slot {
    /// The read of the wrapped key by id, and its write-once update.
    const fn queries(self) -> (&'static str, &'static str) {
        match self {
            Self::Vault => (
                "SELECT wrapped_dek FROM vault WHERE id = $1",
                "UPDATE vault SET wrapped_dek = $1 WHERE id = $2 AND wrapped_dek IS NULL",
            ),
            Self::Notifications => (
                "SELECT wrapped_dek FROM notification_tenant WHERE tenant_id = $1",
                "INSERT INTO notification_tenant (tenant_id, wrapped_dek) VALUES ($2, $1)
                 ON CONFLICT (tenant_id) DO UPDATE SET wrapped_dek = EXCLUDED.wrapped_dek
                 WHERE notification_tenant.wrapped_dek IS NULL",
            ),
        }
    }
}

/// What wraps the data keys: Vault Transit, or a local master key.
enum Wrapper {
    Transit(Arc<Vault>),
    Local(DataKey),
}

/// The prefix of a data key wrapped by the local master key.
pub const LOCAL_PREFIX: &str = "kura-local:v1:";

impl Wrapper {
    async fn new_data_key(&self, transit: &str) -> Result<String, String> {
        match self {
            Self::Transit(v) => v
                .new_data_key(transit)
                .await
                .map(|(_plain, wrapped)| wrapped)
                .map_err(|e| e.to_string()),
            Self::Local(master) => {
                let mut dek = zeroize::Zeroizing::new([0u8; 32]);
                aws_lc_rs::rand::fill(dek.as_mut_slice())
                    .map_err(|_| "no randomness".to_owned())?;
                let sealed = super::crypto::seal_value(master, transit.as_bytes(), dek.as_slice())
                    .map_err(|e| e.to_string())?;
                use base64::Engine as _;
                Ok(format!(
                    "{LOCAL_PREFIX}{}",
                    base64::engine::general_purpose::STANDARD.encode(sealed)
                ))
            }
        }
    }

    async fn unwrap(
        &self,
        transit: &str,
        wrapped: &str,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, String> {
        match (self, wrapped.strip_prefix(LOCAL_PREFIX)) {
            (Self::Transit(_), Some(_)) => {
                Err("this data key was wrapped by a local master key, not Vault Transit".into())
            }
            (Self::Transit(v), None) => v
                .unwrap_data_key(transit, wrapped)
                .await
                .map_err(|e| e.to_string()),
            (Self::Local(master), Some(b64)) => {
                use base64::Engine as _;
                let sealed = base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| e.to_string())?;
                super::crypto::open_value(master, transit.as_bytes(), &sealed)
                    .map_err(|e| e.to_string())
            }
            (Self::Local(_), None) => Err(
                "this data key was wrapped by Vault Transit, but the server runs without Vault"
                    .into(),
            ),
        }
    }
}

pub struct DataKeys {
    wrapper: Wrapper,
    cache: Mutex<Cache>,
}

impl DataKeys {
    pub fn new(vault: Arc<Vault>) -> Self {
        Self::with(Wrapper::Transit(vault))
    }

    /// Without Vault: data keys wrapped by this master key.
    pub fn local(master: DataKey) -> Self {
        Self::with(Wrapper::Local(master))
    }

    fn with(wrapper: Wrapper) -> Self {
        Self {
            wrapper,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn cached(&self, id: Uuid) -> Option<Arc<DataKey>> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache
            .get(&id)
            .filter(|(_, at)| at.elapsed() < CACHE_TTL)
            .map(|(k, _)| k.clone())
    }

    fn remember(&self, id: Uuid, key: Arc<DataKey>) {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if cache.len() >= CACHE_SIZE {
            cache.retain(|_, (_, at)| at.elapsed() < CACHE_TTL);
        }
        if cache.len() >= CACHE_SIZE
            && let Some(oldest) = cache.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| *k)
        {
            cache.remove(&oldest);
        }
        cache.insert(id, (key, Instant::now()));
    }

    /// The data key of `vault`, created on first use.
    pub async fn get(
        &self,
        pool: &sqlx::PgPool,
        tenant: Uuid,
        tenant_slug: &str,
        vault: Uuid,
    ) -> Result<Arc<DataKey>, ApiError> {
        self.get_in(pool, tenant, tenant_slug, vault, Slot::Vault)
            .await
    }

    /// The tenant's data key for notification secrets (RFC 0003 decision 8),
    /// wrapped by the same Transit key, created on first use.
    pub async fn notifications(
        &self,
        pool: &sqlx::PgPool,
        tenant: Uuid,
        tenant_slug: &str,
    ) -> Result<Arc<DataKey>, ApiError> {
        self.get_in(pool, tenant, tenant_slug, tenant, Slot::Notifications)
            .await
    }

    async fn get_in(
        &self,
        pool: &sqlx::PgPool,
        tenant: Uuid,
        tenant_slug: &str,
        id: Uuid,
        slot: Slot,
    ) -> Result<Arc<DataKey>, ApiError> {
        if let Some(k) = self.cached(id) {
            return Ok(k);
        }
        let (read, write) = slot.queries();
        let vault = id;
        let transit = transit_key_name(tenant_slug);
        let unavailable = ApiError::Unavailable;
        let mut tx = db::begin_tenant(pool, tenant, None).await?;
        let stored: Option<String> = sqlx::query_scalar(read)
            .bind(vault)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
        let wrapped = if let Some(w) = stored {
            w
        } else {
            let wrapped = self
                .wrapper
                .new_data_key(&transit)
                .await
                .map_err(unavailable)?;
            sqlx::query(write)
                .bind(&wrapped)
                .bind(vault)
                .execute(&mut *tx)
                .await?;
            // Read back: if another replica won the race, its key is the key.
            sqlx::query_scalar::<_, Option<String>>(read)
                .bind(vault)
                .fetch_optional(&mut *tx)
                .await?
                .flatten()
                .ok_or(ApiError::Unavailable("data key not stored".into()))?
        };
        tx.commit().await?;
        let plain = self
            .wrapper
            .unwrap(&transit, &wrapped)
            .await
            .map_err(unavailable)?;
        let key = Arc::new(
            DataKey::from_bytes(&plain).map_err(|e| ApiError::Unavailable(e.to_string()))?,
        );
        self.remember(id, key.clone());
        Ok(key)
    }

    pub fn cached_keys(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}
