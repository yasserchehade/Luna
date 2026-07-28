import assert from "node:assert/strict";
import test from "node:test";
import type {
  CloudConsentScope,
  ConversationService,
  IntelligenceProviderStatus,
} from "../conversation/conversationService";
import {
  cloudAssistanceLoadErrorMessage,
  loadCloudAssistanceOptionsData,
  providerApiKeysLinkLabel,
  providerSetupAvailability,
} from "./CloudAssistanceOptions";

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
    "listIntelligenceProviderStatuses" | "listCloudConsentScopes"
  > = {
    async listIntelligenceProviderStatuses() {
      return providers;
    },
    async listCloudConsentScopes(): Promise<CloudConsentScope[]> {
      throw new Error("protected Household intelligence state is unavailable");
    },
  };

  const result = await loadCloudAssistanceOptionsData(service, "household");

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
      statusLabel: "Luna access unavailable",
    },
  );
});
