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
        existingAlias: null,
        budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
        maxBudgetUsd: 1,
      };
    },
    async createGatewayAccess(input) {
      assert.deepEqual(input, {
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
        alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
        budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
        maxBudgetUsd: 1,
      });
      return {
        alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
        credential: "sk-narrow-device-key",
        expiresAt: "2026-07-29T14:00:00.000Z",
      };
    },
    async reserveGatewayAlias(input) {
      assert.deepEqual(input, {
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
        alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
        budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
        maxBudgetUsd: 1,
      });
    },
    async recordReady(input) {
      recorded.push(input);
    },
  });

  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {
    state: "ready",
    credential: "sk-narrow-device-key",
    expiresAt: "2026-07-29T14:00:00.000Z",
  });
  assert.deepEqual(recorded, [{
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
    alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
    expiresAt: "2026-07-29T14:00:00.000Z",
  }]);
});

test("renewal revokes the previous device alias before minting its replacement", async () => {
  const actions: string[] = [];
  const response = await handleManagedIntelligenceProvisioning(new Request(
    "https://luna.test/managed-intelligence-provisioning",
    { method: "POST", body: JSON.stringify({
      devicePublicKey: "age1trusteddevice",
      challengeId: "f462a4ac-9688-4c23-90e7-8a9f449b975d",
      nonce: "d916a996-710d-4a43-84ac-b28427151a7f",
      authorizationSignature: "base64-device-signature",
    }) },
  ), {
    async authorizeDevice() {
      return {
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
        existingAlias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
        budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
        maxBudgetUsd: 1,
      };
    },
    async revokeGatewayAccessByAlias(alias) {
      actions.push(`revoke:${alias}`);
    },
    async reserveGatewayAlias({ alias }) {
      actions.push(`reserve:${alias}`);
    },
    async createGatewayAccess() {
      actions.push("generate");
      return {
        alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
        credential: "renewed-key",
        expiresAt: "2026-07-29T14:00:00.000Z",
      };
    },
    async recordReady() {
      actions.push("ready");
    },
  });

  assert.equal(response.status, 200);
  assert.deepEqual(actions, [
    "revoke:luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
    "reserve:luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
    "generate",
    "ready",
  ]);
});

test("an entitlement revoked before alias reservation cannot mint a gateway key", async () => {
  let generated = false;
  await assert.rejects(() => handleManagedIntelligenceProvisioning(new Request(
    "https://luna.test/managed-intelligence-provisioning",
    { method: "POST", body: JSON.stringify({
      devicePublicKey: "age1trusteddevice",
      challengeId: "f462a4ac-9688-4c23-90e7-8a9f449b975d",
      nonce: "d916a996-710d-4a43-84ac-b28427151a7f",
      authorizationSignature: "base64-device-signature",
    }) },
  ), {
    async authorizeDevice() {
      return {
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
        existingAlias: null,
        budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
        maxBudgetUsd: 1,
      };
    },
    async reserveGatewayAlias() {
      throw new Error("entitlement revoked");
    },
    async createGatewayAccess() {
      generated = true;
      throw new Error("must not run");
    },
    async recordReady() {},
  }), /entitlement revoked/);
  assert.equal(generated, false);
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
    durationHours: 24,
    rpmLimit: 6,
    tpmLimit: 8_000,
    now: () => new Date("2026-07-28T14:00:00.000Z"),
    fetch: async (url, init) => {
      requests.push({ url: String(url), init });
      const payload = String(url).endsWith("/key/generate")
        ? { key: "sk-narrow-device-key", expires: "2026-07-29T14:00:00.000Z" }
        : {};
      return new Response(JSON.stringify(payload), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });

  const access = await client.createGatewayAccess({
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
    alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
    budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
    maxBudgetUsd: 1,
  });

  assert.equal(access.credential, "sk-narrow-device-key");
  assert.equal(access.expiresAt, "2026-07-29T14:00:00.000Z");
  assert.equal(requests[0].url, "https://gateway-admin.luna.test/team/new");
  const team = JSON.parse(String(requests[0].init?.body));
  const expectedTeamId = "luna-household-d70c8675-0261-4797-b6df-4109c3d678cd-49c52e29-bd9b-4bcb-a4e8-030f9de11111";
  assert.equal(team.team_id, expectedTeamId);
  assert.equal(team.max_budget, 1);
  assert.equal(team.rpm_limit, 6);
  assert.equal(team.tpm_limit, 8_000);
  assert.equal(requests[1].url, "https://gateway-admin.luna.test/key/generate");
  const key = JSON.parse(String(requests[1].init?.body));
  assert.deepEqual(key.models, ["openai/gpt-4.1-mini"]);
  assert.deepEqual(key.allowed_routes, ["/v1/chat/completions"]);
  assert.equal(key.duration, "24h");
  assert.equal(key.team_id, expectedTeamId);
  assert.equal(key.max_budget, undefined, "the cap must be shared by the Household team, not multiplied per key");
  assert.equal(key.rpm_limit, 6);
  assert.equal(key.tpm_limit, 8_000);
  assert.deepEqual(key.metadata, {
    purpose: "luna-managed-intelligence",
    household_id: "d70c8675-0261-4797-b6df-4109c3d678cd",
    trusted_device_id: "acdf892b-1967-4376-82b2-e144ff480740",
    budget_scope_id: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
  });
  assert.doesNotMatch(String(requests[1].init?.body), /master-secret|provider|document|email/i);
});

test("managed gateway credentials cannot exceed the 24-hour safety bound", () => {
  assert.throws(() => createLiteLlmManagedAccessClient({
    adminEndpoint: "https://gateway-admin.luna.test",
    endpoint: "https://gateway.luna.test/v1/chat/completions",
    masterKey: "sk-litellm-master-secret",
    durationHours: 240,
    rpmLimit: 6,
    tpmLimit: 8_000,
    fetch,
  }), /credential duration is invalid/);
});

test("an overlong generated expiry is rejected and its key is revoked", async () => {
  const requests: Array<{ url: string; init?: RequestInit }> = [];
  const client = createLiteLlmManagedAccessClient({
    adminEndpoint: "https://gateway-admin.luna.test",
    endpoint: "https://gateway.luna.test/v1/chat/completions",
    masterKey: "sk-litellm-master-secret",
    durationHours: 24,
    rpmLimit: 6,
    tpmLimit: 8_000,
    now: () => new Date("2026-07-28T14:00:00.000Z"),
    fetch: async (url, init) => {
      requests.push({ url: String(url), init });
      const payload = String(url).endsWith("/key/generate")
        ? { key: "sk-overlong-device-key", expires: "2026-07-30T14:00:00.000Z" }
        : {};
      return new Response(JSON.stringify(payload), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });

  await assert.rejects(() => client.createGatewayAccess({
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    deviceId: "acdf892b-1967-4376-82b2-e144ff480740",
    alias: "luna-managed-acdf892b-1967-4376-82b2-e144ff480740",
    budgetScopeId: "49c52e29-bd9b-4bcb-a4e8-030f9de11111",
    maxBudgetUsd: 1,
  }), /unsafe credential expiry/);
  assert.equal(requests.at(-1)?.url, "https://gateway-admin.luna.test/key/delete");
  assert.deepEqual(JSON.parse(String(requests.at(-1)?.init?.body)), {
    keys: ["sk-overlong-device-key"],
  });
});
