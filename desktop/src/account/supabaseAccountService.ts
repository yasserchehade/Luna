import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import type {
  AccountSessionStorage,
  AccountService,
  HouseholdId,
  HouseholdSession,
  LunaAccountId,
  RegisterAccountRequest,
  RegisterRecoveredTrustedDeviceRequest,
  RegisterTrustedDeviceRequest,
  RevokeTrustedDeviceRequest,
  ResetPasswordRequest,
  TrustedDeviceRecord,
  VerificationRequested,
} from "./accountService";

type HouseholdRow = {
  account_id: string;
  organiser_name: string;
  email: string;
  household_id: string;
  household_name: string;
};

type TrustedDeviceRow = {
  device_id: string;
  device_label: string;
  device_public_key: string;
  key_epoch: number;
  device_status: "active" | "revoked";
};

const accountExistsCodes = new Set(["email_exists", "user_already_exists"]);
const householdSessionStorageKey = "luna-household-session";

export class SupabaseAccountService implements AccountService {
  private readonly client: SupabaseClient;

  constructor(
    url: string,
    publishableKey: string,
    private readonly sessionStorage?: AccountSessionStorage,
  ) {
    this.client = createClient(url, publishableKey, {
      auth: {
        autoRefreshToken: true,
        detectSessionInUrl: false,
        flowType: "pkce",
        persistSession: sessionStorage !== undefined,
        storage: sessionStorage,
      },
    });
  }

  async register(request: RegisterAccountRequest): Promise<VerificationRequested> {
    const { error } = await this.client.auth.signUp({
      email: request.email,
      password: request.password,
      options: { data: { organiser_name: request.organiserName } },
    });

    if (error && !accountExistsCodes.has(error.code ?? "")) throw error;
    return { email: request.email };
  }

  async verifyEmail(email: string, code: string): Promise<void> {
    const { error } = await this.client.auth.verifyOtp({ email, token: code, type: "email" });
    if (error) throw error;
  }

  async createHousehold(name: string): Promise<HouseholdSession> {
    const { data, error } = await this.client.rpc("create_household", { requested_name: name });
    if (error) throw error;
    return this.rememberHousehold(mapHousehold(singleRow(data)));
  }

  async requestPasswordReset(email: string): Promise<void> {
    const { error } = await this.client.auth.resetPasswordForEmail(email);
    if (error) throw error;
  }

  async resetPassword(request: ResetPasswordRequest): Promise<void> {
    const verification = await this.client.auth.verifyOtp({
      email: request.email,
      token: request.recoveryCode,
      type: "recovery",
    });
    if (verification.error) throw verification.error;

    if (await this.getAuthenticatorStatus() === "challengeRequired") {
      await this.verifyAuthenticatorChallenge(request.authenticatorCode);
    }

    const update = await this.client.auth.updateUser({ password: request.newPassword });
    if (update.error) throw update.error;
  }

  async beginAuthenticatorEnrollment() {
    const { data, error } = await this.client.auth.mfa.enroll({
      factorType: "totp",
      friendlyName: "Luna authenticator",
    });
    if (error) throw error;
    return {
      factorId: data.id,
      qrCode: data.totp.qr_code,
      secret: data.totp.secret,
    };
  }

  async verifyAuthenticatorEnrollment(factorId: string, code: string): Promise<void> {
    const challenge = await this.client.auth.mfa.challenge({ factorId });
    if (challenge.error) throw challenge.error;
    const verification = await this.client.auth.mfa.verify({
      factorId,
      challengeId: challenge.data.id,
      code,
    });
    if (verification.error) throw verification.error;
  }

  async getAuthenticatorStatus() {
    const { data, error } = await this.client.auth.mfa.getAuthenticatorAssuranceLevel();
    if (error) throw error;
    if (data.currentLevel === "aal2") return "verified" as const;
    if (data.nextLevel === "aal2") return "challengeRequired" as const;
    return "unenrolled" as const;
  }

