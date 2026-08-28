use crate::commands::cloud_sync::{
    auto_backup_interval_duration, build_backup_data, restore_backup_data,
};
use crate::commands::encrypted_store::{load_secure_user_data, store_secure_user_data};
use crate::state::AppState;
use pebble_core::{new_id, now_timestamp, PebbleError};
use pebble_crypto::passphrase::{
    decrypt_with_passphrase, encrypt_with_passphrase, PassphraseEncryptedBlob,
};
use pebble_store::s3_backend::{S3Backend, S3BackendConfig, S3Provider};
use pebble_store::sync_backend::{is_etag_conflict, PutPrecondition, SyncBackend};
use pebble_store::vault::{
    sha256_hex, vault_meta_object_key, vault_object_key, VaultConflict, VaultMeta,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

const S3_SYNC_CONFIG_KEY: &str = "s3-sync-config";
const S3_SYNC_STATE_KEY: &str = "s3-sync-state";
const S3_DEVICE_ID_KEY: &str = "s3-sync-device-id";
const VAULT_PUSH_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(3);
const STARTUP_PULL_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct S3SyncConfig {
    pub provider: S3Provider,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    pub prefix: String,
    pub passphrase: String,
    pub enabled: bool,
    pub interval_minutes: u64,
}

impl S3SyncConfig {
    fn backend_config(&self) -> S3BackendConfig {
        S3BackendConfig {
            provider: self.provider,
            endpoint: self.endpoint.clone(),
            region: self.region.clone(),
            bucket: self.bucket.clone(),
            access_key: self.access_key.clone(),
            secret_key: self.secret_key.clone(),
            prefix: self.prefix.clone(),
        }
    }

    fn validate_credentials(&self) -> std::result::Result<(), PebbleError> {
        self.backend_config().validate()
    }

    fn validate_for_sync(&self) -> std::result::Result<(), PebbleError> {
        self.validate_credentials()?;
        if self.passphrase.trim().is_empty() {
            return Err(PebbleError::Validation(
                "A sync passphrase is required. The cloud vault is always encrypted and never uses the device key.".to_string(),
            ));
        }
        auto_backup_interval_duration(self.interval_minutes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct S3SyncRuntime {
    last_sync_at: Option<i64>,
    last_revision: Option<u64>,
    last_checksum: Option<String>,
    last_vault_etag: Option<String>,
    last_meta_etag: Option<String>,
    dirty: bool,
    pending_conflict: Option<VaultConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3SyncStatus {
    pub last_sync_at: Option<i64>,
    pub revision: Option<u64>,
    pub dirty: bool,
    pub pending_conflict: Option<VaultConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum VaultSyncResult {
    Synced {
        last_sync_at: i64,
        revision: u64,
    },
    Pulled {
        last_sync_at: i64,
        revision: u64,
        message: String,
    },
    Conflict {
        local: VaultMeta,
        cloud: VaultMeta,
    },
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictChoice {
    Cloud,
    Local,
}

fn load_config_inner(
    store: &pebble_store::Store,
    crypto: &pebble_crypto::CryptoService,
) -> std::result::Result<Option<S3SyncConfig>, PebbleError> {
    let Some(json) = load_secure_user_data(crypto, store, S3_SYNC_CONFIG_KEY)? else {
        return Ok(None);
    };
    let config: S3SyncConfig = serde_json::from_slice(&json)
        .map_err(|e| PebbleError::Internal(format!("Failed to deserialize S3 sync config: {e}")))?;
    auto_backup_interval_duration(config.interval_minutes)?;
    Ok(Some(config))
}

fn load_runtime_inner(
    store: &pebble_store::Store,
    crypto: &pebble_crypto::CryptoService,
) -> std::result::Result<S3SyncRuntime, PebbleError> {
    let Some(json) = load_secure_user_data(crypto, store, S3_SYNC_STATE_KEY)? else {
        return Ok(S3SyncRuntime::default());
    };
    serde_json::from_slice(&json)
        .map_err(|e| PebbleError::Internal(format!("Failed to deserialize S3 sync state: {e}")))
}

fn save_runtime(state: &AppState, runtime: &S3SyncRuntime) -> std::result::Result<(), PebbleError> {
    let json = serde_json::to_vec(runtime)
        .map_err(|e| PebbleError::Internal(format!("Failed to serialize S3 sync state: {e}")))?;
    store_secure_user_data(state.crypto()?, &state.store, S3_SYNC_STATE_KEY, &json)
}

fn device_id(state: &AppState) -> std::result::Result<String, PebbleError> {
    if let Some(existing) = load_secure_user_data(state.crypto()?, &state.store, S3_DEVICE_ID_KEY)? {
        return String::from_utf8(existing).map_err(|e| {
            PebbleError::Internal(format!("Stored S3 device id was not valid UTF-8: {e}"))
        });
    }
    let id = new_id();
    store_secure_user_data(state.crypto()?, &state.store, S3_DEVICE_ID_KEY, id.as_bytes())?;
    Ok(id)
}

fn seal_blob(plaintext: &[u8], passphrase: &str) -> std::result::Result<Vec<u8>, PebbleError> {
    let blob = encrypt_with_passphrase(plaintext, passphrase)?;
    serde_json::to_vec(&blob)
        .map_err(|e| PebbleError::Internal(format!("Failed to serialize encrypted vault: {e}")))
}

fn unseal_blob(data: &[u8], passphrase: &str) -> std::result::Result<Vec<u8>, PebbleError> {
    let blob: PassphraseEncryptedBlob = serde_json::from_slice(data).map_err(|e| {
        PebbleError::Validation(format!(
            "Cloud object is not a passphrase-encrypted Pebble vault: {e}"
        ))
    })?;
    decrypt_with_passphrase(&blob, passphrase)
}

fn local_placeholder_meta(runtime: &S3SyncRuntime, device_id: &str) -> VaultMeta {
    VaultMeta {
        revision: runtime.last_revision.unwrap_or(0),
        checksum: runtime.last_checksum.clone().unwrap_or_default(),
        device_id: device_id.to_string(),
        updated_at: runtime.last_sync_at.unwrap_or(0),
    }
}

async fn read_cloud_meta(
    backend: &S3Backend,
    config: &S3SyncConfig,
) -> std::result::Result<Option<(VaultMeta, Option<String>)>, PebbleError> {
    let key = vault_meta_object_key(&config.prefix)?;
    let Some(object) = backend.get(&key).await? else {
        return Ok(None);
    };
    let plaintext = unseal_blob(&object.data, &config.passphrase)?;
    let meta: VaultMeta = serde_json::from_slice(&plaintext)
        .map_err(|e| PebbleError::Validation(format!("Invalid vault metadata: {e}")))?;
    Ok(Some((meta, object.etag)))
}

fn persist_conflict(
    state: &AppState,
    runtime: &mut S3SyncRuntime,
    local: VaultMeta,
    cloud: VaultMeta,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    runtime.pending_conflict = Some(VaultConflict {
        local: local.clone(),
        cloud: cloud.clone(),
    });
    save_runtime(state, runtime)?;
    Ok(VaultSyncResult::Conflict { local, cloud })
}

async fn emit_conflict(app: &AppHandle, result: &VaultSyncResult) {
    if let VaultSyncResult::Conflict { local, cloud } = result {
        let _ = app.emit(
            "cloud-sync:vault-conflict",
            VaultConflict {
                local: local.clone(),
                cloud: cloud.clone(),
            },
        );
    }
}

async fn push_vault(
    state: &AppState,
    config: &S3SyncConfig,
    use_latest_remote_etag: bool,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    config.validate_for_sync()?;
    let backend = S3Backend::new(config.backend_config())?;
    let vault_key = vault_object_key(&config.prefix)?;
    let meta_key = vault_meta_object_key(&config.prefix)?;
    let mut runtime = load_runtime_inner(&state.store, state.crypto()?)?;
    let device = device_id(state)?;
    let cloud_head = backend.head(&vault_key).await?;
    let cloud_meta = read_cloud_meta(&backend, config).await?;

    if !use_latest_remote_etag {
        if let Some(remote) = cloud_head.as_ref() {
            let remote_etag = remote.etag.clone();
            let etag_changed = match (runtime.last_vault_etag.as_ref(), remote_etag.as_ref()) {
                (Some(local_etag), Some(cloud_etag)) => local_etag != cloud_etag,
                (None, Some(_)) => true,
                _ => false,
            };
            let checksum_changed = match (runtime.last_checksum.as_ref(), cloud_meta.as_ref()) {
                (Some(local_sum), Some((meta, _))) => local_sum != &meta.checksum,
                (None, Some(_)) => true,
                _ => false,
            };
            if etag_changed || checksum_changed {
                let local = local_placeholder_meta(&runtime, &device);
                let cloud =
                    cloud_meta
                        .as_ref()
                        .map(|(meta, _)| meta.clone())
                        .unwrap_or(VaultMeta {
                            revision: runtime.last_revision.unwrap_or(0) + 1,
                            checksum: remote_etag.clone().unwrap_or_default(),
                            device_id: "cloud".to_string(),
                            updated_at: now_timestamp(),
                        });
                return persist_conflict(state, &mut runtime, local, cloud);
            }
        }
    }

    // Settings vault only: accounts metadata + passphrase-wrapped secrets.
    // Never SQLite, Tantivy, mail bodies, or attachments.
    let plaintext = build_backup_data(state, Some(config.passphrase.clone()))?;
    let sealed = seal_blob(&plaintext, &config.passphrase)?;
    let checksum = sha256_hex(&sealed);
    let revision = cloud_meta
        .as_ref()
        .map(|(meta, _)| meta.revision)
        .or(runtime.last_revision)
        .unwrap_or(0)
        .saturating_add(1);
    let meta = VaultMeta {
        revision,
        checksum: checksum.clone(),
        device_id: device,
        updated_at: now_timestamp(),
    };
    let sealed_meta = seal_blob(
        &serde_json::to_vec(&meta).map_err(|e| {
            PebbleError::Internal(format!("Failed to serialize vault metadata: {e}"))
        })?,
        &config.passphrase,
    )?;

    let vault_precondition = if use_latest_remote_etag {
        match cloud_head.as_ref().and_then(|head| head.etag.as_deref()) {
            Some(etag) => PutPrecondition::IfMatch(etag),
            None => PutPrecondition::IfNoneMatchStar,
        }
    } else if let Some(etag) = runtime.last_vault_etag.as_deref() {
        PutPrecondition::IfMatch(etag)
    } else if cloud_head.is_some() {
        let local = local_placeholder_meta(&runtime, &meta.device_id);
        let cloud = cloud_meta
            .as_ref()
            .map(|(meta, _)| meta.clone())
            .unwrap_or_else(|| local.clone());
        return persist_conflict(state, &mut runtime, local, cloud);
    } else {
        PutPrecondition::IfNoneMatchStar
    };

    let vault_put = match backend.put(&vault_key, &sealed, vault_precondition).await {
        Ok(result) => result,
        Err(error) if is_etag_conflict(&error) => {
            let local = local_placeholder_meta(&runtime, &meta.device_id);
            let cloud = read_cloud_meta(&backend, config)
                .await?
                .map(|(meta, _)| meta)
                .unwrap_or(local.clone());
            return persist_conflict(state, &mut runtime, local, cloud);
        }
        Err(error) => return Err(error),
    };

    let meta_precondition = if use_latest_remote_etag {
        match cloud_meta.as_ref().and_then(|(_, etag)| etag.as_deref()) {
            Some(etag) => PutPrecondition::IfMatch(etag),
            None => PutPrecondition::IfNoneMatchStar,
        }
    } else if let Some(etag) = runtime.last_meta_etag.as_deref() {
        PutPrecondition::IfMatch(etag)
    } else {
        PutPrecondition::IfNoneMatchStar
    };

    let meta_put = match backend
        .put(&meta_key, &sealed_meta, meta_precondition)
        .await
    {
        Ok(result) => result,
        Err(error) if is_etag_conflict(&error) => {
            let local = meta.clone();
            let cloud = read_cloud_meta(&backend, config)
                .await?
                .map(|(meta, _)| meta)
                .unwrap_or(local.clone());
            return persist_conflict(state, &mut runtime, local, cloud);
        }
        Err(error) => return Err(error),
    };

    runtime.last_sync_at = Some(meta.updated_at);
    runtime.last_revision = Some(meta.revision);
    runtime.last_checksum = Some(checksum);
    runtime.last_vault_etag = vault_put.etag;
    runtime.last_meta_etag = meta_put.etag;
    runtime.dirty = false;
    runtime.pending_conflict = None;
    save_runtime(state, &runtime)?;
    state.vault_push.dirty.store(false, Ordering::SeqCst);
    Ok(VaultSyncResult::Synced {
        last_sync_at: meta.updated_at,
        revision: meta.revision,
    })
}

async fn pull_vault(
    state: &AppState,
    config: &S3SyncConfig,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    config.validate_for_sync()?;
    let backend = S3Backend::new(config.backend_config())?;
    let vault_key = vault_object_key(&config.prefix)?;
    let Some(object) = backend.get(&vault_key).await? else {
        return Ok(VaultSyncResult::Empty);
    };
    let plaintext = unseal_blob(&object.data, &config.passphrase)?;
    let message = restore_backup_data(state, &plaintext, Some(config.passphrase.clone()))?;
    let cloud_meta = read_cloud_meta(&backend, config).await?;
    let checksum = sha256_hex(&object.data);
    let revision = cloud_meta
        .as_ref()
        .map(|(meta, _)| meta.revision)
        .unwrap_or(1);
    let updated_at = cloud_meta
        .as_ref()
        .map(|(meta, _)| meta.updated_at)
        .unwrap_or_else(now_timestamp);
    let mut runtime = load_runtime_inner(&state.store, state.crypto()?)?;
    runtime.last_sync_at = Some(updated_at);
    runtime.last_revision = Some(revision);
    runtime.last_checksum = Some(checksum);
    runtime.last_vault_etag = object.etag;
    runtime.last_meta_etag = cloud_meta.and_then(|(_, etag)| etag);
    runtime.dirty = false;
    runtime.pending_conflict = None;
    save_runtime(state, &runtime)?;
    state.vault_push.dirty.store(false, Ordering::SeqCst);
    Ok(VaultSyncResult::Pulled {
        last_sync_at: updated_at,
        revision,
        message,
    })
}

async fn startup_reconcile(
    state: &AppState,
    config: &S3SyncConfig,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    let runtime = load_runtime_inner(&state.store, state.crypto()?)?;
    let backend = S3Backend::new(config.backend_config())?;
    let cloud_meta = read_cloud_meta(&backend, config).await?;
    let has_accounts = !state.store.list_accounts()?.is_empty();
    let dirty = runtime.dirty || state.vault_push.dirty.load(Ordering::SeqCst);

    match cloud_meta {
        None => {
            if dirty || has_accounts {
                push_vault(state, config, false).await
            } else {
                Ok(VaultSyncResult::Empty)
            }
        }
        Some((cloud, _)) => {
            let same_checksum = runtime.last_checksum.as_deref() == Some(cloud.checksum.as_str());
            if same_checksum {
                if dirty {
                    push_vault(state, config, false).await
                } else {
                    Ok(VaultSyncResult::Synced {
                        last_sync_at: runtime.last_sync_at.unwrap_or(cloud.updated_at),
                        revision: cloud.revision,
                    })
                }
            } else if dirty || (has_accounts && runtime.last_checksum.is_none()) {
                let local = local_placeholder_meta(&runtime, &device_id(state)?);
                let mut runtime = runtime;
                persist_conflict(state, &mut runtime, local, cloud)
            } else {
                pull_vault(state, config).await
            }
        }
    }
}

pub fn request_s3_vault_push(state: &AppState) {
    let Ok(crypto) = state.crypto() else { return };
    if let Ok(Some(config)) = load_config_inner(&state.store, crypto) {
        if !config.passphrase.trim().is_empty() && config.validate_credentials().is_ok() {
            if let Ok(mut runtime) = load_runtime_inner(&state.store, crypto) {
                runtime.dirty = true;
                let _ = save_runtime(state, &runtime);
            }
            state.vault_push.request();
        }
    }
}

#[tauri::command]
pub async fn test_s3_connection(config: S3SyncConfig) -> std::result::Result<String, PebbleError> {
    config.validate_credentials()?;
    let backend = S3Backend::new(config.backend_config())?;
    backend.test_connection().await?;
    Ok("Connection successful".to_string())
}

#[tauri::command]
pub fn save_s3_sync_config(
    state: State<'_, AppState>,
    config: S3SyncConfig,
) -> std::result::Result<(), PebbleError> {
    if config.enabled {
        config.validate_for_sync()?;
    } else {
        auto_backup_interval_duration(config.interval_minutes)?;
        if !config.access_key.trim().is_empty() || !config.secret_key.trim().is_empty() {
            config.validate_credentials()?;
        }
    }
    let json = serde_json::to_vec(&config)
        .map_err(|e| PebbleError::Internal(format!("Failed to serialize S3 sync config: {e}")))?;
    store_secure_user_data(state.crypto()?, &state.store, S3_SYNC_CONFIG_KEY, &json)
}

#[tauri::command]
pub fn load_s3_sync_config(
    state: State<'_, AppState>,
) -> std::result::Result<Option<S3SyncConfig>, PebbleError> {
    load_config_inner(&state.store, state.crypto()?)
}

#[tauri::command]
pub fn delete_s3_sync_config(state: State<'_, AppState>) -> std::result::Result<(), PebbleError> {
    state.store.delete_secure_user_data(S3_SYNC_CONFIG_KEY)?;
    Ok(())
}

#[tauri::command]
pub fn get_s3_sync_status(
    state: State<'_, AppState>,
) -> std::result::Result<S3SyncStatus, PebbleError> {
    let runtime = load_runtime_inner(&state.store, state.crypto()?)?;
    Ok(S3SyncStatus {
        last_sync_at: runtime.last_sync_at,
        revision: runtime.last_revision,
        dirty: runtime.dirty || state.vault_push.dirty.load(Ordering::SeqCst),
        pending_conflict: runtime.pending_conflict,
    })
}

#[tauri::command]
pub async fn sync_s3_vault(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    let config = load_config_inner(&state.store, state.crypto()?)?.ok_or_else(|| {
        PebbleError::Validation("Save S3-compatible sync settings before syncing".to_string())
    })?;
    let result = push_vault(&state, &config, false).await?;
    emit_conflict(&app, &result).await;
    if matches!(result, VaultSyncResult::Synced { .. }) {
        let _ = app.emit("cloud-sync:vault-synced", ());
    }
    Ok(result)
}

#[tauri::command]
pub async fn restore_s3_vault(
    app: AppHandle,
    state: State<'_, AppState>,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    let config = load_config_inner(&state.store, state.crypto()?)?.ok_or_else(|| {
        PebbleError::Validation("Save S3-compatible sync settings before restoring".to_string())
    })?;
    let result = pull_vault(&state, &config).await?;
    if matches!(result, VaultSyncResult::Pulled { .. }) {
        let _ = app.emit("cloud-sync:vault-restored", ());
    }
    Ok(result)
}

#[tauri::command]
pub async fn resolve_s3_vault_conflict(
    app: AppHandle,
    state: State<'_, AppState>,
    choice: ConflictChoice,
) -> std::result::Result<VaultSyncResult, PebbleError> {
    let config = load_config_inner(&state.store, state.crypto()?)?.ok_or_else(|| {
        PebbleError::Validation(
            "Save S3-compatible sync settings before resolving a conflict".to_string(),
        )
    })?;
    let result = match choice {
        ConflictChoice::Cloud => pull_vault(&state, &config).await?,
        ConflictChoice::Local => push_vault(&state, &config, true).await?,
    };
    emit_conflict(&app, &result).await;
    match &result {
        VaultSyncResult::Pulled { .. } => {
            let _ = app.emit("cloud-sync:vault-restored", ());
        }
        VaultSyncResult::Synced { .. } => {
            let _ = app.emit("cloud-sync:vault-synced", ());
        }
        _ => {}
    }
    Ok(result)
}

pub async fn run_s3_vault_worker(app: AppHandle) {
    tokio::time::sleep(STARTUP_PULL_DELAY).await;
    {
        let state = app.state::<AppState>();
        if let Ok(crypto) = state.crypto() {
        match load_config_inner(&state.store, crypto) {
            Ok(Some(config)) if !config.passphrase.trim().is_empty() => {
                match startup_reconcile(&state, &config).await {
                    Ok(result) => {
                        emit_conflict(&app, &result).await;
                        match &result {
                            VaultSyncResult::Pulled { .. } => {
                                tracing::info!("[s3-vault] startup pull restored settings");
                                let _ = app.emit("cloud-sync:vault-restored", ());
                            }
                            VaultSyncResult::Synced { .. } => {
                                tracing::info!(
                                    "[s3-vault] startup reconcile pushed or confirmed vault"
                                );
                            }
                            VaultSyncResult::Conflict { .. } => {
                                tracing::warn!(
                                    "[s3-vault] startup conflict; waiting for user choice"
                                );
                            }
                            VaultSyncResult::Empty => {}
                        }
                    }
                    Err(error) => {
                        tracing::warn!("[s3-vault] startup reconcile failed: {error}");
                    }
                }
            }
            Ok(_) => {}
            Err(error) => tracing::warn!("[s3-vault] invalid stored config: {error}"),
        }
        }
    }

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    let mut last_interval_push: Option<std::time::Instant> = None;
    loop {
        tokio::select! {
            _ = async {
                let signal = app.state::<AppState>().vault_push.clone();
                signal.notify.notified().await;
            } => {
                loop {
                    tokio::select! {
                        _ = async {
                            let signal = app.state::<AppState>().vault_push.clone();
                            signal.notify.notified().await;
                        } => {}
                        _ = tokio::time::sleep(VAULT_PUSH_DEBOUNCE) => break,
                    }
                }
                let state = app.state::<AppState>();
                let Ok(crypto) = state.crypto() else { continue };
                match load_config_inner(&state.store, crypto) {
                    Ok(Some(config)) if !config.passphrase.trim().is_empty() => {
                        match push_vault(&state, &config, false).await {
                            Ok(result) => {
                                emit_conflict(&app, &result).await;
                                if matches!(result, VaultSyncResult::Synced { .. }) {
                                    last_interval_push = Some(std::time::Instant::now());
                                    let _ = app.emit("cloud-sync:vault-synced", ());
                                }
                            }
                            Err(error) => tracing::warn!("[s3-vault] debounced push failed: {error}"),
                        }
                    }
                    _ => {}
                }
            }
            _ = interval.tick() => {
                let state = app.state::<AppState>();
                let Ok(crypto) = state.crypto() else { continue };
                let Ok(Some(config)) = load_config_inner(&state.store, crypto) else {
                    continue;
                };
                if !config.enabled || config.passphrase.trim().is_empty() {
                    continue;
                }
                let Ok(duration) = auto_backup_interval_duration(config.interval_minutes) else {
                    continue;
                };
                if let Some(last) = last_interval_push {
                    if last.elapsed() < duration {
                        continue;
                    }
                }
                match push_vault(&state, &config, false).await {
                    Ok(result) => {
                        emit_conflict(&app, &result).await;
                        if matches!(result, VaultSyncResult::Synced { .. }) {
                            last_interval_push = Some(std::time::Instant::now());
                            let _ = app.emit("cloud-sync:vault-synced", ());
                        }
                    }
                    Err(error) => tracing::warn!("[s3-vault] interval push failed: {error}"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_vault_is_ciphertext_without_plaintext_settings() {
        let plaintext = br#"{"version":2,"accounts":[{"email":"user@example.com"}]}"#;
        let sealed = seal_blob(plaintext, "sync-passphrase").unwrap();
        let as_text = String::from_utf8(sealed.clone()).unwrap();
        assert!(!as_text.contains("user@example.com"));
        assert!(as_text.contains("aes-256-gcm"));
        let opened = unseal_blob(&sealed, "sync-passphrase").unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn sync_config_requires_passphrase_for_enabled_sync() {
        let config = S3SyncConfig {
            provider: S3Provider::R2,
            endpoint: "https://abc.r2.cloudflarestorage.com".to_string(),
            region: "auto".to_string(),
            bucket: "vault".to_string(),
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            prefix: "pebble".to_string(),
            passphrase: String::new(),
            enabled: true,
            interval_minutes: 60,
        };
        assert!(config.validate_for_sync().is_err());
    }
}
