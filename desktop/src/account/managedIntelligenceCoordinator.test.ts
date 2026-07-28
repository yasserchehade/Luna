import assert from "node:assert/strict";
import test from "node:test";
import type { AccountService, HouseholdIntelligenceAccess, HouseholdSession } from "./accountService";
import { synchronizeManagedIntelligenceAccess } from "./managedIntelligenceCoordinator";
import type { ConversationService } from "../conversation/conversationService";
import type { TrustedDeviceService } from "../trusted-device/trustedDeviceService";

const session = {
  accountId: "account" as never,
  organiserName: "Alex Morgan",
  email: "alex@example.com",
  householdId: "household" as never,
  householdName: "Morgan Household",
} satisfies HouseholdSession;

function entitledAccess(expiresAt: string): HouseholdIntelligenceAccess {
  return {
    householdId: session.householdId,
    plan: "managed",
    entitlementState: "entitled",
    deviceState: "ready",
    entitlementSource: "complimentary",
    maxBudgetUsd: 1,
    validUntil: "2026-09-01T00:00:00.000Z",
    credentialExpiresAt: expiresAt,
  };
}

test("a current managed credential is retained without unnecessary reprovisioning", async () => {
  let provisioned = 0;
  let cleared = 0;
  await synchronizeManagedIntelligenceAccess({
    async getHouseholdIntelligenceAccess(devicePublicKey?: string) {
      assert.equal(devicePublicKey, "age1device");
      return entitledAccess("2026-07-29T14:00:00.000Z");
    },
    async beginManagedIntelligenceDeviceProvisioning() {
      provisioned += 1;
      throw new Error("must not provision");
    },
  } as unknown as AccountService, {
    async listIntelligenceProviderStatuses() {
      return [{
        descriptor: {
          id: "managed-openai",
          name: "OpenAI",
          description: "Managed",
          models: [{ id: "gpt-4.1-mini", name: "GPT-4.1 mini" }],
          managedByLuna: true,
          authUrl: null,
        },
        gatewayConfigured: true,
        configured: true,
      }];
    },
    async clearManagedIntelligenceGatewayCredential() {
      cleared += 1;
    },
  } as unknown as ConversationService, {
    async currentDevicePublicKey() {
      return "age1device";
    },
  } as unknown as TrustedDeviceService, session, new Date("2026-07-28T12:00:00.000Z"));

  assert.equal(provisioned, 0);
  assert.equal(cleared, 0);
});

test("an expiring managed credential is renewed and replaced in the OS vault", async () => {
  const stored: string[] = [];
  await synchronizeManagedIntelligenceAccess({
    async getHouseholdIntelligenceAccess() {
      return entitledAccess("2026-07-28T12:30:00.000Z");
    },
    async beginManagedIntelligenceDeviceProvisioning() {
      return { id: "challenge", nonce: "nonce", expiresAt: "2026-07-28T12:05:00.000Z" };
    },
    async provisionManagedIntelligenceDeviceAccess(request: {
      devicePublicKey: string;
      challengeId: string;
      nonce: string;
      authorizationSignature: string;
    }) {
      assert.equal(request.authorizationSignature, "signature");
      return { state: "ready", credential: "renewed-managed-key" };
    },
  } as unknown as AccountService, {
    async listIntelligenceProviderStatuses() {
      return [{
        descriptor: {
          id: "managed-openai",
          name: "OpenAI",
          description: "Managed",
          models: [{ id: "gpt-4.1-mini", name: "GPT-4.1 mini" }],
          managedByLuna: true,
          authUrl: null,
        },
        gatewayConfigured: true,
        configured: true,
      }];
    },
    async setManagedIntelligenceGatewayCredential(_householdId: string, credential: string) {
      stored.push(credential);
    },
  } as unknown as ConversationService, {
    async currentDevicePublicKey() {
      return "age1device";
    },
    async signManagedIntelligenceDeviceProvisioning() {
      return "signature";
    },
  } as unknown as TrustedDeviceService, session, new Date("2026-07-28T12:00:00.000Z"));

  assert.deepEqual(stored, ["renewed-managed-key"]);
});

test("loss of Household entitlement clears only the managed credential", async () => {
  const cleared: string[] = [];
  await synchronizeManagedIntelligenceAccess({
    async getHouseholdIntelligenceAccess() {
      return {
        ...entitledAccess("2026-07-29T14:00:00.000Z"),
        plan: "free",
        entitlementState: "ended",
        deviceState: "notApplicable",
      };
    },
  } as unknown as AccountService, {
    async clearManagedIntelligenceGatewayCredential(householdId: string) {
      cleared.push(householdId);
    },
  } as unknown as ConversationService, {
    async currentDevicePublicKey() {
      return "age1device";
    },
  } as unknown as TrustedDeviceService, session);

  assert.deepEqual(cleared, [session.householdId]);
});
