export type RegisterAccountRequest = {
  organiserName: string;
  email: string;
  password: string;
};

export type VerificationRequested = {
  email: string;
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

export interface AccountService {
  register(request: RegisterAccountRequest): Promise<VerificationRequested>;
  verifyEmail(email: string, code: string): Promise<void>;
  createHousehold(name: string): Promise<HouseholdSession>;
  requestPasswordReset(email: string): Promise<void>;
  resetPassword(email: string, code: string, newPassword: string): Promise<void>;
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
  async signIn() {
    throw new Error("The Luna account service is not configured.");
  },
  async signOut() {
    throw new Error("The Luna account service is not configured.");
  },
};
