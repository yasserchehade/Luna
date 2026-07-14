use crate::{CredentialVault, VaultError};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use thiserror::Error;

const CREDENTIAL_CHUNK_BYTES: usize = 2_048;
const MAX_CREDENTIAL_CHUNKS: usize = 64;
const CHUNKED_VALUE_PREFIX: &str = "luna-account-session:v1:";

#[derive(Clone)]
pub struct AccountSessionStore<V: CredentialVault> {
    vault: V,
}

impl<V: CredentialVault> AccountSessionStore<V> {
    pub fn new(vault: V) -> Self {
        Self { vault }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, AccountSessionError> {
        let name = storage_name(key)?;
        let secret = match self.vault.get_secret(&name)? {
            Some(secret) => secret,
            None => return Ok(None),
        };

        let secret = match ChunkManifest::parse(&secret)? {
            Some(manifest) => {
                let mut value = Vec::with_capacity(manifest.chunks * CREDENTIAL_CHUNK_BYTES);
                for index in 0..manifest.chunks {
                    let chunk = self
                        .vault
                        .get_secret(&chunk_name(&name, &manifest.generation, index))?
                        .ok_or(AccountSessionError::InvalidValue)?;
                    value.extend_from_slice(&chunk);
                }
                value
            }
            None => secret,
        };

        String::from_utf8(secret)
            .map(Some)
            .map_err(|_| AccountSessionError::InvalidValue)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), AccountSessionError> {
        let name = storage_name(key)?;
        let previous_manifest = self
            .vault
            .get_secret(&name)?
            .and_then(|secret| ChunkManifest::parse(&secret).ok().flatten());

        if value.len() <= CREDENTIAL_CHUNK_BYTES {
            self.vault.set_secret(&name, value.as_bytes())?;
            self.remove_chunks(&name, previous_manifest.as_ref());
            return Ok(());
        }

        let chunks = value.len().div_ceil(CREDENTIAL_CHUNK_BYTES);
        if chunks > MAX_CREDENTIAL_CHUNKS {
            return Err(AccountSessionError::InvalidValue);
        }

        let generation = new_generation()?;
        for (index, chunk) in value.as_bytes().chunks(CREDENTIAL_CHUNK_BYTES).enumerate() {
            if let Err(error) = self
                .vault
                .set_secret(&chunk_name(&name, &generation, index), chunk)
            {
                self.remove_chunks(
                    &name,
                    Some(&ChunkManifest {
                        generation,
                        chunks: index,
                    }),
                );
                return Err(error.into());
            }
        }

        let manifest = ChunkManifest { generation, chunks };
        if let Err(error) = self.vault.set_secret(&name, manifest.encode().as_bytes()) {
            self.remove_chunks(&name, Some(&manifest));
            return Err(error.into());
        }
        self.remove_chunks(&name, previous_manifest.as_ref());
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Result<(), AccountSessionError> {
        let name = storage_name(key)?;
        let manifest = self
            .vault
            .get_secret(&name)?
            .and_then(|secret| ChunkManifest::parse(&secret).ok().flatten());
        self.vault.delete_secret(&name)?;
        if let Some(manifest) = manifest {
            for index in 0..manifest.chunks {
                self.vault
                    .delete_secret(&chunk_name(&name, &manifest.generation, index))?;
            }
        }
        Ok(())
    }

    fn remove_chunks(&self, name: &str, manifest: Option<&ChunkManifest>) {
        if let Some(manifest) = manifest {
            for index in 0..manifest.chunks {
                let _ = self
                    .vault
                    .delete_secret(&chunk_name(name, &manifest.generation, index));
            }
        }
    }
}

#[derive(Debug)]
struct ChunkManifest {
    generation: String,
    chunks: usize,
}

impl ChunkManifest {
    fn encode(&self) -> String {
        format!("{CHUNKED_VALUE_PREFIX}{}:{}", self.generation, self.chunks)
    }

    fn parse(secret: &[u8]) -> Result<Option<Self>, AccountSessionError> {
        let value = match std::str::from_utf8(secret) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        let Some(manifest) = value.strip_prefix(CHUNKED_VALUE_PREFIX) else {
            return Ok(None);
        };
        let Some((generation, chunks)) = manifest.split_once(':') else {
            return Err(AccountSessionError::InvalidValue);
        };
        let chunks = chunks
            .parse::<usize>()
            .map_err(|_| AccountSessionError::InvalidValue)?;
        if generation.is_empty()
            || !generation
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
            || chunks == 0
            || chunks > MAX_CREDENTIAL_CHUNKS
        {
            return Err(AccountSessionError::InvalidValue);
        }
        Ok(Some(Self {
            generation: generation.to_owned(),
            chunks,
        }))
    }
}

fn new_generation() -> Result<String, AccountSessionError> {
    let mut bytes = [0_u8; 12];
    getrandom::fill(&mut bytes).map_err(|_| AccountSessionError::RandomnessUnavailable)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn chunk_name(name: &str, generation: &str, index: usize) -> String {
    format!("{name}:chunk:{generation}:{index}")
}

#[derive(Debug, Error)]
pub enum AccountSessionError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("the account session storage key is invalid")]
    InvalidKey,
    #[error("the stored account session is invalid")]
    InvalidValue,
    #[error("secure randomness is unavailable")]
    RandomnessUnavailable,
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

    #[derive(Clone)]
    struct MemoryVault {
        secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        max_secret_bytes: usize,
    }

    impl Default for MemoryVault {
        fn default() -> Self {
            Self {
                secrets: Arc::default(),
                max_secret_bytes: usize::MAX,
            }
        }
    }

    impl MemoryVault {
        fn with_limit(max_secret_bytes: usize) -> Self {
            Self {
                secrets: Arc::default(),
                max_secret_bytes,
            }
        }
    }

    impl CredentialVault for MemoryVault {
        fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError> {
            if secret.len() > self.max_secret_bytes {
                return Err(VaultError::Rejected);
            }
            self.secrets
                .lock()
                .map_err(|_| VaultError::Unavailable)?
                .insert(name.to_owned(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError> {
            Ok(self
                .secrets
                .lock()
                .map_err(|_| VaultError::Unavailable)?
                .get(name)
                .cloned())
        }

        fn delete_secret(&self, name: &str) -> Result<(), VaultError> {
            self.secrets
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

    #[test]
    fn oversized_supabase_session_is_chunked_below_windows_vault_limit() {
        let vault = MemoryVault::with_limit(2_560);
        let store = AccountSessionStore::new(vault.clone());
        let session = "s".repeat(2_716);

        store
            .set("sb-project-auth-token", &session)
            .expect("oversized session should be chunked");

        assert_eq!(
            store
                .get("sb-project-auth-token")
                .expect("chunked session should be readable"),
            Some(session)
        );
        assert!(vault
            .secrets
            .lock()
            .expect("vault should be readable")
            .values()
            .all(|secret| secret.len() <= CREDENTIAL_CHUNK_BYTES));
    }

    #[test]
    fn replacing_and_removing_chunked_session_cleans_up_chunks() {
        let vault = MemoryVault::with_limit(2_560);
        let store = AccountSessionStore::new(vault.clone());

        store
            .set("sb-project-auth-token", &"s".repeat(2_716))
            .expect("oversized session should be stored");
        store
            .set("sb-project-auth-token", "replacement")
            .expect("small replacement should be stored");
        assert_eq!(
            vault
                .secrets
                .lock()
                .expect("vault should be readable")
                .len(),
            1
        );

        store
            .remove("sb-project-auth-token")
            .expect("replacement should be removable");
        assert!(vault
            .secrets
            .lock()
            .expect("vault should be readable")
            .is_empty());
    }
}
