use age::{
    secrecy::{ExposeSecret, SecretString},
    x25519,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use bip39::Mnemonic;
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{Arc, Mutex},
};
use thiserror::Error;

const HOUSEHOLD_KEY_BYTES: usize = 32;
const STATE_NONCE_BYTES: usize = 24;
const PIN_SALT_BYTES: usize = 16;

pub trait CredentialVault: Clone + Send + Sync + 'static {
    fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError>;
    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError>;
    fn delete_secret(&self, name: &str) -> Result<(), VaultError>;
}

#[derive(Clone)]
pub struct OsCredentialVault {
    service: String,
}

impl OsCredentialVault {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, name: &str) -> Result<keyring::Entry, VaultError> {
        keyring::Entry::new(&self.service, name).map_err(|_| VaultError::Unavailable)
    }
}

impl CredentialVault for OsCredentialVault {
    fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError> {
        self.entry(name)?
            .set_secret(secret)
            .map_err(|_| VaultError::Rejected)
    }

    fn get_secret(&self, name: &str) -> Result<Option<Vec<u8>>, VaultError> {
        match self.entry(name)?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(VaultError::Rejected),
        }
    }

    fn delete_secret(&self, name: &str) -> Result<(), VaultError> {
        match self.entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(VaultError::Rejected),
        }
    }
}

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("the credential vault is unavailable")]
    Unavailable,
    #[error("the credential vault rejected the operation")]
    Rejected,
}

#[derive(Debug, Error)]
pub enum TrustedDeviceError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("this device does not hold the required Household key")]
    MissingHouseholdKey,
    #[error("this device has not completed Recovery Key confirmation")]
    UnconfirmedRecoveryKey,
    #[error("the Recovery Key is invalid")]
    InvalidRecoveryKey,
    #[error("the device PIN must contain at least six digits")]
    WeakDevicePin,
    #[error("the device PIN is incorrect")]
    InvalidDevicePin,
    #[error("the device unlock state is unavailable")]
    UnlockStateUnavailable,
    #[error("this Trusted Device is locked")]
    DeviceLocked,
    #[error("this beta Trusted Device must be re-enrolled before replacing its Recovery Key")]
    MissingDeviceAuthorizationKey,
    #[error("protected Household state could not be opened")]
    ProtectedStateRejected,
    #[error("trusted-device cryptography failed")]
    Cryptography,
}

#[derive(Debug)]
pub struct FirstDeviceEnrollment {
    pub device_public_key: String,
    pub device_authorization_public_key: [u8; 32],
    pub device_key_envelope: Vec<u8>,
    pub recovery_key: String,
    pub recovery_envelope: Vec<u8>,
    pub recovery_verification_key: [u8; 32],
}

#[derive(Debug)]
pub struct RecoveryKeyReplacement {
    pub recovery_key: String,
    pub recovery_envelope: Vec<u8>,
    pub recovery_verification_key: [u8; 32],
    pub device_authorization_signature: [u8; 64],
}

#[derive(Debug)]
pub struct RecoveredDeviceEnrollment {
    pub device_public_key: String,
    pub device_authorization_public_key: [u8; 32],
    pub device_key_envelope: Vec<u8>,
    pub recovery_authorization_signature: [u8; 64],
}

#[derive(Debug)]
pub struct HouseholdKeyRotation {
    pub device_envelopes: Vec<RotatedDeviceEnvelope>,
    pub recovery_envelope: Vec<u8>,
    pub recovery_authorization_signature: [u8; 64],
}

