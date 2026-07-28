type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>;

export function createPaddleBillingClient(configuration: {
  apiKey: string;
  managedPriceId: string;
  fetch: FetchLike;
}) {
  const apiBaseUrl = "https://sandbox-api.paddle.com";
  const request = async (path: string, body: Record<string, unknown>): Promise<Record<string, unknown>> => {
    const response = await configuration.fetch(
      `${apiBaseUrl}${path}`,
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${configuration.apiKey}`,
          "content-type": "application/json",
          "paddle-version": "1",
        },
        body: JSON.stringify(body),
      },
    );
    if (!response.ok) throw new Error("Paddle billing is temporarily unavailable.");
    const payload = await response.json() as { data?: unknown };
    if (!payload.data || typeof payload.data !== "object") {
      throw new Error("Paddle returned an invalid billing session.");
    }
    return payload.data as Record<string, unknown>;
  };

  return {
    async createCheckout(input: { householdId: string; email: string }) {
      const data = await request("/transactions", {
        items: [{ price_id: configuration.managedPriceId, quantity: 1 }],
        collection_mode: "automatic",
        custom_data: { household_id: input.householdId },
      });
      const checkout = data.checkout;
      if (
        typeof data.id !== "string"
        || !checkout
        || typeof checkout !== "object"
        || typeof (checkout as Record<string, unknown>).url !== "string"
      ) throw new Error("Paddle returned an invalid checkout session.");
      return {
        transactionId: data.id,
        url: (checkout as Record<string, unknown>).url as string,
      };
    },
    async createCustomerPortal(input: { customerId: string; subscriptionId: string }) {
      const data = await request(`/customers/${encodeURIComponent(input.customerId)}/portal-sessions`, {
        subscription_ids: [input.subscriptionId],
      });
      const urls = data.urls;
      if (
        !urls
        || typeof urls !== "object"
        || !((urls as Record<string, unknown>).general)
        || typeof (urls as { general: Record<string, unknown> }).general.overview !== "string"
      ) throw new Error("Paddle returned an invalid customer portal session.");
      return { url: (urls as { general: { overview: string } }).general.overview };
    },
    async getPaddleSubscription(subscriptionId: string) {
      const response = await configuration.fetch(
        `${apiBaseUrl}/subscriptions/${encodeURIComponent(subscriptionId)}`,
        {
          method: "GET",
          headers: {
            authorization: `Bearer ${configuration.apiKey}`,
            "paddle-version": "1",
          },
        },
      );
      if (!response.ok) throw new Error("Paddle subscription reconciliation is unavailable.");
      const payload = await response.json() as { data?: Record<string, unknown> };
      const data = payload.data;
      const period = data?.current_billing_period as Record<string, unknown> | null;
      if (
        !data
        || typeof data.status !== "string"
        || !["trialing", "active", "past_due", "paused", "canceled"].includes(data.status)
        || typeof data.updated_at !== "string"
      ) throw new Error("Paddle returned invalid subscription state.");
      const validUntil = period?.ends_at;
      return {
        status: data.status as "trialing" | "active" | "past_due" | "paused" | "canceled",
        updatedAt: data.updated_at,
        validUntil: typeof validUntil === "string" ? validUntil : null,
      };
    },
  };
}
