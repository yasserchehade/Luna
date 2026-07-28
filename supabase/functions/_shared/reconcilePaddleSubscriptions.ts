import type { PaddleSubscriptionEvent } from "./paddleWebhook.ts";

type SubscriptionReference = {
  householdId: string;
  customerId: string;
  subscriptionId: string;
};

export async function reconcilePaddleSubscriptions(dependencies: {
  maxBudgetUsd: number;
  listSubscriptions(): Promise<SubscriptionReference[]>;
  getPaddleSubscription(subscriptionId: string): Promise<{
    status: PaddleSubscriptionEvent["status"];
    updatedAt: string;
    validUntil: string | null;
  }>;
  applySubscriptionEvent(event: PaddleSubscriptionEvent): Promise<{ applied: boolean }>;
}): Promise<{ checked: number; applied: number; failed: number }> {
  let checked = 0;
  let applied = 0;
  let failed = 0;
  for (const subscription of await dependencies.listSubscriptions()) {
    checked += 1;
    try {
      const current = await dependencies.getPaddleSubscription(subscription.subscriptionId);
      const result = await dependencies.applySubscriptionEvent({
        eventId: `reconcile:${subscription.subscriptionId}:${current.updatedAt}`,
        eventType: "subscription.updated",
        occurredAt: current.updatedAt,
        householdId: subscription.householdId,
        customerId: subscription.customerId,
        subscriptionId: subscription.subscriptionId,
        status: current.status,
        validUntil: current.validUntil,
        maxBudgetUsd: dependencies.maxBudgetUsd,
      });
      if (result.applied) applied += 1;
    } catch {
      failed += 1;
    }
  }
  return { checked, applied, failed };
}
