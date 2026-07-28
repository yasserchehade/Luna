import assert from "node:assert/strict";
import test from "node:test";
import { reconcilePaddleSubscriptions } from "../../supabase/functions/_shared/reconcilePaddleSubscriptions";

test("Paddle reconciliation repairs missed subscription state through the same ordered event seam", async () => {
  const applied: unknown[] = [];
  const result = await reconcilePaddleSubscriptions({
    requestLimit: 1_000,
    async listSubscriptions() {
      return [{
        householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
        customerId: "ctm_01k1a2b3c4d5e6f7g8h9j0k1m2",
        subscriptionId: "sub_01k1a2b3c4d5e6f7g8h9j0k1m2",
      }];
    },
    async getPaddleSubscription(subscriptionId) {
      assert.equal(subscriptionId, "sub_01k1a2b3c4d5e6f7g8h9j0k1m2");
      return {
        status: "active",
        updatedAt: "2026-07-28T15:00:00.000Z",
        validUntil: "2026-08-28T15:00:00.000Z",
      };
    },
    async applySubscriptionEvent(event) {
      applied.push(event);
      return { applied: true };
    },
  });

  assert.deepEqual(result, { checked: 1, applied: 1, failed: 0 });
  assert.deepEqual(applied, [{
    eventId: "reconcile:sub_01k1a2b3c4d5e6f7g8h9j0k1m2:2026-07-28T15:00:00.000Z",
    eventType: "subscription.updated",
    occurredAt: "2026-07-28T15:00:00.000Z",
    householdId: "d70c8675-0261-4797-b6df-4109c3d678cd",
    customerId: "ctm_01k1a2b3c4d5e6f7g8h9j0k1m2",
    subscriptionId: "sub_01k1a2b3c4d5e6f7g8h9j0k1m2",
    status: "active",
    validUntil: "2026-08-28T15:00:00.000Z",
    requestLimit: 1_000,
  }]);
});
