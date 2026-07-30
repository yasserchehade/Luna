import assert from "node:assert/strict";
import test from "node:test";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";
import {
  cloudAssistanceLoadErrorMessage,
  defaultRouteSelection,
  loadCloudAssistanceOptionsData,
  providerApiKeysLinkLabel,
  providerSetupAvailability,
} from "./CloudAssistanceOptions";

test("a stale saved default proposes the configured replacement route", () => {
  const providers: IntelligenceProviderStatus[] = [{
    descriptor: {
      id: "openai",
      name: "OpenAI",
      description: "Luna-managed Cloud Assistance",
      models: [{ id: "gpt-4.1-mini", name: "GPT-4.1 mini" }],
      managedByLuna: true,
      authUrl: null,
    },
    gatewayConfigured: true,
    configured: true,
  }];

  assert.equal(
    defaultRouteSelection({
      providerId: "openai",
      modelId: "gpt-5.6-luna",
      invalid: false,
    }, providers, ""),
    "openai::gpt-4.1-mini",
  );
});

test("an available saved default remains the selected exact route", () => {
  const providers: IntelligenceProviderStatus[] = [{
    descriptor: {
      id: "openai",
      name: "OpenAI",
      description: "Luna-managed Cloud Assistance",
      models: [{ id: "gpt-4.1-mini", name: "GPT-4.1 mini" }],
      managedByLuna: true,
      authUrl: null,
    },
    gatewayConfigured: true,
    configured: true,
  }];

  assert.equal(
    defaultRouteSelection({
      providerId: "openai",
      modelId: "gpt-4.1-mini",
      invalid: false,
    }, providers, ""),
    "openai::gpt-4.1-mini",
  );
});

test("provider controls remain available when protected Consent Grants cannot be opened", async () => {
  const providers: IntelligenceProviderStatus[] = [{
    descriptor: {
      id: "byok-openai",
      name: "OpenAI — bring your own key",
      description: "Customer-billed Cloud Assistance",
      models: [{ id: "gpt-4.1-mini", name: "GPT-4.1 mini" }],
      managedByLuna: false,
      authUrl: "https://platform.openai.com/api-keys",
    },
    gatewayConfigured: true,
    configured: false,
  }];
  const service: Pick<
    ConversationService,
    | "listIntelligenceProviderStatuses"
    | "listCloudConsentScopes"
    | "getDefaultIntelligenceProvider"
  > = {
    async listIntelligenceProviderStatuses() {
      return providers;
    },
    async listCloudConsentScopes(): Promise<CloudConsentScope[]> {
      throw new Error("protected Household intelligence state is unavailable");
    },
    async getDefaultIntelligenceProvider() {
      return null;
    },
  };

  const result = await loadCloudAssistanceOptionsData(service, {
    async getHouseholdIntelligenceAccess() {
      return {
        householdId: "household" as never,
        plan: "free",
        entitlementState: "free",
        deviceState: "notApplicable",
        entitlementSource: null,
        maxBudgetUsd: null,
        validUntil: null,
        credentialExpiresAt: null,
      };
    },
    async createManagedIntelligenceCheckoutSession() {
      return { url: "https://pay.paddle.io/test" };
    },
    async createManagedIntelligenceCustomerPortalSession() {
      return { url: "https://customer-portal.paddle.com/test" };
    },
  }, {
    async currentDevicePublicKey() {
      return "age1trusteddevice";
    },
  }, "household" as never);

  assert.deepEqual(result.providers, providers);
  assert.deepEqual(result.scopes, []);
  assert.match(
    result.consentError,
    /protected Household intelligence state is unavailable/,
  );
  assert.equal(result.providerError, "");
});

test("protected-state failures use customer language without exposing implementation details", () => {
  const message = cloudAssistanceLoadErrorMessage(
    "consents",
    "protected Household intelligence state is unavailable",
  );

  assert.match(message, /older Consent Grants/);
  assert.match(message, /Provider setup remains available/);
  assert.doesNotMatch(message, /protected Household intelligence state/i);
});

test("provider key links do not repeat Open in the OpenAI name", () => {
  assert.equal(
    providerApiKeysLinkLabel("OpenAI — bring your own key"),
    "OpenAI API keys",
  );
});

test("provider key setup waits for automatically provisioned Luna access", () => {
  assert.deepEqual(
    providerSetupAvailability({
      gatewayConfigured: false,
      configured: false,
    }),
    {
      enabled: false,
      statusLabel: "Provider setup unavailable",
    },
  );
});