  async verifyAuthenticatorChallenge(code: string): Promise<void> {
    const factors = await this.client.auth.mfa.listFactors();
    if (factors.error) throw factors.error;
    const factor = factors.data.totp.find(({ status }) => status === "verified");
    if (!factor) throw new Error("No verified authenticator is available.");
    const challenge = await this.client.auth.mfa.challenge({ factorId: factor.id });
    if (challenge.error) throw challenge.error;
    const verification = await this.client.auth.mfa.verify({
      factorId: factor.id,
      challengeId: challenge.data.id,
      code,
    });
    if (verification.error) throw verification.error;
  }

  async registerFirstTrustedDevice(request: RegisterTrustedDeviceRequest): Promise<TrustedDeviceRecord> {
    const { data, error } = await this.client.rpc("register_first_trusted_device", {
      requested_label: request.label,
      requested_public_key: request.publicKey,
      requested_key_envelope: request.keyEnvelope,
      requested_recovery_envelope: request.recoveryEnvelope,
      requested_recovery_verification_key: request.recoveryVerificationKey,
    });
    if (error) throw error;
    return mapTrustedDevice(singleTrustedDeviceRow(data));
  }

  async registerRecoveredTrustedDevice(
    request: RegisterRecoveredTrustedDeviceRequest,
  ): Promise<TrustedDeviceRecord> {
    const { data, error } = await this.client.rpc("register_recovered_trusted_device", {
      requested_label: request.label,
      requested_public_key: request.publicKey,
      requested_key_envelope: request.keyEnvelope,
      requested_key_epoch: request.keyEpoch,
      requested_recovery_authorization_signature: request.recoveryAuthorizationSignature,
    });
    if (error) throw error;
    return mapTrustedDevice(singleTrustedDeviceRow(data));
  }

  async getTrustedDeviceRecoveryEnvelope() {
    const { data, error } = await this.client.rpc("current_trusted_device_recovery");
    if (error) throw error;
    const row = Array.isArray(data) ? data[0] : data;
    if (!row || typeof row.recovery_envelope !== "string" || typeof row.key_epoch !== "number") {
      throw new Error("The Luna account service returned invalid Trusted Device recovery data.");
    }
    return { recoveryEnvelope: row.recovery_envelope, keyEpoch: row.key_epoch };
  }

  async listTrustedDevices(): Promise<TrustedDeviceRecord[]> {
    const { data, error } = await this.client.rpc("current_trusted_devices");
    if (error) throw error;
    if (!Array.isArray(data)) {
      throw new Error("The Luna account service returned invalid Trusted Devices.");
    }
    return data.map((row) => mapTrustedDevice(singleTrustedDeviceRow(row)));
  }

  async getTrustedDeviceKeyCoordination(publicKey: string) {
    const { data, error } = await this.client.rpc("current_trusted_device_key", {
      requested_public_key: publicKey,
    });
    if (error) throw error;
    const row = Array.isArray(data) ? data[0] : data;
    if (
      !row
      || typeof row.key_envelope !== "string"
      || typeof row.key_epoch !== "number"
      || (row.device_status !== "active" && row.device_status !== "revoked")
    ) {
      throw new Error("The Luna account service returned invalid Trusted Device coordination.");
    }
    return {
      keyEnvelope: row.key_envelope,
      keyEpoch: row.key_epoch,
      status: row.device_status,
    };
  }

  async revokeTrustedDevice(request: RevokeTrustedDeviceRequest): Promise<TrustedDeviceRecord[]> {
    const { data, error } = await this.client.rpc("revoke_trusted_device", {
      requested_device_id: request.deviceId,
      requested_current_device_public_key: request.currentDevicePublicKey,
      requested_current_key_epoch: request.currentKeyEpoch,
      requested_recovery_envelope: request.recoveryEnvelope,
      requested_device_envelopes: request.deviceEnvelopes,
      requested_recovery_authorization_signature: request.recoveryAuthorizationSignature,
    });
    if (error) throw error;
    if (!Array.isArray(data)) {
      throw new Error("The Luna account service returned invalid Trusted Devices.");
    }
    return data.map((row) => mapTrustedDevice(singleTrustedDeviceRow(row)));
  }

