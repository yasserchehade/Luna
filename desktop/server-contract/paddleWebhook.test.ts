import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import test from "node:test";
import {
  handlePaddleWebhook,
  type PaddleSubscriptionEvent,
} from "../../supabase/functions/_shared/paddleWebhook";

test("a signed Paddle subscription event reaches the access-critical server boundary", async () => {
  const body = JSON.stringify({
    event_id: "evt_01k1a2b3c4d5e6f7g8h9j0k1m2",
    event_type: "subscription.updated",
    occurred_at: "2026-07-28T14:00:00.000Z",
    data: {
      id: "sub_01k1a2b3c4d5e6f7g8h9j0k1m2",
      customer_id: "ctm_01k1a2b3c4d5e6f7g8h9j0k1m2",
      status: "active",
      custom_data: { household_id: "d70c8675-0261-4797-b6df-4109c3d678cd" },
      items: [{ price: { id: "pri_01k1a2b3c4d5e6f7g8h9j0k1m2" } }],
      current_billing_period: { ends_at: "2026-08-28T14:00:00.000Z" },
    },
  });
  const timestamp = "1785247200";
  const secret = "pdl_ntfset_test_secret";
  const signature = createHmac("sha256", secret)
    .update(`${timestamp}:${body}`, "utf8")
    .digest("hex");
  const applied: PaddleSubscriptionEvent[] = [];

  const response = await handlePaddleWebhook(new Request("https://luna.test/paddle-webhook", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "paddle-signature": `ts=${timestamp};h1=${signature}`,
    },
    body,
  }), {
    expectedPriceId: "pri_01k1a2b3c4d5e6f7g8h9j0k1m2",
    managedRequestLimit: 1_000,
    now: () => new Date("2026-07-28T14:00:03.000Z"),
    webhookSecret: secret,
    async applySubscriptionEvent(event) {
      applied.push(event);
      return { applied: true };
    },
  });

  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { received: true, applied: true });
  assert.deepEqual(applied, [{
    eventId: "evt_01k1a2b3c4d5e6f7g8h9j0k1m2",
    eventType: "subscription.updated",
    occurredAt: "2026-07-28T14:00:00.000Z",
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    customerId: "ctm_01k1a2b3c4d5e6f7g8h9j0k1m2",
    subscriptionId: "sub_01k1a2b3c4d5e6f7g8h9j0k1m2",
    status: "active",
    validUntil: "2026-08-28T14:00:00.000Z",
    requestLimit: 1_000,
  }]);
});

test("Paddle events with a bad signature or a different price are rejected", async () => {
  const secret = "pdl_ntfset_test_secret";
  const timestamp = "1785247200";
  const event = {
    event_id: "evt_01k1a2b3c4d5e6f7g8h9j0k1m3",
    event_type: "subscription.updated",
    occurred_at: "2026-07-28T14:00:00.000Z",
    data: {
      id: "sub_01k1a2b3c4d5e6f7g8h9j0k1m3",
      customer_id: "ctm_01k1a2b3c4d5e6f7g8h9j0k1m3",
      status: "active",
      custom_data: { household_id: "d70c8675-0261-4797-b6df-4109c3d678cd" },
      items: [{ price: { id: "pri_wrong_product" } }],
      current_billing_period: { ends_at: "2026-08-28T14:00:00.000Z" },
    },
  };
  const body = JSON.stringify(event);
  const signature = createHmac("sha256", secret)
    .update(`${timestamp}:${body}`, "utf8")
    .digest("hex");
  let applyCount = 0;
  const dependencies = {
    expectedPriceId: "pri_01k1a2b3c4d5e6f7g8h9j0k1m2",
    managedRequestLimit: 1_000,
    now: () => new Date("2026-07-28T14:00:03.000Z"),
    webhookSecret: secret,
    async applySubscriptionEvent() {
      applyCount += 1;
      return { applied: true };
    },
  };

  const badSignature = await handlePaddleWebhook(new Request("https://luna.test/paddle-webhook", {
    method: "POST",
    headers: { "paddle-signature": `ts=${timestamp};h1=not-the-signature` },
    body,
  }), dependencies);
  assert.equal(badSignature.status, 401);

  const wrongPrice = await handlePaddleWebhook(new Request("https://luna.test/paddle-webhook", {
    method: "POST",
    headers: { "paddle-signature": `ts=${timestamp};h1=${signature}` },
    body,
  }), dependencies);
  assert.equal(wrongPrice.status, 400);
  assert.equal(applyCount, 0);
});
