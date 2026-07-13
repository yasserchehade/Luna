import type { HouseholdId } from "../account/accountService";

export type TrustedDeviceEnrollment = {
  devicePublicKey: string;
  deviceKeyEnvelope: string;
  recoveryKey: string;
  recoveryEnvelope: string;
  recoveryVerificationKey: string;
};

export type RecoveredTrustedDevice = {
  devicePublicKey: string;
  deviceKeyEnvelope: string;
  recoveryAuthorizationSignature: string;
};

export type RotatedDeviceKeyEnvelope = {
  devicePublicKey: string;
  keyEnvelope: string;
};

export type HouseholdKeyRotation = {
  deviceEnvelopes: RotatedDeviceKeyEnvelope[];
  recoveryEnvelope: string;
  recoveryAuthorizationSignature: string;
};

export interface TrustedDeviceService {
  isCurrentDeviceTrusted(householdId: HouseholdId): Promise<boolean>;
  isCurrentDeviceUnlocked(householdId: HouseholdId): Promise<boolean>;
  currentDevicePublicKey(householdId: HouseholdId): Promise<string>;
  currentKeyEpoch(householdId: HouseholdId): Promise<number>;
  setCurrentKeyEpoch(householdId: HouseholdId, keyEpoch: number): Promise<void>;
  configureDevicePin(householdId: HouseholdId, pin: string): Promise<void>;
  unlockDevice(householdId: HouseholdId, pin: string): Promise<void>;
  lockDevice(householdId: HouseholdId): Promise<void>;
  forgetCurrentDevice(householdId: HouseholdId): Promise<void>;
  enrolFirstDevice(householdId: HouseholdId): Promise<TrustedDeviceEnrollment>;
  recoverDevice(
    householdId: HouseholdId,
    recoveryKey: string,
    recoveryEnvelope: string,
    keyEpoch: number,
  ): Promise<RecoveredTrustedDevice>;
  finalizeRecoveredDevice(householdId: HouseholdId, keyEpoch: number): Promise<void>;
  prepareHouseholdKeyRotation(
    householdId: HouseholdId,
    recoveryKey: string,
    recoveryEnvelope: string,
    retainedDevicePublicKeys: string[],
    currentKeyEpoch: number,
    revokedDeviceId: string,
  ): Promise<HouseholdKeyRotation>;
  finalizeHouseholdKeyRotation(householdId: HouseholdId, keyEpoch: number): Promise<void>;
  discardHouseholdKeyRotation(householdId: HouseholdId): Promise<void>;
  applyRotatedDeviceEnvelope(
    householdId: HouseholdId,
    keyEnvelope: string,
    keyEpoch: number,
  ): Promise<void>;
  confirmRecoveryKey(householdId: HouseholdId, recoveryKey: string, recoveryEnvelope: string): Promise<void>;
}

export const unavailableTrustedDeviceService: TrustedDeviceService = {
  async isCurrentDeviceTrusted() {
    return false;
  },
  async isCurrentDeviceUnlocked() {
    return false;
  },
  async currentDevicePublicKey() {
    throw new Error("Trusted Device identity is not configured.");
  },
  async currentKeyEpoch() {
    throw new Error("Trusted Device key coordination is not configured.");
  },
  async setCurrentKeyEpoch() {
    throw new Error("Trusted Device key coordination is not configured.");
  },
  async configureDevicePin() {
    throw new Error("Trusted Device PIN setup is not configured.");
  },
  async unlockDevice() {
    throw new Error("Trusted Device unlock is not configured.");
  },
  async lockDevice() {
    throw new Error("Trusted Device locking is not configured.");
  },
  async forgetCurrentDevice() {
    throw new Error("Trusted Device removal is not configured.");
  },
  async enrolFirstDevice() {
    throw new Error("Trusted Device enrolment is not configured.");
  },
  async recoverDevice() {
    throw new Error("Trusted Device recovery is not configured.");
  },
  async finalizeRecoveredDevice() {
    throw new Error("Trusted Device recovery is not configured.");
  },
  async prepareHouseholdKeyRotation() {
    throw new Error("Trusted Device revocation is not configured.");
  },
  async finalizeHouseholdKeyRotation() {
    throw new Error("Trusted Device revocation is not configured.");
  },
  async discardHouseholdKeyRotation() {
    throw new Error("Trusted Device revocation is not configured.");
  },
  async applyRotatedDeviceEnvelope() {
    throw new Error("Trusted Device key coordination is not configured.");
  },
  async confirmRecoveryKey() {
    throw new Error("Trusted Device enrolment is not configured.");
  },
};
