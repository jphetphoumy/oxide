use std::sync::{Arc, OnceLock, RwLock};

use anyhow::{Context, Result};

const SERVICE: &str = "oxide";

trait KeyStore: Send + Sync {
    fn set_secret(&self, name: &str, value: &str) -> Result<()>;
    fn get_secret(&self, name: &str) -> Result<Option<String>>;
    fn delete_secret(&self, name: &str) -> Result<()>;
}

struct SystemKeyStore;

pub fn save_access_token(token: &str) -> Result<()> {
    save_secret("access_token", token)
}

pub fn get_access_token() -> Result<Option<String>> {
    get_secret("access_token")
}

pub fn save_refresh_token(token: &str) -> Result<()> {
    save_secret("refresh_token", token)
}

pub fn get_refresh_token() -> Result<Option<String>> {
    get_secret("refresh_token")
}

pub fn save_workspace_id(id: &str) -> Result<()> {
    save_secret("workspace_id", id)
}

pub fn get_workspace_id() -> Result<Option<String>> {
    get_secret("workspace_id")
}

pub fn save_region(region: &str) -> Result<()> {
    save_secret("region", region)
}

pub fn get_region() -> Result<Option<String>> {
    get_secret("region")
}

pub fn clear_all() -> Result<()> {
    for name in ["access_token", "refresh_token", "workspace_id", "region"] {
        delete_secret(name)?;
    }
    Ok(())
}

fn save_secret(name: &str, value: &str) -> Result<()> {
    current_store().set_secret(name, value)
}

fn get_secret(name: &str) -> Result<Option<String>> {
    current_store().get_secret(name)
}

fn delete_secret(name: &str) -> Result<()> {
    current_store().delete_secret(name)
}

impl KeyStore for SystemKeyStore {
    fn set_secret(&self, name: &str, value: &str) -> Result<()> {
        entry(name)?
            .set_password(value)
            .with_context(|| format!("failed to store {name} in the system keyring"))?;
        Ok(())
    }

    fn get_secret(&self, name: &str) -> Result<Option<String>> {
        match entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => {
                Err(error).with_context(|| format!("failed to read {name} from the system keyring"))
            }
        }
    }

    fn delete_secret(&self, name: &str) -> Result<()> {
        match entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to delete {name} from the system keyring")),
        }
    }
}

fn entry(name: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, name)
        .with_context(|| format!("failed to create keyring entry for {name}"))
}

fn current_store() -> Arc<dyn KeyStore> {
    let store_lock = store_cell().get_or_init(|| RwLock::new(Arc::new(SystemKeyStore)));
    match store_lock.read() {
        Ok(store) => Arc::clone(&store),
        Err(_) => Arc::new(SystemKeyStore),
    }
}

fn store_cell() -> &'static OnceLock<RwLock<Arc<dyn KeyStore>>> {
    static STORE: OnceLock<RwLock<Arc<dyn KeyStore>>> = OnceLock::new();
    &STORE
}

#[cfg(test)]
fn set_store_for_tests(store: Arc<dyn KeyStore>) {
    let store_lock = store_cell().get_or_init(|| RwLock::new(Arc::new(SystemKeyStore)));
    if let Ok(mut current) = store_lock.write() {
        *current = store;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryKeyStore {
        values: Mutex<HashMap<String, String>>,
    }

    impl KeyStore for MemoryKeyStore {
        fn set_secret(&self, name: &str, value: &str) -> Result<()> {
            self.values
                .lock()
                .map_err(|error| anyhow::anyhow!("lock poisoned: {error}"))?
                .insert(name.to_string(), value.to_string());
            Ok(())
        }

        fn get_secret(&self, name: &str) -> Result<Option<String>> {
            Ok(self
                .values
                .lock()
                .map_err(|error| anyhow::anyhow!("lock poisoned: {error}"))?
                .get(name)
                .cloned())
        }

        fn delete_secret(&self, name: &str) -> Result<()> {
            self.values
                .lock()
                .map_err(|error| anyhow::anyhow!("lock poisoned: {error}"))?
                .remove(name);
            Ok(())
        }
    }

    #[test]
    fn storage_apis_round_trip_through_mock_store() {
        let _guard = test_lock().lock();
        set_store_for_tests(Arc::new(MemoryKeyStore::default()));

        assert!(save_access_token("token").is_ok());
        assert_eq!(get_access_token().ok(), Some(Some("token".to_string())));

        set_store_for_tests(Arc::new(SystemKeyStore));
    }

    #[test]
    fn clear_all_uses_mock_store() {
        let _guard = test_lock().lock();
        set_store_for_tests(Arc::new(MemoryKeyStore::default()));

        assert!(save_access_token("access").is_ok());
        assert!(save_refresh_token("refresh").is_ok());
        assert!(save_workspace_id("workspace").is_ok());
        assert!(save_region("us-central1").is_ok());
        assert!(clear_all().is_ok());
        assert_eq!(get_access_token().ok(), Some(None));
        assert_eq!(get_refresh_token().ok(), Some(None));
        assert_eq!(get_workspace_id().ok(), Some(None));
        assert_eq!(get_region().ok(), Some(None));

        set_store_for_tests(Arc::new(SystemKeyStore));
    }

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }
}
