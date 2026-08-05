import assert from "node:assert/strict";
import test from "node:test";
import type { ConversationService } from "./conversationService";
import {
  loadDocumentCloudAssistanceData,
  resolveHouseholdAgentArrival,
} from "./ConversationWorkspace";

test("protected consent recovery failure does not hide a configured managed provider", async () => {
  const providers = [{
    descriptor: {
      id: "managed-openai",
      name: "OpenAI",
      description: "Managed",
      models: [{ id: "gpt-5.6-luna", name: "GPT-5.6 Luna" }],
      managedByLuna: true,
      authUrl: null,
    },
    gatewayConfigured: true,
    configured: true,
  }];

  const result = await loadDocumentCloudAssistanceData({
    async listIntelligenceProviderStatuses() {
      return providers;
    },
    async listCloudConsentScopes() {
      throw new Error("protected consent memory could not be opened");
    },
  } as Pick<ConversationService, "listIntelligenceProviderStatuses" | "listCloudConsentScopes">, "household");

  assert.deepEqual(result.providers, providers);
  assert.deepEqual(result.scopes, []);
  assert.match(result.scopeError, /protected consent memory/);
  assert.equal(result.providerError, "");
});

test("a clarification reply resumes the single open Household Work after restart", () => {
  const arrival = resolveHouseholdAgentArrival(
    [{ id: 42 }],
    [{
      sourceRefs: ["document-42"],
      status: "needsClarification",
    }],
    null,
    "The rental property.",
  );

  assert.equal(arrival?.id, 42);
});

test("an ambiguous reply does not guess between multiple open Household Work items", () => {
  const arrival = resolveHouseholdAgentArrival(
    [{ id: 42 }, { id: 43 }],
    [
      { sourceRefs: ["document-42"], status: "needsClarification" },
      { sourceRefs: ["document-43"], status: "active" },
    ],
    null,
    "The rental property.",
  );

  assert.equal(arrival, null);
});
