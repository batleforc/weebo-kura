//! Chunk storage (RFC 0001 §6.4, §6.6): encrypted chunks in an S3-compatible
//! bucket, their rows and the blob manifests in PostgreSQL.

pub mod crypto;
pub mod keys;

use std::sync::Arc;
use std::time::Duration;

use object_store::ObjectStore;
use object_store::aws::AmazonS3Builder;
use object_store::{ClientOptions, RetryConfig};

use crate::vault::BucketSecret;

/// Outbound calls to object storage wait at most this long (§6.11).
pub const TIMEOUT: Duration = Duration::from_secs(30);
/// Retries in total, inside the SDK included (§6.11).
pub const MAX_RETRIES: usize = 3;

/// The bucket and the data keys of every vault.
pub struct Storage {
    objects: std::sync::RwLock<Arc<dyn ObjectStore>>,
    pub keys: keys::DataKeys,
}

impl Storage {
    pub fn new(objects: Arc<dyn ObjectStore>, keys: keys::DataKeys) -> Self {
        Self {
            objects: std::sync::RwLock::new(objects),
            keys,
        }
    }

    /// The current bucket client; a rotation swaps it (review P2).
    pub fn objects(&self) -> Arc<dyn ObjectStore> {
        self.objects
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_objects(&self, objects: Arc<dyn ObjectStore>) {
        *self
            .objects
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = objects;
    }
}

#[derive(Debug, thiserror::Error)]
#[error("object storage: {0}")]
pub struct StorageError(String);

/// An S3 client for the bucket secret read from Vault KV.
pub fn s3(bucket: &BucketSecret) -> Result<Arc<dyn ObjectStore>, StorageError> {
    let retry = RetryConfig {
        max_retries: MAX_RETRIES,
        retry_timeout: TIMEOUT,
        ..RetryConfig::default()
    };
    let store = AmazonS3Builder::new()
        .with_endpoint(&bucket.endpoint)
        .with_bucket_name(&bucket.bucket)
        .with_region(&bucket.region)
        .with_access_key_id(&bucket.access_key_id)
        .with_secret_access_key(&bucket.secret_access_key)
        .with_virtual_hosted_style_request(false)
        .with_retry(retry)
        // allow_http lives in the client options, which would reset it if set on the builder.
        .with_client_options(
            ClientOptions::new()
                .with_timeout(TIMEOUT)
                .with_allow_http(bucket.endpoint.starts_with("http://")),
        )
        .build()
        .map_err(|e| StorageError(e.to_string()))?;
    Ok(Arc::new(store))
}

/// Object storage and data keys without Vault (`localSecrets`): the bucket
/// secret and the master key read from their files.
pub fn local_storage(c: &crate::config::LocalSecrets) -> Result<Storage, StorageError> {
    use base64::Engine as _;
    let read = |p: &std::path::Path| {
        std::fs::read_to_string(p)
            .map(zeroize::Zeroizing::new)
            .map_err(|e| StorageError(format!("{}: {e}", p.display())))
    };
    let bucket: crate::vault::BucketSecret = serde_json::from_str(&read(&c.bucket_file)?)
        .map_err(|e| StorageError(format!("{}: {e}", c.bucket_file.display())))?;
    let raw = zeroize::Zeroizing::new(
        base64::engine::general_purpose::STANDARD
            .decode(read(&c.master_key_file)?.trim())
            .map_err(|e| StorageError(format!("{}: {e}", c.master_key_file.display())))?,
    );
    let master = crypto::DataKey::from_bytes(&raw).map_err(|_| {
        StorageError(format!(
            "{}: expected 32 bytes, base64 (openssl rand -base64 32)",
            c.master_key_file.display()
        ))
    })?;
    Ok(Storage::new(s3(&bucket)?, keys::DataKeys::local(master)))
}
