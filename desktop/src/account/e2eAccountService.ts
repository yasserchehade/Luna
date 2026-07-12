import type {
  AccountService,
  HouseholdId,
  LunaAccountId,
  RegisterAccountRequest,
} from "./accountService";

let registration: RegisterAccountRequest | undefined;
let verified = false;
let householdSession: Awaited<ReturnType<AccountService["createHousehold"]>> | undefined;

export const e2eAccountService: AccountService = {
  async register(request) {
    registration = request;
    return { email: request.email };
  },
  async verifyEmail(email, code) {
    if (!registration || registration.email !== email || code !== "123456") {
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
      householdId: "household-rivera" as HouseholdId,
      householdName,
    };
    return householdSession;
  },
  async signIn(email, password) {
    if (!registration || !householdSession || registration.email !== email || registration.password !== password) {
      throw new Error("Invalid credentials.");
    }
    return householdSession;
  },
  async signOut() {
    return;
  },
};
