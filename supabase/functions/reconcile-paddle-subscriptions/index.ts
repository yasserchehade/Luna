import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import { createPaddleBillingClient } from "../_shared/paddleBillingClient.ts";
import { reconcilePaddleSubscriptions } from "../_shared/reconcilePaddleSubscriptions.ts";

const supabaseUrl = requiredEnvironment("SUPABASE_URL");
const serviceRoleKey = requiredEnvironment("SUPABASE_SERVICE_ROLE_KEY");
const reconcileSecret = requiredEnvironment("LUNA_RECONCILIATION_SECRET");
const requestLimit = Number(requiredEnvironment("PADDLE_MANAGED_REQUEST_LIMIT"));
const admin = createClient(supabaseUrl, serviceRoleKey, { auth: { persistSession: false } });
const paddle = createPaddleBillingClient({
  apiBaseUrl: Deno.env.get("PADDLE_API_BASE_URL") ?? "https://sandbox-api.paddle.com",
  apiKey: requiredEnvironment("PADDLE_API_KEY"),
  managedPriceId: requiredEnvironment("PADDLE_MANAGED_PRICE_ID"),
  fetch,
});

Deno.serve(async (request) => {
  if (request.method !== "POST") return Response.json({ error: "Method not allowed" }, { status: 405 });
  if (request.headers.get("x-luna-reconciliation-secret") !== reconcileSecret) {
    return Response.json({ error: "Reconciliation authority is required" }, { status: 401 });
  }
  const result = await reconcilePaddleSubscriptions({
    requestLimit,
    async listSubscriptions() {
      const { data, error } = await admin.rpc("paddle_subscriptions_for_reconciliation");
      if (error) throw new Error("Paddle subscriptions could not be loaded.");
      return (data ?? []).map((row: Record<string, string>) => ({
        householdId: row.household_id,
        customerId: row.external_customer_id,
        subscriptionId: row.external_subscription_id,
      }));
    },
    getPaddleSubscription: paddle.getPaddleSubscription,
    async applySubscriptionEvent(event) {
      const { data, error } = await admin.rpc("apply_paddle_subscription_event", {
        requested_event_id: event.eventId,
        requested_event_type: event.eventType,
        requested_occurred_at: event.occurredAt,
        requested_household_id: event.householdId,
        requested_customer_id: event.customerId,
        requested_subscription_id: event.subscriptionId,
        requested_status: event.status,
        requested_valid_until: event.validUntil,
        requested_request_limit: event.requestLimit,
      });
      if (error) throw new Error("Reconciled Paddle subscription state could not be applied.");
      return { applied: data === true };
    },
  });
  return Response.json(result, { status: result.failed === 0 ? 200 : 503 });
});

function requiredEnvironment(name: string): string {
  const value = Deno.env.get(name)?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}
