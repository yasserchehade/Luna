import { invoke } from "@tauri-apps/api/core";
import type { TrustedDeviceService } from "./trustedDeviceService";

export const tauriTrustedDeviceService: TrustedDeviceService = {
  async isCurrentDeviceTrusted(householdId) {
    return invoke<boolean>("is_current_device_trusted", { householdId });
  },
  async isCurrentDeviceUnlocked(householdId) {
    return invoke<boolean>("is_current_device_unlocked", { householdId });
  },
  async currentDevicePublicKey(householdId) {
    return invoke<string>("current_device_public_key", { householdId });
  },
  async signManagedIntelligenceDeviceProvisioning(householdId, nonce) {
    return invoke<string>("sign_managed_intelligence_device_provisioning", { householdId, nonce });
  },
  async currentKeyEpoch(householdId) {
    return invoke<number>("current_key_epoch", { householdId });
  },
  async portableAuthorizationCutoff(householdId, deviceId) {
    return invoke("portable_authorization_cutoff", { householdId, deviceId });
  },
  async setCurrentKeyEpoch(householdId, keyEpoch) {
    await invoke("set_current_key_epoch", { householdId, keyEpoch });
  },
  async configureDevicePin(householdId, pin) {
    await invoke("configure_device_pin", { householdId, pin });
  },
  async unlockDevice(householdId, pin) {
    await invoke("unlock_device", { householdId, pin });
  },
  async lockDevice(householdId) {
    await invoke("lock_device", { householdId });
  },
  async forgetCurrentDevice(householdId) {
    await invoke("forget_current_device", { householdId });
  },
  async enrolFirstDevice(householdId) {
    return invoke("enrol_first_device", { householdId });
  },
  async recoverDevice(householdId, recoveryKey, recoveryEnvelope, keyEpoch) {
    return invoke("recover_device", { householdId, recoveryKey, recoveryEnvelope, keyEpoch });
  },
  async finalizeRecoveredDevice(householdId, keyEpoch) {
    await invoke("finalize_recovered_device", { householdId, keyEpoch });
  },
  async prepareRecoveryKeyReplacement(householdId, currentKeyEpoch, currentRecoveryVerificationKey) {
    return invoke("prepare_recovery_key_replacement", {
      householdId,
      currentKeyEpoch,
      currentRecoveryVerificationKey,
    });
  },
  async confirmRecoveryKeyReplacement(householdId, recoveryKey, recoveryEnvelope) {
    await invoke("confirm_recovery_key_replacement", { householdId, recoveryKey, recoveryEnvelope });
  },
  async prepareHouseholdKeyRotation(
    householdId,
    recoveryKey,
    recoveryEnvelope,
    retainedDevicePublicKeys,
    currentKeyEpoch,
    revokedDeviceId,
  ) {
    return invoke("prepare_household_key_rotation", {
      householdId,
      recoveryKey,
      recoveryEnvelope,
      retainedDevicePublicKeys,
      currentKeyEpoch,
      revokedDeviceId,
    });
  },
  async finalizeHouseholdKeyRotation(householdId, keyEpoch) {
    await invoke("finalize_household_key_rotation", { householdId, keyEpoch });
  },
  async discardHouseholdKeyRotation(householdId) {
    await invoke("discard_household_key_rotation", { householdId });
  },
  async applyRotatedDeviceEnvelope(householdId, keyEnvelope, keyEpoch) {
    await invoke("apply_rotated_device_envelope", { householdId, keyEnvelope, keyEpoch });
  },
  async confirmRecoveryKey(householdId, recoveryKey, recoveryEnvelope) {
    await invoke("confirm_recovery_key", { householdId, recoveryKey, recoveryEnvelope });
  },
};
