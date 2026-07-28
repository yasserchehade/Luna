import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import { createPaddleBillingClient } from "../_shared/paddleBillingClient.ts";
import {
  handleHouseholdBillingSession,
  type HouseholdBillingContext,
} from "../_shared/householdBillingSession.ts";

const supabaseUrl = requiredEnvironment("SUPABASE_URL");
const publishableKey = requiredAnyEnvironment("SUPABASE_PUBLISHABLE_KEY", "SUPABASE_ANON_KEY");
const serviceRoleKey = requiredEnvironment("SUPABASE_SERVICE_ROLE_KEY");
const admin = createClient(supabaseUrl, serviceRoleKey, { auth: { persistSession: false } });
const paddle = createPaddleBillingClient({
  apiBaseUrl: Deno.env.get("PADDLE_API_BASE_URL") ?? "https://sandbox-api.paddle.com",
  apiKey: requiredEnvironment("PADDLE_API_KEY"),
  managedPriceId: requiredEnvironment("PADDLE_MANAGED_PRICE_ID"),
  fetch,
});
const corsHeaders = {
  "access-control-allow-origin": "*",
  "access-control-allow-headers": "authorization, x-client-info, apikey, content-type",
};

Deno.serve(async (request) => {
  if (request.method === "OPTIONS") return new Response(null, { status: 204, headers: corsHeaders });
  const response = await handleHouseholdBillingSession(request, {
    async authenticateOrganiser(incoming): Promise<HouseholdBillingContext | null> {
      const authorization = incoming.headers.get("authorization");
      if (!authorization?.startsWith("Bearer ")) return null;
      const account = createClient(supabaseUrl, publishableKey, {
        auth: { persistSession: false },
        global: { headers: { authorization } },
      });
      const { data, error } = await account.rpc("current_household_billing_context");
      const row = Array.isArray(data) ? data[0] : data;
      if (error || !row) return null;
      return {
        householdId: row.household_id,
        email: row.organiser_email,
        externalCustomerId: row.external_customer_id,
        externalSubscriptionId: row.external_subscription_id,
      };
    },
    createCheckout: paddle.createCheckout,
    createCustomerPortal: paddle.createCustomerPortal,
    async recordCheckoutPending(householdId, transactionId) {
      const { error } = await admin.rpc("record_paddle_checkout_pending", {
        requested_household_id: householdId,
        requested_transaction_id: transactionId,
      });
      if (error) throw new Error("The Paddle checkout could not be recorded.");
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
