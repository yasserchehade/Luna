import assert from "node:assert/strict";
import test from "node:test";
import {
  handleHouseholdBillingSession,
  type HouseholdBillingContext,
} from "../../supabase/functions/_shared/householdBillingSession";
import { createPaddleBillingClient } from "../../supabase/functions/_shared/paddleBillingClient";

test("an authenticated Household Organiser can create a Paddle sandbox checkout session", async () => {
  const context: HouseholdBillingContext = {
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    email: "organiser@example.com",
    externalCustomerId: null,
    externalSubscriptionId: null,
  };
  const pending: Array<{ householdId: string; transactionId: string }> = [];

  const response = await handleHouseholdBillingSession(new Request(
    "https://luna.test/household-billing-session",
    {
      method: "POST",
      headers: {
        authorization: "Bearer signed-member-token",
        "content-type": "application/json",
      },
      body: JSON.stringify({ action: "checkout" }),
    },
  ), {
    async authenticateOrganiser(request) {
      assert.equal(request.headers.get("authorization"), "Bearer signed-member-token");
      return context;
    },
    async createCheckout(request) {
      assert.deepEqual(request, {
        householdId: context.householdId,
        email: context.email,
      });
      return {
        transactionId: "txn_01k1a2b3c4d5e6f7g8h9j0k1m2",
        url: "https://pay.paddle.io/checkout/test-session",
      };
    },
    async createCustomerPortal() {
      throw new Error("portal must not be called");
    },
    async recordCheckoutPending(householdId, transactionId) {
      pending.push({ householdId, transactionId });
    },
  });

  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {
    url: "https://pay.paddle.io/checkout/test-session",
  });
  assert.deepEqual(pending, [{
    householdId: context.householdId,
    transactionId: "txn_01k1a2b3c4d5e6f7g8h9j0k1m2",
  }]);
});

test("a Household with a Billing Subscription receives a temporary Paddle customer-portal session", async () => {
  const context: HouseholdBillingContext = {
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    email: "organiser@example.com",
    externalCustomerId: "ctm_01k1a2b3c4d5e6f7g8h9j0k1m2",
    externalSubscriptionId: "sub_01k1a2b3c4d5e6f7g8h9j0k1m2",
  };

  const response = await handleHouseholdBillingSession(new Request(
    "https://luna.test/household-billing-session",
    {
      method: "POST",
      headers: { authorization: "Bearer signed-member-token" },
      body: JSON.stringify({ action: "portal" }),
    },
  ), {
    async authenticateOrganiser() {
      return context;
    },
    async createCheckout() {
      throw new Error("checkout must not be called");
    },
    async createCustomerPortal(request) {
      assert.deepEqual(request, {
        customerId: context.externalCustomerId,
        subscriptionId: context.externalSubscriptionId,
      });
      return { url: "https://customer-portal.paddle.com/session/test-session" };
    },
    async recordCheckoutPending() {
      throw new Error("checkout state must not be recorded");
    },
  });

  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), {
    url: "https://customer-portal.paddle.com/session/test-session",
  });
});

test("Paddle checkout receives only the managed price and opaque Household reference", async () => {
  const requests: Array<{ url: string; init?: RequestInit }> = [];
  const client = createPaddleBillingClient({
    apiKey: "pdl_sdbx_test_secret",
    managedPriceId: "pri_01k1a2b3c4d5e6f7g8h9j0k1m2",
    async fetch(url, init) {
      requests.push({ url: String(url), init });
      return new Response(JSON.stringify({
        data: {
          id: "txn_01k1a2b3c4d5e6f7g8h9j0k1m2",
          checkout: { url: "https://pay.paddle.io/checkout/test-session" },
        },
      }), { status: 201, headers: { "content-type": "application/json" } });
    },
  });

  assert.deepEqual(await client.createCheckout({
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    email: "organiser@example.com",
  }), {
    transactionId: "txn_01k1a2b3c4d5e6f7g8h9j0k1m2",
    url: "https://pay.paddle.io/checkout/test-session",
  });
  assert.equal(requests.length, 1);
  assert.equal(requests[0].url, "https://sandbox-api.paddle.com/transactions");
  assert.equal(new Headers(requests[0].init?.headers).get("authorization"), "Bearer pdl_sdbx_test_secret");
  assert.deepEqual(JSON.parse(String(requests[0].init?.body)), {
    items: [{ price_id: "pri_01k1a2b3c4d5e6f7g8h9j0k1m2", quantity: 1 }],
    collection_mode: "automatic",
    custom_data: { household_id: "d70c8675-0261-4797-b6df-4109c3d678cd" },
  });
  assert.doesNotMatch(String(requests[0].init?.body), /organiser@example\.com|card|password/i);
});
