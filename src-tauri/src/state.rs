use crate::realtime::SyncTrigger;
use pebble_core::PebbleError;
use pebble_crypto::CryptoService;
use pebble_search::TantivySearch;
use pebble_store::Store;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::{mpsc, watch, Mutex, Notify};

pub type KeyedLockRegistry = Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>;
pub type OAuthAccountLockRegistry = KeyedLockRegistry;

pub const SEARCH_STILL_STARTING: &str = "Search is still starting. Please try again in a moment.";
pub const CRYPTO_STILL_STARTING: &str =
    "Device encryption is still starting. Please try again in a moment.";

pub struct SyncHandle {
    pub stop_tx: watch::Sender<bool>,
    pub trigger_tx: mpsc::UnboundedSender<SyncTrigger>,
    pub task: tokio::task::JoinHandle<()>,
}

pub struct VaultPushSignal {
    pub notify: Notify,
    pub dirty: AtomicBool,
}

impl VaultPushSignal {
    pub fn request(&self) {
        self.dirty.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }
}

pub struct AppState {
    pub store: Arc<Store>,
    search_slot: OnceLock<Arc<TantivySearch>>,
    crypto_slot: OnceLock<Arc<CryptoService>>,
    search_error: OnceLock<String>,
    crypto_error: OnceLock<String>,
    pub oauth_account_locks: OAuthAccountLockRegistry,
    pub secure_user_data_locks: KeyedLockRegistry,
    pub sync_handles: Mutex<HashMap<String, SyncHandle>>,
    /// Kept alive so the snooze watcher's `stop_rx` remains open.
    #[allow(dead_code)]
    pub snooze_stop_tx: std::sync::mpsc::Sender<()>,
    pub attachments_dir: PathBuf,
    pub notifications_enabled: Arc<AtomicBool>,
    pub notification_attention_active: Arc<AtomicBool>,
    pub vault_push: Arc<VaultPushSignal>,
}

impl AppState {
    pub fn new(
        store: Store,
        search: TantivySearch,
        crypto: CryptoService,
        snooze_stop_tx: std::sync::mpsc::Sender<()>,
        attachments_dir: PathBuf,
    ) -> Self {
        let state = Self::new_deferred(store, snooze_stop_tx, attachments_dir);
        let _ = state.search_slot.set(Arc::new(search));
        let _ = state.crypto_slot.set(Arc::new(crypto));
        state
    }

    /// Register store-backed state before Tantivy / Keystore finish.
    /// Search and crypto commands fail lazily until the matching setter runs.
    pub fn new_deferred(
        store: Store,
        snooze_stop_tx: std::sync::mpsc::Sender<()>,
        attachments_dir: PathBuf,
    ) -> Self {
        Self {
            store: Arc::new(store),
            search_slot: OnceLock::new(),
            crypto_slot: OnceLock::new(),
            search_error: OnceLock::new(),
            crypto_error: OnceLock::new(),
            oauth_account_locks: Arc::new(Mutex::new(HashMap::new())),
            secure_user_data_locks: Arc::new(Mutex::new(HashMap::new())),
            sync_handles: Mutex::new(HashMap::new()),
            snooze_stop_tx,
            attachments_dir,
            notifications_enabled: Arc::new(AtomicBool::new(true)),
            notification_attention_active: Arc::new(AtomicBool::new(false)),
            vault_push: Arc::new(VaultPushSignal {
                notify: Notify::new(),
                dirty: AtomicBool::new(false),
            }),
        }
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn set_search(&self, search: TantivySearch) {
        let _ = self.search_slot.set(Arc::new(search));
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn set_crypto(&self, crypto: CryptoService) {
        let _ = self.crypto_slot.set(Arc::new(crypto));
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn set_search_error(&self, error: impl Into<String>) {
        let _ = self.search_error.set(error.into());
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn set_crypto_error(&self, error: impl Into<String>) {
        let _ = self.crypto_error.set(error.into());
    }

    pub fn search(&self) -> Result<&TantivySearch, PebbleError> {
        self.search_slot
            .get()
            .map(Arc::as_ref)
            .ok_or_else(|| self.search_not_ready())
    }

    pub fn crypto(&self) -> Result<&CryptoService, PebbleError> {
        self.crypto_slot
            .get()
            .map(Arc::as_ref)
            .ok_or_else(|| self.crypto_not_ready())
    }

    pub fn search_arc(&self) -> Result<Arc<TantivySearch>, PebbleError> {
        self.search_slot
            .get()
            .cloned()
            .ok_or_else(|| self.search_not_ready())
    }

    pub fn crypto_arc(&self) -> Result<Arc<CryptoService>, PebbleError> {
        self.crypto_slot
            .get()
            .cloned()
            .ok_or_else(|| self.crypto_not_ready())
    }

    fn search_not_ready(&self) -> PebbleError {
        PebbleError::Internal(
            self.search_error
                .get()
                .cloned()
                .unwrap_or_else(|| SEARCH_STILL_STARTING.to_string()),
        )
    }

    fn crypto_not_ready(&self) -> PebbleError {
        PebbleError::Internal(
            self.crypto_error
                .get()
                .cloned()
                .unwrap_or_else(|| CRYPTO_STILL_STARTING.to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pebble_core::new_id;

    fn deferred_state() -> AppState {
        let (snooze_stop_tx, _snooze_stop_rx) = std::sync::mpsc::channel();
        AppState::new_deferred(
            Store::open_in_memory().unwrap(),
            snooze_stop_tx,
            std::env::temp_dir().join(format!("pebble-state-deferred-{}", new_id())),
        )
    }

    #[test]
    fn deferred_search_and_crypto_fail_until_set() {
        let state = deferred_state();
        let search_err = match state.search() {
            Err(error) => error,
            Ok(_) => panic!("search should be pending"),
        };
        assert!(search_err.to_string().contains("Search is still starting"));
        let crypto_err = match state.crypto() {
            Err(error) => error,
            Ok(_) => panic!("crypto should be pending"),
        };
        assert!(crypto_err
            .to_string()
            .contains("Device encryption is still starting"));

        state.set_search(TantivySearch::open_in_memory().unwrap());
        state.set_crypto(CryptoService::from_key([7; 32]));
        assert!(state.search().is_ok());
        assert!(state.crypto().is_ok());
    }

    #[test]
    fn deferred_crypto_surfaces_init_failure_on_first_use() {
        let state = deferred_state();
        state.set_search_error("Failed to open the search index: io");
        state.set_crypto_error("Failed to initialize the device encryption key: keystore");
        let search_err = match state.search() {
            Err(error) => error,
            Ok(_) => panic!("search init failed"),
        };
        assert!(search_err.to_string().contains("Failed to open the search index"));
        let err = match state.crypto() {
            Err(error) => error,
            Ok(_) => panic!("crypto init failed"),
        };
        assert!(err
            .to_string()
            .contains("Failed to initialize the device encryption key"));
    }
}
