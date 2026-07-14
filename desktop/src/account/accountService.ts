export type RegisterAccountRequest = {
  organiserName: string;
  email: string;
  password: string;
};

export type VerificationRequested = {
  email: string;
};

export type AuthenticatorEnrollment = {
  factorId: string;
  qrCode: string;
  secret: string;
};

export type AuthenticatorStatus = "unenrolled" | "challengeRequired" | "verified";

export type ResetPasswordRequest = {
  email: string;
  recoveryCode: string;
  authenticatorCode: string;
  newPassword: string;
};

export type RegisterTrustedDeviceRequest = {
  label: string;
  publicKey: string;
  authorizationPublicKey: string;
  keyEnvelope: string;
  recoveryEnvelope: string;
  recoveryVerificationKey: string;
};

export type ReplaceRecoveryKeyRequest = {
  currentDevicePublicKey: string;
  currentKeyEpoch: number;
  currentRecoveryVerificationKey: string;
  recoveryEnvelope: string;
  recoveryVerificationKey: string;
  deviceAuthorizationSignature: string;
};

export type RegisterRecoveredTrustedDeviceRequest = Omit<
  RegisterTrustedDeviceRequest,
  "recoveryEnvelope" | "recoveryVerificationKey"
> & {
  keyEpoch: number;
  recoveryAuthorizationSignature: string;
};

export type TrustedDeviceRecord = {
  id: string;
  label: string;
  publicKey: string;
  keyEpoch: number;
  status: "active" | "revoked";
};

export type RevokeTrustedDeviceRequest = {
  deviceId: string;
  currentDevicePublicKey: string;
  currentKeyEpoch: number;
  recoveryEnvelope: string;
  deviceEnvelopes: Array<{ devicePublicKey: string; keyEnvelope: string }>;
  recoveryAuthorizationSignature: string;
};

export type TrustedDeviceKeyCoordination = {
  keyEnvelope: string;
  keyEpoch: number;
  status: "active" | "revoked";
};

declare const lunaAccountIdBrand: unique symbol;
declare const householdIdBrand: unique symbol;

export type LunaAccountId = string & { readonly [lunaAccountIdBrand]: true };
export type HouseholdId = string & { readonly [householdIdBrand]: true };

export type HouseholdSession = {
  accountId: LunaAccountId;
  organiserName: string;
  email: string;
  householdId: HouseholdId;
  householdName: string;
};

export type AccountSessionStorage = {
  getItem(key: string): string | null | Promise<string | null>;
  setItem(key: string, value: string): void | Promise<void>;
  removeItem(key: string): void | Promise<void>;
};

export interface AccountService {
  register(request: RegisterAccountRequest): Promise<VerificationRequested>;
  verifyEmail(email: string, code: string): Promise<void>;
  createHousehold(name: string): Promise<HouseholdSession>;
  requestPasswordReset(email: string): Promise<void>;
  resetPassword(request: ResetPasswordRequest): Promise<void>;
  beginAuthenticatorEnrollment(): Promise<AuthenticatorEnrollment>;
  verifyAuthenticatorEnrollment(factorId: string, code: string): Promise<void>;
  getAuthenticatorStatus(): Promise<AuthenticatorStatus>;
  verifyAuthenticatorChallenge(code: string): Promise<void>;
  registerFirstTrustedDevice(request: RegisterTrustedDeviceRequest): Promise<TrustedDeviceRecord>;
  registerRecoveredTrustedDevice(request: RegisterRecoveredTrustedDeviceRequest): Promise<TrustedDeviceRecord>;
  getTrustedDeviceRecoveryEnvelope(): Promise<{
    recoveryEnvelope: string;
    recoveryVerificationKey: string;
    keyEpoch: number;
  }>;
  listTrustedDevices(): Promise<TrustedDeviceRecord[]>;
  getTrustedDeviceKeyCoordination(publicKey: string): Promise<TrustedDeviceKeyCoordination>;
  replaceRecoveryKey(request: ReplaceRecoveryKeyRequest): Promise<void>;
  revokeTrustedDevice(request: RevokeTrustedDeviceRequest): Promise<TrustedDeviceRecord[]>;
  restoreSession(): Promise<HouseholdSession | null>;
  signIn(email: string, password: string): Promise<HouseholdSession>;
  signOut(): Promise<void>;
}

export const unavailableAccountService: AccountService = {
  async register() {
    throw new Error("The Luna account service is not configured.");
  },
  async verifyEmail() {
    throw new Error("The Luna account service is not configured.");
  },
  async createHousehold() {
    throw new Error("The Luna account service is not configured.");
  },
  async requestPasswordReset() {
    throw new Error("The Luna account service is not configured.");
  },
  async resetPassword() {
    throw new Error("The Luna account service is not configured.");
  },
  async beginAuthenticatorEnrollment() {
    throw new Error("The Luna account service is not configured.");
  },
  async verifyAuthenticatorEnrollment() {
    throw new Error("The Luna account service is not configured.");
  },
  async getAuthenticatorStatus() {
    throw new Error("The Luna account service is not configured.");
  },
  async verifyAuthenticatorChallenge() {
    throw new Error("The Luna account service is not configured.");
  },
  async registerFirstTrustedDevice() {
    throw new Error("The Luna account service is not configured.");
  },
  async registerRecoveredTrustedDevice() {
    throw new Error("The Luna account service is not configured.");
  },
  async getTrustedDeviceRecoveryEnvelope() {
    throw new Error("The Luna account service is not configured.");
  },
  async listTrustedDevices() {
    throw new Error("Trusted Device listing is not configured.");
  },
  async getTrustedDeviceKeyCoordination() {
    throw new Error("Trusted Device key coordination is not configured.");
  },
  async replaceRecoveryKey() {
    throw new Error("Recovery Key Replacement is not configured.");
  },
  async revokeTrustedDevice() {
    throw new Error("Trusted Device revocation is not configured.");
  },
  async restoreSession() {
    return null;
  },
  async signIn() {
    throw new Error("The Luna account service is not configured.");
  },
  async signOut() {
    throw new Error("The Luna account service is not configured.");
  },
};
