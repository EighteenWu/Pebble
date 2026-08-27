use pebble_core::{PebbleError, Result};
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::cloud_sync::WebDavClient;
use crate::vault::sha256_hex;

pub const ETAG_CONFLICT_PREFIX: &str = "etag_conflict";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutPrecondition<'a> {
    Unconditional,
    IfMatch(&'a str),
    IfNoneMatchStar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PutObjectResult {
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetObjectResult {
    pub data: Vec<u8>,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadObjectResult {
    pub etag: Option<String>,
    pub content_length: Option<u64>,
}

pub fn etag_conflict_error() -> PebbleError {
    PebbleError::Sync(
        "etag_conflict: remote object changed; choose Use cloud or Use local".to_string(),
    )
}

pub fn is_etag_conflict(err: &PebbleError) -> bool {
    matches!(err, PebbleError::Sync(message) if message.starts_with(ETAG_CONFLICT_PREFIX))
}

pub fn normalize_etag(raw: &str) -> String {
    let trimmed = raw.trim();
    let trimmed = trimmed.strip_prefix("W/").unwrap_or(trimmed).trim();
    trimmed.trim_matches('"').to_string()
}

pub fn quoted_etag(etag: &str) -> String {
    let normalized = normalize_etag(etag);
    format!("\"{normalized}\"")
}

/// Storage adapter used by WebDAV backup, local JSON import/export, and S3 vault sync.
pub trait SyncBackend: Send + Sync {
    fn test_connection(&self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn put(
        &self,
        key: &str,
        data: &[u8],
        precondition: PutPrecondition<'_>,
    ) -> impl std::future::Future<Output = Result<PutObjectResult>> + Send;
    fn get(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<GetObjectResult>>> + Send;
    fn head(
        &self,
        key: &str,
    ) -> impl std::future::Future<Output = Result<Option<HeadObjectResult>>> + Send;
}

pub struct WebDavBackend {
    client: WebDavClient,
}

impl WebDavBackend {
    pub fn new(url: String, username: String, password: String) -> Result<Self> {
        Ok(Self {
            client: WebDavClient::new(url, username, password)?,
        })
    }
}

impl SyncBackend for WebDavBackend {
    async fn test_connection(&self) -> Result<()> {
        self.client.test_connection().await
    }

    async fn put(
        &self,
        key: &str,
        data: &[u8],
        precondition: PutPrecondition<'_>,
    ) -> Result<PutObjectResult> {
        self.client.put_object(key, data, precondition).await
    }

    async fn get(&self, key: &str) -> Result<Option<GetObjectResult>> {
        self.client.get_object(key).await
    }

    async fn head(&self, key: &str) -> Result<Option<HeadObjectResult>> {
        self.client.head_object(key).await
    }
}

/// In-memory JSON backend used by local file import/export and unit tests.
#[derive(Default)]
pub struct LocalJsonBackend {
    objects: Mutex<HashMap<String, (Vec<u8>, String)>>,
}

impl LocalJsonBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_object(key: impl Into<String>, data: Vec<u8>) -> Self {
        let key = key.into();
        let etag = sha256_hex(&data);
        let mut objects = HashMap::new();
        objects.insert(key, (data, etag));
        Self {
            objects: Mutex::new(objects),
        }
    }

    pub async fn object_bytes(&self, key: &str) -> Option<Vec<u8>> {
        self.objects
            .lock()
            .await
            .get(key)
            .map(|(data, _)| data.clone())
    }
}

impl SyncBackend for LocalJsonBackend {
    async fn test_connection(&self) -> Result<()> {
        Ok(())
    }

    async fn put(
        &self,
        key: &str,
        data: &[u8],
        precondition: PutPrecondition<'_>,
    ) -> Result<PutObjectResult> {
        let mut objects = self.objects.lock().await;
        let current = objects.get(key);
        match precondition {
            PutPrecondition::Unconditional => {}
            PutPrecondition::IfNoneMatchStar => {
                if current.is_some() {
                    return Err(etag_conflict_error());
                }
            }
            PutPrecondition::IfMatch(expected) => {
                let Some((_, etag)) = current else {
                    return Err(etag_conflict_error());
                };
                if normalize_etag(etag) != normalize_etag(expected) {
                    return Err(etag_conflict_error());
                }
            }
        }
        let etag = sha256_hex(data);
        objects.insert(key.to_string(), (data.to_vec(), etag.clone()));
        Ok(PutObjectResult { etag: Some(etag) })
    }

    async fn get(&self, key: &str) -> Result<Option<GetObjectResult>> {
        Ok(self
            .objects
            .lock()
            .await
            .get(key)
            .map(|(data, etag)| GetObjectResult {
                data: data.clone(),
                etag: Some(etag.clone()),
            }))
    }

    async fn head(&self, key: &str) -> Result<Option<HeadObjectResult>> {
        Ok(self
            .objects
            .lock()
            .await
            .get(key)
            .map(|(data, etag)| HeadObjectResult {
                etag: Some(etag.clone()),
                content_length: Some(data.len() as u64),
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_json_round_trip_and_etag_conflict() {
        let backend = LocalJsonBackend::new();
        let key = "pebble-settings-backup.json";
        let first = backend
            .put(key, b"{\"version\":2}", PutPrecondition::IfNoneMatchStar)
            .await
            .unwrap();
        let etag = first.etag.expect("etag");

        let loaded = backend.get(key).await.unwrap().unwrap();
        assert_eq!(loaded.data, b"{\"version\":2}");
        assert_eq!(loaded.etag.as_deref(), Some(etag.as_str()));

        let conflict = backend
            .put(key, b"stale", PutPrecondition::IfMatch("wrong"))
            .await
            .unwrap_err();
        assert!(is_etag_conflict(&conflict));

        backend
            .put(key, b"updated", PutPrecondition::IfMatch(&etag))
            .await
            .unwrap();
        assert_eq!(
            backend.object_bytes(key).await.as_deref(),
            Some(&b"updated"[..])
        );
    }

    #[tokio::test]
    async fn local_json_create_only_rejects_existing_object() {
        let backend = LocalJsonBackend::with_object("vault.json", b"already".to_vec());
        let err = backend
            .put("vault.json", b"new", PutPrecondition::IfNoneMatchStar)
            .await
            .unwrap_err();
        assert!(is_etag_conflict(&err));
    }
}
