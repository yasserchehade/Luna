import type {
  AccountService,
  HouseholdId,
  LunaAccountId,
  RegisterAccountRequest,
} from "./accountService";
import { e2eAccountFixture } from "./e2eAccountFixture";

let registration: RegisterAccountRequest | undefined;
let verified = false;
let householdSession: Awaited<ReturnType<AccountService["createHousehold"]>> | undefined;
let currentPassword = "";
let passwordResetRequestPending = false;
let recoveryCodeAvailable = false;

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
      householdId: "household-rivera" as HouseholdId,
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
  async resetPassword(email, code, newPassword) {
    if (
      !registration
      || registration.email !== email
      || code !== e2eAccountFixture.recoveryCode
      || !recoveryCodeAvailable
    ) {
      throw new Error("Invalid recovery code.");
    }
    recoveryCodeAvailable = false;
    currentPassword = newPassword;
  },
  async signIn(email, password) {
    if (!registration || !householdSession || registration.email !== email || currentPassword !== password) {
      throw new Error("Invalid credentials.");
    }
    return householdSession;
  },
  async signOut() {
    return;
  },
};
