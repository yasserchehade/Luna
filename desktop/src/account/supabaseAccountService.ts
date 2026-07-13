import { createClient, type SupabaseClient } from "@supabase/supabase-js";
import type {
  AccountService,
  HouseholdId,
  HouseholdSession,
  LunaAccountId,
  RegisterAccountRequest,
  VerificationRequested,
} from "./accountService";

type HouseholdRow = {
  account_id: string;
  organiser_name: string;
  email: string;
  household_id: string;
  household_name: string;
};

const accountExistsCodes = new Set(["email_exists", "user_already_exists"]);

export class SupabaseAccountService implements AccountService {
  private readonly client: SupabaseClient;

  constructor(url: string, publishableKey: string) {
    this.client = createClient(url, publishableKey, {
      auth: {
        autoRefreshToken: true,
        detectSessionInUrl: false,
        flowType: "pkce",
        persistSession: false,
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
    return mapHousehold(singleRow(data));
  }

  async requestPasswordReset(email: string): Promise<void> {
    const { error } = await this.client.auth.resetPasswordForEmail(email);
    if (error) throw error;
  }

  async resetPassword(email: string, code: string, newPassword: string): Promise<void> {
    const verification = await this.client.auth.verifyOtp({
      email,
      token: code,
      type: "recovery",
    });
    if (verification.error) throw verification.error;

    const update = await this.client.auth.updateUser({ password: newPassword });
    if (update.error) throw update.error;
  }

  async signIn(email: string, password: string): Promise<HouseholdSession> {
    const { error } = await this.client.auth.signInWithPassword({ email, password });
    if (error) throw error;

    const { data, error: householdError } = await this.client.rpc("current_household");
    if (householdError) throw householdError;
    return mapHousehold(singleRow(data));
  }

  async signOut(): Promise<void> {
    const { error } = await this.client.auth.signOut({ scope: "local" });
    if (error) throw error;
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
