use crate::{CredentialVault, VaultError};
use thiserror::Error;

#[derive(Clone)]
pub struct AccountSessionStore<V: CredentialVault> {
    vault: V,
}

impl<V: CredentialVault> AccountSessionStore<V> {
    pub fn new(vault: V) -> Self {
        Self { vault }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, AccountSessionError> {
        let secret = match self.vault.get_secret(&storage_name(key)?)? {
            Some(secret) => secret,
            None => return Ok(None),
        };
        String::from_utf8(secret)
            .map(Some)
            .map_err(|_| AccountSessionError::InvalidValue)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), AccountSessionError> {
        self.vault
            .set_secret(&storage_name(key)?, value.as_bytes())?;
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Result<(), AccountSessionError> {
        self.vault.delete_secret(&storage_name(key)?)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum AccountSessionError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("the account session storage key is invalid")]
    InvalidKey,
    #[error("the stored account session is invalid")]
    InvalidValue,
}

fn storage_name(key: &str) -> Result<String, AccountSessionError> {
    if key.is_empty()
        || key.len() > 256
        || !key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".:_-".contains(character))
    {
        return Err(AccountSessionError::InvalidKey);
    }
    Ok(format!("auth-storage:{key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    #[derive(Clone, Default)]
    struct MemoryVault(Arc<Mutex<HashMap<String, Vec<u8>>>>);

    impl CredentialVault for MemoryVault {
        fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError> {
            self.0
                .lock()
                .map_err(|_| VaultError::Unavailable)?
                .insert(name.to_owned(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError> {
            Ok(self
                .0
                .lock()
                .map_err(|_| VaultError::Unavailable)?
                .get(name)
                .cloned())
        }

        fn delete_secret(&self, name: &str) -> Result<(), VaultError> {
            self.0
                .lock()
                .map_err(|_| VaultError::Unavailable)?
                .remove(name);
            Ok(())
        }
    }

    #[test]
    fn account_session_round_trips_through_credential_vault() {
        let store = AccountSessionStore::new(MemoryVault::default());
        store
            .set("sb-project-auth-token", "sensitive-session")
            .expect("session should be stored");
        assert_eq!(
            store
                .get("sb-project-auth-token")
                .expect("session should be read"),
            Some("sensitive-session".to_owned())
        );
        store
            .remove("sb-project-auth-token")
            .expect("session should be removed");
        assert_eq!(
            store
                .get("sb-project-auth-token")
                .expect("missing session should be readable"),
            None
        );
    }
}