  async signIn(email: string, password: string): Promise<HouseholdSession> {
    const { error } = await this.client.auth.signInWithPassword({ email, password });
    if (error) throw error;

    const { data, error: householdError } = await this.client.rpc("current_household");
    if (householdError) throw householdError;
    return this.rememberHousehold(mapHousehold(singleRow(data)));
  }

  async restoreSession(): Promise<HouseholdSession | null> {
    const session = await this.client.auth.getSession();
    if (session.error || !session.data.session) return null;
    if (await this.getAuthenticatorStatus() !== "verified") return null;
    const cachedHousehold = await this.readRememberedHousehold();
    if (cachedHousehold) return cachedHousehold;
    const { data, error } = await this.client.rpc("current_household");
    if (error) return null;
    return this.rememberHousehold(mapHousehold(singleRow(data)));
  }

  async signOut(): Promise<void> {
    const { error } = await this.client.auth.signOut({ scope: "local" });
    if (error) throw error;
    await this.sessionStorage?.removeItem(householdSessionStorageKey);
  }

  private async rememberHousehold(session: HouseholdSession): Promise<HouseholdSession> {
    await this.sessionStorage?.setItem(householdSessionStorageKey, JSON.stringify(session));
    return session;
  }

  private async readRememberedHousehold(): Promise<HouseholdSession | null> {
    const stored = await this.sessionStorage?.getItem(householdSessionStorageKey);
    if (!stored) return null;
    try {
      return mapHouseholdSession(JSON.parse(stored));
    } catch {
      await this.sessionStorage?.removeItem(householdSessionStorageKey);
      return null;
    }
  }
}

function singleRow(data: unknown): HouseholdRow {
  const row = Array.isArray(data) ? data[0] : data;
  if (!isHouseholdRow(row)) throw new Error("The Luna account service returned an invalid Household.");
  return row;
}

function isHouseholdRow(value: unknown): value is HouseholdRow {
  if (!value || typeof value !== "object") return false;
  const row = value as Record<string, unknown>;
  return ["account_id", "organiser_name", "email", "household_id", "household_name"]
    .every((field) => typeof row[field] === "string" && row[field] !== "");
}

function mapHousehold(row: HouseholdRow): HouseholdSession {
  return {
    accountId: row.account_id as LunaAccountId,
    organiserName: row.organiser_name,
    email: row.email,
    householdId: row.household_id as HouseholdId,
    householdName: row.household_name,
  };
}

function mapHouseholdSession(value: unknown): HouseholdSession {
  if (!value || typeof value !== "object") {
    throw new Error("The stored Luna Household session is invalid.");
  }
  const session = value as Record<string, unknown>;
  if (!["accountId", "organiserName", "email", "householdId", "householdName"].every(
    (field) => typeof session[field] === "string" && session[field] !== "",
  )) {
    throw new Error("The stored Luna Household session is invalid.");
  }
  return session as HouseholdSession;
}

function singleTrustedDeviceRow(data: unknown): TrustedDeviceRow {
  const row = Array.isArray(data) ? data[0] : data;
  if (!row || typeof row !== "object") {
    throw new Error("The Luna account service returned an invalid Trusted Device.");
  }
  const candidate = row as Record<string, unknown>;
  if (
    typeof candidate.device_id !== "string"
    || typeof candidate.device_label !== "string"
    || typeof candidate.device_public_key !== "string"
    || typeof candidate.key_epoch !== "number"
    || (candidate.device_status !== "active" && candidate.device_status !== "revoked")
  ) {
    throw new Error("The Luna account service returned an invalid Trusted Device.");
  }
  return candidate as TrustedDeviceRow;
}

function mapTrustedDevice(row: TrustedDeviceRow): TrustedDeviceRecord {
  return {
    id: row.device_id,
    label: row.device_label,
    publicKey: row.device_public_key,
    keyEpoch: row.key_epoch,
    status: row.device_status,
  };
}
