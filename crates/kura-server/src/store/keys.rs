//! One data key per vault (RFC 0001 §6.6, decision 29): created by Transit,
//! stored only wrapped, written once with `UPDATE … WHERE wrapped_dek IS
//! NULL`, then read back; no chunk is encrypted under a key that was not read
//! back, so two replicas racing keep one key. Plaintext keys are cached at
//! most 1,000, for 10 minutes: the one deliberate fail-open of a Vault outage
//! (§10.2).

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

pub struct DataKeys {
    vault: Arc<Vault>,
    cache: Mutex<Cache>,
}

impl DataKeys {
    pub fn new(vault: Arc<Vault>) -> Self {
        Self {
            vault,
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
        if let Some(k) = self.cached(vault) {
            return Ok(k);
        }
        let transit = transit_key_name(tenant_slug);
        let unavailable = |e: crate::vault::VaultError| ApiError::Unavailable(e.to_string());
        let mut tx = db::begin_tenant(pool, tenant, None).await?;
        let stored: Option<String> =
            sqlx::query_scalar("SELECT wrapped_dek FROM vault WHERE id = $1")
                .bind(vault)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
        let wrapped = match stored {
            Some(w) => w,
            None => {
                let (_ours, wrapped) = self
                    .vault
                    .new_data_key(&transit)
                    .await
                    .map_err(unavailable)?;
                sqlx::query(
                    "UPDATE vault SET wrapped_dek = $1 WHERE id = $2 AND wrapped_dek IS NULL",
                )
                .bind(&wrapped)
                .bind(vault)
                .execute(&mut *tx)
                .await?;
                // Read back: if another replica won the race, its key is the key.
                sqlx::query_scalar::<_, Option<String>>(
                    "SELECT wrapped_dek FROM vault WHERE id = $1",
                )
                .bind(vault)
                .fetch_one(&mut *tx)
                .await?
                .ok_or(ApiError::Unavailable("data key not stored".into()))?
            }
        };
        tx.commit().await?;
        let plain = self
            .vault
            .unwrap_data_key(&transit, &wrapped)
            .await
            .map_err(unavailable)?;
        let key = Arc::new(
            DataKey::from_bytes(&plain).map_err(|e| ApiError::Unavailable(e.to_string()))?,
        );
        self.remember(vault, key.clone());
        Ok(key)
    }

    pub fn cached_keys(&self) -> usize {
        self.cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}
