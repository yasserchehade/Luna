import assert from "node:assert/strict";
import test from "node:test";
import { handleManagedIntelligenceProvisioning } from "../../supabase/functions/_shared/managedIntelligenceProvisioning";
import { createLiteLlmManagedAccessClient } from "../../supabase/functions/_shared/liteLlmManagedAccessClient";
import { reconcileManagedIntelligenceAccess } from "../../supabase/functions/_shared/reconcileManagedIntelligenceAccess";

test("an entitled Trusted Device receives a narrow generated gateway credential", async () => {
  const recorded: unknown[] = [];
  const response = await handleManagedIntelligenceProvisioning(new Request(
    "https://luna.test/managed-intelligence-provisioning",
    {
      method: "POST",
      headers: {
        authorization: "Bearer signed-member-token",
        "content-type": "application/json",
      },
      body: JSON.stringify({
        devicePublicKey: "age1trusteddevice",
        challengeId: "f462a4ac-9688-4c23-90e7-8a9f449b975d",
        nonce: "d916a996-710d-4a43-84ac-b28427151a7f",
        authorizationSignature: "base64-device-signature",
      }),
    },
  ), {
    async authorizeDevice(request, proof) {
      assert.equal(request.headers.get("authorization"), "Bearer signed-member-token");
      assert.equal(proof.devicePublicKey, "age1trusteddevice");
      return {
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
      };
    },
    async createGatewayAccess(input) {
      assert.deepEqual(input, {
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
      });
      return { alias: "luna-device-acdf892b", credential: "sk-narrow-device-key" };
    },
    async recordReady(input) {
      recorded.push(input);
    },
  });

  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {
    state: "ready",
    credential: "sk-narrow-device-key",
  });
  assert.deepEqual(recorded, [{
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
    alias: "luna-device-acdf892b",
  }]);
});

test("revoked Household access removes each attributable gateway key", async () => {
  const revoked: string[] = [];
  const recorded: string[] = [];
  const result = await reconcileManagedIntelligenceAccess({
    async listPendingRevocations() {
      return [{
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
        alias: "luna-managed-acdf892b",
      }];
    },
    async revokeGatewayAccessByAlias(alias) {
      revoked.push(alias);
    },
    async recordGatewayRevoked(input) {
      recorded.push(`${input.householdId}:${input.deviceId}`);
    },
  });

  assert.deepEqual(result, { revoked: 1, failed: 0 });
  assert.deepEqual(revoked, ["luna-managed-acdf892b"]);
  assert.deepEqual(recorded, [
    "d70c8675-0261-4797-b6df-4109c3d678cd:acdf892b-1967-4376-82b2-e144ff480740",
  ]);
});

test("generated LiteLLM access is device-attributed, route-limited, rate-limited, and spend-capped", async () => {
  const requests: Array<{ url: string; init?: RequestInit }> = [];
  const client = createLiteLlmManagedAccessClient({
    adminEndpoint: "https://gateway-admin.luna.test",
    endpoint: "https://gateway.luna.test/v1/chat/completions",
    masterKey: "sk-litellm-master-secret",
    maxBudgetUsd: 1,
    duration: "24h",
    fetch: async (url, init) => {
      requests.push({ url: String(url), init });
      return new Response(JSON.stringify({ key: "sk-narrow-device-key" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });

  const access = await client.createGatewayAccess({
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
  });

  assert.equal(access.credential, "sk-narrow-device-key");
  assert.equal(requests[0].url, "https://gateway-admin.luna.test/key/generate");
  const body = JSON.parse(String(requests[0].init?.body));
  assert.deepEqual(body.models, ["openai/gpt-4.1-mini"]);
  assert.deepEqual(body.allowed_routes, ["/v1/chat/completions", "/v1/models"]);
  assert.equal(body.duration, "24h");
  assert.equal(body.max_budget, 1);
  assert.equal(body.rpm_limit, 6);
  assert.equal(body.tpm_limit, 8_000);
  assert.deepEqual(body.metadata, {
    purpose: "luna-managed-intelligence",
    household_id: "d70c8675-0261-4797-b6df-4109c3d678cd",
    trusted_device_id: "acdf892b-1967-4376-82b2-e144ff480740",
  });
  assert.doesNotMatch(String(requests[0].init?.body), /master-secret|provider|document|email/i);
});
