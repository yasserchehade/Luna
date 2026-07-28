import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import { createLiteLlmManagedAccessClient } from "../_shared/liteLlmManagedAccessClient.ts";
import { handleManagedIntelligenceProvisioning } from "../_shared/managedIntelligenceProvisioning.ts";

const supabaseUrl = requiredEnvironment("SUPABASE_URL");
const publishableKey = requiredAnyEnvironment("SUPABASE_PUBLISHABLE_KEY", "SUPABASE_ANON_KEY");
const serviceRoleKey = requiredEnvironment("SUPABASE_SERVICE_ROLE_KEY");
const admin = createClient(supabaseUrl, serviceRoleKey, { auth: { persistSession: false } });
const gateway = createLiteLlmManagedAccessClient({
  adminEndpoint: requiredEnvironment("LITELLM_ADMIN_URL"),
  endpoint: requiredEnvironment("LUNA_MANAGED_INTELLIGENCE_URL"),
  masterKey: requiredEnvironment("LITELLM_MASTER_KEY"),
  durationHours: Number(Deno.env.get("LITELLM_DEVICE_KEY_DURATION_HOURS") ?? "24"),
  rpmLimit: Number(Deno.env.get("LITELLM_HOUSEHOLD_RPM_LIMIT") ?? "6"),
  tpmLimit: Number(Deno.env.get("LITELLM_HOUSEHOLD_TPM_LIMIT") ?? "8000"),
  fetch,
});
const corsHeaders = {
  "access-control-allow-origin": "*",
  "access-control-allow-headers": "authorization, x-client-info, apikey, content-type",
};

Deno.serve(async (request) => {
  if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: corsHeaders });
  const response = await handleManagedIntelligenceProvisioning(request, {
    async authorizeDevice(incoming, proof) {
      const authorization = incoming.headers.get("authorization");
      if (!authorization?.startsWith("Bearer ")) return null;
      const account = createClient(supabaseUrl, publishableKey, {
        auth: { persistSession: false },
        global: { headers: { authorization } },
      });
      const { data, error } = await account.rpc("authorize_managed_intelligence_device_provisioning", {
        requested_device_public_key: proof.devicePublicKey,
        requested_challenge_id: proof.challengeId,
        requested_nonce: proof.nonce,
        requested_authorization_signature: proof.authorizationSignature,
      });
      const row = Array.isArray(data) ? data[0] : data;
      if (error || !row) return null;
      return {
        householdId: row.household_id,
        deviceId: row.device_id,
        existingAlias: row.existing_gateway_key_alias,
        budgetScopeId: row.budget_scope_id,
        maxBudgetUsd: Number(row.max_budget_usd),
      };
    },
    createGatewayAccess: gateway.createGatewayAccess,
    revokeGatewayAccess: gateway.revokeGatewayAccess,
    revokeGatewayAccessByAlias: gateway.revokeGatewayAccessByAlias,
    async reserveGatewayAlias(input) {
      const { error } = await admin.rpc("reserve_managed_intelligence_device_gateway_alias", {
        requested_household_id: input.householdId,
        requested_device_id: input.deviceId,
        requested_gateway_key_alias: input.alias,
        requested_budget_scope_id: input.budgetScopeId,
        requested_max_budget_usd: input.maxBudgetUsd,
      });
      if (error) throw new Error("Managed Trusted Device access is no longer eligible.");
    },
    async recordReady(input) {
      const { error } = await admin.rpc("record_managed_intelligence_device_access", {
        requested_household_id: input.householdId,
        requested_device_id: input.deviceId,
        requested_status: "ready",
        requested_gateway_key_alias: input.alias,
        requested_credential_expires_at: input.expiresAt,
      });
      if (error) throw new Error("Managed Trusted Device access could not be recorded.");
    },
  });
  for (const [name, value] of Object.entries(corsHeaders)) response.headers.set(name, value);
  return response;
});

function requiredEnvironment(name: string): string {
  const value = Deno.env.get(name)?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function requiredAnyEnvironment(...names: string[]): string {
  for (const name of names) {
    const value = Deno.env.get(name)?.trim();
    if (value) return value;
  }
  throw new Error(`${names.join(" or ")} is required`);
}
