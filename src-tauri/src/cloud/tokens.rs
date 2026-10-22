//! Where the refresh token lives: Windows Credential Manager, never SQLite or logs.

use std::sync::{Arc, Mutex};

use keyring_core::api::CredentialStoreApi;

const SERVICE: &str = "dev.keplerhaloxx.atlas";
const USER: &str = "refresh_token";

pub trait TokenStore: Send + Sync {
    fn get(&self) -> Option<String>;
    fn set(&self, token: &str);
    fn clear(&self);
}

/// The real store: one generic credential named after the app.
pub struct CredentialManager(Option<Arc<windows_native_keyring_store::Store>>);

impl CredentialManager {
    pub fn new() -> Self {
        let store = windows_native_keyring_store::Store::new()
            .inspect_err(|e| tracing::error!(error = %e, "Credential Manager unavailable"))
            .ok();
        Self(store)
    }

    fn entry(&self) -> Option<keyring_core::Entry> {
        self.0.as_ref()?.build(SERVICE, USER, None).ok()
    }
}

impl Default for CredentialManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStore for CredentialManager {
    fn get(&self) -> Option<String> {
        match self.entry()?.get_password() {
            Ok(token) => Some(token),
            Err(keyring_core::Error::NoEntry) => None,
            Err(e) => {
                tracing::warn!(error = %e, "couldn't read the saved sign-in");
                None
            }
        }
    }

    fn set(&self, token: &str) {
        if let Some(Err(e)) = self.entry().map(|e| e.set_password(token)) {
            tracing::warn!(error = %e, "couldn't save the sign-in");
        }
    }

    fn clear(&self) {
        match self.entry().map(|e| e.delete_credential()) {
            Some(Ok(())) | Some(Err(keyring_core::Error::NoEntry)) | None => {}
            Some(Err(e)) => tracing::warn!(error = %e, "couldn't remove the saved sign-in"),
        }
    }
}

/// For tests: forgets everything when the app closes.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl TokenStore for MemoryStore {
    fn get(&self) -> Option<String> {
        self.0.lock().unwrap().clone()
    }
    fn set(&self, token: &str) {
        *self.0.lock().unwrap() = Some(token.to_owned());
    }
    fn clear(&self) {
        *self.0.lock().unwrap() = None;
    }
}
