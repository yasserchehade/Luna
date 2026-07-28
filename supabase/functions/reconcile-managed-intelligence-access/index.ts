import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import { createLiteLlmManagedAccessClient } from "../_shared/liteLlmManagedAccessClient.ts";
import { reconcileManagedIntelligenceAccess } from "../_shared/reconcileManagedIntelligenceAccess.ts";

const supabaseUrl = requiredEnvironment("SUPABASE_URL");
const serviceRoleKey = requiredEnvironment("SUPABASE_SERVICE_ROLE_KEY");
const reconcileSecret = requiredEnvironment("LUNA_RECONCILIATION_SECRET");
const admin = createClient(supabaseUrl, serviceRoleKey, { auth: { persistSession: false } });
const gateway = createLiteLlmManagedAccessClient({
  adminEndpoint: requiredEnvironment("LITELLM_ADMIN_URL"),
  endpoint: requiredEnvironment("LUNA_MANAGED_INTELLIGENCE_URL"),
  masterKey: requiredEnvironment("LITELLM_MASTER_KEY"),
  durationHours: Number(Deno.env.get("LITELLM_DEVICE_KEY_DURATION_HOURS") ?? "24"),
  requestTimeoutMs: Number(Deno.env.get("LITELLM_ADMIN_REQUEST_TIMEOUT_MS") ?? "10000"),
  rpmLimit: Number(Deno.env.get("LITELLM_HOUSEHOLD_RPM_LIMIT") ?? "6"),
  tpmLimit: Number(Deno.env.get("LITELLM_HOUSEHOLD_TPM_LIMIT") ?? "8000"),
  fetch,
});

Deno.serve(async (request) => {
  if (request.method !== "POST") return Response.json({ error: "Method not allowed" }, { status: 405 });
  if (request.headers.get("x-luna-reconciliation-secret") !== reconcileSecret) {
    return Response.json({ error: "Reconciliation authority is required" }, { status: 401 });
  }
  const result = await reconcileManagedIntelligenceAccess({
    async listPendingRevocations() {
      const { data, error } = await admin.rpc("pending_managed_intelligence_revocations");
      if (error) throw new Error("Managed access revocations could not be loaded.");
      return (data ?? []).map((row: Record<string, string>) => ({
        householdId: row.household_id,
        deviceId: row.device_id,
        alias: row.gateway_key_alias,
      }));
    },
    revokeGatewayAccessByAlias: gateway.revokeGatewayAccessByAlias,
    async recordGatewayRevoked(input) {
      const { error } = await admin.rpc("record_managed_intelligence_gateway_revoked", {
        requested_household_id: input.householdId,
        requested_device_id: input.deviceId,
      });
      if (error) throw new Error("Managed gateway revocation could not be recorded.");
    },
  });
  return Response.json(result, { status: result.failed === 0 ? 200 : 503 });
});

function requiredEnvironment(name: string): string {
  const value = Deno.env.get(name)?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}