#[derive(Debug)]
pub struct RotatedDeviceEnvelope {
    pub device_public_key: String,
    pub key_envelope: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProtectedHouseholdState {
    nonce: [u8; STATE_NONCE_BYTES],
    ciphertext: Vec<u8>,
}

#[derive(Clone)]
pub struct TrustedDeviceManager<V: CredentialVault> {
    vault: V,
    unlocked_households: Arc<Mutex<HashSet<String>>>,
}

impl<V: CredentialVault> TrustedDeviceManager<V> {
    pub fn new(vault: V) -> Self {
        Self {
            vault,
            unlocked_households: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Returns a clone of the OS-backed credential vault handle. Callers use
    /// this only for narrow external-boundary credentials; protected Household
    /// state still goes through this manager's encryption boundary.
    pub fn vault(&self) -> V {
        self.vault.clone()
    }

    pub fn is_current_device_trusted(
        &self,
        household_id: &str,
    ) -> Result<bool, TrustedDeviceError> {
        Ok(self.require_confirmed(household_id).is_ok()
            && self.household_key(household_id).is_ok()
            && self.current_key_epoch(household_id).is_ok()
            && self
                .vault
                .get_secret(&device_pin_name(household_id))?
                .is_some())
    }

    pub fn is_current_device_unlocked(
        &self,
        household_id: &str,
    ) -> Result<bool, TrustedDeviceError> {
        if !self.is_current_device_trusted(household_id)? {
            return Ok(false);
        }
        self.unlocked_households
            .lock()
            .map(|households| households.contains(household_id))
            .map_err(|_| TrustedDeviceError::UnlockStateUnavailable)
    }

    pub fn current_device_public_key(
        &self,
        household_id: &str,
    ) -> Result<String, TrustedDeviceError> {
        Ok(self.device_identity(household_id)?.to_public().to_string())
    }

    pub fn current_key_epoch(&self, household_id: &str) -> Result<u32, TrustedDeviceError> {
        let value = self
            .vault
            .get_secret(&key_epoch_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        let bytes: [u8; 4] = value
            .try_into()
            .map_err(|_| TrustedDeviceError::Cryptography)?;
        let epoch = u32::from_le_bytes(bytes);
        if epoch == 0 {
            return Err(TrustedDeviceError::Cryptography);
        }
        Ok(epoch)
    }

    pub fn set_current_key_epoch(
        &self,
        household_id: &str,
        key_epoch: u32,
    ) -> Result<(), TrustedDeviceError> {
        if key_epoch == 0 {
            return Err(TrustedDeviceError::Cryptography);
        }
        self.vault
            .set_secret(&key_epoch_name(household_id), &key_epoch.to_le_bytes())?;
        Ok(())
    }

    pub fn configure_device_pin(
        &self,
        household_id: &str,
        pin: &str,
    ) -> Result<(), TrustedDeviceError> {
        validate_device_pin(pin)?;
        self.require_confirmed(household_id)?;
        self.household_key(household_id)?;
        let salt = SaltString::encode_b64(&random_bytes::<PIN_SALT_BYTES>()?)
            .map_err(|_| TrustedDeviceError::Cryptography)?;
        let hash = Argon2::default()
            .hash_password(pin.as_bytes(), &salt)
            .map_err(|_| TrustedDeviceError::Cryptography)?
            .to_string();
        self.vault
            .set_secret(&device_pin_name(household_id), hash.as_bytes())?;
        self.mark_unlocked(household_id)
    }

    pub fn unlock_device(&self, household_id: &str, pin: &str) -> Result<(), TrustedDeviceError> {
        let encoded = self
            .vault
            .get_secret(&device_pin_name(household_id))?
            .ok_or(TrustedDeviceError::InvalidDevicePin)?;
        let encoded = String::from_utf8(encoded).map_err(|_| TrustedDeviceError::Cryptography)?;
        let hash = PasswordHash::new(&encoded).map_err(|_| TrustedDeviceError::Cryptography)?;
        Argon2::default()
            .verify_password(pin.as_bytes(), &hash)
            .map_err(|_| TrustedDeviceError::InvalidDevicePin)?;
        self.mark_unlocked(household_id)
    }

    pub fn lock_device(&self, household_id: &str) {
        if let Ok(mut households) = self.unlocked_households.lock() {
            households.remove(household_id);
        }
    }

    pub fn forget_current_device(&self, household_id: &str) -> Result<(), TrustedDeviceError> {
        self.vault
            .delete_secret(&device_identity_name(household_id))?;
        self.vault
            .delete_secret(&household_key_name(household_id))?;
        self.vault
            .delete_secret(&trust_confirmation_name(household_id))?;
        self.vault.delete_secret(&device_pin_name(household_id))?;
        self.vault.delete_secret(&key_epoch_name(household_id))?;
        self.vault
            .delete_secret(&pending_device_identity_name(household_id))?;
        self.vault
            .delete_secret(&pending_device_authorization_key_name(household_id))?;
        self.vault
            .delete_secret(&pending_household_key_name(household_id))?;
        self.vault
            .delete_secret(&pending_rotation_key_name(household_id))?;
        self.vault
            .delete_secret(&device_authorization_key_name(household_id))?;
        self.lock_device(household_id);
        Ok(())
    }

    pub fn enrol_first_device(
        &self,
        household_id: &str,
    ) -> Result<FirstDeviceEnrollment, TrustedDeviceError> {
        let device_identity = x25519::Identity::generate();
        let device_public_key = device_identity.to_public().to_string();
        let device_secret = device_identity.to_string();
        let device_authorization_key = SigningKey::from_bytes(&random_bytes::<32>()?);
        let device_authorization_public_key = device_authorization_key.verifying_key().to_bytes();
        let household_key = random_bytes::<HOUSEHOLD_KEY_BYTES>()?;
        let recovery_key = Mnemonic::from_entropy(&random_bytes::<HOUSEHOLD_KEY_BYTES>()?)
            .map_err(|_| TrustedDeviceError::Cryptography)?
            .to_string();
        let recovery_envelope = encrypt_for_recovery(&recovery_key, &household_key)?;
        let recovery_verification_key = recovery_signing_key(&recovery_key)?
            .verifying_key()
            .to_bytes();
        let device_key_envelope = age::encrypt(&device_identity.to_public(), &household_key)
            .map_err(|_| TrustedDeviceError::Cryptography)?;

        self.vault.set_secret(
            &device_identity_name(household_id),
            device_secret.expose_secret().as_bytes(),
        )?;
        self.vault
            .set_secret(&household_key_name(household_id), &household_key)?;
        self.vault.set_secret(
            &device_authorization_key_name(household_id),
            &device_authorization_key.to_bytes(),
        )?;

        Ok(FirstDeviceEnrollment {
            device_public_key,
            device_authorization_public_key,
            device_key_envelope,
            recovery_key,
            recovery_envelope,
            recovery_verification_key,
        })
    }

    pub fn prepare_recovery_key_replacement(
        &self,
        household_id: &str,
        current_key_epoch: u32,
        current_recovery_verification_key: &[u8; 32],
    ) -> Result<RecoveryKeyReplacement, TrustedDeviceError> {
        self.require_unlocked(household_id)?;
        if self.current_key_epoch(household_id)? != current_key_epoch {
            return Err(TrustedDeviceError::Cryptography);
        }
        let household_key = self.household_key(household_id)?;
        let recovery_key = Mnemonic::from_entropy(&random_bytes::<HOUSEHOLD_KEY_BYTES>()?)
            .map_err(|_| TrustedDeviceError::Cryptography)?
            .to_string();
        let recovery_envelope = encrypt_for_recovery(&recovery_key, &household_key)?;
        let recovery_verification_key = recovery_signing_key(&recovery_key)?
            .verifying_key()
            .to_bytes();
        let device_public_key = self.current_device_public_key(household_id)?;
        let device_authorization_signature = self
            .device_authorization_signing_key(household_id)?
            .sign(
                replace_recovery_key_authorization(
                    household_id,
                    current_key_epoch,
                    &device_public_key,
                    current_recovery_verification_key,
                    &recovery_envelope,
                    &recovery_verification_key,
                )
                .as_bytes(),
            )
            .to_bytes();

        Ok(RecoveryKeyReplacement {
            recovery_key,
            recovery_envelope,
            recovery_verification_key,
            device_authorization_signature,
        })
    }

    pub fn confirm_recovery_key_replacement(
        &self,
        household_id: &str,
        recovery_key: &str,
        recovery_envelope: &[u8],
    ) -> Result<(), TrustedDeviceError> {
        self.require_unlocked(household_id)?;
        let recovered = decrypt_recovery_envelope(recovery_key, recovery_envelope)?;
        if recovered != self.household_key(household_id)? {
            return Err(TrustedDeviceError::InvalidRecoveryKey);
        }
        Ok(())
    }

    pub fn recover_device(
        &self,
        household_id: &str,
        recovery_key: &str,
        recovery_envelope: &[u8],
        key_epoch: u32,
    ) -> Result<RecoveredDeviceEnrollment, TrustedDeviceError> {
        let household_key = decrypt_recovery_envelope(recovery_key, recovery_envelope)?;
        if household_key.len() != HOUSEHOLD_KEY_BYTES {
            return Err(TrustedDeviceError::InvalidRecoveryKey);
        }

        let device_identity = x25519::Identity::generate();
        let device_public_key = device_identity.to_public().to_string();
        let device_secret = device_identity.to_string();
        let device_authorization_key = SigningKey::from_bytes(&random_bytes::<32>()?);
        let device_authorization_public_key = device_authorization_key.verifying_key().to_bytes();
        let device_key_envelope = age::encrypt(&device_identity.to_public(), &household_key)
            .map_err(|_| TrustedDeviceError::Cryptography)?;
        let recovery_authorization_signature = recovery_signing_key(recovery_key)?
            .sign(
                recover_device_authorization(
                    household_id,
                    key_epoch,
                    &device_public_key,
                    &device_authorization_public_key,
                    &device_key_envelope,
                )
                .as_bytes(),
            )
            .to_bytes();

        self.vault.set_secret(
            &pending_device_identity_name(household_id),
            device_secret.expose_secret().as_bytes(),
        )?;
        self.vault
            .set_secret(&pending_household_key_name(household_id), &household_key)?;
        self.vault.set_secret(
            &pending_device_authorization_key_name(household_id),
            &device_authorization_key.to_bytes(),
        )?;

        Ok(RecoveredDeviceEnrollment {
            device_public_key,
            device_authorization_public_key,
            device_key_envelope,
            recovery_authorization_signature,
        })
    }

    pub fn finalize_recovered_device(
        &self,
        household_id: &str,
        key_epoch: u32,
    ) -> Result<(), TrustedDeviceError> {
        let identity = self
            .vault
            .get_secret(&pending_device_identity_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        let household_key = self
            .vault
            .get_secret(&pending_household_key_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        let device_authorization_key = self
            .vault
            .get_secret(&pending_device_authorization_key_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        self.vault
            .set_secret(&device_identity_name(household_id), &identity)?;
        self.vault
            .set_secret(&household_key_name(household_id), &household_key)?;
        self.vault.set_secret(
            &device_authorization_key_name(household_id),
            &device_authorization_key,
        )?;
        self.vault
            .set_secret(&trust_confirmation_name(household_id), &[1])?;
        self.set_current_key_epoch(household_id, key_epoch)?;
        self.vault
            .delete_secret(&pending_device_identity_name(household_id))?;
        self.vault
            .delete_secret(&pending_household_key_name(household_id))?;
        self.vault
            .delete_secret(&pending_device_authorization_key_name(household_id))?;
        Ok(())
    }

    pub fn confirm_recovery_key(
        &self,
        household_id: &str,
        recovery_key: &str,
        recovery_envelope: &[u8],
    ) -> Result<(), TrustedDeviceError> {
        let recovered = decrypt_recovery_envelope(recovery_key, recovery_envelope)?;
        let stored = self.household_key(household_id)?;
        if recovered != stored {
            return Err(TrustedDeviceError::InvalidRecoveryKey);
        }
        self.vault
            .set_secret(&trust_confirmation_name(household_id), &[1])?;
        Ok(())
    }

    pub fn prepare_household_key_rotation_after_revocation(
        &self,
        household_id: &str,
        recovery_key: &str,
        current_recovery_envelope: &[u8],
        retained_device_public_keys: &[String],
        current_key_epoch: u32,
        revoked_device_id: &str,
    ) -> Result<HouseholdKeyRotation, TrustedDeviceError> {
        self.require_unlocked(household_id)?;
        let current_key = self.household_key(household_id)?;
        let recovered = decrypt_recovery_envelope(recovery_key, current_recovery_envelope)?;
        if recovered != current_key {
            return Err(TrustedDeviceError::InvalidRecoveryKey);
        }

        if retained_device_public_keys.is_empty() {
            return Err(TrustedDeviceError::Cryptography);
        }
        let rotated_key = random_bytes::<HOUSEHOLD_KEY_BYTES>()?;
        let mut device_envelopes = Vec::with_capacity(retained_device_public_keys.len());
        for public_key in retained_device_public_keys {
            let recipient = x25519::Recipient::from_str(public_key)
                .map_err(|_| TrustedDeviceError::Cryptography)?;
            let key_envelope = age::encrypt(&recipient, &rotated_key)
                .map_err(|_| TrustedDeviceError::Cryptography)?;
            device_envelopes.push(RotatedDeviceEnvelope {
                device_public_key: public_key.clone(),
                key_envelope,
            });
        }
        let recovery_envelope = encrypt_for_recovery(recovery_key, &rotated_key)?;
        let current_device_public_key = self.current_device_public_key(household_id)?;
        let recovery_authorization_signature = recovery_signing_key(recovery_key)?
            .sign(
                revoke_device_authorization(
                    household_id,
                    current_key_epoch,
                    revoked_device_id,
                    &current_device_public_key,
                    &recovery_envelope,
                    &device_envelopes,
                )
                .as_bytes(),
            )
            .to_bytes();

        self.vault
            .set_secret(&pending_rotation_key_name(household_id), &rotated_key)?;

        Ok(HouseholdKeyRotation {
            device_envelopes,
            recovery_envelope,
            recovery_authorization_signature,
        })
    }

    pub fn finalize_household_key_rotation(
        &self,
        household_id: &str,
        key_epoch: u32,
    ) -> Result<(), TrustedDeviceError> {
        let key = self
            .vault
            .get_secret(&pending_rotation_key_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        self.vault
            .set_secret(&household_key_name(household_id), &key)?;
        self.set_current_key_epoch(household_id, key_epoch)?;
        self.vault
            .delete_secret(&pending_rotation_key_name(household_id))?;
        Ok(())
    }

    pub fn discard_household_key_rotation(
        &self,
        household_id: &str,
    ) -> Result<(), TrustedDeviceError> {
        self.vault
            .delete_secret(&pending_rotation_key_name(household_id))?;
        Ok(())
    }

    pub fn apply_rotated_device_envelope(
        &self,
        household_id: &str,
        key_envelope: &[u8],
        key_epoch: u32,
    ) -> Result<(), TrustedDeviceError> {
        self.require_unlocked(household_id)?;
        let identity = self.device_identity(household_id)?;
        let key = age::decrypt(&identity, key_envelope)
            .map_err(|_| TrustedDeviceError::ProtectedStateRejected)?;
        if key.len() != HOUSEHOLD_KEY_BYTES {
            return Err(TrustedDeviceError::ProtectedStateRejected);
        }
        self.vault
            .set_secret(&household_key_name(household_id), &key)?;
        self.set_current_key_epoch(household_id, key_epoch)?;
        Ok(())
    }

    pub fn protect_household_state(
        &self,
        household_id: &str,
        plaintext: &[u8],
    ) -> Result<ProtectedHouseholdState, TrustedDeviceError> {
        self.require_unlocked(household_id)?;
        let key = self.household_key(household_id)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&key)
            .map_err(|_| TrustedDeviceError::Cryptography)?;
        let nonce = random_bytes::<STATE_NONCE_BYTES>()?;
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: household_id.as_bytes(),
                },
            )
            .map_err(|_| TrustedDeviceError::Cryptography)?;
        Ok(ProtectedHouseholdState { nonce, ciphertext })
    }

    pub fn open_household_state(
        &self,
        household_id: &str,
        protected: &ProtectedHouseholdState,
    ) -> Result<Vec<u8>, TrustedDeviceError> {
        self.require_unlocked(household_id)?;
        let key = self.household_key(household_id)?;
        let cipher = XChaCha20Poly1305::new_from_slice(&key)
            .map_err(|_| TrustedDeviceError::ProtectedStateRejected)?;
        cipher
            .decrypt(
                XNonce::from_slice(&protected.nonce),
                Payload {
                    msg: &protected.ciphertext,
                    aad: household_id.as_bytes(),
                },
            )
            .map_err(|_| TrustedDeviceError::ProtectedStateRejected)
    }

    fn require_confirmed(&self, household_id: &str) -> Result<(), TrustedDeviceError> {
        match self
            .vault
            .get_secret(&trust_confirmation_name(household_id))?
        {
            Some(value) if value == [1] => Ok(()),
            _ => Err(TrustedDeviceError::UnconfirmedRecoveryKey),
        }
    }

    fn household_key(&self, household_id: &str) -> Result<Vec<u8>, TrustedDeviceError> {
        let key = self
            .vault
            .get_secret(&household_key_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        if key.len() != HOUSEHOLD_KEY_BYTES {
            return Err(TrustedDeviceError::MissingHouseholdKey);
        }
        Ok(key)
    }

    fn device_identity(&self, household_id: &str) -> Result<x25519::Identity, TrustedDeviceError> {
        let secret = self
            .vault
            .get_secret(&device_identity_name(household_id))?
            .ok_or(TrustedDeviceError::MissingHouseholdKey)?;
        let text = String::from_utf8(secret).map_err(|_| TrustedDeviceError::Cryptography)?;
        x25519::Identity::from_str(&text).map_err(|_| TrustedDeviceError::Cryptography)
    }

    fn device_authorization_signing_key(
        &self,
        household_id: &str,
    ) -> Result<SigningKey, TrustedDeviceError> {
        let secret = self
            .vault
            .get_secret(&device_authorization_key_name(household_id))?
            .ok_or(TrustedDeviceError::MissingDeviceAuthorizationKey)?;
        let bytes: [u8; 32] = secret
            .try_into()
            .map_err(|_| TrustedDeviceError::Cryptography)?;
        Ok(SigningKey::from_bytes(&bytes))
    }

    fn require_unlocked(&self, household_id: &str) -> Result<(), TrustedDeviceError> {
        if self.is_current_device_unlocked(household_id)? {
            Ok(())
        } else {
            Err(TrustedDeviceError::DeviceLocked)
        }
    }

    fn mark_unlocked(&self, household_id: &str) -> Result<(), TrustedDeviceError> {
        self.unlocked_households
            .lock()
            .map_err(|_| TrustedDeviceError::UnlockStateUnavailable)?
            .insert(household_id.to_owned());
        Ok(())
    }
}

fn validate_device_pin(pin: &str) -> Result<(), TrustedDeviceError> {
    if pin.len() >= 6 && pin.chars().all(|character| character.is_ascii_digit()) {
        Ok(())
    } else {
        Err(TrustedDeviceError::WeakDevicePin)
    }
}

fn encrypt_for_recovery(
    recovery_key: &str,
    household_key: &[u8],
) -> Result<Vec<u8>, TrustedDeviceError> {
    let passphrase = SecretString::from(recovery_key.to_owned());
    let recipient = age::scrypt::Recipient::new(passphrase);
    age::encrypt(&recipient, household_key).map_err(|_| TrustedDeviceError::Cryptography)
}

fn decrypt_recovery_envelope(
    recovery_key: &str,
    recovery_envelope: &[u8],
) -> Result<Vec<u8>, TrustedDeviceError> {
    let passphrase = SecretString::from(recovery_key.to_owned());
    let identity = age::scrypt::Identity::new(passphrase);
    age::decrypt(&identity, recovery_envelope).map_err(|_| TrustedDeviceError::InvalidRecoveryKey)
}

fn recovery_signing_key(recovery_key: &str) -> Result<SigningKey, TrustedDeviceError> {
    let entropy = Mnemonic::parse(recovery_key)
        .map_err(|_| TrustedDeviceError::InvalidRecoveryKey)?
        .to_entropy();
    let seed: [u8; 32] = entropy
        .try_into()
        .map_err(|_| TrustedDeviceError::InvalidRecoveryKey)?;
    Ok(SigningKey::from_bytes(&seed))
}

fn recover_device_authorization(
    household_id: &str,
    key_epoch: u32,
    device_public_key: &str,
    device_authorization_public_key: &[u8; 32],
    device_key_envelope: &[u8],
) -> String {
    canonical_authorization(
        "luna:recover-device:v2:",
        [
            household_id.to_owned(),
            key_epoch.to_string(),
            device_public_key.to_owned(),
            BASE64.encode(device_authorization_public_key),
            BASE64.encode(device_key_envelope),
        ],
    )
}

fn revoke_device_authorization(
    household_id: &str,
    key_epoch: u32,
    revoked_device_id: &str,
    current_device_public_key: &str,
    recovery_envelope: &[u8],
    device_envelopes: &[RotatedDeviceEnvelope],
) -> String {
    let mut sorted_envelopes = device_envelopes.iter().collect::<Vec<_>>();
    sorted_envelopes.sort_by(|left, right| left.device_public_key.cmp(&right.device_public_key));
    let mut fields = vec![
        household_id.to_owned(),
        key_epoch.to_string(),
        revoked_device_id.to_owned(),
        current_device_public_key.to_owned(),
        BASE64.encode(recovery_envelope),
        sorted_envelopes.len().to_string(),
    ];
    for envelope in sorted_envelopes {
        fields.push(envelope.device_public_key.clone());
        fields.push(BASE64.encode(&envelope.key_envelope));
    }
    canonical_authorization("luna:revoke-device:v2:", fields)
}

fn replace_recovery_key_authorization(
    household_id: &str,
    key_epoch: u32,
    current_device_public_key: &str,
    current_recovery_verification_key: &[u8; 32],
    recovery_envelope: &[u8],
    recovery_verification_key: &[u8; 32],
) -> String {
    canonical_authorization(
        "luna:replace-recovery-key:v1:",
        [
            household_id.to_owned(),
            key_epoch.to_string(),
            current_device_public_key.to_owned(),
            BASE64.encode(current_recovery_verification_key),
            BASE64.encode(recovery_envelope),
            BASE64.encode(recovery_verification_key),
        ],
    )
}

fn canonical_authorization(
    domain_separator: &str,
    fields: impl IntoIterator<Item = String>,
) -> String {
    let mut message = domain_separator.to_owned();
    for field in fields {
        message.push_str(&field.len().to_string());
        message.push(':');
        message.push_str(&field);
    }
    message
}

fn random_bytes<const N: usize>() -> Result<[u8; N], TrustedDeviceError> {
    let mut bytes = [0_u8; N];
    getrandom::fill(&mut bytes).map_err(|_| TrustedDeviceError::Cryptography)?;
    Ok(bytes)
}

fn device_identity_name(household_id: &str) -> String {
    format!("household:{household_id}:device-identity")
}

fn device_authorization_key_name(household_id: &str) -> String {
    format!("household:{household_id}:device-authorization-key")
}

fn household_key_name(household_id: &str) -> String {
    format!("household:{household_id}:memory-key")
}

fn trust_confirmation_name(household_id: &str) -> String {
    format!("household:{household_id}:recovery-confirmed")
}

fn device_pin_name(household_id: &str) -> String {
    format!("household:{household_id}:device-pin")
}

fn pending_device_identity_name(household_id: &str) -> String {
    format!("household:{household_id}:pending-device-identity")
}

fn pending_device_authorization_key_name(household_id: &str) -> String {
    format!("household:{household_id}:pending-device-authorization-key")
}

fn pending_household_key_name(household_id: &str) -> String {
    format!("household:{household_id}:pending-memory-key")
}

fn pending_rotation_key_name(household_id: &str) -> String {
    format!("household:{household_id}:pending-rotation-key")
}

fn key_epoch_name(household_id: &str) -> String {
    format!("household:{household_id}:key-epoch")
}
