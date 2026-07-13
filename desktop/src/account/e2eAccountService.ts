import type {
  AccountService,
  HouseholdId,
  LunaAccountId,
  RegisterAccountRequest,
  TrustedDeviceRecord,
} from "./accountService";
import { e2eAccountFixture } from "./e2eAccountFixture";

let registration: RegisterAccountRequest | undefined;
let verified = false;
let householdSession: Awaited<ReturnType<AccountService["createHousehold"]>> | undefined;
let currentPassword = "";
let passwordResetRequestPending = false;
let recoveryCodeAvailable = false;
let authenticatorEnrolled = false;
let authenticatorVerified = false;
let recoveryEnvelope = "";
let trustedDevices: TrustedDeviceRecord[] = [];
const trustedDeviceEnvelopes = new Map<string, string>();

type E2eRemoteRotation = {
  currentKeyEpoch: number;
  recoveryEnvelope: string;
  deviceEnvelopes: Array<{ devicePublicKey: string; keyEnvelope: string }>;
};

export const e2eAccountTestControl = {
  currentRecovery() {
    return {
      recoveryEnvelope,
      keyEpoch: Math.max(...trustedDevices.map(({ keyEpoch }) => keyEpoch)),
    };
  },
  simulateRemoteRotation(rotation: E2eRemoteRotation) {
    const activeDevices = trustedDevices.filter(({ status }) => status === "active");
    const currentKeyEpoch = Math.max(...activeDevices.map(({ keyEpoch }) => keyEpoch));
    if (
      rotation.currentKeyEpoch !== currentKeyEpoch
      || rotation.deviceEnvelopes.length !== activeDevices.length
      || activeDevices.some(({ publicKey }) => !rotation.deviceEnvelopes.some(
        ({ devicePublicKey }) => devicePublicKey === publicKey,
      ))
    ) {
      throw new Error("The simulated remote rotation is incomplete.");
    }
    recoveryEnvelope = rotation.recoveryEnvelope;
    for (const envelope of rotation.deviceEnvelopes) {
      trustedDeviceEnvelopes.set(envelope.devicePublicKey, envelope.keyEnvelope);
    }
    trustedDevices = trustedDevices.map((device) => (
      device.status === "active" ? { ...device, keyEpoch: currentKeyEpoch + 1 } : device
    ));
  },
};

