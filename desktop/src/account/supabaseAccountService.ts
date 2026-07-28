import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import type {
  AccountSessionStorage,
  AccountService,
  AuthorizeManagedIntelligenceDeviceProvisioningRequest,
  HouseholdId,
  HouseholdIntelligenceAccess,
  HouseholdSession,
  LunaAccountId,
  ManagedIntelligenceProvisioningChallenge,
  RegisterAccountRequest,
  RegisterRecoveredTrustedDeviceRequest,
  RegisterTrustedDeviceRequest,
  ReplaceRecoveryKeyRequest,
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
  authorization_public_key?: string;
  activated_key_epoch?: number;
  revoked_after_key_epoch?: number | null;
  revoked_after_sequence?: number | null;
  revoked_after_event_digest?: string | null;
  key_epoch: number;
  device_status: "active" | "revoked";
};

type HouseholdIntelligenceAccessRow = {
  household_id: string;
  plan_code: "free" | "managed";
  entitlement_state: "free" | "checkout_pending" | "entitled" | "payment_problem" | "ended";
  device_state: "not_applicable" | "pending" | "ready" | "revoked";
  entitlement_source: "complimentary" | "billing" | null;
  max_budget_usd: number | null;
  valid_until: string | null;
  credential_expires_at: string | null;
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
      requested_authorization_public_key: request.authorizationPublicKey,
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
      requested_authorization_public_key: request.authorizationPublicKey,
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
    if (
      !row
      || typeof row.recovery_envelope !== "string"
      || typeof row.recovery_verification_key !== "string"
      || typeof row.key_epoch !== "number"
    ) {
      throw new Error("The Luna account service returned invalid Trusted Device recovery data.");
    }
    return {
      recoveryEnvelope: row.recovery_envelope,
      recoveryVerificationKey: row.recovery_verification_key,
      keyEpoch: row.key_epoch,
    };
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

  async replaceRecoveryKey(request: ReplaceRecoveryKeyRequest): Promise<void> {
    const { error } = await this.client.rpc("replace_recovery_key", {
      requested_current_device_public_key: request.currentDevicePublicKey,
      requested_current_key_epoch: request.currentKeyEpoch,
      requested_current_recovery_verification_key: request.currentRecoveryVerificationKey,
      requested_recovery_envelope: request.recoveryEnvelope,
      requested_recovery_verification_key: request.recoveryVerificationKey,
      requested_device_authorization_signature: request.deviceAuthorizationSignature,
    });
    if (error) throw error;
  }

  async revokeTrustedDevice(request: RevokeTrustedDeviceRequest): Promise<TrustedDeviceRecord[]> {
    const { data, error } = await this.client.rpc("revoke_trusted_device_with_portable_cutoff", {
      requested_device_id: request.deviceId,
      requested_current_device_public_key: request.currentDevicePublicKey,
      requested_current_key_epoch: request.currentKeyEpoch,
      requested_recovery_envelope: request.recoveryEnvelope,
      requested_device_envelopes: request.deviceEnvelopes,
      requested_recovery_authorization_signature: request.recoveryAuthorizationSignature,
      requested_revoked_after_key_epoch: request.portableCutoff?.keyEpoch ?? null,
      requested_revoked_after_sequence: request.portableCutoff?.sequence ?? null,
      requested_revoked_after_event_digest: request.portableCutoff?.eventDigest ?? null,
    });
    if (error) throw error;
    if (!Array.isArray(data)) {
      throw new Error("The Luna account service returned invalid Trusted Devices.");
    }
    return data.map((row) => mapTrustedDevice(singleTrustedDeviceRow(row)));
  }

  async getHouseholdIntelligenceAccess(devicePublicKey?: string): Promise<HouseholdIntelligenceAccess> {
    const { data, error } = await this.client.rpc("current_household_intelligence_access", {
      requested_device_public_key: devicePublicKey ?? null,
    });
    if (error) throw error;
    return mapHouseholdIntelligenceAccess(singleHouseholdIntelligenceAccessRow(data));
  }

  async createManagedIntelligenceCheckoutSession(): Promise<{ url: string }> {
    return this.createBillingSession("checkout");
  }

  async createManagedIntelligenceCustomerPortalSession(): Promise<{ url: string }> {
    return this.createBillingSession("portal");
  }

  async beginManagedIntelligenceDeviceProvisioning(
    devicePublicKey: string,
  ): Promise<ManagedIntelligenceProvisioningChallenge> {
    const { data, error } = await this.client.rpc("begin_managed_intelligence_device_provisioning", {
      requested_device_public_key: devicePublicKey,
    });
    if (error) throw error;
    const row = Array.isArray(data) ? data[0] : data;
    if (
      !row
      || typeof row.challenge_id !== "string"
      || typeof row.challenge_nonce !== "string"
      || typeof row.expires_at !== "string"
    ) throw new Error("The Luna account service returned an invalid provisioning challenge.");
    return { id: row.challenge_id, nonce: row.challenge_nonce, expiresAt: row.expires_at };
  }

  async provisionManagedIntelligenceDeviceAccess(
    request: AuthorizeManagedIntelligenceDeviceProvisioningRequest,
  ): Promise<{ state: "ready"; credential: string }> {
    const { data, error } = await this.client.functions.invoke("managed-intelligence-provisioning", {
      body: {
        devicePublicKey: request.devicePublicKey,
        challengeId: request.challengeId,
        nonce: request.nonce,
        authorizationSignature: request.authorizationSignature,
      },
    });
    if (error) throw error;
    if (!data || data.state !== "ready" || typeof data.credential !== "string" || !data.credential) {
      throw new Error("The Luna account service returned invalid managed device access.");
    }
    return { state: "ready", credential: data.credential };
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

  private async createBillingSession(action: "checkout" | "portal"): Promise<{ url: string }> {
    const { data, error } = await this.client.functions.invoke("household-billing-session", {
      body: { action },
    });
    if (error) throw error;
    if (!data || typeof data !== "object" || typeof data.url !== "string") {
      throw new Error("The Luna account service returned an invalid billing session.");
    }
    const url = new URL(data.url);
    if (url.protocol !== "https:") {
      throw new Error("The Luna account service returned an invalid billing session.");
    }
    return { url: url.toString() };
  }
}

function singleHouseholdIntelligenceAccessRow(data: unknown): HouseholdIntelligenceAccessRow {
  const row = Array.isArray(data) ? data[0] : data;
  if (!row || typeof row !== "object") {
    throw new Error("The Luna account service returned invalid Household Intelligence access.");
  }
  const candidate = row as Record<string, unknown>;
  const validEntitlementStates = new Set(["free", "checkout_pending", "entitled", "payment_problem", "ended"]);
  const validDeviceStates = new Set(["not_applicable", "pending", "ready", "revoked"]);
  if (
    typeof candidate.household_id !== "string"
    || (candidate.plan_code !== "free" && candidate.plan_code !== "managed")
    || typeof candidate.entitlement_state !== "string"
    || !validEntitlementStates.has(candidate.entitlement_state)
    || typeof candidate.device_state !== "string"
    || !validDeviceStates.has(candidate.device_state)
    || (candidate.entitlement_source !== null
      && candidate.entitlement_source !== "complimentary"
      && candidate.entitlement_source !== "billing")
    || (candidate.max_budget_usd !== null && typeof candidate.max_budget_usd !== "number")
    || (candidate.valid_until !== null && typeof candidate.valid_until !== "string")
    || (candidate.credential_expires_at !== null && typeof candidate.credential_expires_at !== "string")
  ) {
    throw new Error("The Luna account service returned invalid Household Intelligence access.");
  }
  return candidate as HouseholdIntelligenceAccessRow;
}

function mapHouseholdIntelligenceAccess(row: HouseholdIntelligenceAccessRow): HouseholdIntelligenceAccess {
  return {
    householdId: row.household_id as HouseholdId,
    plan: row.plan_code,
    entitlementState: row.entitlement_state === "checkout_pending"
      ? "checkoutPending"
      : row.entitlement_state === "payment_problem"
      ? "paymentProblem"
      : row.entitlement_state,
    deviceState: row.device_state === "not_applicable" ? "notApplicable" : row.device_state,
    entitlementSource: row.entitlement_source,
    maxBudgetUsd: row.max_budget_usd,
    validUntil: row.valid_until,
    credentialExpiresAt: row.credential_expires_at,
  };
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
    || (
      candidate.authorization_public_key !== undefined
      && typeof candidate.authorization_public_key !== "string"
    )
    || (
      candidate.activated_key_epoch !== undefined
      && typeof candidate.activated_key_epoch !== "number"
    )
    || (
      candidate.revoked_after_key_epoch != null
      && typeof candidate.revoked_after_key_epoch !== "number"
    )
    || (
      candidate.revoked_after_sequence != null
      && typeof candidate.revoked_after_sequence !== "number"
    )
    || (
      candidate.revoked_after_event_digest != null
      && typeof candidate.revoked_after_event_digest !== "string"
    )
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
    authorizationPublicKey: row.authorization_public_key,
    activatedKeyEpoch: row.activated_key_epoch,
    revokedAfter: (
      typeof row.revoked_after_key_epoch === "number"
      && typeof row.revoked_after_sequence === "number"
      && typeof row.revoked_after_event_digest === "string"
    ) ? {
      keyEpoch: row.revoked_after_key_epoch,
      sequence: row.revoked_after_sequence,
      eventDigest: row.revoked_after_event_digest,
    } : undefined,
    keyEpoch: row.key_epoch,
    status: row.device_status,
  };
}
