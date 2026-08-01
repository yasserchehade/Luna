use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use luna_core::{CredentialVault, TrustedDeviceManager, VaultError};

#[derive(Clone, Default)]
struct MemoryCredentialVault {
    secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl CredentialVault for MemoryCredentialVault {
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

#[test]
fn a_confirmed_recovery_key_makes_the_first_device_trusted() {
    let household_id = "11111111-1111-4111-8111-111111111111";
    let manager = TrustedDeviceManager::new(MemoryCredentialVault::default());

    let enrollment = manager
        .enrol_first_device(household_id)
        .expect("first device enrolment should succeed");

    assert!(enrollment.device_public_key.starts_with("age1"));
    assert_eq!(enrollment.recovery_key.split_whitespace().count(), 24);
    assert!(manager
        .protect_household_state(household_id, b"household memory")
        .is_err());
    assert!(manager
        .confirm_recovery_key(
            household_id,
            "wrong recovery key",
            &enrollment.recovery_envelope
        )
        .is_err());
    manager
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("the displayed Recovery Key should confirm");
    manager
        .set_current_key_epoch(household_id, 1)
        .expect("service registration should record the first key epoch");
    assert!(!manager
        .is_current_device_trusted(household_id)
        .expect("trust status should be readable before PIN setup"));
    manager
        .configure_device_pin(household_id, "246810")
        .expect("a local device PIN should complete trust setup");

    let protected = manager
        .protect_household_state(household_id, b"household memory")
        .expect("trusted device should protect household state");
    assert_eq!(
        manager
            .open_household_state(household_id, &protected)
            .expect("trusted device should read household state"),
        b"household memory"
    );
    manager.lock_device(household_id);
    assert!(manager
        .open_household_state(household_id, &protected)
        .is_err());
    assert!(manager.unlock_device(household_id, "000000").is_err());
    manager
        .unlock_device(household_id, "246810")
        .expect("the configured local device PIN should unlock Household memory");

    let password_reset_session = TrustedDeviceManager::new(MemoryCredentialVault::default());
    assert!(password_reset_session
        .open_household_state(household_id, &protected)
        .is_err());
}

#[test]
fn a_legacy_trusted_device_backfills_its_missing_current_epoch_key() {
    let household_id = "10101010-1010-4010-8010-101010101010";
    let vault = MemoryCredentialVault::default();
    let manager = TrustedDeviceManager::new(vault.clone());
    let enrollment = manager
        .enrol_first_device(household_id)
        .expect("first device enrolment should succeed");
    manager
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("the Recovery Key should confirm");
    manager
        .set_current_key_epoch(household_id, 1)
        .expect("the current key epoch should be recorded");
    manager
        .configure_device_pin(household_id, "246810")
        .expect("the device PIN should complete trust");
    let protected = manager
        .protect_household_state(household_id, b"portable household memory")
        .expect("the unlocked device should protect portable memory");

    vault
        .delete_secret(&format!("household:{household_id}:memory-key:epoch:1"))
        .expect("the fixture should represent a device enrolled before epoch-key retention");

    assert_eq!(
        manager
            .open_household_state_at_epoch(household_id, 1, &protected)
            .expect("the current key should backfill its missing epoch slot"),
        b"portable household memory"
    );
    assert!(vault
        .get_secret(&format!("household:{household_id}:memory-key:epoch:1"))
        .expect("the vault should remain readable")
        .is_some());
}

#[test]
fn an_unlocked_trusted_device_authorizes_its_managed_intelligence_challenge() {
    let household_id = "13131313-1313-4313-8313-131313131313";
    let manager = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let enrollment = manager
        .enrol_first_device(household_id)
        .expect("first device enrolment should succeed");
    manager
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("the Recovery Key should confirm");
    manager
        .set_current_key_epoch(household_id, 1)
        .expect("the first key epoch should be recorded");
    manager
        .configure_device_pin(household_id, "246810")
        .expect("PIN setup should complete trust");
    manager
        .unlock_device(household_id, "246810")
        .expect("the Trusted Device should unlock");
    let nonce = "d916a996-710d-4a43-84ac-b28427151a7f";

    let signature = manager
        .sign_managed_intelligence_device_provisioning(household_id, nonce)
        .expect("the unlocked Trusted Device should sign the provisioning challenge");
    let verifier = VerifyingKey::from_bytes(&enrollment.device_authorization_public_key)
        .expect("the device authorization verifier should be valid");
    let authorization = canonical_authorization(
        "luna:managed-intelligence-device:v1:",
        [
            household_id.to_owned(),
            enrollment.device_public_key.clone(),
            nonce.to_owned(),
        ],
    );
    verifier
        .verify(&authorization, &Signature::from_bytes(&signature))
        .expect("the signature should bind the Household, Trusted Device and one-time nonce");
}

#[test]
fn an_unlocked_trusted_device_prepares_and_confirms_recovery_key_replacement() {
    let household_id = "12121212-1212-4212-8212-121212121212";
    let manager = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let enrollment = manager
        .enrol_first_device(household_id)
        .expect("first device enrolment should succeed");
    manager
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("the original Recovery Key should confirm");
    manager
        .set_current_key_epoch(household_id, 1)
        .expect("service registration should record the first key epoch");
    manager
        .configure_device_pin(household_id, "246810")
        .expect("a local Device PIN should complete trust setup");

    manager.lock_device(household_id);
    assert!(manager
        .prepare_recovery_key_replacement(household_id, 1, &enrollment.recovery_verification_key)
        .is_err());
    manager
        .unlock_device(household_id, "246810")
        .expect("the Trusted Device should unlock");

    let replacement = manager
        .prepare_recovery_key_replacement(household_id, 1, &enrollment.recovery_verification_key)
        .expect("an unlocked Trusted Device should prepare replacement recovery material");
    assert_eq!(replacement.recovery_key.split_whitespace().count(), 24);
    assert_ne!(replacement.recovery_key, enrollment.recovery_key);
    assert!(manager
        .confirm_recovery_key_replacement(
            household_id,
            "wrong replacement key",
            &replacement.recovery_envelope,
        )
        .is_err());
    manager
        .confirm_recovery_key_replacement(
            household_id,
            &replacement.recovery_key,
            &replacement.recovery_envelope,
        )
        .expect("the displayed replacement Recovery Key should confirm");
    let protected = manager
        .protect_household_state(household_id, b"memory protected before replacement")
        .expect("the current Trusted Device should retain the Household key");

    let device_verifier = VerifyingKey::from_bytes(&enrollment.device_authorization_public_key)
        .expect("the device authorization verifier should be an Ed25519 public key");
    device_verifier
        .verify(
            &canonical_authorization(
                "luna:replace-recovery-key:v1:",
                [
                    household_id.to_owned(),
                    "1".to_owned(),
                    enrollment.device_public_key.clone(),
                    BASE64.encode(enrollment.recovery_verification_key),
                    BASE64.encode(&replacement.recovery_envelope),
                    BASE64.encode(replacement.recovery_verification_key),
                ],
            ),
            &Signature::from_bytes(&replacement.device_authorization_signature),
        )
        .expect("replacement should prove current Trusted Device possession");

    manager
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("preparing replacement must not mutate the existing recovery material locally");

    let old_key_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    assert!(
        old_key_device
            .recover_device(
                household_id,
                &enrollment.recovery_key,
                &replacement.recovery_envelope,
                1,
            )
            .is_err(),
        "the previous Recovery Key must not open the replacement envelope"
    );

    let replacement_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    replacement_device
        .recover_device(
            household_id,
            &replacement.recovery_key,
            &replacement.recovery_envelope,
            1,
        )
        .expect("the replacement Recovery Key should recover a new device");
    replacement_device
        .finalize_recovered_device(household_id, 1)
        .expect("the replacement device should finalize after service registration");
    replacement_device
        .configure_device_pin(household_id, "864209")
        .expect("the replacement device should require a local PIN");
    assert_eq!(
        replacement_device
            .open_household_state(household_id, &protected)
            .expect("the replacement device should open existing Household state"),
        b"memory protected before replacement"
    );
}

#[test]
fn the_recovery_key_enrols_a_replacement_trusted_device() {
    let household_id = "22222222-2222-4222-8222-222222222222";
    let first_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let enrollment = first_device
        .enrol_first_device(household_id)
        .expect("first device enrolment should succeed");
    first_device
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("Recovery Key confirmation should succeed");
    first_device
        .set_current_key_epoch(household_id, 1)
        .expect("service registration should record the first key epoch");
    first_device
        .configure_device_pin(household_id, "123456")
        .expect("the first device should require a local PIN");
    let protected = first_device
        .protect_household_state(household_id, b"portable Household memory")
        .expect("first device should protect Household state");

    let replacement = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let recovered = replacement
        .recover_device(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
            1,
        )
        .expect("Recovery Key should enrol the replacement device");

    assert!(recovered.device_public_key.starts_with("age1"));
    assert_ne!(recovered.device_public_key, enrollment.device_public_key);
    let recovery_verifier = VerifyingKey::from_bytes(&enrollment.recovery_verification_key)
        .expect("the recovery verifier should be an Ed25519 public key");
    recovery_verifier
        .verify(
            &canonical_authorization(
                "luna:recover-device:v2:",
                [
                    household_id.to_owned(),
                    "1".to_owned(),
                    recovered.device_public_key.clone(),
                    BASE64.encode(recovered.device_authorization_public_key),
                    BASE64.encode(&recovered.device_key_envelope),
                ],
            ),
            &Signature::from_bytes(&recovered.recovery_authorization_signature),
        )
        .expect("replacement registration should prove Recovery Key possession");
    assert!(recovery_verifier
        .verify(
            &canonical_authorization(
                "luna:recover-device:v2:",
                [
                    household_id.to_owned(),
                    "1".to_owned(),
                    recovered.device_public_key.clone(),
                    BASE64.encode(recovered.device_authorization_public_key),
                    BASE64.encode(b"substituted envelope"),
                ],
            ),
            &Signature::from_bytes(&recovered.recovery_authorization_signature),
        )
        .is_err());
    assert!(!replacement
        .is_current_device_trusted(household_id)
        .expect("recovery should remain pending until service registration succeeds"));
    replacement
        .finalize_recovered_device(household_id, 1)
        .expect("successful service registration should finalize local recovery");
    replacement
        .configure_device_pin(household_id, "135790")
        .expect("the replacement device should require a local PIN");
    assert_eq!(
        replacement
            .open_household_state(household_id, &protected)
            .expect("replacement device should read Household state"),
        b"portable Household memory"
    );
}

#[test]
fn revoked_and_incorrectly_keyed_devices_cannot_read_new_household_state() {
    let household_id = "33333333-3333-4333-8333-333333333333";
    let first_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let enrollment = first_device
        .enrol_first_device(household_id)
        .expect("first device enrolment should succeed");
    first_device
        .confirm_recovery_key(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
        )
        .expect("Recovery Key confirmation should succeed");

    first_device
        .set_current_key_epoch(household_id, 1)
        .expect("service registration should record the first key epoch");
    first_device
        .configure_device_pin(household_id, "112233")
        .expect("the first device should require a local PIN");

    let active_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let active_enrollment = active_device
        .recover_device(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
            1,
        )
        .expect("replacement device recovery should succeed");
    active_device
        .finalize_recovered_device(household_id, 1)
        .expect("active-device recovery should finalize");
    active_device
        .configure_device_pin(household_id, "223344")
        .expect("the active device should require a local PIN");

    let surviving_device = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let surviving_enrollment = surviving_device
        .recover_device(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
            1,
        )
        .expect("another retained device should recover");
    surviving_device
        .finalize_recovered_device(household_id, 1)
        .expect("retained-device recovery should finalize");
    surviving_device
        .configure_device_pin(household_id, "334455")
        .expect("the retained device should require a local PIN");
    let protected_before_revocation = active_device
        .protect_household_state(household_id, b"state written before revocation")
        .expect("active device should protect Household state before rotation");

    let rotation = active_device
        .prepare_household_key_rotation_after_revocation(
            household_id,
            &enrollment.recovery_key,
            &enrollment.recovery_envelope,
            &[
                active_enrollment.device_public_key.clone(),
                surviving_enrollment.device_public_key.clone(),
            ],
            1,
            "revoked-first-device-id",
        )
        .expect("revocation should rotate the Household key");
    let recovery_verifier = VerifyingKey::from_bytes(&enrollment.recovery_verification_key)
        .expect("the recovery verifier should be valid");
    let rotation_authorization = canonical_rotation_authorization(
        household_id,
        "revoked-first-device-id",
        &active_enrollment.device_public_key,
        &rotation.recovery_envelope,
        &rotation.device_envelopes,
    );
    recovery_verifier
        .verify(
            &rotation_authorization,
            &Signature::from_bytes(&rotation.recovery_authorization_signature),
        )
        .expect("revocation should prove Recovery Key possession");
    let mut substituted_authorization = rotation_authorization;
    substituted_authorization.extend_from_slice(b"substituted envelope");
    assert!(recovery_verifier
        .verify(
            &substituted_authorization,
            &Signature::from_bytes(&rotation.recovery_authorization_signature),
        )
        .is_err());
    active_device
        .finalize_household_key_rotation(household_id, 2)
        .expect("a persisted service rotation should become active locally");
    let surviving_envelope = rotation
        .device_envelopes
        .iter()
        .find(|envelope| envelope.device_public_key == surviving_enrollment.device_public_key)
        .expect("every retained device should receive an envelope");
    surviving_device.lock_device(household_id);
    assert!(surviving_device
        .apply_rotated_device_envelope(household_id, &surviving_envelope.key_envelope, 2)
        .is_err());
    surviving_device
        .unlock_device(household_id, "334455")
        .expect("the retained device should unlock before applying a rotated envelope");
    surviving_device
        .apply_rotated_device_envelope(household_id, &surviving_envelope.key_envelope, 2)
        .expect("the retained device should accept its rotated key envelope");
    assert_eq!(
        surviving_device
            .current_key_epoch(household_id)
            .expect("the retained device should record the rotated epoch"),
        2
    );
    let protected_after_revocation = active_device
        .protect_household_state(household_id, b"state written after revocation")
        .expect("active device should protect new Household state");

    assert!(first_device
        .open_household_state(household_id, &protected_after_revocation)
        .is_err());
    assert_eq!(
        surviving_device
            .open_household_state(household_id, &protected_after_revocation)
            .expect("another retained device should read state after rotation"),
        b"state written after revocation"
    );
    assert_eq!(
        active_device
            .open_household_state_at_epoch(household_id, 1, &protected_before_revocation)
            .expect("a retained device should keep access to pre-rotation Household state"),
        b"state written before revocation"
    );
    assert_eq!(
        surviving_device
            .open_household_state_at_epoch(household_id, 1, &protected_before_revocation)
            .expect("another retained device should rebuild pre-rotation Household state"),
        b"state written before revocation"
    );
    let recovered_after_rotation = TrustedDeviceManager::new(MemoryCredentialVault::default());
    recovered_after_rotation
        .recover_device(
            household_id,
            &enrollment.recovery_key,
            &rotation.recovery_envelope,
            2,
        )
        .expect("a new device should recover the complete historical keyring");
    recovered_after_rotation
        .finalize_recovered_device(household_id, 2)
        .expect("post-rotation recovery should finalize at the current epoch");
    recovered_after_rotation
        .configure_device_pin(household_id, "556677")
        .expect("the post-rotation device should require a local PIN");
    assert_eq!(
        recovered_after_rotation
            .open_household_state_at_epoch(household_id, 1, &protected_before_revocation)
            .expect("a newly recovered device should rebuild pre-rotation History"),
        b"state written before revocation"
    );
    assert_eq!(
        recovered_after_rotation
            .open_household_state_at_epoch(household_id, 2, &protected_after_revocation)
            .expect("a newly recovered device should read current Household state"),
        b"state written after revocation"
    );

    let incorrectly_keyed = TrustedDeviceManager::new(MemoryCredentialVault::default());
    let unrelated = incorrectly_keyed
        .enrol_first_device(household_id)
        .expect("unrelated key setup should succeed");
    incorrectly_keyed
        .confirm_recovery_key(
            household_id,
            &unrelated.recovery_key,
            &unrelated.recovery_envelope,
        )
        .expect("unrelated key should confirm only its own envelope");
    incorrectly_keyed
        .set_current_key_epoch(household_id, 1)
        .expect("the unrelated device should record its own epoch");
    incorrectly_keyed
        .configure_device_pin(household_id, "445566")
        .expect("the unrelated device should complete its own setup");
    assert!(incorrectly_keyed
        .open_household_state(household_id, &protected_after_revocation)
        .is_err());
}

fn canonical_rotation_authorization(
    household_id: &str,
    revoked_device_id: &str,
    current_device_public_key: &str,
    recovery_envelope: &[u8],
    device_envelopes: &[luna_core::RotatedDeviceEnvelope],
) -> Vec<u8> {
    let mut sorted_envelopes = device_envelopes.iter().collect::<Vec<_>>();
    sorted_envelopes.sort_by(|left, right| left.device_public_key.cmp(&right.device_public_key));
    let mut fields = vec![
        household_id.to_owned(),
        "1".to_owned(),
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

fn canonical_authorization(
    domain_separator: &str,
    fields: impl IntoIterator<Item = String>,
) -> Vec<u8> {
    let mut message = domain_separator.to_owned();
    for field in fields {
        message.push_str(&field.len().to_string());
        message.push(':');
        message.push_str(&field);
    }
    message.into_bytes()
}