export const e2eAccountService: AccountService = {
  async register(request) {
    registration = request;
    currentPassword = request.password;
    return { email: request.email };
  },
  async verifyEmail(email, code) {
    if (!registration || registration.email !== email || code !== e2eAccountFixture.verificationCode) {
      throw new Error("Invalid verification code.");
    }
    verified = true;
  },
  async createHousehold(householdName) {
    if (!registration || !verified) {
      throw new Error("Account verification is required.");
    }
    householdSession = {
      accountId: "account-sam-rivera" as LunaAccountId,
      organiserName: registration.organiserName,
      email: registration.email,
      householdId: e2eAccountFixture.householdId as HouseholdId,
      householdName,
    };
    return householdSession;
  },
  async requestPasswordReset(email) {
    if (passwordResetRequestPending) {
      throw new Error("A password recovery request is already pending.");
    }
    passwordResetRequestPending = true;
    await new Promise((resolve) => setTimeout(resolve, 250));
    passwordResetRequestPending = false;
    recoveryCodeAvailable = registration?.email === email;
  },
  async resetPassword(request) {
    if (
      !registration
      || registration.email !== request.email
      || request.recoveryCode !== e2eAccountFixture.recoveryCode
      || !recoveryCodeAvailable
      || (authenticatorEnrolled && request.authenticatorCode !== e2eAccountFixture.authenticatorCode)
    ) {
      throw new Error("Invalid recovery code.");
    }
    recoveryCodeAvailable = false;
    currentPassword = request.newPassword;
  },
  async beginAuthenticatorEnrollment() {
    return {
      factorId: "factor-sam-rivera",
      qrCode: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Crect width='160' height='160' fill='white'/%3E%3Cpath d='M16 16h48v48H16zm80 0h48v48H96zM16 96h48v48H16zm80 0h16v16H96zm32 0h16v48h-16zM96 128h16v16H96z' fill='black'/%3E%3C/svg%3E",
      secret: e2eAccountFixture.authenticatorSecret,
    };
  },
  async verifyAuthenticatorEnrollment(_factorId, code) {
    if (code !== e2eAccountFixture.authenticatorCode) {
      throw new Error("Invalid authenticator code.");
    }
    authenticatorEnrolled = true;
    authenticatorVerified = true;
  },
  async getAuthenticatorStatus() {
    if (!authenticatorEnrolled) return "unenrolled";
    return authenticatorVerified ? "verified" : "challengeRequired";
  },
  async verifyAuthenticatorChallenge(code) {
    if (!authenticatorEnrolled || code !== e2eAccountFixture.authenticatorCode) {
      throw new Error("Invalid authenticator code.");
    }
    authenticatorVerified = true;
  },
  async registerFirstTrustedDevice(request) {
    if (!request.recoveryVerificationKey) throw new Error("Recovery verifier is required.");
    recoveryEnvelope = request.recoveryEnvelope;
    const device: TrustedDeviceRecord = {
      id: "device-sam-rivera",
      label: request.label,
      publicKey: request.publicKey,
      keyEpoch: 1,
      status: "active",
    };
    trustedDevices.push(device);
    trustedDeviceEnvelopes.set(device.publicKey, request.keyEnvelope);
    return device;
  },
  async registerRecoveredTrustedDevice(request) {
    if (!request.recoveryAuthorizationSignature) throw new Error("Recovery authorization is required.");
    const device: TrustedDeviceRecord = {
      id: "device-sam-rivera-replacement",
      label: request.label,
      publicKey: request.publicKey,
      keyEpoch: request.keyEpoch,
      status: "active",
    };
    trustedDevices.push(device);
    trustedDeviceEnvelopes.set(device.publicKey, request.keyEnvelope);
    return device;
  },
  async getTrustedDeviceRecoveryEnvelope() {
    return {
      recoveryEnvelope,
      keyEpoch: Math.max(...trustedDevices.map(({ keyEpoch }) => keyEpoch)),
    };
  },
  async listTrustedDevices() {
    return trustedDevices.map((device) => ({ ...device }));
  },
  async getTrustedDeviceKeyCoordination(publicKey) {
    const device = trustedDevices.find((candidate) => candidate.publicKey === publicKey);
    if (!device) throw new Error("Trusted Device not found.");
    const keyEnvelope = trustedDeviceEnvelopes.get(device.publicKey);
    if (!keyEnvelope) throw new Error("Trusted Device envelope not found.");
    return { keyEnvelope, keyEpoch: device.keyEpoch, status: device.status };
  },
  async revokeTrustedDevice(request) {
    if (!request.recoveryAuthorizationSignature) throw new Error("Recovery authorization is required.");
    const target = trustedDevices.find(({ id }) => id === request.deviceId);
    const current = trustedDevices.find(({ publicKey }) => publicKey === request.currentDevicePublicKey);
    if (!target || !current || target.id === current.id || target.status !== "active") {
      throw new Error("Trusted Device revocation is invalid.");
    }
    const retained = trustedDevices.filter(({ id, status }) => id !== target.id && status === "active");
    if (
      request.deviceEnvelopes.length !== retained.length
      || retained.some(({ publicKey }) => !request.deviceEnvelopes.some(
        ({ devicePublicKey }) => devicePublicKey === publicKey,
      ))
    ) {
      throw new Error("Every retained device requires a rotated key envelope.");
    }
    const nextEpoch = request.currentKeyEpoch + 1;
    recoveryEnvelope = request.recoveryEnvelope;
    for (const envelope of request.deviceEnvelopes) {
      trustedDeviceEnvelopes.set(envelope.devicePublicKey, envelope.keyEnvelope);
    }
    trustedDevices = trustedDevices.map((device) => {
      if (device.id === target.id) return { ...device, status: "revoked" };
      if (device.status !== "active") return device;
      return { ...device, keyEpoch: nextEpoch };
    });
    return trustedDevices.map((device) => ({ ...device }));
  },
  async signIn(email, password) {
    if (!registration || !householdSession || registration.email !== email || currentPassword !== password) {
      throw new Error("Invalid credentials.");
    }
    return householdSession;
  },
  async signOut() {
    authenticatorVerified = false;
  },
};
