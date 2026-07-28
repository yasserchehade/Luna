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
  maxBudgetUsd: Number(requiredEnvironment("LITELLM_DEVICE_MAX_BUDGET_USD")),
  duration: Deno.env.get("LITELLM_DEVICE_KEY_DURATION") ?? "24h",
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
      return { householdId: row.household_id, deviceId: row.device_id };
    },
    createGatewayAccess: gateway.createGatewayAccess,
    revokeGatewayAccess: gateway.revokeGatewayAccess,
    async recordReady(input) {
      const { error } = await admin.rpc("record_managed_intelligence_device_access", {
        requested_household_id: input.householdId,
        requested_device_id: input.deviceId,
        requested_status: "ready",
        requested_gateway_key_alias: input.alias,
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
