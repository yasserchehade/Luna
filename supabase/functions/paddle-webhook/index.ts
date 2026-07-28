import { createClient } from "npm:@supabase/supabase-js@2.110.2";
import {
  handlePaddleWebhook,
  type PaddleSubscriptionEvent,
} from "../_shared/paddleWebhook.ts";

const supabaseUrl = requiredEnvironment("SUPABASE_URL");
const serviceRoleKey = requiredEnvironment("SUPABASE_SERVICE_ROLE_KEY");
const webhookSecret = requiredEnvironment("PADDLE_WEBHOOK_SECRET");
const expectedPriceId = requiredEnvironment("PADDLE_MANAGED_PRICE_ID");
const managedRequestLimit = Number(requiredEnvironment("PADDLE_MANAGED_REQUEST_LIMIT"));
const supabase = createClient(supabaseUrl, serviceRoleKey, {
  auth: { persistSession: false },
});

Deno.serve((request) => handlePaddleWebhook(request, {
  webhookSecret,
  expectedPriceId,
  managedRequestLimit,
  now: () => new Date(),
  async applySubscriptionEvent(event: PaddleSubscriptionEvent) {
    const { data, error } = await supabase.rpc("apply_paddle_subscription_event", {
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
    if (error) throw new Error("The Paddle subscription event could not be applied.");
    return { applied: data === true };
  },
}));

function requiredEnvironment(name: string): string {
  const value = Deno.env.get(name)?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}
