mod account_session;
mod settings;
mod trusted_device;

pub use account_session::{AccountSessionError, AccountSessionStore};
pub use settings::SettingsStore;
pub use trusted_device::{
    CredentialVault, FirstDeviceEnrollment, HouseholdKeyRotation, OsCredentialVault,
    ProtectedHouseholdState, RecoveredDeviceEnrollment, RotatedDeviceEnvelope, TrustedDeviceError,
    TrustedDeviceManager, VaultError,
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use tauri::{Manager, State};

#[cfg(feature = "e2e")]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[cfg(not(feature = "e2e"))]
type DeviceManager = TrustedDeviceManager<OsCredentialVault>;
#[cfg(not(feature = "e2e"))]
type AccountSessionManager = AccountSessionStore<OsCredentialVault>;
#[cfg(feature = "e2e")]
type DeviceManager = TrustedDeviceManager<E2eCredentialVault>;
#[cfg(feature = "e2e")]
type AccountSessionManager = AccountSessionStore<E2eCredentialVault>;

#[cfg(feature = "e2e")]
#[derive(Clone, Default)]
struct E2eCredentialVault {
    secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

#[cfg(feature = "e2e")]
impl CredentialVault for E2eCredentialVault {
    fn set_secret(&self, name: &str, secret: &[u8]) -> Result<(), VaultError> {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceEnrollmentResponse {
    device_public_key: String,
    device_authorization_public_key: String,
    device_key_envelope: String,
    recovery_key: String,
    recovery_envelope: String,
    recovery_verification_key: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveredDeviceResponse {
    device_public_key: String,
    device_authorization_public_key: String,
    device_key_envelope: String,
    recovery_authorization_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryKeyReplacementResponse {
    recovery_key: String,
    recovery_envelope: String,
    recovery_verification_key: String,
    device_authorization_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RotatedDeviceEnvelopeResponse {
    device_public_key: String,
    key_envelope: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HouseholdKeyRotationResponse {
    device_envelopes: Vec<RotatedDeviceEnvelopeResponse>,
    recovery_envelope: String,
    recovery_authorization_signature: String,
}

#[tauri::command]
fn get_account_session_item(
    store: State<'_, AccountSessionManager>,
    key: String,
) -> Result<Option<String>, String> {
    store.get(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_account_session_item(
    store: State<'_, AccountSessionManager>,
    key: String,
    value: String,
) -> Result<(), String> {
    store.set(&key, &value).map_err(|error| error.to_string())
}

#[tauri::command]
fn remove_account_session_item(
    store: State<'_, AccountSessionManager>,
    key: String,
) -> Result<(), String> {
    store.remove(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn get_setting(store: State<'_, SettingsStore>, key: String) -> Result<Option<String>, String> {
    store.get(&key).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_setting(store: State<'_, SettingsStore>, key: String, value: String) -> Result<(), String> {
    store.set(&key, &value).map_err(|error| error.to_string())
}

#[tauri::command]
fn is_current_device_trusted(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<bool, String> {
    manager
        .is_current_device_trusted(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn is_current_device_unlocked(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<bool, String> {
    manager
        .is_current_device_unlocked(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn current_device_public_key(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<String, String> {
    manager
        .current_device_public_key(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn current_key_epoch(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<u32, String> {
    manager
        .current_key_epoch(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_current_key_epoch(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_epoch: u32,
) -> Result<(), String> {
    manager
        .set_current_key_epoch(&household_id, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn configure_device_pin(
    manager: State<'_, DeviceManager>,
    household_id: String,
    pin: String,
) -> Result<(), String> {
    manager
        .configure_device_pin(&household_id, &pin)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn unlock_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
    pin: String,
) -> Result<(), String> {
    manager
        .unlock_device(&household_id, &pin)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn lock_device(manager: State<'_, DeviceManager>, household_id: String) {
    manager.lock_device(&household_id);
}

#[tauri::command]
fn forget_current_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<(), String> {
    manager
        .forget_current_device(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn enrol_first_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<DeviceEnrollmentResponse, String> {
    let enrollment = manager
        .enrol_first_device(&household_id)
        .map_err(|error| error.to_string())?;
    Ok(DeviceEnrollmentResponse {
        device_public_key: enrollment.device_public_key,
        device_authorization_public_key: BASE64.encode(enrollment.device_authorization_public_key),
        device_key_envelope: BASE64.encode(enrollment.device_key_envelope),
        recovery_key: enrollment.recovery_key,
        recovery_envelope: BASE64.encode(enrollment.recovery_envelope),
        recovery_verification_key: BASE64.encode(enrollment.recovery_verification_key),
    })
}

#[tauri::command]
fn confirm_recovery_key(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
) -> Result<(), String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The Recovery Key envelope is invalid.".to_owned())?;
    manager
        .confirm_recovery_key(&household_id, &recovery_key, &envelope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn recover_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
    key_epoch: u32,
) -> Result<RecoveredDeviceResponse, String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The Recovery Key envelope is invalid.".to_owned())?;
    let recovered = manager
        .recover_device(&household_id, &recovery_key, &envelope, key_epoch)
        .map_err(|error| error.to_string())?;
    Ok(RecoveredDeviceResponse {
        device_public_key: recovered.device_public_key,
        device_authorization_public_key: BASE64.encode(recovered.device_authorization_public_key),
        device_key_envelope: BASE64.encode(recovered.device_key_envelope),
        recovery_authorization_signature: BASE64.encode(recovered.recovery_authorization_signature),
    })
}

#[tauri::command]
fn prepare_recovery_key_replacement(
    manager: State<'_, DeviceManager>,
    household_id: String,
    current_key_epoch: u32,
    current_recovery_verification_key: String,
) -> Result<RecoveryKeyReplacementResponse, String> {
    let current_recovery_verification_key: [u8; 32] = BASE64
        .decode(current_recovery_verification_key)
        .map_err(|_| "The current Recovery Key verifier is invalid.".to_owned())?
        .try_into()
        .map_err(|_| "The current Recovery Key verifier is invalid.".to_owned())?;
    let replacement = manager
        .prepare_recovery_key_replacement(
            &household_id,
            current_key_epoch,
            &current_recovery_verification_key,
        )
        .map_err(|error| error.to_string())?;
    Ok(RecoveryKeyReplacementResponse {
        recovery_key: replacement.recovery_key,
        recovery_envelope: BASE64.encode(replacement.recovery_envelope),
        recovery_verification_key: BASE64.encode(replacement.recovery_verification_key),
        device_authorization_signature: BASE64.encode(replacement.device_authorization_signature),
    })
}

#[tauri::command]
fn confirm_recovery_key_replacement(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
) -> Result<(), String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The replacement Recovery Key envelope is invalid.".to_owned())?;
    manager
        .confirm_recovery_key_replacement(&household_id, &recovery_key, &envelope)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn finalize_recovered_device(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_epoch: u32,
) -> Result<(), String> {
    manager
        .finalize_recovered_device(&household_id, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn prepare_household_key_rotation(
    manager: State<'_, DeviceManager>,
    household_id: String,
    recovery_key: String,
    recovery_envelope: String,
    retained_device_public_keys: Vec<String>,
    current_key_epoch: u32,
    revoked_device_id: String,
) -> Result<HouseholdKeyRotationResponse, String> {
    let envelope = BASE64
        .decode(recovery_envelope)
        .map_err(|_| "The Recovery Key envelope is invalid.".to_owned())?;
    let rotation = manager
        .prepare_household_key_rotation_after_revocation(
            &household_id,
            &recovery_key,
            &envelope,
            &retained_device_public_keys,
            current_key_epoch,
            &revoked_device_id,
        )
        .map_err(|error| error.to_string())?;
    Ok(HouseholdKeyRotationResponse {
        device_envelopes: rotation
            .device_envelopes
            .into_iter()
            .map(|device| RotatedDeviceEnvelopeResponse {
                device_public_key: device.device_public_key,
                key_envelope: BASE64.encode(device.key_envelope),
            })
            .collect(),
        recovery_envelope: BASE64.encode(rotation.recovery_envelope),
        recovery_authorization_signature: BASE64.encode(rotation.recovery_authorization_signature),
    })
}

#[tauri::command]
fn finalize_household_key_rotation(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_epoch: u32,
) -> Result<(), String> {
    manager
        .finalize_household_key_rotation(&household_id, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn discard_household_key_rotation(
    manager: State<'_, DeviceManager>,
    household_id: String,
) -> Result<(), String> {
    manager
        .discard_household_key_rotation(&household_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_rotated_device_envelope(
    manager: State<'_, DeviceManager>,
    household_id: String,
    key_envelope: String,
    key_epoch: u32,
) -> Result<(), String> {
    let envelope = BASE64
        .decode(key_envelope)
        .map_err(|_| "The Trusted Device key envelope is invalid.".to_owned())?;
    manager
        .apply_rotated_device_envelope(&household_id, &envelope, key_epoch)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn protect_household_state(
    manager: State<'_, DeviceManager>,
    household_id: String,
    plaintext: String,
) -> Result<ProtectedHouseholdState, String> {
    manager
        .protect_household_state(&household_id, plaintext.as_bytes())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_household_state(
    manager: State<'_, DeviceManager>,
    household_id: String,
    protected: ProtectedHouseholdState,
) -> Result<String, String> {
    let plaintext = manager
        .open_household_state(&household_id, &protected)
        .map_err(|error| error.to_string())?;
    String::from_utf8(plaintext).map_err(|_| "Protected Household state is not UTF-8.".to_owned())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(all(debug_assertions, feature = "e2e"))]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    let builder = builder.setup(|app| {
        let application_data = app.path().app_data_dir()?;
        std::fs::create_dir_all(&application_data)?;
        let settings = SettingsStore::open(application_data.join("luna.db"))?;
        app.manage(settings);
        #[cfg(not(feature = "e2e"))]
        app.manage(TrustedDeviceManager::new(OsCredentialVault::new(
            "app.luna.household",
        )));
        #[cfg(not(feature = "e2e"))]
        app.manage(AccountSessionStore::new(OsCredentialVault::new(
            "app.luna.account",
        )));
        #[cfg(feature = "e2e")]
        app.manage(TrustedDeviceManager::new(E2eCredentialVault::default()));
        #[cfg(feature = "e2e")]
        app.manage(AccountSessionStore::new(E2eCredentialVault::default()));
        Ok(())
    });

    let builder = builder.invoke_handler(tauri::generate_handler![
        get_setting,
        set_setting,
        get_account_session_item,
        set_account_session_item,
        remove_account_session_item,
        is_current_device_trusted,
        is_current_device_unlocked,
        current_device_public_key,
        current_key_epoch,
        set_current_key_epoch,
        configure_device_pin,
        unlock_device,
        lock_device,
        forget_current_device,
        enrol_first_device,
        confirm_recovery_key,
        recover_device,
        finalize_recovered_device,
        prepare_recovery_key_replacement,
        confirm_recovery_key_replacement,
        prepare_household_key_rotation,
        finalize_household_key_rotation,
        discard_household_key_rotation,
        apply_rotated_device_envelope,
        protect_household_state,
        open_household_state
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("Luna failed to start");
}
